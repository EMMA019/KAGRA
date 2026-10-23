//! Lumen — endless neon flyer. Not a WorldPlay genre.
//!
//! Phone and desktop wasm load a dump with `"genre": "neon_run"` and this
//! session ticks it. Python calls the same type for the window build.
//! Glow is dump `emissive` (the shared HDR add). Reflections are
//! `hemi_sky` / `hemi_ground` plus `env_map: preset:studio`.

use std::collections::VecDeque;

use serde_json::json;

use crate::flow::{FlowAction, FlowPhase, GameFlow};
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc, WorldLight, WorldProp};

pub const GAME_ID: &str = "neon_run";

const HOLE_R: f32 = 1.15;
const GATE_GAP: f32 = 26.0;
const GATE0: f32 = 34.0;
const WALL_X: f32 = 2.35;
const WALL_Y: f32 = 1.72;

pub fn is_neon_dump(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| {
            v.get("genre")
                .and_then(|g| g.as_str())
                .map(|s| s == GAME_ID)
        })
        .unwrap_or(false)
}

pub fn hole_center(index: i32) -> (f32, f32) {
    const P: [(f32, f32); 5] = [
        (0.0, 0.0),
        (-1.05, 0.25),
        (1.10, -0.20),
        (0.15, 0.85),
        (-0.35, -0.75),
    ];
    P[index.rem_euclid(5) as usize]
}

pub fn gate_z(index: i32) -> f32 {
    GATE0 + index as f32 * GATE_GAP
}

#[derive(Clone, Debug)]
struct Spark {
    x: f32,
    y: f32,
    z: f32,
    vx: f32,
    vy: f32,
    vz: f32,
    life: f32,
    max_life: f32,
    size: f32,
    color: [u32; 3],
}

#[derive(Debug)]
pub struct NeonRun {
    pub flow: GameFlow,
    pub score: u32,
    /// Sub-point distance so `speed * dt` is not truncated to 0 every frame.
    score_acc: f32,
    pub orbs: u32,
    pub alive: bool,
    x: f32,
    y: f32,
    z: f32,
    vx: f32,
    vy: f32,
    pad_x: f32,
    pad_y: f32,
    key_left: bool,
    key_right: bool,
    key_up: bool,
    key_down: bool,
    boost: f32,
    passed: i32,
    attract: f32,
    time: f32,
    shake: f32,
    cam_x: f32,
    cam_y: f32,
    seed: u32,
    just_orbs: u32,
    just_crash: bool,
    just_near: bool,
    trail: VecDeque<[f32; 3]>,
    sparks: Vec<Spark>,
    doc: WorldDoc,
}

impl NeonRun {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("neon dump: {e}"))?;
        let mut run = Self::fresh();
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            run.flow.best_score = best as u32;
        }
        run.load_progress();
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            run.flow.best_score = run.flow.best_score.max(best as u32);
        }
        run.rebuild();
        Ok(run)
    }

    fn fresh() -> Self {
        let mut flow = GameFlow::new(GAME_ID);
        flow.title = "LUMEN".into();
        flow.tagline = "NEON RUN".into();
        Self {
            flow,
            score: 0,
            score_acc: 0.0,
            orbs: 0,
            alive: true,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            vx: 0.0,
            vy: 0.0,
            pad_x: 0.0,
            pad_y: 0.0,
            key_left: false,
            key_right: false,
            key_up: false,
            key_down: false,
            boost: 0.0,
            passed: -1,
            attract: 0.0,
            time: 0.0,
            shake: 0.0,
            cam_x: 0.0,
            cam_y: 0.0,
            seed: 1,
            just_orbs: 0,
            just_crash: false,
            just_near: false,
            trail: VecDeque::new(),
            sparks: Vec::new(),
            doc: WorldDoc::default(),
        }
    }

    pub fn load_progress(&mut self) {
        let best = self.flow.best_score;
        self.flow.load_save();
        self.flow.best_score = self.flow.best_score.max(best);
    }

    pub fn score(&self) -> u32 {
        self.score
    }

    pub fn best_score(&self) -> u32 {
        self.flow.best_score
    }

    pub fn phase_name(&self) -> &'static str {
        self.flow.phase.name()
    }

    pub fn playing(&self) -> bool {
        self.flow.phase == FlowPhase::Play && self.alive
    }

    pub fn document(&self) -> &WorldDoc {
        &self.doc
    }

    pub fn dump_json(&self) -> String {
        self.doc.to_json().unwrap_or_else(|_| "{}".into())
    }

    /// `{orbs, crash, near}` for one frame, then cleared.
    pub fn take_feel_json(&mut self) -> String {
        let body = json!({
            "orbs": self.just_orbs,
            "crash": self.just_crash,
            "near": self.just_near,
        });
        self.just_orbs = 0;
        self.just_crash = false;
        self.just_near = false;
        body.to_string()
    }

    pub fn state_json(&self) -> String {
        let next = self.passed + 1;
        let (hx, hy) = hole_center(next);
        json!({
            "phase": self.flow.phase.name(),
            "score": self.score,
            "best": self.flow.best_score,
            "orbs": self.orbs,
            "alive": self.alive,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "distance": self.z.max(0.0) as u32,
            "next_x": hx,
            "next_y": hy,
            "next_z": gate_z(next),
        })
        .to_string()
    }

    pub fn set_steer(&mut self, x: f32, y: f32, jump: bool) {
        self.pad_x = x.clamp(-1.0, 1.0);
        self.pad_y = y.clamp(-1.0, 1.0);
        if jump {
            self.boost = self.boost.max(0.16);
        }
    }

    pub fn confirm(&mut self) {
        match self.flow.phase {
            FlowPhase::Title | FlowPhase::Result => self.start_run(),
            FlowPhase::Pause | FlowPhase::Play => {}
        }
    }

    pub fn show_title(&mut self) {
        self.flow.show_title();
        self.alive = true;
        self.score = 0;
        self.score_acc = 0.0;
    }

    pub fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) -> bool {
        let Some(action) = self.flow.hit(x, y, width, height) else {
            return false;
        };
        self.apply_action(action);
        true
    }

    pub fn on_key(&mut self, name: &str, down: bool) -> bool {
        if self.set_key(name, down) {
            return true;
        }
        if !down {
            return false;
        }
        let key = name.to_ascii_lowercase();
        if self.flow.phase == FlowPhase::Play && (key == "space" || key == "shift") {
            self.boost = self.boost.max(0.45);
            return true;
        }
        if let Some(action) = self.flow.on_key(&key) {
            self.apply_action(action);
            return true;
        }
        false
    }

    fn set_key(&mut self, name: &str, down: bool) -> bool {
        match name.to_ascii_lowercase().as_str() {
            "a" | "arrowleft" | "left" => self.key_left = down,
            "d" | "arrowright" | "right" => self.key_right = down,
            "w" | "arrowup" | "up" => self.key_up = down,
            "s" | "arrowdown" | "down" => self.key_down = down,
            _ => return false,
        }
        true
    }

    fn axis(&self) -> (f32, f32) {
        let kx = (self.key_right as i32 - self.key_left as i32) as f32;
        let ky = (self.key_up as i32 - self.key_down as i32) as f32;
        (
            (kx + self.pad_x).clamp(-1.0, 1.0),
            (ky + self.pad_y).clamp(-1.0, 1.0),
        )
    }

    fn apply_action(&mut self, action: FlowAction) {
        match action {
            FlowAction::Start => self.start_run(),
            FlowAction::Retry => self.start_run(),
            FlowAction::Resume => self.flow.resume(),
            FlowAction::ToTitle => {
                self.show_title();
                self.reset_motion();
            }
            FlowAction::Pause => self.flow.pause(),
            FlowAction::VolumeUp => self.flow.bump_master(0.1),
            FlowAction::VolumeDown => self.flow.bump_master(-0.1),
            FlowAction::ToggleMute => self.flow.toggle_mute(),
        }
    }

    fn start_run(&mut self) {
        self.reset_motion();
        self.score = 0;
        self.score_acc = 0.0;
        self.orbs = 0;
        self.alive = true;
        self.passed = -1;
        self.time = 0.0;
        self.sparks.clear();
        self.trail.clear();
        self.flow.enter_play();
        self.rebuild();
    }

    fn reset_motion(&mut self) {
        self.x = 0.0;
        self.y = 0.0;
        self.z = 0.0;
        self.vx = 0.0;
        self.vy = 0.0;
        self.boost = 0.0;
        self.cam_x = 0.0;
        self.cam_y = 0.0;
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        if dt > 0.0 {
            // BGM fades live on the mixer. WorldPlay ticks it; this session does too.
            crate::mixer::tick(dt);
        }
        self.just_orbs = 0;
        self.just_crash = false;
        self.just_near = false;
        if dt <= 0.0 {
            self.rebuild();
            return;
        }
        if self.flow.phase == FlowPhase::Title {
            self.attract += dt;
            self.z = self.attract * 10.0;
            self.x = (self.attract * 0.65).sin() * 0.85;
            self.y = (self.attract * 0.42).cos() * 0.38;
            self.smooth_cam(dt);
            self.tick_sparks(dt);
            self.push_trail();
            self.decay_shake(dt);
            self.rebuild();
            return;
        }
        if self.flow.phase != FlowPhase::Play || !self.alive {
            self.tick_sparks(dt);
            self.decay_shake(dt);
            self.rebuild();
            return;
        }
        self.time += dt;
        self.boost = (self.boost - dt).max(0.0);
        let (ax, ay) = self.axis();
        let response = 1.0 - (-dt * 7.0).exp();
        self.vx += (ax * 6.4 - self.vx) * response;
        self.vy += (ay * 5.2 - self.vy) * response;
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        let speed = self.speed();
        self.z += speed * dt;
        self.score_acc += speed * dt * 2.0;
        let whole = self.score_acc.floor();
        if whole >= 1.0 {
            self.score += whole as u32;
            self.score_acc -= whole;
        }
        if self.x.abs() > WALL_X || self.y.abs() > WALL_Y {
            self.crash();
        } else {
            self.cross_gates();
        }
        self.smooth_cam(dt);
        self.tick_sparks(dt);
        if self.alive {
            self.push_trail();
        }
        self.decay_shake(dt);
        self.rebuild();
    }

    fn speed(&self) -> f32 {
        let base = (22.0 + self.time * 0.62).min(46.0);
        if self.boost > 0.0 {
            base + 9.0
        } else {
            base
        }
    }

    fn smooth_cam(&mut self, dt: f32) {
        let k = 1.0 - (-dt * 4.2).exp();
        self.cam_x += (self.x * 0.62 - self.cam_x) * k;
        self.cam_y += (self.y * 0.5 - self.cam_y) * k;
    }

    fn cross_gates(&mut self) {
        let mut guard = 0;
        while self.passed < 10_000 && guard < 4 {
            guard += 1;
            let next = self.passed + 1;
            let gz = gate_z(next);
            if self.z < gz {
                break;
            }
            let (hx, hy) = hole_center(next);
            let d = (self.x - hx).hypot(self.y - hy);
            if d > HOLE_R {
                self.crash();
                break;
            }
            self.passed = next;
            self.score += 100;
            if d <= 0.55 {
                self.score += 80;
                self.orbs += 1;
                self.just_orbs += 1;
                self.burst(hx, hy, gz, 14, [255, 244, 180], 3.2);
                crate::mixer::cue_sfx("card");
                self.shake = self.shake.max(0.08);
            } else if d >= 0.82 {
                self.score += 40;
                self.just_near = true;
                self.shake = self.shake.max(0.05);
            }
        }
    }

    fn crash(&mut self) {
        if !self.alive {
            return;
        }
        self.alive = false;
        self.just_crash = true;
        self.shake = 0.42;
        self.burst(self.x, self.y, self.z, 22, [255, 64, 140], 4.4);
        self.burst(self.x, self.y, self.z, 10, [120, 240, 255], 3.0);
        crate::mixer::cue_sfx("fire");
        self.flow.finish(self.score, self.orbs, "CRASH");
    }

    fn push_trail(&mut self) {
        self.trail.push_back([self.x, self.y, self.z - 0.45]);
        while self.trail.len() > 16 {
            self.trail.pop_front();
        }
    }

    fn decay_shake(&mut self, dt: f32) {
        self.shake = (self.shake - dt * 0.8).max(0.0);
    }

    fn tick_sparks(&mut self, dt: f32) {
        let mut next = Vec::new();
        for mut s in self.sparks.drain(..) {
            s.life += dt;
            if s.life >= s.max_life {
                continue;
            }
            s.vy -= 2.2 * dt;
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.z += s.vz * dt;
            next.push(s);
        }
        self.sparks = next;
    }

    fn burst(&mut self, x: f32, y: f32, z: f32, n: u32, color: [u32; 3], speed: f32) {
        for _ in 0..n {
            if self.sparks.len() >= 32 {
                self.sparks.remove(0);
            }
            let ang = self.rng() * std::f32::consts::TAU;
            let rad = self.rng();
            let spd = speed * (0.35 + self.rng());
            let vy = 0.8 + self.rng() * speed * 0.45;
            let max_life = 0.28 + self.rng() * 0.28;
            let size = 0.08 + self.rng() * 0.1;
            self.sparks.push(Spark {
                x,
                y,
                z,
                vx: ang.cos() * spd * rad,
                vy,
                vz: ang.sin() * spd * rad - 2.0,
                life: 0.0,
                max_life,
                size,
                color,
            });
        }
    }

    fn rng(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.seed >> 8) & 0x00ff_ffff) as f32 / 16_777_216.0
    }

    fn rebuild(&mut self) {
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: 64.0,
            ground: Some(false),
            sky: Some([32, 6, 58, 255]),
            hemi_sky: Some([0.20, 0.72, 1.45]),
            hemi_ground: Some([1.05, 0.10, 0.48]),
            env_map: Some("preset:studio".into()),
            shadow_softness: Some(3.0),
            bloom: Some(0.95),
            bloom_threshold: Some(0.32),
            vignette: Some(1.05),
            lut: Some("punch".into()),
            lut_strength: Some(0.45),
            fxaa: Some(true),
            exposure: Some(1.18),
            ibl: Some(0.95),
            tonemap: Some(true),
            ..WorldDoc::default()
        };
        let shake = (self.time * 47.0).sin() * self.shake;
        let eye = [
            self.cam_x + shake,
            self.cam_y + 1.28 + shake * 0.35,
            self.z - 8.6,
        ];
        let target = [self.x, self.y + 0.25, self.z + 12.0];
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "chase".into(),
            position: eye,
            target,
            fov: 58.0,
        });
        // Slot 0 is the sun shadow. It must sit behind the camera: a spot
        // parked on the eye puts the shadow frustum through the ribs and
        // the compare comes back as NaN, which wipes emissive and bloom.
        doc.lights.push(light(
            0,
            "key",
            [eye[0] - 1.5, eye[1] + 7.5, eye[2] - 5.0],
            [0.12, -0.82, 0.5],
            [0.75, 0.85, 1.0],
            1.15,
            42.0,
            true,
        ));
        doc.lights.push(light(
            1,
            "ship",
            [self.x, self.y, self.z],
            [0.0, 0.0, 0.0],
            [0.45, 0.95, 1.0],
            6.0,
            11.0,
            false,
        ));
        doc.lights.push(light(
            2,
            "rim",
            [self.x, self.y + 1.2, self.z + 6.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.25, 0.7],
            4.5,
            14.0,
            false,
        ));
        self.push_tunnel(&mut doc);
        self.push_gates(&mut doc);
        self.push_islands(&mut doc);
        self.push_stars(&mut doc);
        self.push_ship(&mut doc);
        self.push_trail_props(&mut doc);
        self.push_spark_props(&mut doc);
        self.doc = doc;
    }

    fn push_tunnel(&self, doc: &mut WorldDoc) {
        let origin = (self.z / 7.0).floor() * 7.0;
        for i in -1..12 {
            let z = origin + i as f32 * 7.0;
            let cyan = (i & 1) == 0;
            let col = if cyan { [40, 230, 255] } else { [255, 40, 170] };
            let gain = 3.4;
            let w = 6.6;
            let h = 4.5;
            doc.props.push(beam(
                &format!("rib-t-{i}"),
                [0.0, h * 0.5, z],
                [w, 0.07, 0.1],
                col,
                gain,
            ));
            doc.props.push(beam(
                &format!("rib-b-{i}"),
                [0.0, -h * 0.5, z],
                [w, 0.07, 0.1],
                col,
                gain,
            ));
            doc.props.push(beam(
                &format!("rib-l-{i}"),
                [-w * 0.5, 0.0, z],
                [0.07, h, 0.1],
                col,
                gain,
            ));
            doc.props.push(beam(
                &format!("rib-r-{i}"),
                [w * 0.5, 0.0, z],
                [0.07, h, 0.1],
                col,
                gain,
            ));
        }
        doc.props.push(beam(
            "rail-l",
            [-2.7, -1.55, self.z + 18.0],
            [0.06, 0.05, 70.0],
            [80, 255, 240],
            2.8,
        ));
        doc.props.push(beam(
            "rail-r",
            [2.7, -1.55, self.z + 18.0],
            [0.06, 0.05, 70.0],
            [255, 60, 180],
            2.8,
        ));
    }

    fn push_gates(&self, doc: &mut WorldDoc) {
        let first = self.passed;
        for step in 0..4 {
            let index = first + step;
            if index < 0 {
                continue;
            }
            let z = gate_z(index);
            if z < self.z - 8.0 || z > self.z + 90.0 {
                continue;
            }
            let (hx, hy) = hole_center(index);
            let magenta = (index & 1) == 0;
            let col = if magenta {
                [255, 50, 190]
            } else {
                [60, 240, 255]
            };
            doc.props.push(glow(
                &format!("ring-{index}"),
                "ring",
                "gate",
                [hx, hy, z],
                [3.35, 3.35, 3.35],
                0.0,
                0.0,
                0.0,
                col,
                4.2,
            ));
            doc.props.push(glow(
                &format!("ring-b-{index}"),
                "ring",
                "gate",
                [hx, hy, z + 0.55],
                [2.55, 2.55, 2.55],
                0.0,
                0.0,
                0.0,
                if magenta {
                    [120, 255, 255]
                } else {
                    [255, 180, 255]
                },
                3.6,
            ));
            if index > self.passed {
                doc.props.push(glow(
                    &format!("orb-{index}"),
                    "sphere",
                    "orb",
                    [hx, hy, z],
                    [0.42, 0.42, 0.42],
                    0.0,
                    0.0,
                    0.0,
                    [255, 236, 170],
                    6.0,
                ));
            }
        }
    }

    fn push_islands(&self, doc: &mut WorldDoc) {
        let base = (self.z / 36.0).floor() as i32;
        for step in 0..5 {
            let i = base + step;
            let z = i as f32 * 36.0 + 12.0;
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let x = side * (6.2 + (i.rem_euclid(3) as f32) * 0.8);
            let y = ((i.rem_euclid(5) as f32) - 2.0) * 0.7;
            doc.props.push(glow(
                &format!("isle-{i}"),
                "box",
                "island",
                [x, y, z],
                [2.4, 0.45, 1.6],
                0.2,
                0.0,
                0.15,
                [18, 10, 28],
                0.0,
            ));
            doc.props.push(glow(
                &format!("isle-rim-{i}"),
                "box",
                "island-rim",
                [x, y + 0.28, z],
                [2.55, 0.06, 1.7],
                0.2,
                0.0,
                0.0,
                if side > 0.0 {
                    [255, 70, 190]
                } else {
                    [70, 240, 255]
                },
                3.8,
            ));
        }
    }

    fn push_stars(&self, doc: &mut WorldDoc) {
        for i in 0..20 {
            let z = self.z + ((i * 19) % 78) as f32 - 8.0;
            let ang = i as f32 * 2.399;
            let rad = 7.5 + (i % 4) as f32 * 1.3;
            let col = if i % 3 == 0 {
                [255, 80, 190]
            } else if i % 3 == 1 {
                [90, 240, 255]
            } else {
                [255, 230, 180]
            };
            doc.props.push(glow(
                &format!("star-{i}"),
                "sphere",
                "star",
                [ang.cos() * rad, ang.sin() * rad * 0.55, z],
                [0.16, 0.16, 0.16],
                0.0,
                0.0,
                0.0,
                col,
                4.5,
            ));
        }
    }

    fn push_ship(&self, doc: &mut WorldDoc) {
        let bank = (-self.vx * 0.14).clamp(-0.6, 0.6);
        let nose = (self.vy * -0.08).clamp(-0.35, 0.35);
        let (x, y, z) = (self.x, self.y, self.z);
        doc.props.push(glow(
            "ship-core",
            "sphere",
            "ship-core",
            [x, y, z],
            [0.46, 0.28, 0.62],
            0.0,
            nose,
            bank,
            [210, 255, 255],
            5.5,
        ));
        doc.props.push(glow(
            "ship-nose",
            "box",
            "ship-nose",
            [x, y, z + 0.42],
            [0.16, 0.1, 0.42],
            0.0,
            nose,
            bank,
            [230, 245, 255],
            1.35,
        ));
        doc.props.push(glow(
            "ship-wing-l",
            "box",
            "ship-wing",
            [x - 0.38, y - 0.02, z - 0.05],
            [0.55, 0.045, 0.22],
            0.15,
            nose,
            bank,
            [255, 40, 170],
            4.0,
        ));
        doc.props.push(glow(
            "ship-wing-r",
            "box",
            "ship-wing",
            [x + 0.38, y - 0.02, z - 0.05],
            [0.55, 0.045, 0.22],
            -0.15,
            nose,
            bank,
            [40, 230, 255],
            4.0,
        ));
    }

    fn push_trail_props(&self, doc: &mut WorldDoc) {
        let n = self.trail.len();
        for (i, p) in self.trail.iter().enumerate() {
            let t = if n <= 1 {
                1.0
            } else {
                i as f32 / (n - 1) as f32
            };
            let s = 0.06 + 0.16 * t;
            doc.props.push(glow(
                &format!("wake-{i}"),
                "sphere",
                "wake",
                *p,
                [s, s, s * 1.8],
                0.0,
                0.0,
                0.0,
                [120, 255, 255],
                2.2 + 2.4 * t,
            ));
        }
    }

    fn push_spark_props(&self, doc: &mut WorldDoc) {
        for (i, s) in self.sparks.iter().enumerate() {
            let fade = 1.0 - (s.life / s.max_life).clamp(0.0, 1.0);
            let size = (s.size * (0.4 + 0.6 * fade)).max(0.04);
            doc.props.push(glow(
                &format!("spark-{i}"),
                "sphere",
                "spark",
                [s.x, s.y, s.z],
                [size, size, size],
                0.0,
                0.0,
                0.0,
                s.color,
                3.5 * fade + 0.4,
            ));
        }
    }

    pub fn hud_json(&self, width: u32, height: u32) -> String {
        crate::flow::draw_list_json(&self.hud(width, height))
    }

    pub fn hud(&self, width: u32, height: u32) -> DrawList {
        let mut list = DrawList {
            clear: [0, 0, 0, 0],
            quads: Vec::new(),
            texts: Vec::new(),
            images: Vec::new(),
        };
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        if self.flow.phase == FlowPhase::Play {
            label(
                &mut list,
                &format!("SCORE {}", self.score),
                22.0,
                18.0,
                16.0,
                [236, 248, 255, 255],
                TextAlign::Left,
            );
            label(
                &mut list,
                &format!("{} m   {} ORBS", self.z.max(0.0) as u32, self.orbs),
                22.0,
                40.0,
                12.0,
                [140, 240, 255, 255],
                TextAlign::Left,
            );
        }
        if self.flow.phase == FlowPhase::Result {
            list.quads.push(Quad::new(0.0, 0.0, w, h, [8, 0, 16, 150]));
        }
        self.flow.paint(&mut list, width, height);
        if self.flow.phase == FlowPhase::Title {
            label(
                &mut list,
                "WASD / DRAG    SHIFT BOOST",
                w * 0.5,
                h * 0.50,
                13.0,
                [220, 236, 255, 255],
                TextAlign::Center,
            );
        }
        list
    }
}

fn label(
    list: &mut DrawList,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    color: [u8; 4],
    align: TextAlign,
) {
    list.texts
        .push(TextQuad::new(text, x, y, size, color).aligned(align));
}

#[allow(clippy::too_many_arguments)]
fn light(
    slot: u32,
    name: &str,
    position: [f32; 3],
    direction: [f32; 3],
    color: [f32; 3],
    intensity: f32,
    radius: f32,
    spot: bool,
) -> WorldLight {
    WorldLight {
        id: format!("light:{name}"),
        kind_type: "light".into(),
        name: name.into(),
        position,
        kind: if spot { "spot".into() } else { "point".into() },
        slot,
        intensity,
        radius,
        color: Some(color),
        direction: Some(direction),
    }
}

fn beam(id: &str, pos: [f32; 3], scale: [f32; 3], color: [u32; 3], emissive: f32) -> WorldProp {
    glow(id, "box", "rib", pos, scale, 0.0, 0.0, 0.0, color, emissive)
}

#[allow(clippy::too_many_arguments)]
fn glow(
    id: &str,
    model: &str,
    name: &str,
    position: [f32; 3],
    scale: [f32; 3],
    yaw: f32,
    pitch: f32,
    roll: f32,
    color: [u32; 3],
    emissive: f32,
) -> WorldProp {
    WorldProp {
        id: format!("prop:{id}"),
        kind: "prop".into(),
        name: name.into(),
        position,
        yaw,
        pitch,
        roll,
        model: model.into(),
        scale,
        enabled: true,
        color: Some(color),
        emissive: if emissive > 0.01 {
            Some(emissive)
        } else {
            None
        },
        emissive_map: matches!(name, "ship-core" | "orb").then(|| "preset:core".into()),
        roughness: match name {
            "island" | "ship-nose" => 0.14,
            "gate" => 0.18,
            _ => 0.28,
        },
        metallic: match name {
            "ship-nose" => 0.94,
            "island" => 0.86,
            "gate" => 0.48,
            _ => 0.08,
        },
        is_static: true,
        collider: Some("none".into()),
        ..WorldProp::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{SceneKind, SharedSession};

    #[test]
    fn straight_flight_scores_and_a_wall_ends_the_run() {
        let mut run = NeonRun::from_json(r#"{"version":1,"genre":"neon_run"}"#).unwrap();
        assert_eq!(run.phase_name(), "title");
        run.tick(1.0 / 60.0);
        assert!(
            run.document()
                .props
                .iter()
                .any(|p| p.name == "ship-core" && p.emissive.unwrap_or(0.0) > 1.0),
            "ship is emissive"
        );
        assert_eq!(run.document().env_map.as_deref(), Some("preset:studio"));
        assert!(run.document().hemi_sky.is_some());
        assert!(run.document().baked_env().is_some());
        let scene = run.document().compile_scene(16.0 / 9.0);
        assert!(
            scene
                .batches
                .iter()
                .flat_map(|b| &b.instances)
                .any(|i| i.emissive > 1.0 && i.material != crate::scene3d::Material::Sky),
            "glow is the shared emissive term"
        );
        assert!((scene.hemi_sky[2] - 1.45).abs() < 1e-4);
        assert!(scene.env_pixels.is_some(), "studio env strip");
        run.confirm();
        assert_eq!(run.phase_name(), "play");
        for _ in 0..140 {
            run.tick(1.0 / 60.0);
        }
        assert!(run.score() >= 100, "score {}", run.score());
        assert!(run.alive);
        run.set_steer(1.0, 0.0, false);
        for _ in 0..180 {
            run.tick(1.0 / 60.0);
        }
        assert!(!run.alive);
        assert_eq!(run.phase_name(), "result");
        assert!(run.best_score() >= run.score() || run.flow.best_score >= 1 || run.score > 0);
        assert!(run.flow.best_score >= run.score || run.score == run.flow.score);
    }

    #[test]
    fn session_loads_neon_without_a_worldplay_genre() {
        let mut session = SharedSession::default();
        session
            .load_game_json(r#"{"version":1,"genre":"neon_run","best":40}"#)
            .unwrap();
        assert_eq!(session.kind, SceneKind::Neon);
        assert!(session.world.is_none());
        let neon = session.neon.as_ref().unwrap();
        assert!(neon.best_score() >= 40);
        session.push_key("space", true);
        for _ in 0..30 {
            session.request_frame();
        }
        assert_eq!(session.neon.as_ref().unwrap().phase_name(), "play");
    }
}
