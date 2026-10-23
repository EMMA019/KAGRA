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
//! The default rig is a damped third person behind and above the cat.
//! `cam: "classic"` keeps the older fixed chase.

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
/// Slide along the magenta rail. The rail, the arch, and the floor are
/// scenery. Only a crate, barrel, or barrier ends the run.
const LANE_LIMIT: f32 = 2.15;
/// Pull the kill box in from the measured mesh so a graze survives.
const HIT_INSET: f32 = 0.06;
const HIT_TOP: f32 = 0.82;
const SPEED0: f32 = 10.5;
const SPEED_RAMP: f32 = 0.22;
const SPEED_CAP: f32 = 20.0;
const GRAVITY: f32 = -22.0;
const JUMP_V: f32 = 7.1;
const GRACE_SECS: f32 = 1.6;
const CAM_BACK: f32 = 7.4;
const CAM_LIFT: f32 = 1.72;
const CAM_FOV: f32 = 50.0;
/// Closer and a little higher than the classic rig, so the tunnel has depth
/// and the cat still sits in the lower half of a 412×915 frame.
const CHASE_BACK: f32 = 4.85;
const CHASE_LIFT: f32 = 2.15;
const CHASE_LOOK: f32 = 10.5;
const CHASE_LOOK_Y: f32 = 0.62;
const CHASE_FOV: f32 = 50.0;
const CHASE_FOV_FAST: f32 = 58.0;
/// Full-steer Dutch angle. Visual only: the stick stays world −X.
const CHASE_ROLL: f32 = 0.07;

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

/// Critically damped scalar. `omega` is radians per second.
#[derive(Clone, Copy, Debug)]
struct Spring1 {
    pos: f32,
    vel: f32,
}

impl Default for Spring1 {
    fn default() -> Self {
        Self { pos: 0.0, vel: 0.0 }
    }
}

impl Spring1 {
    fn snap(&mut self, target: f32) {
        self.pos = target;
        self.vel = 0.0;
    }

    fn step(&mut self, target: f32, omega: f32, dt: f32) {
        let omega = omega.max(0.01);
        let exp = (-omega * dt).exp();
        let change = self.pos - target;
        let temp = (self.vel + omega * change) * dt;
        self.pos = target + (change + temp) * exp;
        self.vel = (self.vel - omega * temp) * exp;
    }
}

/// Same clock as `kagra.juice.Juice.camera_offset`. Amplitude decays with
/// the square of the time still left. Three sines, meters.
#[derive(Clone, Copy, Debug)]
struct JuiceShake {
    t: f32,
    dur: f32,
    amp: f32,
    freq: f32,
}

impl Default for JuiceShake {
    fn default() -> Self {
        Self {
            t: 0.0,
            dur: 0.0,
            amp: 0.0,
            freq: 28.0,
        }
    }
}

impl JuiceShake {
    /// `kagra.juice.Juice.shake` defaults are 0.08 m and 0.18 s at 28 Hz.
    fn shake(&mut self, amplitude: f32, duration: f32) {
        self.amp = amplitude.max(0.0);
        self.dur = duration.max(1e-4);
        self.freq = 28.0;
        self.t = 0.0;
    }

    fn tick(&mut self, dt: f32) {
        if self.dur > 0.0 && self.t < self.dur {
            self.t += dt;
        }
    }

    fn offset(&self) -> [f32; 3] {
        if self.dur <= 1e-6 || self.t >= self.dur {
            return [0.0, 0.0, 0.0];
        }
        let u = 1.0 - (self.t / self.dur);
        let amp = self.amp * u * u;
        let phase = self.t * self.freq;
        [
            (phase * 17.0).sin() * amp,
            (phase * 23.0).sin() * amp * 0.55,
            (phase * 11.0).sin() * amp * 0.25,
        ]
    }
}

#[derive(Debug)]
pub struct NeonCat {
    pub flow: GameFlow,
    pub score: u32,
    score_acc: f32,
    pub rings: u32,
    pub alive: bool,
    /// `CRATE` / `BARREL` / `BARRIER` after a hit. Empty while alive.
    death: String,
    debug: bool,
    /// Dump `quality` (`low` / `mobile` / `high`). The page sets this.
    quality: Option<String>,
    /// `cam: "classic"` keeps the fixed chase. The default is the spring rig.
    classic: bool,
    /// Obstacle indices whose glTF was actually added to the dump.
    spawned: HashSet<i32>,
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
    /// Landing bump. Same offset as `Juice.camera_offset`.
    land: JuiceShake,
    cam_x: f32,
    eye_x: Spring1,
    eye_y: Spring1,
    look_x: Spring1,
    bank: Spring1,
    fov_spring: Spring1,
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
        if let Some(q) = value.get("quality").and_then(|v| v.as_str()) {
            let q = q.trim();
            if matches!(q, "low" | "0" | "off" | "mobile" | "high" | "2") {
                run.quality = Some(q.to_string());
            }
        }
        if value
            .get("cam")
            .and_then(|v| v.as_str())
            .is_some_and(|cam| cam.eq_ignore_ascii_case("classic"))
        {
            run.classic = true;
        }
        run.rebuild();
        Ok(run)
    }

    fn fresh() -> Self {
        let mut flow = GameFlow::new(GAME_ID);
        flow.title = "NEON CAT".into();
        flow.tagline = "RUN  ·  JUMP".into();
        let mut run = Self {
            flow,
            score: 0,
            score_acc: 0.0,
            rings: 0,
            alive: true,
            death: String::new(),
            debug: false,
            quality: None,
            classic: false,
            spawned: HashSet::new(),
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
            land: JuiceShake::default(),
            cam_x: 0.0,
            eye_x: Spring1::default(),
            eye_y: Spring1::default(),
            look_x: Spring1::default(),
            bank: Spring1::default(),
            fov_spring: Spring1::default(),
            attract: 0.0,
            seed: 11,
            got: HashSet::new(),
            just_rings: 0,
            just_crash: false,
            just_jump: false,
            sparks: Vec::new(),
            doc: WorldDoc::default(),
        };
        run.snap_chase();
        run
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

    pub fn death(&self) -> &str {
        &self.death
    }

    /// `?debug=1` draws the kill boxes. They are not colliders.
    pub fn set_debug(&mut self, on: bool) {
        self.debug = on;
        self.rebuild();
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
        let ahead = obstacle_shape(oi);
        let (ox, oz, oh) = (ahead.hit_cx, ahead.hit_cz, ahead.hit_top);
        json!({
            "phase": self.flow.phase.name(),
            "score": self.score,
            "best": self.flow.best_score,
            "rings": self.rings,
            "alive": self.alive,
            "death": self.death,
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
            "cam_forward": self.cam_forward(),
            "cam": if self.classic { "classic" } else { "chase" },
            "fov": self.chase_fov(),
        })
    }

    fn cam_forward(&self) -> [f32; 3] {
        let eye = self.chase_eye();
        let target = self.chase_target();
        let d = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len < 1e-5 {
            [0.0, 0.0, 1.0]
        } else {
            [d[0] / len, d[1] / len, d[2] / len]
        }
    }

    fn screen_right(&self) -> glam::Vec3 {
        let eye = self.chase_eye();
        let target = self.chase_target();
        crate::scene3d::screen_right(glam::Vec3::from_array(eye), glam::Vec3::from_array(target))
    }

    fn chase_target(&self) -> [f32; 3] {
        if self.classic {
            return [self.x * 0.35, 0.85, self.z + 12.0];
        }
        let bump = self.view_bump();
        [
            self.look_x.pos + bump[0],
            CHASE_LOOK_Y + self.y * 0.15 + bump[1],
            self.z + CHASE_LOOK + bump[2],
        ]
    }

    fn chase_eye(&self) -> [f32; 3] {
        if self.classic {
            let shake = (self.time * 53.0).sin() * self.shake;
            return [
                self.cam_x + shake,
                CAM_LIFT + self.y * 0.35 + shake * 0.25,
                self.z - CAM_BACK,
            ];
        }
        let bump = self.view_bump();
        [
            self.eye_x.pos + bump[0],
            self.eye_y.pos + bump[1],
            self.z - CHASE_BACK + bump[2],
        ]
    }

    /// Crash shake plus the landing juice offset. Added to the eye and the
    /// look target so the bump translates the rig.
    fn view_bump(&self) -> [f32; 3] {
        let shake = (self.time * 53.0).sin() * self.shake;
        let land = self.land.offset();
        [land[0] + shake, land[1] + shake * 0.25, land[2]]
    }

    fn chase_fov(&self) -> f32 {
        if self.classic {
            CAM_FOV
        } else {
            self.fov_spring.pos
        }
    }

    fn chase_roll(&self) -> f32 {
        if self.classic {
            0.0
        } else {
            self.bank.pos
        }
    }

    fn fov_for_speed(&self) -> f32 {
        let span = (SPEED_CAP - SPEED0).max(0.01);
        let t = ((self.speed() - SPEED0) / span).clamp(0.0, 1.0);
        CHASE_FOV + t * (CHASE_FOV_FAST - CHASE_FOV)
    }

    fn snap_chase(&mut self) {
        self.eye_x.snap(self.x);
        self.eye_y.snap(CHASE_LIFT + self.y * 0.42);
        self.look_x.snap(self.x * 0.9);
        self.bank.snap(0.0);
        self.fov_spring.snap(self.fov_for_speed());
        self.cam_x = self.x * 0.72;
    }

    /// Screen-right steer toward the next ring, and jump when the next
    /// obstacle is close and lined up. `+lx` is world −X on this camera.
    pub fn script_steer(&self) -> (f32, bool) {
        let ri = next_ahead(self.z, RING_Z0, RING_GAP, ring_z);
        let dx = ring_x(ri) - self.x;
        let lx = (-dx * 1.4).clamp(-1.0, 1.0);
        let oi = next_ahead(self.z, OBS_Z0, OBS_GAP, obs_z);
        let ahead_obs = obstacle_shape(oi);
        let (ox, oz) = (ahead_obs.hit_cx, ahead_obs.hit_cz);
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
        self.reset_motion();
        self.alive = true;
        self.death.clear();
        self.score = 0;
        self.score_acc = 0.0;
        self.rings = 0;
        self.attract = 0.0;
        self.time = 0.0;
        self.anim_t = 0.05;
        self.jump_held = false;
        self.key_left = false;
        self.key_right = false;
        self.key_jump = false;
        self.pad_x = 0.0;
        self.pad_up = 0.0;
        self.got.clear();
        self.sparks.clear();
        self.rebuild();
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
        self.death.clear();
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
        self.land = JuiceShake::default();
        self.cam_x = 0.0;
        self.snap_chase();
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
            self.decay_feel(dt);
            self.anim_t += dt * 0.35;
            if self.hitstop == 0.0 && !self.alive && self.flow.phase == FlowPhase::Play {
                let line = format!("HIT {}", self.death);
                self.flow.finish(self.score, self.rings, &line);
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
            self.decay_feel(dt);
            self.rebuild();
            return;
        }
        if self.flow.phase != FlowPhase::Play || !self.alive {
            self.tick_sparks(dt);
            self.decay_feel(dt);
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
                if !self.classic {
                    // Same defaults as `kagra.juice.Juice.shake`.
                    self.land.shake(0.08, 0.18);
                }
            }
            self.y = 0.0;
            self.vy = 0.0;
        }
        self.land_squash = (self.land_squash - dt).max(0.0);
        self.x += self.vx * dt;
        if self.x.abs() > LANE_LIMIT {
            self.x = self.x.signum() * LANE_LIMIT;
            if self.x.is_sign_positive() {
                self.vx = self.vx.min(0.0);
            } else {
                self.vx = self.vx.max(0.0);
            }
        }
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
        // Grace covers the spawn stretch. The rail is not a kill plane,
        // so steering into it during grace cannot strand the cat outside.
        if self.time >= GRACE_SECS {
            self.touch_obstacles();
        }
        if self.alive {
            self.collect_rings();
        }
        self.smooth_cam(dt);
        self.tick_sparks(dt);
        self.decay_feel(dt);
        self.rebuild();
    }

    fn speed(&self) -> f32 {
        (SPEED0 + self.time * SPEED_RAMP).min(SPEED_CAP)
    }

    fn smooth_cam(&mut self, dt: f32) {
        if self.classic {
            let k = 1.0 - (-dt * 4.5).exp();
            self.cam_x += (self.x * 0.72 - self.cam_x) * k;
            return;
        }
        let want_y = CHASE_LIFT + self.y * 0.42;
        // Held steer keeps the bank after the rail zeros vx. A bare
        // lateral velocity still banks, which is what the tests set.
        let steer = if self.axis_x().abs() > 0.2 {
            -self.axis_x()
        } else {
            (self.vx / 5.6).clamp(-1.0, 1.0)
        };
        let want_roll = steer * CHASE_ROLL;
        self.eye_x.step(self.x, 12.0, dt);
        self.eye_y.step(want_y, 8.0, dt);
        self.look_x.step(self.x * 0.9, 8.0, dt);
        self.bank.step(want_roll, 5.5, dt);
        self.fov_spring.step(self.fov_for_speed(), 2.4, dt);
    }

    fn touch_obstacles(&mut self) {
        let i0 = near_index(self.z, OBS_Z0, OBS_GAP).saturating_sub(1);
        for i in i0..i0 + 3 {
            if i < 0 {
                continue;
            }
            if !self.spawned.contains(&i) {
                continue;
            }
            let shape = obstacle_shape(i);
            if feet_hit(&shape, self.x, self.y, self.z) {
                self.crash(shape.kind);
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

    fn crash(&mut self, reason: &str) {
        if !self.alive {
            return;
        }
        self.alive = false;
        self.death = reason.to_string();
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

    fn decay_feel(&mut self, dt: f32) {
        self.decay_shake(dt);
        self.land.tick(dt);
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
            quality: self.quality.clone(),
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
            name: if self.classic {
                "classic".into()
            } else {
                "chase".into()
            },
            position: eye,
            target,
            fov: self.chase_fov(),
            roll: self.chase_roll(),
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
        self.spawned = self.push_obstacles(&mut doc);
        if self.debug {
            self.push_hitboxes(&mut doc);
        }
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

    fn push_obstacles(&self, doc: &mut WorldDoc) -> HashSet<i32> {
        let mut spawned = HashSet::new();
        let i0 = near_index(self.z, OBS_Z0, OBS_GAP).saturating_sub(1);
        for i in i0..i0 + 6 {
            if i < 0 {
                continue;
            }
            let shape = obstacle_shape(i);
            if shape.origin[2] < self.z - 2.0 {
                continue;
            }
            if !obstacle_drawable(shape.path) {
                continue;
            }
            if let Some(obstacle) = glb_prop(
                &format!("obs-{i}"),
                "obstacle",
                shape.path,
                shape.origin,
                shape.scale,
                0.0,
                0.0,
                [255, 255, 255],
                0.25,
                [0.7, 0.85, 1.0],
            ) {
                doc.props.push(obstacle);
                spawned.insert(i);
            }
        }
        spawned
    }

    fn push_hitboxes(&self, doc: &mut WorldDoc) {
        let z = self.z + 18.0;
        for side in [-1.0_f32, 1.0] {
            doc.props.push(glow(
                &format!("lane-lim-{side}"),
                "box",
                "hitbox",
                [side * LANE_LIMIT, 0.2, z],
                [0.03, 0.55, 36.0],
                [255, 210, 40],
                2.2,
            ));
        }
        let i0 = near_index(self.z, OBS_Z0, OBS_GAP).saturating_sub(1);
        for i in i0..i0 + 6 {
            if i < 0 {
                continue;
            }
            let shape = obstacle_shape(i);
            if shape.origin[2] < self.z - 2.0 {
                continue;
            }
            // The kill box is inset inside the mesh, so a cage on that box
            // disappears into the crate. Draw the skin a little proud, in
            // cyan, so the phone overlay shows which prop can kill.
            let proud = 0.1;
            push_aabb_cage(
                doc,
                &format!("hit-{i}"),
                [
                    shape.skin_min[0] - proud,
                    shape.skin_min[1],
                    shape.skin_min[2] - proud,
                ],
                [
                    shape.skin_max[0] + proud,
                    shape.skin_max[1] + proud,
                    shape.skin_max[2] + proud,
                ],
                [60, 255, 255],
            );
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
        self.paint_hud(width, height, true)
    }

    /// `menu` draws the in-canvas title / result column. The browser page
    /// owns that chrome and passes `false` so a CRASH panel cannot sit
    /// under the HTML card.
    pub fn paint_hud(&self, width: u32, height: u32, menu: bool) -> DrawList {
        let mut list = DrawList {
            clear: [0, 0, 0, 0],
            quads: Vec::new(),
            texts: Vec::new(),
            images: Vec::new(),
        };
        if menu {
            self.flow.paint(&mut list, width, height);
        }
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

struct ObstacleShape {
    kind: &'static str,
    path: &'static str,
    /// Prop origin. Scale is about this point, then the mesh is translated.
    origin: [f32; 3],
    scale: [f32; 3],
    /// World AABB of the baked mesh.
    skin_min: [f32; 3],
    skin_max: [f32; 3],
    hit_cx: f32,
    hit_cz: f32,
    hit_hx: f32,
    hit_hz: f32,
    /// Feet at or above this clear the obstacle.
    hit_top: f32,
}

/// Local AABB after the glTF node bake (`mesh.bounds()`), times the prop
/// scale. Y stays on the floor. XZ is centered on `(obs_x, obs_z)`.
/// The kill box is inset so it sits inside that mesh.
fn obstacle_shape(index: i32) -> ObstacleShape {
    let anchor_x = obs_x(index);
    let anchor_z = obs_z(index);
    match index.rem_euclid(3) {
        // crate.glb: [-0.25, 0, -0.25] .. [0.25, 0.5, 0.25]
        0 => shaped(
            "CRATE",
            CRATE_GLB,
            anchor_x,
            anchor_z,
            [1.15, 1.15, 1.15],
            [-0.25, 0.0, -0.25],
            [0.25, 0.5, 0.25],
        ),
        // barrel.glb accessor max is 0.258813 / 0.476190
        1 => shaped(
            "BARREL",
            BARREL_GLB,
            anchor_x,
            anchor_z,
            [1.2, 1.2, 1.2],
            [-0.258813, 0.0, -0.258813],
            [0.258813, 0.476190, 0.258813],
        ),
        // barrierRed.glb: [0, 0, -0.123] .. [0.25, 0.1312, 0]
        _ => shaped(
            "BARRIER",
            BARRIER_GLB,
            anchor_x,
            anchor_z,
            [4.4, 3.8, 3.4],
            [0.0, 0.0, -0.123],
            [0.25, 0.1312, 0.0],
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn shaped(
    kind: &'static str,
    path: &'static str,
    anchor_x: f32,
    anchor_z: f32,
    scale: [f32; 3],
    local_min: [f32; 3],
    local_max: [f32; 3],
) -> ObstacleShape {
    let origin = [
        anchor_x - scale[0] * (local_min[0] + local_max[0]) * 0.5,
        -scale[1] * local_min[1],
        anchor_z - scale[2] * (local_min[2] + local_max[2]) * 0.5,
    ];
    let skin_min = [
        origin[0] + scale[0] * local_min[0],
        origin[1] + scale[1] * local_min[1],
        origin[2] + scale[2] * local_min[2],
    ];
    let skin_max = [
        origin[0] + scale[0] * local_max[0],
        origin[1] + scale[1] * local_max[1],
        origin[2] + scale[2] * local_max[2],
    ];
    let hx = ((skin_max[0] - skin_min[0]) * 0.5 - HIT_INSET).max(0.05);
    let hz = ((skin_max[2] - skin_min[2]) * 0.5 - HIT_INSET).max(0.05);
    ObstacleShape {
        kind,
        path,
        origin,
        scale,
        skin_min,
        skin_max,
        hit_cx: anchor_x,
        hit_cz: anchor_z,
        hit_hx: hx,
        hit_hz: hz,
        hit_top: (skin_max[1] - skin_min[1]) * HIT_TOP,
    }
}

fn feet_hit(shape: &ObstacleShape, x: f32, y: f32, z: f32) -> bool {
    (x - shape.hit_cx).abs() <= shape.hit_hx
        && (z - shape.hit_cz).abs() <= shape.hit_hz
        && y < shape.hit_top
}

fn push_aabb_cage(doc: &mut WorldDoc, id: &str, min: [f32; 3], max: [f32; 3], color: [u32; 3]) {
    let t = 0.07;
    let cx = (min[0] + max[0]) * 0.5;
    let cy = (min[1] + max[1]) * 0.5;
    let cz = (min[2] + max[2]) * 0.5;
    let sx = (max[0] - min[0]).max(t);
    let sy = (max[1] - min[1]).max(t);
    let sz = (max[2] - min[2]).max(t);
    let corners = [
        (min[0], min[2]),
        (min[0], max[2]),
        (max[0], min[2]),
        (max[0], max[2]),
    ];
    for (i, (x, z)) in corners.iter().enumerate() {
        doc.props.push(glow(
            &format!("{id}-v{i}"),
            "box",
            "hitbox",
            [*x, cy, *z],
            [t, sy, t],
            color,
            2.6,
        ));
    }
    for (j, y) in [min[1], max[1]].iter().enumerate() {
        doc.props.push(glow(
            &format!("{id}-xa{j}"),
            "box",
            "hitbox",
            [cx, *y, min[2]],
            [sx, t, t],
            color,
            2.6,
        ));
        doc.props.push(glow(
            &format!("{id}-xb{j}"),
            "box",
            "hitbox",
            [cx, *y, max[2]],
            [sx, t, t],
            color,
            2.6,
        ));
        doc.props.push(glow(
            &format!("{id}-za{j}"),
            "box",
            "hitbox",
            [min[0], *y, cz],
            [t, t, sz],
            color,
            2.6,
        ));
        doc.props.push(glow(
            &format!("{id}-zb{j}"),
            "box",
            "hitbox",
            [max[0], *y, cz],
            [t, t, sz],
            color,
            2.6,
        ));
    }
}

/// True when `path` is a mounted or on-disk GLB that parses.
/// A miss is not cached, so a wasm page can mount the file after boot.
fn obstacle_drawable(path: &str) -> bool {
    if glb_ready(path) {
        true
    } else {
        note_missing_mesh(path);
        false
    }
}

fn note_missing_mesh(path: &str) {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut guard) = seen.lock() {
        if !guard.insert(path.to_string()) {
            return;
        }
    }
    let msg = format!("neon cat: obstacle mesh failed to load, not spawning {path}");
    #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
    web_sys::console::warn_1(&msg.into());
    #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
    eprintln!("{msg}");
}

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

    #[test]
    fn title_clears_a_crash_and_start_runs() {
        let phase = |session: &SharedSession| {
            let value: serde_json::Value = serde_json::from_str(&session.stats_json()).unwrap();
            value["game_phase"].as_str().unwrap().to_string()
        };
        let mut session = SharedSession::default();
        session
            .load_game_json(r#"{"version":1,"genre":"neon_cat"}"#)
            .unwrap();
        session.show_title();
        assert_eq!(phase(&session), "title");
        assert!(session.cat.as_ref().unwrap().z().abs() < 0.01);
        session.start_game();
        assert_eq!(phase(&session), "playing");
        for _ in 0..60 * 6 {
            session.set_play_input(0.0, 0.0, false, false, false);
            session.request_frame();
            if session.cat.as_ref().unwrap().phase_name() == "result" {
                break;
            }
        }
        assert_eq!(session.cat.as_ref().unwrap().phase_name(), "result");
        assert_eq!(phase(&session), "complete");
        let crashed = session.draw_list();
        assert!(
            crashed.texts.iter().any(|text| text.text == "HIT CRATE"),
            "the result names the obstacle, got {:?}",
            crashed
                .texts
                .iter()
                .map(|text| &text.text)
                .collect::<Vec<_>>()
        );
        assert_eq!(session.cat.as_ref().unwrap().death(), "CRATE");
        session.show_title();
        assert_eq!(phase(&session), "title");
        assert!(
            session.cat.as_ref().unwrap().z().abs() < 0.01,
            "title puts the cat back at the start, not the crash arch"
        );
        let titled = session.draw_list();
        assert!(titled.texts.iter().all(|text| text.text != "CRASH"));
        session.set_external_menu(true);
        let hosted = session.draw_list();
        assert!(
            hosted
                .texts
                .iter()
                .all(|text| text.text != "CRASH" && text.text != "START" && text.text != "TITLE"),
            "the HTML card owns the menu"
        );
        session.start_game();
        let z0 = session.cat.as_ref().unwrap().z();
        for _ in 0..30 {
            session.request_frame();
        }
        assert_eq!(phase(&session), "playing");
        assert!(session.cat.as_ref().unwrap().z() > z0 + 1.0);
    }

    fn place_on(index: i32, x: f32, y: f32) -> NeonCat {
        let mut run = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        run.confirm();
        run.time = GRACE_SECS + 0.05;
        run.x = x;
        run.y = y;
        // One tick advances z by ~0.18. Start behind the anchor so the
        // thin barrier (half ~0.15) is still under the feet.
        run.z = obs_z(index) - 0.12;
        run.vx = 0.0;
        run.vy = 0.0;
        run
    }

    #[test]
    fn hitboxes_sit_inside_the_glb_and_a_graze_lives() {
        for index in 0..3 {
            let shape = obstacle_shape(index);
            let file = crate::assets::resolve_asset(shape.path)
                .unwrap_or_else(|| panic!("missing {}", shape.path));
            let bytes = crate::assets::read_bytes(&file).unwrap();
            let bounds = crate::gltf_load::mesh_from_glb(&bytes).unwrap().bounds();
            let vmin = [
                shape.origin[0] + shape.scale[0] * bounds.min.x,
                shape.origin[1] + shape.scale[1] * bounds.min.y,
                shape.origin[2] + shape.scale[2] * bounds.min.z,
            ];
            let vmax = [
                shape.origin[0] + shape.scale[0] * bounds.max.x,
                shape.origin[1] + shape.scale[1] * bounds.max.y,
                shape.origin[2] + shape.scale[2] * bounds.max.z,
            ];
            let hmin = [
                shape.hit_cx - shape.hit_hx,
                0.0,
                shape.hit_cz - shape.hit_hz,
            ];
            let hmax = [
                shape.hit_cx + shape.hit_hx,
                shape.hit_top,
                shape.hit_cz + shape.hit_hz,
            ];
            for axis in 0..3 {
                assert!(
                    hmin[axis] >= vmin[axis] - 1e-3,
                    "{} axis {axis} hit {} under mesh {}",
                    shape.kind,
                    hmin[axis],
                    vmin[axis]
                );
                assert!(
                    hmax[axis] <= vmax[axis] + 1e-3,
                    "{} axis {axis} hit {} past mesh {}",
                    shape.kind,
                    hmax[axis],
                    vmax[axis]
                );
            }
            assert!(
                hmax[0] - hmin[0] < (vmax[0] - vmin[0]) - 0.08,
                "{} x hit should be inset",
                shape.kind
            );
            assert!(
                hmax[2] - hmin[2] < (vmax[2] - vmin[2]) - 0.08,
                "{} z hit should be inset",
                shape.kind
            );
            assert!(
                hmax[1] < vmax[1] - 0.02,
                "{} top {} should sit under the mesh {}",
                shape.kind,
                hmax[1],
                vmax[1]
            );
        }

        // 0.40 is outside the crate mesh (half ~0.29) and inside the old pad.
        let crate_shape = obstacle_shape(0);
        assert!(!feet_hit(&crate_shape, obs_x(0) + 0.40, 0.0, obs_z(0)));
        assert!(feet_hit(&crate_shape, obs_x(0), 0.0, obs_z(0)));
        assert!(!feet_hit(
            &crate_shape,
            obs_x(0),
            crate_shape.hit_top,
            obs_z(0)
        ));

        let mut graze = place_on(0, obs_x(0) + 0.40, 0.0);
        graze.tick(1.0 / 60.0);
        assert!(graze.alive, "a point beside the crate is not a hit");
        assert!(graze.death().is_empty());

        let mut hit = place_on(0, obs_x(0), 0.0);
        hit.tick(1.0 / 60.0);
        assert!(!hit.alive);
        assert_eq!(hit.death(), "CRATE");
        for _ in 0..12 {
            hit.tick(1.0 / 60.0);
        }
        assert_eq!(hit.phase_name(), "result");
        assert_eq!(hit.flow.subtitle, "HIT CRATE");

        let mut barrel = place_on(1, obs_x(1), 0.0);
        barrel.tick(1.0 / 60.0);
        assert_eq!(barrel.death(), "BARREL");
        let mut barrier = place_on(2, obs_x(2), 0.0);
        barrier.tick(1.0 / 60.0);
        assert_eq!(barrier.death(), "BARRIER");

        let mut jumped = place_on(0, obs_x(0), crate_shape.hit_top + 0.05);
        jumped.tick(1.0 / 60.0);
        assert!(jumped.alive, "feet above the crate clear it");
    }

    #[test]
    fn grace_and_the_rail_and_the_arch_do_not_kill() {
        let mut spawn = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        spawn.confirm();
        spawn.x = obs_x(0);
        spawn.z = obs_z(0);
        spawn.time = 0.2;
        spawn.tick(1.0 / 60.0);
        assert!(
            spawn.alive,
            "grace skips the obstacle under the spawn stretch"
        );

        let mut rail = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        rail.confirm();
        rail.set_steer(1.0, 0.0, false);
        for _ in 0..60 * 4 {
            rail.tick(1.0 / 60.0);
        }
        assert!(rail.alive, "full steer used to die on the empty wall");
        assert!(rail.x().abs() <= LANE_LIMIT + 1e-3, "x {}", rail.x());
        assert_eq!(rail.phase_name(), "play");
        assert!(rail.death().is_empty());

        let mut arch = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        arch.confirm();
        arch.time = GRACE_SECS + 0.2;
        arch.x = 2.05;
        arch.z = gate_z(0);
        arch.tick(1.0 / 60.0);
        assert!(
            arch.alive,
            "the arch pillar and the old wall plane are not a collider"
        );
        assert!(arch.death().is_empty());
    }

    #[test]
    fn debug_overlay_draws_hitboxes() {
        let mut run = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        assert!(run
            .document()
            .props
            .iter()
            .all(|prop| prop.name != "hitbox"));
        run.set_debug(true);
        assert!(run
            .document()
            .props
            .iter()
            .any(|prop| prop.name == "hitbox"));
        let state: serde_json::Value = serde_json::from_str(&run.state_json()).unwrap();
        assert_eq!(state["death"], "");
        run.set_debug(false);
        assert!(run
            .document()
            .props
            .iter()
            .all(|prop| prop.name != "hitbox"));
    }

    #[test]
    fn a_missing_mesh_is_not_a_collider() {
        assert!(!obstacle_drawable(
            "assets/models/kenney/platformer-kit/not-a-real-obstacle.glb"
        ));
        let mut run = place_on(0, obs_x(0), 0.0);
        run.spawned.clear();
        run.touch_obstacles();
        assert!(run.alive, "no drawn mesh, no kill");
        assert!(run.death().is_empty());
    }

    #[test]
    fn every_collider_has_a_drawn_mesh_on_phone_tiers() {
        let aspect = 412.0 / 915.0;
        for quality in ["mobile", "low"] {
            let json = format!(r#"{{"version":1,"genre":"neon_cat","quality":"{quality}"}}"#);
            let mut run = NeonCat::from_json(&json).unwrap();
            run.confirm();
            run.z = obs_z(0) - 8.0;
            run.rebuild();
            assert_eq!(run.document().quality.as_deref(), Some(quality));
            let doc = run.document();
            if quality == "mobile" {
                assert_eq!(doc.ssao_radius(), 4.0);
                assert_eq!(doc.shadow_cascade_count(), 2);
            } else {
                assert_eq!(doc.ssao_radius(), 0.0);
                assert_eq!(doc.shadow_cascade_count(), 1);
            }
            let scene = doc.compile_scene(aspect);
            let compiled = doc.compile_meshes();
            for index in 0..3 {
                let shape = obstacle_shape(index);
                assert!(
                    run.spawned.contains(&index),
                    "{} was not spawned",
                    shape.kind
                );
                let prop = doc
                    .props
                    .iter()
                    .find(|prop| prop.id == format!("prop:obs-{index}"))
                    .unwrap_or_else(|| panic!("{} missing from the dump", shape.kind));
                assert_eq!(prop.gltf.as_deref(), Some(shape.path));
                let file = crate::assets::resolve_asset(shape.path).unwrap();
                let bytes = crate::assets::read_bytes(&file).unwrap();
                let loaded = crate::gltf_load::mesh_from_glb(&bytes).unwrap();
                assert!(
                    loaded.indices.len() > 36,
                    "{} compiled as the unit-cube alias",
                    shape.kind
                );
                assert!(
                    loaded.alpha_cut < 0.0,
                    "{} alpha_cut {} would discard the mesh",
                    shape.kind,
                    loaded.alpha_cut
                );
                if shape.kind != "BARRIER" {
                    assert!(
                        loaded.albedo.is_some(),
                        "{} colormap did not decode",
                        shape.kind
                    );
                }
                let (id, mesh) = compiled
                    .iter()
                    .find(|(_, mesh)| {
                        mesh.indices.len() == loaded.indices.len()
                            && mesh.vertices.len() == loaded.vertices.len()
                    })
                    .unwrap_or_else(|| panic!("{} is not a compiled mesh", shape.kind));
                assert!(
                    mesh.alpha_cut < 0.0,
                    "drawn {} alpha_cut {}",
                    shape.kind,
                    mesh.alpha_cut
                );
                assert!(
                    scene
                        .batches
                        .iter()
                        .any(|batch| batch.mesh == *id && !batch.instances.is_empty()),
                    "{} was culled at portrait aspect on {quality}",
                    shape.kind
                );
            }
        }
    }

    fn portrait_ndc(run: &NeonCat, point: [f32; 3]) -> glam::Vec3 {
        let aspect = 412.0 / 915.0;
        let scene = run.document().compile_scene(aspect);
        let clip = scene.camera.view_projection(aspect) * glam::Vec3::from_array(point).extend(1.0);
        assert!(clip.w > 0.2, "point {point:?} is behind the camera");
        clip.truncate() / clip.w
    }

    #[test]
    fn chase_camera_springs_ahead_banks_and_opens_with_speed() {
        let mut run = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        run.confirm();
        let cam = &run.document().cameras[0];
        assert_eq!(cam.name, "chase");
        assert!((cam.position[2] - (0.0 - CHASE_BACK)).abs() < 1e-3);
        assert!((cam.position[1] - CHASE_LIFT).abs() < 1e-3);
        assert!((cam.target[2] - CHASE_LOOK).abs() < 1e-3);
        assert!(cam.target[2] > cam.position[2] + 8.0);
        assert!((cam.fov - CHASE_FOV).abs() < 1e-3);
        assert!(cam.roll.abs() < 1e-5);

        run.x = 1.4;
        for _ in 0..18 {
            run.smooth_cam(1.0 / 60.0);
        }
        run.rebuild();
        let eye_x = run.document().cameras[0].position[0];
        assert!(
            (0.5..1.35).contains(&eye_x),
            "spring should be following 1.4 but still lagging, eye x {eye_x}"
        );

        run.time = 36.0;
        let before = run.document().cameras[0].fov;
        for _ in 0..200 {
            run.smooth_cam(1.0 / 60.0);
        }
        run.rebuild();
        let opened = run.document().cameras[0].fov;
        assert!(
            opened > before + 4.0,
            "fov {before} should widen with speed, got {opened}"
        );
        assert!(opened < CHASE_FOV_FAST + 0.2);

        run.vx = -5.6;
        for _ in 0..40 {
            run.smooth_cam(1.0 / 60.0);
        }
        run.rebuild();
        let roll = run.document().cameras[0].roll;
        assert!(
            roll < -0.02 && roll > -0.09,
            "screen-right steer banks the rig, roll {roll}"
        );
        let scene = run.document().compile_scene(412.0 / 915.0);
        assert!(
            (scene.camera.up - glam::Vec3::Y).length() > 0.01,
            "roll must tilt the camera up, got {:?}",
            scene.camera.up
        );
        let fwd = run.cam_forward();
        assert!(fwd[2] > 0.9, "the rig still looks down the tunnel, {fwd:?}");
    }

    #[test]
    fn classic_cam_keeps_the_old_rig() {
        let mut run =
            NeonCat::from_json(r#"{"version":1,"genre":"neon_cat","cam":"classic"}"#).unwrap();
        run.confirm();
        run.z = 10.0;
        run.y = 0.4;
        run.rebuild();
        let cam = &run.document().cameras[0];
        assert_eq!(cam.name, "classic");
        assert!((cam.position[2] - (10.0 - CAM_BACK)).abs() < 1e-3);
        assert!((cam.position[1] - (CAM_LIFT + 0.4 * 0.35)).abs() < 1e-3);
        assert!((cam.target[1] - 0.85).abs() < 1e-3);
        assert!((cam.target[2] - 22.0).abs() < 1e-3);
        assert!((cam.fov - CAM_FOV).abs() < 1e-4);
        assert!(cam.roll.abs() < 1e-6);
        run.vx = -5.6;
        run.time = 40.0;
        for _ in 0..30 {
            run.smooth_cam(1.0 / 60.0);
        }
        run.rebuild();
        let held = &run.document().cameras[0];
        assert!((held.fov - CAM_FOV).abs() < 1e-4);
        assert!(held.roll.abs() < 1e-6);
    }

    #[test]
    fn landing_bump_uses_the_juice_shake() {
        let mut run = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        run.confirm();
        run.y = 0.05;
        run.vy = -6.0;
        run.tick(1.0 / 60.0);
        assert_eq!(run.y(), 0.0);
        let bump = run.land.offset();
        let energy = bump[0].abs() + bump[1].abs() + bump[2].abs();
        assert!(
            energy > 1e-3,
            "landing should start the juice offset, got {bump:?}"
        );
        let eye = run.document().cameras[0].position;
        let target = run.document().cameras[0].target;
        assert!(
            (eye[0] - target[0] - (run.eye_x.pos - run.look_x.pos)).abs() < 1e-3,
            "the bump translates the eye and the look target together"
        );

        let mut classic =
            NeonCat::from_json(r#"{"version":1,"genre":"neon_cat","cam":"classic"}"#).unwrap();
        classic.confirm();
        classic.y = 0.05;
        classic.vy = -6.0;
        classic.tick(1.0 / 60.0);
        classic.tick(1.0 / 60.0);
        let still = classic.land.offset();
        assert!(
            still.iter().all(|v| v.abs() < 1e-6),
            "classic keeps the old camera, got {still:?}"
        );
    }

    #[test]
    fn portrait_phone_keeps_the_next_obstacle_in_frame() {
        let mut run =
            NeonCat::from_json(r#"{"version":1,"genre":"neon_cat","quality":"mobile"}"#).unwrap();
        run.confirm();
        for _ in 0..60 * 6 {
            run.set_steer(0.0, 0.0, false);
            run.tick(1.0 / 60.0);
            if (obs_z(0) - run.z() - 8.0).abs() < 0.35 {
                break;
            }
        }
        assert!(
            (obs_z(0) - run.z() - 8.0).abs() < 1.0,
            "z {} never reached the approach",
            run.z()
        );
        assert!(run.alive);
        let shape = obstacle_shape(0);
        let obs = portrait_ndc(&run, [shape.hit_cx, shape.hit_top * 0.45, shape.hit_cz]);
        assert!(
            obs.x.abs() < 0.55,
            "center crate should stay readable, ndc {:?}",
            obs
        );
        assert!(
            obs.y > -0.45 && obs.y < 0.45,
            "crate should sit in the tunnel ahead, ndc {:?}",
            obs
        );
        let cat = portrait_ndc(&run, [run.x(), run.y() + 0.45, run.z()]);
        assert!(
            cat.y < -0.15 && cat.y > -0.9,
            "the cat stays in the lower frame, ndc {:?}",
            cat
        );

        run.x = -LANE_LIMIT;
        run.vx = -5.6;
        for _ in 0..24 {
            run.smooth_cam(1.0 / 60.0);
        }
        run.rebuild();
        let edged = portrait_ndc(&run, [shape.hit_cx, shape.hit_top * 0.45, shape.hit_cz]);
        assert!(
            edged.x.abs() < 0.85,
            "a rail-side cat should still see the center crate, ndc {:?}",
            edged
        );
    }
}
