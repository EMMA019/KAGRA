//! Horde Gates: a crowd runs down a sunny road, steers through number gates,
//! and meets zombie waves with whatever head count is left.
//!
//! Drag (or the stick / keys / gamepad) moves the whole crowd left and right.
//! Each gate is a pair (`x2`, `+10`, `-5`, `/2`). A wave removes that many
//! people when the crowd is larger, and ends the run when it is not. Five
//! levels live in the world JSON. Continue and the level end call
//! [`game_core::ads`], a no-op until a portal SDK is plugged in.
//!
//! Characters are decimated pose meshes, instanced. The soldier has no attack
//! or death clip, so those moments are a lean and a fall. Zombie attack and
//! death are baked frames of the CC0 clips.

use std::cell::Cell;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::flow::{FlowPhase, GameFlow};
use crate::game_core::ads::Ads;
use crate::game_core::crowd;
use crate::game_core::floats::{FloatStyle, Floats};
use crate::game_core::gates::{self, Op};
use crate::game_core::motion::{self, Motion};
use crate::game_core::progress::{Better, Progress};
use crate::game_core::view::View;
use crate::game_core::{declare_models, tick_frame, Core, Kit};
use crate::hop_beat::{glb_prop, glb_ready, light, prop};
use crate::hud_art::{Confetti, Painter, Stage};
use crate::hud_kit::{self, Card, CardStyle};
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, Quad, TextAlign, TextFace, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc};

pub const GAME_ID: &str = "zombie_rush";

pub fn is_zombie_rush_dump(json: &str) -> bool {
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
pub struct GateSpec {
    /// 0..1 along the level.
    pub at: f32,
    pub left: String,
    pub right: String,
}

impl Default for GateSpec {
    fn default() -> Self {
        Self {
            at: 0.5,
            left: "x2".into(),
            right: "+5".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WaveSpec {
    pub at: f32,
    pub count: u32,
    pub boss: bool,
}

impl Default for WaveSpec {
    fn default() -> Self {
        Self {
            at: 0.8,
            count: 4,
            boss: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LevelSpec {
    pub name: String,
    pub name_ja: String,
    /// How long a straight run of this level takes, before clash pauses.
    pub secs: f32,
    pub start: u32,
    /// Metres per second. The road is `secs * speed` long.
    pub speed: f32,
    pub gates: Vec<GateSpec>,
    pub waves: Vec<WaveSpec>,
}

impl Default for LevelSpec {
    fn default() -> Self {
        Self {
            name: "Mile".into(),
            name_ja: "マイル".into(),
            secs: 32.0,
            start: 8,
            speed: 8.0,
            gates: vec![],
            waves: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZombieRushSpec {
    pub id: String,
    pub title: String,
    pub title_ja: String,
    /// How fast a full stick crosses the road, metres per second.
    pub steer: f32,
    /// Half-width of the road the crowd may use, metres.
    pub road: f32,
    pub levels: Vec<LevelSpec>,
    pub audio: Option<Value>,
}

impl Default for ZombieRushSpec {
    fn default() -> Self {
        Self {
            id: GAME_ID.into(),
            title: "Horde Gates".into(),
            title_ja: "ホードゲート".into(),
            steer: 11.0,
            road: 3.35,
            levels: campaign(),
            audio: Some(default_audio()),
        }
    }
}

impl ZombieRushSpec {
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let spec: Self =
            serde_json::from_value(v.clone()).map_err(|e| format!("zombie_rush spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.levels.is_empty() || self.levels.len() > 12 {
            return Err("zombie_rush: levels must have 1..=12 entries".into());
        }
        let range = |name: &str, v: f32, lo: f32, hi: f32| {
            if v.is_finite() && (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(format!("zombie_rush: {name} must be {lo}..={hi}, got {v}"))
            }
        };
        range("steer", self.steer, 2.0, 30.0)?;
        range("road", self.road, 1.5, 8.0)?;
        for (i, lv) in self.levels.iter().enumerate() {
            let at = format!("levels[{i}]");
            range(&format!("{at}.secs"), lv.secs, 15.0, 90.0)?;
            range(&format!("{at}.speed"), lv.speed, 3.0, 18.0)?;
            if !(1..=500).contains(&lv.start) {
                return Err(format!("zombie_rush: {at}.start must be 1..=500"));
            }
            if lv.name.is_empty() || lv.name_ja.is_empty() {
                return Err(format!("zombie_rush: {at} needs a name in both languages"));
            }
            for (j, g) in lv.gates.iter().enumerate() {
                range(&format!("{at}.gates[{j}].at"), g.at, 0.05, 0.99)?;
                if Op::parse(&g.left).is_none() || Op::parse(&g.right).is_none() {
                    return Err(format!(
                        "zombie_rush: {at}.gates[{j}] needs ops like x2, +10, -5, /2"
                    ));
                }
            }
            for (j, w) in lv.waves.iter().enumerate() {
                range(&format!("{at}.waves[{j}].at"), w.at, 0.05, 0.99)?;
                if !(1..=400).contains(&w.count) {
                    return Err(format!(
                        "zombie_rush: {at}.waves[{j}].count must be 1..=400"
                    ));
                }
            }
        }
        Ok(())
    }
}

fn gate(at: f32, left: &str, right: &str) -> GateSpec {
    GateSpec {
        at,
        left: left.into(),
        right: right.into(),
    }
}

fn wave(at: f32, count: u32, boss: bool) -> WaveSpec {
    WaveSpec { at, count, boss }
}

fn level(
    name: &str,
    name_ja: &str,
    secs: f32,
    start: u32,
    speed: f32,
    gates: Vec<GateSpec>,
    waves: Vec<WaveSpec>,
) -> LevelSpec {
    LevelSpec {
        name: name.into(),
        name_ja: name_ja.into(),
        secs,
        start,
        speed,
        gates,
        waves,
    }
}

/// Five runs, each 30–60 s, each meaner than the last. The greedy-best line
/// survives and the greedy-worst line does not (`tests` below check both).
fn campaign() -> Vec<LevelSpec> {
    vec![
        level(
            "Sunny Mile",
            "サニーマイル",
            32.0,
            8,
            8.0,
            vec![
                gate(0.18, "x2", "+3"),
                gate(0.46, "+8", "-5"),
                gate(0.70, "/2", "x2"),
            ],
            vec![
                wave(0.32, 6, false),
                wave(0.58, 8, false),
                wave(0.84, 12, false),
                wave(0.95, 6, true),
            ],
        ),
        level(
            "Red Gate",
            "レッドゲート",
            38.0,
            10,
            8.8,
            vec![
                gate(0.15, "x2", "+6"),
                gate(0.42, "+18", "x2"),
                gate(0.68, "-8", "/2"),
            ],
            vec![
                wave(0.28, 6, false),
                wave(0.55, 10, false),
                wave(0.80, 8, false),
                wave(0.94, 5, true),
            ],
        ),
        level(
            "Split Road",
            "スプリット",
            44.0,
            8,
            9.2,
            vec![
                gate(0.14, "x2", "-4"),
                gate(0.40, "+12", "x2"),
                gate(0.64, "/2", "x2"),
                gate(0.88, "+10", "-8"),
            ],
            vec![
                wave(0.26, 5, false),
                wave(0.52, 10, false),
                wave(0.76, 14, false),
                wave(0.96, 16, true),
            ],
        ),
        level(
            "Thin Crowd",
            "うすいれつ",
            50.0,
            6,
            9.6,
            vec![
                gate(0.12, "x2", "+2"),
                gate(0.34, "x2", "/2"),
                gate(0.58, "+20", "x2"),
                gate(0.80, "-10", "/2"),
            ],
            vec![
                wave(0.22, 4, false),
                wave(0.46, 7, false),
                wave(0.70, 16, false),
                wave(0.90, 3, false),
                wave(0.97, 2, true),
            ],
        ),
        level(
            "Last Horde",
            "ラストホード",
            58.0,
            10,
            10.2,
            vec![
                gate(0.10, "x2", "+4"),
                gate(0.28, "x2", "-6"),
                gate(0.48, "+24", "x2"),
                gate(0.68, "x2", "/2"),
                gate(0.86, "/2", "-30"),
            ],
            vec![
                wave(0.18, 8, false),
                wave(0.38, 10, false),
                wave(0.58, 16, false),
                wave(0.78, 18, false),
                wave(0.93, 8, false),
                wave(0.98, 4, true),
            ],
        ),
    ]
}

fn default_audio() -> Value {
    let sfx = "assets/audio/oga-retro-synth-sfx/";
    let cue =
        |f: &str, v: f64, r: f64| json!({"file": format!("{sfx}{f}"), "volume": v, "rate": r});
    json!({
        "enabled": true,
        "master": 0.85,
        "sfx_volume": 1.0,
        "bgm_volume": 0.45,
        "bgm": {"file": "assets/audio/oga-flowerbed-fields/flowerbed_fields.mp3", "volume": 0.55, "fade_secs": 0.4},
        "sfx": {
            "go": {"file": "assets/audio/kenney-interface-sounds/confirmation_001.wav", "volume": 0.65, "rate": 1.0},
            "gate": cue("retro_coin_01.wav", 0.55, 1.15),
            "bad": cue("retro_die_01.wav", 0.7, 1.05),
            "hit": {"file": "assets/audio/kenney-impact-sounds/impactGeneric_light_001.wav", "volume": 0.8, "rate": 0.9},
            "win": cue("power_up_04.wav", 0.8, 1.0),
            "lose": cue("retro_die_03.wav", 0.75, 1.0),
            "save": cue("power_up_02.wav", 0.7, 1.1)
        }
    })
}

// ---------------------------------------------------------------- models ---

const Z_RUN: [&str; 4] = [
    "assets/models/zombie_rush/zombie_run_0.glb",
    "assets/models/zombie_rush/zombie_run_1.glb",
    "assets/models/zombie_rush/zombie_run_2.glb",
    "assets/models/zombie_rush/zombie_run_3.glb",
];
const Z_ATK: &str = "assets/models/zombie_rush/zombie_attack.glb";
const Z_DIE: &str = "assets/models/zombie_rush/zombie_death.glb";
const Z_LOD: &str = "assets/models/zombie_rush/zombie_lod.glb";
const H_RUN: [&str; 4] = [
    "assets/models/zombie_rush/human_run_0.glb",
    "assets/models/zombie_rush/human_run_1.glb",
    "assets/models/zombie_rush/human_run_2.glb",
    "assets/models/zombie_rush/human_run_3.glb",
];
const H_LOD: &str = "assets/models/zombie_rush/human_lod.glb";
const BARRIER: &str = "assets/models/zombie_rush/props/crush_barrier.glb";
const CONE: &str = "assets/models/zombie_rush/props/cone.glb";
const BLOCK: &str = "assets/models/zombie_rush/props/road_block.glb";

fn all_models() -> Vec<&'static str> {
    let mut m = Vec::new();
    m.extend(Z_RUN);
    m.extend([Z_ATK, Z_DIE, Z_LOD]);
    m.extend(H_RUN);
    m.extend([H_LOD, BARRIER, CONE, BLOCK]);
    m
}

// ----------------------------------------------------------------- run ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Beat {
    Run,
    Win,
    Lose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Btn {
    None,
    Retry,
    Next,
    Cont,
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Gate { left: Op, right: Op },
    Wave { count: u32, boss: bool },
}

#[derive(Clone, Copy, Debug)]
struct Event {
    at: f32,
    kind: Kind,
}

#[derive(Clone, Copy, Debug)]
struct Clash {
    foes: u32,
    boss: bool,
    age: f32,
    resolved: bool,
    won: bool,
    /// Road position of the line, so the bodies stay put while the count resolves.
    z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tier {
    Low,
    Mobile,
    High,
}

#[derive(Clone, Copy, Debug)]
struct Corpse {
    x: f32,
    z: f32,
    age: f32,
    boss: bool,
}

#[derive(Debug)]
pub struct ZombieRush {
    spec: ZombieRushSpec,
    core: Core,
    flow: GameFlow,
    doc: WorldDoc,
    lang_ja: bool,
    t: f32,
    level: usize,
    /// Metres down the road. World z is `-dist` (the camera looks toward -Z).
    dist: f32,
    x: f32,
    count: u32,
    /// Score banked from levels already left. The live score adds this level.
    bank: u32,
    keys: f32,
    finger: Option<u32>,
    last_px: f32,
    dragging: bool,
    events: Vec<Event>,
    next_event: usize,
    beat: Beat,
    card_t: f32,
    clash: Option<Clash>,
    swell: f32,
    flash: f32,
    floats: Floats,
    ads: Ads,
    continued: bool,
    count_before: u32,
    win: bool,
    new_best: bool,
    best_before: u32,
    progress: Progress,
    corpses: Vec<Corpse>,
    tier: Tier,
    ui: Cell<[(Btn, [f32; 4]); 2]>,
    size: Cell<(f32, f32)>,
}

impl ZombieRush {
    pub fn new(spec: ZombieRushSpec) -> Self {
        let n = spec.levels.len();
        let mut flow = GameFlow::new(spec.id.clone());
        flow.title = spec.title.clone();
        flow.tagline = String::new();
        let mut g = Self {
            spec,
            core: Core::default(),
            flow,
            doc: WorldDoc::default(),
            lang_ja: false,
            t: 0.0,
            level: 0,
            dist: 0.0,
            x: 0.0,
            count: 1,
            bank: 0,
            keys: 0.0,
            finger: None,
            last_px: 0.0,
            dragging: false,
            events: Vec::new(),
            next_event: 0,
            beat: Beat::Run,
            card_t: 0.0,
            clash: None,
            swell: 0.0,
            flash: 0.0,
            floats: Floats::new(FloatStyle {
                secs: 0.95,
                rise: 1.6,
                size: 46.0,
                pop: 0.62,
            }),
            ads: Ads::new(),
            continued: false,
            count_before: 1,
            win: false,
            new_best: false,
            best_before: 0,
            progress: Progress::new(n, "best_score", Better::Higher).with_places(0),
            corpses: Vec::new(),
            tier: Tier::Mobile,
            ui: Cell::new([(Btn::None, [0.0; 4]); 2]),
            size: Cell::new((390.0, 844.0)),
        };
        g.start_level();
        g.beat = Beat::Run;
        g
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| format!("zombie_rush dump: {e}"))?;
        let spec = match value.get("game") {
            Some(g) => ZombieRushSpec::from_value(g)?,
            None => ZombieRushSpec::default(),
        };
        let mut g = Self::new(spec);
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            g.flow.best_score = best as u32;
        }
        if let Some(save) = value.get("save") {
            g.progress.load(save);
        }
        Ok(g)
    }

    pub fn world_value(&self) -> Value {
        let game: Value = serde_json::to_value(&self.spec).unwrap_or(Value::Null);
        json!({"version": 1, "genre": GAME_ID, "game": game})
    }

    fn tr<'a>(&self, en: &'a str, ja: &'a str) -> &'a str {
        if self.lang_ja {
            ja
        } else {
            en
        }
    }

    fn lvl(&self) -> &LevelSpec {
        &self.spec.levels[self.level.min(self.spec.levels.len() - 1)]
    }

    fn length(&self) -> f32 {
        self.lvl().secs * self.lvl().speed
    }

    fn limit(&self) -> f32 {
        self.spec.road - 0.35
    }

    fn events_of(level: &LevelSpec) -> Vec<Event> {
        let mut ev = Vec::new();
        for g in &level.gates {
            if let (Some(left), Some(right)) = (Op::parse(&g.left), Op::parse(&g.right)) {
                ev.push(Event {
                    at: g.at,
                    kind: Kind::Gate { left, right },
                });
            }
        }
        for w in &level.waves {
            ev.push(Event {
                at: w.at,
                kind: Kind::Wave {
                    count: w.count,
                    boss: w.boss,
                },
            });
        }
        ev.sort_by(|a, b| {
            a.at.partial_cmp(&b.at)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    let rank = |k: &Kind| match k {
                        Kind::Gate { .. } => 0,
                        Kind::Wave { .. } => 1,
                    };
                    rank(&a.kind).cmp(&rank(&b.kind))
                })
        });
        ev
    }

    fn start_level(&mut self) {
        let start = self.lvl().start;
        self.dist = 0.0;
        self.x = 0.0;
        self.count = start;
        self.count_before = start;
        self.events = Self::events_of(self.lvl());
        self.next_event = 0;
        self.beat = Beat::Run;
        self.card_t = 0.0;
        self.clash = None;
        self.swell = 0.0;
        self.flash = 0.0;
        self.continued = false;
        self.corpses.clear();
        self.floats.clear();
        self.t = 0.0;
        self.core.stats.alive = true;
        self.core.stats.death.clear();
        self.core.stats.score = self.bank;
        self.dragging = false;
        self.finger = None;
    }

    fn note_score(&mut self) {
        let s = self.bank + self.count.saturating_mul(10) + self.dist.max(0.0) as u32;
        self.core.stats.score = s;
        if s > self.flow.best_score {
            self.flow.best_score = s;
            self.new_best = true;
        }
    }

    fn enter_win(&mut self) {
        if self.beat != Beat::Run {
            return;
        }
        self.beat = Beat::Win;
        self.card_t = 0.0;
        self.note_score();
        self.progress
            .clear(self.level, self.core.stats.score as f32);
        self.ads.break_slot("level_end");
        crate::mixer::cue_sfx("win");
        self.core.juice.shake = self.core.juice.shake.max(0.2);
    }

    fn enter_lose(&mut self, before: u32) {
        if self.beat != Beat::Run {
            return;
        }
        self.count_before = before.max(1);
        self.beat = Beat::Lose;
        self.card_t = 0.0;
        self.core.stats.death = "horde".into();
        self.note_score();
        self.ads.break_slot("death");
        crate::mixer::cue_sfx("lose");
        self.core.juice.shake = self.core.juice.shake.max(0.7);
        self.flash = 1.0;
    }

    fn press(&mut self, btn: Btn) {
        match btn {
            Btn::Retry => {
                self.start_level();
                crate::mixer::cue_sfx("go");
                crate::mixer::play_bgm("bgm", 0.35);
            }
            Btn::Next => {
                self.bank = self
                    .core
                    .stats
                    .score
                    .saturating_add(self.count.saturating_mul(25) + 200);
                self.core.stats.score = self.bank;
                if self.level + 1 < self.spec.levels.len() {
                    self.level += 1;
                    self.start_level();
                    crate::mixer::cue_sfx("go");
                } else {
                    self.win = true;
                    let score = self.core.stats.score;
                    self.flow.finish(score, self.count, "CLEAR");
                    crate::mixer::stop_bgm(0.4);
                }
            }
            Btn::Cont => self.mercy(),
            Btn::None => {}
        }
    }

    /// One continue per level. The stub does not grant a reward; the run still
    /// hands back half the people (at least 4) so the preview is playable.
    /// A live SDK should grant this only when [`Ads::rewarded`] returns true.
    fn mercy(&mut self) {
        if self.beat != Beat::Lose || self.continued {
            return;
        }
        let granted = self.ads.rewarded("continue");
        if !granted {
            // Stub path: the request is recorded and the mercy is the game's.
        }
        self.continued = true;
        self.count = (self.count_before / 2).max(4);
        self.swell = 1.0;
        self.dist = (self.dist - 8.0).max(0.0);
        self.clash = None;
        self.beat = Beat::Run;
        self.card_t = 0.0;
        self.core.stats.death.clear();
        self.core.stats.alive = true;
        crate::mixer::cue_sfx("save");
    }

    fn fire(&mut self, ev: Event) {
        match ev.kind {
            Kind::Gate { left, right } => {
                let op = if self.x < 0.0 { left } else { right };
                let before = self.count;
                self.count = op.apply(before);
                self.swell = 1.0;
                let friendly = op.friendly();
                let color = if friendly {
                    [150, 255, 170, 255]
                } else {
                    [255, 120, 110, 255]
                };
                let z = -ev.at * self.length();
                self.floats.push([self.x, 2.3, z], op.label(), color, 0.0);
                self.core.juice.shake =
                    self.core.juice.shake.max(if friendly { 0.12 } else { 0.4 });
                self.flash = self.flash.max(if friendly { 0.22 } else { 0.85 });
                crate::mixer::cue_sfx(if friendly { "gate" } else { "bad" });
                if self.count == 0 {
                    self.enter_lose(before);
                }
            }
            Kind::Wave { count, boss } => {
                self.clash = Some(Clash {
                    foes: count,
                    boss,
                    age: 0.0,
                    resolved: false,
                    won: false,
                    z: -ev.at * self.length(),
                });
                self.core.juice.shake = self.core.juice.shake.max(if boss { 0.85 } else { 0.55 });
                self.flash = 1.0;
                crate::mixer::cue_sfx("hit");
            }
        }
    }

    fn step_run(&mut self, dt: f32) {
        if let Some(clash) = self.clash.as_mut() {
            clash.age += dt;
            if !clash.resolved && clash.age >= 0.45 {
                clash.resolved = true;
                let before = self.count;
                let foes = clash.foes;
                let z = clash.z;
                match gates::clash(before, foes) {
                    Some(left) => {
                        self.count = left;
                        self.swell = 1.0;
                        clash.won = true;
                        self.core.stats.score =
                            self.core.stats.score.saturating_add(foes.saturating_mul(8));
                        let n = foes.min(8) as usize;
                        for i in 0..n {
                            let side = (i as f32 - n as f32 * 0.5) * 0.7;
                            self.corpses.push(Corpse {
                                x: side,
                                z,
                                age: 0.0,
                                boss: clash.boss && i == 0,
                            });
                        }
                    }
                    None => {
                        clash.won = false;
                        self.count = 0;
                    }
                }
                let won = clash.won;
                if !won {
                    self.enter_lose(before);
                }
            }
            if self.clash.is_some_and(|c| c.age >= 1.05) && self.beat == Beat::Run {
                self.clash = None;
            }
            return;
        }
        if !self.dragging {
            self.x = (self.x + self.keys * self.spec.steer * dt).clamp(-self.limit(), self.limit());
        }
        self.dist += self.lvl().speed * dt;
        let len = self.length();
        while self.beat == Beat::Run
            && self.next_event < self.events.len()
            && self.dist >= self.events[self.next_event].at * len
        {
            let ev = self.events[self.next_event];
            self.next_event += 1;
            self.fire(ev);
        }
        if self.beat == Beat::Run && self.clash.is_none() && self.dist >= len {
            self.enter_win();
        }
    }

    fn step_sim(&mut self, dt: f32) {
        self.t += dt;
        self.card_t += dt;
        self.floats.tick(dt);
        self.swell = (self.swell - dt * 2.4).max(0.0);
        self.flash = (self.flash - dt * 3.2).max(0.0);
        for c in &mut self.corpses {
            c.age += dt;
        }
        self.corpses.retain(|c| c.age < 1.35);
        if self.beat == Beat::Run {
            self.step_run(dt);
            self.note_score();
        }
    }

    fn confirm(&mut self) {
        match self.flow.phase {
            FlowPhase::Title | FlowPhase::Result => {
                self.level = 0;
                self.bank = 0;
                self.win = false;
                self.new_best = false;
                self.best_before = self.flow.best_score;
                self.core = Core::default();
                self.ads = Ads::new();
                self.start_level();
                self.flow.enter_play();
                crate::mixer::cue_sfx("go");
                crate::mixer::play_bgm("bgm", 0.4);
            }
            FlowPhase::Pause => self.flow.resume(),
            FlowPhase::Play => {}
        }
    }

    fn budgets(&self) -> (usize, usize) {
        match self.tier {
            Tier::Low => (8, 14),
            Tier::Mobile => (16, 28),
            Tier::High => (24, 40),
        }
    }

    fn upcoming_wave(&self) -> Option<(f32, u32, bool)> {
        self.events[self.next_event..]
            .iter()
            .find_map(|e| match e.kind {
                Kind::Wave { count, boss } => Some((e.at * self.length(), count, boss)),
                _ => None,
            })
    }

    // ---- picture

    fn build_doc(&mut self) {
        let len_hint = self.length();
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: len_hint.max(40.0),
            ground: Some(false),
            sky: Some([128, 206, 255, 255]),
            hemi_sky: Some([0.72, 0.86, 1.0]),
            hemi_ground: Some([0.42, 0.5, 0.28]),
            bloom: Some(0.22),
            vignette: Some(0.22),
            fxaa: Some(true),
            tonemap: Some(true),
            exposure: Some(1.08),
            fog_start: Some(28.0),
            fog_end: Some(78.0),
            cull: Some(true),
            contact_blobs: Some(false),
            ..WorldDoc::default()
        };
        self.road(&mut doc);
        self.gates(&mut doc);
        self.crowd(&mut doc);
        self.zombies(&mut doc);
        declare_models(&mut doc, &all_models());
        let (eye, target) = self.camera();
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "chase".into(),
            position: eye,
            target,
            fov: 48.0,
            roll: 0.0,
        });
        doc.lights.push(light(
            0,
            "sun",
            [eye[0] + 6.0, 14.0, eye[2] + 4.0],
            [-0.35, -0.85, -0.4],
            [1.0, 0.96, 0.88],
            1.55,
            80.0,
            false,
        ));
        self.doc = doc;
    }

    fn camera(&self) -> ([f32; 3], [f32; 3]) {
        let z = -self.dist;
        let sh = self.core.juice.shake;
        let jx = (self.t * 47.0).sin() * sh * 0.22;
        let jy = (self.t * 61.0).cos() * sh * 0.12;
        let eye = [self.x * 0.25 + jx, 5.6 + jy, z + 12.2];
        let target = [self.x * 0.65, 1.25, z - 7.5];
        (eye, target)
    }

    fn road(&self, doc: &mut WorldDoc) {
        let seg = 16.0;
        let base = (self.dist / seg).floor() * seg;
        let half = self.spec.road + 0.8;
        for i in 0..8 {
            let zc = -(base + (i as f32 - 1.0) * seg + seg * 0.5);
            doc.props.push(prop(
                &format!("asphalt-{i}"),
                "box",
                "road",
                [0.0, -0.08, zc],
                [half * 2.0, 0.16, seg + 0.05],
                [92, 96, 104],
                0.0,
                0.86,
                0.0,
            ));
            doc.props.push(prop(
                &format!("grass-l-{i}"),
                "box",
                "grass",
                [-half - 6.0, -0.12, zc],
                [12.0, 0.1, seg + 0.05],
                [72, 186, 64],
                0.0,
                0.9,
                0.0,
            ));
            doc.props.push(prop(
                &format!("grass-r-{i}"),
                "box",
                "grass",
                [half + 6.0, -0.12, zc],
                [12.0, 0.1, seg + 0.05],
                [86, 198, 72],
                0.0,
                0.9,
                0.0,
            ));
            doc.props.push(prop(
                &format!("line-{i}"),
                "box",
                "line",
                [0.0, 0.02, zc],
                [0.16, 0.04, 3.2],
                [255, 214, 60],
                0.15,
                0.5,
                0.0,
            ));
            let side = if i % 2 == 0 { -1.0 } else { 1.0 };
            self.prop_or_box(
                doc,
                &format!("cone-{i}"),
                CONE,
                [side * (half + 0.55), 0.0, zc + 3.0],
                [0.42, 0.42, 0.42],
                [255, 140, 30],
            );
        }
    }

    fn gates(&self, doc: &mut WorldDoc) {
        let len = self.length();
        for (i, ev) in self.events.iter().enumerate() {
            let Kind::Gate { left, right } = ev.kind else {
                continue;
            };
            let ahead = ev.at * len - self.dist;
            if !(-3.0..32.0).contains(&ahead) {
                continue;
            }
            let z = -ev.at * len;
            for (side, op, name) in [(-1.0, left, "l"), (1.0, right, "r")] {
                let x = side * self.spec.road * 0.52;
                let friendly = op.friendly();
                let col = if friendly {
                    [46, 210, 96]
                } else {
                    [230, 54, 58]
                };
                doc.props.push(prop(
                    &format!("panel-{i}-{name}"),
                    "box",
                    "gate",
                    [x, 1.35, z],
                    [self.spec.road * 0.92, 2.15, 0.18],
                    col,
                    0.42,
                    0.35,
                    0.0,
                ));
                self.prop_or_box(
                    doc,
                    &format!("post-{i}-{name}"),
                    if side < 0.0 { BARRIER } else { BLOCK },
                    [x, 0.0, z - 0.7],
                    [0.38, 0.38, 0.38],
                    [210, 210, 220],
                );
            }
            self.prop_or_box(
                doc,
                &format!("mid-{i}"),
                CONE,
                [0.0, 0.0, z + 2.2],
                [0.48, 0.48, 0.48],
                [255, 150, 40],
            );
        }
    }

    fn crowd(&self, doc: &mut WorldDoc) {
        let (near, far) = self.budgets();
        let shown = (self.count as usize).clamp(1, near + far);
        let slots = crowd::formation(shown, 0.72, 5);
        let falling = self.beat == Beat::Lose;
        let lean = self
            .clash
            .map(|c| if c.resolved { 0.15 } else { 0.55 })
            .unwrap_or(0.0);
        for (i, (side, back)) in slots.iter().enumerate() {
            let (phase, tempo) = motion::phase_tempo(i as u32 + 3);
            let pose = motion::pose(
                Motion::Shamble,
                self.t,
                phase,
                tempo,
                if i < near { 0.28 } else { 0.8 },
                0.0,
            );
            let frame = crowd::flipbook(self.t, phase, 4);
            let path = if i < near { H_RUN[frame] } else { H_LOD };
            let mut pitch = -pose.pitch - lean;
            let mut y = pose.lift;
            if falling {
                let k = (self.card_t * 2.4).min(1.0);
                pitch = -1.15 * k;
                y = -0.45 * k;
            }
            let swell = 1.0 + self.swell * 0.22;
            self.figure(
                doc,
                &format!("human-{i}"),
                path,
                [self.x + *side, y, -self.dist - *back],
                0.0,
                pitch,
                pose.roll,
                swell * pose.wide,
                swell * pose.tall,
                [64, 96, 70],
            );
        }
    }

    fn zombies(&self, doc: &mut WorldDoc) {
        if let Some(clash) = self.clash {
            if !(clash.resolved && clash.won) {
                self.zombie_line(doc, clash.foes, clash.z, clash.boss, true);
            }
        } else if self.beat == Beat::Run {
            if let Some((at, count, boss)) = self.upcoming_wave() {
                let ahead = at - self.dist;
                if (0.0..34.0).contains(&ahead) {
                    self.zombie_line(doc, count, -at, boss, false);
                }
            }
        }
        for (i, c) in self.corpses.iter().enumerate() {
            let k = (c.age / 1.2).min(1.0);
            self.figure(
                doc,
                &format!("corpse-{i}"),
                Z_DIE,
                [c.x, -0.15 * k, c.z],
                std::f32::consts::PI,
                -0.15,
                0.0,
                if c.boss { 1.55 } else { 1.0 },
                1.0 - 0.2 * k,
                [120, 30, 36],
            );
        }
    }

    fn zombie_line(&self, doc: &mut WorldDoc, count: u32, z: f32, boss: bool, attack: bool) {
        let (near, far) = self.budgets();
        let shown = (count as usize).clamp(1, near + far);
        let slots = crowd::formation(shown, 0.78, 6);
        for (i, (side, back)) in slots.iter().enumerate() {
            let (phase, tempo) = motion::phase_tempo(100 + i as u32);
            let pose = motion::pose(Motion::Shamble, self.t, phase, tempo, 0.55, 0.0);
            let frame = crowd::flipbook(self.t + 0.17, phase, 4);
            let path = if attack && i < near {
                Z_ATK
            } else if i < near {
                Z_RUN[frame]
            } else {
                Z_LOD
            };
            let big = boss && i == 0;
            let s = if big { 1.62 } else { 1.0 };
            self.figure(
                doc,
                &format!("zombie-{i}"),
                path,
                [*side, pose.lift, z - *back],
                std::f32::consts::PI,
                -pose.pitch,
                pose.roll,
                s * pose.wide,
                s * pose.tall,
                [150, 42, 48],
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn figure(
        &self,
        doc: &mut WorldDoc,
        id: &str,
        path: &str,
        pos: [f32; 3],
        yaw: f32,
        pitch: f32,
        roll: f32,
        wide: f32,
        tall: f32,
        fallback: [u32; 3],
    ) {
        let scale = [wide, tall, wide];
        let mut p = if glb_ready(path) {
            let mut g = glb_prop(id, path, "figure", pos, scale, [255, 255, 255], 0.0, None);
            g.color = None;
            g
        } else {
            prop(
                id,
                "capsule",
                "figure",
                [pos[0], pos[1] + 0.9, pos[2]],
                [0.42 * wide, 1.7 * tall, 0.42 * wide],
                fallback,
                0.0,
                0.55,
                0.0,
            )
        };
        p.yaw = yaw;
        p.pitch = pitch;
        p.roll = roll;
        doc.props.push(p);
    }

    fn prop_or_box(
        &self,
        doc: &mut WorldDoc,
        id: &str,
        path: &str,
        pos: [f32; 3],
        scale: [f32; 3],
        col: [u32; 3],
    ) {
        let p = if glb_ready(path) {
            let mut g = glb_prop(id, path, "prop", pos, scale, [255, 255, 255], 0.0, None);
            g.color = None;
            g
        } else {
            prop(
                id,
                "box",
                "prop",
                [pos[0], pos[1] + 0.4, pos[2]],
                [0.6, 0.8, 0.6],
                col,
                0.05,
                0.6,
                0.0,
            )
        };
        doc.props.push(p);
    }

    // ---- HUD

    pub fn hud(&self, w: u32, h: u32, _menu: bool) -> DrawList {
        let (w, h) = (w as f32, h as f32);
        self.size.set((w.max(1.0), h.max(1.0)));
        let s = hud_scale(w, h);
        let mut list = DrawList::default();
        self.ui.set([(Btn::None, [0.0; 4]); 2]);
        if !matches!(self.flow.phase, FlowPhase::Play | FlowPhase::Pause) {
            return list;
        }
        let count_size = 54.0 * s * (1.0 + self.swell * 0.28);
        list.texts.push(
            TextQuad::new(
                &format!("{}", self.count),
                w * 0.5,
                8.0 * s,
                count_size,
                [255, 255, 255, 255],
            )
            .aligned(TextAlign::Center)
            .with_face(TextFace::Display),
        );
        list.texts.push(TextQuad::new(
            &format!(
                "{}  {}",
                self.level + 1,
                self.tr(&self.lvl().name, &self.lvl().name_ja)
            ),
            16.0 * s,
            18.0 * s,
            16.0 * s,
            [20, 40, 60, 230],
        ));
        let frac = (self.dist / self.length()).clamp(0.0, 1.0);
        hud_kit::gauge(
            &mut list,
            w * 0.5 - 90.0 * s,
            64.0 * s,
            180.0 * s,
            8.0 * s,
            frac,
            [255, 196, 48, 255],
            [20, 40, 70, 160],
        );
        if self.t < 2.6 && self.level == 0 && self.beat == Beat::Run {
            list.texts.push(
                TextQuad::new(
                    self.tr("DRAG LEFT OR RIGHT", "ひだり みぎ に ドラッグ"),
                    w * 0.5,
                    h * 0.22,
                    22.0 * s,
                    [20, 36, 64, 230],
                )
                .aligned(TextAlign::Center)
                .with_face(TextFace::Display),
            );
        }
        if self.beat == Beat::Run {
            self.gate_numbers(&mut list, w, h, s);
        }
        if let Some((_, foes, boss)) = self.wave_label() {
            let label = if boss {
                format!("{}  VS  {foes}", self.tr("BOSS", "ボス"))
            } else {
                format!("VS  {foes}")
            };
            list.texts.push(
                TextQuad::new(&label, w * 0.5, 78.0 * s, 28.0 * s, [255, 70, 70, 255])
                    .aligned(TextAlign::Center)
                    .with_face(TextFace::Display),
            );
        }
        if self.beat == Beat::Run {
            let view = self.view();
            self.floats.draw(&mut list, &view, w, h, s);
        }
        if self.flash > 0.02 {
            let a = (self.flash * 150.0) as u8;
            list.quads
                .push(Quad::new(0.0, 0.0, w, h, [255, 255, 255, a]));
        }
        if self.beat == Beat::Win {
            self.card_end(&mut list, w, h, s, true);
        } else if self.beat == Beat::Lose {
            self.card_end(&mut list, w, h, s, false);
        }
        if self.flow.phase == FlowPhase::Pause {
            list.texts.push(
                TextQuad::new(
                    self.tr("PAUSED", "いちじていし"),
                    w * 0.5,
                    h * 0.42,
                    28.0 * s,
                    [255, 255, 255, 255],
                )
                .aligned(TextAlign::Center),
            );
        }
        list
    }

    fn wave_label(&self) -> Option<(f32, u32, bool)> {
        if let Some(c) = self.clash {
            return Some((0.0, c.foes, c.boss));
        }
        self.upcoming_wave().filter(|(at, _, _)| {
            let ahead = at - self.dist;
            (0.0..22.0).contains(&ahead)
        })
    }

    fn view(&self) -> View {
        let (eye, target) = self.camera();
        View {
            eye,
            target,
            fov: 48.0,
        }
    }

    fn gate_numbers(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        let view = self.view();
        let len = self.length();
        for ev in &self.events {
            let Kind::Gate { left, right } = ev.kind else {
                continue;
            };
            let ahead = ev.at * len - self.dist;
            if !(-1.0..28.0).contains(&ahead) {
                continue;
            }
            let z = -ev.at * len;
            for (side, op) in [(-1.0, left), (1.0, right)] {
                let x = side * self.spec.road * 0.52;
                let Some((sx, sy)) = view.project([x, 2.15, z], w, h) else {
                    continue;
                };
                let col = if op.friendly() {
                    [230, 255, 236, 255]
                } else {
                    [255, 236, 230, 255]
                };
                list.texts.push(
                    TextQuad::new(&op.label(), sx, sy, 40.0 * s, col)
                        .aligned(TextAlign::Center)
                        .with_face(TextFace::Display),
                );
            }
        }
    }

    fn card_end(&self, list: &mut DrawList, w: f32, h: f32, s: f32, won: bool) {
        let card = Card::open(list, w, h, s, 250.0 * s, CardStyle::default());
        card.line(
            list,
            if won {
                self.tr("CLEARED", "クリア")
            } else {
                self.tr("WIPED OUT", "やられた")
            },
            22.0,
            32.0,
            if won {
                [40, 140, 70, 255]
            } else {
                [180, 40, 48, 255]
            },
        );
        let rows = [
            (self.tr("Alive", "いきてる"), format!("{}", self.count)),
            (
                self.tr("Score", "スコア"),
                format!("{}", self.core.stats.score),
            ),
            (self.tr("Level", "レベル"), format!("{}", self.level + 1)),
        ];
        card.rows(list, &rows, 78.0, 32.0);
        let bw = 150.0 * s;
        let bh = 46.0 * s;
        let y = card.y + card.h - bh - 18.0 * s;
        let gap = 12.0 * s;
        let total = bw * 2.0 + gap;
        let x0 = card.x + (card.w - total) * 0.5;
        let mut slots = [(Btn::None, [0.0; 4]); 2];
        if won {
            let q = hud_kit::pill_button(
                list,
                x0,
                y,
                bw,
                bh,
                self.tr("RETRY", "もういちど"),
                16.0 * s,
                [70, 90, 120, 255],
                [255, 255, 255, 255],
            );
            slots[0] = (Btn::Retry, [q.x, q.y, q.w, q.h]);
            let q = hud_kit::pill_button(
                list,
                x0 + bw + gap,
                y,
                bw,
                bh,
                if self.level + 1 < self.spec.levels.len() {
                    self.tr("NEXT", "つぎへ")
                } else {
                    self.tr("FINISH", "おわり")
                },
                16.0 * s,
                [40, 170, 90, 255],
                [255, 255, 255, 255],
            );
            slots[1] = (Btn::Next, [q.x, q.y, q.w, q.h]);
            let stage = Stage::fit(w, h, 400.0, 700.0);
            let mut paint = Painter::new(list, stage);
            paint.layer();
            Confetti::burst([200.0, 40.0], 11 + self.level as u32)
                .count(48)
                .draw(&mut paint, self.card_t);
        } else {
            let q = hud_kit::pill_button(
                list,
                x0,
                y,
                bw,
                bh,
                self.tr("RETRY", "もういちど"),
                16.0 * s,
                [70, 90, 120, 255],
                [255, 255, 255, 255],
            );
            slots[0] = (Btn::Retry, [q.x, q.y, q.w, q.h]);
            if !self.continued {
                let q = hud_kit::pill_button(
                    list,
                    x0 + bw + gap,
                    y,
                    bw,
                    bh,
                    self.tr("CONTINUE", "つづける"),
                    16.0 * s,
                    [230, 140, 40, 255],
                    [40, 24, 8, 255],
                );
                slots[1] = (Btn::Cont, [q.x, q.y, q.w, q.h]);
            }
        }
        self.ui.set(slots);
    }

    pub fn state_value(&self) -> Value {
        let br = self.ads.last("break");
        let rw = self.ads.last("rewarded");
        let foes = self
            .clash
            .map(|c| c.foes)
            .or_else(|| self.upcoming_wave().map(|w| w.1))
            .unwrap_or(0);
        json!({
            "phase": self.flow.phase.name(),
            "score": self.core.stats.score,
            "best": self.flow.best_score,
            "level": self.level + 1,
            "levels": self.spec.levels.len(),
            "count": self.count,
            "foes": foes,
            "dist": self.dist,
            "secs": self.t,
            "win": self.win || self.beat == Beat::Win,
            "death": self.core.stats.death,
            "death_text": if self.core.stats.death.is_empty() { "" } else { self.tr("The horde was bigger", "ホードのほうがおおかった") },
            "ad": {
                "break_id": br.map(|p| p.0).unwrap_or(0),
                "break_slot": br.map(|p| p.1).unwrap_or(""),
                "reward_id": rw.map(|p| p.0).unwrap_or(0),
                "reward_slot": rw.map(|p| p.1).unwrap_or(""),
            },
            "result": if self.flow.phase == FlowPhase::Result {
                json!({
                    "key": self.spec.id,
                    "title": self.tr(&self.spec.title, &self.spec.title_ja),
                    "enabled": true,
                    "top_n": 5,
                    "columns": ["score"],
                    "sort": "score",
                    "run": {"score": self.core.stats.score, "win": u32::from(self.win)},
                    "can_revive": false,
                    "new_best": self.new_best,
                })
            } else { Value::Null },
        })
    }

    fn touch_ev(&mut self, ev: PointerEvent, w: u32, h: u32) {
        if self.beat != Beat::Run || self.flow.phase != FlowPhase::Play {
            return;
        }
        let w = w.max(1) as f32;
        self.size.set((w, h.max(1) as f32));
        match ev.phase {
            PointerPhase::Begin => {
                self.finger = Some(ev.id);
                self.last_px = ev.x;
                self.dragging = true;
            }
            PointerPhase::Move if self.finger == Some(ev.id) => {
                let dx = ev.x - self.last_px;
                self.last_px = ev.x;
                let road = self.spec.road * 2.2;
                self.x = (self.x + dx / w * road).clamp(-self.limit(), self.limit());
            }
            PointerPhase::End | PointerPhase::Cancel if self.finger == Some(ev.id) => {
                self.finger = None;
                self.dragging = false;
            }
            _ => {}
        }
    }
}

fn hud_scale(w: f32, h: f32) -> f32 {
    (w.min(h) / 400.0).clamp(0.85, 2.4)
}

impl Kit for ZombieRush {
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
        self.t += dt;
        self.x = (self.t * 0.7).sin() * 1.1;
    }
    fn rebuild(&mut self) {
        self.build_doc();
    }
}

impl crate::kit_host::KitHost for ZombieRush {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn name(&self) -> &'static str {
        "horde gates"
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
        ZombieRush::confirm(self);
    }
    fn show_title(&mut self) {
        self.flow.show_title();
        self.core.stats.alive = true;
    }
    fn pointer(&mut self, x: f32, y: f32, _width: u32, _height: u32) -> bool {
        if !matches!(self.beat, Beat::Win | Beat::Lose) {
            return false;
        }
        for (btn, r) in self.ui.get() {
            if btn != Btn::None && x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3] {
                self.press(btn);
                return true;
            }
        }
        true
    }
    fn on_key(&mut self, _name: &str, _down: bool) -> bool {
        false
    }
    fn tick(&mut self, dt: f32) {
        tick_frame(self, dt);
    }
    fn hud(&self, width: u32, height: u32, menu: bool) -> DrawList {
        ZombieRush::hud(self, width, height, menu)
    }
    fn walk(&mut self, lx: f32, _lz: f32, _jump: bool) {
        self.keys = lx.clamp(-1.0, 1.0);
    }
    fn touch(&mut self, ev: PointerEvent, w: u32, h: u32, _t: f32) {
        self.touch_ev(ev, w, h);
    }
    fn uses_swipe(&self) -> bool {
        true
    }
    fn set_lang(&mut self, lang: &str) {
        self.lang_ja = lang.eq_ignore_ascii_case("ja");
    }
    fn pause_run(&mut self) {
        if self.flow.phase == FlowPhase::Play {
            self.flow.pause();
            self.dragging = false;
            self.finger = None;
        }
    }
    fn resume_run(&mut self) {
        self.flow.resume();
    }
    fn save_json(&self) -> Option<String> {
        Some(self.progress.to_value().to_string())
    }
    fn supports_quality_tiers(&self) -> bool {
        true
    }
    fn quality_hints(&mut self, _atmo: f32, particles: f32) {
        self.tier = if particles < 0.35 {
            Tier::Low
        } else if particles < 0.8 {
            Tier::Mobile
        } else {
            Tier::High
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit_host::KitHost;

    const DT: f32 = 1.0 / 60.0;

    fn script(g: &mut ZombieRush, better: bool) {
        for _ in 0..8000 {
            if g.beat != Beat::Run {
                break;
            }
            if g.clash.is_none() {
                if let Some(ev) = g.events.get(g.next_event).copied() {
                    if let Kind::Gate { left, right } = ev.kind {
                        let l = left.apply(g.count);
                        let r = right.apply(g.count);
                        let go_left = if better { l >= r } else { l < r };
                        g.x = if go_left { -2.2 } else { 2.2 };
                    }
                }
            }
            g.step_sim(DT);
        }
    }

    #[test]
    fn five_levels_between_thirty_and_sixty_seconds() {
        let spec = ZombieRushSpec::default();
        spec.validate().unwrap();
        assert_eq!(spec.levels.len(), 5);
        assert!(spec.levels.iter().all(|l| (30.0..=60.0).contains(&l.secs)));
        let mut v = ZombieRush::new(spec).world_value()["game"].clone();
        v["nope"] = json!(1);
        assert!(ZombieRushSpec::from_value(&v).is_err());
    }

    #[test]
    fn the_better_gate_clears_every_level_and_the_worse_one_wipes_you() {
        let mut g = ZombieRush::new(ZombieRushSpec::default());
        g.confirm();
        for n in 0..5 {
            script(&mut g, true);
            assert_eq!(g.beat, Beat::Win, "level {} count {}", n + 1, g.count);
            assert!(g.count > 0);
            assert!(g
                .ads
                .events
                .iter()
                .any(|e| e.kind == "break" && e.slot == "level_end"));
            g.press(Btn::Next);
        }
        assert_eq!(g.phase_name(), "result");
        assert!(g.state_value()["result"].is_object());

        for n in 0..5 {
            let mut g = ZombieRush::new(ZombieRushSpec::default());
            g.level = n;
            g.start_level();
            script(&mut g, false);
            assert_eq!(g.beat, Beat::Lose, "level {}", n + 1);
            assert!(g.ads.events.iter().any(|e| e.slot == "death"));
        }
    }

    #[test]
    fn continue_fires_the_reward_hook_and_hands_people_back() {
        let mut g = ZombieRush::new(ZombieRushSpec::default());
        g.confirm();
        script(&mut g, false);
        assert_eq!(g.beat, Beat::Lose);
        let before = g.ads.events.len();
        g.mercy();
        assert_eq!(g.beat, Beat::Run);
        assert!(g.count >= 4);
        assert!(g.ads.events[before..]
            .iter()
            .any(|e| e.kind == "rewarded" && e.slot == "continue"));
        g.mercy();
        assert_eq!(g.beat, Beat::Run, "only one continue");
    }

    #[test]
    fn drag_and_keys_steer_the_crowd() {
        let mut g = ZombieRush::new(ZombieRushSpec::default());
        g.confirm();
        g.touch(ev(1, PointerPhase::Begin, 40.0, 500.0), 390, 844, 0.0);
        g.touch(ev(1, PointerPhase::Move, 300.0, 500.0), 390, 844, 0.0);
        assert!(g.x > 1.0, "drag right is +x, got {}", g.x);
        g.touch(ev(1, PointerPhase::End, 300.0, 500.0), 390, 844, 0.0);
        g.x = 0.0;
        g.walk(-1.0, 1.0, false);
        for _ in 0..20 {
            g.step_sim(DT);
        }
        assert!(g.x < -1.0, "stick left is -x, got {}", g.x);
    }

    #[test]
    fn the_mesh_set_stays_put_while_the_crowd_moves() {
        let mut g = ZombieRush::new(ZombieRushSpec::default());
        g.confirm();
        g.build_doc();
        let key = g.doc.gpu_mesh_key();
        g.t += 0.5;
        g.x = 2.0;
        g.dist = 12.0;
        g.build_doc();
        assert_eq!(g.doc.gpu_mesh_key(), key);
        assert!(g.doc.props.len() > 20);
    }

    #[test]
    fn japanese_and_english_cards_both_have_words() {
        let mut g = ZombieRush::new(ZombieRushSpec::default());
        g.confirm();
        script(&mut g, true);
        g.set_lang("ja");
        let hud = g.hud(390, 844, false);
        let text = hud
            .texts
            .iter()
            .map(|t| t.text.clone())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("クリア"), "{text}");
        assert!(text.contains("つぎへ"), "{text}");
        g.set_lang("en");
        let hud = g.hud(390, 844, false);
        let text = hud
            .texts
            .iter()
            .map(|t| t.text.clone())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("CLEARED"), "{text}");
        let _ = g.hud(390, 844, false);
        let hit = g.ui.get()[1];
        assert_eq!(hit.0, Btn::Next);
        assert!(g.pointer(hit.1[0] + 4.0, hit.1[1] + 4.0, 390, 844));
        assert_eq!(g.level, 1);
    }

    #[test]
    fn pause_holds_the_road() {
        let mut g = ZombieRush::new(ZombieRushSpec::default());
        g.confirm();
        g.tick(DT);
        g.pause_run();
        let d = g.dist;
        g.tick(DT);
        assert_eq!(g.dist, d);
        g.resume_run();
        g.tick(DT);
        assert!(g.dist > d);
    }

    #[test]
    fn the_baked_fixture_is_the_default_world() {
        let want = ZombieRush::new(ZombieRushSpec::default()).world_value();
        // f32 levels don't round-trip through JSON text as the same Value bits.
        // Compare the file to a fresh pretty-print of the spec, which is what we ship.
        let text = serde_json::to_string_pretty(&want).unwrap() + "\n";
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/zombie_rush_world.json"
        );
        if std::env::var("KAGRA_BAKE").is_ok() {
            std::fs::write(path, &text).unwrap();
        }
        let got = std::fs::read_to_string(path).unwrap_or_default();
        assert_eq!(
            got, text,
            "run `KAGRA_BAKE=1 cargo test -p kagra-shared --lib zombie_rush::` to refresh it"
        );
        let got: Value = serde_json::from_str(&got).unwrap();
        assert!(is_zombie_rush_dump(&got.to_string()));
        let loaded = crate::kit_host::load(&got.to_string()).unwrap().unwrap();
        assert_eq!(loaded.name(), "horde gates");
    }

    fn ev(id: u32, phase: PointerPhase, x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            id,
            x,
            y,
            phase,
            pressure: 1.0,
        }
    }

    /// `KAGRA_SHOT=dir` writes HUD pngs and a world dump for the offscreen renderer.
    #[test]
    fn shots() {
        let Ok(dir) = std::env::var("KAGRA_SHOT") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        let save = |name: &str, g: &ZombieRush| {
            let list = g.hud(390, 844, false);
            let pic = crate::raster2d::picture(&list, 390, 844);
            pic.save_png(&dir.join(name)).unwrap();
        };
        let mut g = ZombieRush::new(ZombieRushSpec::default());
        g.confirm();
        g.set_lang("en");
        for _ in 0..90 {
            g.step_sim(DT);
            g.build_doc();
        }
        save("hud_run.png", &g);
        std::fs::write(dir.join("run_world.json"), g.doc.to_json().unwrap()).unwrap();
        // Steer into the first gate.
        g.x = -2.2;
        for _ in 0..400 {
            g.step_sim(DT);
            g.build_doc();
            if g.dist > 50.0 {
                break;
            }
        }
        save("hud_gate.png", &g);
        std::fs::write(dir.join("gate_world.json"), g.doc.to_json().unwrap()).unwrap();
        script(&mut g, true);
        g.build_doc();
        save("hud_win.png", &g);
        let mut lost = ZombieRush::new(ZombieRushSpec::default());
        lost.confirm();
        script(&mut lost, false);
        lost.build_doc();
        save("hud_lose.png", &lost);
    }
}
