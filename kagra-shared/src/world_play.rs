//! Live `WorldDoc` tick: WASD / look → walker + chase camera + collectathon loop.
//!
//! Shared-side. Matches collectathon `WalkInput` (camera-relative wish, sit
//! on heightfield, optional jump). Title → play → result lives here so
//! `python -m kagra.play_world` is one complete loop. Python `Walk.wish` /
//! `CharacterController` is the leftover VRM motor — documented, not copied,
//! and not Rapier.

use crate::action::{self, ActionGame};
use crate::action2d::{self, Action2dGame};
use crate::collectathon::{
    spawn_coins, spawn_stars, won, IsleGame, WalkInput, BODY_H, CAM_DISTANCE, CAM_HEIGHT,
    CAM_LOOK_Y, GRAVITY, JUMP_V, PICK_REACH, PLAYER_SPEED, STAR_XZ,
};
use crate::cook::{self, CookGame};
use crate::fight::{self, FightGame};
use crate::fish::{self, FishGame};
use crate::flow::{FlowAction, FlowPhase, GameFlow};
use crate::fps::{self, FpsGame};
use crate::game::GamePhase;
use crate::gltf_load::{head_world_pos, step_springs};
use crate::input::{PointerEvent, PointerPhase};
use crate::lookat;
use crate::morph;
use crate::novel::{self, NovelGame};
use crate::platformer::{self, PlatformGame};
use crate::puzzle::{self, PuzzleGame};
use crate::race::{self, RaceGame};
use crate::rhythm::{self, RhythmGame};
use crate::rpg::{self, RpgGame};
use crate::scene::{DrawList, Quad};
use crate::shop::{self, ShopGame};
use crate::sim::{self, SimGame};
use crate::sports::{self, SportsGame};
use crate::spring::SpringState;
use crate::sprite;
use crate::stealth::{self, StealthGame};
use crate::survival::{self, SurvivalGame};
use crate::td::{self, TdGame};
use crate::world_doc::{
    load_skinned, load_skinned_parts, walker_rest_min_y, WorldDoc, WorldEvent, WorldProp,
    WorldTimer, WorldWalker,
};
use crate::world_survivor::{self, SurvivorLoop};
use glam::Vec3;
use std::collections::HashMap;

/// Running play state around a dump document. `doc` is the JSON source of
/// truth after each tick (walker position/yaw + camera + live pickups).
#[derive(Clone, Debug)]
pub struct WorldPlay {
    pub doc: WorldDoc,
    pub input: WalkInput,
    pub look_yaw: f32,
    pub look_pitch: f32,
    pub game: IsleGame,
    pub action: ActionGame,
    pub action2d: Action2dGame,
    pub platform: PlatformGame,
    pub rpg: RpgGame,
    pub fps: FpsGame,
    pub td: TdGame,
    pub race: RaceGame,
    pub fight: FightGame,
    pub novel: NovelGame,
    pub stealth: StealthGame,
    pub puzzle: PuzzleGame,
    pub sports: SportsGame,
    pub sim: SimGame,
    pub survival: SurvivalGame,
    pub rhythm: RhythmGame,
    pub fish: FishGame,
    pub shop: ShopGame,
    pub cook: CookGame,
    /// Horde-prototype survivor loop (`world_survivor`). Inert unless
    /// the world carries a `survivor` spec.
    pub survivor_loop: SurvivorLoop,
    /// Title / pause / result for genres that opt in (`uses_flow`).
    pub flow: GameFlow,
    seed: WorldDoc,
    vy: f32,
    spring: SpringState,
    blink_t: f32,
    /// NPC animation (phase 2): last seen position per NPC walker id, so
    /// the next step can tell a moving NPC from a standing one.
    npc_prev: HashMap<String, [f32; 3]>,
    /// NPC animation: clip `(name, duration)` resolved per
    /// `spec|clip_name|anim`, cached so steps do not re-resolve clips.
    /// The skins themselves stay in `world_doc`'s parts cache.
    npc_clip_cache: HashMap<String, Option<(String, f32)>>,
    /// NPC animation: dt carried for far NPCs whose phase update is
    /// thinned to every other step (the carried dt is applied in full,
    /// so the average rate is unchanged).
    npc_carry: HashMap<String, f32>,
    /// NPC animation: step counter driving the far-NPC thinning.
    npc_tick: u64,
    /// The player walker's authored `clip_name`, captured on the first
    /// step (outer `None` = not captured yet). When the player also
    /// names a `move_clip`, this is the standing clip the step returns
    /// to — and keeps animating — once input stops.
    stand_clip: Option<Option<String>>,
}

impl WorldPlay {
    pub fn new(doc: WorldDoc) -> Self {
        let mut doc = doc;
        seed_collectathon_pickups(&mut doc);
        action::seed(&mut doc);
        action2d::seed(&mut doc);
        platformer::seed(&mut doc);
        rpg::seed(&mut doc);
        fps::seed(&mut doc);
        td::seed(&mut doc);
        race::seed(&mut doc);
        fight::seed(&mut doc);
        novel::seed(&mut doc);
        stealth::seed(&mut doc);
        puzzle::seed(&mut doc);
        sports::seed(&mut doc);
        sim::seed(&mut doc);
        survival::seed(&mut doc);
        rhythm::seed(&mut doc);
        fish::seed(&mut doc);
        shop::seed(&mut doc);
        cook::seed(&mut doc);
        let rpg_coins = rpg::is_rpg(&doc).then_some(doc.coins);
        refresh_coin_count(&mut doc);
        if let Some(coins) = rpg_coins {
            rpg::restore_coins(&mut doc, coins);
        }
        if survival::is_survival(&doc) {
            doc.coins = survival::NEED;
        }
        if rhythm::is_rhythm(&doc) {
            doc.coins = 0;
        }
        if fish::is_fish(&doc) {
            doc.coins = 0;
        }
        if shop::is_shop(&doc) {
            doc.coins = shop::START;
        }
        if cook::is_cook(&doc) {
            doc.coins = 0;
        }
        if fps::is_fps(&doc) {
            doc.coins = fps::MAG;
        }
        if td::is_td(&doc) {
            doc.coins = td::starting_coins(&doc);
        }
        let look_yaw = look_yaw_from_doc(&doc);
        let action = ActionGame::from_doc(&doc);
        let action2d_game = Action2dGame::from_doc(&doc);
        let platform = PlatformGame::from_doc(&doc);
        let rpg_game = RpgGame::from_doc(&doc);
        let fps_game = FpsGame::from_doc(&doc);
        let td_game = TdGame::from_doc(&doc);
        let race_game = RaceGame::from_doc(&doc);
        let fight_game = FightGame::from_doc(&doc);
        let novel_game = NovelGame::from_doc(&doc);
        let stealth_game = StealthGame::from_doc(&doc);
        let puzzle_game = PuzzleGame::from_doc(&doc);
        let sports_game = SportsGame::from_doc(&doc);
        let sim_game = SimGame::from_doc(&doc);
        let survival_game = SurvivalGame::from_doc(&doc);
        let rhythm_game = RhythmGame::from_doc(&doc);
        let fish_game = FishGame::from_doc(&doc);
        let shop_game = ShopGame::from_doc(&doc);
        let cook_game = CookGame::from_doc(&doc);
        // Survivor loop: read the pool under authored names first,
        // then park it off-stage so the seed doc starts parked too.
        let survivor_loop = SurvivorLoop::from_doc(&doc);
        world_survivor::park_pool(&mut doc);
        if action2d::is_action2d(&doc) {
            action2d::place_side_camera(&mut doc);
        }
        if fps::is_fps(&doc) {
            fps::place_eye_camera(&mut doc, look_yaw, 0.0);
        }
        if td::is_td(&doc) {
            td::place_overview_camera(&mut doc);
        }
        if race::is_race(&doc) {
            race::place_chase_camera(&mut doc);
        }
        if fight::is_fight(&doc) {
            fight::place_dual_camera(&mut doc);
        }
        if novel::is_novel(&doc) {
            novel::place_room_camera(&mut doc);
        }
        if stealth::is_stealth(&doc) {
            stealth::place_room_camera(&mut doc);
        }
        if puzzle::is_puzzle(&doc) {
            puzzle::place_room_camera(&mut doc);
        }
        if sports::is_sports(&doc) {
            sports::place_chase_camera(&mut doc);
        }
        if sim::is_sim(&doc) {
            sim::place_chase_camera(&mut doc);
        }
        if survival::is_survival(&doc) {
            survival::place_chase_camera(&mut doc);
        }
        if rhythm::is_rhythm(&doc) {
            rhythm::place_stage_camera(&mut doc);
        }
        if fish::is_fish(&doc) {
            fish::place_dock_camera(&mut doc);
        }
        if shop::is_shop(&doc) {
            shop::place_stall_camera(&mut doc);
        }
        if cook::is_cook(&doc) {
            cook::place_stove_camera(&mut doc);
        }
        doc.refresh_asset_status();
        let game_id = doc.genre.clone().unwrap_or_default();
        let game = if is_collectathon(&doc)
            || action::is_action(&doc)
            || action2d::is_action2d(&doc)
            || platformer::is_platformer(&doc)
            || rpg::is_rpg(&doc)
            || fps::is_fps(&doc)
            || td::is_td(&doc)
            || race::is_race(&doc)
            || fight::is_fight(&doc)
            || novel::is_novel(&doc)
            || stealth::is_stealth(&doc)
            || puzzle::is_puzzle(&doc)
            || sports::is_sports(&doc)
            || sim::is_sim(&doc)
            || survival::is_survival(&doc)
            || rhythm::is_rhythm(&doc)
            || fish::is_fish(&doc)
            || shop::is_shop(&doc)
            || cook::is_cook(&doc)
        {
            IsleGame::default()
        } else {
            let mut g = IsleGame::default();
            g.start();
            g
        };
        Self {
            seed: doc.clone(),
            doc,
            input: WalkInput::default(),
            look_yaw,
            look_pitch: 0.0,
            game,
            action,
            action2d: action2d_game,
            platform,
            rpg: rpg_game,
            fps: fps_game,
            td: td_game,
            race: race_game,
            fight: fight_game,
            novel: novel_game,
            stealth: stealth_game,
            puzzle: puzzle_game,
            sports: sports_game,
            sim: sim_game,
            survival: survival_game,
            rhythm: rhythm_game,
            fish: fish_game,
            shop: shop_game,
            cook: cook_game,
            survivor_loop,
            flow: GameFlow::new(game_id),
            vy: 0.0,
            spring: SpringState::default(),
            blink_t: 0.0,
            npc_prev: HashMap::new(),
            npc_clip_cache: HashMap::new(),
            npc_carry: HashMap::new(),
            npc_tick: 0,
            stand_clip: None,
        }
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        Ok(Self::new(WorldDoc::from_json(json)?))
    }

    /// Title / result: Space or Enter. Playing ignores this (Space is jump).
    /// Pause uses the flow buttons so confirm does not restart a paused run.
    pub fn confirm(&mut self) {
        if self.uses_flow() && self.flow.phase == FlowPhase::Pause {
            return;
        }
        match self.game.phase {
            GamePhase::Title | GamePhase::Complete => {
                self.start();
                if self.uses_flow() {
                    self.flow.enter_play();
                }
            }
            GamePhase::Playing => {}
        }
    }

    pub fn uses_flow(&self) -> bool {
        self.is_platformer()
    }

    pub fn flow_paused(&self) -> bool {
        self.uses_flow() && self.flow.phase == FlowPhase::Pause
    }

    pub fn flow_phase_name(&self) -> &'static str {
        if self.uses_flow() {
            self.flow.phase.name()
        } else {
            match self.game.phase {
                GamePhase::Title => "title",
                GamePhase::Playing => "play",
                GamePhase::Complete => "result",
            }
        }
    }

    pub fn apply_flow_key(&mut self, key: &str) -> bool {
        if !self.uses_flow() {
            return false;
        }
        let Some(action) = self.flow.on_key(key) else {
            return false;
        };
        self.apply_flow_action(action);
        true
    }

    /// Raw pointer feed for survivor worlds: the floating touch
    /// stick and the level-up cards live in the survivor loop (the
    /// arena kits' stick, ported). The session routes every phase
    /// here; the loop itself gates on the run being live and on
    /// whether the cards are up. Inert on worlds without a
    /// `survivor` spec, so no other genre feels it.
    pub fn survivor_touch(&mut self, ev: PointerEvent, width: u32, height: u32) {
        // A release always counts (Moonlit clears its finger on
        // End/Cancel in every phase): a lift that lands on a phase
        // boundary must not leave the stick held.
        let release = matches!(ev.phase, PointerPhase::End | PointerPhase::Cancel);
        if self.has_survivor() && (self.game.is_playing() || release) {
            self.survivor_loop.touch(ev, width, height);
        }
    }

    pub fn apply_flow_pointer(&mut self, x: f32, y: f32, width: u32, height: u32) -> bool {
        if !self.uses_flow() {
            return false;
        }
        let Some(action) = self.flow.hit(x, y, width, height) else {
            return false;
        };
        self.apply_flow_action(action);
        true
    }

    pub fn request_pause(&mut self) -> bool {
        if self.uses_flow() && self.flow.phase == FlowPhase::Play && self.game.is_playing() {
            self.flow.pause();
            true
        } else {
            false
        }
    }

    fn apply_flow_action(&mut self, action: FlowAction) {
        if !self.uses_flow() {
            return;
        }
        match action {
            FlowAction::Start => self.confirm(),
            FlowAction::Retry => {
                self.flow.phase = FlowPhase::Title;
                self.game.phase = GamePhase::Complete;
                self.confirm();
            }
            FlowAction::Resume => self.flow.resume(),
            FlowAction::ToTitle => {
                self.flow.show_title();
                self.game.phase = GamePhase::Title;
                self.input = WalkInput::default();
            }
            FlowAction::Pause => {
                self.flow.pause();
            }
            FlowAction::VolumeUp => self.flow.bump_master(0.1),
            FlowAction::VolumeDown => self.flow.bump_master(-0.1),
            FlowAction::ToggleMute => self.flow.toggle_mute(),
        }
    }

    pub fn load_progress(&mut self) {
        if self.uses_flow() {
            self.flow.load_save();
        }
    }

    pub fn save_progress(&self) {
        if self.uses_flow() {
            let _ = self.flow.write_save();
        }
    }

    pub fn hud_json(&self, width: u32, height: u32) -> String {
        crate::flow::draw_list_json(&self.build_hud(width, height))
    }

    pub fn start(&mut self) {
        let best = self.game.best_score;
        self.doc = self.seed.clone();
        self.input = WalkInput::default();
        self.look_yaw = look_yaw_from_doc(&self.doc);
        self.look_pitch = 0.0;
        self.vy = 0.0;
        self.blink_t = 0.0;
        // NPC positions reset with the doc; a stale previous position
        // would read as a one-step teleport (a bogus movement speed).
        self.npc_prev.clear();
        self.npc_carry.clear();
        self.npc_tick = 0;
        self.game = IsleGame::default();
        self.game.best_score = best;
        self.game.start();
        self.action = ActionGame::from_doc(&self.doc);
        self.action2d = Action2dGame::from_doc(&self.doc);
        let ckpt = self.platform.checkpoint;
        self.platform = PlatformGame::from_doc(&self.doc);
        if ckpt.is_some() && self.is_platformer() {
            self.platform.checkpoint = ckpt;
            platformer::restore_checkpoint(&mut self.doc, &self.platform);
        }
        self.rpg = RpgGame::from_doc(&self.doc);
        self.fps = FpsGame::from_doc(&self.doc);
        self.td = TdGame::from_doc(&self.doc);
        self.race = RaceGame::from_doc(&self.doc);
        self.fight = FightGame::from_doc(&self.doc);
        self.novel = NovelGame::from_doc(&self.doc);
        self.stealth = StealthGame::from_doc(&self.doc);
        self.puzzle = PuzzleGame::from_doc(&self.doc);
        self.sports = SportsGame::from_doc(&self.doc);
        self.sim = SimGame::from_doc(&self.doc);
        self.survival = SurvivalGame::from_doc(&self.doc);
        self.rhythm = RhythmGame::from_doc(&self.doc);
        self.fish = FishGame::from_doc(&self.doc);
        self.shop = ShopGame::from_doc(&self.doc);
        self.cook = CookGame::from_doc(&self.doc);
        // The seed doc is already parked (see `new`), so a plain
        // rebuild resets the survivor run.
        self.survivor_loop = SurvivorLoop::from_doc(&self.doc);
        if action2d::is_action2d(&self.doc) {
            action2d::place_side_camera(&mut self.doc);
        }
        if fps::is_fps(&self.doc) {
            fps::place_eye_camera(&mut self.doc, self.look_yaw, self.look_pitch);
        }
        if td::is_td(&self.doc) {
            td::place_overview_camera(&mut self.doc);
        }
        if race::is_race(&self.doc) {
            race::place_chase_camera(&mut self.doc);
        }
        if fight::is_fight(&self.doc) {
            fight::place_dual_camera(&mut self.doc);
        }
        if novel::is_novel(&self.doc) {
            novel::place_room_camera(&mut self.doc);
        }
        if stealth::is_stealth(&self.doc) {
            stealth::place_room_camera(&mut self.doc);
        }
        if puzzle::is_puzzle(&self.doc) {
            puzzle::place_room_camera(&mut self.doc);
        }
        if sports::is_sports(&self.doc) {
            sports::place_chase_camera(&mut self.doc);
        }
        if sim::is_sim(&self.doc) {
            sim::place_chase_camera(&mut self.doc);
        }
        if survival::is_survival(&self.doc) {
            survival::place_chase_camera(&mut self.doc);
        }
        if rhythm::is_rhythm(&self.doc) {
            rhythm::place_stage_camera(&mut self.doc);
        }
        if fish::is_fish(&self.doc) {
            fish::place_dock_camera(&mut self.doc);
        }
        if shop::is_shop(&self.doc) {
            shop::place_stall_camera(&mut self.doc);
        }
        if cook::is_cook(&self.doc) {
            cook::place_stove_camera(&mut self.doc);
        }
        let rpg_coins = rpg::is_rpg(&self.doc).then_some(self.doc.coins);
        refresh_coin_count(&mut self.doc);
        if let Some(coins) = rpg_coins {
            rpg::restore_coins(&mut self.doc, coins);
        }
        if survival::is_survival(&self.doc) {
            self.doc.coins = survival::NEED;
        }
        if rhythm::is_rhythm(&self.doc) {
            self.doc.coins = 0;
        }
        if fish::is_fish(&self.doc) {
            self.doc.coins = 0;
        }
        if shop::is_shop(&self.doc) {
            self.doc.coins = shop::START;
        }
        if cook::is_cook(&self.doc) {
            self.doc.coins = 0;
        }
        if fps::is_fps(&self.doc) {
            self.doc.coins = fps::MAG;
        }
        if td::is_td(&self.doc) {
            self.doc.coins = td::starting_coins(&self.doc);
        }
    }

    pub fn is_collectathon(&self) -> bool {
        is_collectathon(&self.doc) || is_collectathon(&self.seed)
    }

    pub fn is_action(&self) -> bool {
        (action::is_action(&self.doc) || action::is_action(&self.seed)) && !self.is_action2d()
    }

    pub fn is_action2d(&self) -> bool {
        action2d::is_action2d(&self.doc) || action2d::is_action2d(&self.seed)
    }

    pub fn is_platformer(&self) -> bool {
        platformer::is_platformer(&self.doc) || platformer::is_platformer(&self.seed)
    }

    pub fn is_rpg(&self) -> bool {
        rpg::is_rpg(&self.doc) || rpg::is_rpg(&self.seed)
    }

    pub fn is_sprite(&self) -> bool {
        sprite::is_sprite(&self.doc) || sprite::is_sprite(&self.seed)
    }

    pub fn is_fps(&self) -> bool {
        fps::is_fps(&self.doc) || fps::is_fps(&self.seed)
    }

    pub fn is_td(&self) -> bool {
        td::is_td(&self.doc) || td::is_td(&self.seed)
    }

    pub fn is_race(&self) -> bool {
        race::is_race(&self.doc) || race::is_race(&self.seed)
    }

    pub fn is_fight(&self) -> bool {
        fight::is_fight(&self.doc) || fight::is_fight(&self.seed)
    }

    pub fn is_novel(&self) -> bool {
        novel::is_novel(&self.doc) || novel::is_novel(&self.seed)
    }

    pub fn is_stealth(&self) -> bool {
        stealth::is_stealth(&self.doc) || stealth::is_stealth(&self.seed)
    }

    pub fn is_puzzle(&self) -> bool {
        puzzle::is_puzzle(&self.doc) || puzzle::is_puzzle(&self.seed)
    }

    pub fn is_sports(&self) -> bool {
        sports::is_sports(&self.doc) || sports::is_sports(&self.seed)
    }

    pub fn is_sim(&self) -> bool {
        sim::is_sim(&self.doc) || sim::is_sim(&self.seed)
    }

    pub fn is_survival(&self) -> bool {
        survival::is_survival(&self.doc) || survival::is_survival(&self.seed)
    }

    /// Horde-prototype survivor loop present (`survivor` spec block).
    pub fn has_survivor(&self) -> bool {
        self.doc.survivor.is_some()
    }

    pub fn is_rhythm(&self) -> bool {
        rhythm::is_rhythm(&self.doc) || rhythm::is_rhythm(&self.seed)
    }

    pub fn is_fish(&self) -> bool {
        fish::is_fish(&self.doc) || fish::is_fish(&self.seed)
    }

    pub fn is_shop(&self) -> bool {
        shop::is_shop(&self.doc) || shop::is_shop(&self.seed)
    }

    pub fn is_cook(&self) -> bool {
        cook::is_cook(&self.doc) || cook::is_cook(&self.seed)
    }

    /// Named glTF/VRM walker dump (Emma) that is not Crest / FPS / TD / RPG.
    pub fn is_gltf_walk_play(&self) -> bool {
        if self.is_collectathon() || self.is_fps() || self.is_td() || self.is_rpg() {
            return false;
        }
        walker_has_gltf(&self.doc) || walker_has_gltf(&self.seed)
    }

    /// Arrows add to WalkInput (not look). Crest/FPS keep arrows as look.
    pub fn arrows_move_walker(&self) -> bool {
        self.is_race()
            || self.is_fight()
            || self.is_stealth()
            || self.is_puzzle()
            || self.is_sports()
            || self.is_sim()
            || self.is_action2d()
            || self.is_gltf_walk_play()
    }
    /// Mouse / arrow look. Pitch is clamped.
    pub fn add_look(&mut self, dyaw: f32, dpitch: f32) {
        self.look_yaw += dyaw;
        self.look_pitch = (self.look_pitch + dpitch).clamp(-0.7, 0.55);
    }

    /// Advance walker + chase camera + pickups. `dt` is seconds (clamped).
    pub fn tick(&mut self, dt: f32) {
        puffin::GlobalProfiler::lock().new_frame();
        puffin::profile_function!();
        let dt = dt.clamp(0.0, 0.05);
        if dt > 0.0 {
            crate::mixer::tick(dt);
        }
        if dt <= 0.0 {
            return;
        }
        if !self.game.is_playing() {
            return;
        }
        if self.flow_paused() {
            return;
        }
        if self.has_survivor() && self.survivor_loop.level_pending {
            // Level-up cards are up: the whole world holds still so
            // the choice is made in safety. The HUD and the card taps
            // live outside this step, so the screen stays alive.
            return;
        }
        let mut input = self.input.clamped();
        if self.has_survivor() {
            // The floating stick, while a thumb is down, steers in
            // place of the shell input (keys / pad still set the
            // fallback the loop falls back to on release).
            if let Some((lx, lz)) = self.survivor_loop.stick_vector() {
                input.lx = lx;
                input.lz = lz;
            }
        }
        let walked = if self.is_stealth() {
            stealth::locomotion(input)
        } else {
            input
        };
        if self.is_action2d() {
            action2d::tick(&mut self.doc, &mut self.action2d, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.action2d.dead || self.action2d.won {
                self.game.phase = GamePhase::Complete;
                self.game.score = self.action2d.kills * 250 + self.action2d.hits * 20;
            }
            return;
        }
        if self.is_rpg() {
            let mut walk = input;
            walk.jump = false;
            if self.rpg.blocks_walk() {
                walk.lx = 0.0;
                walk.lz = 0.0;
            }
            self.step_walker(walk, dt);
            rpg::tick(&mut self.doc, &mut self.rpg, input, dt);
            self.follow_camera();
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            self.input.dodge = false;
            if self.rpg.won || self.rpg.lost {
                self.game.phase = GamePhase::Complete;
            }
            return;
        }
        if self.is_platformer() && input.jump {
            let grounded = self
                .doc
                .player
                .as_ref()
                .map(|w| w.on_ground)
                .unwrap_or(false);
            if grounded {
                crate::mixer::cue_sfx("jump");
            }
        }
        self.step_walker(walked, dt);
        self.follow_camera();
        if self.is_action() {
            action::tick(&mut self.doc, &mut self.action, input, self.look_yaw, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            self.input.dodge = false;
            if self.action.dead || self.action.won {
                self.game.phase = GamePhase::Complete;
                self.game.score = self.action.kills * 250 + self.action.hits * 20;
            }
            return;
        }
        if self.is_platformer() {
            platformer::tick(&mut self.doc, &mut self.platform, &mut self.vy, input, dt);
            self.follow_camera();
            self.game.time_s += dt;
            self.input.jump = false;
            if self.platform.dead || self.platform.won {
                self.game.phase = GamePhase::Complete;
                let mut score = self.platform.landed * 50;
                if self.platform.won {
                    score += 1000;
                }
                self.game.score = score;
                let subtitle = if self.platform.won { "GOAL" } else { "FELL" };
                self.flow.finish(score, self.platform.landed, subtitle);
            }
            return;
        }
        if self.is_fps() {
            self.vy = 0.0;
            fps::pin_player_capsule(&mut self.doc);
            fps::place_eye_camera(&mut self.doc, self.look_yaw, self.look_pitch);
            fps::tick(
                &mut self.doc,
                &mut self.fps,
                input,
                self.look_yaw,
                self.look_pitch,
                dt,
            );
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            self.input.dodge = false;
            if self.fps.won || self.fps.lost {
                self.game.phase = GamePhase::Complete;
                self.game.score = self.fps.kills * 250 + self.fps.hits * 20;
            }
            return;
        }
        if self.is_td() {
            td::place_overview_camera(&mut self.doc);
            td::tick(&mut self.doc, &mut self.td, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.td.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = self.td.kills * 250 + self.td.hits * 20;
            }
            return;
        }
        if self.is_race() {
            race::tick(&mut self.doc, &mut self.race, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.race.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = self.race.laps * 250;
            }
            return;
        }
        if self.is_fight() {
            fight::tick(&mut self.doc, &mut self.fight, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.fight.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = self.fight.hits * 250;
            }
            return;
        }
        if self.is_novel() {
            novel::tick(&mut self.doc, &mut self.novel, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.novel.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = if self.novel.choice == 0 { 1 } else { 2 };
            }
            return;
        }
        if self.is_stealth() {
            stealth::tick(&mut self.doc, &mut self.stealth, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            self.input.dodge = false;
            if self.stealth.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = if self.stealth.clear { 250 } else { 0 };
            }
            return;
        }
        if self.is_puzzle() {
            puzzle::tick(&mut self.doc, &mut self.puzzle, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.puzzle.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = if self.puzzle.solved { 250 } else { 0 };
            }
            return;
        }
        if self.is_sports() {
            sports::tick(&mut self.doc, &mut self.sports, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.sports.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = if self.sports.scored { 250 } else { 0 };
            }
            return;
        }
        if self.is_sim() {
            sim::tick(&mut self.doc, &mut self.sim, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.sim.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = if self.sim.full { 250 } else { 0 };
            }
            return;
        }
        if self.is_survival() {
            survival::tick(&mut self.doc, &mut self.survival, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.survival.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = if self.survival.ok { 250 } else { 0 };
            }
            return;
        }
        if self.is_rhythm() {
            rhythm::tick(&mut self.doc, &mut self.rhythm, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.rhythm.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = if self.rhythm.clear { 250 } else { 0 };
            }
            return;
        }
        if self.is_fish() {
            fish::tick(&mut self.doc, &mut self.fish, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.fish.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = if self.fish.caught { 250 } else { 0 };
            }
            return;
        }
        if self.is_shop() {
            shop::tick(&mut self.doc, &mut self.shop, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.shop.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = if self.shop.bought { 250 } else { 0 };
            }
            return;
        }
        if self.is_cook() {
            cook::tick(&mut self.doc, &mut self.cook, input, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            if self.cook.done {
                self.game.phase = GamePhase::Complete;
                self.game.score = if self.cook.cooked { 250 } else { 0 };
            }
            return;
        }
        if self.has_survivor() {
            let spec = self.doc.survivor.clone().unwrap_or_default();
            if world_survivor::step(&mut self.doc, &mut self.survivor_loop, &spec, dt) {
                self.game.score =
                    self.survivor_loop.kills * 250 + self.survivor_loop.elapsed as u32;
                self.game.phase = GamePhase::Complete;
                self.input.jump = false;
                return;
            }
        }
        self.tick_timers(dt);
        self.step_interact();
        self.collect_pickups();
        self.game.time_s += dt;
        self.input.jump = false;
        if won(self.game.stars) {
            self.game
                .finish(self.game.stars, self.game.coins, self.game.time_s);
        }
    }

    /// Font-free HUD: title band / star+coin pips / result band.
    pub fn build_hud(&self, width: u32, height: u32) -> DrawList {
        if self.is_action2d() {
            return action2d::build_hud(&self.action2d, self.game.phase, width, height);
        }
        if self.is_action() {
            return action::build_hud(&self.action, self.game.phase, width, height);
        }
        if self.is_platformer() {
            let mut list = platformer::build_hud(&self.platform, self.game.phase, width, height);
            // Flow owns title / pause / result. The genre slabs would sit on
            // the same buttons. A `start()` that never confirmed stays in
            // Play for the unit tests, so those pips stay.
            let flow_covers = self.uses_flow()
                && match self.flow.phase {
                    FlowPhase::Play => false,
                    FlowPhase::Title => !self.game.is_playing(),
                    FlowPhase::Pause | FlowPhase::Result => true,
                };
            if flow_covers {
                list.quads.clear();
            }
            self.flow.paint(&mut list, width, height);
            return list;
        }
        if self.is_rpg() {
            return rpg::build_hud(&self.rpg, self.game.phase, width, height);
        }
        if self.is_fps() {
            return fps::build_hud(&self.fps, self.game.phase, width, height);
        }
        if self.is_td() {
            return td::build_hud(&self.td, self.game.phase, width, height);
        }
        if self.is_race() {
            return race::build_hud(&self.race, self.game.phase, width, height);
        }
        if self.is_fight() {
            return fight::build_hud(&self.fight, self.game.phase, width, height);
        }
        if self.is_novel() {
            return novel::build_hud(&self.novel, self.game.phase, width, height);
        }
        if self.is_stealth() {
            return stealth::build_hud(&self.stealth, self.game.phase, width, height);
        }
        if self.is_puzzle() {
            return puzzle::build_hud(&self.puzzle, self.game.phase, width, height);
        }
        if self.is_sports() {
            return sports::build_hud(&self.sports, self.game.phase, width, height);
        }
        if self.is_sim() {
            return sim::build_hud(&self.sim, self.game.phase, width, height);
        }
        if self.is_survival() {
            return survival::build_hud(&self.survival, self.game.phase, width, height);
        }
        if self.is_rhythm() {
            return rhythm::build_hud(&self.rhythm, self.game.phase, width, height);
        }
        if self.is_fish() {
            return fish::build_hud(&self.fish, self.game.phase, width, height);
        }
        if self.is_shop() {
            return shop::build_hud(&self.shop, self.game.phase, width, height);
        }
        if self.is_cook() {
            return cook::build_hud(&self.cook, self.game.phase, width, height);
        }
        if self.has_survivor() {
            return world_survivor::build_hud(&self.survivor_loop, self.game.phase, width, height);
        }
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
        let pad = 16.0 * scale;
        let mut quads = Vec::new();

        if self
            .doc
            .player
            .as_ref()
            .and_then(|w| w.load_error.as_deref())
            .is_some_and(|e| !e.is_empty())
        {
            quads.push(Quad::new(
                pad,
                h - 32.0 * scale - pad,
                w - pad * 2.0,
                28.0 * scale,
                [180, 48, 40, 230],
            ));
        }
        match self.game.phase {
            GamePhase::Title => {
                quads.push(Quad::new(0.0, 0.0, w, h, [10, 14, 12, 150]));
                quads.push(Quad::new(
                    w * 0.18,
                    h * 0.28,
                    w * 0.64,
                    h * 0.18,
                    [18, 24, 18, 230],
                ));
                quads.push(Quad::new(
                    w * 0.32,
                    h * 0.58,
                    w * 0.36,
                    52.0 * scale,
                    [240, 196, 72, 255],
                ));
            }
            GamePhase::Playing => {
                let pip = 22.0 * scale;
                let gap = 6.0 * scale;
                if is_collectathon(&self.doc) {
                    for i in 0..STAR_XZ.len() {
                        let x = pad + i as f32 * (pip + gap);
                        let got = (i as u32) < self.game.stars;
                        quads.push(Quad::new(
                            x,
                            pad,
                            pip,
                            pip,
                            if got {
                                [240, 196, 72, 255]
                            } else {
                                [20, 24, 18, 150]
                            },
                        ));
                    }
                }
                // Coin pips only for collected coins. coins=0 → none (no gray row).
                if self.game.coins > 0 {
                    let coin_w = 8.0 * scale;
                    let y = if is_collectathon(&self.doc) {
                        pad + pip + 8.0 * scale
                    } else {
                        pad
                    };
                    for i in 0..self.game.coins.min(24) {
                        quads.push(Quad::new(
                            pad + i as f32 * (coin_w + 3.0 * scale),
                            y,
                            coin_w,
                            coin_w,
                            [255, 210, 70, 255],
                        ));
                    }
                }
                // Interact prompt: nearest usable prop within reach. Text-less
                // bar (font-free HUD); the label is the prompt/kind string.
                if let Some((i, _)) = self.nearest_interact() {
                    if let Some(it) = self.doc.props[i].interact.as_ref() {
                        let label = if it.prompt.is_empty() {
                            it.kind.clone()
                        } else {
                            it.prompt.clone()
                        };
                        let bw = (label.len() as f32 * 13.0 + 64.0) * scale;
                        let bh = 30.0 * scale;
                        quads.push(Quad::new(
                            (w - bw) * 0.5,
                            h - 64.0 * scale,
                            bw,
                            bh,
                            [34, 46, 38, 230],
                        ));
                    }
                }
            }
            GamePhase::Complete => {
                quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 16, 12, 210]));
                let bar = (self.game.score.min(2400) as f32 / 2400.0).clamp(0.08, 1.0);
                quads.push(Quad::new(
                    w * 0.22,
                    h * 0.40,
                    w * 0.56 * bar,
                    18.0 * scale,
                    [240, 196, 72, 255],
                ));
                quads.push(Quad::new(
                    w * 0.32,
                    h * 0.62,
                    w * 0.36,
                    48.0 * scale,
                    [70, 160, 110, 240],
                ));
            }
        }

        DrawList {
            clear: [130, 165, 205, 255],
            quads,
            ..Default::default()
        }
    }

    fn step_walker(&mut self, input: WalkInput, dt: f32) {
        let half = self.doc.half.max(4.0);
        let (s, c) = self.look_yaw.sin_cos();
        let fwd = Vec3::new(s, 0.0, c);
        // Camera sits at look - fwd*dist and looks along +fwd, so screen-right
        // is fwd × up = (-c, 0, s). +lx (D / ArrowRight) must strafe screen-right.
        let right = Vec3::new(-c, 0.0, s);
        let wish = right * input.lx + fwd * input.lz;
        let wish_len = wish.length();

        let (mut updated, mut x, mut y, mut z, mut yaw, mut on_ground) = {
            let Some(w) = player_ref(&self.doc) else {
                return;
            };
            (
                w.clone(),
                w.position[0],
                w.position[1],
                w.position[2],
                w.yaw,
                w.on_ground,
            )
        };
        if self.stand_clip.is_none() {
            self.stand_clip = Some(updated.clip_name.clone());
        }

        if wish_len > 0.08 {
            let dir = wish / wish_len;
            let speed = PLAYER_SPEED * wish_len.min(1.0);
            x += dir.x * speed * dt;
            z += dir.z * speed * dt;
            yaw = dir.x.atan2(dir.z);
            updated.clip += dt;
            // Engine-derived locomotion state. Genre names ("cast", "reel")
            // are not touched here; they only survive while standing still.
            updated.anim = "walk".into();
            if let Some(mc) = &updated.move_clip {
                updated.clip_name = Some(mc.clone());
            }
            updated.anim_blend += (1.0 - updated.anim_blend) * (dt * 8.0).min(1.0);
        } else {
            if updated.anim == "walk" {
                updated.anim = "idle".into();
            }
            let stand = self
                .stand_clip
                .as_ref()
                .and_then(|c| c.clone())
                .filter(|s| !s.trim().is_empty());
            match (stand, updated.move_clip.is_some()) {
                (Some(sc), true) => {
                    // Named standing clip (e.g. a rifle aim idle): keep
                    // its phase running and the pose blended in instead
                    // of freezing at phase 0 and decaying to bind pose.
                    updated.clip_name = Some(sc);
                    updated.clip += dt;
                    if let Some(spec) = npc_gltf_spec(&updated) {
                        if let Some((_, dur)) = npc_clip_info(&spec, &updated) {
                            if dur > 1e-6 {
                                updated.clip = updated.clip.rem_euclid(dur);
                            }
                        }
                    }
                    updated.anim_blend += (1.0 - updated.anim_blend) * (dt * 8.0).min(1.0);
                }
                _ => {
                    updated.clip = 0.0;
                    updated.anim_blend *= (1.0 - (dt * 8.0).min(1.0)).max(0.0);
                }
            }
        }
        let pad = 2.0;
        x = x.clamp(-half + pad, half - pad);
        z = z.clamp(-half + pad, half - pad);

        if input.jump && on_ground {
            self.vy = JUMP_V;
        }
        self.vy -= GRAVITY * dt;
        y += self.vy * dt;
        let ground = sit_y(&self.doc, slope_ground(&self.doc, x, z));
        if y <= ground {
            y = ground;
            self.vy = 0.0;
            on_ground = true;
        } else {
            on_ground = false;
        }

        updated.kind = "walker".into();
        updated.name = "player".into();
        updated.position = [x, y, z];
        updated.yaw = yaw;
        updated.face = yaw;
        updated.on_ground = on_ground;
        write_player(&mut self.doc, updated);
        self.step_hair(dt);
        self.step_morph(dt);
        self.step_seekers(dt);
        self.step_npc_walkers(dt);
    }

    /// Advance `seek` NPCs toward the player (horde prototype driver).
    ///
    /// A walker with `seek > 0` closes on the player at that speed,
    /// playing its locomotion clip (`move_clip`, default "walk"), until
    /// it is `seek_stop` meters out (default [`DEFAULT_SEEK_STOP`]);
    /// there it stops and plays its attack clip (`seek_attack`,
    /// default "attack"). A clip switch restarts the phase, so an
    /// attack starts on its first frame. Seekers always face the
    /// player and push each other apart, so a crowd rings the player
    /// instead of stacking into one body. Positions move before
    /// [`Self::step_npc_walkers`], so the phase pass reads the closing
    /// speed from the position delta and stride-scales the locomotion
    /// clip, exactly like shell-driven walkers.
    fn step_seekers(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let Some(player) = player_ref(&self.doc) else {
            return;
        };
        let player_id = player.id.clone();
        let pp = player.position;
        let half = self.doc.half.max(4.0);
        // Pass 1: approach, face the player, swap clips by range.
        for w in &mut self.doc.walkers {
            if w.id == player_id || w.seek <= 0.0 {
                continue;
            }
            let dx = pp[0] - w.position[0];
            let dz = pp[2] - w.position[2];
            let dist = (dx * dx + dz * dz).sqrt();
            let stop = if w.seek_stop > 0.0 {
                w.seek_stop
            } else {
                DEFAULT_SEEK_STOP
            };
            let yaw = dx.atan2(dz);
            w.yaw = yaw;
            w.face = yaw;
            let want = if dist > stop {
                let step = (w.seek * dt).min(dist - stop);
                if dist > 1e-6 {
                    w.position[0] += dx / dist * step;
                    w.position[2] += dz / dist * step;
                }
                w.move_clip.clone().unwrap_or_else(|| "walk".into())
            } else {
                w.seek_attack.clone().unwrap_or_else(|| "attack".into())
            };
            if w.clip_name.as_deref() != Some(want.as_str()) {
                w.clip_name = Some(want);
                w.clip = 0.0;
            }
            let pad = 1.0;
            w.position[0] = w.position[0].clamp(-half + pad, half - pad);
            w.position[2] = w.position[2].clamp(-half + pad, half - pad);
        }
        // Pass 2: pairwise separation, seekers only.
        let idx: Vec<usize> = self
            .doc
            .walkers
            .iter()
            .enumerate()
            .filter(|(_, w)| w.id != player_id && w.seek > 0.0)
            .map(|(i, _)| i)
            .collect();
        for a in 0..idx.len() {
            for b in (a + 1)..idx.len() {
                let (i, j) = (idx[a], idx[b]);
                let (dx, dz) = {
                    let wi = &self.doc.walkers[i];
                    let wj = &self.doc.walkers[j];
                    (
                        wj.position[0] - wi.position[0],
                        wj.position[2] - wi.position[2],
                    )
                };
                let d2 = dx * dx + dz * dz;
                if d2 >= SEEK_SEPARATION * SEEK_SEPARATION || d2 < 1e-9 {
                    continue;
                }
                let d = d2.sqrt();
                let push = (SEEK_SEPARATION - d) * 0.5;
                let (ux, uz) = (dx / d, dz / d);
                self.doc.walkers[i].position[0] -= ux * push;
                self.doc.walkers[i].position[2] -= uz * push;
                self.doc.walkers[j].position[0] += ux * push;
                self.doc.walkers[j].position[2] += uz * push;
            }
        }
    }

    /// Advance every non-player skinned walker's clip phase (VRM phase 2).
    ///
    /// The player is stepped by [`Self::step_walker`] and never touched
    /// here. An NPC is animated when its model is a skinned mesh with a
    /// resolvable clip: rigid KayKit-style walkers keep their genre
    /// drivers (`action` / `sports` advance those clips themselves), and
    /// clip-less walkers keep the old fixed-pose behaviour.
    ///
    /// Rate: a moving NPC (position delta since the previous step)
    /// plays at `speed / PLAYER_SPEED`, the same stride scaling the
    /// player's `clip += dt` implies at full speed. A standing NPC with
    /// an explicit `clip_name` plays that clip at natural speed (an
    /// in-place `run` / `attack`), except idle-named clips, which — like
    /// any standing NPC — advance at [`NPC_IDLE_RATE`] so the walker
    /// idles instead of sticking to the rest pose. While a phase
    /// advances, `anim_blend` eases toward 1 with the player's own
    /// smoothing, so a world JSON no longer needs `anim_blend: 1.0` to
    /// show an NPC's clip pose; unlike the player, a standing NPC's
    /// blend never decays back to rest. NPCs named `down` / `ko`
    /// (the defeat conventions of `fps` / `action`) are left alone.
    ///
    /// Load: far NPCs (beyond [`NPC_FAR_DIST`]) update every other
    /// step with the skipped dt carried over, halving the (already
    /// cheap) phase writes without changing the average rate. Skinning
    /// itself stays in the renderer.
    fn step_npc_walkers(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        self.npc_tick += 1;
        let Some(player) = player_ref(&self.doc) else {
            return;
        };
        let player_id = player.id.clone();
        let player_pos = player.position;
        // Pass 1 (shared borrows of `doc`, disjoint field borrows for the
        // caches): decide each NPC's new clip / blend.
        let mut updates: Vec<(String, f32, f32)> = Vec::new();
        let live: Vec<String> = self
            .doc
            .walkers
            .iter()
            .filter(|w| w.id != player_id)
            .map(|w| w.id.clone())
            .collect();
        self.npc_prev.retain(|id, _| live.contains(id));
        self.npc_carry.retain(|id, _| live.contains(id));
        for w in &self.doc.walkers {
            if w.id == player_id || w.name == "down" || w.name == "ko" {
                continue;
            }
            // Movement since the previous step (horizontal, m/s).
            let speed = match self.npc_prev.get(&w.id) {
                Some(prev) => {
                    let dx = w.position[0] - prev[0];
                    let dz = w.position[2] - prev[2];
                    (dx * dx + dz * dz).sqrt() / dt
                }
                None => 0.0,
            };
            self.npc_prev.insert(w.id.clone(), w.position);
            let Some(spec) = npc_gltf_spec(w) else {
                continue;
            };
            let key = format!(
                "{}|{}|{}",
                spec,
                w.clip_name.as_deref().unwrap_or(""),
                w.anim
            );
            let info = self
                .npc_clip_cache
                .entry(key)
                .or_insert_with(|| npc_clip_info(&spec, w))
                .clone();
            let Some((clip_name, duration)) = info else {
                continue;
            };
            // Far-NPC thinning: bank the dt and update every other step.
            let dx = w.position[0] - player_pos[0];
            let dz = w.position[2] - player_pos[2];
            let far = (dx * dx + dz * dz).sqrt() > NPC_FAR_DIST;
            if far && self.npc_tick % 2 == 1 {
                *self.npc_carry.entry(w.id.clone()).or_insert(0.0) += dt;
                continue;
            }
            let eff_dt = dt + self.npc_carry.remove(&w.id).unwrap_or(0.0);
            let lower = clip_name.to_ascii_lowercase();
            let is_idle = lower == "idle" || lower.ends_with("_idle");
            let explicit = w
                .clip_name
                .as_deref()
                .map(str::trim)
                .is_some_and(|s| !s.is_empty());
            let rate = if speed > 0.15 {
                (speed / PLAYER_SPEED).clamp(0.25, 1.5)
            } else if explicit && !is_idle {
                1.0
            } else {
                NPC_IDLE_RATE
            };
            let advanced = w.clip + eff_dt * rate;
            let clip = if duration > 1e-6 {
                advanced.rem_euclid(duration)
            } else {
                advanced
            };
            let blend = w.anim_blend + (1.0 - w.anim_blend) * (eff_dt * 8.0).min(1.0);
            updates.push((w.id.clone(), clip, blend));
        }
        // Pass 2: write back.
        for (id, clip, blend) in updates {
            if let Some(w) = self.doc.walkers.iter_mut().find(|w| w.id == id) {
                w.clip = clip;
                w.anim_blend = blend;
            }
        }
    }

    fn step_hair(&mut self, dt: f32) {
        let (spec, clip) = {
            let Some(w) = player_ref(&self.doc) else {
                return;
            };
            let spec = w
                .gltf
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(w.model.trim())
                .to_string();
            if spec.is_empty() {
                return;
            }
            (spec, w.clip)
        };
        let Some(skin) = load_skinned(&spec) else {
            return;
        };
        if skin.springs.is_empty() {
            return;
        }
        let t = if clip <= 0.0 { None } else { Some(clip) };
        let hair = step_springs(&skin, &mut self.spring, t, dt);
        let Some(w) = player_ref(&self.doc) else {
            return;
        };
        let mut updated = w.clone();
        updated.hair = hair;
        write_player(&mut self.doc, updated);
    }

    fn step_morph(&mut self, dt: f32) {
        let (spec, expression) = {
            let Some(w) = player_ref(&self.doc) else {
                return;
            };
            let spec = w
                .gltf
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(w.model.trim())
                .to_string();
            if spec.is_empty() {
                return;
            }
            (spec, w.expression.clone())
        };
        // モーフは Face 等のパーツにある（Body は morphs 空でも expressions
        // を持つ）ので、全パーツで判定する。
        let Some(parts) = load_skinned_parts(&spec) else {
            return;
        };
        if !parts.iter().any(|p| !p.morphs.is_empty()) {
            return;
        }
        // "blink"（または未指定）= 自動まばたき。名前付き表情（smile /
        // angry / aa / ...）はモデルのいずれかのパーツが持っていれば重み 1.0。
        let morph = if expression.eq_ignore_ascii_case("blink") || expression.is_empty() {
            self.blink_t += dt;
            let mut m = morph::blink_weight(self.blink_t);
            if self.input.attack || self.rpg.talking {
                m = 1.0;
            }
            m
        } else if parts.iter().any(|p| p.expressions.has(&expression)) {
            1.0
        } else {
            0.0
        };
        let Some(w) = player_ref(&self.doc) else {
            return;
        };
        let mut updated = w.clone();
        updated.morph = morph;
        write_player(&mut self.doc, updated);
    }

    fn step_look(&mut self) {
        let (spec, pos, yaw) = {
            let Some(w) = player_ref(&self.doc) else {
                return;
            };
            let spec = w
                .gltf
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(w.model.trim())
                .to_string();
            if spec.is_empty() {
                return;
            }
            (spec, Vec3::from_array(w.position), w.yaw)
        };
        let Some(skin) = load_skinned(&spec) else {
            return;
        };
        if lookat::head_node(&skin.humanoid).is_none() {
            return;
        }
        let Some(cam) = self.doc.cameras.first() else {
            return;
        };
        let head = pos + head_world_pos(&skin);
        let cam_pos = Vec3::from_array(cam.position);
        let (ly, lp) = lookat::yaw_pitch_toward(head, cam_pos, yaw);
        let meta = skin.look_at.clone().unwrap_or_default();
        let (ly, lp) = lookat::clamp_head(&meta, ly, lp);
        let Some(w) = player_ref(&self.doc) else {
            return;
        };
        let mut updated = w.clone();
        updated.look_yaw = ly;
        updated.look_pitch = lp;
        write_player(&mut self.doc, updated);
    }

    fn follow_camera(&mut self) {
        let Some(w) = player_ref(&self.doc) else {
            return;
        };
        let look = Vec3::new(w.position[0], w.position[1] + CAM_LOOK_Y, w.position[2]);
        let (s, c) = self.look_yaw.sin_cos();
        let pitch = self.look_pitch;
        // A survivor world may pull the chase camera out and up
        // (its spec carries the framing); every other world keeps
        // the stock distance / height.
        let (mut dist, mut base_h) = (CAM_DISTANCE, CAM_HEIGHT);
        if let Some(spec) = &self.doc.survivor {
            dist = spec.cam_distance;
            base_h = spec.cam_height;
        }
        let height = base_h - CAM_LOOK_Y + pitch * 4.0;
        let eye = look + Vec3::new(-s * dist, height, -c * dist);
        let fov = self.doc.cameras.first().map(|c| c.fov).unwrap_or(54.0);
        if let Some(cam) = self.doc.cameras.first_mut() {
            cam.position = eye.to_array();
            cam.target = look.to_array();
        } else {
            self.doc.cameras.push(crate::world_doc::WorldCamera {
                id: "camera:main".into(),
                kind: "camera".into(),
                name: "main".into(),
                position: eye.to_array(),
                target: look.to_array(),
                fov,
                roll: 0.0,
            });
        }
        self.step_look();
    }

    fn collect_pickups(&mut self) {
        let Some(w) = player_ref(&self.doc) else {
            return;
        };
        let px = w.position[0];
        let pz = w.position[2];
        for prop in &mut self.doc.props {
            if !prop.enabled {
                continue;
            }
            if prop.name != "coin" && prop.name != "star" {
                continue;
            }
            let dx = px - prop.position[0];
            let dz = pz - prop.position[2];
            if (dx * dx + dz * dz).sqrt() <= PICK_REACH {
                prop.enabled = false;
            }
        }
        self.game.stars = self
            .doc
            .props
            .iter()
            .filter(|p| p.name == "star" && !p.enabled)
            .count() as u32;
        self.game.coins = self
            .doc
            .props
            .iter()
            .filter(|p| p.name == "coin" && !p.enabled)
            .count() as u32;
        refresh_coin_count(&mut self.doc);
    }

    /// Record an event (aggregated by name). Systems read it via
    /// [`Self::take_events`]. No callback bus — the dump is the bus.
    pub fn emit_event(&mut self, name: &str, data: Option<serde_json::Value>) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        if let Some(ev) = self.doc.events.iter_mut().find(|e| e.name == name) {
            ev.count += 1;
            if data.is_some() {
                ev.data = data;
            }
            return;
        }
        self.doc.events.push(WorldEvent {
            id: format!("event:{}", self.doc.events.len()),
            kind: "event".into(),
            name,
            count: 1,
            data,
        });
    }

    /// Take (consume) all events with this name. Each system handles a
    /// happening exactly once; the dump stops carrying it afterwards.
    pub fn take_events(&mut self, name: &str) -> Vec<WorldEvent> {
        let mut out = Vec::new();
        self.doc.events.retain(|e| {
            if e.name == name {
                out.push(e.clone());
                false
            } else {
                true
            }
        });
        out
    }

    /// Start (or restart) a named timer. The engine counts it down and emits
    /// `on_done` as an event when it reaches 0. Genre code calls this; the
    /// dump shows the countdown (`timers[]`) without any engine hook.
    pub fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        let id = format!("timer:{name}");
        if let Some(t) = self.doc.timers.iter_mut().find(|t| t.id == id) {
            t.name = name.into();
            t.seconds = seconds;
            t.remaining = seconds;
            t.on_done = on_done.map(|s| s.into());
            t.active = true;
            return id;
        }
        self.doc.timers.push(WorldTimer {
            id: id.clone(),
            kind: "timer".into(),
            name: name.into(),
            seconds,
            remaining: seconds,
            on_done: on_done.map(|s| s.into()),
            active: true,
        });
        id
    }

    /// Advance generic timers; a timer that hits 0 emits its `on_done` event
    /// and parks (`active = false`). Cast→bite, cook→done, cooldowns all map
    /// onto this. Genre code starts timers, the engine counts them down.
    pub fn tick_timers(&mut self, dt: f32) {
        let mut done: Vec<String> = Vec::new();
        for t in &mut self.doc.timers {
            if !t.active {
                continue;
            }
            t.remaining = (t.remaining - dt).max(0.0);
            if t.remaining <= 1e-4 {
                t.active = false;
                if let Some(ev) = t.on_done.clone() {
                    done.push(ev);
                }
            }
        }
        for ev in done {
            self.emit_event(&ev, None);
        }
    }

    /// Nearest enabled prop with interaction metadata within its own reach.
    /// Returns (prop index, distance). None = nothing usable nearby.
    pub fn nearest_interact(&self) -> Option<(usize, f32)> {
        let p = player_ref(&self.doc)?;
        let (px, pz) = (p.position[0], p.position[2]);
        let mut best: Option<(usize, f32)> = None;
        for (i, prop) in self.doc.props.iter().enumerate() {
            if !prop.enabled || prop.interact.is_none() {
                continue;
            }
            let reach = prop.interact.as_ref().map(|it| it.reach).unwrap_or(2.5);
            let dx = px - prop.position[0];
            let dz = pz - prop.position[2];
            let d = (dx * dx + dz * dz).sqrt();
            if d <= reach && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best
    }

    /// J/click on a reachable interactable emits its `on_use` event. Pure
    /// glue: genre code reacts by taking the event. No genre logic here.
    pub fn step_interact(&mut self) {
        if !self.input.attack {
            return;
        }
        let Some((i, _)) = self.nearest_interact() else {
            return;
        };
        let Some(on_use) = self.doc.props[i]
            .interact
            .as_ref()
            .and_then(|it| it.on_use.clone())
        else {
            return;
        };
        let (id, name) = (self.doc.props[i].id.clone(), self.doc.props[i].name.clone());
        self.emit_event(
            &on_use,
            Some(serde_json::json!({ "prop": id, "name": name })),
        );
    }
}

fn is_collectathon(doc: &WorldDoc) -> bool {
    // Genre dispatch is dump-only. Do not add another name-heuristic here.
    // New games stay in Python (`kagra.gameloop` + a free WorldDoc).
    doc.genre_is(crate::collectathon::GAME_ID) || doc.genre.as_deref() == Some("collectathon")
}

fn seed_collectathon_pickups(doc: &mut WorldDoc) {
    if !is_collectathon(doc) {
        return;
    }
    let stars = doc
        .props
        .iter()
        .filter(|p| p.name == "star" && p.enabled)
        .count();
    if stars >= STAR_XZ.len() {
        sit_pickups(doc);
        return;
    }
    doc.props.retain(|p| p.name != "star" && p.name != "coin");
    for (i, p) in spawn_stars().into_iter().enumerate() {
        let y = doc.height_at(p.x, p.z) + 1.55;
        let color = if i + 1 == STAR_XZ.len() {
            [255, 214, 70]
        } else if i % 2 == 0 {
            [220, 70, 70]
        } else {
            [70, 170, 90]
        };
        doc.props.push(WorldProp {
            id: format!("prop:star-{i}"),
            kind: "prop".into(),
            name: "star".into(),
            position: [p.x, y, p.z],
            model: "box".into(),
            scale: [0.55, 0.85, 0.12],
            enabled: true,
            color: Some(color),
            ..Default::default()
        });
    }
    for (i, p) in spawn_coins().into_iter().enumerate() {
        let y = doc.height_at(p.x, p.z) + 0.55;
        doc.props.push(WorldProp {
            id: format!("prop:coin-{i}"),
            kind: "prop".into(),
            name: "coin".into(),
            position: [p.x, y, p.z],
            yaw: i as f32 * 0.35,
            model: "sphere".into(),
            scale: [0.42, 0.08, 0.42],
            enabled: true,
            color: Some([255, 208, 64]),
            metallic: 1.0,
            roughness: 0.12,
            ..Default::default()
        });
    }
}

fn sit_pickups(doc: &mut WorldDoc) {
    let mut updates: Vec<(usize, f32, bool)> = Vec::new();
    for (i, prop) in doc.props.iter().enumerate() {
        if !prop.enabled {
            continue;
        }
        if prop.name != "star" && prop.name != "coin" {
            continue;
        }
        let extra = if prop.name == "star" { 1.55 } else { 0.55 };
        let y = doc.height_at(prop.position[0], prop.position[2]) + extra;
        updates.push((i, y, prop.name == "coin" && prop.metallic < 0.5));
    }
    for (i, y, metal) in updates {
        if let Some(prop) = doc.props.get_mut(i) {
            prop.position[1] = y;
            if metal {
                prop.metallic = 1.0;
                prop.roughness = 0.12;
            }
        }
    }
}

fn refresh_coin_count(doc: &mut WorldDoc) {
    doc.coins = doc
        .props
        .iter()
        .filter(|p| p.name == "coin" && p.enabled)
        .count() as u32;
}

/// Standing-NPC clip rate (× natural speed). Slow enough to read as an
/// idle sway rather than a walk, fast enough that the phase visibly moves.
const NPC_IDLE_RATE: f32 = 0.4;

/// Beyond this distance from the player (meters), an NPC's phase update
/// is thinned to every other step (dt carried, rate unchanged).
const NPC_FAR_DIST: f32 = 20.0;

/// Stop distance for a `seek` walker whose `seek_stop` is unset: close
/// enough for a melee swing to read, far enough to keep the bodies
/// from intersecting the player's.
const DEFAULT_SEEK_STOP: f32 = 1.7;

/// Minimum horizontal spacing the seek pass keeps between seekers.
const SEEK_SEPARATION: f32 = 0.85;

/// The glTF/VRM spec a walker renders with (`gltf` wins over `model`),
/// mirroring the player-side resolution in `step_hair`.
fn npc_gltf_spec(w: &WorldWalker) -> Option<String> {
    let spec = w
        .gltf
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(w.model.trim());
    if spec.is_empty() {
        None
    } else {
        Some(spec.to_string())
    }
}

/// `(clip name, duration)` for an NPC's spec, from the first skinned
/// part. `None` for clip-less models and for rigid (unskinned) parts,
/// whose clips genre code drives itself.
fn npc_clip_info(spec: &str, w: &WorldWalker) -> Option<(String, f32)> {
    let parts = load_skinned_parts(spec)?;
    let skin = parts.iter().find(|p| !p.skin_joints.is_empty())?;
    crate::gltf_load::walker_clip_info(skin, w.clip_name.as_deref(), &w.anim)
}

fn player_ref(doc: &WorldDoc) -> Option<&WorldWalker> {
    doc.player.as_ref().or(doc.walkers.first())
}

fn walker_has_gltf(doc: &WorldDoc) -> bool {
    let Some(w) = player_ref(doc) else {
        return false;
    };
    if w.gltf
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty())
    {
        return true;
    }
    let m = w.model.trim().to_ascii_lowercase();
    m.ends_with(".gltf") || m.ends_with(".glb") || m.ends_with(".vrm")
}

fn sit_y(doc: &WorldDoc, ground: f32) -> f32 {
    let Some(w) = player_ref(doc) else {
        return ground + BODY_H;
    };
    let spec = w
        .gltf
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    if spec.is_empty() {
        return ground + BODY_H;
    }
    ground - walker_rest_min_y(spec)
}

/// 足元リング（半径 r）の最大地形高。坂でカプセルの足が斜面に
/// 埋まらず浮かないようにする（0.19 の 8 点リング接地相当）。
fn slope_ground(doc: &WorldDoc, x: f32, z: f32) -> f32 {
    let r = 0.3;
    let mut g = doc.height_at(x, z);
    for (dx, dz) in [
        (r, 0.0),
        (-r, 0.0),
        (0.0, r),
        (0.0, -r),
        (r * 0.7, r * 0.7),
        (-r * 0.7, r * 0.7),
        (r * 0.7, -r * 0.7),
        (-r * 0.7, -r * 0.7),
    ] {
        g = g.max(doc.height_at(x + dx, z + dz));
    }
    g
}

fn write_player(doc: &mut WorldDoc, walker: WorldWalker) {
    if let Some(existing) = doc.player.as_mut() {
        *existing = walker.clone();
    } else {
        doc.player = Some(walker.clone());
    }
    let mut found = false;
    for w in &mut doc.walkers {
        if w.id == walker.id {
            *w = walker.clone();
            found = true;
        }
    }
    if !found {
        if let Some(first) = doc.walkers.first_mut() {
            *first = walker;
        } else {
            doc.walkers.push(walker);
        }
    }
}

fn look_yaw_from_doc(doc: &WorldDoc) -> f32 {
    let Some(cam) = doc.cameras.first() else {
        return 0.0;
    };
    let dx = cam.position[0] - cam.target[0];
    let dz = cam.position[2] - cam.target[2];
    // collectathon: eye = look + (-sin(yaw)*dist, …, -cos(yaw)*dist)
    (-dx).atan2(-dz)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectathon::{coin_path, BODY_H, STAR_NEED};
    use serde_json::json;

    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const ORB: &str = include_str!("../tests/fixtures/orb_rush_world.json");
    const FISH: &str = include_str!("../tests/fixtures/interact_fish_world.json");

    #[test]
    fn crest_emma_collectathon_ticks_vrm_walker() {
        const CREST_EMMA: &str = include_str!("../tests/fixtures/crest_emma_world.json");
        let mut play = WorldPlay::from_json(CREST_EMMA).expect("crest+emma dump");
        play.confirm(); // title → play
        let start = play.doc.player.as_ref().unwrap().position;
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..45 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap();
        let d = (Vec3::from_array(p.position) - Vec3::from_array(start)).length();
        assert!(d > 1.0, "VRM walker must walk the collectathon, d={d}");
        assert!(p.on_ground, "VRM walker sits on the grass");
        assert!(play.is_collectathon());
    }

    #[test]
    fn slope_ground_uses_foot_ring_max() {
        let doc = WorldDoc::from_json(&slope_dump()).expect("slope dump");
        // 中心 y=1.0 でも +x リングは y=2.0 → カプセルは斜面の高い側に接地
        assert!((slope_ground(&doc, 0.0, 0.0) - 2.0).abs() < 1e-5);
        // 平らな場所は中心のまま
        assert!((slope_ground(&doc, 5.0, 5.0) - 0.5).abs() < 1e-5);
    }

    fn slope_dump() -> String {
        serde_json::json!({
            "version": 1, "half": 16.0, "floor_y": 0.0, "gravity": 9.8, "water_y": null,
            "coins": 0, "player": null, "props": [], "walkers": [], "lights": [], "cameras": [],
            "heightfield": { "samples": [
                [0.0, 0.0, 1.0], [0.3, 0.0, 2.0], [-0.3, 0.0, 0.5],
                [0.0, 0.3, 1.2], [0.0, -0.3, 0.8], [5.0, 5.0, 0.5]
            ] }
        })
        .to_string()
    }

    #[test]
    fn wasd_tick_moves_walker_on_heightfield() {
        let mut play = WorldPlay::from_json(CREST).unwrap();
        play.start();
        let start = play.doc.player.as_ref().unwrap().position;
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..45 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap();
        let dx = p.position[0] - start[0];
        let dz = p.position[2] - start[2];
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(
            dist > 1.5,
            "WASD forward should move walker in WorldDoc, dist={dist} pos={:?}",
            p.position
        );
        assert!(p.on_ground, "tick sits on the named height fn");
        let ground = play.doc.height_at(p.position[0], p.position[2]) + BODY_H;
        assert!(
            (p.position[1] - ground).abs() < 0.05,
            "foot y {} vs ground {}",
            p.position[1],
            ground
        );
        let twin = play
            .doc
            .walkers
            .iter()
            .find(|w| w.id == "walker:player")
            .unwrap();
        assert_eq!(twin.position, p.position);
    }

    #[test]
    fn look_updates_camera_in_world_doc() {
        let mut play = WorldPlay::from_json(CREST).unwrap();
        play.start();
        let yaw0 = play.look_yaw;
        let eye0 = play.doc.cameras[0].position;
        play.add_look(0.6, 0.0);
        play.tick(1.0 / 60.0);
        assert!((play.look_yaw - yaw0 - 0.6).abs() < 1e-4);
        let eye = play.doc.cameras[0].position;
        let d = (eye[0] - eye0[0]).abs() + (eye[2] - eye0[2]).abs();
        assert!(d > 0.2, "chase camera should orbit, delta={d}");
        let tgt = play.doc.cameras[0].target;
        let p = play.doc.player.as_ref().unwrap().position;
        assert!((tgt[0] - p[0]).abs() < 0.05);
        assert!((tgt[2] - p[2]).abs() < 0.05);
    }

    #[test]
    fn strafe_and_idle_tick_orb_rush_floor() {
        let mut play = WorldPlay::from_json(ORB).unwrap();
        let start = play.doc.player.as_ref().unwrap().position;
        play.input = WalkInput {
            lx: 1.0,
            lz: 0.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap();
        assert!(
            (p.position[0] - start[0]).abs() > 0.4,
            "strafe should move x, got {:?}",
            p.position
        );
        play.input = WalkInput::default();
        let mid = p.position;
        play.tick(1.0 / 60.0);
        let p2 = play.doc.player.as_ref().unwrap();
        let drift = (p2.position[0] - mid[0]).abs() + (p2.position[2] - mid[2]).abs();
        assert!(drift < 0.02, "idle tick must not drift, {drift}");
    }

    #[test]
    fn title_does_not_walk_until_confirm() {
        let mut play = WorldPlay::from_json(CREST).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        let z = play.doc.player.as_ref().unwrap().position[2];
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        assert_eq!(play.doc.player.as_ref().unwrap().position[2], z);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap().position;
        let dist = ((p[0]).powi(2) + (p[2] - z).powi(2)).sqrt();
        assert!(
            dist > 1.0,
            "after start, WASD should move, dist={dist} pos={p:?}"
        );
    }

    #[test]
    fn picking_a_star_counts_and_six_finishes() {
        let mut play = WorldPlay::from_json(CREST).unwrap();
        play.start();
        let stars: Vec<_> = play
            .doc
            .props
            .iter()
            .filter(|p| p.name == "star" && p.enabled)
            .map(|p| (p.position[0], p.position[2]))
            .collect();
        assert!(
            stars.len() >= STAR_NEED as usize,
            "collectathon layout needs {} stars, got {}",
            STAR_NEED,
            stars.len()
        );
        assert!(
            play.doc.coins >= 8,
            "coins live in the dump, got {}",
            play.doc.coins
        );
        assert!(coin_path().len() >= 20);
        let first = stars[0];
        let y0 = play.doc.height_at(first.0, first.1) + BODY_H;
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [first.0, y0, first.1];
        }
        let walker = play.doc.player.clone().unwrap();
        write_player(&mut play.doc, walker);
        play.tick(1.0 / 60.0);
        assert_eq!(play.game.stars, 1);
        assert!(play
            .doc
            .props
            .iter()
            .any(|p| p.name == "star" && !p.enabled));
        let live_coins = play
            .doc
            .props
            .iter()
            .filter(|p| p.name == "coin" && p.enabled)
            .count();
        assert_eq!(play.doc.coins, live_coins as u32);

        let rest: Vec<_> = play
            .doc
            .props
            .iter()
            .filter(|p| p.name == "star" && p.enabled)
            .map(|p| (p.position[0], p.position[2]))
            .take(STAR_NEED as usize - 1)
            .collect();
        for (x, z) in rest {
            let y = play.doc.height_at(x, z) + BODY_H;
            if let Some(p) = play.doc.player.as_mut() {
                p.position = [x, y, z];
            }
            let walker = play.doc.player.clone().unwrap();
            write_player(&mut play.doc, walker);
            play.tick(1.0 / 60.0);
        }
        assert!(won(play.game.stars), "stars {}", play.game.stars);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert!(play.game.score > 0);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn crest_seed_sits_coins_on_heightfield() {
        let play = WorldPlay::from_json(CREST).unwrap();
        let coins: Vec<_> = play
            .doc
            .props
            .iter()
            .filter(|p| p.name == "coin" && p.enabled)
            .collect();
        assert!(coins.len() >= 8);
        for c in &coins {
            let ground = play.doc.height_at(c.position[0], c.position[2]);
            assert!(
                (c.position[1] - ground - 0.55).abs() < 0.05,
                "coin y {} vs ground {}",
                c.position[1],
                ground
            );
            assert!(c.metallic >= 0.5);
        }
        let player = play.doc.player.as_ref().unwrap();
        assert!(player.on_ground);
    }

    #[test]
    fn wasd_plays_walk_clip_idle_is_tpose() {
        const DUMP: &str = include_str!("../tests/fixtures/skinned_walker_world.json");
        let mut play = WorldPlay::from_json(DUMP).unwrap();
        play.start();
        let rest = play.doc.player.as_ref().unwrap().clip;
        assert_eq!(rest, 0.0);
        let rest_mesh = play
            .doc
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .unwrap()
            .1;
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        let w = play.doc.player.as_ref().unwrap();
        assert!(w.clip > 0.2, "WASD must advance walk clip, got {}", w.clip);
        assert_eq!(w.gltf.as_deref(), Some("walk_skinned.gltf"));
        let walk_mesh = play
            .doc
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .unwrap()
            .1;
        let mut max_d = 0.0f32;
        for (a, b) in rest_mesh.vertices.iter().zip(walk_mesh.vertices.iter()) {
            let d = (glam::Vec3::from_array(a.pos) - glam::Vec3::from_array(b.pos)).length();
            max_d = max_d.max(d);
        }
        assert!(
            max_d > 0.02,
            "walking must not be a T-pose statue, max_d={max_d}"
        );
        play.input = WalkInput::default();
        play.tick(1.0 / 60.0);
        assert_eq!(
            play.doc.player.as_ref().unwrap().clip,
            0.0,
            "idle returns to T-pose clip 0"
        );
    }

    /// Player + one NPC walker dump for the NPC-animation tests. The NPC
    /// object is passed in whole so each test controls its clip fields.
    fn npc_dump(npc: serde_json::Value) -> String {
        serde_json::json!({
            "version": 1, "half": 16.0, "floor_y": 0.0, "gravity": 9.8, "water_y": null,
            "coins": 0,
            "player": {
                "id": "walker:player", "type": "walker", "name": "player",
                "position": [0.0, 0.0, 0.0], "yaw": 0.0, "face": 0.0,
                "on_ground": true, "model": "capsule", "gltf": "walk_skinned.gltf",
                "clip": 0.0
            },
            "props": [],
            "walkers": [
                {
                    "id": "walker:player", "type": "walker", "name": "player",
                    "position": [0.0, 0.0, 0.0], "yaw": 0.0, "face": 0.0,
                    "on_ground": true, "model": "capsule", "gltf": "walk_skinned.gltf",
                    "clip": 0.0
                },
                npc
            ],
            "lights": [],
            "cameras": [{
                "id": "camera:main", "type": "camera", "name": "main",
                "position": [0.0, 2.4, 6.0], "target": [0.0, 0.9, 0.0], "fov": 54.0
            }],
            "heightfield": null
        })
        .to_string()
    }

    fn npc_walker<'a>(play: &'a WorldPlay, id: &str) -> &'a WorldWalker {
        play.doc.walkers.iter().find(|w| w.id == id).unwrap()
    }

    /// The NPC's skinned mesh (slot order is walker order: player, NPC).
    fn npc_mesh(doc: &WorldDoc) -> crate::scene3d::MeshData {
        doc.compile_meshes()
            .into_iter()
            .filter(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .nth(1)
            .unwrap()
            .1
    }

    #[test]
    fn npc_clip_name_advances_and_blends_without_json_blend() {
        // Phase 1 finding: an NPC needed `anim_blend: 1.0` in the world
        // JSON or its clip phase never showed. The step now advances the
        // phase and raises the blend itself for a `clip_name` NPC.
        let dump = npc_dump(json!({
            "id": "walker:npc", "type": "walker", "name": "npc",
            "position": [1.7, 0.0, -0.8], "yaw": -0.5, "face": -0.5,
            "on_ground": true, "model": "capsule", "gltf": "walk_skinned.gltf",
            "clip": 0.10, "clip_name": "Walk"
        }));
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        assert_eq!(npc_walker(&play, "walker:npc").anim_blend, 0.0);
        play.input = WalkInput::default();
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        let npc = npc_walker(&play, "walker:npc");
        // Natural rate: 0.10 + 20/60 s ≈ 0.43 (Walk duration is 1.0 s).
        assert!(
            npc.clip > 0.35 && npc.clip < 0.55,
            "NPC clip must advance in step, got {}",
            npc.clip
        );
        assert!(
            npc.anim_blend > 0.8,
            "step must raise NPC blend without a JSON anim_blend, got {}",
            npc.anim_blend
        );
        // The player is untouched by the NPC pass (idle stays T-pose).
        assert_eq!(play.doc.player.as_ref().unwrap().clip, 0.0);
        // And the NPC's sampled pose actually left the rest pose.
        let live = npc_mesh(&play.doc);
        let mut rest_doc = play.doc.clone();
        for w in &mut rest_doc.walkers {
            if w.id == "walker:npc" {
                w.clip = 0.0;
                w.anim_blend = 0.0;
            }
        }
        let rest = npc_mesh(&rest_doc);
        let mut max_d = 0.0f32;
        for (a, b) in rest.vertices.iter().zip(live.vertices.iter()) {
            let d = (glam::Vec3::from_array(a.pos) - glam::Vec3::from_array(b.pos)).length();
            max_d = max_d.max(d);
        }
        assert!(
            max_d > 0.02,
            "NPC must not stay a rest statue, max_d={max_d}"
        );
    }

    #[test]
    fn stationary_npc_idles_instead_of_sticking_to_rest() {
        // The phase-1 verify fixture: NPC has a fixed clip phase and no
        // clip_name / anim_blend at all. It must still animate (slowly)
        // and blend away from rest instead of freezing as a statue.
        const DUMP: &str = include_str!("../tests/fixtures/vrm_verify_world.json");
        let mut play = WorldPlay::from_json(DUMP).unwrap();
        play.start();
        let start = npc_walker(&play, "walker:pose").clip;
        assert_eq!(start, 0.45);
        play.input = WalkInput::default();
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        let npc = npc_walker(&play, "walker:pose");
        assert!(
            (npc.clip - start).abs() > 0.08,
            "stationary NPC phase must advance, {start} -> {}",
            npc.clip
        );
        // Mixamo walk is ~0.733 s; the phase stays wrapped inside it.
        assert!(
            npc.clip >= 0.0 && npc.clip < 0.74,
            "phase wraps at the clip duration, got {}",
            npc.clip
        );
        assert!(
            npc.anim_blend > 0.5,
            "stationary NPC must blend off rest, got {}",
            npc.anim_blend
        );
    }

    #[test]
    fn npc_clip_wraps_at_its_clip_duration() {
        let dump = npc_dump(json!({
            "id": "walker:npc", "type": "walker", "name": "npc",
            "position": [1.7, 0.0, -0.8], "yaw": 0.0, "face": 0.0,
            "on_ground": true, "model": "capsule", "gltf": "walk_skinned.gltf",
            "clip": 0.95, "clip_name": "Walk"
        }));
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        play.input = WalkInput::default();
        for _ in 0..12 {
            play.tick(1.0 / 60.0); // +0.2 s at natural rate → 1.15 → wraps
        }
        let clip = npc_walker(&play, "walker:npc").clip;
        assert!(
            clip > 0.05 && clip < 0.3,
            "phase must wrap at duration 1.0, got {clip}"
        );
    }

    #[test]
    fn moving_npc_advances_at_speed_scaled_rate() {
        // No clip_name: a walker whose position moves (shell / genre code
        // drives the dump) plays its default clip at speed-scaled rate,
        // well above the standing idle rate.
        let dump = npc_dump(json!({
            "id": "walker:npc", "type": "walker", "name": "npc",
            "position": [0.0, 0.0, -5.0], "yaw": 0.0, "face": 0.0,
            "on_ground": true, "model": "capsule", "gltf": "walk_skinned.gltf",
            "clip": 0.0
        }));
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        play.input = WalkInput::default();
        let dt = 1.0 / 60.0;
        for _ in 0..20 {
            if let Some(w) = play.doc.walkers.iter_mut().find(|w| w.id == "walker:npc") {
                w.position[2] += PLAYER_SPEED * dt;
            }
            play.tick(dt);
        }
        let clip = npc_walker(&play, "walker:npc").clip;
        assert!(
            clip > 0.25,
            "full-speed NPC must advance near dt rate (idle rate would \
             give ~0.13), got {clip}"
        );
    }

    #[test]
    fn clipless_npc_keeps_fixed_pose() {
        // Scope guard: a walker with no skinned clip (capsule) is not
        // animated by the NPC pass — its dump fields stay as authored.
        let dump = npc_dump(json!({
            "id": "walker:npc", "type": "walker", "name": "npc",
            "position": [1.7, 0.0, -0.8], "yaw": 0.0, "face": 0.0,
            "on_ground": true, "model": "capsule", "clip": 0.3
        }));
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        play.input = WalkInput::default();
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        let npc = npc_walker(&play, "walker:npc");
        assert_eq!(npc.clip, 0.3);
        assert_eq!(npc.anim_blend, 0.0);
    }

    #[test]
    fn seeker_closes_in_then_swaps_walk_for_attack() {
        // Horde prototype driver: a `seek` NPC walks at its seek speed
        // toward the player (at the origin), then stops at the default
        // stop distance and switches its clip to the attack clip.
        let dump = npc_dump(json!({
            "id": "walker:npc", "type": "walker", "name": "zombie",
            "position": [0.0, 0.0, -8.0], "yaw": 0.0, "face": 0.0,
            "on_ground": true, "model": "capsule", "gltf": "walk_skinned.gltf",
            "clip": 0.0, "clip_name": "walk", "seek": 2.0
        }));
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        play.input = WalkInput::default();
        let dt = 1.0 / 60.0;
        for _ in 0..60 {
            play.tick(dt);
        }
        let npc = npc_walker(&play, "walker:npc");
        assert!(
            (npc.position[2] + 6.0).abs() < 0.05,
            "seeker must close 2 m in 1 s at seek 2.0, z={}",
            npc.position[2]
        );
        assert_eq!(npc.clip_name.as_deref(), Some("walk"));
        // It faces the player (movement direction +Z => yaw 0).
        assert!(npc.yaw.abs() < 1e-4, "yaw {}", npc.yaw);
        // Run it in: it must stop at the stop ring, not inside the player.
        for _ in 0..600 {
            play.tick(dt);
        }
        let npc = npc_walker(&play, "walker:npc");
        let dist = (npc.position[0].powi(2) + npc.position[2].powi(2)).sqrt();
        assert!(
            (dist - DEFAULT_SEEK_STOP).abs() < 0.35,
            "seeker must hold the stop ring, dist={dist}"
        );
        assert_eq!(npc.clip_name.as_deref(), Some("attack"));
    }

    #[test]
    fn player_stand_clip_keeps_animating_and_move_clip_swaps() {
        // A player with clip_name + move_clip: standing plays the named
        // standing clip (phase advances, blend rises — no bind-pose
        // freeze), moving swaps to the move clip, standing again
        // restores the standing clip.
        let dump = {
            let mut v: serde_json::Value = serde_json::from_str(&npc_dump(json!({
                "id": "walker:npc", "type": "walker", "name": "npc",
                "position": [3.0, 0.0, -3.0], "yaw": 0.0, "face": 0.0,
                "on_ground": true, "model": "capsule", "clip": 0.0
            })))
            .unwrap();
            v["player"]["clip_name"] = json!("Walk");
            // (npc_dump duplicates the player into walkers[0]; set both.)
            v["player"]["move_clip"] = json!("Run");
            v["walkers"][0]["clip_name"] = json!("Walk");
            v["walkers"][0]["move_clip"] = json!("Run");
            v.to_string()
        };
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        play.input = WalkInput::default();
        let dt = 1.0 / 60.0;
        for _ in 0..30 {
            play.tick(dt);
        }
        let p = play.doc.player.as_ref().unwrap();
        assert!(
            p.clip > 0.2 && p.clip < 0.9,
            "standing clip phase must advance (wraps at 1.0), got {}",
            p.clip
        );
        assert!(
            p.anim_blend > 0.5,
            "standing clip must stay blended in, got {}",
            p.anim_blend
        );
        assert_eq!(p.clip_name.as_deref(), Some("Walk"));
        // Move forward: the move clip takes over.
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..10 {
            play.tick(dt);
        }
        let p = play.doc.player.as_ref().unwrap();
        assert_eq!(p.clip_name.as_deref(), Some("Run"));
        // Stop: back to the standing clip, still animating.
        play.input = WalkInput::default();
        for _ in 0..10 {
            play.tick(dt);
        }
        let p = play.doc.player.as_ref().unwrap();
        assert_eq!(p.clip_name.as_deref(), Some("Walk"));
    }

    #[test]
    fn wasd_plays_vrm_walk_clip_like_gltf() {
        const DUMP: &str = include_str!("../tests/fixtures/vrm_walker_world.json");
        let mut play = WorldPlay::from_json(DUMP).unwrap();
        play.start();
        assert_eq!(
            play.doc.player.as_ref().unwrap().gltf.as_deref(),
            Some("walk_skinned.vrm")
        );
        let rest = play
            .doc
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .unwrap()
            .1;
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(
            play.doc.player.as_ref().unwrap().clip > 0.2,
            "WASD must advance VRM walk clip"
        );
        let walk = play
            .doc
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .unwrap()
            .1;
        let mut max_d = 0.0f32;
        for (a, b) in rest.vertices.iter().zip(walk.vertices.iter()) {
            let d = (glam::Vec3::from_array(a.pos) - glam::Vec3::from_array(b.pos)).length();
            max_d = max_d.max(d);
        }
        assert!(
            max_d > 0.02,
            "VRM walking must not be a T-pose statue, max_d={max_d}"
        );
    }

    #[test]
    fn idle_and_walk_change_hair_yaw() {
        const DUMP: &str = include_str!("../tests/fixtures/vrm_walker_world.json");
        let mut play = WorldPlay::from_json(DUMP).unwrap();
        play.start();
        let hair0 = play.doc.player.as_ref().unwrap().hair;
        play.input = WalkInput::default();
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        let hair_idle = play.doc.player.as_ref().unwrap().hair;
        assert!(
            (hair_idle - hair0).abs() > 1e-4,
            "idle Verlet must change dump hair, hair0={hair0} idle={hair_idle}"
        );
        assert_eq!(play.doc.player.as_ref().unwrap().clip, 0.0);
        let idle_mesh = play
            .doc
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .unwrap()
            .1;
        let mut zero = play.doc.clone();
        if let Some(w) = zero.player.as_mut() {
            w.hair = 0.0;
        }
        for w in &mut zero.walkers {
            w.hair = 0.0;
        }
        let rest_mesh = zero
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .unwrap()
            .1;
        let mut max_d = 0.0f32;
        for (a, b) in rest_mesh.vertices.iter().zip(idle_mesh.vertices.iter()) {
            let d = (glam::Vec3::from_array(a.pos) - glam::Vec3::from_array(b.pos)).length();
            max_d = max_d.max(d);
        }
        assert!(
            max_d > 0.005,
            "idle hair yaw must move mesh, max_d={max_d} hair={hair_idle}"
        );
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        let hair_walk = play.doc.player.as_ref().unwrap().hair;
        assert!(
            hair_walk.abs() > 1e-6 || (hair_walk - hair_idle).abs() > 0.0,
            "walk still steps springs, hair_walk={hair_walk}"
        );
    }

    #[test]
    fn idle_blink_and_hold_j_set_dump_morph() {
        const DUMP: &str = include_str!("../tests/fixtures/vrm_walker_world.json");
        let mut play = WorldPlay::from_json(DUMP).unwrap();
        play.start();
        assert_eq!(play.doc.player.as_ref().unwrap().morph, 0.0);
        play.input = WalkInput::default();
        for _ in 0..4 {
            play.tick(1.0 / 60.0);
        }
        let blink = play.doc.player.as_ref().unwrap().morph;
        assert!(blink > 0.4, "idle blink must raise dump morph, got {blink}");
        let blink_mesh = play
            .doc
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .unwrap()
            .1;
        let mut zero = play.doc.clone();
        if let Some(w) = zero.player.as_mut() {
            w.morph = 0.0;
        }
        for w in &mut zero.walkers {
            w.morph = 0.0;
        }
        let rest_mesh = zero
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .unwrap()
            .1;
        let mut max_d = 0.0f32;
        for (a, b) in rest_mesh.vertices.iter().zip(blink_mesh.vertices.iter()) {
            let d = (glam::Vec3::from_array(a.pos) - glam::Vec3::from_array(b.pos)).length();
            max_d = max_d.max(d);
        }
        assert!(
            max_d > 0.02,
            "idle blink must move mesh, max_d={max_d} morph={blink}"
        );
        play.input = WalkInput {
            lx: 0.0,
            lz: 0.0,
            jump: false,
            attack: true,
            dodge: false,
        };
        play.tick(1.0 / 60.0);
        let held = play.doc.player.as_ref().unwrap().morph;
        assert!(
            (held - 1.0).abs() < 1e-5,
            "hold J (attack) forces morph 1, got {held}"
        );
    }

    #[test]
    fn crest_isle_walker_stays_capsule_morph_zero() {
        const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
        let mut play = WorldPlay::from_json(CREST).unwrap();
        play.start();
        play.input = WalkInput::default();
        for _ in 0..10 {
            play.tick(1.0 / 60.0);
        }
        let w = play.doc.player.as_ref().unwrap();
        assert!(w.gltf.is_none() || w.gltf.as_deref() == Some(""));
        assert_eq!(w.morph, 0.0);
        assert_eq!(w.look_yaw, 0.0);
        assert_eq!(w.look_pitch, 0.0);
    }

    #[test]
    fn idle_look_at_camera_sets_dump_yaw() {
        const DUMP: &str = include_str!("../tests/fixtures/vrm_walker_world.json");
        let mut play = WorldPlay::from_json(DUMP).unwrap();
        play.start();
        play.input = WalkInput::default();
        for _ in 0..4 {
            play.tick(1.0 / 60.0);
        }
        let ly = play.doc.player.as_ref().unwrap().look_yaw;
        let lp = play.doc.player.as_ref().unwrap().look_pitch;
        assert!(
            ly.abs() > 0.2 || lp.abs() > 0.05,
            "chase cam look-at must set dump yaw/pitch, look_yaw={ly} look_pitch={lp}"
        );
        let live = play
            .doc
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .unwrap()
            .1;
        let bind = {
            let mut idle = play.doc.clone();
            if let Some(w) = idle.player.as_mut() {
                w.look_yaw = 0.0;
                w.look_pitch = 0.0;
            }
            for w in &mut idle.walkers {
                w.look_yaw = 0.0;
                w.look_pitch = 0.0;
            }
            idle.compile_meshes()
                .into_iter()
                .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
                .unwrap()
                .1
        };
        let mut max_d = 0.0f32;
        for (a, b) in bind.vertices.iter().zip(live.vertices.iter()) {
            max_d =
                max_d.max((glam::Vec3::from_array(a.pos) - glam::Vec3::from_array(b.pos)).length());
        }
        assert!(
            max_d > 0.01,
            "look-at toward camera must move mesh, max_d={max_d} look_yaw={ly}"
        );
    }

    #[test]
    fn emma_walker_wasd_mixamo_walks() {
        const DUMP: &str = include_str!("../tests/fixtures/emma_walker_world.json");
        let mut play = WorldPlay::from_json(DUMP).unwrap();
        play.start();
        assert_eq!(
            play.doc.player.as_ref().unwrap().gltf.as_deref(),
            Some("assets/Emma.vrm")
        );
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(play.doc.player.as_ref().unwrap().clip > 0.2);
        let walk = play
            .doc
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .unwrap()
            .1;
        let bind = {
            let mut idle = play.doc.clone();
            if let Some(w) = idle.player.as_mut() {
                w.clip = 0.0;
            }
            for w in &mut idle.walkers {
                w.clip = 0.0;
            }
            idle.compile_meshes()
                .into_iter()
                .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
                .unwrap()
                .1
        };
        let mut max_d = 0.0f32;
        for (a, b) in bind.vertices.iter().zip(walk.vertices.iter()) {
            let d = (glam::Vec3::from_array(a.pos) - glam::Vec3::from_array(b.pos)).length();
            max_d = max_d.max(d);
        }
        assert!(
            max_d > 0.02,
            "Emma Mixamo walk (or tpose fallback) max_d={max_d}"
        );
        assert!(play.arrows_move_walker());
        assert!(play.is_gltf_walk_play());
        let pos = play.doc.player.as_ref().unwrap().position;
        let dist = (pos[0] * pos[0] + pos[2] * pos[2]).sqrt();
        assert!(dist > 0.2, "WASD must translate Emma on grass, dist={dist}");
        assert!(
            pos[1] < 0.35,
            "feet on grass, not capsule BODY_H hover, y={}",
            pos[1]
        );
        // Strafe: +lx (ArrowRight / D) is screen-right. Emma chase cam sits at
        // +Z (look_yaw=π), so screen-right = fwd × up = +X.
        play.input = WalkInput {
            lx: 1.0,
            lz: 0.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        let sx = play.doc.player.as_ref().unwrap().position[0];
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap();
        assert!(
            p.position[0] > sx + 0.4,
            "ArrowRight / D must strafe screen-right (+X), x {} from {}",
            p.position[0],
            sx
        );
        play.input = WalkInput::default();
        play.tick(1.0 / 60.0);
        assert_eq!(play.doc.player.as_ref().unwrap().clip, 0.0);
    }

    #[test]
    fn expression_preset_flips_morph_weight() {
        const DUMP: &str = include_str!("../tests/fixtures/emma_walker_world.json");
        let mut play = WorldPlay::from_json(DUMP).unwrap();
        play.start();
        // モデルが aa を持つ場合のみ検証（Emma.vrm on disk）。tpose フォール
        // バック（morphs 空）やディスク無しはスキップ。
        let spec = play
            .doc
            .player
            .as_ref()
            .unwrap()
            .gltf
            .as_deref()
            .unwrap_or("")
            .to_string();
        let Some(parts) = load_skinned_parts(&spec) else {
            return;
        };
        if !parts.iter().any(|p| p.expressions.has("aa")) {
            return;
        }
        if let Some(w) = play.doc.player.as_mut() {
            w.expression = "aa".into();
        }
        play.tick(1.0 / 60.0);
        assert_eq!(
            play.doc.player.as_ref().unwrap().morph,
            1.0,
            "named expression applies full weight"
        );
        // blink に戻すと自動まばたきエンベロープ。
        if let Some(w) = play.doc.player.as_mut() {
            w.expression = "blink".into();
        }
        play.tick(1.0 / 60.0);
        let m = play.doc.player.as_ref().unwrap().morph;
        assert!(m <= 1.0, "auto blink envelope, got {m}");
        // dump に expression が残る。
        let json = play.doc.to_json().unwrap();
        assert!(json.contains("\"expression\": \"blink\""));
    }

    #[test]
    fn emma_play_hides_collectathon_pips_and_notes_load() {
        const DUMP: &str = include_str!("../tests/fixtures/emma_walker_world.json");
        let play = WorldPlay::from_json(DUMP).unwrap();
        assert!(!play.is_collectathon());
        assert_eq!(play.doc.coins, 0);
        assert_eq!(play.game.coins, 0);
        assert_eq!(play.game.phase, crate::game::GamePhase::Playing);
        let hud = play.build_hud(960, 540);
        let err = play.doc.player.as_ref().and_then(|w| w.load_error.as_ref());
        if err.is_some() {
            assert!(
                hud.quads.iter().any(|q| q.color == [180, 48, 40, 230]),
                "load_error must be dump-visible on the HUD"
            );
        } else {
            assert!(
                hud.quads.is_empty(),
                "coins=0 walker dump must not show 8 gray Crest pips, got {}",
                hud.quads.len()
            );
        }
        assert_eq!(
            play.doc.player.as_ref().unwrap().gltf.as_deref(),
            Some("assets/Emma.vrm")
        );
        let json = play.doc.to_json().unwrap();
        if play.doc.player.as_ref().unwrap().load_error.is_some() {
            assert!(json.contains("load_error"));
        }
        let scene = play.doc.compile_scene(1.0);
        assert!(
            !scene.batches.iter().any(|b| b.mesh.0 == 2),
            "play compile must not draw capsule"
        );
        assert!(
            scene
                .batches
                .iter()
                .any(|b| b.mesh.0 >= crate::world_doc::MESH_GLTF_BASE),
            "play compile must draw the skinned walker"
        );
    }

    // ── Adhesive API: interact / event / timer / state→animation ──────────

    #[test]
    fn interact_emits_on_use_within_reach() {
        let mut play = WorldPlay::from_json(FISH).unwrap();
        play.start();
        // Player at origin; shore at (4,0,0) with reach 5 → usable.
        assert!(play.nearest_interact().is_some(), "shore in reach");
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        let evs = play.take_events("cast");
        assert_eq!(evs.len(), 1, "J at the shore emits the on_use event");
        assert_eq!(evs[0].count, 1);
        let data = evs[0].data.as_ref().expect("payload");
        assert_eq!(data["prop"], "prop:shore");
    }

    #[test]
    fn interact_out_of_reach_does_not_emit() {
        let mut play = WorldPlay::from_json(FISH).unwrap();
        play.start();
        if let Some(w) = play.doc.player.as_mut() {
            w.position = [20.0, 0.0, 0.0];
        }
        assert!(play.nearest_interact().is_none(), "shore out of reach");
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(
            play.take_events("cast").is_empty(),
            "no on_use event outside reach"
        );
    }

    #[test]
    fn events_aggregate_and_take_consumes() {
        let mut play = WorldPlay::from_json(FISH).unwrap();
        play.start();
        play.emit_event("fish_caught", Some(json!({"kind": "koi"})));
        play.emit_event("fish_caught", Some(json!({"kind": "saba"})));
        play.emit_event("other", None);
        let evs = play.take_events("fish_caught");
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].count, 2, "same-name events aggregate");
        assert_eq!(evs[0].data.as_ref().unwrap()["kind"], "saba");
        assert_eq!(play.doc.events.len(), 1, "only 'other' remains");
        assert_eq!(play.take_events("other").len(), 1);
        assert!(play.doc.events.is_empty(), "take consumes");
    }

    #[test]
    fn timer_counts_down_and_emits_on_done() {
        let mut play = WorldPlay::from_json(FISH).unwrap();
        play.start();
        play.start_timer("cast", 3.0, Some("bite"));
        assert_eq!(play.doc.timers.len(), 1);
        assert!(play.doc.timers[0].active);
        for _ in 0..150 {
            play.tick(1.0 / 60.0); // 2.5s — not done yet
        }
        assert!(play.take_events("bite").is_empty(), "bite only after 3s");
        assert!(play.doc.timers[0].active);
        for _ in 0..60 {
            play.tick(1.0 / 60.0); // +1.0s → 3.5s total
        }
        let evs = play.take_events("bite");
        assert_eq!(evs.len(), 1, "on_done emits the named event");
        assert!(!play.doc.timers[0].active, "done timer parks");
        assert!(play.doc.timers[0].remaining <= 1e-4);
    }

    #[test]
    fn anim_follows_wish_and_keeps_genre_state() {
        let mut play = WorldPlay::from_json(FISH).unwrap();
        play.start();
        assert_eq!(play.doc.player.as_ref().unwrap().anim, "idle");
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..10 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap();
        assert_eq!(p.anim, "walk", "wish derives locomotion state");
        assert!(p.clip > 0.0);
        play.input = WalkInput::default();
        play.tick(1.0 / 60.0);
        assert_eq!(play.doc.player.as_ref().unwrap().anim, "idle");
        if let Some(w) = play.doc.player.as_mut() {
            w.anim = "cast".into();
        }
        play.tick(1.0 / 60.0);
        assert_eq!(
            play.doc.player.as_ref().unwrap().anim,
            "cast",
            "genre anim survives standing still"
        );
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        play.tick(1.0 / 60.0);
        assert_eq!(
            play.doc.player.as_ref().unwrap().anim,
            "walk",
            "movement overwrites genre anim"
        );
    }

    #[test]
    fn cast_interact_timer_bite_loop() {
        // The whole adhesive loop on one dump: J at the shore → "cast" event
        // → game starts a timer → countdown → "bite" event → game flips the
        // animation state. Engine only carries data; the reactions here stand
        // in for genre code (fish.rs-style), not engine logic.
        let mut play = WorldPlay::from_json(FISH).unwrap();
        play.start();
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.take_events("cast").len(), 1, "interact → cast event");
        play.input.attack = false;

        play.start_timer("cast", 3.0, Some("bite"));
        if let Some(w) = play.doc.player.as_mut() {
            w.anim = "cast".into();
        }
        for _ in 0..180 {
            play.tick(1.0 / 60.0); // 3.0s
        }
        assert_eq!(play.take_events("bite").len(), 1, "timer → bite event");
        assert_eq!(play.doc.player.as_ref().unwrap().anim, "cast");

        if let Some(w) = play.doc.player.as_mut() {
            w.anim = "hit".into();
        }
        play.tick(1.0 / 60.0);
        assert_eq!(
            play.doc.player.as_ref().unwrap().anim,
            "hit",
            "game switches anim on the bite event"
        );
    }

    #[test]
    fn dock_prop_does_not_steal_collectathon_without_genre() {
        let mut data: serde_json::Value = serde_json::from_str(CREST).unwrap();
        data["genre"] = serde_json::json!("crest_isle");
        let props = data["props"].as_array_mut().unwrap();
        props.push(serde_json::json!({
            "id": "prop:dock",
            "type": "prop",
            "name": "dock",
            "position": [3.0, 0.2, -6.0],
            "model": "box",
            "scale": [2.0, 0.4, 2.0],
            "enabled": true
        }));
        let mut play = WorldPlay::from_json(&data.to_string()).unwrap();
        play.confirm();
        play.tick(1.0 / 60.0);
        assert!(
            play.is_collectathon(),
            "explicit crest_isle genre stays collectathon"
        );
        assert!(!play.is_fish(), "name=dock must not start fishing");
        assert!(play.doc.coins > 0, "collectathon coins must not be zeroed");
    }

    #[test]
    fn bar_room_door_is_not_an_rpg() {
        const BAR: &str = include_str!("../tests/fixtures/bar_room_world.json");
        let play = WorldPlay::from_json(BAR).unwrap();
        assert!(!play.is_rpg(), "door scenery must not start town_gate");
        assert!(!play.is_shop());
        assert!(!play.is_cook());
        assert!(!play.is_fish());
        assert!(!play.is_collectathon());
    }

    #[test]
    fn missing_genre_is_free_world() {
        let mut data: serde_json::Value = serde_json::from_str(CREST).unwrap();
        data.as_object_mut().unwrap().remove("genre");
        let mut play = WorldPlay::from_json(&data.to_string()).unwrap();
        play.confirm();
        assert!(!play.is_collectathon());
        assert!(!play.is_fish());
        assert!(!play.is_rpg());
    }
}
