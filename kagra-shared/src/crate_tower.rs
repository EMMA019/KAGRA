//! Crate Tower: one button, stack crates as high as you can.
//!
//! A crate swings from side to side above the tower; tap (or Space) to drop it. Land it on
//! the top crate and the tower grows; land it dead centre for a PERFECT (it snaps square and
//! the combo bonus grows); miss the top and it tumbles off and costs a life. Every crate that
//! lands off centre shifts the tower's balance: lean too far and it topples. The swing speeds
//! up as the tower grows. Every `chest_every`-th crate is a treasure chest worth a bonus.
//!
//! Built from the Starter template (`scripts/new_kit.py`) on the shared parts:
//!
//! - [`crate::game_core`]: the fixed-step driver ([`tick_frame`]), run stats, hitstop, the
//!   touch camera [`View`].
//! - [`crate::hud_kit`]: the rounded [`round_panel`].
//! - [`crate::kit_host::KitHost`]: what the session and the page call.

use std::cell::Cell;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::flow::{FlowPhase, GameFlow};
use crate::game_core::view::View;
use crate::game_core::{declare_models, tick_frame, Core, Kit};
use crate::hop_beat::{glb_prop, glb_ready, light, prop};
use crate::hud_kit::round_panel;
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, Quad, TextAlign, TextFace, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc};

/// The `genre` of this kit's world file (and the `game.id` default).
pub const GAME_ID: &str = "crate_tower";

/// True for a world file whose `genre` is this kit's.
pub fn is_crate_tower_dump(json: &str) -> bool {
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
pub struct CrateTowerSpec {
    pub id: String,
    pub title: String,
    /// Crate edge (m).
    pub size: f32,
    /// How far (m) the hanging crate swings either side of the tower's middle.
    pub swing: f32,
    /// Swing speed (radians of the swing per second) at the start, added per crate, and at most.
    pub speed: f32,
    pub speed_per: f32,
    pub speed_max: f32,
    /// How high (m) above the top the crate hangs, and how fast (m/s) it drops.
    pub hover: f32,
    pub drop_speed: f32,
    /// A crate whose middle is more than this share of `size` off the top crate falls off.
    pub land_tol: f32,
    /// Within this (m) of the top crate's middle is a PERFECT: it snaps square.
    pub perfect_tol: f32,
    /// The tower topples when its middle (the mean of the crates) is this far (m) off the base.
    pub lean_limit: f32,
    pub lives: u32,
    /// Points per crate, and per PERFECT in a row (on top).
    pub crate_score: u32,
    pub perfect_score: u32,
    /// Every n-th crate is a chest worth `chest_score` more (0 = never).
    pub chest_every: u32,
    pub chest_score: u32,
    /// Music and sound cues for the page (file names under the bundle). Passed through.
    pub audio: Option<Value>,
}

impl Default for CrateTowerSpec {
    fn default() -> Self {
        Self {
            id: GAME_ID.into(),
            title: "Crate Tower".into(),
            size: 1.0,
            swing: 2.0,
            speed: 1.7,
            speed_per: 0.07,
            speed_max: 4.6,
            hover: 2.4,
            drop_speed: 14.0,
            land_tol: 0.7,
            perfect_tol: 0.1,
            lean_limit: 0.42,
            lives: 3,
            crate_score: 10,
            perfect_score: 5,
            chest_every: 10,
            chest_score: 50,
            audio: Some(default_audio()),
        }
    }
}

impl CrateTowerSpec {
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let spec: Self =
            serde_json::from_value(v.clone()).map_err(|e| format!("crate_tower spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), String> {
        let range = |name: &str, v: f32, lo: f32, hi: f32| {
            if v.is_finite() && (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(format!("crate_tower: {name} must be {lo}..={hi}, got {v}"))
            }
        };
        range("size", self.size, 0.2, 5.0)?;
        range("swing", self.swing, 0.2, 20.0)?;
        range("speed", self.speed, 0.1, 20.0)?;
        range("speed_per", self.speed_per, 0.0, 2.0)?;
        range("speed_max", self.speed_max, self.speed, 30.0)?;
        range("hover", self.hover, 0.5, 20.0)?;
        range("drop_speed", self.drop_speed, 1.0, 100.0)?;
        range("land_tol", self.land_tol, 0.1, 1.0)?;
        range("perfect_tol", self.perfect_tol, 0.0, 1.0)?;
        range("lean_limit", self.lean_limit, 0.05, 10.0)?;
        if !(1..=9).contains(&self.lives) {
            return Err("crate_tower: lives must be 1..=9".into());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- look ----

const CRATE_GLB: &str = "assets/models/kenney/platformer-kit/crate.glb";
const BARREL_GLB: &str = "assets/models/kenney/platformer-kit/barrel.glb";
const CHEST_GLB: &str = "assets/models/kenney/mini-dungeon/chest.glb";
/// The crate models are 0.5 m cubes.
const MODEL_M: f32 = 0.5;
/// The dock behind the tower.
const SPACE: &str = "assets/models/kenney/space-kit/";
const BACKDROP: [(&str, f32, f32, f32); 5] = [
    ("hangar_smallA.glb", -12.0, -26.0, 1.6),
    ("machine_generatorLarge.glb", 10.0, -22.0, 2.0),
    ("satelliteDish.glb", -2.0, -32.0, 2.5),
    ("structure_closed.glb", 6.0, -34.0, 2.5),
    ("turret_single.glb", 16.0, -28.0, 2.0),
];

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
            "land": {"file": "assets/audio/kenney-impact-sounds/impactGeneric_light_001.wav", "volume": 0.6, "rate": 0.8},
            "perfect": cue("retro_coin_01.wav", 0.5, 1.0),
            "chest": cue("power_up_04.wav", 0.6, 1.2),
            "miss": cue("retro_die_01.wav", 0.6, 1.0),
            "gameover": cue("retro_die_03.wav", 0.8, 1.0),
            "go": {"file": "assets/audio/kenney-interface-sounds/confirmation_001.wav", "volume": 0.7, "rate": 1.0}
        }
    })
}

// ----------------------------------------------------------------- run ----

/// A crate falling off the tower: x, y, sideways speed, fall speed, spin, seconds left.
type Tumble = [f32; 6];

#[derive(Debug)]
pub struct CrateTower {
    spec: CrateTowerSpec,
    core: Core,
    flow: GameFlow,
    doc: WorldDoc,
    /// Seconds into the run (and the title's clock).
    t: f32,
    /// The middle (x, m) of each crate on the tower, from the bottom.
    tower: Vec<f32>,
    /// The swing's phase; the hanging crate is at `swing * sin(phase)`.
    phase: f32,
    /// The dropped crate: its x and height, while it falls.
    dropping: Option<(f32, f32)>,
    tumbles: Vec<Tumble>,
    lives: u32,
    perfects: u32,
    combo: u32,
    best_combo: u32,
    /// The last word to flash (PERFECT, a chest, a miss) and seconds left.
    shout: (&'static str, f32),
    /// The camera's height, easing toward the top.
    cam_y: f32,
    win: bool,
    new_best: bool,
    best_before: u32,
    lang_ja: bool,
    /// The canvas in pixels, from the last HUD call or touch (the camera fits it).
    size: Cell<(f32, f32)>,
}

impl CrateTower {
    pub fn new(spec: CrateTowerSpec) -> Self {
        let mut flow = GameFlow::new(spec.id.clone());
        flow.title = spec.title.clone();
        flow.tagline = String::new();
        let mut g = Self {
            lives: spec.lives,
            spec,
            core: Core::default(),
            flow,
            doc: WorldDoc::default(),
            t: 0.0,
            tower: Vec::new(),
            phase: 0.0,
            dropping: None,
            tumbles: Vec::new(),
            perfects: 0,
            combo: 0,
            best_combo: 0,
            shout: ("", 0.0),
            cam_y: 0.0,
            win: false,
            new_best: false,
            best_before: 0,
            lang_ja: false,
            size: Cell::new((390.0, 844.0)),
        };
        g.reset_field();
        g.build_doc();
        g
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| format!("crate_tower dump: {e}"))?;
        let spec = match value.get("game") {
            Some(g) => CrateTowerSpec::from_value(g)?,
            None => CrateTowerSpec::default(),
        };
        let mut g = Self::new(spec);
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            g.flow.best_score = best as u32;
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
        if self.lang_ja {
            ja
        } else {
            en
        }
    }

    // ---- what the rules read

    /// Crates on the tower above the base.
    fn height(&self) -> u32 {
        self.tower.len().saturating_sub(1) as u32
    }

    /// The top of the tower (m).
    fn top_y(&self) -> f32 {
        self.tower.len() as f32 * self.spec.size
    }

    fn top_x(&self) -> f32 {
        self.tower.last().copied().unwrap_or(0.0)
    }

    /// The tower's middle: the mean of its crates (m, 0 = square on the base).
    fn lean(&self) -> f32 {
        if self.tower.is_empty() {
            0.0
        } else {
            self.tower.iter().sum::<f32>() / self.tower.len() as f32
        }
    }

    fn speed(&self) -> f32 {
        (self.spec.speed + self.spec.speed_per * self.height() as f32).min(self.spec.speed_max)
    }

    /// Where the hanging crate is now.
    fn hang_x(&self) -> f32 {
        self.spec.swing * self.phase.sin()
    }

    fn is_chest(&self, i: usize) -> bool {
        self.spec.chest_every > 0 && i > 0 && i.is_multiple_of(self.spec.chest_every as usize)
    }

    // ---- flow

    fn reset_field(&mut self) {
        self.t = 0.0;
        // The base crate sits square on the plinth.
        self.tower = vec![0.0];
        self.phase = -std::f32::consts::FRAC_PI_2;
        self.dropping = None;
        self.tumbles.clear();
        self.lives = self.spec.lives;
        self.perfects = 0;
        self.combo = 0;
        self.best_combo = 0;
        self.shout = ("", 0.0);
        self.cam_y = self.top_y();
        self.win = false;
        self.new_best = false;
    }

    pub fn confirm(&mut self) {
        match self.flow.phase {
            FlowPhase::Title | FlowPhase::Result => {
                self.core = Core::default();
                self.reset_field();
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

    fn lose(&mut self, why: &str) {
        self.core.stats.alive = false;
        self.core.stats.just_crash = true;
        self.core.stats.death = why.into();
        self.core.juice.hitstop = 0.3;
        self.new_best = self.core.stats.score > self.best_before;
        crate::mixer::stop_bgm(0.6);
        crate::mixer::cue_sfx("gameover");
    }

    // ---- sim

    /// Drop the hanging crate (the one button).
    pub fn drop_crate(&mut self) -> bool {
        if self.flow.phase != FlowPhase::Play || !self.core.stats.alive || self.dropping.is_some() {
            return false;
        }
        self.dropping = Some((self.hang_x(), self.top_y() + self.spec.hover));
        true
    }

    fn land(&mut self, x: f32) {
        let size = self.spec.size;
        let dx = x - self.top_x();
        // The next crate starts at the far side from this one.
        self.phase = if x >= 0.0 {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        };
        if dx.abs() > self.spec.land_tol * size {
            // Off the edge: it tumbles away.
            self.tumbles.push([
                x,
                self.top_y(),
                dx.signum() * 1.5,
                0.0,
                dx.signum() * 4.0,
                1.5,
            ]);
            self.combo = 0;
            self.lives = self.lives.saturating_sub(1);
            self.shout = ("MISS", 0.8);
            self.core.juice.hit(0.2, 0.05);
            crate::mixer::cue_sfx("miss");
            if self.lives == 0 {
                self.lose("dropped");
            }
            return;
        }
        let perfect = dx.abs() <= self.spec.perfect_tol;
        let x = if perfect { self.top_x() } else { x };
        self.tower.push(x);
        self.core.stats.score += self.spec.crate_score;
        self.core.stats.rings = self.height();
        self.core.stats.just_rings += 1;
        if perfect {
            self.combo += 1;
            self.perfects += 1;
            self.best_combo = self.best_combo.max(self.combo);
            self.core.stats.score += self.spec.perfect_score * self.combo;
            self.shout = ("PERFECT", 0.8);
            crate::mixer::cue_sfx("perfect");
        } else {
            self.combo = 0;
            crate::mixer::cue_sfx("land");
        }
        if self.is_chest(self.tower.len() - 1) {
            self.core.stats.score += self.spec.chest_score;
            self.shout = ("CHEST", 1.0);
            crate::mixer::cue_sfx("chest");
        }
        if self.lean().abs() > self.spec.lean_limit {
            // The tower goes over: every crate above the base tumbles that way.
            let side = self.lean().signum();
            for (i, &cx) in self.tower.iter().enumerate().skip(1) {
                let k = i as f32 / self.tower.len() as f32;
                self.tumbles.push([
                    cx,
                    i as f32 * size,
                    side * (1.0 + 3.0 * k),
                    0.0,
                    side * 3.0,
                    2.0,
                ]);
            }
            self.tower.truncate(1);
            self.shout = ("TOPPLED", 1.5);
            self.core.juice.hit(0.4, 0.1);
            self.lose("toppled");
        }
    }

    /// One fixed step of the rules.
    fn step_sim(&mut self, dt: f32) {
        self.t += dt;
        self.shout.1 = (self.shout.1 - dt).max(0.0);
        for b in &mut self.tumbles {
            b[3] -= 18.0 * dt;
            b[0] += b[2] * dt;
            b[1] += b[3] * dt;
            b[5] -= dt;
        }
        self.tumbles.retain(|b| b[5] > 0.0);
        let speed = self.speed();
        if let Some((x, y)) = self.dropping {
            let y = y - self.spec.drop_speed * dt;
            if y <= self.top_y() {
                self.dropping = None;
                self.land(x);
            } else {
                self.dropping = Some((x, y));
            }
        } else {
            self.phase += speed * dt;
        }
        self.cam_y += (self.top_y() - self.cam_y) * (dt * 4.0).min(1.0);
    }

    // ---- input

    fn view(&self) -> View {
        let (w, h) = self.size.get();
        let s = self.spec.size;
        let y = self.cam_y;
        let reach = self.spec.swing + s;
        // Keep the swing, the hanging crate and a few crates of the tower in sight.
        let keep = [
            [-reach, y + self.spec.hover + s * 1.3, 0.0],
            [reach, y + self.spec.hover + s * 1.3, 0.0],
            [-reach, y - 4.0 * s, 0.0],
            [reach, y - 4.0 * s, 0.0],
        ];
        View::frame(
            [0.0, y + 0.2 * self.spec.hover, 0.0],
            [0.0, 0.35, 1.0],
            42.0,
            &keep,
            w / h.max(1.0),
        )
    }

    pub fn touch_ev(&mut self, ev: PointerEvent, w: u32, h: u32) {
        self.size.set((w.max(1) as f32, h.max(1) as f32));
        if ev.phase == PointerPhase::Begin {
            self.drop_crate();
        }
    }

    pub fn key(&mut self, name: &str, down: bool) -> bool {
        let k = name.to_ascii_lowercase();
        if down && matches!(k.as_str(), "space" | " " | "arrowdown" | "s" | "enter") {
            return self.drop_crate();
        }
        false
    }

    // ---- picture

    /// The 3D world. Built once per frame from the state.
    fn build_doc(&mut self) {
        let size = self.spec.size;
        let k = size / MODEL_M;
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: 30.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([7, 9, 22, 255]),
            hemi_sky: Some([0.2, 0.24, 0.45]),
            hemi_ground: Some([0.12, 0.1, 0.16]),
            env_map: Some("preset:studio".into()),
            ibl: Some(0.25),
            exposure: Some(0.9),
            fog_start: Some(30.0),
            fog_end: Some(80.0),
            bloom: Some(0.5),
            bloom_threshold: Some(0.9),
            vignette: Some(0.5),
            fxaa: Some(true),
            tonemap: Some(true),
            ..WorldDoc::default()
        };
        doc.props.push(prop(
            "floor",
            "box",
            "floor",
            [0.0, -0.6, -15.0],
            [120.0, 0.2, 70.0],
            [40, 46, 58],
            0.0,
            0.95,
            0.0,
        ));
        // A dark wall far behind, so the picture is night all the way up.
        doc.props.push(prop(
            "night",
            "box",
            "night",
            [0.0, 30.0, -45.0],
            [220.0, 120.0, 1.0],
            [8, 10, 24],
            0.0,
            1.0,
            0.0,
        ));
        doc.props.push(prop(
            "plinth",
            "box",
            "plinth",
            [0.0, -0.25, 0.0],
            [size * 1.6, 0.5, size * 1.6],
            [74, 80, 96],
            0.0,
            0.8,
            0.1,
        ));
        let model = |id: &str, path: &str, name: &str, pos: [f32; 3], s: f32, col: [u32; 3]| {
            if glb_ready(path) {
                glb_prop(id, path, name, pos, [s; 3], [255, 255, 255], 0.0, None)
            } else {
                prop(
                    id,
                    "box",
                    name,
                    [pos[0], pos[1] + s * MODEL_M * 0.5, pos[2]],
                    [s * MODEL_M; 3],
                    col,
                    0.0,
                    0.8,
                    0.0,
                )
            }
        };
        for (i, (file, x, z, s)) in BACKDROP.iter().enumerate() {
            let mut p = model(
                &format!("dock-{i}"),
                &format!("{SPACE}{file}"),
                "dock",
                [*x, -0.5, *z],
                *s,
                [80, 90, 120],
            );
            p.yaw = -x.atan2(-z);
            p.color = Some([110, 118, 150]);
            doc.props.push(p);
        }
        // The tower leans a little with its balance, more toward the top.
        let lean = self.lean() / self.spec.lean_limit;
        let n = self.tower.len().max(1) as f32;
        for (i, &x) in self.tower.iter().enumerate() {
            let path = if self.is_chest(i) {
                CHEST_GLB
            } else if i % 4 == 3 {
                BARREL_GLB
            } else {
                CRATE_GLB
            };
            let mut p = model(
                &format!("crate-{i}"),
                path,
                "crate",
                [x, i as f32 * size, 0.0],
                k,
                [190, 140, 90],
            );
            p.yaw = ((i * 37 % 11) as f32 - 5.0) * 0.03;
            p.roll = -lean * 0.05 * (i as f32 / n);
            if self.is_chest(i) {
                p.emissive = Some(0.35);
                p.emissive_color = Some([1.0, 0.75, 0.3]);
            }
            doc.props.push(p);
        }
        // The hanging (or falling) crate, its rope, and where it would land.
        if matches!(self.flow.phase, FlowPhase::Play | FlowPhase::Pause) && self.core.stats.alive {
            let (x, y) = self
                .dropping
                .unwrap_or((self.hang_x(), self.top_y() + self.spec.hover));
            let next = self.tower.len();
            let path = if self.is_chest(next) {
                CHEST_GLB
            } else {
                CRATE_GLB
            };
            doc.props
                .push(model("hang", path, "hang", [x, y, 0.0], k, [220, 170, 110]));
            if self.dropping.is_none() {
                doc.props.push(prop(
                    "rope",
                    "box",
                    "rope",
                    [x, y + size + 3.0, 0.0],
                    [0.04, 6.0, 0.04],
                    [150, 140, 120],
                    0.0,
                    0.9,
                    0.0,
                ));
            }
            let dx = (x - self.top_x()).abs();
            let col = if dx <= self.spec.perfect_tol {
                [140, 255, 170]
            } else if dx <= self.spec.land_tol * size {
                [255, 220, 140]
            } else {
                [255, 90, 90]
            };
            doc.props.push(prop(
                "aim",
                "sphere",
                "aim",
                [x, self.top_y() + 0.01, 0.0],
                [size * 0.35, 0.02, size * 0.35],
                col,
                1.2,
                0.5,
                0.0,
            ));
        }
        for (i, b) in self.tumbles.iter().enumerate() {
            let mut p = model(
                &format!("tumble-{i}"),
                CRATE_GLB,
                "tumble",
                [b[0], b[1], 0.3],
                k,
                [190, 140, 90],
            );
            p.roll = -b[4] * (2.0 - b[5]);
            doc.props.push(p);
        }
        // The crate, barrel and chest stay on the GPU from the first frame (no hitch at the
        // first barrel or chest).
        declare_models(&mut doc, &[CRATE_GLB, BARREL_GLB, CHEST_GLB]);
        let view = self.view();
        let sh = self.core.juice.shake;
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "tower".into(),
            position: [
                view.eye[0] + sh * (self.t * 53.0).sin(),
                view.eye[1] + sh * 0.5 * (self.t * 41.0).cos(),
                view.eye[2],
            ],
            target: view.target,
            fov: view.fov,
            roll: 0.0,
        });
        doc.lights.push(light(
            0,
            "moon",
            [-4.0, self.cam_y + 12.0, 8.0],
            [0.0; 3],
            [0.72, 0.8, 1.0],
            1.1,
            60.0,
            false,
        ));
        doc.lights.push(light(
            1,
            "lamp",
            [1.5, self.top_y() + 1.5, 2.0],
            [0.0; 3],
            [1.0, 0.75, 0.45],
            2.2,
            9.0,
            false,
        ));
        self.doc = doc;
    }

    /// The in-play HUD (the page draws the title and result cards).
    pub fn hud(&self, w: u32, h: u32, _menu: bool) -> DrawList {
        let (w, h) = (w as f32, h as f32);
        self.size.set((w.max(1.0), h.max(1.0)));
        let s = hud_scale(w, h);
        let mut list = DrawList::default();
        if !matches!(self.flow.phase, FlowPhase::Play | FlowPhase::Pause) {
            return list;
        }
        list.texts.push(
            TextQuad::new(
                &format!("{}", self.height()),
                w * 0.5,
                12.0 * s,
                40.0 * s,
                [255, 236, 200, 255],
            )
            .aligned(TextAlign::Center)
            .with_face(TextFace::Display),
        );
        list.texts.push(
            TextQuad::new(
                &format!("{}", self.core.stats.score),
                14.0 * s,
                14.0 * s,
                22.0 * s,
                [255, 236, 200, 255],
            )
            .with_face(TextFace::Display),
        );
        // Lives: little crates, top right.
        let r = 9.0 * s;
        for i in 0..self.spec.lives {
            let x = w - 16.0 * s - (i as f32 + 1.0) * (r * 2.0 + 5.0 * s);
            let col = if i < self.lives {
                [214, 160, 96, 255]
            } else {
                [90, 90, 110, 120]
            };
            list.quads
                .push(Quad::round(x, 18.0 * s, r * 2.0, r * 2.0, 3.0 * s, col));
        }
        // Balance: a track with a marker; red near the limit.
        let (bw, bh) = (150.0 * s, 8.0 * s);
        let (bx, by) = (w * 0.5 - bw * 0.5, 64.0 * s);
        round_panel(
            &mut list,
            bx,
            by,
            bw,
            bh,
            bh * 0.5,
            [10, 10, 30, 170],
            [255, 255, 255, 40],
            1.0,
            (0.0, 0.0, 0),
        );
        let k = (self.lean() / self.spec.lean_limit).clamp(-1.0, 1.0);
        let col = if k.abs() > 0.7 {
            [255, 100, 90, 255]
        } else {
            [255, 220, 150, 255]
        };
        list.quads.push(Quad::circle(
            bx + bw * 0.5 + k * (bw * 0.5 - bh),
            by + bh * 0.5,
            bh * 0.9,
            col,
        ));
        list.quads.push(Quad::new(
            bx + bw * 0.5 - 0.5 * s,
            by - 2.0 * s,
            1.0 * s,
            bh + 4.0 * s,
            [255, 255, 255, 120],
        ));
        if self.shout.1 > 0.0 {
            let (word, col) = match self.shout.0 {
                "PERFECT" => (self.tr("PERFECT", "ぴったり！"), [150, 255, 180]),
                "CHEST" => (self.tr("TREASURE +50", "たからばこ +50"), [255, 210, 120]),
                "MISS" => (self.tr("MISS", "おっと"), [255, 130, 120]),
                "TOPPLED" => (self.tr("TOPPLED", "たおれた"), [255, 130, 120]),
                _ => ("", [255, 255, 255]),
            };
            let a = (255.0 * (self.shout.1 / 0.4).min(1.0)) as u8;
            let label = if self.shout.0 == "PERFECT" && self.combo > 1 {
                format!("{word} x{}", self.combo)
            } else {
                word.to_string()
            };
            list.texts.push(
                TextQuad::new(
                    &label,
                    w * 0.5,
                    h * 0.3,
                    26.0 * s,
                    [col[0], col[1], col[2], a],
                )
                .aligned(TextAlign::Center),
            );
        }
        if self.height() == 0 && self.dropping.is_none() && self.flow.phase == FlowPhase::Play {
            list.texts.push(
                TextQuad::new(
                    self.tr("Tap to drop the crate", "タップで箱を落とす"),
                    w * 0.5,
                    h * 0.78,
                    22.0 * s,
                    [255, 240, 220, 230],
                )
                .aligned(TextAlign::Center),
            );
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

    // ---- what the page reads

    pub fn state_value(&self) -> Value {
        json!({
            "phase": self.flow.phase.name(),
            "score": self.core.stats.score,
            "best": self.flow.best_score,
            "secs": self.t,
            "height": self.height(),
            "lives": self.lives,
            "perfects": self.perfects,
            "combo": self.combo,
            "lean": self.lean(),
            "win": self.win,
            "death": self.core.stats.death,
            "death_text": match self.core.stats.death.as_str() {
                "" => "",
                "toppled" => self.tr("The tower toppled", "塔がたおれた"),
                _ => self.tr("Out of crates", "箱がなくなった"),
            },
            "result": if self.flow.phase == FlowPhase::Result {
                json!({
                    "key": self.spec.id,
                    "title": self.spec.title,
                    "enabled": true,
                    "top_n": 5,
                    "columns": ["height", "perfect", "score"],
                    "sort": "score",
                    "run": {"score": self.core.stats.score, "height": self.height(), "perfect": self.perfects},
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

impl Kit for CrateTower {
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
    fn substeps(&self, _dt: f32) -> u32 {
        1
    }
    fn step(&mut self, dt: f32) {
        self.step_sim(dt);
    }
    fn attract(&mut self, dt: f32) {
        self.t += dt;
        self.phase += self.spec.speed * dt;
    }
    fn follow(&mut self, _dt: f32) {}
    fn blocks_spark(&self, _x: f32, _y: f32, _z: f32) -> bool {
        false
    }
    fn rebuild(&mut self) {
        self.build_doc();
    }
}

impl crate::kit_host::KitHost for CrateTower {
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
        let body = self.doc.to_json().unwrap_or_else(|_| "{}".into());
        match body.strip_prefix('{') {
            Some(rest) => format!("{{\n  \"sim\": {},{rest}", self.state_value()),
            None => body,
        }
    }
    fn state_json(&self) -> String {
        self.state_value().to_string()
    }
    fn confirm(&mut self) {
        CrateTower::confirm(self)
    }
    fn show_title(&mut self) {
        CrateTower::show_title(self)
    }
    fn pointer(&mut self, _x: f32, _y: f32, _width: u32, _height: u32) -> bool {
        false
    }
    fn on_key(&mut self, name: &str, down: bool) -> bool {
        self.key(name, down)
    }
    fn tick(&mut self, dt: f32) {
        tick_frame(self, dt);
    }
    fn hud(&self, width: u32, height: u32, menu: bool) -> DrawList {
        CrateTower::hud(self, width, height, menu)
    }
    fn walk(&mut self, _lx: f32, _lz: f32, _jump: bool) {}
    fn touch(&mut self, ev: PointerEvent, w: u32, h: u32, _t: f32) {
        self.touch_ev(ev, w, h);
    }
    fn uses_swipe(&self) -> bool {
        // The kit reads the pointer itself (a tap drops): the page hides its own pads.
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
mod tests {
    use super::*;
    use crate::kit_host::KitHost;

    const DT: f32 = 1.0 / 60.0;

    fn started() -> CrateTower {
        let mut g = CrateTower::new(CrateTowerSpec::default());
        g.confirm();
        g
    }

    /// Swing until the hanging crate is at `x` (within a step), then drop and let it land.
    fn drop_at(g: &mut CrateTower, x: f32) {
        g.phase = (x / g.spec.swing).clamp(-1.0, 1.0).asin();
        assert!(g.drop_crate());
        for _ in 0..120 {
            g.tick(DT);
            if g.dropping.is_none() {
                break;
            }
        }
    }

    #[test]
    fn the_default_spec_is_valid_and_strict() {
        CrateTowerSpec::default().validate().unwrap();
        let mut v = CrateTower::new(CrateTowerSpec::default()).world_value()["game"].clone();
        v["bogus"] = json!(1);
        assert!(CrateTowerSpec::from_value(&v).is_err());
        let bad = CrateTowerSpec {
            lives: 0,
            ..CrateTowerSpec::default()
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn a_centred_drop_is_perfect_and_the_combo_pays() {
        let mut g = started();
        drop_at(&mut g, 0.0);
        assert_eq!(g.height(), 1);
        assert_eq!(g.perfects, 1);
        assert_eq!(g.tower[1], 0.0, "snaps square");
        drop_at(&mut g, 0.05);
        assert_eq!(g.combo, 2);
        assert_eq!(g.core.stats.score, (10 + 5) + (10 + 10));
        drop_at(&mut g, 0.4);
        assert_eq!(g.height(), 3);
        assert_eq!(g.combo, 0, "off centre breaks the combo");
        assert!((g.top_x() - 0.4).abs() < 0.05);
    }

    #[test]
    fn a_miss_costs_a_life_and_the_last_ends_the_run() {
        let mut g = started();
        for i in 0..3 {
            drop_at(&mut g, 1.9);
            assert_eq!(g.lives, 2 - i);
            assert_eq!(g.height(), 0);
        }
        for _ in 0..60 {
            g.tick(DT);
        }
        assert_eq!(g.phase_name(), "result");
        assert_eq!(g.state_value()["death"], "dropped");
    }

    #[test]
    fn leaning_too_far_topples_the_tower() {
        let mut g = started();
        let mut x = 0.0_f32;
        while g.core.stats.alive {
            x += 0.6;
            drop_at(&mut g, x.min(1.9));
            assert!(g.height() < 10, "it should have gone over by now");
        }
        assert_eq!(g.state_value()["death"], "toppled");
        assert!(!g.tumbles.is_empty());
    }

    #[test]
    fn every_tenth_is_a_chest_and_the_swing_speeds_up() {
        let mut g = started();
        let s0 = g.speed();
        for _ in 0..10 {
            drop_at(&mut g, 0.0);
        }
        assert_eq!(g.height(), 10);
        assert!(g.is_chest(10));
        // 10 crates, 10 perfects in a row (5 * (1 + ... + 10)), one chest.
        assert_eq!(g.core.stats.score, 100 + 5 * 55 + 50);
        assert!(g.speed() > s0);
    }

    #[test]
    fn one_drop_at_a_time_and_space_drops() {
        let mut g = started();
        assert!(g.on_key("space", true));
        assert!(!g.drop_crate(), "already falling");
        assert!(!g.on_key("space", false));
        g.pause_run();
        for _ in 0..30 {
            g.tick(DT);
        }
        assert!(g.dropping.is_some(), "paused mid-air");
        g.resume_run();
        for _ in 0..60 {
            g.tick(DT);
        }
        assert!(g.dropping.is_none());
    }

    #[test]
    fn the_swing_and_the_top_are_on_screen_either_way_up() {
        let mut g = started();
        for _ in 0..6 {
            drop_at(&mut g, 0.0);
        }
        for _ in 0..120 {
            g.tick(DT);
        }
        for (w, h) in [(390_u32, 844_u32), (844, 390)] {
            let _ = g.hud(w, h, false);
            g.rebuild();
            let cam = &g.document().cameras[0];
            let view = View {
                eye: cam.position,
                target: cam.target,
                fov: cam.fov,
            };
            let top = g.top_y();
            for p in [
                [-g.spec.swing, top + g.spec.hover, 0.0],
                [g.spec.swing, top + g.spec.hover, 0.0],
                [0.0, top, 0.0],
            ] {
                let (sx, sy) = view.project(p, w as f32, h as f32).unwrap();
                assert!(
                    (0.0..=w as f32).contains(&sx) && (0.0..=h as f32).contains(&sy),
                    "{w}x{h}: {p:?} at ({sx}, {sy})"
                );
            }
            let hud = g.hud(w, h, false);
            assert!(hud.texts.iter().any(|t| t.face == TextFace::Display));
        }
    }

    #[test]
    fn the_gpu_model_set_never_changes_while_stacking() {
        let mut g = started();
        g.rebuild();
        let key = g.document().gpu_mesh_key();
        for i in 0..14 {
            drop_at(&mut g, if i % 5 == 4 { 1.9 } else { 0.0 });
            g.rebuild();
            assert_eq!(g.document().gpu_mesh_key(), key, "crate {i}");
        }
    }

    #[test]
    fn the_baked_fixture_is_the_default_world() {
        let want = CrateTower::new(CrateTowerSpec::default()).world_value();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/crate_tower_world.json"
        );
        if std::env::var("KAGRA_BAKE").is_ok() {
            std::fs::write(path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
        }
        let got: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default())
            .unwrap_or(Value::Null);
        assert_eq!(
            got, want,
            "run `KAGRA_BAKE=1 cargo test -p kagra-shared --lib crate_tower::` to refresh it"
        );
        assert!(is_crate_tower_dump(&got.to_string()));
        let loaded = crate::kit_host::load(&got.to_string()).unwrap().unwrap();
        assert_eq!(loaded.name(), GAME_ID, "the session finds this kit");
    }
}
