//! Lantern Watch: keep the graveyard lantern lit until dawn.
//!
//! The dead walk in from the dark toward the lantern in the middle of the graveyard. Tap one
//! to strike it: a ghost or a skeleton falls to one tap, a zombie takes three, and every tap
//! that lands knocks the walker back. A walker that reaches the lantern dims it; the fifth one
//! puts it out. Last `secs` seconds to see the dawn. Hits in a row build a combo multiplier;
//! a tap on empty ground breaks it. One thumb, portrait or landscape.
//!
//! Built from the Starter template (`scripts/new_kit.py`) on the shared parts:
//!
//! - [`crate::game_core`]: the fixed-step driver ([`tick_frame`]), run stats, hitstop, the
//!   crowd [`motion`], the touch camera [`View`].
//! - [`crate::hud_kit`]: the rounded [`gauge`].
//! - [`crate::kit_host::KitHost`]: what the session and the page call.

use std::cell::Cell;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::flow::{FlowPhase, GameFlow};
use crate::game_core::motion::{self, Motion};
use crate::game_core::swarm::Rng;
use crate::game_core::view::View;
use crate::game_core::{tick_frame, Core, Kit};
use crate::hop_beat::{glb_prop, glb_ready, light, prop};
use crate::hud_kit::gauge;
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, Quad, TextAlign, TextFace, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc};

/// The `genre` of this kit's world file (and the `game.id` default).
pub const GAME_ID: &str = "lantern_watch";

/// True for a world file whose `genre` is this kit's.
pub fn is_lantern_watch_dump(json: &str) -> bool {
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

/// One kind of walker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FoeKind {
    pub id: String,
    /// A GLB under the bundle; a missing one draws as a sphere.
    pub model: String,
    /// Body motion: `float`, `shamble`, `hop`, `glide` or `stomp`.
    pub motion: String,
    /// Taps to fell it.
    pub hp: u32,
    /// Walking speed (m/s) toward the lantern.
    pub speed: f32,
    /// Sideways weave (m) while it walks, 0 = straight.
    pub weave: f32,
    pub score: u32,
    /// Seconds into the night before it appears, and its share of the spawns after that.
    pub from_secs: f32,
    pub weight: f32,
    /// Model scale.
    pub scale: f32,
}

impl Default for FoeKind {
    fn default() -> Self {
        Self {
            id: "ghost".into(),
            model: format!("{GRAVE}character-ghost.glb"),
            motion: "float".into(),
            hp: 1,
            speed: 1.5,
            weave: 0.0,
            score: 10,
            from_secs: 0.0,
            weight: 1.0,
            scale: 1.9,
        }
    }
}

const GRAVE: &str = "assets/models/kenney/graveyard-kit/";

fn default_foes() -> Vec<FoeKind> {
    let kind =
        |id: &str, file: &str, m: &str, hp, speed, weave, score, from, weight, scale| FoeKind {
            id: id.into(),
            model: format!("{GRAVE}{file}"),
            motion: m.into(),
            hp,
            speed,
            weave,
            score,
            from_secs: from,
            weight,
            scale,
        };
    vec![
        kind(
            "ghost",
            "character-ghost.glb",
            "float",
            1,
            1.5,
            0.6,
            10,
            0.0,
            1.0,
            1.9,
        ),
        kind(
            "skeleton",
            "character-skeleton.glb",
            "hop",
            1,
            2.4,
            0.0,
            15,
            15.0,
            0.7,
            1.9,
        ),
        kind(
            "zombie",
            "character-zombie.glb",
            "shamble",
            3,
            0.8,
            0.0,
            40,
            30.0,
            0.5,
            2.0,
        ),
        kind(
            "vampire",
            "character-vampire.glb",
            "glide",
            2,
            1.9,
            1.4,
            30,
            50.0,
            0.5,
            2.0,
        ),
    ]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LanternWatchSpec {
    pub id: String,
    pub title: String,
    /// Seconds until dawn (a win).
    pub secs: f32,
    /// The graveyard is an ellipse: half width (x) and half depth (z), metres. Walkers start on
    /// its rim. Taller than wide, so a portrait phone shows it large.
    pub field_x: f32,
    pub field_z: f32,
    /// Walkers the lantern can take, and how close (m) counts as reaching it.
    pub lantern_hp: u32,
    pub lantern_radius: f32,
    /// Seconds between spawns at dusk and at the end of the night.
    pub start_gap: f32,
    pub end_gap: f32,
    /// Walkers on the field at most.
    pub max_foes: u32,
    /// How far (m) a tap that does not fell a walker pushes it back.
    pub knock: f32,
    /// Tap reach in pixels at 400 px of short side.
    pub tap_px: f32,
    /// Each hit in a row adds this to the score multiplier, up to `combo_max`.
    pub combo_step: f32,
    pub combo_max: f32,
    /// Score for each light left at dawn.
    pub dawn_bonus: u32,
    pub foes: Vec<FoeKind>,
    pub seed: u32,
    /// Music and sound cues for the page (file names under the bundle). Passed through.
    pub audio: Option<Value>,
}

impl Default for LanternWatchSpec {
    fn default() -> Self {
        Self {
            id: GAME_ID.into(),
            title: "Lantern Watch".into(),
            secs: 90.0,
            field_x: 5.0,
            field_z: 8.5,
            lantern_hp: 5,
            lantern_radius: 0.9,
            start_gap: 1.6,
            end_gap: 0.42,
            max_foes: 24,
            knock: 1.3,
            tap_px: 50.0,
            combo_step: 0.1,
            combo_max: 3.0,
            dawn_bonus: 100,
            foes: default_foes(),
            seed: 11,
            audio: Some(default_audio()),
        }
    }
}

impl LanternWatchSpec {
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let spec: Self =
            serde_json::from_value(v.clone()).map_err(|e| format!("lantern_watch spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), String> {
        let range = |name: &str, v: f32, lo: f32, hi: f32| {
            if v.is_finite() && (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(format!(
                    "lantern_watch: {name} must be {lo}..={hi}, got {v}"
                ))
            }
        };
        range("secs", self.secs, 5.0, 600.0)?;
        range("field_x", self.field_x, 2.0, 30.0)?;
        range("field_z", self.field_z, 2.0, 30.0)?;
        range("lantern_radius", self.lantern_radius, 0.2, 3.0)?;
        range("start_gap", self.start_gap, 0.05, 10.0)?;
        range("end_gap", self.end_gap, 0.05, 10.0)?;
        range("knock", self.knock, 0.0, 10.0)?;
        range("tap_px", self.tap_px, 10.0, 200.0)?;
        range("combo_step", self.combo_step, 0.0, 1.0)?;
        range("combo_max", self.combo_max, 1.0, 10.0)?;
        if !(1..=20).contains(&self.lantern_hp) {
            return Err("lantern_watch: lantern_hp must be 1..=20".into());
        }
        if !(1..=200).contains(&self.max_foes) {
            return Err("lantern_watch: max_foes must be 1..=200".into());
        }
        if self.foes.is_empty() || self.foes.len() > 16 {
            return Err("lantern_watch: foes must have 1..=16 kinds".into());
        }
        for f in &self.foes {
            let name = format!("foes.{}", f.id);
            if Motion::parse(&f.motion).is_none() {
                return Err(format!(
                    "lantern_watch: {name}.motion: unknown {:?}",
                    f.motion
                ));
            }
            if !(1..=50).contains(&f.hp) {
                return Err(format!("lantern_watch: {name}.hp must be 1..=50"));
            }
            range(&format!("{name}.speed"), f.speed, 0.05, 20.0)?;
            range(&format!("{name}.weave"), f.weave, 0.0, 5.0)?;
            range(&format!("{name}.from_secs"), f.from_secs, 0.0, 600.0)?;
            range(&format!("{name}.weight"), f.weight, 0.0, 100.0)?;
            range(&format!("{name}.scale"), f.scale, 0.1, 10.0)?;
        }
        if !self
            .foes
            .iter()
            .any(|f| f.from_secs <= 0.0 && f.weight > 0.0)
        {
            return Err("lantern_watch: one kind must walk from the start (from_secs 0)".into());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- look ----

const LANTERN_GLB: &str = "assets/models/kenney/graveyard-kit/lantern-candle.glb";
/// The graveyard dressing, in a fixed ring outside the field.
const DRESSING: [&str; 5] = [
    "gravestone-round.glb",
    "gravestone-cross.glb",
    "gravestone-broken.glb",
    "cross-wood.glb",
    "pumpkin-carved.glb",
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
            "hit": {"file": "assets/audio/kenney-impact-sounds/impactGeneric_light_001.wav", "volume": 0.5, "rate": 1.2},
            "kill": {"file": "assets/audio/oga-wet-squish/impactsplat03.wav", "volume": 0.35, "rate": 1.1},
            "hurt": cue("retro_die_01.wav", 0.7, 1.0),
            "win": cue("power_up_04.wav", 0.8, 1.0),
            "gameover": cue("retro_die_03.wav", 0.8, 1.0),
            "go": {"file": "assets/audio/kenney-interface-sounds/confirmation_001.wav", "volume": 0.7, "rate": 1.0}
        }
    })
}

// ----------------------------------------------------------------- run ----

#[derive(Clone, Debug)]
struct Foe {
    uid: u32,
    kind: usize,
    x: f32,
    z: f32,
    hp: u32,
    /// The hit flash, seconds left.
    flash: f32,
    /// Seconds since it rose (it grows in).
    age: f32,
}

#[derive(Debug)]
pub struct LanternWatch {
    spec: LanternWatchSpec,
    core: Core,
    flow: GameFlow,
    doc: WorldDoc,
    rng: Rng,
    /// Seconds into the night (and the title's clock).
    t: f32,
    foes: Vec<Foe>,
    next_uid: u32,
    spawn_in: f32,
    light: u32,
    kills: u32,
    combo: u32,
    best_combo: u32,
    /// Puffs where a walker fell: x, z, seconds left.
    puffs: Vec<[f32; 3]>,
    /// Tap rings on the HUD: x, y (pixels), seconds left, landed (1) or missed (0).
    taps: Vec<[f32; 4]>,
    /// The lantern's flicker after a hit, seconds left.
    dim: f32,
    win: bool,
    new_best: bool,
    best_before: u32,
    lang_ja: bool,
    /// The canvas in pixels, from the last HUD call or touch (the camera fits it).
    size: Cell<(f32, f32)>,
}

impl LanternWatch {
    pub fn new(spec: LanternWatchSpec) -> Self {
        let rng = Rng::new(spec.seed);
        let mut flow = GameFlow::new(spec.id.clone());
        flow.title = spec.title.clone();
        flow.tagline = String::new();
        let mut g = Self {
            light: spec.lantern_hp,
            spec,
            core: Core::default(),
            flow,
            doc: WorldDoc::default(),
            rng,
            t: 0.0,
            foes: Vec::new(),
            next_uid: 1,
            spawn_in: 0.0,
            kills: 0,
            combo: 0,
            best_combo: 0,
            puffs: Vec::new(),
            taps: Vec::new(),
            dim: 0.0,
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
            serde_json::from_str(json).map_err(|e| format!("lantern_watch dump: {e}"))?;
        let spec = match value.get("game") {
            Some(g) => LanternWatchSpec::from_value(g)?,
            None => LanternWatchSpec::default(),
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

    /// The score multiplier the current combo earns (the first hit is x1).
    fn mult(&self) -> f32 {
        (1.0 + self.combo.saturating_sub(1) as f32 * self.spec.combo_step).min(self.spec.combo_max)
    }

    // ---- flow

    fn reset_field(&mut self) {
        self.rng = Rng::new(self.spec.seed);
        self.t = 0.0;
        self.foes.clear();
        self.next_uid = 1;
        self.spawn_in = 0.6;
        self.light = self.spec.lantern_hp;
        self.kills = 0;
        self.combo = 0;
        self.best_combo = 0;
        self.puffs.clear();
        self.taps.clear();
        self.dim = 0.0;
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

    fn finish(&mut self, win: bool) {
        self.win = win;
        if win {
            self.core.stats.score += self.light * self.spec.dawn_bonus;
        }
        crate::mixer::stop_bgm(0.6);
        crate::mixer::cue_sfx(if win { "win" } else { "gameover" });
        let score = self.core.stats.score;
        self.new_best = score > self.best_before;
        self.flow
            .finish(score, self.kills, if win { "DAWN" } else { "DARK" });
    }

    // ---- sim

    /// Seconds until the next walker rises, easing from dusk to the end of the night.
    fn gap(&self) -> f32 {
        let k = (self.t / self.spec.secs).clamp(0.0, 1.0);
        self.spec.start_gap + (self.spec.end_gap - self.spec.start_gap) * k
    }

    fn spawn(&mut self) {
        let total: f32 = self
            .spec
            .foes
            .iter()
            .filter(|f| f.from_secs <= self.t)
            .map(|f| f.weight)
            .sum();
        if total <= 0.0 {
            return;
        }
        let mut pick = self.rng.range(0.0, total);
        let mut kind = 0;
        for (i, f) in self.spec.foes.iter().enumerate() {
            if f.from_secs > self.t || f.weight <= 0.0 {
                continue;
            }
            kind = i;
            if pick < f.weight {
                break;
            }
            pick -= f.weight;
        }
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        let uid = self.next_uid;
        self.next_uid += 1;
        self.foes.push(Foe {
            uid,
            kind,
            x: a.cos() * self.spec.field_x,
            z: a.sin() * self.spec.field_z,
            hp: self.spec.foes[kind].hp,
            flash: 0.0,
            age: 0.0,
        });
    }

    /// One fixed step of the rules.
    fn step_sim(&mut self, dt: f32) {
        self.t += dt;
        self.dim = (self.dim - dt).max(0.0);
        for p in &mut self.puffs {
            p[2] -= dt;
        }
        self.puffs.retain(|p| p[2] > 0.0);
        for t in &mut self.taps {
            t[2] -= dt;
        }
        self.taps.retain(|t| t[2] > 0.0);
        self.spawn_in -= dt;
        if self.spawn_in <= 0.0 {
            self.spawn_in += self.gap();
            if self.foes.len() < self.spec.max_foes as usize {
                self.spawn();
            }
        }
        // Walk in toward the lantern, weaving if the kind weaves.
        let reach = self.spec.lantern_radius;
        let mut reached = 0;
        let t = self.t;
        let kinds = &self.spec.foes;
        self.foes.retain_mut(|f| {
            let k = &kinds[f.kind];
            f.age += dt;
            f.flash = (f.flash - dt).max(0.0);
            let d = f.x.hypot(f.z).max(1e-3);
            let (ux, uz) = (-f.x / d, -f.z / d);
            let (phase, tempo) = motion::phase_tempo(f.uid);
            let side = k.weave * (t * 1.7 * tempo + phase).cos() * 1.7 * tempo;
            let slow = if f.flash > 0.0 { 0.3 } else { 1.0 };
            f.x += (ux * k.speed * slow - uz * side * (d / 3.0).min(1.0)) * dt;
            f.z += (uz * k.speed * slow + ux * side * (d / 3.0).min(1.0)) * dt;
            if f.x.hypot(f.z) < reach {
                reached += 1;
                false
            } else {
                true
            }
        });
        if reached > 0 {
            self.light = self.light.saturating_sub(reached);
            self.combo = 0;
            self.dim = 0.6;
            self.core.juice.hit(0.25, 0.08);
            crate::mixer::cue_sfx("hurt");
            if self.light == 0 {
                self.core.stats.alive = false;
                self.core.stats.just_crash = true;
                self.core.stats.death = "lantern".into();
                self.core.juice.hitstop = 0.35;
                self.new_best = self.core.stats.score > self.best_before;
                crate::mixer::stop_bgm(0.6);
                crate::mixer::cue_sfx("gameover");
                return;
            }
        }
        if self.t >= self.spec.secs {
            self.foes.clear();
            self.finish(true);
        }
    }

    // ---- input

    fn view(&self) -> View {
        let (w, h) = self.size.get();
        let (fx, fz) = (self.spec.field_x + 0.8, self.spec.field_z + 0.6);
        View::frame(
            [0.0, 0.0, 0.4],
            [0.0, 2.4, 1.0],
            40.0,
            &View::ground_box(-fx, fx, -fz, fz, 0.0),
            w / h.max(1.0),
        )
    }

    /// A tap at a canvas pixel: the nearest walker within reach takes a hit.
    pub fn tap(&mut self, x: f32, y: f32) -> bool {
        if self.flow.phase != FlowPhase::Play || !self.core.stats.alive {
            return false;
        }
        let (w, h) = self.size.get();
        let view = self.view();
        let reach = self.spec.tap_px * hud_scale(w, h);
        let mut best: Option<(usize, f32)> = None;
        for (i, f) in self.foes.iter().enumerate() {
            let k = &self.spec.foes[f.kind];
            // The middle of the body, not the feet: that is where a thumb goes.
            let Some((sx, sy)) = view.project([f.x, 0.35 * k.scale, f.z], w, h) else {
                continue;
            };
            let d = (sx - x).hypot(sy - y);
            if d < reach && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        let Some((i, _)) = best else {
            self.combo = 0;
            self.taps.push([x, y, 0.3, 0.0]);
            return false;
        };
        self.taps.push([x, y, 0.35, 1.0]);
        self.combo += 1;
        self.best_combo = self.best_combo.max(self.combo);
        let f = &mut self.foes[i];
        f.hp -= 1;
        f.flash = 0.12;
        if f.hp == 0 {
            let f = self.foes.swap_remove(i);
            let pts = (self.spec.foes[f.kind].score as f32 * self.mult()).round() as u32;
            self.core.stats.score += pts;
            self.kills += 1;
            self.core.stats.rings = self.kills;
            self.core.stats.just_rings += 1;
            self.puffs.push([f.x, f.z, 0.5]);
            crate::mixer::cue_sfx("kill");
        } else {
            let d = f.x.hypot(f.z).max(1e-3);
            let k = self.spec.knock / d;
            f.x += f.x * k;
            f.z += f.z * k;
            crate::mixer::cue_sfx("hit");
        }
        true
    }

    pub fn touch_ev(&mut self, ev: PointerEvent, w: u32, h: u32) {
        self.size.set((w.max(1) as f32, h.max(1) as f32));
        if ev.phase == PointerPhase::Begin {
            self.tap(ev.x, ev.y);
        }
    }

    // ---- picture

    /// The 3D world. Built once per frame from the state.
    fn build_doc(&mut self) {
        let (fx, fz) = (self.spec.field_x, self.spec.field_z);
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: fz + 6.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([8, 10, 26, 255]),
            hemi_sky: Some([0.17, 0.21, 0.4]),
            hemi_ground: Some([0.13, 0.12, 0.2]),
            env_map: Some("preset:studio".into()),
            ibl: Some(0.25),
            exposure: Some(0.85),
            fog_start: Some(40.0),
            fog_end: Some(90.0),
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
            [0.0, -0.1, 0.0],
            [90.0, 0.2, 90.0],
            [42, 50, 54],
            0.0,
            0.95,
            0.0,
        ));
        // The path ring where the dead rise: a faint ellipse of stones.
        for i in 0..28 {
            let a = i as f32 / 28.0 * std::f32::consts::TAU;
            doc.props.push(prop(
                &format!("rim-{i}"),
                "sphere",
                "rim",
                [a.cos() * (fx + 0.3), 0.0, a.sin() * (fz + 0.3)],
                [0.22, 0.08, 0.22],
                [70, 76, 92],
                0.0,
                0.9,
                0.0,
            ));
        }
        let model = |id: &str, path: &str, name: &str, pos: [f32; 3], s: f32, col: [u32; 3]| {
            if glb_ready(path) {
                glb_prop(id, path, name, pos, [s; 3], [255, 255, 255], 0.0, None)
            } else {
                prop(
                    id,
                    "sphere",
                    name,
                    [pos[0], pos[1] + s * 0.4, pos[2]],
                    [s * 0.8; 3],
                    col,
                    0.0,
                    0.7,
                    0.0,
                )
            }
        };
        // Graves and crosses just outside the rim, pumpkins among them (they glow).
        for i in 0..18 {
            let a = (i as f32 + 0.5) / 18.0 * std::f32::consts::TAU;
            let w = 1.0 + 0.35 * ((i * 7 % 5) as f32 / 4.0);
            let pick = (i * 3 + i / 4) % DRESSING.len();
            let path = format!("{GRAVE}{}", DRESSING[pick]);
            let mut p = model(
                &format!("grave-{i}"),
                &path,
                "grave",
                [a.cos() * (fx + 1.6 * w), 0.0, a.sin() * (fz + 1.6 * w)],
                2.0,
                [90, 96, 110],
            );
            p.yaw = (-a.cos()).atan2(-a.sin());
            p.color = Some([120, 124, 140]);
            if pick == 4 {
                p.emissive = Some(0.6);
                p.emissive_color = Some([1.0, 0.55, 0.15]);
            }
            doc.props.push(p);
        }
        // The lantern: brighter with every light left, a flicker after a hit.
        let lit = self.light as f32 / self.spec.lantern_hp as f32;
        let flicker = 1.0 + 0.06 * (self.t * 13.0).sin() * (self.t * 7.3).cos();
        let glow = (0.35 + 1.4 * lit) * flicker * if self.dim > 0.0 { 0.5 } else { 1.0 };
        let mut lamp = model(
            "lantern",
            LANTERN_GLB,
            "lantern",
            [0.0; 3],
            2.6,
            [255, 200, 120],
        );
        lamp.emissive = Some(glow * 0.45);
        lamp.emissive_color = Some([1.0, 0.7, 0.3]);
        doc.props.push(lamp);
        // The dead.
        for f in &self.foes {
            let k = &self.spec.foes[f.kind];
            let m = Motion::parse(&k.motion).unwrap_or(Motion::Float);
            let (phase, tempo) = motion::phase_tempo(f.uid);
            let pose = motion::pose(m, self.t, phase, tempo, 1.0, f.flash);
            let grow = (f.age / 0.4).min(1.0);
            let s = k.scale * (0.3 + 0.7 * grow);
            let lift = if m == Motion::Float { 0.12 } else { 0.0 };
            let mut p = model(
                &format!("foe-{}", f.uid),
                &k.model,
                "foe",
                [f.x, lift + pose.lift * s.min(1.5) - 0.6 * (1.0 - grow), f.z],
                s,
                [200, 205, 235],
            );
            p.scale = [s * pose.wide, s * pose.tall, s * pose.wide];
            p.color = Some([215, 222, 255]);
            p.yaw = (-f.x).atan2(-f.z);
            p.pitch = -pose.pitch;
            p.roll = pose.roll;
            let hurt = 1.0 - f.hp as f32 / k.hp.max(1) as f32;
            p.emissive = Some(0.08 + 0.25 * hurt + if f.flash > 0.0 { 2.0 } else { 0.0 });
            p.emissive_color = Some(if f.flash > 0.0 {
                [1.0, 1.0, 1.0]
            } else {
                [0.55, 0.6, 1.0]
            });
            doc.props.push(p);
        }
        // Where a walker fell: a pale wisp that rises and shrinks.
        for (i, pf) in self.puffs.iter().enumerate() {
            let k = pf[2] / 0.5;
            doc.props.push(prop(
                &format!("puff-{i}"),
                "sphere",
                "puff",
                [pf[0], 0.5 + (1.0 - k) * 1.4, pf[1]],
                [0.7 * k + 0.1; 3],
                [190, 210, 255],
                2.5 * k,
                0.5,
                0.0,
            ));
        }
        let view = self.view();
        let sh = self.core.juice.shake;
        let shake = [
            sh * (self.t * 53.0).sin(),
            sh * 0.5 * (self.t * 41.0).cos(),
            0.0,
        ];
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "graveyard".into(),
            position: [
                view.eye[0] + shake[0],
                view.eye[1] + shake[1],
                view.eye[2] + shake[2],
            ],
            target: view.target,
            fov: view.fov,
            roll: 0.0,
        });
        doc.lights.push(light(
            0,
            "moon",
            [0.0, 12.0, 5.0],
            [0.0; 3],
            [0.7, 0.78, 1.0],
            1.0,
            40.0,
            false,
        ));
        doc.lights.push(light(
            1,
            "lantern",
            [0.0, 1.6, 0.0],
            [0.0, -1.0, 0.0],
            [1.0, 0.72, 0.38],
            glow * 2.6,
            8.0 + 5.0 * lit,
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
        let left = (self.spec.secs - self.t).max(0.0).ceil() as u32;
        list.texts.push(
            TextQuad::new(
                &format!("{}:{:02}", left / 60, left % 60),
                w * 0.5,
                14.0 * s,
                28.0 * s,
                [255, 236, 200, 255],
            )
            .aligned(TextAlign::Center)
            .with_face(TextFace::Display),
        );
        // Night to dawn: the bar fills toward morning.
        gauge(
            &mut list,
            w * 0.5 - 80.0 * s,
            50.0 * s,
            160.0 * s,
            7.0 * s,
            self.t / self.spec.secs,
            [255, 196, 140, 255],
            [8, 8, 24, 170],
        );
        list.texts.push(
            TextQuad::new(
                &format!("{}", self.core.stats.score),
                14.0 * s,
                14.0 * s,
                24.0 * s,
                [255, 236, 200, 255],
            )
            .with_face(TextFace::Display),
        );
        // The lantern's lights, top right.
        let n = self.spec.lantern_hp;
        let r = 7.0 * s;
        for i in 0..n {
            let cx = w - 16.0 * s - r - i as f32 * (r * 2.0 + 5.0 * s);
            let cy = 28.0 * s;
            if i < self.light {
                list.quads
                    .push(Quad::circle(cx, cy, r * 1.6, [255, 170, 70, 50]));
                list.quads
                    .push(Quad::circle(cx, cy, r, [255, 200, 110, 255]));
            } else {
                list.quads
                    .push(Quad::ring(cx, cy, r, 1.5 * s, [150, 150, 180, 160]));
            }
        }
        if self.combo >= 3 {
            list.texts.push(
                TextQuad::new(
                    &format!(
                        "x{:.1}  {} {}",
                        self.mult(),
                        self.combo,
                        self.tr("COMBO", "コンボ")
                    ),
                    w * 0.5,
                    70.0 * s,
                    18.0 * s,
                    [190, 215, 255, 255],
                )
                .aligned(TextAlign::Center),
            );
        }
        if self.dim > 0.0 {
            let a = (self.dim / 0.6 * 90.0) as u8;
            list.quads.push(Quad::new(0.0, 0.0, w, h, [40, 0, 10, a]));
        }
        for t in &self.taps {
            let k = t[2] / 0.35;
            let (r, col) = if t[3] > 0.0 {
                (
                    26.0 * s * (1.4 - 0.6 * k),
                    [255, 230, 170, (220.0 * k) as u8],
                )
            } else {
                (18.0 * s, [170, 170, 200, (140.0 * k) as u8])
            };
            list.quads.push(Quad::ring(t[0], t[1], r, 2.5 * s, col));
        }
        if self.t < 4.0 && self.flow.phase == FlowPhase::Play {
            list.texts.push(
                TextQuad::new(
                    self.tr("Tap the dead!", "近づく亡者をタップ！"),
                    w * 0.5,
                    h * 0.72,
                    22.0 * s,
                    [255, 240, 220, (255.0 * (1.0 - self.t / 4.0).min(1.0)) as u8],
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
            "secs_total": self.spec.secs,
            "kills": self.kills,
            "light": self.light,
            "combo": self.combo,
            "foes": self.foes.len(),
            "win": self.win,
            "death": self.core.stats.death,
            "death_text": if self.core.stats.death.is_empty() { "" } else { self.tr("The lantern went out", "ランタンが消えた") },
            "result": if self.flow.phase == FlowPhase::Result {
                json!({
                    "key": format!("{}:{}", self.spec.id, self.spec.seed),
                    "title": self.spec.title,
                    "enabled": true,
                    "top_n": 5,
                    "columns": ["kills", "combo", "score"],
                    "sort": "score",
                    "run": {"score": self.core.stats.score, "kills": self.kills, "combo": self.best_combo, "win": u32::from(self.win)},
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

impl Kit for LanternWatch {
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
    }
    fn follow(&mut self, _dt: f32) {}
    fn blocks_spark(&self, _x: f32, _y: f32, _z: f32) -> bool {
        false
    }
    fn rebuild(&mut self) {
        self.build_doc();
    }
}

impl crate::kit_host::KitHost for LanternWatch {
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
        LanternWatch::confirm(self)
    }
    fn show_title(&mut self) {
        LanternWatch::show_title(self)
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
        LanternWatch::hud(self, width, height, menu)
    }
    fn walk(&mut self, _lx: f32, _lz: f32, _jump: bool) {}
    fn touch(&mut self, ev: PointerEvent, w: u32, h: u32, _t: f32) {
        self.touch_ev(ev, w, h);
    }
    fn uses_swipe(&self) -> bool {
        // The kit reads the pointer itself (taps): the page hides its own pads.
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
    const W: f32 = 390.0;
    const H: f32 = 844.0;

    fn started() -> LanternWatch {
        let mut g = LanternWatch::new(LanternWatchSpec::default());
        let _ = g.hud(W as u32, H as u32, false);
        g.confirm();
        g
    }

    fn one_foe(g: &mut LanternWatch, kind: usize, x: f32, z: f32) -> u32 {
        g.foes.clear();
        g.spawn_in = 999.0;
        g.foes.push(Foe {
            uid: 77,
            kind,
            x,
            z,
            hp: g.spec.foes[kind].hp,
            flash: 0.0,
            age: 1.0,
        });
        77
    }

    fn tap_on(g: &mut LanternWatch, i: usize) -> bool {
        let f = g.foes[i].clone();
        let k = &g.spec.foes[f.kind];
        let (x, y) = g.view().project([f.x, 0.35 * k.scale, f.z], W, H).unwrap();
        g.touch(
            PointerEvent {
                id: 1,
                x,
                y,
                phase: PointerPhase::Begin,
                pressure: 1.0,
            },
            W as u32,
            H as u32,
            0.0,
        );
        g.foes.get(i).is_none_or(|n| n.uid != f.uid || n.hp < f.hp)
    }

    #[test]
    fn the_default_spec_is_valid_and_strict() {
        LanternWatchSpec::default().validate().unwrap();
        let mut v = LanternWatch::new(LanternWatchSpec::default()).world_value()["game"].clone();
        v["bogus"] = json!(1);
        assert!(LanternWatchSpec::from_value(&v).is_err(), "unknown keys");
        let mut v = LanternWatch::new(LanternWatchSpec::default()).world_value()["game"].clone();
        v["foes"][0]["motion"] = json!("moonwalk");
        assert!(LanternWatchSpec::from_value(&v).is_err(), "unknown motion");
        let late = LanternWatchSpec {
            foes: vec![FoeKind {
                from_secs: 5.0,
                ..FoeKind::default()
            }],
            ..LanternWatchSpec::default()
        };
        assert!(late.validate().is_err(), "nobody walks at dusk");
    }

    #[test]
    fn the_dead_rise_on_the_rim_and_walk_in() {
        let mut g = started();
        for _ in 0..(3 * 60) {
            g.tick(DT);
        }
        assert!(!g.foes.is_empty(), "walkers rise");
        let f = &g.foes[0];
        let rim = (f.x / g.spec.field_x).hypot(f.z / g.spec.field_z);
        assert!(rim < 1.0, "they walk in: {rim}");
        // Late in the night the later kinds walk too, and more often.
        assert!(g.gap() < g.spec.start_gap);
        let mut g = started();
        g.t = 60.0;
        for _ in 0..200 {
            g.spawn();
        }
        for id in ["ghost", "skeleton", "zombie", "vampire"] {
            assert!(
                g.foes.iter().any(|f| g.spec.foes[f.kind].id == id),
                "{id} walks by 60 s"
            );
        }
    }

    #[test]
    fn a_tap_fells_a_ghost_and_a_zombie_takes_three() {
        let mut g = started();
        one_foe(&mut g, 0, 2.0, -5.0);
        assert!(tap_on(&mut g, 0));
        assert!(g.foes.is_empty());
        assert_eq!(g.kills, 1);
        assert_eq!(g.core.stats.score, 10);
        one_foe(&mut g, 2, -2.0, -4.0);
        let d0 = g.foes[0].x.hypot(g.foes[0].z);
        assert!(tap_on(&mut g, 0));
        assert_eq!(g.foes[0].hp, 2, "a zombie takes three");
        assert!(
            g.foes[0].x.hypot(g.foes[0].z) > d0 + 1.0,
            "and is knocked back"
        );
        assert!(tap_on(&mut g, 0));
        assert!(tap_on(&mut g, 0));
        assert!(g.foes.is_empty());
        assert_eq!(g.combo, 4);
        // Four in a row: x1.3 on the zombie's 40.
        assert_eq!(g.core.stats.score, 10 + 52);
    }

    #[test]
    fn a_tap_on_empty_ground_breaks_the_combo() {
        let mut g = started();
        g.combo = 5;
        one_foe(&mut g, 0, 3.0, -6.0);
        assert!(!g.tap(W * 0.5, H * 0.9));
        assert_eq!(g.combo, 0);
        assert_eq!(g.foes.len(), 1);
    }

    #[test]
    fn walkers_that_reach_the_lantern_put_it_out() {
        let mut g = started();
        for i in 0..g.spec.lantern_hp {
            one_foe(&mut g, 0, 0.0, -1.0);
            for _ in 0..60 {
                g.tick(DT);
            }
            assert_eq!(g.light, g.spec.lantern_hp - i - 1);
        }
        for _ in 0..60 {
            g.tick(DT);
        }
        assert_eq!(g.phase_name(), "result");
        assert_eq!(g.state_value()["death"], "lantern");
        assert_eq!(g.state_value()["win"], false);
    }

    #[test]
    fn dawn_wins_with_a_bonus_for_each_light() {
        let mut g = LanternWatch::new(LanternWatchSpec {
            secs: 5.0,
            start_gap: 10.0,
            end_gap: 10.0,
            ..LanternWatchSpec::default()
        });
        g.confirm();
        g.spawn_in = 999.0;
        for _ in 0..(6 * 60) {
            g.tick(DT);
        }
        assert_eq!(g.phase_name(), "result");
        assert_eq!(g.state_value()["win"], true);
        assert_eq!(g.core.stats.score, 5 * 100);
        assert!(g.state_value()["result"].is_object());
    }

    #[test]
    fn the_field_and_every_walker_are_on_screen_either_way_up() {
        let mut g = started();
        for (w, h) in [(390_u32, 844_u32), (844, 390), (1024, 768)] {
            let _ = g.hud(w, h, false);
            g.rebuild();
            let cam = &g.document().cameras[0];
            let view = View {
                eye: cam.position,
                target: cam.target,
                fov: cam.fov,
            };
            let (fx, fz) = (g.spec.field_x, g.spec.field_z);
            for (x, z) in [(fx, 0.0), (-fx, 0.0), (0.0, fz), (0.0, -fz)] {
                let (sx, sy) = view.project([x, 0.0, z], w as f32, h as f32).unwrap();
                assert!(
                    (0.0..=w as f32).contains(&sx) && (0.0..=h as f32).contains(&sy),
                    "{w}x{h}: rim ({x}, {z}) at ({sx}, {sy})"
                );
            }
        }
    }

    #[test]
    fn pause_freezes_the_night() {
        let mut g = started();
        g.tick(DT);
        g.pause_run();
        let t = g.t;
        for _ in 0..30 {
            g.tick(DT);
        }
        assert_eq!(g.t, t);
        assert!(!g.tap(100.0, 100.0), "no taps while paused");
        g.resume_run();
        g.tick(DT);
        assert!(g.t > t);
        let hud = g.hud(390, 844, false);
        assert!(hud.texts.iter().any(|t| t.face == TextFace::Display));
    }

    #[test]
    fn the_baked_fixture_is_the_default_world() {
        let want = LanternWatch::new(LanternWatchSpec::default()).world_value();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/lantern_watch_world.json"
        );
        if std::env::var("KAGRA_BAKE").is_ok() {
            std::fs::write(path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
        }
        let got: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default())
            .unwrap_or(Value::Null);
        assert_eq!(
            got, want,
            "run `KAGRA_BAKE=1 cargo test -p kagra-shared --lib lantern_watch::` to refresh it"
        );
        assert!(is_lantern_watch_dump(&got.to_string()));
        let loaded = crate::kit_host::load(&got.to_string()).unwrap().unwrap();
        assert_eq!(loaded.name(), GAME_ID, "the session finds this kit");
    }
}
