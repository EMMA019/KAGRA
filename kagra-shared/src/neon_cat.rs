//! Neon Cat Run. Not a WorldPlay genre.
//!
//! The cat, the arches, the rings, and the low obstacles are CC0 glTF.
//! See `docs/assets.md` and each pack's `CREDITS.md`. The tunnel grid and
//! the magenta rails stay emissive scenery: no CC0 pack is an infinite
//! synthwave shader. A missing GLB leaves the engine fallback (a capsule
//! for the cat, no prop for the rest). It does not invent a box cat.
//!
//! The chase camera looks along +Z. Screen-right is world -X.
//! `D` / `+lx` steer that way. Space, the jump button, and stick-up jump.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use serde_json::json;

use crate::flow::{FlowAction, FlowPhase, GameFlow};
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc, WorldLight, WorldProp, WorldWalker};

pub const GAME_ID: &str = "neon_cat";

const CAT_GLB: &str = "assets/models/kenney/cube-pets/animal-cat.glb";
const GATE_GLB: &str = "assets/models/quaternius/modular-dungeons-pack/Arch.glb";
const RING_GLB: &str = "assets/models/kenney/racing-kit/overheadRound.glb";
const CRATE_GLB: &str = "assets/models/kenney/platformer-kit/crate.glb";
const BARREL_GLB: &str = "assets/models/kenney/platformer-kit/barrel.glb";
const BARRIER_GLB: &str = "assets/models/kenney/racing-kit/barrierRed.glb";

/// Model-space feet sit near y = -0.3. This scale keeps the cat under the arch.
const CAT_SCALE: f32 = 0.58;
const CAT_FOOT: f32 = 0.30;
const WALL_X: f32 = 2.05;
const SPEED0: f32 = 10.5;
const SPEED_RAMP: f32 = 0.22;
const SPEED_CAP: f32 = 20.0;
const GRAVITY: f32 = -22.0;
const JUMP_V: f32 = 7.1;
const GRACE_SECS: f32 = 1.6;
const CAM_BACK: f32 = 7.4;
const CAM_LIFT: f32 = 1.72;
const CAM_FOV: f32 = 50.0;

const OBS_Z0: f32 = 26.0;
const OBS_GAP: f32 = 14.0;
const RING_Z0: f32 = 14.0;
const RING_GAP: f32 = 17.0;
const GATE_Z0: f32 = 10.0;
const GATE_GAP: f32 = 24.0;

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
pub struct NeonCat {
    pub flow: GameFlow,
    pub score: u32,
    score_acc: f32,
    pub rings: u32,
    pub alive: bool,
    x: f32,
    y: f32,
    z: f32,
    vx: f32,
    vy: f32,
    yaw: f32,
    pad_x: f32,
    pad_up: f32,
    jump_held: bool,
    key_left: bool,
    key_right: bool,
    key_jump: bool,
    anim_t: f32,
    time: f32,
    shake: f32,
    hitstop: f32,
    land_squash: f32,
    cam_x: f32,
    attract: f32,
    seed: u32,
    got: HashSet<i32>,
    just_rings: u32,
    just_crash: bool,
    just_jump: bool,
    sparks: Vec<Spark>,
    doc: WorldDoc,
}

pub fn is_cat_dump(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| {
            v.get("genre")
                .and_then(|g| g.as_str())
                .map(|s| s == GAME_ID)
        })
        .unwrap_or(false)
}

fn obs_z(index: i32) -> f32 {
    OBS_Z0 + index as f32 * OBS_GAP
}

fn obs_x(index: i32) -> f32 {
    (index as f32 * 0.85).sin() * 1.2
}

fn ring_z(index: i32) -> f32 {
    RING_Z0 + index as f32 * RING_GAP
}

fn ring_x(index: i32) -> f32 {
    if index <= 0 {
        return 0.0;
    }
    (index as f32 * 1.2).sin() * 0.95
}

fn gate_z(index: i32) -> f32 {
    GATE_Z0 + index as f32 * GATE_GAP
}

fn near_index(z: f32, origin: f32, gap: f32) -> i32 {
    ((z - origin) / gap).floor() as i32
}

/// Index of the next marker that is still in front of `z`.
fn next_ahead(z: f32, origin: f32, gap: f32, at: fn(i32) -> f32) -> i32 {
    let mut i = near_index(z, origin, gap).max(0);
    if at(i) < z + 0.35 {
        i += 1;
    }
    i
}

impl NeonCat {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("neon cat dump: {e}"))?;
        let mut run = Self::fresh();
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            run.flow.best_score = best as u32;
        }
        run.flow.load_save();
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            run.flow.best_score = run.flow.best_score.max(best as u32);
        }
        run.rebuild();
        Ok(run)
    }

    fn fresh() -> Self {
        let mut flow = GameFlow::new(GAME_ID);
        flow.title = "NEON CAT".into();
        flow.tagline = "RUN  ·  JUMP".into();
        Self {
            flow,
            score: 0,
            score_acc: 0.0,
            rings: 0,
            alive: true,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            vx: 0.0,
            vy: 0.0,
            yaw: 0.0,
            pad_x: 0.0,
            pad_up: 0.0,
            jump_held: false,
            key_left: false,
            key_right: false,
            key_jump: false,
            anim_t: 0.05,
            time: 0.0,
            shake: 0.0,
            hitstop: 0.0,
            land_squash: 0.0,
            cam_x: 0.0,
            attract: 0.0,
            seed: 11,
            got: HashSet::new(),
            just_rings: 0,
            just_crash: false,
            just_jump: false,
            sparks: Vec::new(),
            doc: WorldDoc::default(),
        }
    }

    pub fn score(&self) -> u32 {
        self.score
    }

    pub fn rings(&self) -> u32 {
        self.rings
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

    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> f32 {
        self.y
    }

    pub fn z(&self) -> f32 {
        self.z
    }

    pub fn document(&self) -> &WorldDoc {
        &self.doc
    }

    pub fn dump_json(&self) -> String {
        let body = self.doc.to_json().unwrap_or_else(|_| "{}".into());
        splice_json_key(&body, "sim", &self.sim_value())
    }

    pub fn state_json(&self) -> String {
        self.sim_value().to_string()
    }

    pub fn take_feel_json(&mut self) -> String {
        let body = json!({
            "crash": self.just_crash,
            "rings": self.just_rings,
            "jump": self.just_jump,
            "x": self.x,
            "y": self.y,
            "z": self.z,
        });
        self.just_crash = false;
        self.just_rings = 0;
        self.just_jump = false;
        body.to_string()
    }

    fn sim_value(&self) -> serde_json::Value {
        let oi = next_ahead(self.z, OBS_Z0, OBS_GAP, obs_z);
        let ri = next_ahead(self.z, RING_Z0, RING_GAP, ring_z);
        let (ox, oz, oh) = obstacle_hit(oi);
        json!({
            "phase": self.flow.phase.name(),
            "score": self.score,
            "best": self.flow.best_score,
            "rings": self.rings,
            "alive": self.alive,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "screen_right": self.screen_right().to_array(),
            "screen_left": (-self.screen_right()).to_array(),
            "next_ring_x": ring_x(ri),
            "next_ring_z": ring_z(ri),
            "next_obs_x": ox,
            "next_obs_z": oz,
            "next_obs_h": oh,
            "cam_forward": [0.0, 0.0, 1.0],
        })
    }

    fn screen_right(&self) -> glam::Vec3 {
        let eye = self.chase_eye();
        let target = self.chase_target();
        crate::scene3d::screen_right(glam::Vec3::from_array(eye), glam::Vec3::from_array(target))
    }

    fn chase_target(&self) -> [f32; 3] {
        [self.x * 0.35, 0.85, self.z + 12.0]
    }

    fn chase_eye(&self) -> [f32; 3] {
        let shake = (self.time * 53.0).sin() * self.shake;
        [
            self.cam_x + shake,
            CAM_LIFT + self.y * 0.35 + shake * 0.25,
            self.z - CAM_BACK,
        ]
    }

    /// Screen-right steer toward the next ring, and jump when the next
    /// obstacle is close and lined up. `+lx` is world −X on this camera.
    pub fn script_steer(&self) -> (f32, bool) {
        let ri = next_ahead(self.z, RING_Z0, RING_GAP, ring_z);
        let dx = ring_x(ri) - self.x;
        let lx = (-dx * 1.4).clamp(-1.0, 1.0);
        let oi = next_ahead(self.z, OBS_Z0, OBS_GAP, obs_z);
        let (ox, oz, _) = obstacle_hit(oi);
        let ahead = oz - self.z;
        let jump = (0.15..2.5).contains(&ahead) && (self.x - ox).abs() < 0.95;
        (lx, jump)
    }

    /// `lx` is screen-right. `lz` above 0.55 (stick up) is a jump.
    pub fn set_steer(&mut self, lx: f32, lz: f32, jump: bool) {
        self.pad_x = lx.clamp(-1.0, 1.0);
        self.pad_up = lz.clamp(-1.0, 1.0);
        self.jump_held = jump || lz > 0.55;
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
        let key = name.to_ascii_lowercase();
        match key.as_str() {
            "a" | "arrowleft" | "left" => {
                self.key_left = down;
                return true;
            }
            "d" | "arrowright" | "right" => {
                self.key_right = down;
                return true;
            }
            "w" | "arrowup" | "up" | "space" => {
                self.key_jump = down;
                return true;
            }
            _ => {}
        }
        if !down {
            return false;
        }
        if let Some(action) = self.flow.on_key(&key) {
            self.apply_action(action);
            return true;
        }
        false
    }

    fn apply_action(&mut self, action: FlowAction) {
        match action {
            FlowAction::Start | FlowAction::Retry => self.start_run(),
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
        self.rings = 0;
        self.alive = true;
        self.got.clear();
        self.sparks.clear();
        self.time = 0.0;
        self.flow.enter_play();
        self.rebuild();
    }

    fn reset_motion(&mut self) {
        self.x = 0.0;
        self.y = 0.0;
        self.z = 0.0;
        self.vx = 0.0;
        self.vy = 0.0;
        self.yaw = 0.0;
        self.hitstop = 0.0;
        self.shake = 0.0;
        self.cam_x = 0.0;
    }

    fn axis_x(&self) -> f32 {
        let kx = (self.key_right as i32 - self.key_left as i32) as f32;
        (kx + self.pad_x).clamp(-1.0, 1.0)
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        if dt > 0.0 {
            crate::mixer::tick(dt);
        }
        self.just_rings = 0;
        self.just_crash = false;
        self.just_jump = false;
        if dt <= 0.0 {
            self.rebuild();
            return;
        }
        if self.hitstop > 0.0 {
            self.hitstop = (self.hitstop - dt).max(0.0);
            self.tick_sparks(dt);
            self.decay_shake(dt);
            self.anim_t += dt * 0.35;
            if self.hitstop == 0.0 && !self.alive && self.flow.phase == FlowPhase::Play {
                self.flow.finish(self.score, self.rings, "CRASH");
            }
            self.rebuild();
            return;
        }
        if self.flow.phase == FlowPhase::Title {
            self.attract += dt;
            self.z = self.attract * 8.0;
            self.x = (self.attract * 0.7).sin() * 0.65;
            self.y = 0.0;
            self.anim_t += dt;
            self.smooth_cam(dt);
            self.tick_sparks(dt);
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
        let ax = self.axis_x();
        // +ax is screen-right. This camera looks along +Z, so that is world -X.
        let steer = -ax;
        let response = 1.0 - (-dt * 8.0).exp();
        self.vx += (steer * 5.6 - self.vx) * response;
        let grounded = self.y <= 0.001 && self.vy <= 0.0;
        let want_jump = self.jump_held || self.key_jump || self.pad_up > 0.55;
        if grounded && want_jump {
            self.vy = JUMP_V;
            self.y = 0.02;
            self.just_jump = true;
            crate::mixer::cue_sfx("jump");
        }
        self.vy += GRAVITY * dt;
        self.y += self.vy * dt;
        if self.y <= 0.0 {
            if self.vy < -1.5 {
                self.land_squash = 0.12;
            }
            self.y = 0.0;
            self.vy = 0.0;
        }
        self.land_squash = (self.land_squash - dt).max(0.0);
        self.x += self.vx * dt;
        self.yaw += (self.vx * 0.14 - self.yaw) * response;
        let speed = self.speed();
        self.z += speed * dt;
        self.score_acc += speed * dt * 2.2;
        let whole = self.score_acc.floor();
        if whole >= 1.0 {
            self.score += whole as u32;
            self.score_acc -= whole;
        }
        self.anim_t += dt * if self.y > 0.05 { 0.7 } else { 1.15 };
        let grace = self.time < GRACE_SECS;
        if !grace && self.x.abs() > WALL_X {
            self.crash();
        } else if !grace {
            self.touch_obstacles();
        }
        if self.alive {
            self.collect_rings();
        }
        self.smooth_cam(dt);
        self.tick_sparks(dt);
        self.decay_shake(dt);
        self.rebuild();
    }

    fn speed(&self) -> f32 {
        (SPEED0 + self.time * SPEED_RAMP).min(SPEED_CAP)
    }

    fn smooth_cam(&mut self, dt: f32) {
        let k = 1.0 - (-dt * 4.5).exp();
        self.cam_x += (self.x * 0.72 - self.cam_x) * k;
    }

    fn touch_obstacles(&mut self) {
        let i0 = near_index(self.z, OBS_Z0, OBS_GAP).saturating_sub(1);
        for i in i0..i0 + 3 {
            if i < 0 {
                continue;
            }
            let (ox, oz, height) = obstacle_hit(i);
            let half = obstacle_half(i);
            if (self.z - oz).abs() > half.1 + 0.25 {
                continue;
            }
            if (self.x - ox).abs() > half.0 + 0.22 {
                continue;
            }
            if self.y < height * 0.72 {
                self.crash();
                return;
            }
        }
    }

    fn collect_rings(&mut self) {
        let i0 = near_index(self.z, RING_Z0, RING_GAP).saturating_sub(1);
        for i in i0..i0 + 3 {
            if i < 0 || self.got.contains(&i) {
                continue;
            }
            let dx = self.x - ring_x(i);
            let dz = self.z - ring_z(i);
            if dx * dx + dz * dz > 1.15 * 1.15 {
                continue;
            }
            self.got.insert(i);
            self.rings += 1;
            self.just_rings += 1;
            self.score += 100;
            self.shake = self.shake.max(0.06);
            self.burst(self.x, self.y + 0.6, self.z, 14, [80, 255, 255], 3.4, 0.12);
            crate::mixer::cue_sfx("card");
        }
    }

    fn crash(&mut self) {
        if !self.alive {
            return;
        }
        self.alive = false;
        self.just_crash = true;
        self.shake = 0.46;
        self.hitstop = 0.11;
        self.vy = 0.0;
        self.burst(self.x, self.y + 0.4, self.z, 18, [255, 60, 170], 4.6, 0.16);
        self.burst(self.x, self.y + 0.5, self.z, 10, [90, 240, 255], 3.2, 0.12);
        crate::mixer::cue_sfx("fire");
    }

    fn decay_shake(&mut self, dt: f32) {
        self.shake = (self.shake - dt * 0.85).max(0.0);
    }

    #[allow(clippy::too_many_arguments)]
    fn burst(&mut self, x: f32, y: f32, z: f32, n: u32, color: [u32; 3], speed: f32, size: f32) {
        for _ in 0..n {
            if self.sparks.len() >= 36 {
                self.sparks.remove(0);
            }
            let ang = self.rng() * std::f32::consts::TAU;
            let rad = self.rng();
            let spd = speed * (0.4 + self.rng());
            let vy = 1.1 + self.rng() * speed * 0.4;
            let max_life = 0.28 + self.rng() * 0.28;
            let spark_size = size * (0.7 + self.rng() * 0.6);
            self.sparks.push(Spark {
                x,
                y,
                z,
                vx: ang.cos() * spd * rad,
                vy,
                vz: ang.sin() * spd * rad - 1.5,
                life: 0.0,
                max_life,
                size: spark_size,
                color,
            });
        }
    }

    fn tick_sparks(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.sparks.len() {
            let s = &mut self.sparks[i];
            s.life += dt;
            if s.life >= s.max_life {
                self.sparks.remove(i);
                continue;
            }
            s.vy -= 9.0 * dt;
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.z += s.vz * dt;
            i += 1;
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
            half: 48.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([10, 4, 28, 255]),
            hemi_sky: Some([0.22, 0.12, 0.48]),
            hemi_ground: Some([0.10, 0.02, 0.16]),
            env_map: Some("preset:studio".into()),
            shadow_softness: Some(1.0),
            bloom: Some(0.42),
            bloom_threshold: Some(0.85),
            vignette: Some(0.42),
            lut: Some("cool".into()),
            lut_strength: Some(0.2),
            fxaa: Some(true),
            exposure: Some(1.05),
            ibl: Some(0.34),
            tonemap: Some(true),
            cull: Some(true),
            fog_start: Some(16.0),
            fog_end: Some(62.0),
            ..WorldDoc::default()
        };
        let eye = self.chase_eye();
        let target = self.chase_target();
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "chase".into(),
            position: eye,
            target,
            fov: CAM_FOV,
        });
        doc.lights.push(light(
            0,
            "key",
            [eye[0] - 2.0, eye[1] + 6.0, eye[2] - 4.0],
            [0.15, -0.8, 0.55],
            [0.65, 0.55, 1.0],
            1.15,
            42.0,
            true,
        ));
        doc.lights.push(light(
            1,
            "cyan",
            [self.x, self.y + 1.2, self.z + 2.0],
            [0.0, 0.0, 0.0],
            [0.35, 0.95, 1.0],
            1.7,
            8.0,
            false,
        ));
        doc.lights.push(light(
            2,
            "rim",
            [self.x, self.y + 1.4, self.z + 8.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.25, 0.7],
            1.35,
            14.0,
            false,
        ));
        self.push_tunnel(&mut doc);
        self.push_gates(&mut doc);
        self.push_rings(&mut doc);
        self.push_obstacles(&mut doc);
        self.push_cat(&mut doc);
        self.push_sparks(&mut doc);
        self.doc = doc;
    }

    fn push_tunnel(&self, doc: &mut WorldDoc) {
        let z = self.z + 22.0;
        doc.props.push(beam(
            "floor",
            [0.0, -0.04, z],
            [6.4, 0.06, 70.0],
            [12, 8, 36],
            0.05,
        ));
        for k in -3..=3 {
            let col = if k % 2 == 0 {
                [40, 210, 255]
            } else {
                [255, 40, 180]
            };
            doc.props.push(beam(
                &format!("lane-{k}"),
                [k as f32 * 0.7, 0.02, z],
                [0.035, 0.02, 64.0],
                col,
                0.85,
            ));
        }
        let origin = (self.z / 2.0).floor() * 2.0;
        for i in 0..22 {
            let gz = origin + i as f32 * 2.0;
            if gz < self.z - 2.0 {
                continue;
            }
            let col = if i % 2 == 0 {
                [50, 140, 255]
            } else {
                [180, 40, 220]
            };
            doc.props.push(beam(
                &format!("grid-{i}"),
                [0.0, 0.025, gz],
                [5.6, 0.015, 0.04],
                col,
                0.7,
            ));
        }
        for side in [-1.0_f32, 1.0] {
            let name = if side < 0.0 { "wall-l" } else { "wall-r" };
            doc.props.push(beam(
                name,
                [side * 2.7, 1.35, z],
                [0.08, 2.8, 70.0],
                [18, 10, 48],
                0.08,
            ));
            doc.props.push(beam(
                &format!("rail-{name}"),
                [side * 2.35, 0.08, z],
                [0.06, 0.07, 64.0],
                [255, 30, 170],
                2.2,
            ));
            let line0 = (self.z / 4.0).floor() * 4.0;
            for i in 0..14 {
                let gz = line0 + i as f32 * 4.0;
                if gz < self.z - 1.0 {
                    continue;
                }
                doc.props.push(beam(
                    &format!("wgrid-{name}-{i}"),
                    [side * 2.66, 1.35, gz],
                    [0.025, 2.5, 0.03],
                    [50, 170, 255],
                    0.95,
                ));
            }
        }
        let rib0 = (self.z / 4.0).floor() * 4.0;
        for i in 0..12 {
            let rz = rib0 + i as f32 * 4.0;
            if rz < self.z + 6.0 {
                continue;
            }
            let col = if i % 2 == 0 {
                [255, 40, 180]
            } else {
                [60, 200, 255]
            };
            doc.props.push(beam(
                &format!("rib-{i}"),
                [0.0, 2.55, rz],
                [5.4, 0.04, 0.05],
                col,
                1.15,
            ));
        }
    }

    fn push_gates(&self, doc: &mut WorldDoc) {
        let i0 = near_index(self.z, GATE_Z0, GATE_GAP).saturating_sub(1);
        for i in i0..i0 + 5 {
            if i < 0 {
                continue;
            }
            let z = gate_z(i);
            if z < self.z + 3.0 {
                continue;
            }
            if let Some(gate) = glb_prop(
                &format!("gate-{i}"),
                "gate",
                GATE_GLB,
                [0.0, 0.0, z],
                [1.15, 1.15, 1.15],
                0.0,
                0.0,
                [255, 50, 190],
                1.7,
                [1.0, 0.15, 0.65],
            ) {
                doc.props.push(gate);
            }
        }
    }

    fn push_rings(&self, doc: &mut WorldDoc) {
        let scale = 1.65;
        let i0 = near_index(self.z, RING_Z0, RING_GAP).saturating_sub(1);
        for i in i0..i0 + 5 {
            if i < 0 || self.got.contains(&i) {
                continue;
            }
            let z = ring_z(i);
            if z < self.z - 1.5 {
                continue;
            }
            // Mesh center sits about +0.5 x and +0.47 y in the file.
            let x = ring_x(i) - 0.50 * scale;
            let y = 0.05;
            if let Some(ring) = glb_prop(
                &format!("ring-{i}"),
                "ring",
                RING_GLB,
                [x, y, z + 0.16 * scale],
                [scale, scale, scale],
                0.0,
                0.0,
                [70, 255, 255],
                2.1,
                [0.25, 1.0, 1.0],
            ) {
                doc.props.push(ring);
            }
        }
    }

    fn push_obstacles(&self, doc: &mut WorldDoc) {
        let i0 = near_index(self.z, OBS_Z0, OBS_GAP).saturating_sub(1);
        for i in i0..i0 + 6 {
            if i < 0 {
                continue;
            }
            let z = obs_z(i);
            if z < self.z - 2.0 {
                continue;
            }
            let kind = i.rem_euclid(3);
            let (path, scale, ox) = match kind {
                0 => (CRATE_GLB, [1.15, 1.15, 1.15], obs_x(i)),
                1 => (BARREL_GLB, [1.2, 1.2, 1.2], obs_x(i)),
                _ => {
                    let sx = 4.4;
                    (BARRIER_GLB, [sx, 3.8, 3.4], obs_x(i) - 0.125 * sx)
                }
            };
            if let Some(obstacle) = glb_prop(
                &format!("obs-{i}"),
                "obstacle",
                path,
                [ox, 0.0, z],
                scale,
                0.0,
                0.0,
                [255, 255, 255],
                0.25,
                [0.7, 0.85, 1.0],
            ) {
                doc.props.push(obstacle);
            }
        }
    }

    fn push_cat(&mut self, doc: &mut WorldDoc) {
        let mut stretch = 1.0;
        if self.y > 0.15 {
            stretch = 1.14;
        } else if self.land_squash > 0.0 {
            stretch = 0.78;
        }
        let mut cat = WorldWalker {
            id: "walker:cat".into(),
            kind: "walker".into(),
            name: "cat".into(),
            position: [self.x, self.y + CAT_FOOT * CAT_SCALE, self.z],
            yaw: self.yaw,
            on_ground: self.y <= 0.02,
            model: String::new(),
            gltf: None,
            clip: self.anim_t,
            anim: "run".into(),
            anim_blend: 1.0,
            scale: Some([CAT_SCALE, CAT_SCALE * stretch, CAT_SCALE]),
            ..WorldWalker::default()
        };
        if glb_ready(CAT_GLB) {
            cat.gltf = Some(CAT_GLB.into());
        }
        doc.walkers.push(cat.clone());
        doc.player = Some(cat);
    }

    fn push_sparks(&self, doc: &mut WorldDoc) {
        for (i, s) in self.sparks.iter().enumerate() {
            let fade = 1.0 - (s.life / s.max_life).clamp(0.0, 1.0);
            let size = (s.size * (0.45 + 0.55 * fade)).max(0.04);
            doc.props.push(glow(
                &format!("spark-{i}"),
                "sphere",
                "spark",
                [s.x, s.y, s.z],
                [size, size, size],
                s.color,
                2.0 * fade + 0.4,
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
        self.flow.paint(&mut list, width, height);
        if self.flow.phase == FlowPhase::Play || self.hitstop > 0.0 {
            list.quads
                .push(Quad::new(12.0, 10.0, 168.0, 36.0, [8, 4, 20, 160]));
            label(
                &mut list,
                &format!("SCORE {}", self.score),
                22.0,
                16.0,
                16.0,
                [180, 245, 255, 255],
                TextAlign::Left,
            );
        }
        list
    }
}

/// Half extents in x and z, and the height a jump must clear.
fn obstacle_hit(index: i32) -> (f32, f32, f32) {
    let kind = index.rem_euclid(3);
    let height = match kind {
        0 => 0.50 * 1.15,
        1 => 0.48 * 1.2,
        _ => 0.131 * 3.8,
    };
    (obs_x(index), obs_z(index), height)
}

fn obstacle_half(index: i32) -> (f32, f32) {
    match index.rem_euclid(3) {
        0 => (0.25 * 1.15, 0.25 * 1.15),
        1 => (0.26 * 1.2, 0.26 * 1.2),
        _ => (0.125 * 4.4, 0.06 * 3.4),
    }
}

/// True when `path` is a mounted or on-disk GLB that parses.
/// A miss is not cached, so a wasm page can mount the file after boot.
fn glb_ready(path: &str) -> bool {
    static OK: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let cache = OK.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(guard) = cache.lock() {
        if guard.contains(path) {
            return true;
        }
    }
    let Some(file) = crate::assets::resolve_asset(path) else {
        return false;
    };
    let Ok(bytes) = crate::assets::read_bytes(&file) else {
        return false;
    };
    if crate::gltf_load::mesh_from_glb(&bytes).is_err() {
        return false;
    }
    if let Ok(mut guard) = cache.lock() {
        guard.insert(path.to_string());
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn glb_prop(
    id: &str,
    name: &str,
    path: &str,
    position: [f32; 3],
    scale: [f32; 3],
    yaw: f32,
    pitch: f32,
    color: [u32; 3],
    emissive: f32,
    emissive_color: [f32; 3],
) -> Option<WorldProp> {
    if !glb_ready(path) {
        return None;
    }
    let mut prop = glow(id, "box", name, position, scale, color, emissive);
    prop.yaw = yaw;
    prop.pitch = pitch;
    prop.emissive_color = Some(emissive_color);
    prop.roughness = 0.35;
    prop.metallic = 0.15;
    prop.gltf = Some(path.to_string());
    // Caller color tints the glTF albedo. White leaves a textured prop alone.
    // Magenta / cyan is how the stone arch and the hoop read as neon.
    prop.color = Some(color);
    Some(prop)
}

fn beam(id: &str, pos: [f32; 3], scale: [f32; 3], color: [u32; 3], emissive: f32) -> WorldProp {
    glow(id, "box", "grid", pos, scale, color, emissive)
}

fn glow(
    id: &str,
    model: &str,
    name: &str,
    position: [f32; 3],
    scale: [f32; 3],
    color: [u32; 3],
    emissive: f32,
) -> WorldProp {
    WorldProp {
        id: format!("prop:{id}"),
        kind: "prop".into(),
        name: name.into(),
        position,
        model: model.into(),
        scale,
        enabled: true,
        color: Some(color),
        emissive: Some(emissive),
        roughness: 0.32,
        metallic: 0.05,
        collider: Some("none".into()),
        is_static: true,
        ..WorldProp::default()
    }
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

fn splice_json_key(body: &str, key: &str, value: &serde_json::Value) -> String {
    let pretty = serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".into());
    let mut block = format!("\n  \"{key}\": ");
    for (i, line) in pretty.lines().enumerate() {
        if i == 0 {
            block.push_str(line);
        } else {
            block.push('\n');
            block.push_str("  ");
            block.push_str(line);
        }
    }
    let Some(rest) = body.strip_prefix('{') else {
        return body.to_string();
    };
    format!("{{{block},{rest}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{SceneKind, SharedSession};

    #[test]
    fn screen_right_steer_moves_negative_x() {
        let mut run = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        run.confirm();
        let before = run.x();
        run.set_steer(1.0, 0.0, false);
        for _ in 0..40 {
            run.tick(1.0 / 60.0);
        }
        assert!(
            run.x() < before - 0.4,
            "D / +lx is screen-right, world -X on this chase cam, {} -> {}",
            before,
            run.x()
        );
        let right = run.screen_right();
        assert!(right.x < -0.5, "screen_right.x {}", right.x);
    }

    #[test]
    fn jump_clears_the_first_crate_and_a_miss_crashes() {
        let mut run = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        run.confirm();
        let mut jumped = false;
        for _ in 0..60 * 8 {
            let ahead = obs_z(0) - run.z();
            let close = ahead < 2.4 && ahead > 0.2 && (run.x() - obs_x(0)).abs() < 0.8;
            run.set_steer(0.0, 0.0, close && !jumped);
            if close {
                jumped = true;
            }
            run.tick(1.0 / 60.0);
            if run.z() > obs_z(0) + 1.0 {
                break;
            }
        }
        assert!(
            run.alive,
            "a timed jump should clear the crate at z {}",
            obs_z(0)
        );
        assert!(run.z() > obs_z(0));

        let mut crash = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        crash.confirm();
        for _ in 0..60 * 8 {
            crash.set_steer(0.0, 0.0, false);
            crash.tick(1.0 / 60.0);
            if !crash.alive {
                break;
            }
        }
        assert!(!crash.alive, "staying on the ground hits the crate");
        for _ in 0..20 {
            crash.tick(1.0 / 60.0);
        }
        assert_eq!(crash.phase_name(), "result");
    }

    #[test]
    fn running_through_the_first_ring_scores() {
        let mut run = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        run.confirm();
        for _ in 0..60 * 4 {
            run.set_steer(0.0, 0.0, false);
            run.tick(1.0 / 60.0);
            if run.rings() > 0 {
                break;
            }
        }
        assert!(
            run.rings() >= 1,
            "z {} should have crossed the center ring",
            run.z()
        );
        assert!(run.score() >= 100);
    }

    #[test]
    fn cat_and_gate_are_gltf() {
        let run = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        let cat = run.document().player.as_ref().unwrap();
        assert_eq!(cat.gltf.as_deref(), Some(CAT_GLB));
        assert_eq!(cat.anim, "run");
        assert!(
            run.document()
                .props
                .iter()
                .any(|p| p.name == "gate" && p.gltf.as_deref() == Some(GATE_GLB)),
            "arch gate must be the dungeon GLB"
        );
        assert!(run
            .document()
            .props
            .iter()
            .any(|p| p.name == "ring" && p.gltf.as_deref() == Some(RING_GLB)));
        assert!(run
            .document()
            .props
            .iter()
            .any(|p| p.name == "obstacle" && p.gltf.is_some()));
    }

    #[test]
    fn session_loads_cat_without_replacing_neon() {
        let mut session = SharedSession::default();
        session
            .load_game_json(r#"{"version":1,"genre":"neon_cat"}"#)
            .unwrap();
        assert_eq!(session.kind, SceneKind::NeonCat);
        assert!(session.cat.is_some());
        assert!(session.neon.is_none());
        assert!(session.world.is_none());
        session
            .load_game_json(r#"{"version":1,"genre":"neon_run"}"#)
            .unwrap();
        assert_eq!(session.kind, SceneKind::Neon);
        assert!(session.cat.is_none());
    }
}
