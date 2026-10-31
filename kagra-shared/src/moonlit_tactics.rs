//! Moonlit Tactics: a small turn-based tactics game in the moonlit graveyard.
//!
//! The night watch (knight, lancer, archer, cleric) holds the cemetery against the dead.
//! Your phase: tap one of your units to see where it can go (blue), tap a cell to move there,
//! then tap a red foe to attack (or a green friend to heal, for the cleric), or WAIT. A foe
//! that can reach you hits back (a counter). When everyone has acted the dead take their
//! turn: each goes for the attack that hurts most, or walks closer. Clear the dead (or bring
//! down their leader) to win the map; lose everyone (or your leader) and the night is lost.
//! Maps, classes and numbers are JSON (`game.maps`, `game.classes`).
//!
//! Built from the Starter template (`scripts/new_kit.py`) on the shared parts:
//! [`crate::game_core::grid`] (reach, paths, ranges), [`crate::game_core::view`] (the camera
//! and the cell under a finger), [`declare_models`], [`tick_frame`], `hud_kit` panels.
//!
//! - `rules.rs`: the board, combat, the foes' plan (no picture, no input).
//! - `maps.rs`: the classes and maps that ship.
//! - `doc.rs`: the 3D picture (a persistent prop list per map).
//! - `hud.rs`: the HUD and the buttons.

mod doc;
mod hud;
mod maps;
pub mod rules;
#[cfg(test)]
mod tests;

use std::cell::Cell as StdCell;
use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::flow::{FlowPhase, GameFlow};
use crate::game_core::floats::Floats;
use crate::game_core::grid::{self, Cell};
use crate::game_core::progress::{Better, Progress};
use crate::game_core::view::View;
use crate::game_core::{tick_frame, Core, Kit};
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::DrawList;
use crate::world_doc::WorldDoc;
use rules::{Board, ClassSpec, MapSpec, Side};

/// The `genre` of this kit's world file (and the `game.id` default).
pub const GAME_ID: &str = "moonlit_tactics";

/// True for a world file whose `genre` is this kit's.
pub fn is_moonlit_tactics_dump(json: &str) -> bool {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|v| {
            v.get("genre")
                .and_then(|g| g.as_str())
                .map(|s| s == GAME_ID)
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------- spec ----

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MoonlitTacticsSpec {
    pub id: String,
    pub title: String,
    pub classes: Vec<ClassSpec>,
    pub maps: Vec<MapSpec>,
    /// Metres per cell.
    pub tile: f32,
    /// Seconds a unit takes per cell when it walks, and for a strike.
    pub step_secs: f32,
    pub strike_secs: f32,
    /// Points: a map won, each hero standing at the end, each turn under par.
    pub map_score: u32,
    pub survivor_score: u32,
    pub turn_score: u32,
    /// Music and sound cues for the page (file names under the bundle). Passed through.
    pub audio: Option<Value>,
}

impl Default for MoonlitTacticsSpec {
    fn default() -> Self {
        Self {
            id: GAME_ID.into(),
            title: "Moonlit Tactics".into(),
            classes: maps::classes(),
            maps: vec![maps::chapel_yard(), maps::keepers_gate()],
            tile: 1.2,
            step_secs: 0.11,
            strike_secs: 0.45,
            map_score: 500,
            survivor_score: 150,
            turn_score: 60,
            audio: Some(default_audio()),
        }
    }
}

impl MoonlitTacticsSpec {
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let spec: Self =
            serde_json::from_value(v.clone()).map_err(|e| format!("moonlit_tactics spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), String> {
        let range = |name: &str, v: f32, lo: f32, hi: f32| {
            if v.is_finite() && (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(format!(
                    "moonlit_tactics: {name} must be {lo}..={hi}, got {v}"
                ))
            }
        };
        range("tile", self.tile, 0.4, 4.0)?;
        range("step_secs", self.step_secs, 0.0, 2.0)?;
        range("strike_secs", self.strike_secs, 0.0, 5.0)?;
        if self.classes.is_empty() || self.maps.is_empty() || self.maps.len() > 32 {
            return Err("moonlit_tactics: needs classes and 1..=32 maps".into());
        }
        for (i, c) in self.classes.iter().enumerate() {
            if self.classes[..i].iter().any(|o| o.id == c.id) {
                return Err(format!("moonlit_tactics: class {:?} twice", c.id));
            }
            if c.hp < 1 || c.mv < 0 || c.range_min < 0 || c.range_max < c.range_min.max(1) {
                return Err(format!(
                    "moonlit_tactics: class {:?} needs hp >= 1, mv >= 0, 1 <= range_max >= range_min",
                    c.id
                ));
            }
        }
        for (i, m) in self.maps.iter().enumerate() {
            m.validate(&format!("moonlit_tactics: maps[{i}]"), &self.classes)?;
        }
        Ok(())
    }
}

fn default_audio() -> Value {
    let sfx = "assets/audio/oga-retro-synth-sfx/";
    let cue =
        |f: &str, v: f64, r: f64| json!({"file": format!("{sfx}{f}"), "volume": v, "rate": r});
    let ui = |v: f64, r: f64| json!({"file": "assets/audio/kenney-interface-sounds/confirmation_001.wav", "volume": v, "rate": r});
    let hit = |v: f64, r: f64| json!({"file": "assets/audio/kenney-impact-sounds/impactGeneric_light_001.wav", "volume": v, "rate": r});
    json!({
        "enabled": true,
        "master": 0.8,
        "sfx_volume": 1.0,
        "bgm_volume": 0.5,
        "bgm": {"file": "assets/audio/oga-flowerbed-fields/flowerbed_fields.mp3", "volume": 0.45, "fade_secs": 0.5},
        "sfx": {
            "select": ui(0.4, 1.3),
            "move": ui(0.25, 0.8),
            "hit": hit(0.6, 1.0),
            "kill": {"file": "assets/audio/oga-wet-squish/impactsplat03.wav", "volume": 0.45, "rate": 0.9},
            "heal": cue("power_up_02.wav", 0.5, 1.4),
            "phase": cue("retro_coin_01.wav", 0.3, 0.7),
            "win": cue("power_up_04.wav", 0.8, 1.0),
            "gameover": cue("retro_die_03.wav", 0.8, 1.0),
            "go": ui(0.7, 1.0)
        }
    })
}

// ----------------------------------------------------------------- run ----

/// Where the night is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// The map's story card; a tap starts it.
    Intro,
    Player,
    Enemy,
    /// The map is won (card, a tap goes on) or lost.
    Won,
    Lost,
}

/// Something the picture plays out before the next input.
#[derive(Clone, Debug, PartialEq)]
enum Anim {
    Walk { u: usize, path: Vec<Cell>, t: f32 },
    Strike { a: usize, d: usize, t: f32 },
    Pause { t: f32 },
}

/// A button on the HUD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Btn {
    Wait,
    Undo,
    EndTurn,
}

/// One map's numbers for the card and the total.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Tally {
    turns: u32,
    survivors: u32,
    kills: u32,
    score: u32,
}

#[derive(Debug)]
pub struct MoonlitTactics {
    spec: MoonlitTacticsSpec,
    core: Core,
    flow: GameFlow,
    doc: WorldDoc,
    slots: doc::Slots,
    doc_map: Option<usize>,
    board: Board,
    map_i: usize,
    mode: Mode,
    turn: u32,
    /// The selected hero, where it came from if it has moved (Undo), and where it can go.
    sel: Option<usize>,
    moved_from: Option<Cell>,
    reach: Vec<(Cell, i32)>,
    anims: VecDeque<Anim>,
    floats: Floats,
    /// Foes still to act this enemy phase.
    foe_queue: Vec<usize>,
    /// Which way each unit faces (yaw).
    facing: Vec<f32>,
    /// The phase banner and how long it shows.
    banner: f32,
    kills: u32,
    tallies: Vec<Tally>,
    /// Kept on the device (`page.save`): maps won, fewest turns per map.
    progress: Progress,
    /// The map a run starts at (0 unless a link or the map select asks for another).
    first_map: usize,
    /// A new fewest-turns record on the map just won.
    new_record: bool,
    /// The keys' last sideways push on the first story card (map select).
    prev_pick: f32,
    clock: f32,
    win: bool,
    new_best: bool,
    best_before: u32,
    lang_ja: bool,
    size: StdCell<(f32, f32)>,
}

impl MoonlitTactics {
    pub fn new(spec: MoonlitTacticsSpec) -> Self {
        let mut flow = GameFlow::new(spec.id.clone());
        flow.title = spec.title.clone();
        flow.tagline = String::new();
        let board = Board::new(&spec.maps[0], &spec.classes);
        let progress = Progress::new(spec.maps.len(), "best_turns", Better::Lower).with_places(0);
        let mut g = Self {
            spec,
            core: Core::default(),
            flow,
            doc: WorldDoc::default(),
            slots: doc::Slots::default(),
            doc_map: None,
            board,
            map_i: 0,
            mode: Mode::Intro,
            turn: 1,
            sel: None,
            moved_from: None,
            reach: Vec::new(),
            anims: VecDeque::new(),
            floats: Floats::default(),
            foe_queue: Vec::new(),
            facing: Vec::new(),
            banner: 0.0,
            kills: 0,
            tallies: Vec::new(),
            progress,
            first_map: 0,
            new_record: false,
            prev_pick: 0.0,
            clock: 0.0,
            win: false,
            new_best: false,
            best_before: 0,
            lang_ja: false,
            size: StdCell::new((390.0, 844.0)),
        };
        g.load_map(0);
        g.build_doc();
        g
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| format!("moonlit_tactics dump: {e}"))?;
        let spec = match value.get("game") {
            Some(g) => MoonlitTacticsSpec::from_value(g)?,
            None => MoonlitTacticsSpec::default(),
        };
        let mut g = Self::new(spec);
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            g.flow.best_score = best as u32;
        }
        // What the device kept (`page.save`): the maps won and the records.
        if let Some(sv) = value.get("save") {
            g.progress.load(sv);
        }
        // `?map=2` on the preview link (the manifest's `page.url_params`) starts there.
        let map = value.get("map").and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        });
        if let Some(n) = map {
            g.first_map = (n.max(1) as usize - 1).min(g.spec.maps.len() - 1);
            g.load_map(g.first_map);
        }
        Ok(g)
    }

    /// The world file this spec ships as (the baked fixture is exactly this).
    pub fn world_value(&self) -> Value {
        let game: Value =
            serde_json::from_str(&serde_json::to_string(&self.spec).unwrap_or_default())
                .unwrap_or_default();
        json!({"version": 1, "genre": GAME_ID, "game": game})
    }

    fn tr<'a>(&self, en: &'a str, ja: &'a str) -> &'a str {
        if self.lang_ja && !ja.is_empty() {
            ja
        } else {
            en
        }
    }

    // ---- flow

    fn load_map(&mut self, i: usize) {
        self.map_i = i.min(self.spec.maps.len() - 1);
        self.board = Board::new(&self.spec.maps[self.map_i], &self.spec.classes);
        self.mode = Mode::Intro;
        self.turn = 1;
        self.sel = None;
        self.moved_from = None;
        self.reach.clear();
        self.anims.clear();
        self.floats.clear();
        self.foe_queue.clear();
        self.kills = 0;
        self.new_record = false;
        self.facing = self
            .board
            .units
            .iter()
            .map(|u| {
                if u.side == Side::Hero {
                    std::f32::consts::PI
                } else {
                    0.0
                }
            })
            .collect();
    }

    pub fn confirm(&mut self) {
        match self.flow.phase {
            FlowPhase::Title | FlowPhase::Result => {
                self.core = Core::default();
                self.tallies.clear();
                self.win = false;
                self.new_best = false;
                self.load_map(self.first_map);
                self.best_before = self.flow.best_score;
                self.flow.enter_play();
                crate::mixer::cue_sfx("go");
                crate::mixer::play_bgm("bgm", 0.5);
            }
            FlowPhase::Pause => self.flow.resume(),
            FlowPhase::Play => {}
        }
    }

    pub fn show_title(&mut self) {
        self.flow.show_title();
        self.core.stats.alive = true;
        self.core.stats.score = 0;
    }

    pub fn pause(&mut self) {
        if self.flow.phase == FlowPhase::Play && self.core.stats.alive {
            self.flow.pause();
        }
    }

    fn start_player_phase(&mut self) {
        self.board.refresh(Side::Hero);
        self.mode = Mode::Player;
        self.banner = 1.2;
        self.sel = None;
        self.moved_from = None;
        self.reach.clear();
        crate::mixer::cue_sfx("phase");
    }

    fn start_enemy_phase(&mut self) {
        self.sel = None;
        self.moved_from = None;
        self.reach.clear();
        self.board.refresh(Side::Foe);
        self.mode = Mode::Enemy;
        self.banner = 1.2;
        // Nearest to the heroes first, so the front line moves before the back.
        let mut q: Vec<usize> = self.board.living(Side::Foe).collect();
        let heroes: Vec<Cell> = self
            .board
            .living(Side::Hero)
            .map(|h| self.board.units[h].at)
            .collect();
        let near = |u: usize| {
            heroes
                .iter()
                .map(|h| grid::manhattan(*h, self.board.units[u].at))
                .min()
                .unwrap_or(0)
        };
        // Popped from the back: the farthest go last.
        q.sort_by_key(|&u| std::cmp::Reverse(near(u)));
        self.foe_queue = q;
        self.anims.push_back(Anim::Pause { t: 0.9 });
        crate::mixer::cue_sfx("phase");
    }

    /// The map is decided (when the board says so).
    fn settle(&mut self) -> bool {
        let Some(won) = self.board.outcome() else {
            return false;
        };
        if won {
            let map = &self.spec.maps[self.map_i];
            let survivors = self.board.living(Side::Hero).count() as u32;
            let under = map.par_turns.saturating_sub(self.turn);
            let score = self.spec.map_score
                + survivors * self.spec.survivor_score
                + under * self.spec.turn_score;
            self.tallies.push(Tally {
                turns: self.turn,
                survivors,
                kills: self.kills,
                score,
            });
            self.core.stats.score += score;
            self.new_record = self.progress.clear(self.map_i, self.turn as f32);
            self.mode = Mode::Won;
            crate::mixer::cue_sfx("win");
        } else {
            self.mode = Mode::Lost;
            crate::mixer::stop_bgm(0.6);
            crate::mixer::cue_sfx("gameover");
        }
        true
    }

    /// A tap on a card: start the map, or go on after a win, or end after a loss.
    fn advance(&mut self) {
        match self.mode {
            Mode::Intro => self.start_player_phase(),
            Mode::Won => {
                if self.map_i + 1 < self.spec.maps.len() {
                    self.load_map(self.map_i + 1);
                } else {
                    self.finish(true);
                }
            }
            Mode::Lost => self.finish(false),
            _ => {}
        }
    }

    /// The map select is open on the run's first story card, once a map has been won.
    fn can_pick(&self) -> bool {
        self.mode == Mode::Intro && self.tallies.is_empty() && self.progress.can_pick()
    }

    /// Step the run's first map by `dir` among the maps reached (won + 1).
    fn pick_map(&mut self, dir: i32) {
        if !self.can_pick() {
            return;
        }
        let next = self.progress.step(self.map_i, dir);
        if next != self.map_i {
            self.load_map(next);
            crate::mixer::cue_sfx("select");
        }
    }

    fn finish(&mut self, win: bool) {
        self.win = win;
        crate::mixer::stop_bgm(0.6);
        let score = self.core.stats.score;
        self.new_best = score > self.best_before;
        if !win {
            self.core.stats.death = "fallen".into();
        }
        let kills =
            self.tallies.iter().map(|t| t.kills).sum::<u32>() + if win { 0 } else { self.kills };
        self.flow
            .finish(score, kills, if win { "DAWN" } else { "FALLEN" });
    }

    // ---- the player's moves

    fn select(&mut self, u: usize) {
        self.sel = Some(u);
        self.moved_from = None;
        self.reach = self.board.reach(u);
        crate::mixer::cue_sfx("select");
    }

    fn deselect(&mut self) {
        self.sel = None;
        self.moved_from = None;
        self.reach.clear();
    }

    /// Walk the selected hero to `c` (one of its reach cells).
    fn move_to(&mut self, c: Cell) {
        let Some(u) = self.sel else {
            return;
        };
        let from = self.board.units[u].at;
        let (w, h) = self.board.size();
        let path = grid::path(w, h, from, c, |x| self.board.enter_cost(u, x))
            .unwrap_or_else(|| vec![from, c]);
        self.face_along(u, &path);
        self.board.units[u].at = c;
        self.board.units[u].moved = true;
        self.moved_from = Some(from);
        self.reach.clear();
        self.anims.push_back(Anim::Walk { u, path, t: 0.0 });
        crate::mixer::cue_sfx("move");
    }

    fn face_along(&mut self, u: usize, path: &[Cell]) {
        if let [.., a, b] = path {
            self.facing[u] = ((b.0 - a.0) as f32).atan2((b.1 - a.1) as f32);
        }
    }

    fn face(&mut self, u: usize, to: Cell) {
        let at = self.board.units[u].at;
        if at != to {
            self.facing[u] = ((to.0 - at.0) as f32).atan2((to.1 - at.1) as f32);
        }
    }

    /// Unit `a` (on its cell) attacks or heals `d`, with the picture to go with it.
    fn act_on(&mut self, a: usize, d: usize) {
        self.face(a, self.board.units[d].at);
        if self.board.units[d].side != self.board.units[a].side {
            self.face(d, self.board.units[a].at);
        }
        let t = self.spec.tile;
        let pos = |b: &Board, u: usize| doc::cell_pos(b, b.units[u].at, t);
        let dpos = pos(&self.board, d);
        let apos = pos(&self.board, a);
        let delay = self.spec.strike_secs * 0.5;
        if self.board.class(a).heal > 0 && self.board.units[d].side == self.board.units[a].side {
            let n = self.board.heal(a, d);
            self.float(dpos, format!("+{n}"), [120, 255, 160, 255], delay);
            crate::mixer::cue_sfx("heal");
        } else {
            let ex = self.board.attack(a, d);
            self.float(dpos, format!("-{}", ex.dealt), [255, 120, 100, 255], delay);
            if let Some(c) = ex.countered {
                self.float(apos, format!("-{c}"), [255, 170, 120, 255], delay + 0.25);
            }
            if ex.attacker_died {
                self.kills += u32::from(self.board.units[a].side == Side::Foe);
            }
            if ex.killed {
                self.kills += u32::from(self.board.units[d].side == Side::Foe);
                crate::mixer::cue_sfx("kill");
            } else {
                crate::mixer::cue_sfx("hit");
            }
            self.core.juice.hit(0.12, 0.0);
        }
        self.anims.push_back(Anim::Strike { a, d, t: 0.0 });
    }

    fn float(&mut self, at: [f32; 3], text: String, color: [u8; 4], delay: f32) {
        let y = 1.4 * self.spec.tile;
        self.floats.push([at[0], y, at[2]], text, color, delay);
    }

    /// The selected hero is done (after acting or waiting).
    fn finish_unit(&mut self) {
        if let Some(u) = self.sel {
            self.board.units[u].moved = true;
            self.board.units[u].acted = true;
        }
        self.deselect();
    }

    fn press(&mut self, b: Btn) {
        match b {
            Btn::Wait => self.finish_unit(),
            Btn::Undo => {
                if let (Some(u), Some(from)) = (self.sel, self.moved_from) {
                    self.board.units[u].at = from;
                    self.board.units[u].moved = false;
                    self.select(u);
                }
            }
            Btn::EndTurn => {
                self.deselect();
                for u in self.board.living(Side::Hero).collect::<Vec<_>>() {
                    self.board.units[u].acted = true;
                    self.board.units[u].moved = true;
                }
            }
        }
    }

    /// A tap on cell `c` in the player phase.
    pub fn tap_cell(&mut self, c: Cell) {
        if self.mode != Mode::Player || !self.anims.is_empty() {
            return;
        }
        let here = self.board.at(c);
        let Some(u) = self.sel else {
            if let Some(h) = here.filter(|&h| self.ready(h)) {
                self.select(h);
            }
            return;
        };
        let at = self.board.units[u].at;
        // A target from where the unit stands (moved or not).
        if let Some(t) = here.filter(|t| self.board.targets_from(u, at).contains(t)) {
            self.act_on(u, t);
            self.finish_unit();
            return;
        }
        if self.moved_from.is_some() {
            if c == at {
                self.finish_unit();
            }
            return;
        }
        if c != at && self.reach.iter().any(|(r, _)| *r == c) {
            self.move_to(c);
            return;
        }
        match here {
            Some(h) if h != u && self.ready(h) => self.select(h),
            _ => self.deselect(),
        }
    }

    /// A hero that has not acted yet this phase.
    fn ready(&self, u: usize) -> bool {
        let x = &self.board.units[u];
        x.alive() && x.side == Side::Hero && !x.acted
    }

    // ---- sim

    fn step_sim(&mut self, dt: f32) {
        self.clock += dt;
        self.banner = (self.banner - dt).max(0.0);
        self.floats.tick(dt);
        if let Some(a) = self.anims.front_mut() {
            let done = match a {
                Anim::Walk { path, t, .. } => {
                    *t += dt;
                    *t >= (path.len().saturating_sub(1)) as f32 * self.spec.step_secs
                }
                Anim::Strike { t, .. } => {
                    *t += dt;
                    *t >= self.spec.strike_secs
                }
                Anim::Pause { t } => {
                    *t -= dt;
                    *t <= 0.0
                }
            };
            if done {
                self.anims.pop_front();
            }
            return;
        }
        if matches!(self.mode, Mode::Player | Mode::Enemy) && self.settle() {
            return;
        }
        match self.mode {
            Mode::Player => {
                if self
                    .board
                    .living(Side::Hero)
                    .all(|h| self.board.units[h].acted)
                {
                    self.start_enemy_phase();
                }
            }
            Mode::Enemy => match self.foe_queue.pop() {
                Some(u) => self.foe_turn(u),
                None => {
                    self.turn += 1;
                    self.start_player_phase();
                }
            },
            _ => {}
        }
    }

    /// One foe moves and (maybe) attacks.
    fn foe_turn(&mut self, u: usize) {
        if !self.board.units[u].alive() {
            return;
        }
        let (dest, target) = self.board.plan(u);
        let from = self.board.units[u].at;
        if dest != from {
            let (w, h) = self.board.size();
            let path = grid::path(w, h, from, dest, |x| self.board.enter_cost(u, x))
                .unwrap_or_else(|| vec![from, dest]);
            self.face_along(u, &path);
            self.board.units[u].at = dest;
            self.anims.push_back(Anim::Walk { u, path, t: 0.0 });
        }
        if let Some(t) = target {
            self.act_on(u, t);
        } else {
            self.anims.push_back(Anim::Pause { t: 0.15 });
        }
    }

    // ---- input

    fn view(&self) -> View {
        let (w, h) = self.size.get();
        doc::board_view(&self.board, self.spec.tile, w / h.max(1.0))
    }

    pub fn touch_ev(&mut self, ev: PointerEvent, w: u32, h: u32) {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        self.size.set((w, h));
        if ev.phase != PointerPhase::Begin || self.flow.phase != FlowPhase::Play {
            return;
        }
        if matches!(self.mode, Mode::Intro | Mode::Won | Mode::Lost) {
            let side = crate::hud_kit::pick_side(ev.x, w);
            if self.can_pick() && side != 0 {
                self.pick_map(side);
            } else if self.anims.is_empty() {
                self.advance();
            }
            return;
        }
        if self.mode != Mode::Player || !self.anims.is_empty() {
            return;
        }
        if let Some(b) = self.button_at(ev.x, ev.y, w, h) {
            self.press(b);
            return;
        }
        let view = self.view();
        if let Some((x, z)) = view.ground_at(ev.x, ev.y, w, h, 0.0) {
            if let Some(c) = doc::cell_at(&self.board, x, z, self.spec.tile) {
                self.tap_cell(c);
            }
        }
    }

    // ---- what the page reads

    pub fn state_value(&self) -> Value {
        let mode = match self.mode {
            Mode::Intro => "intro",
            Mode::Player => "player",
            Mode::Enemy => "enemy",
            Mode::Won => "won",
            Mode::Lost => "lost",
        };
        json!({
            "phase": self.flow.phase.name(),
            "mode": mode,
            "score": self.core.stats.score,
            "best": self.flow.best_score,
            "map": self.map_i + 1,
            "maps": self.spec.maps.len(),
            "turn": self.turn,
            "heroes": self.board.living(Side::Hero).count(),
            "foes": self.board.living(Side::Foe).count(),
            "win": self.win,
            "death": self.core.stats.death,
            "death_text": if self.core.stats.death.is_empty() { "" } else { self.tr("The watch has fallen", "見張りは倒れた") },
            "result": if self.flow.phase == FlowPhase::Result {
                let turns: u32 = self.tallies.iter().map(|t| t.turns).sum();
                json!({
                    "key": self.spec.id,
                    "title": self.spec.title,
                    "enabled": true,
                    "top_n": 5,
                    "columns": ["kills", "turns", "score"],
                    "sort": "score",
                    "run": {"score": self.core.stats.score, "kills": self.tallies.iter().map(|t| t.kills).sum::<u32>(), "turns": turns},
                    "can_revive": false,
                    "new_best": self.new_best,
                })
            } else {
                Value::Null
            },
        })
    }
}

fn hud_scale(w: f32, h: f32) -> f32 {
    (w.min(h) / 400.0).clamp(0.9, 2.6)
}

// ------------------------------------------------------------ the host ----

impl Kit for MoonlitTactics {
    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }
    fn flow(&self) -> &GameFlow {
        &self.flow
    }
    fn flow_mut(&mut self) -> &mut GameFlow {
        &mut self.flow
    }
    fn step(&mut self, dt: f32) {
        self.step_sim(dt);
    }
    fn attract(&mut self, dt: f32) {
        self.clock += dt;
    }
    fn rebuild(&mut self) {
        self.update_doc();
    }
}

impl crate::kit_host::KitHost for MoonlitTactics {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn name(&self) -> &'static str {
        GAME_ID
    }
    fn phase_name(&self) -> &'static str {
        self.flow.phase.name()
    }
    fn playing(&self) -> bool {
        self.flow.phase == FlowPhase::Play
    }
    fn document(&self) -> &WorldDoc {
        &self.doc
    }
    fn dump_json(&self) -> String {
        crate::kit_host::dump_with_sim(&self.doc, &self.state_value())
    }
    fn state_json(&self) -> String {
        self.state_value().to_string()
    }
    fn confirm(&mut self) {
        MoonlitTactics::confirm(self)
    }
    fn show_title(&mut self) {
        MoonlitTactics::show_title(self)
    }
    fn pointer(&mut self, _x: f32, _y: f32, _width: u32, _height: u32) -> bool {
        false
    }
    fn on_key(&mut self, _name: &str, _down: bool) -> bool {
        false
    }
    fn tick(&mut self, dt: f32) {
        tick_frame(self, dt);
    }
    fn hud(&self, width: u32, height: u32, menu: bool) -> DrawList {
        MoonlitTactics::hud(self, width, height, menu)
    }
    fn walk(&mut self, lx: f32, _lz: f32, _jump: bool) {
        // Left / right on the first story card picks the map.
        let push = if lx.abs() > 0.5 { lx.signum() } else { 0.0 };
        if push != 0.0 && push != self.prev_pick {
            self.pick_map(push as i32);
        }
        self.prev_pick = push;
    }
    fn save_json(&self) -> Option<String> {
        Some(self.progress.to_value().to_string())
    }
    fn touch(&mut self, ev: PointerEvent, w: u32, h: u32, _t: f32) {
        self.touch_ev(ev, w, h);
    }
    fn uses_swipe(&self) -> bool {
        // The kit reads taps itself.
        true
    }
    fn set_lang(&mut self, lang: &str) {
        self.lang_ja = lang.eq_ignore_ascii_case("ja");
    }
    fn pause_run(&mut self) {
        self.pause();
    }
    fn resume_run(&mut self) {
        self.flow.resume();
    }
}
