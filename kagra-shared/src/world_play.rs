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
use crate::fps::{self, FpsGame};
use crate::game::GamePhase;
use crate::gltf_load::{head_world_pos, step_springs};
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
use crate::world_doc::{load_skinned, WorldDoc, WorldProp, WorldWalker};
use glam::Vec3;

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
    seed: WorldDoc,
    vy: f32,
    spring: SpringState,
    blink_t: f32,
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
            doc.coins = td::START;
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
            vy: 0.0,
            spring: SpringState::default(),
            blink_t: 0.0,
        }
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        Ok(Self::new(WorldDoc::from_json(json)?))
    }

    /// Title / result: Space or Enter. Playing ignores this (Space is jump).
    pub fn confirm(&mut self) {
        match self.game.phase {
            GamePhase::Title | GamePhase::Complete => self.start(),
            GamePhase::Playing => {}
        }
    }

    pub fn start(&mut self) {
        let best = self.game.best_score;
        self.doc = self.seed.clone();
        self.input = WalkInput::default();
        self.look_yaw = look_yaw_from_doc(&self.doc);
        self.look_pitch = 0.0;
        self.vy = 0.0;
        self.blink_t = 0.0;
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
            self.doc.coins = td::START;
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

    /// Mouse / arrow look. Pitch is clamped.
    pub fn add_look(&mut self, dyaw: f32, dpitch: f32) {
        self.look_yaw += dyaw;
        self.look_pitch = (self.look_pitch + dpitch).clamp(-0.7, 0.55);
    }

    /// Advance walker + chase camera + pickups. `dt` is seconds (clamped).
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        if dt <= 0.0 {
            return;
        }
        if !self.game.is_playing() {
            return;
        }
        let input = self.input.clamped();
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
        self.step_walker(input, dt);
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
                self.game.score = self.platform.landed * 50;
            }
            return;
        }
        if self.is_fps() {
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
            if self.fps.won {
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
            return platformer::build_hud(&self.platform, self.game.phase, width, height);
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
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
        let pad = 16.0 * scale;
        let mut quads = Vec::new();

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
        }
    }

    fn step_walker(&mut self, input: WalkInput, dt: f32) {
        let half = self.doc.half.max(4.0);
        let (s, c) = self.look_yaw.sin_cos();
        let fwd = Vec3::new(s, 0.0, c);
        let right = Vec3::new(c, 0.0, -s);
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

        if wish_len > 0.08 {
            let dir = wish / wish_len;
            let speed = PLAYER_SPEED * wish_len.min(1.0);
            x += dir.x * speed * dt;
            z += dir.z * speed * dt;
            yaw = dir.x.atan2(dir.z);
            updated.clip += dt;
        } else {
            updated.clip = 0.0;
        }
        let pad = 2.0;
        x = x.clamp(-half + pad, half - pad);
        z = z.clamp(-half + pad, half - pad);

        if input.jump && on_ground {
            self.vy = JUMP_V;
        }
        self.vy -= GRAVITY * dt;
        y += self.vy * dt;
        let ground = self.doc.height_at(x, z) + BODY_H;
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
        let spec = {
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
            spec
        };
        let Some(skin) = load_skinned(&spec) else {
            return;
        };
        if skin.morphs.is_empty() {
            return;
        }
        self.blink_t += dt;
        let mut morph = morph::blink_weight(self.blink_t);
        if self.input.attack || self.rpg.talking {
            morph = 1.0;
        }
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
        let dist = CAM_DISTANCE;
        let height = CAM_HEIGHT - CAM_LOOK_Y + pitch * 4.0;
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
}

fn is_collectathon(doc: &WorldDoc) -> bool {
    doc.heightfield.as_ref().and_then(|h| h.fn_name.as_deref()) == Some("open_world_height")
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

fn player_ref(doc: &WorldDoc) -> Option<&WorldWalker> {
    doc.player.as_ref().or(doc.walkers.first())
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

    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const ORB: &str = include_str!("../tests/fixtures/orb_rush_world.json");

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
        play.input = WalkInput::default();
        play.tick(1.0 / 60.0);
        assert_eq!(play.doc.player.as_ref().unwrap().clip, 0.0);
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
        assert!(
            hud.quads.is_empty(),
            "coins=0 walker dump must not show 8 gray Crest pips, got {}",
            hud.quads.len()
        );
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
}
