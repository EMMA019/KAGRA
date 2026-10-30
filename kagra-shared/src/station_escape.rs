//! Station Escape: run, jump and dash out of a space station that is coming apart.
//!
//! A side-on 3D platformer in stages. The left thumb runs (a floating stick, sideways only),
//! the right thumb has JUMP (hold for higher) and DASH (one in the air). Stages are JSON:
//! solid, one-way, crumbling and moving ledges, blinking lasers, spikes, power cells to
//! collect, checkpoints and the exit. A fall, a laser or spikes send you back to the last
//! checkpoint; no game over. Each stage opens with a story card and ends with a clear card
//! (time, cells, falls); the last one ends the run with the total.
//!
//! Built from the Starter template (`scripts/new_kit.py`) on the shared parts:
//!
//! - [`crate::game_core::platform`]: the body (coyote time, jump buffer, short hop, air dash,
//!   one-way and moving platforms).
//! - [`crate::game_core::stick`], [`crate::game_core::view`], [`declare_models`],
//!   [`tick_frame`].
//! - [`crate::hud_kit`]: rounded panels and gauges.
//!
//! The picture is a persistent prop list built once per stage; a frame only edits positions,
//! colours and `enabled` in place (no per-frame allocation, a fixed GPU model set).

mod boss;
mod foes;
mod stages;

pub use boss::Boss;
pub use foes::{Drone, Turret};

use std::cell::Cell;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::flow::{FlowPhase, GameFlow};
use crate::game_core::platform::{touches, Body, BodyInput, BodySpec, Solid};
use crate::game_core::progress::{self, Better, Progress};
use crate::game_core::view::View;
use crate::game_core::{declare_models, stick, tick_frame, Core, Kit};
use crate::hop_beat::{glb_prop, glb_ready, light, prop};
use crate::hud_kit::{Card, CardStyle};
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, Quad, TextAlign, TextFace, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc};

/// The `genre` of this kit's world file (and the `game.id` default).
pub const GAME_ID: &str = "station_escape";

/// True for a world file whose `genre` is this kit's.
pub fn is_station_escape_dump(json: &str) -> bool {
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

/// A ledge. (x, y) is its bottom-left corner. `kind`: `solid`, `oneway` (jump up through it),
/// `crumble` (falls a moment after you land on it, comes back later) or `move` (travels `to`
/// and back, `secs` each way).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Block {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub kind: String,
    pub to: [f32; 2],
    pub secs: f32,
}

impl Default for Block {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            kind: "solid".into(),
            to: [0.0, 0.0],
            secs: 2.0,
        }
    }
}

/// A row of spikes on the floor. With `off` > 0 they pull in for `off` seconds and stay out for
/// `on` (offset by `phase`), and rattle for the last 0.35 s before they come out.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub on: f32,
    pub off: f32,
    pub phase: f32,
}

impl Rect {
    /// (out, about to come out) at time `t`.
    fn state(&self, t: f32) -> (bool, bool) {
        cycle(self.on, self.off, self.phase, t)
    }
}

/// A thing that is on for `on` seconds and off for `off` (0 = always on), offset by `phase`:
/// (on now, coming on within 0.35 s).
fn cycle(on: f32, off: f32, phase: f32, t: f32) -> (bool, bool) {
    if off <= 0.0 {
        return (true, false);
    }
    let period = on + off;
    let u = (t + phase).rem_euclid(period);
    let now = u < on;
    (now, !now && u > period - 0.35)
}

/// A laser beam: on for `on` seconds, off for `off` (0 = always on), offset by `phase`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Laser {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub on: f32,
    pub off: f32,
    pub phase: f32,
}

impl Laser {
    /// Is the beam on at time `t`? It shows a warning for the last 0.35 s before it comes on.
    fn state(&self, t: f32) -> (bool, bool) {
        cycle(self.on, self.off, self.phase, t)
    }
}

/// A line of tutorial text shown while the hero is past `x` (until the next hint).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Hint {
    pub x: f32,
    pub en: String,
    pub ja: String,
}

/// Scenery behind the play plane: a model, where (z < 0 is behind), how big, which way.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Deco {
    pub model: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub scale: f32,
    pub yaw: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Stage {
    pub name: String,
    pub name_ja: String,
    pub story: String,
    pub story_ja: String,
    /// Where the hero starts, and the exit door (feet).
    pub spawn: [f32; 2],
    pub exit: [f32; 2],
    /// Falling below this is a fall.
    pub kill_y: f32,
    /// A clear under this many seconds earns the time bonus.
    pub par_secs: f32,
    pub blocks: Vec<Block>,
    pub lasers: Vec<Laser>,
    pub spikes: Vec<Rect>,
    pub cells: Vec<[f32; 2]>,
    pub checkpoints: Vec<[f32; 2]>,
    pub hints: Vec<Hint>,
    pub decos: Vec<Deco>,
    pub drones: Vec<Drone>,
    pub turrets: Vec<Turret>,
    /// The Warden: the exit stays shut until it is down.
    pub boss: Option<Boss>,
}

impl Default for Stage {
    fn default() -> Self {
        Self {
            name: "Stage".into(),
            name_ja: String::new(),
            story: String::new(),
            story_ja: String::new(),
            spawn: [0.0, 0.0],
            exit: [10.0, 0.0],
            kill_y: -5.0,
            par_secs: 60.0,
            blocks: Vec::new(),
            lasers: Vec::new(),
            spikes: Vec::new(),
            cells: Vec::new(),
            checkpoints: Vec::new(),
            hints: Vec::new(),
            decos: Vec::new(),
            drones: Vec::new(),
            turrets: Vec::new(),
            boss: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StationEscapeSpec {
    pub id: String,
    pub title: String,
    pub body: BodySpec,
    pub stages: Vec<Stage>,
    /// A crumbling ledge falls this long after it is first stood on, and is back after
    /// `crumble_back` seconds.
    pub crumble_secs: f32,
    pub crumble_back: f32,
    /// Seconds between a fall and the respawn.
    pub respawn_secs: f32,
    /// Points: a cleared stage, each cell, each second under par, each fall (taken away).
    pub clear_score: u32,
    pub cell_score: u32,
    pub par_score: u32,
    pub fall_cost: u32,
    /// Points for each drone wrecked (a stomp or a dash), and the stomp's bounce (m/s).
    pub foe_score: u32,
    /// Points for bringing the Warden down.
    pub boss_score: u32,
    pub stomp_bounce: f32,
    /// Thumb stick radius and buttons, in pixels at 400 px of short side; the stick's dead zone.
    pub stick_px: f32,
    pub stick_dead: f32,
    pub button_px: f32,
    /// Music and sound cues for the page (file names under the bundle). Passed through.
    pub audio: Option<Value>,
}

impl Default for StationEscapeSpec {
    fn default() -> Self {
        Self {
            id: GAME_ID.into(),
            title: "Station Escape".into(),
            body: BodySpec::default(),
            stages: vec![
                stages::docking_bay(),
                stages::reactor_ring(),
                stages::cargo_spine(),
                stages::escape_pod(),
                stages::reactor_core(),
            ],
            crumble_secs: 0.45,
            crumble_back: 3.0,
            respawn_secs: 0.7,
            clear_score: 500,
            cell_score: 50,
            par_score: 10,
            fall_cost: 25,
            foe_score: 40,
            boss_score: 800,
            stomp_bounce: 9.0,
            stick_px: 64.0,
            stick_dead: 0.15,
            button_px: 40.0,
            audio: Some(default_audio()),
        }
    }
}

impl StationEscapeSpec {
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let spec: Self =
            serde_json::from_value(v.clone()).map_err(|e| format!("station_escape spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), String> {
        let range = |name: &str, v: f32, lo: f32, hi: f32| {
            if v.is_finite() && (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(format!(
                    "station_escape: {name} must be {lo}..={hi}, got {v}"
                ))
            }
        };
        self.body.validate("station_escape: body")?;
        range("crumble_secs", self.crumble_secs, 0.0, 10.0)?;
        range("crumble_back", self.crumble_back, 0.1, 60.0)?;
        range("respawn_secs", self.respawn_secs, 0.0, 5.0)?;
        range("stomp_bounce", self.stomp_bounce, 0.0, 60.0)?;
        range("stick_px", self.stick_px, 20.0, 200.0)?;
        range("stick_dead", self.stick_dead, 0.0, 0.6)?;
        range("button_px", self.button_px, 16.0, 120.0)?;
        if self.stages.is_empty() || self.stages.len() > 32 {
            return Err("station_escape: stages must have 1..=32 entries".into());
        }
        for (i, st) in self.stages.iter().enumerate() {
            let at = format!("stages[{i}]");
            if st.blocks.is_empty() {
                return Err(format!("station_escape: {at}.blocks is empty"));
            }
            for (j, b) in st.blocks.iter().enumerate() {
                if !matches!(b.kind.as_str(), "solid" | "oneway" | "crumble" | "move") {
                    return Err(format!(
                        "station_escape: {at}.blocks[{j}].kind: unknown {:?}",
                        b.kind
                    ));
                }
                if !(b.w > 0.0 && b.h > 0.0 && b.w.is_finite() && b.h.is_finite()) {
                    return Err(format!("station_escape: {at}.blocks[{j}] needs w, h > 0"));
                }
                if b.kind == "move" {
                    range(&format!("{at}.blocks[{j}].secs"), b.secs, 0.2, 60.0)?;
                }
            }
            if let Some(b) = &st.boss {
                if !(1..=20).contains(&b.hp) || b.x1 < b.x0 + 6.0 || b.high <= b.low + 1.0 {
                    return Err(format!(
                        "station_escape: {at}.boss needs hp 1..=20, x1 >= x0 + 6 and high > low + 1"
                    ));
                }
                for (name, v) in [
                    ("hover_secs", b.hover_secs),
                    ("warn_secs", b.warn_secs),
                    ("swoop_secs", b.swoop_secs),
                    ("climb_secs", b.climb_secs),
                ] {
                    range(&format!("{at}.boss.{name}"), v, 0.2, 30.0)?;
                }
            }
            for (j, d) in st.drones.iter().enumerate() {
                range(&format!("{at}.drones[{j}].speed"), d.speed, 0.1, 30.0)?;
            }
            for (j, tu) in st.turrets.iter().enumerate() {
                range(&format!("{at}.turrets[{j}].every"), tu.every, 0.2, 60.0)?;
                range(&format!("{at}.turrets[{j}].speed"), tu.speed, 0.5, 60.0)?;
                range(&format!("{at}.turrets[{j}].range"), tu.range, 0.5, 200.0)?;
            }
            for (j, l) in st.lasers.iter().enumerate() {
                range(&format!("{at}.lasers[{j}].on"), l.on, 0.0, 60.0)?;
                range(&format!("{at}.lasers[{j}].off"), l.off, 0.0, 60.0)?;
            }
            if st.kill_y >= st.spawn[1] {
                return Err(format!(
                    "station_escape: {at}.kill_y must be below the spawn"
                ));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- look ----

const DRONE_GLB: &str = "assets/models/kenney/space-kit/craft_speederC.glb";
const TURRET_GLB: &str = "assets/models/kenney/space-kit/turret_single.glb";
const HERO_GLB: &str = "assets/models/kenney/mini-characters/character-female-d.glb";
const CELL_GLB: &str = "assets/models/kenney/platformer-kit/jewel.glb";
/// The hero model is 0.78 m tall at scale 1.
const HERO_MODEL_M: f32 = 0.78;
/// Landing puffs drawn at once.
const PUFFS: usize = 8;
/// Far stars.
const STARS: usize = 60;

fn default_audio() -> Value {
    let sfx = "assets/audio/oga-retro-synth-sfx/";
    let cue =
        |f: &str, v: f64, r: f64| json!({"file": format!("{sfx}{f}"), "volume": v, "rate": r});
    json!({
        "enabled": true,
        "master": 0.8,
        "sfx_volume": 1.0,
        "bgm_volume": 0.5,
        "bgm": {"file": "assets/audio/oga-flowerbed-fields/flowerbed_fields.mp3", "volume": 0.5, "fade_secs": 0.5},
        "sfx": {
            "jump": cue("shot_01.wav", 0.18, 1.9),
            "dash": cue("shot_01.wav", 0.3, 0.7),
            "land": {"file": "assets/audio/kenney-impact-sounds/impactGeneric_light_001.wav", "volume": 0.35, "rate": 1.2},
            "cell": cue("retro_coin_01.wav", 0.4, 1.2),
            "check": cue("power_up_02.wav", 0.5, 1.3),
            "fall": cue("retro_die_01.wav", 0.6, 1.0),
            "stomp": {"file": "assets/audio/kenney-impact-sounds/impactGeneric_light_001.wav", "volume": 0.6, "rate": 1.6},
            "boom": cue("retro_explosion_02.wav", 0.7, 0.8),
            "shot": cue("shot_01.wav", 0.12, 1.3),
            "crumble": {"file": "assets/audio/kenney-impact-sounds/impactGeneric_light_001.wav", "volume": 0.4, "rate": 0.6},
            "clear": cue("power_up_04.wav", 0.8, 1.0),
            "go": {"file": "assets/audio/kenney-interface-sounds/confirmation_001.wav", "volume": 0.7, "rate": 1.0}
        }
    })
}

// ----------------------------------------------------------------- run ----

/// Where the run is inside the play phase.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Beat {
    /// The story card; a tap (or the jump) starts the stage.
    Intro,
    Run,
    /// Fell: seconds until the respawn.
    Down(f32),
    /// The clear card; a tap goes on.
    Clear,
}

/// A crumbling ledge: seconds since first stood on (None = untouched), seconds until back.
#[derive(Clone, Copy, Debug, Default)]
struct Crumble {
    since: Option<f32>,
    gone: f32,
}

/// One stage's numbers, for the clear card and the total.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Tally {
    secs: f32,
    cells: u32,
    falls: u32,
    wrecks: u32,
    score: u32,
}

/// Indices of the props a frame edits.
#[derive(Clone, Debug, Default)]
struct Slots {
    hero: usize,
    blocks: Vec<usize>,
    lasers: Vec<usize>,
    cells: Vec<usize>,
    checks: Vec<usize>,
    exit: usize,
    puffs: usize,
    meteors: usize,
    drones: Vec<usize>,
    bolts: usize,
    /// The first cone of each spike row.
    spikes: Vec<usize>,
    boss: Option<usize>,
}

#[derive(Debug)]
pub struct StationEscape {
    spec: StationEscapeSpec,
    core: Core,
    flow: GameFlow,
    doc: WorldDoc,
    slots: Slots,
    /// The stage the doc was built for.
    doc_stage: Option<usize>,
    stage: usize,
    /// The stage a run starts at (0 unless a link asks for another).
    first_stage: usize,
    beat: Beat,
    /// Seconds on the stage clock (from Run), and the picture's clock.
    t: f32,
    clock: f32,
    body: Body,
    solids: Vec<Solid>,
    /// Which block each solid is (for crumbling).
    solid_block: Vec<usize>,
    crumbles: Vec<Crumble>,
    got: Vec<bool>,
    check: Option<usize>,
    falls: u32,
    wrecks: u32,
    foes: foes::Foes,
    warden: Option<boss::Warden>,
    /// Kept on the device (`page.save`): stages cleared, best clear time per stage.
    progress: Progress,
    /// The stick or keys' last sideways push on the first story card (stage select).
    prev_pick: f32,
    tallies: Vec<Tally>,
    puffs: [[f32; 3]; PUFFS],
    next_puff: usize,
    /// Smoothed camera centre.
    cam: (f32, f32),
    /// Body squash after landing (s left) and stretch after a jump.
    squash: f32,
    // Input.
    keys_x: f32,
    keys_jump: bool,
    keys_dash: bool,
    prev_jump: bool,
    prev_dash: bool,
    /// Touch: the stick thumb (id, position, base), the jump thumb, a dash tap waiting.
    stick_id: Option<u32>,
    finger: Option<(f32, f32)>,
    base: (f32, f32),
    jump_id: Option<u32>,
    dash_tap: bool,
    jump_tap: bool,
    win: bool,
    new_best: bool,
    best_before: u32,
    lang_ja: bool,
    size: Cell<(f32, f32)>,
}

impl StationEscape {
    pub fn new(spec: StationEscapeSpec) -> Self {
        let mut flow = GameFlow::new(spec.id.clone());
        flow.title = spec.title.clone();
        flow.tagline = String::new();
        let spawn = spec.stages[0].spawn;
        let progress = Progress::new(spec.stages.len(), "best_secs", Better::Lower);
        let mut g = Self {
            spec,
            core: Core::default(),
            flow,
            doc: WorldDoc::default(),
            slots: Slots::default(),
            doc_stage: None,
            stage: 0,
            first_stage: 0,
            beat: Beat::Intro,
            t: 0.0,
            clock: 0.0,
            body: Body::at(spawn[0], spawn[1]),
            solids: Vec::new(),
            solid_block: Vec::new(),
            crumbles: Vec::new(),
            got: Vec::new(),
            check: None,
            falls: 0,
            wrecks: 0,
            foes: foes::Foes::default(),
            warden: None,
            progress,
            prev_pick: 0.0,
            tallies: Vec::new(),
            puffs: [[0.0; 3]; PUFFS],
            next_puff: 0,
            cam: (spawn[0], spawn[1]),
            squash: 0.0,
            keys_x: 0.0,
            keys_jump: false,
            keys_dash: false,
            prev_jump: false,
            prev_dash: false,
            stick_id: None,
            finger: None,
            base: (0.0, 0.0),
            jump_id: None,
            dash_tap: false,
            jump_tap: false,
            win: false,
            new_best: false,
            best_before: 0,
            lang_ja: false,
            size: Cell::new((390.0, 844.0)),
        };
        g.load_stage(0);
        g.build_doc();
        g
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| format!("station_escape dump: {e}"))?;
        let spec = match value.get("game") {
            Some(g) => StationEscapeSpec::from_value(g)?,
            None => StationEscapeSpec::default(),
        };
        let mut g = Self::new(spec);
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            g.flow.best_score = best as u32;
        }
        // What the device kept (`page.save`): the stages cleared and the records.
        if let Some(sv) = value.get("save") {
            g.progress.load(sv);
        }
        // `?stage=3` on the preview link (the manifest's `page.url_params`) starts there.
        let stage = value.get("stage").and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        });
        if let Some(n) = stage {
            g.first_stage = (n.max(1) as usize - 1).min(g.spec.stages.len() - 1);
            g.load_stage(g.first_stage);
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

    fn st(&self) -> &Stage {
        &self.spec.stages[self.stage]
    }

    // ---- flow

    fn load_stage(&mut self, i: usize) {
        self.stage = i.min(self.spec.stages.len() - 1);
        let st = &self.spec.stages[self.stage];
        self.crumbles = vec![Crumble::default(); st.blocks.len()];
        self.got = vec![false; st.cells.len()];
        self.check = None;
        self.falls = 0;
        self.wrecks = 0;
        self.foes = foes::Foes::new(&st.drones, &st.turrets);
        self.warden = st.boss.as_ref().map(boss::Warden::new);
        self.t = 0.0;
        self.beat = Beat::Intro;
        self.body = Body::at(st.spawn[0], st.spawn[1]);
        self.cam = (st.spawn[0], st.spawn[1]);
        self.squash = 0.0;
        self.puffs = [[0.0; 3]; PUFFS];
        self.update_solids();
    }

    pub fn confirm(&mut self) {
        match self.flow.phase {
            FlowPhase::Title | FlowPhase::Result => {
                self.core = Core::default();
                self.tallies.clear();
                self.win = false;
                self.new_best = false;
                self.load_stage(self.first_stage);
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
            self.release_touches();
        }
    }

    fn release_touches(&mut self) {
        self.stick_id = None;
        self.finger = None;
        self.jump_id = None;
        self.jump_tap = false;
        self.dash_tap = false;
    }

    /// A tap on a card (or the jump key): start the stage, or go on after a clear.
    fn advance(&mut self) {
        match self.beat {
            Beat::Intro => {
                self.beat = Beat::Run;
                crate::mixer::cue_sfx("go");
            }
            Beat::Clear => {
                if self.stage + 1 < self.spec.stages.len() {
                    self.load_stage(self.stage + 1);
                } else {
                    self.finish();
                }
            }
            _ => {}
        }
    }

    fn finish(&mut self) {
        self.win = true;
        crate::mixer::stop_bgm(0.6);
        let score = self.core.stats.score;
        self.new_best = score > self.best_before;
        self.flow.finish(score, self.cells_total(), "ESCAPED");
    }

    /// Stage select is open on the run's first story card, once a stage has been cleared.
    fn can_pick(&self) -> bool {
        self.beat == Beat::Intro && self.tallies.is_empty() && self.progress.can_pick()
    }

    /// Step the first stage of the run by `dir` among the stages reached (cleared + 1).
    fn pick_stage(&mut self, dir: i32) {
        if !self.can_pick() {
            return;
        }
        let next = self.progress.step(self.stage, dir);
        if next != self.stage {
            self.load_stage(next);
            crate::mixer::cue_sfx("check");
        }
    }

    fn cells_total(&self) -> u32 {
        self.tallies.iter().map(|t| t.cells).sum()
    }

    // ---- sim

    /// Where each block is at stage time `t` (moving ones travel and come back).
    fn block_at(b: &Block, t: f32) -> (f32, f32, (f32, f32)) {
        if b.kind != "move" {
            return (b.x, b.y, (0.0, 0.0));
        }
        let period = 2.0 * b.secs;
        let u = t.rem_euclid(period) / b.secs;
        // Ease in and out, out and back.
        let (k, dir) = if u < 1.0 { (u, 1.0) } else { (2.0 - u, -1.0) };
        let e = 0.5 - 0.5 * (k * std::f32::consts::PI).cos();
        let de = 0.5 * std::f32::consts::PI * (k * std::f32::consts::PI).sin() / b.secs * dir;
        (
            b.x + b.to[0] * e,
            b.y + b.to[1] * e,
            (b.to[0] * de, b.to[1] * de),
        )
    }

    fn update_solids(&mut self) {
        self.solids.clear();
        self.solid_block.clear();
        let t = self.t;
        for (i, b) in self.spec.stages[self.stage].blocks.iter().enumerate() {
            if b.kind == "crumble" && self.crumbles[i].gone > 0.0 {
                continue;
            }
            let (x, y, vel) = Self::block_at(b, t);
            self.solids.push(Solid {
                x0: x,
                x1: x + b.w,
                y0: y,
                y1: y + b.h,
                vel,
                one_way: b.kind == "oneway",
            });
            self.solid_block.push(i);
        }
    }

    fn puff(&mut self, x: f32, y: f32) {
        self.puffs[self.next_puff] = [x, y, 0.35];
        self.next_puff = (self.next_puff + 1) % PUFFS;
    }

    fn fall(&mut self) {
        self.falls += 1;
        self.beat = Beat::Down(self.spec.respawn_secs);
        self.core.juice.hit(0.25, 0.06);
        crate::mixer::cue_sfx("fall");
    }

    fn respawn(&mut self) {
        let st = &self.spec.stages[self.stage];
        let at = match self.check {
            Some(c) => st.checkpoints[c],
            None => st.spawn,
        };
        self.body.reset_to(at[0], at[1] + 0.05);
        self.body.facing = 1.0;
        for c in &mut self.crumbles {
            *c = Crumble::default();
        }
        self.foes.reset(&st.turrets);
        if let (Some(w), Some(b)) = (self.warden.as_mut(), st.boss.as_ref()) {
            w.restart(b);
        }
        self.beat = Beat::Run;
    }

    fn clear_stage(&mut self) {
        let st = self.st();
        let cells = self.got.iter().filter(|g| **g).count() as u32;
        let under = (st.par_secs - self.t).max(0.0);
        let score = (self.spec.clear_score
            + cells * self.spec.cell_score
            + self.wrecks * self.spec.foe_score
            + if self.warden.as_ref().is_some_and(|w| w.dead()) {
                self.spec.boss_score
            } else {
                0
            }
            + (under * self.spec.par_score as f32) as u32)
            .saturating_sub(self.falls * self.spec.fall_cost)
            .max(100);
        self.tallies.push(Tally {
            secs: self.t,
            cells,
            falls: self.falls,
            wrecks: self.wrecks,
            score,
        });
        self.core.stats.score += score;
        self.core.stats.rings = self.cells_total();
        self.beat = Beat::Clear;
        self.progress.clear(self.stage, self.t);
        crate::mixer::cue_sfx("clear");
    }

    /// One fixed step of the rules.
    fn step_sim(&mut self, dt: f32) {
        self.clock += dt;
        for p in &mut self.puffs {
            p[2] = (p[2] - dt).max(0.0);
        }
        self.squash = (self.squash - dt).max(0.0);
        // This step's buttons: held, and new presses since the last step.
        let jump_held = self.keys_jump || self.jump_id.is_some();
        let dash_held = self.keys_dash;
        let jump_pressed = (jump_held && !self.prev_jump) || self.jump_tap;
        let dash_pressed = (dash_held && !self.prev_dash) || self.dash_tap;
        self.prev_jump = jump_held;
        self.prev_dash = dash_held;
        self.jump_tap = false;
        self.dash_tap = false;
        match self.beat {
            Beat::Intro | Beat::Clear => {
                // On the run's first story card, left / right picks among the stages reached.
                let pick = if self.keys_x.abs() > 0.5 {
                    self.keys_x.signum()
                } else {
                    0.0
                };
                if pick != 0.0 && pick != self.prev_pick {
                    self.pick_stage(pick as i32);
                }
                self.prev_pick = pick;
                if jump_pressed {
                    self.advance();
                }
                self.follow_cam(dt);
                return;
            }
            Beat::Down(left) => {
                let left = left - dt;
                if left <= 0.0 {
                    self.respawn();
                } else {
                    self.beat = Beat::Down(left);
                }
                return;
            }
            Beat::Run => {}
        }
        self.t += dt;
        // Crumbling ledges count down, fall, and come back.
        for c in &mut self.crumbles {
            if c.gone > 0.0 {
                c.gone -= dt;
                if c.gone <= 0.0 {
                    *c = Crumble::default();
                }
            } else if let Some(s) = c.since.as_mut() {
                *s += dt;
            }
        }
        let mut crumbled = false;
        for c in &mut self.crumbles {
            if c.since.is_some_and(|s| s >= self.spec.crumble_secs) {
                c.since = None;
                c.gone = self.spec.crumble_back;
                crumbled = true;
            }
        }
        if crumbled {
            crate::mixer::cue_sfx("crumble");
        }
        self.update_solids();
        // The body.
        let (w, h) = self.size.get();
        let mx = match self.finger {
            Some(f) => stick::analog(self.base, f, self.stick_radius(w, h), self.spec.stick_dead).0,
            None => self.keys_x,
        };
        let input = BodyInput {
            move_x: mx,
            jump: jump_held,
            jump_pressed,
            dash_pressed,
        };
        let ev = self.body.step(&self.spec.body, input, &self.solids, dt);
        if ev.jumped {
            crate::mixer::cue_sfx("jump");
        }
        if ev.dashed {
            crate::mixer::cue_sfx("dash");
            self.puff(self.body.x, self.body.y + 0.5);
        }
        if let Some(v) = ev.landed {
            if v > 4.0 {
                self.squash = 0.12;
                self.puff(self.body.x, self.body.y);
                crate::mixer::cue_sfx("land");
            }
        }
        if let Some(g) = ev.ground {
            let b = self.solid_block[g];
            if self.st().blocks[b].kind == "crumble" && self.crumbles[b].since.is_none() {
                self.crumbles[b].since = Some(0.0);
            }
        }
        let spec = self.spec.body.clone();
        // Drones and turrets.
        let st = &self.spec.stages[self.stage];
        let hits = self.foes.step(
            &st.drones,
            &st.turrets,
            &self.solids,
            &mut self.body,
            &spec,
            self.t,
            dt,
            self.spec.stomp_bounce,
        );
        let wrecked = hits.stomped + hits.wrecked;
        if wrecked > 0 {
            self.wrecks += wrecked;
            self.core.juice.hit(0.12, 0.05);
            self.puff(self.body.x, self.body.y);
            crate::mixer::cue_sfx("stomp");
        }
        if hits.fired > 0 && self.foes_near() {
            crate::mixer::cue_sfx("shot");
        }
        // The Warden.
        let mut boss_hurt = false;
        if let (Some(w), Some(b)) = (
            self.warden.as_mut(),
            self.spec.stages[self.stage].boss.as_ref(),
        ) {
            let bh = w.step(
                b,
                &mut self.foes,
                &mut self.body,
                &spec,
                dt,
                self.spec.stomp_bounce,
            );
            boss_hurt = bh.hurt;
            if bh.fired > 0 {
                crate::mixer::cue_sfx("shot");
            }
            if bh.hit {
                self.core.juice.hit(0.3, 0.08);
                crate::mixer::cue_sfx("stomp");
            }
            if bh.killed {
                self.core.juice.hit(0.6, 0.15);
                crate::mixer::cue_sfx("boom");
                for k in 0..PUFFS {
                    let a = k as f32 * 0.8;
                    self.puffs[k] = [w.x + a.cos() * 0.9, w.y - 0.5 + a.sin() * 0.6, 0.35];
                }
            }
        }
        let st = &self.spec.stages[self.stage];
        // Cells.
        let mut picked = 0;
        for (i, c) in st.cells.iter().enumerate() {
            if !self.got[i]
                && touches(
                    &self.body,
                    &spec,
                    c[0] - 0.35,
                    c[0] + 0.35,
                    c[1] - 0.35,
                    c[1] + 0.35,
                )
            {
                self.got[i] = true;
                picked += 1;
            }
        }
        // Checkpoints: the furthest one passed.
        let mut checked = None;
        for (i, c) in st.checkpoints.iter().enumerate() {
            if self.check.is_none_or(|k| i > k)
                && touches(
                    &self.body,
                    &spec,
                    c[0] - 0.6,
                    c[0] + 0.6,
                    c[1] - 0.2,
                    c[1] + 2.5,
                )
            {
                checked = Some(i);
            }
        }
        let hurt = hits.hurt
            || boss_hurt
            || self.body.y < st.kill_y
            || st.spikes.iter().any(|s| {
                s.state(self.t).0
                    && touches(
                        &self.body,
                        &spec,
                        s.x + 0.1,
                        s.x + s.w - 0.1,
                        s.y,
                        s.y + s.h,
                    )
            })
            || st.lasers.iter().any(|l| {
                l.state(self.t).0 && touches(&self.body, &spec, l.x, l.x + l.w, l.y, l.y + l.h)
            });
        let open = self.warden.as_ref().is_none_or(|w| w.dead());
        let out = open
            && touches(
                &self.body,
                &spec,
                st.exit[0] - 0.5,
                st.exit[0] + 0.5,
                st.exit[1],
                st.exit[1] + 2.0,
            );
        if picked > 0 {
            crate::mixer::cue_sfx("cell");
            self.core.stats.just_rings += picked;
        }
        if let Some(i) = checked {
            self.check = Some(i);
            crate::mixer::cue_sfx("check");
        }
        if hurt {
            self.fall();
        } else if out {
            self.clear_stage();
        }
        self.follow_cam(dt);
    }

    /// Any turret within earshot (a shot far off screen stays quiet).
    fn foes_near(&self) -> bool {
        self.st()
            .turrets
            .iter()
            .any(|t| (t.x - self.body.x).abs() < 14.0)
    }

    fn follow_cam(&mut self, dt: f32) {
        let k = (dt * 5.0).min(1.0);
        let ahead = self.body.facing * 1.6 + self.body.vx * 0.12;
        let mut tx = self.body.x + ahead;
        let mut ty = self.body.y + 0.9;
        // In a boss fight the view leans toward the Warden so both stay in sight.
        if let Some(w) = self.warden.as_ref().filter(|w| !w.dead()) {
            tx += (w.x - tx).clamp(-6.0, 6.0) * 0.45;
            ty += ((w.y - 1.0 - ty) * 0.5).max(0.0);
        }
        self.cam.0 += (tx - self.cam.0) * k;
        // Up and down follow slower while in the air, so a jump does not shake the view.
        let ky = if self.body.grounded { k } else { k * 0.4 };
        self.cam.1 += (ty - self.cam.1) * ky;
    }

    // ---- input

    fn stick_radius(&self, w: f32, h: f32) -> f32 {
        self.spec.stick_px * hud_scale(w, h)
    }

    /// The two buttons: (centre, radius) of JUMP and DASH.
    fn buttons_at(&self, w: f32, h: f32) -> [((f32, f32), f32); 2] {
        let s = hud_scale(w, h);
        let r = self.spec.button_px * s;
        let jump = (w - r * 1.5, h - r * 1.6 - 10.0 * s);
        let dash = (jump.0 - r * 2.2, jump.1 + r * 0.25);
        [(jump, r), (dash, r * 0.85)]
    }

    fn stick_rest(&self, w: f32, h: f32) -> (f32, f32) {
        let r = self.stick_radius(w, h);
        (
            (r * 1.4).max(w * 0.16),
            h - r * 1.5 - 12.0 * hud_scale(w, h),
        )
    }

    pub fn touch_ev(&mut self, ev: PointerEvent, w: u32, h: u32) {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        self.size.set((w, h));
        if self.flow.phase != FlowPhase::Play {
            return;
        }
        let r = self.stick_radius(w, h);
        match ev.phase {
            PointerPhase::Begin => {
                if matches!(self.beat, Beat::Intro | Beat::Clear) {
                    if self.can_pick() && ev.x < w * 0.25 {
                        self.pick_stage(-1);
                    } else if self.can_pick() && ev.x > w * 0.75 {
                        self.pick_stage(1);
                    } else {
                        self.jump_tap = true;
                    }
                    return;
                }
                let [(jc, jr), (dc, dr)] = self.buttons_at(w, h);
                let near = |c: (f32, f32), rad: f32| (ev.x - c.0).hypot(ev.y - c.1) < rad * 1.35;
                if near(dc, dr) {
                    self.dash_tap = true;
                } else if near(jc, jr) || (ev.x > w * 0.5 && self.jump_id.is_none()) {
                    // The right half is all jump, except the DASH button.
                    self.jump_id = Some(ev.id);
                    self.jump_tap = true;
                } else if ev.x <= w * 0.5 && self.stick_id.is_none() {
                    self.stick_id = Some(ev.id);
                    self.finger = Some((ev.x, ev.y));
                    self.base = stick::land(ev.x, ev.y, r, 4.0 * hud_scale(w, h), w, h);
                }
            }
            PointerPhase::Move if self.stick_id == Some(ev.id) => {
                self.finger = Some((ev.x, ev.y));
                self.base = stick::follow(self.base, (ev.x, ev.y), r);
            }
            PointerPhase::End | PointerPhase::Cancel => {
                if self.stick_id == Some(ev.id) {
                    self.stick_id = None;
                    self.finger = None;
                }
                if self.jump_id == Some(ev.id) {
                    self.jump_id = None;
                }
            }
            _ => {}
        }
    }

    // ---- picture

    /// The whole stage, once: scenery, then the props a frame edits.
    fn build_doc(&mut self) {
        let st = self.spec.stages[self.stage].clone();
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: 60.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([4, 6, 16, 255]),
            hemi_sky: Some([0.22, 0.26, 0.46]),
            hemi_ground: Some([0.1, 0.09, 0.15]),
            env_map: Some("preset:studio".into()),
            ibl: Some(0.25),
            exposure: Some(0.95),
            bloom: Some(0.6),
            bloom_threshold: Some(0.85),
            vignette: Some(0.45),
            fxaa: Some(true),
            tonemap: Some(true),
            ..WorldDoc::default()
        };
        let mut models = vec![HERO_GLB, CELL_GLB];
        for d in &st.decos {
            models.push(d.model.as_str());
        }
        models.push("assets/models/kenney/space-kit/meteor.glb");
        models.push(DRONE_GLB);
        models.push(TURRET_GLB);
        declare_models(&mut doc, &models);
        let model = |id: &str, path: &str, name: &str, pos: [f32; 3], s: f32, col: [u32; 3]| {
            if glb_ready(path) {
                glb_prop(id, path, name, pos, [s; 3], [255, 255, 255], 0.0, None)
            } else {
                prop(id, "box", name, pos, [s * 0.5; 3], col, 0.0, 0.7, 0.1)
            }
        };
        // The night: a far dark wall and stars on it.
        let (x0, x1) = st.blocks.iter().fold((f32::MAX, f32::MIN), |(a, b), k| {
            (a.min(k.x), b.max(k.x + k.w + k.to[0].max(0.0)))
        });
        let mid = 0.5 * (x0 + x1);
        doc.props.push(prop(
            "space",
            "box",
            "space",
            [mid, 0.0, -40.0],
            [x1 - x0 + 160.0, 140.0, 1.0],
            [5, 6, 14],
            0.0,
            1.0,
            0.0,
        ));
        let mut seed = 0x9E37_79B9_u32 ^ (self.stage as u32 * 7919);
        let mut rnd = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((seed >> 8) & 0xFFFF) as f32 / 65535.0
        };
        for i in 0..STARS {
            let x = x0 - 40.0 + rnd() * (x1 - x0 + 80.0);
            let y = -20.0 + rnd() * 60.0;
            let s = 0.08 + 0.18 * rnd();
            let warm = rnd() > 0.8;
            doc.props.push(prop(
                &format!("star-{i}"),
                "sphere",
                "star",
                [x, y, -38.0],
                [s; 3],
                if warm {
                    [255, 220, 180]
                } else {
                    [190, 210, 255]
                },
                3.0 + 3.0 * rnd(),
                0.5,
                0.0,
            ));
        }
        for (i, d) in st.decos.iter().enumerate() {
            let mut p = model(
                &format!("deco-{i}"),
                &d.model,
                "deco",
                [d.x, d.y, d.z],
                d.scale,
                [70, 80, 110],
            );
            p.yaw = d.yaw;
            p.color = Some([120, 130, 165]);
            doc.props.push(p);
        }
        let mut slots = Slots::default();
        // Spikes: a row of small cones (they sink into the floor when pulled in).
        for (i, s) in st.spikes.iter().enumerate() {
            slots.spikes.push(doc.props.len());
            let n = (s.w / 0.4).ceil().max(1.0) as usize;
            for k in 0..n {
                doc.props.push(prop(
                    &format!("spike-{i}-{k}"),
                    "cone",
                    "spike",
                    [
                        s.x + (k as f32 + 0.5) * s.w / n as f32,
                        s.y + s.h * 0.5,
                        0.0,
                    ],
                    [0.3, s.h, 0.9],
                    [200, 70, 80],
                    0.5,
                    0.4,
                    0.6,
                ));
            }
        }
        // Blocks: a hull and a glowing trim on the top edge (oneway ledges are thin and teal,
        // crumbling ones amber, moving ones violet).
        for (i, b) in st.blocks.iter().enumerate() {
            let (col, trim) = match b.kind.as_str() {
                "oneway" => ([60, 92, 110], [0.3, 1.0, 0.9]),
                "crumble" => ([110, 88, 64], [1.0, 0.6, 0.2]),
                "move" => ([82, 72, 120], [0.7, 0.5, 1.0]),
                _ => ([58, 64, 84], [0.35, 0.6, 1.0]),
            };
            // A floor slab reaches down out of sight, so a tall phone sees a structure, not a
            // thin shelf over the void (the solid itself stays as specified).
            let deep = if b.kind == "solid" && b.y <= -2.5 {
                -30.0
            } else {
                0.0
            };
            let mut p = prop(
                &format!("block-{i}"),
                "box",
                "block",
                [b.x + b.w * 0.5, b.y + deep + (b.h - deep) * 0.5, 0.0],
                [b.w, b.h - deep, 2.4],
                col,
                0.0,
                0.55,
                0.5,
            );
            p.emissive_color = Some(trim);
            slots.blocks.push(doc.props.len());
            doc.props.push(p);
            doc.props.push(prop(
                &format!("trim-{i}"),
                "box",
                "trim",
                [b.x + b.w * 0.5, b.y + b.h, 1.21],
                [b.w, 0.06, 0.04],
                [255, 255, 255],
                0.0,
                0.4,
                0.0,
            ));
            let t = doc.props.len() - 1;
            doc.props[t].emissive = Some(2.2);
            doc.props[t].emissive_color = Some(trim);
        }
        for (i, l) in st.lasers.iter().enumerate() {
            // Emitter caps, then the beam.
            for (k, y) in [l.y, l.y + l.h].iter().enumerate() {
                doc.props.push(prop(
                    &format!("emitter-{i}-{k}"),
                    "box",
                    "emitter",
                    [l.x + l.w * 0.5, *y, 0.0],
                    [0.45, 0.2, 0.6],
                    [90, 60, 70],
                    0.0,
                    0.4,
                    0.6,
                ));
            }
            slots.lasers.push(doc.props.len());
            let mut beam = prop(
                &format!("laser-{i}"),
                "box",
                "laser",
                [l.x + l.w * 0.5, l.y + l.h * 0.5, 0.0],
                [l.w, l.h, l.w],
                [255, 80, 90],
                4.0,
                0.3,
                0.0,
            );
            beam.emissive_color = Some([1.0, 0.2, 0.25]);
            doc.props.push(beam);
        }
        for (i, c) in st.cells.iter().enumerate() {
            slots.cells.push(doc.props.len());
            let mut p = model(
                &format!("cell-{i}"),
                CELL_GLB,
                "cell",
                [c[0], c[1] - 0.25, 0.0],
                1.5,
                [120, 230, 255],
            );
            p.emissive = Some(0.9);
            p.emissive_color = Some([0.3, 0.85, 1.0]);
            p.color = Some([150, 235, 255]);
            doc.props.push(p);
        }
        for (i, c) in st.checkpoints.iter().enumerate() {
            doc.props.push(prop(
                &format!("pole-{i}"),
                "box",
                "pole",
                [c[0], c[1] + 1.0, -0.6],
                [0.12, 2.0, 0.12],
                [120, 130, 150],
                0.0,
                0.5,
                0.6,
            ));
            slots.checks.push(doc.props.len());
            doc.props.push(prop(
                &format!("beacon-{i}"),
                "sphere",
                "beacon",
                [c[0], c[1] + 2.1, -0.6],
                [0.32; 3],
                [255, 120, 90],
                2.0,
                0.4,
                0.0,
            ));
        }
        // The exit: a frame of light.
        slots.exit = doc.props.len();
        let e = st.exit;
        for (k, (dx, dy, sw, sh)) in [
            (-0.7, 1.2, 0.18, 2.4),
            (0.7, 1.2, 0.18, 2.4),
            (0.0, 2.45, 1.58, 0.18),
        ]
        .iter()
        .enumerate()
        {
            let mut p = prop(
                &format!("exit-{k}"),
                "box",
                "exit",
                [e[0] + dx, e[1] + dy, -0.4],
                [*sw, *sh, 0.3],
                [140, 255, 190],
                3.0,
                0.4,
                0.0,
            );
            p.emissive_color = Some([0.4, 1.0, 0.6]);
            doc.props.push(p);
        }
        // Meteors drifting far behind.
        slots.meteors = doc.props.len();
        for i in 0..4 {
            let mut p = model(
                &format!("meteor-{i}"),
                "assets/models/kenney/space-kit/meteor.glb",
                "meteor",
                [
                    x0 + (x1 - x0) * (0.15 + 0.25 * i as f32),
                    8.0 + 3.0 * i as f32,
                    -22.0,
                ],
                3.0 + i as f32,
                [110, 100, 95],
            );
            p.color = Some([150, 130, 120]);
            doc.props.push(p);
        }
        slots.puffs = doc.props.len();
        for i in 0..PUFFS {
            let mut p = prop(
                &format!("puff-{i}"),
                "sphere",
                "puff",
                [0.0; 3],
                [0.3; 3],
                [200, 220, 255],
                1.5,
                0.6,
                0.0,
            );
            p.enabled = false;
            doc.props.push(p);
        }
        for (i, tu) in st.turrets.iter().enumerate() {
            let mut p = model(
                &format!("turret-{i}"),
                TURRET_GLB,
                "turret",
                [tu.x - tu.dir * 0.3, tu.y - 0.6, 0.0],
                1.8,
                [170, 90, 90],
            );
            // The barrel points along +z at yaw 0: turn it to face along the fire.
            p.yaw = tu.dir.signum() * std::f32::consts::FRAC_PI_2;
            p.color = Some([210, 150, 150]);
            doc.props.push(p);
        }
        for (i, _) in st.drones.iter().enumerate() {
            slots.drones.push(doc.props.len());
            let mut p = model(
                &format!("drone-{i}"),
                DRONE_GLB,
                "drone",
                [0.0; 3],
                0.62,
                [220, 120, 90],
            );
            p.color = Some([255, 130, 110]);
            p.emissive = Some(0.7);
            p.emissive_color = Some([1.0, 0.35, 0.2]);
            doc.props.push(p);
        }
        if st.boss.is_some() {
            slots.boss = Some(doc.props.len());
            let mut p = model("warden", DRONE_GLB, "warden", [0.0; 3], 1.5, [200, 90, 90]);
            p.color = Some([235, 120, 120]);
            p.emissive = Some(0.5);
            p.emissive_color = Some([1.0, 0.3, 0.2]);
            doc.props.push(p);
        }
        slots.bolts = doc.props.len();
        for i in 0..foes::BOLTS {
            let mut p = prop(
                &format!("bolt-{i}"),
                "sphere",
                "bolt",
                [0.0; 3],
                [0.32, 0.22, 0.22],
                [255, 150, 90],
                4.0,
                0.3,
                0.0,
            );
            p.emissive_color = Some([1.0, 0.45, 0.2]);
            p.enabled = false;
            doc.props.push(p);
        }
        slots.hero = doc.props.len();
        let mut hero = model("hero", HERO_GLB, "hero", [0.0; 3], 1.0, [255, 200, 120]);
        hero.emissive = Some(0.05);
        doc.props.push(hero);
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "side".into(),
            position: [0.0, 2.0, 12.0],
            target: [0.0, 1.0, 0.0],
            fov: 40.0,
            roll: 0.0,
        });
        doc.lights.push(light(
            0,
            "key",
            [-6.0, 14.0, 10.0],
            [0.0; 3],
            [0.75, 0.82, 1.0],
            1.1,
            80.0,
            false,
        ));
        doc.lights.push(light(
            1,
            "hero",
            [0.0, 2.0, 3.0],
            [0.0; 3],
            [1.0, 0.8, 0.55],
            1.4,
            8.0,
            false,
        ));
        self.slots = slots;
        self.doc = doc;
        self.doc_stage = Some(self.stage);
        self.update_doc();
    }

    /// A frame: move what moves, light what is lit.
    fn update_doc(&mut self) {
        if self.doc_stage != Some(self.stage) {
            self.build_doc();
            return;
        }
        let st = &self.spec.stages[self.stage];
        let t = self.t;
        let props = &mut self.doc.props;
        for (i, b) in st.blocks.iter().enumerate() {
            let (x, y, _) = Self::block_at(b, t);
            let c = self.crumbles[i];
            let shake = match c.since {
                Some(s) => 0.05 * (s * 60.0).sin() * (s / self.spec.crumble_secs.max(0.01)),
                None => 0.0,
            };
            let on = !(b.kind == "crumble" && c.gone > 0.0);
            let deep = if b.kind == "solid" && b.y <= -2.5 {
                -30.0
            } else {
                0.0
            };
            let p = &mut props[self.slots.blocks[i]];
            p.position = [x + b.w * 0.5 + shake, y + deep + (b.h - deep) * 0.5, 0.0];
            p.enabled = on;
            let trim = &mut props[self.slots.blocks[i] + 1];
            trim.position = [x + b.w * 0.5 + shake, y + b.h, 1.21];
            trim.enabled = on;
        }
        for (i, l) in st.lasers.iter().enumerate() {
            let (on, warn) = l.state(t);
            let p = &mut props[self.slots.lasers[i]];
            p.enabled = on || warn;
            let flick = 1.0 + 0.15 * (self.clock * 40.0).sin();
            p.scale = if on {
                [l.w * flick, l.h, l.w * flick]
            } else {
                [0.04, l.h, 0.04]
            };
            p.emissive = Some(if on { 4.0 } else { 1.2 });
        }
        for (i, c) in st.cells.iter().enumerate() {
            let p = &mut props[self.slots.cells[i]];
            p.enabled = !self.got[i];
            p.position = [
                c[0],
                c[1] - 0.25 + 0.1 * (self.clock * 3.0 + i as f32).sin(),
                0.0,
            ];
            p.yaw = self.clock * 2.0 + i as f32;
        }
        for i in 0..st.checkpoints.len() {
            let lit = self.check.is_some_and(|k| i <= k);
            let p = &mut props[self.slots.checks[i]];
            p.color = Some(if lit { [120, 255, 170] } else { [255, 120, 90] });
            p.emissive_color = Some(if lit {
                [0.4, 1.0, 0.6]
            } else {
                [1.0, 0.4, 0.3]
            });
        }
        // The exit glows green when open, dim red while the Warden still flies.
        let open = self.warden.as_ref().is_none_or(|w| w.dead());
        for k in 0..3 {
            let p = &mut props[self.slots.exit + k];
            p.emissive = Some(if open {
                2.5 + 1.0 * (self.clock * 3.0).sin()
            } else {
                0.8
            });
            p.emissive_color = Some(if open {
                [0.4, 1.0, 0.6]
            } else {
                [1.0, 0.25, 0.2]
            });
            p.color = Some(if open { [140, 255, 190] } else { [180, 70, 70] });
        }
        if let (Some(i), Some(w)) = (self.slots.boss, self.warden.as_ref()) {
            let p = &mut props[i];
            p.enabled = !w.dead();
            p.position = [w.x, w.y - 0.45, 0.0];
            let lean = match w.beat {
                boss::Beat::Swoop => -0.35 * w.dir,
                boss::Beat::Warn => 0.25 * (self.clock * 30.0).sin(),
                _ => 0.08 * (self.clock * 1.7).sin(),
            };
            p.yaw = w.dir * std::f32::consts::FRAC_PI_2;
            p.roll = lean;
            let warn = matches!(w.beat, boss::Beat::Warn | boss::Beat::Swoop);
            p.emissive = Some(if w.flash > 0.0 {
                3.0
            } else if warn {
                1.6
            } else {
                0.5
            });
            p.emissive_color = Some(if w.flash > 0.0 {
                [1.0, 1.0, 1.0]
            } else {
                [1.0, 0.3, 0.2]
            });
        }
        for i in 0..4 {
            let p = &mut props[self.slots.meteors + i];
            p.yaw = self.clock * (0.05 + 0.03 * i as f32);
            p.roll = self.clock * 0.04;
        }
        for (i, sp) in st.spikes.iter().enumerate() {
            let (out, warn) = sp.state(t);
            let n = (sp.w / 0.4).ceil().max(1.0) as usize;
            let rattle = if warn {
                0.04 * (self.clock * 50.0).sin()
            } else {
                0.0
            };
            let sink = if out {
                0.0
            } else if warn {
                0.75
            } else {
                0.95
            };
            for k in 0..n {
                let p = &mut props[self.slots.spikes[i] + k];
                p.position[1] = sp.y + sp.h * 0.5 - sp.h * sink + rattle;
                p.emissive = Some(if warn { 1.6 } else { 0.5 });
            }
        }
        for (i, d) in st.drones.iter().enumerate() {
            let (x, y, dir) = d.at(t);
            let p = &mut props[self.slots.drones[i]];
            p.enabled = self.foes.alive[i];
            p.position = [x, y - 0.25, 0.0];
            p.yaw = dir * std::f32::consts::FRAC_PI_2;
            p.roll = 0.15 * (self.clock * 3.0 + i as f32).sin();
        }
        for (i, b) in self.foes.bolts.iter().enumerate() {
            let p = &mut props[self.slots.bolts + i];
            p.enabled = b.is_some();
            if let Some(b) = b {
                p.position = [b[0], b[1], 0.0];
            }
        }
        for (i, pf) in self.puffs.iter().enumerate() {
            let p = &mut props[self.slots.puffs + i];
            p.enabled = pf[2] > 0.0;
            let k = pf[2] / 0.35;
            p.position = [pf[0], pf[1] + 0.2 + (1.0 - k) * 0.3, 0.2];
            p.scale = [0.6 * (1.4 - k); 3];
            p.emissive = Some(1.5 * k);
        }
        // The hero: lean into the run, bob on the step, squash on landing, stretch in a jump.
        let b = &self.body;
        let s = self.spec.body.height / HERO_MODEL_M;
        let run = (b.vx.abs() / self.spec.body.run).min(1.5);
        let (mut wide, mut tall) = (1.0, 1.0);
        if self.squash > 0.0 {
            let k = self.squash / 0.12;
            wide += 0.25 * k;
            tall -= 0.22 * k;
        } else if !b.grounded {
            let k = (b.vy / self.spec.body.jump).clamp(-1.0, 1.0);
            wide -= 0.08 * k.abs();
            tall += 0.1 * k.abs();
        }
        let bob = if b.grounded {
            0.05 * run * (self.clock * 16.0).sin().abs()
        } else {
            0.0
        };
        let down = matches!(self.beat, Beat::Down(_));
        let hero = &mut props[self.slots.hero];
        hero.enabled = !down || (self.clock * 20.0).sin() > 0.0;
        hero.position = [b.x, b.y + bob, 0.0];
        hero.scale = [s * wide, s * tall, s * wide];
        // Front is +z: turn toward the run, a little toward the camera.
        hero.yaw = b.facing * 1.15;
        hero.pitch = -(0.12 * run.min(1.0) + if b.dashing() { 0.25 } else { 0.0 });
        hero.emissive = Some(if b.dashing() { 0.8 } else { 0.05 });
        hero.emissive_color = Some([0.5, 0.8, 1.0]);
        // Camera and the hero's light.
        let (w, h) = self.size.get();
        let view = self.view(w / h.max(1.0));
        let sh = self.core.juice.shake;
        let cam = &mut self.doc.cameras[0];
        cam.position = [
            view.eye[0] + sh * (self.clock * 53.0).sin(),
            view.eye[1] + sh * 0.5 * (self.clock * 41.0).cos(),
            view.eye[2],
        ];
        cam.target = view.target;
        cam.fov = view.fov;
        self.doc.lights[1].position = [b.x, b.y + 2.0, 3.0];
        self.doc.lights[0].position = [self.cam.0 - 6.0, self.cam.1 + 14.0, 10.0];
    }

    /// Side on: a window around the camera centre, about 13 m across and 8 m tall, whatever
    /// the phone's shape (a tall phone sees more above and below).
    fn view(&self, aspect: f32) -> View {
        let (cx, cy) = self.cam;
        let keep = [
            [cx - 6.5, cy - 3.2, 0.0],
            [cx + 6.5, cy - 3.2, 0.0],
            [cx - 6.5, cy + 4.0, 0.0],
            [cx + 6.5, cy + 4.0, 0.0],
        ];
        View::frame([cx, cy, 0.0], [0.0, 0.14, 1.0], 38.0, &keep, aspect)
    }

    // ---- HUD

    /// The in-play HUD (the page draws the title and result cards).
    pub fn hud(&self, w: u32, h: u32, _menu: bool) -> DrawList {
        let (w, h) = (w as f32, h as f32);
        self.size.set((w.max(1.0), h.max(1.0)));
        let s = hud_scale(w, h);
        let mut list = DrawList::default();
        if !matches!(self.flow.phase, FlowPhase::Play | FlowPhase::Pause) {
            return list;
        }
        let st = self.st();
        let white = [240, 244, 255, 255];
        // Top: stage, clock, cells.
        list.texts.push(TextQuad::new(
            &format!("{}  {}", self.stage + 1, self.tr(&st.name, &st.name_ja)),
            14.0 * s,
            14.0 * s,
            16.0 * s,
            [170, 200, 255, 230],
        ));
        let secs = self.t as u32;
        list.texts.push(
            TextQuad::new(
                &format!("{}:{:02}", secs / 60, secs % 60),
                w * 0.5,
                12.0 * s,
                26.0 * s,
                white,
            )
            .aligned(TextAlign::Center)
            .with_face(TextFace::Display),
        );
        let got = self.got.iter().filter(|g| **g).count();
        list.quads.push(Quad::circle(
            w - 90.0 * s,
            26.0 * s,
            7.0 * s,
            [90, 220, 255, 255],
        ));
        list.texts.push(
            TextQuad::new(
                &format!("{got}/{}", st.cells.len()),
                w - 76.0 * s,
                14.0 * s,
                22.0 * s,
                white,
            )
            .with_face(TextFace::Display),
        );
        // The Warden's health, under the clock.
        if let (Some(wd), Some(b)) = (self.warden.as_ref(), st.boss.as_ref()) {
            if !wd.dead() {
                let (bw, bh) = (180.0 * s, 9.0 * s);
                crate::hud_kit::gauge(
                    &mut list,
                    w * 0.5 - bw * 0.5,
                    48.0 * s,
                    bw,
                    bh,
                    wd.hp as f32 / b.hp as f32,
                    [255, 90, 80, 255],
                    [20, 10, 20, 180],
                );
            }
        }
        // A tutorial hint near the bottom, from the last hint passed.
        if self.beat == Beat::Run {
            if let Some(hn) = st
                .hints
                .iter()
                .filter(|hn| self.body.x >= hn.x && self.body.x < hn.x + 9.0)
                .last()
            {
                list.texts.push(
                    TextQuad::new(
                        self.tr(&hn.en, &hn.ja),
                        w * 0.5,
                        h * 0.2,
                        18.0 * s,
                        [255, 240, 210, 230],
                    )
                    .aligned(TextAlign::Center),
                );
            }
        }
        // Controls.
        if self.beat == Beat::Run || matches!(self.beat, Beat::Down(_)) {
            let rad = self.stick_radius(w, h);
            let look = stick::StickLook::default();
            match self.finger {
                Some(f) => stick::draw(
                    &mut list,
                    self.base,
                    stick::knob(self.base, f, rad),
                    rad,
                    s,
                    true,
                    &look,
                ),
                None => {
                    let c = self.stick_rest(w, h);
                    stick::draw(&mut list, c, c, rad, s, false, &look);
                }
            }
            let [(jc, jr), (dc, dr)] = self.buttons_at(w, h);
            let held = self.jump_id.is_some();
            list.quads.push(Quad::circle(
                jc.0,
                jc.1,
                jr,
                if held {
                    [120, 200, 255, 120]
                } else {
                    [40, 50, 90, 110]
                },
            ));
            list.quads
                .push(Quad::ring(jc.0, jc.1, jr, 2.5 * s, [150, 200, 255, 200]));
            list.texts.push(
                TextQuad::new(
                    self.tr("JUMP", "ジャンプ"),
                    jc.0,
                    jc.1 - 8.0 * s,
                    13.0 * s,
                    white,
                )
                .aligned(TextAlign::Center),
            );
            let ready = !self.body.dashing();
            list.quads.push(Quad::circle(
                dc.0,
                dc.1,
                dr,
                if ready {
                    [60, 40, 100, 110]
                } else {
                    [150, 110, 255, 140]
                },
            ));
            list.quads
                .push(Quad::ring(dc.0, dc.1, dr, 2.5 * s, [200, 160, 255, 200]));
            list.texts.push(
                TextQuad::new(
                    self.tr("DASH", "ダッシュ"),
                    dc.0,
                    dc.1 - 7.0 * s,
                    12.0 * s,
                    white,
                )
                .aligned(TextAlign::Center),
            );
        }
        match self.beat {
            Beat::Intro => self.card_intro(&mut list, w, h, s),
            Beat::Clear => self.card_clear(&mut list, w, h, s),
            _ => {}
        }
        if self.flow.phase == FlowPhase::Pause {
            list.texts.push(
                TextQuad::new(
                    self.tr("PAUSED", "一時停止"),
                    w * 0.5,
                    h * 0.4,
                    30.0 * s,
                    [255; 4],
                )
                .aligned(TextAlign::Center),
            );
        }
        list
    }

    fn card_intro(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        let st = self.st();
        let card = Card::open(list, w, h, s, 210.0 * s, CardStyle::default());
        card.kicker(list, &format!("STAGE {}", self.stage + 1));
        card.line(
            list,
            self.tr(&st.name, &st.name_ja),
            42.0,
            28.0,
            card.style.title,
        );
        card.body(
            list,
            self.tr(&st.story, &st.story_ja),
            88.0,
            16.0,
            self.lang_ja,
        );
        if let Some(b) = self.progress.record(self.stage) {
            card.line(
                list,
                &format!("{} {}", self.tr("BEST", "ベスト"), progress::clock(b)),
                210.0 - 58.0,
                14.0,
                [150, 255, 190, 230],
            );
        }
        if self.can_pick() {
            card.arrows(list, 40.0);
        }
        card.prompt(
            list,
            self.tr("Tap to start", "タップでスタート"),
            self.clock,
            34.0,
        );
    }

    fn card_clear(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        let Some(tl) = self.tallies.last() else {
            return;
        };
        let card = Card::open(list, w, h, s, 250.0 * s, CardStyle::default());
        card.line(
            list,
            self.tr("STAGE CLEAR", "ステージクリア"),
            18.0,
            26.0,
            [150, 255, 190, 255],
        );
        let total = self.st().cells.len();
        let rows = [
            (self.tr("Time", "タイム"), progress::clock(tl.secs)),
            (self.tr("Cells", "セル"), format!("{} / {total}", tl.cells)),
            (self.tr("Falls", "ミス"), format!("{}", tl.falls)),
            (self.tr("Drones", "ドローン"), format!("{}", tl.wrecks)),
            (self.tr("Score", "スコア"), format!("{}", tl.score)),
        ];
        card.rows(list, &rows, 62.0, 28.0);
        let next = if self.stage + 1 < self.spec.stages.len() {
            self.tr("Tap for the next stage", "タップで次のステージ")
        } else {
            self.tr("Tap to finish", "タップでおわり")
        };
        card.prompt(list, next, self.clock, 30.0);
    }

    // ---- what the page reads

    pub fn state_value(&self) -> Value {
        let beat = match self.beat {
            Beat::Intro => "intro",
            Beat::Run => "run",
            Beat::Down(_) => "down",
            Beat::Clear => "clear",
        };
        json!({
            "phase": self.flow.phase.name(),
            "beat": beat,
            "score": self.core.stats.score,
            "best": self.flow.best_score,
            "stage": self.stage + 1,
            "stages": self.spec.stages.len(),
            "secs": self.t,
            "cells": self.got.iter().filter(|g| **g).count(),
            "falls": self.falls,
            "wrecks": self.wrecks,
            "boss_hp": self.warden.as_ref().map(|w| w.hp),
            "cleared": self.progress.cleared,
            "pos": [self.body.x, self.body.y],
            "win": self.win,
            "death": self.core.stats.death,
            "death_text": "",
            "result": if self.flow.phase == FlowPhase::Result {
                let secs: f32 = self.tallies.iter().map(|t| t.secs).sum();
                json!({
                    "key": self.spec.id,
                    "title": self.spec.title,
                    "enabled": true,
                    "top_n": 5,
                    "columns": ["cells", "clear_time", "score"],
                    "sort": "score",
                    "run": {"score": self.core.stats.score, "clear_time": secs.round(), "cells": self.cells_total()},
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

impl Kit for StationEscape {
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
        self.cam.0 += dt * 0.8;
    }
    fn rebuild(&mut self) {
        self.update_doc();
    }
}

impl crate::kit_host::KitHost for StationEscape {
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
        StationEscape::confirm(self)
    }
    fn show_title(&mut self) {
        StationEscape::show_title(self)
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
        StationEscape::hud(self, width, height, menu)
    }
    fn walk(&mut self, lx: f32, _lz: f32, jump: bool) {
        self.keys_x = lx.clamp(-1.0, 1.0);
        self.keys_jump = jump;
    }
    fn save_json(&self) -> Option<String> {
        Some(self.progress.to_value().to_string())
    }
    fn buttons(&mut self, attack: bool, _dodge: bool) {
        self.keys_dash = attack;
    }
    fn touch(&mut self, ev: PointerEvent, w: u32, h: u32, _t: f32) {
        self.touch_ev(ev, w, h);
    }
    fn uses_swipe(&self) -> bool {
        // The kit draws and reads its own stick and buttons.
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

#[cfg(test)]
mod tests;
