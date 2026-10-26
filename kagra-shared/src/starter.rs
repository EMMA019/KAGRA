//! Starter: the smallest whole kit, and the template `scripts/new_kit.py` copies.
//!
//! A hero in a square arena, steered by the floating thumb stick (or WASD / a gamepad). Stars
//! appear one at a time: touch one for points. Hazards bounce around: touch one and the run
//! ends. Last `run.secs` seconds to win. Title -> play -> result, a best score, a pause, two
//! languages, all JSON (`game` in the world file), all on the shared parts:
//!
//! - [`crate::game_core`]: the fixed-step driver ([`tick_frame`]), run stats, hitstop, the
//!   floating [`stick`].
//! - [`crate::hud_kit`]: the rounded [`gauge`].
//! - [`crate::game_core::view`]: a camera that fits the arena on a phone either way up.
//! - [`crate::mixer`]: music and sound cues; `game.audio` names the files the page plays.
//! - [`crate::kit_host::KitHost`]: what the session and the page call.
//!
//! To make a new game, run `python scripts/new_kit.py <slug> "<Title>"`, then change the
//! parts marked `GAME:` below. Keep the rest: it is what the page, the preview and the tests
//! expect. `docs/making_a_game.md` walks through it.

use std::cell::Cell;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::flow::{FlowPhase, GameFlow};
use crate::game_core::swarm::Rng;
use crate::game_core::view::View;
use crate::game_core::{stick, tick_frame, Core, Kit};
use crate::hop_beat::{glb_prop, glb_ready, light, prop};
use crate::hud_kit::gauge;
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, TextAlign, TextFace, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc};

/// The `genre` of this kit's world file (and the `game.id` default).
pub const GAME_ID: &str = "starter";

/// True for a world file whose `genre` is this kit's.
pub fn is_starter_dump(json: &str) -> bool {
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
// GAME: the tunables. Every field has a default, unknown keys are an error, and
// `validate` keeps values in range, so a typo in JSON never loads silently.

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StarterSpec {
    pub id: String,
    pub title: String,
    /// Seconds to last for a win.
    pub secs: f32,
    /// Half size (m) of the square arena.
    pub half: f32,
    /// Hero speed (m/s) and radius (m).
    pub speed: f32,
    pub radius: f32,
    /// Points for a star, and its radius (m).
    pub star_score: u32,
    pub star_radius: f32,
    /// How many hazards, their speed (m/s) and radius (m).
    pub hazards: u32,
    pub hazard_speed: f32,
    pub hazard_radius: f32,
    /// Thumb stick radius in pixels at 400 px of short side, and its dead zone (0..1).
    pub stick_px: f32,
    pub stick_dead: f32,
    pub seed: u32,
    /// Music and sound cues for the page (file names under the bundle). Passed through.
    pub audio: Option<Value>,
}

impl Default for StarterSpec {
    fn default() -> Self {
        Self {
            id: GAME_ID.into(),
            title: "Starter".into(),
            secs: 60.0,
            half: 7.0,
            speed: 5.0,
            radius: 0.4,
            star_score: 10,
            star_radius: 0.45,
            hazards: 4,
            hazard_speed: 3.2,
            hazard_radius: 0.5,
            stick_px: 70.0,
            stick_dead: 0.12,
            seed: 7,
            audio: Some(default_audio()),
        }
    }
}

impl StarterSpec {
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let spec: Self =
            serde_json::from_value(v.clone()).map_err(|e| format!("starter spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), String> {
        let range = |name: &str, v: f32, lo: f32, hi: f32| {
            if v.is_finite() && (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(format!("starter: {name} must be {lo}..={hi}, got {v}"))
            }
        };
        range("secs", self.secs, 5.0, 600.0)?;
        range("half", self.half, 3.0, 40.0)?;
        range("speed", self.speed, 0.5, 20.0)?;
        range("radius", self.radius, 0.1, 2.0)?;
        range("star_radius", self.star_radius, 0.1, 2.0)?;
        range("hazard_speed", self.hazard_speed, 0.0, 20.0)?;
        range("hazard_radius", self.hazard_radius, 0.1, 2.0)?;
        range("stick_px", self.stick_px, 20.0, 200.0)?;
        range("stick_dead", self.stick_dead, 0.0, 0.6)?;
        if self.hazards > 32 {
            return Err("starter: hazards must be 0..=32".into());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- look ----
// GAME: models and colours. A model that is missing falls back to a primitive.

const HERO_GLB: &str = "assets/models/kenney/mini-characters/character-female-a.glb";
const STAR_GLB: &str = "assets/models/kenney/platformer-kit/jewel.glb";
const HAZARD_GLB: &str = "assets/models/kenney/graveyard-kit/character-ghost.glb";

/// GAME: the music and the sound cues. The sim cues a name (`crate::mixer::cue_sfx`); this
/// maps names to files. Add the folders to the manifest's `bundle_assets`.
fn default_audio() -> Value {
    let sfx = "assets/audio/oga-retro-synth-sfx/";
    let cue =
        |f: &str, v: f64, r: f64| json!({"file": format!("{sfx}{f}"), "volume": v, "rate": r});
    json!({
        "enabled": true,
        "master": 0.8,
        "sfx_volume": 1.0,
        "bgm_volume": 0.5,
        "bgm": {"file": "assets/audio/oga-flowerbed-fields/flowerbed_fields.mp3", "volume": 0.6, "fade_secs": 0.5},
        "sfx": {
            "star": cue("retro_coin_01.wav", 0.4, 1.2),
            "hurt": cue("retro_die_01.wav", 0.7, 1.0),
            "win": cue("power_up_04.wav", 0.8, 1.0),
            "gameover": cue("retro_die_03.wav", 0.8, 1.0),
            "go": {"file": "assets/audio/kenney-interface-sounds/confirmation_001.wav", "volume": 0.7, "rate": 1.0}
        }
    })
}

// ----------------------------------------------------------------- run ----

#[derive(Debug)]
pub struct Starter {
    spec: StarterSpec,
    core: Core,
    flow: GameFlow,
    doc: WorldDoc,
    rng: Rng,
    /// Seconds into the run (and the title's clock).
    t: f32,
    hero: (f32, f32),
    star: (f32, f32),
    /// x, z, vx, vz.
    hazards: Vec<[f32; 4]>,
    win: bool,
    new_best: bool,
    best_before: u32,
    lang_ja: bool,
    /// Keyboard / gamepad direction (screen x right, y down).
    keys: (f32, f32),
    /// The stick thumb: its pointer id, where it is, and the base.
    finger_id: Option<u32>,
    finger: Option<(f32, f32)>,
    base: (f32, f32),
    /// The canvas in pixels, from the last HUD call (the camera fits it).
    size: Cell<(f32, f32)>,
}

impl Starter {
    pub fn new(spec: StarterSpec) -> Self {
        let rng = Rng::new(spec.seed);
        let mut flow = GameFlow::new(spec.id.clone());
        flow.title = spec.title.clone();
        flow.tagline = String::new();
        let mut g = Self {
            spec,
            core: Core::default(),
            flow,
            doc: WorldDoc::default(),
            rng,
            t: 0.0,
            hero: (0.0, 0.0),
            star: (0.0, 0.0),
            hazards: Vec::new(),
            win: false,
            new_best: false,
            best_before: 0,
            lang_ja: false,
            keys: (0.0, 0.0),
            finger_id: None,
            finger: None,
            base: (0.0, 0.0),
            size: Cell::new((390.0, 844.0)),
        };
        g.reset_field();
        g.build_doc();
        g
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(json).map_err(|e| format!("starter dump: {e}"))?;
        let spec = match value.get("game") {
            Some(g) => StarterSpec::from_value(g)?,
            None => StarterSpec::default(),
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

    // ---- flow

    fn reset_field(&mut self) {
        self.rng = Rng::new(self.spec.seed);
        self.t = 0.0;
        self.hero = (0.0, 0.0);
        self.win = false;
        self.new_best = false;
        self.place_star();
        let (n, sp, half) = (self.spec.hazards, self.spec.hazard_speed, self.spec.half);
        self.hazards = (0..n)
            .map(|i| {
                // Start in the corners' half of the arena, never on the hero.
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                [
                    side * half * 0.7,
                    self.rng.range(-half * 0.7, half * 0.7),
                    a.cos() * sp,
                    a.sin() * sp,
                ]
            })
            .collect();
    }

    fn place_star(&mut self) {
        let half = self.spec.half - 1.0;
        loop {
            let s = (self.rng.range(-half, half), self.rng.range(-half, half));
            if (s.0 - self.hero.0).hypot(s.1 - self.hero.1) > 2.5 {
                self.star = s;
                return;
            }
        }
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
            self.finger = None;
            self.finger_id = None;
        }
    }

    fn finish(&mut self, win: bool) {
        self.win = win;
        crate::mixer::stop_bgm(0.6);
        crate::mixer::cue_sfx(if win { "win" } else { "gameover" });
        let score = self.core.stats.score;
        self.new_best = score > self.best_before;
        self.flow.finish(
            score,
            self.core.stats.rings,
            if win { "DAWN" } else { "HIT" },
        );
    }

    // ---- sim

    /// GAME: one fixed step of the rules.
    fn step_sim(&mut self, dt: f32) {
        self.t += dt;
        // Move: stick first, keys otherwise. Screen y down is world +z (the camera looks -z).
        let (w, h) = self.size.get();
        let dir = match self.finger {
            Some(f) => stick::analog(self.base, f, self.stick_radius(w, h), self.spec.stick_dead),
            None => {
                let (x, y) = self.keys;
                let l = x.hypot(y);
                if l > 1.0 {
                    (x / l, y / l)
                } else {
                    (x, y)
                }
            }
        };
        let lim = self.spec.half - self.spec.radius;
        self.hero.0 = (self.hero.0 + dir.0 * self.spec.speed * dt).clamp(-lim, lim);
        self.hero.1 = (self.hero.1 + dir.1 * self.spec.speed * dt).clamp(-lim, lim);
        // Hazards bounce off the walls.
        let hl = self.spec.half - self.spec.hazard_radius;
        for z in &mut self.hazards {
            z[0] += z[2] * dt;
            z[1] += z[3] * dt;
            if z[0].abs() > hl {
                z[0] = z[0].clamp(-hl, hl);
                z[2] = -z[2];
            }
            if z[1].abs() > hl {
                z[1] = z[1].clamp(-hl, hl);
                z[3] = -z[3];
            }
        }
        // Star: touch it for points; the next one appears elsewhere.
        let reach = self.spec.radius + self.spec.star_radius;
        if (self.star.0 - self.hero.0).hypot(self.star.1 - self.hero.1) < reach {
            self.core.stats.score += self.spec.star_score;
            self.core.stats.rings += 1;
            self.core.stats.just_rings += 1;
            crate::mixer::cue_sfx("star");
            self.place_star();
        }
        // A hazard ends the run: the core's hitstop, then `tick_frame` finishes it.
        let hit = self.spec.radius + self.spec.hazard_radius;
        if self
            .hazards
            .iter()
            .any(|z| (z[0] - self.hero.0).hypot(z[1] - self.hero.1) < hit)
        {
            self.core.stats.alive = false;
            self.core.stats.just_crash = true;
            self.core.stats.death = "hazard".into();
            self.core.juice.hitstop = 0.25;
            crate::mixer::stop_bgm(0.6);
            crate::mixer::cue_sfx("hurt");
            self.new_best = self.core.stats.score > self.best_before;
            return;
        }
        if self.t >= self.spec.secs {
            self.finish(true);
        }
    }

    // ---- input

    fn stick_radius(&self, w: f32, h: f32) -> f32 {
        self.spec.stick_px * hud_scale(w, h)
    }

    fn stick_rest(&self, w: f32, h: f32) -> (f32, f32) {
        let r = self.stick_radius(w, h);
        (
            (r * 1.35).max(w * 0.2),
            h - r * 1.45 - 12.0 * hud_scale(w, h),
        )
    }

    pub fn touch_ev(&mut self, ev: PointerEvent, w: u32, h: u32) {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        self.size.set((w, h));
        let r = self.stick_radius(w, h);
        match ev.phase {
            PointerPhase::Begin
                if self.flow.phase == FlowPhase::Play && self.finger_id.is_none() =>
            {
                self.finger_id = Some(ev.id);
                self.finger = Some((ev.x, ev.y));
                self.base = stick::land(ev.x, ev.y, r, 4.0 * hud_scale(w, h), w, h);
            }
            PointerPhase::Move if self.finger_id == Some(ev.id) => {
                self.finger = Some((ev.x, ev.y));
                self.base = stick::follow(self.base, (ev.x, ev.y), r);
            }
            PointerPhase::End | PointerPhase::Cancel if self.finger_id == Some(ev.id) => {
                self.finger_id = None;
                self.finger = None;
            }
            _ => {}
        }
    }

    // ---- picture

    /// GAME: the 3D world. Built once per frame from the state (cheap: a few props).
    fn build_doc(&mut self) {
        let half = self.spec.half;
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: half + 4.0,
            ground: Some(false),
            sky: Some([10, 12, 30, 255]),
            hemi_sky: Some([0.25, 0.28, 0.5]),
            hemi_ground: Some([0.12, 0.1, 0.18]),
            bloom: Some(0.4),
            vignette: Some(0.4),
            fxaa: Some(true),
            tonemap: Some(true),
            ..WorldDoc::default()
        };
        let side = half * 2.0;
        doc.props.push(prop(
            "floor",
            "box",
            "floor",
            [0.0, -0.1, 0.0],
            [side, 0.2, side],
            [52, 60, 96],
            0.0,
            0.9,
            0.0,
        ));
        let model =
            |id: &str, path: &str, name: &str, pos: [f32; 3], s: f32, col: [u32; 3], glow: f32| {
                if glb_ready(path) {
                    glb_prop(id, path, name, pos, [s; 3], [255, 255, 255], glow, None)
                } else {
                    prop(
                        id,
                        "sphere",
                        name,
                        [pos[0], pos[1] + s * 0.4, pos[2]],
                        [s * 0.8; 3],
                        col,
                        glow,
                        0.6,
                        0.0,
                    )
                }
            };
        let bob = (self.t * 4.0).sin() * 0.08;
        doc.props.push(model(
            "hero",
            HERO_GLB,
            "hero",
            [self.hero.0, 0.0, self.hero.1],
            1.4,
            [255, 210, 120],
            0.0,
        ));
        doc.props.push(model(
            "star",
            STAR_GLB,
            "star",
            [self.star.0, 0.2 + bob, self.star.1],
            1.6,
            [255, 220, 90],
            0.8,
        ));
        for (i, z) in self.hazards.iter().enumerate() {
            doc.props.push(model(
                &format!("hazard-{i}"),
                HAZARD_GLB,
                "hazard",
                [z[0], 0.15, z[1]],
                1.5,
                [200, 200, 255],
                0.1,
            ));
        }
        let view = self.view();
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "arena".into(),
            position: view.eye,
            target: view.target,
            fov: view.fov,
            roll: 0.0,
        });
        doc.lights.push(light(
            0,
            "key",
            [0.0, 9.0, 3.0],
            [0.0; 3],
            [0.85, 0.9, 1.0],
            1.2,
            30.0,
            false,
        ));
        self.doc = doc;
    }

    /// GAME: the camera. It fits the whole arena on the canvas, portrait or landscape.
    fn view(&self) -> View {
        let half = self.spec.half;
        let (w, h) = self.size.get();
        let m = half + 0.3;
        View::frame(
            [0.0, 0.0, 0.0],
            [0.0, 2.1, 1.25],
            45.0,
            &View::ground_box(-m, m, -m, m, 0.0),
            w / h.max(1.0),
        )
    }

    /// GAME: the in-play HUD (the page draws the title and result cards).
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
                [255, 240, 210, 255],
            )
            .aligned(TextAlign::Center)
            .with_face(TextFace::Display),
        );
        gauge(
            &mut list,
            w * 0.5 - 90.0 * s,
            50.0 * s,
            180.0 * s,
            8.0 * s,
            1.0 - self.t / self.spec.secs,
            [255, 210, 120, 255],
            [10, 10, 30, 170],
        );
        list.texts.push(TextQuad::new(
            &format!("★ {}", self.core.stats.score),
            12.0 * s,
            14.0 * s,
            20.0 * s,
            [255, 230, 150, 255],
        ));
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
            "stars": self.core.stats.rings,
            "win": self.win,
            "death": self.core.stats.death,
            "death_text": if self.core.stats.death.is_empty() { "" } else { self.tr("A hazard got you", "ぶつかった") },
            "result": if self.flow.phase == FlowPhase::Result {
                json!({
                    "key": format!("{}:{}", self.spec.id, self.spec.seed),
                    "title": self.spec.title,
                    "enabled": true,
                    "top_n": 5,
                    "columns": ["stars", "score"],
                    "sort": "score",
                    "run": {"score": self.core.stats.score, "stars": self.core.stats.rings, "win": u32::from(self.win)},
                    "can_revive": false,
                    "new_best": self.new_best,
                })
            } else {
                Value::Null
            },
            "pos": [self.hero.0, self.hero.1],
        })
    }
}

fn hud_scale(w: f32, h: f32) -> f32 {
    (w.min(h) / 400.0).clamp(0.9, 2.6)
}

// ------------------------------------------------------------ the host ----

impl Kit for Starter {
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

impl crate::kit_host::KitHost for Starter {
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
        Starter::confirm(self)
    }
    fn show_title(&mut self) {
        Starter::show_title(self)
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
        Starter::hud(self, width, height, menu)
    }
    fn walk(&mut self, lx: f32, lz: f32, _jump: bool) {
        // Keys and the gamepad: +lz is up the screen, which is -z.
        self.keys = (lx, -lz);
    }
    fn touch(&mut self, ev: PointerEvent, w: u32, h: u32, _t: f32) {
        self.touch_ev(ev, w, h);
    }
    fn uses_swipe(&self) -> bool {
        // The kit reads the pointer itself (the stick): the page hides its own pads.
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

    fn started() -> Starter {
        let mut g = Starter::new(StarterSpec::default());
        g.confirm();
        g
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

    #[test]
    fn the_default_spec_is_valid_and_strict() {
        StarterSpec::default().validate().unwrap();
        let mut v = Starter::new(StarterSpec::default()).world_value()["game"].clone();
        v["bogus"] = json!(1);
        assert!(
            StarterSpec::from_value(&v).is_err(),
            "unknown keys are an error"
        );
        let bad = StarterSpec {
            secs: 0.0,
            ..StarterSpec::default()
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn title_play_and_a_star_scores() {
        let mut g = Starter::new(StarterSpec {
            hazards: 0,
            ..StarterSpec::default()
        });
        assert_eq!(g.phase_name(), "title");
        g.confirm();
        assert_eq!(g.phase_name(), "play");
        g.hero = g.star;
        g.tick(DT);
        assert_eq!(g.core.stats.score, 10);
        assert_eq!(g.state_value()["stars"], 1);
    }

    #[test]
    fn a_hazard_ends_the_run_and_lasting_wins() {
        let mut g = started();
        g.hazards[0] = [g.hero.0, g.hero.1, 0.0, 0.0];
        for _ in 0..60 {
            g.tick(DT);
        }
        assert_eq!(g.phase_name(), "result");
        assert_eq!(g.state_value()["death"], "hazard");
        assert!(g.state_value()["result"].is_object());

        let mut g = Starter::new(StarterSpec {
            hazards: 0,
            secs: 5.0,
            ..StarterSpec::default()
        });
        g.confirm();
        for _ in 0..(6 * 60) {
            g.tick(DT);
        }
        assert_eq!(g.phase_name(), "result");
        assert_eq!(g.state_value()["win"], true);
    }

    #[test]
    fn the_stick_and_the_keys_move_the_hero() {
        let mut g = Starter::new(StarterSpec {
            hazards: 0,
            ..StarterSpec::default()
        });
        g.confirm();
        g.touch(ev(1, PointerPhase::Begin, 100.0, 600.0), 390, 844, 0.0);
        g.touch(ev(1, PointerPhase::Move, 200.0, 600.0), 390, 844, 0.0);
        for _ in 0..30 {
            g.tick(DT);
        }
        assert!(g.hero.0 > 1.0, "right on the screen is +x: {:?}", g.hero);
        g.touch(ev(1, PointerPhase::End, 200.0, 600.0), 390, 844, 0.0);
        g.walk(0.0, 1.0, false);
        let z0 = g.hero.1;
        for _ in 0..30 {
            g.tick(DT);
        }
        assert!(g.hero.1 < z0 - 1.0, "W is up the screen, -z");
        let hud = g.hud(390, 844, false);
        assert!(
            hud.texts.iter().any(|t| t.face == TextFace::Display),
            "the clock"
        );
        assert!(
            hud.quads.iter().any(|q| q.radius > 0.0),
            "gauge and stick are rounded"
        );
    }

    #[test]
    fn the_whole_arena_is_on_screen_either_way_up() {
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
            let half = g.spec.half;
            for (x, z) in [(-half, -half), (half, -half), (-half, half), (half, half)] {
                let (sx, sy) = view.project([x, 0.0, z], w as f32, h as f32).unwrap();
                assert!(
                    (0.0..=w as f32).contains(&sx) && (0.0..=h as f32).contains(&sy),
                    "{w}x{h}: corner ({x}, {z}) at ({sx}, {sy})"
                );
            }
        }
    }

    #[test]
    fn pause_freezes_the_clock() {
        let mut g = started();
        g.hazards.clear();
        g.tick(DT);
        g.pause_run();
        let t = g.t;
        for _ in 0..30 {
            g.tick(DT);
        }
        assert_eq!(g.t, t);
        g.resume_run();
        g.tick(DT);
        assert!(g.t > t);
    }

    #[test]
    fn the_baked_fixture_is_the_default_world() {
        let want = Starter::new(StarterSpec::default()).world_value();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/starter_world.json"
        );
        if std::env::var("KAGRA_BAKE").is_ok() {
            std::fs::write(path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
        }
        let got: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default())
            .unwrap_or(Value::Null);
        assert_eq!(
            got, want,
            "run `KAGRA_BAKE=1 cargo test -p kagra-shared --lib starter::` to refresh it"
        );
        assert!(is_starter_dump(&got.to_string()));
        let loaded = crate::kit_host::load(&got.to_string()).unwrap().unwrap();
        assert_eq!(loaded.name(), GAME_ID, "the session finds this kit");
    }
}
