//! Neon Cat Run. Not a WorldPlay genre.
//!
//! The cat, the arches, the rings, and the low obstacles are CC0 glTF.
//! See `docs/assets.md` and each pack's `CREDITS.md`. The tunnel grid,
//! the wall wireframe, and the magenta rails are screen-space fat lines:
//! no CC0 pack is an infinite synthwave shader. A missing GLB leaves the
//! engine fallback (a capsule for the cat, no prop for the rest). It does
//! not invent a box cat.
//!
//! The chase camera looks along +Z. Screen-right is world -X.
//! `D` / `+lx` steer that way. Space, the jump button, and stick-up jump.
//! The default rig is a damped third person behind and above the cat.
//! `cam: "classic"` keeps the older fixed chase.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use serde_json::json;

use crate::flow::{FlowAction, FlowPhase, GameFlow};
use crate::game_core::{self, Core, Hitbox, Juice, JuiceShake, Kit, Spring1};
use crate::game_spec::GameSpec;
use crate::scene::DrawList;
use crate::world_doc::{WorldCamera, WorldDoc, WorldLight, WorldProp, WorldWalker};

pub const GAME_ID: &str = "neon_cat";

use crate::game_spec::InputScheme;
use crate::gesture::{DragEvent, DragFlick, Gesture, InputBuffer, Swipe, SwipeRecognizer};

/// How long a swipe waits for the cat to be able to act on it.
pub const SWIPE_BUFFER_SECS: f32 = 0.15;

#[path = "neon_cat_measure.rs"]
pub mod measure;

#[derive(Debug)]
pub struct NeonCat {
    /// Everything tunable. Loaded from the world dump's `"game"` key.
    spec: std::sync::Arc<GameSpec>,
    pub flow: GameFlow,
    /// Score, death, and juice (shared core).
    core: Core,
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
    cam_x: f32,
    eye_x: Spring1,
    eye_y: Spring1,
    look_x: Spring1,
    bank: Spring1,
    fov_spring: Spring1,
    attract: f32,
    got: HashSet<i32>,
    doc: WorldDoc,
    /// Swipes wait here up to 0.15 s (jump just before landing).
    swipes: InputBuffer<Swipe>,
    /// Lane a left/right swipe is heading to (world x).
    lane_target: Option<f32>,
    /// Touch recognisers; `spec.input.scheme` picks which one runs.
    drag: DragFlick,
    swiper: SwipeRecognizer,
    /// Drag scheme: cat x when the finger went down, and where it follows now.
    drag_base: f32,
    drag_target: Option<f32>,
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

fn ring_x(s: &GameSpec, index: i32) -> f32 {
    if index <= 0 {
        return 0.0;
    }
    s.runner.rings.track.x(index)
}

fn near_index(z: f32, origin: f32, gap: f32) -> i32 {
    ((z - origin) / gap).floor() as i32
}

/// Index of the next marker that is still in front of `z`.
fn next_ahead(z: f32, origin: f32, gap: f32, at: impl Fn(i32) -> f32) -> i32 {
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
        let spec = match value.get("game") {
            Some(runner) => GameSpec::from_value(runner)?,
            None => GameSpec::default(),
        };
        let mut run = Self::with_spec(spec);
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

    #[cfg(test)]
    fn fresh() -> Self {
        Self::with_spec(GameSpec::default())
    }

    fn with_spec(spec: GameSpec) -> Self {
        let mut flow = GameFlow::new(GAME_ID);
        flow.title = spec.title.clone();
        flow.tagline = "RUN  ·  JUMP".into();
        let seed = spec.seed;
        let mut run = Self {
            spec: std::sync::Arc::new(spec),
            flow,
            core: Core {
                juice: Juice::seeded(seed),
                ..Core::default()
            },
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
            cam_x: 0.0,
            eye_x: Spring1::default(),
            eye_y: Spring1::default(),
            look_x: Spring1::default(),
            bank: Spring1::default(),
            fov_spring: Spring1::default(),
            attract: 0.0,
            got: HashSet::new(),
            doc: WorldDoc::default(),
            swipes: InputBuffer::new(SWIPE_BUFFER_SECS),
            lane_target: None,
            drag: DragFlick::default(),
            swiper: SwipeRecognizer::default(),
            drag_base: 0.0,
            drag_target: None,
        };
        run.drag.flick_dist = run.spec.input.flick_dist;
        run.drag.flick_angle_deg = run.spec.input.flick_angle_deg;
        run.swiper.min_dist = run.spec.input.swipe_dist;
        run.swiper.cooldown = run.spec.input.swipe_cooldown_secs;
        run.swipes.window = run.spec.input.buffer_secs;
        run.snap_chase();
        run
    }

    /// The loaded runner spec.
    pub fn spec(&self) -> &GameSpec {
        &self.spec
    }

    pub fn score(&self) -> u32 {
        self.core.stats.score
    }

    pub fn rings(&self) -> u32 {
        self.core.stats.rings
    }

    pub fn best_score(&self) -> u32 {
        self.flow.best_score
    }

    pub fn phase_name(&self) -> &'static str {
        self.flow.phase.name()
    }

    pub fn alive(&self) -> bool {
        self.core.stats.alive
    }

    pub fn death(&self) -> &str {
        &self.core.stats.death
    }

    /// `?debug=1` draws the kill boxes. They are not colliders.
    pub fn set_debug(&mut self, on: bool) {
        self.debug = on;
        self.rebuild();
    }

    pub fn playing(&self) -> bool {
        self.flow.phase == FlowPhase::Play && self.core.stats.alive
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
            "crash": self.core.stats.just_crash,
            "rings": self.core.stats.just_rings,
            "jump": self.core.stats.just_jump,
            "x": self.x,
            "y": self.y,
            "z": self.z,
        });
        self.core.stats.just_crash = false;
        self.core.stats.just_rings = 0;
        self.core.stats.just_jump = false;
        body.to_string()
    }

    fn sim_value(&self) -> serde_json::Value {
        let oi = next_ahead(
            self.z,
            self.spec.runner.obstacle_track.z0,
            self.spec.runner.obstacle_track.gap,
            |i| self.spec.runner.obstacle_track.z(i),
        );
        let ri = next_ahead(
            self.z,
            self.spec.runner.rings.track.z0,
            self.spec.runner.rings.track.gap,
            |i| self.spec.runner.rings.track.z(i),
        );
        let ahead = obstacle_shape(&self.spec, oi);
        let (ox, oz, oh) = (ahead.hit_cx, ahead.hit_cz, ahead.hit_top);
        json!({
            "phase": self.flow.phase.name(),
            "score": self.core.stats.score,
            "best": self.flow.best_score,
            "rings": self.core.stats.rings,
            "alive": self.core.stats.alive,
            "death": self.core.stats.death,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "screen_right": self.screen_right().to_array(),
            "screen_left": (-self.screen_right()).to_array(),
            "next_ring_x": ring_x(&self.spec, ri),
            "next_ring_z": self.spec.runner.rings.track.z(ri),
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
            self.spec.camera.chase_look_y + self.y * 0.15 + bump[1],
            self.z + self.spec.camera.chase_look + bump[2],
        ]
    }

    fn chase_eye(&self) -> [f32; 3] {
        if self.classic {
            let shake = (self.time * 53.0).sin() * self.core.juice.shake;
            return [
                self.cam_x + shake,
                self.spec.camera.classic_lift + self.y * 0.35 + shake * 0.25,
                self.z - self.spec.camera.classic_back,
            ];
        }
        let bump = self.view_bump();
        [
            self.eye_x.pos + bump[0],
            self.eye_y.pos + bump[1],
            self.z - self.spec.camera.chase_back + bump[2],
        ]
    }

    /// Crash shake plus the landing juice offset. Added to the eye and the
    /// look target so the bump translates the rig.
    fn view_bump(&self) -> [f32; 3] {
        let shake = (self.time * 53.0).sin() * self.core.juice.shake;
        let land = self.core.juice.land.offset();
        [land[0] + shake, land[1] + shake * 0.25, land[2]]
    }

    fn chase_fov(&self) -> f32 {
        if self.classic {
            self.spec.camera.classic_fov
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
        let span = (self.spec.runner.speed.cap - self.spec.runner.speed.start).max(0.01);
        let t = ((self.speed() - self.spec.runner.speed.start) / span).clamp(0.0, 1.0);
        self.spec.camera.fov + t * (self.spec.camera.fov_fast - self.spec.camera.fov)
    }

    fn snap_chase(&mut self) {
        self.eye_x.snap(self.x);
        self.eye_y.snap(self.spec.camera.chase_lift + self.y * 0.42);
        self.look_x.snap(self.x * 0.9);
        self.bank.snap(0.0);
        self.fov_spring.snap(self.fov_for_speed());
        self.cam_x = self.x * 0.72;
    }

    /// Screen-right steer toward the next ring, and jump when the next
    /// obstacle is close and lined up. `+lx` is world −X on this camera.
    pub fn script_steer(&self) -> (f32, bool) {
        let ri = next_ahead(
            self.z,
            self.spec.runner.rings.track.z0,
            self.spec.runner.rings.track.gap,
            |i| self.spec.runner.rings.track.z(i),
        );
        let dx = ring_x(&self.spec, ri) - self.x;
        let lx = (-dx * 1.4).clamp(-1.0, 1.0);
        let oi = next_ahead(
            self.z,
            self.spec.runner.obstacle_track.z0,
            self.spec.runner.obstacle_track.gap,
            |i| self.spec.runner.obstacle_track.z(i),
        );
        let ahead_obs = obstacle_shape(&self.spec, oi);
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
        self.core.stats.alive = true;
        self.core.stats.death.clear();
        self.core.stats.score = 0;
        self.core.stats.score_acc = 0.0;
        self.core.stats.rings = 0;
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
        self.core.juice.sparks.clear();
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
        self.core.stats.score = 0;
        self.core.stats.score_acc = 0.0;
        self.core.stats.rings = 0;
        self.core.stats.alive = true;
        self.core.stats.death.clear();
        self.got.clear();
        self.core.juice.sparks.clear();
        self.time = 0.0;
        self.swipes.clear();
        self.lane_target = None;
        self.drag_target = None;
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
        self.core.juice.hitstop = 0.0;
        self.core.juice.shake = 0.0;
        self.core.juice.land = JuiceShake::default();
        self.cam_x = 0.0;
        self.snap_chase();
    }

    fn axis_x(&self) -> f32 {
        let kx = (self.key_right as i32 - self.key_left as i32) as f32;
        let held = (kx + self.pad_x).clamp(-1.0, 1.0);
        match self.lane_target {
            // Screen-right is world -X, so steer = -axis heads to the target.
            Some(t) if held.abs() < 0.2 => ((self.x - t) * 3.0).clamp(-1.0, 1.0),
            _ => held,
        }
    }

    /// Outermost lane centre (world |x|).
    fn lane_max(&self) -> f32 {
        let l = &self.spec.runner.lanes;
        (l.limit / l.width.max(0.1)).floor() * l.width.max(0.1)
    }

    /// A finger is on the screen (hosts drop their stick/jump zones).
    pub fn touch_active(&self) -> bool {
        self.drag.active() || self.swiper.active()
    }

    /// One pointer event in surface pixels; `t` is any seconds clock.
    /// `spec.input.scheme` decides drag-to-steer or swipe-per-lane.
    pub fn touch(&mut self, ev: crate::input::PointerEvent, w: u32, h: u32, t: f32) {
        match self.spec.input.scheme {
            InputScheme::Swipe => {
                if let Some(Gesture::Swipe(dir)) = self.swiper.push(ev, w, h, t) {
                    self.swipe(dir);
                }
            }
            InputScheme::Drag => {
                for e in self.drag.push(ev, w, h, t) {
                    self.drag_event(e);
                }
            }
        }
    }

    /// Drag scheme. The cat's x follows the finger's horizontal travel:
    /// `input.drag_lane_frac` of the short side is one lane.
    pub fn drag_event(&mut self, e: DragEvent) {
        let playing = matches!(self.flow.phase, FlowPhase::Play) && self.core.stats.alive;
        match e {
            DragEvent::Start => {
                self.drag_base = self.lane_target.unwrap_or(self.x);
                self.lane_target = None;
                self.drag_target = playing.then_some(self.drag_base);
            }
            DragEvent::Drag(dx) if playing => {
                let w = self.spec.runner.lanes.width.max(0.1);
                let lanes = dx / self.spec.input.drag_lane_frac.max(0.01);
                // Screen-right is world -X on this camera.
                let m = self.lane_max();
                self.drag_target = Some((self.drag_base - lanes * w).clamp(-m, m));
            }
            DragEvent::Drag(_) => {}
            DragEvent::Flick(dir) => self.swipe(dir),
            DragEvent::Release => {
                if let Some(t) = self.drag_target.take() {
                    if self.spec.input.drag_snap && playing {
                        let w = self.spec.runner.lanes.width.max(0.1);
                        let m = self.lane_max();
                        self.lane_target = Some(((t / w).round() * w).clamp(-m, m));
                    }
                }
            }
        }
    }

    /// One recognised swipe. Left/right move one lane, up jumps (buffered),
    /// down fast-falls in the air. Taps and swipes outside play do nothing.
    pub fn swipe(&mut self, dir: Swipe) {
        if !matches!(self.flow.phase, FlowPhase::Play) || !self.core.stats.alive {
            return;
        }
        let lanes = &self.spec.runner.lanes;
        let w = lanes.width.max(0.1);
        match dir {
            Swipe::Left | Swipe::Right => {
                let from = self.lane_target.unwrap_or((self.x / w).round() * w);
                // Screen-left is world +X on this camera.
                let step = if dir == Swipe::Left { w } else { -w };
                let m = (lanes.limit / w).floor() * w;
                self.lane_target = Some((from + step).clamp(-m, m));
            }
            Swipe::Up => self.swipes.press(Swipe::Up, self.time),
            Swipe::Down => {
                if self.y > 0.05 {
                    self.vy = self.vy.min(-self.spec.runner.jump.velocity * 1.2);
                } else {
                    // Roll arrives with the gameplay PR; keep it buffered.
                    self.swipes.press(Swipe::Down, self.time);
                }
            }
        }
    }

    pub fn tick(&mut self, dt: f32) {
        game_core::tick_frame(self, dt);
    }

    fn play_step(&mut self, dt: f32) {
        self.time += dt;
        let ax = self.axis_x();
        // +ax is screen-right. This camera looks along +Z, so that is world -X.
        let steer = -ax;
        let response = 1.0 - (-dt * 8.0).exp();
        self.vx += (steer * self.spec.runner.lanes.steer - self.vx) * response;
        if let Some(t) = self.drag_target {
            let k = 1.0 - (-dt / self.spec.input.drag_smooth_secs.max(1e-3)).exp();
            self.vx = (t - self.x) * k / dt.max(1e-4);
        } else if let Some(t) = self.lane_target {
            // Ease into the lane centre without overshoot, capped at steer speed.
            let held = (self.key_right as i32 - self.key_left as i32) as f32 + self.pad_x;
            if held.abs() < 0.2 {
                let k = 1.0 - (-dt / 0.06).exp();
                let cap = self.spec.runner.lanes.steer * 1.5;
                self.vx = ((t - self.x) * k / dt.max(1e-4)).clamp(-cap, cap);
            }
        }
        let grounded = self.y <= 0.001 && self.vy <= 0.0;
        if let Some(t) = self.lane_target {
            let held = (self.key_right as i32 - self.key_left as i32) as f32 + self.pad_x;
            if held.abs() >= 0.2 || (self.x - t).abs() < 0.02 {
                self.lane_target = None;
            }
        }
        let want_jump = self.jump_held || self.key_jump || self.pad_up > 0.55;
        if grounded && (want_jump || self.swipes.take(Swipe::Up, self.time)) {
            self.vy = self.spec.runner.jump.velocity;
            self.y = 0.02;
            self.core.stats.just_jump = true;
            crate::mixer::cue_sfx("jump");
        }
        self.vy += self.spec.runner.jump.gravity * dt;
        self.y += self.vy * dt;
        if self.y <= 0.0 {
            if self.vy < -1.5 {
                self.core.juice.land_squash = 0.12;
                if !self.classic {
                    // Same defaults as `kagra.juice.Juice.shake`.
                    self.core.juice.land.shake(0.08, 0.18);
                }
            }
            self.y = 0.0;
            self.vy = 0.0;
        }
        self.core.juice.land_squash = (self.core.juice.land_squash - dt).max(0.0);
        self.x += self.vx * dt;
        if self.x.abs() > self.spec.runner.lanes.limit {
            self.x = self.x.signum() * self.spec.runner.lanes.limit;
            if self.x.is_sign_positive() {
                self.vx = self.vx.min(0.0);
            } else {
                self.vx = self.vx.max(0.0);
            }
        }
        self.yaw += (self.vx * 0.14 - self.yaw) * response;
        let speed = self.speed();
        self.z += speed * dt;
        self.core.stats.add_points(speed * dt * 2.2);
        self.anim_t += dt * if self.y > 0.05 { 0.7 } else { 1.15 };
        // Grace covers the spawn stretch. The rail is not a kill plane,
        // so steering into it during grace cannot strand the cat outside.
        if self.time >= self.spec.runner.jump.grace_secs {
            self.touch_obstacles();
        }
        if self.core.stats.alive {
            self.collect_rings();
        }
    }

    /// Obstacles drawn ahead. At least `self.spec.runner.draw.lead_secs` of travel, never fewer than 4.
    fn obstacle_lead_count(&self) -> i32 {
        ((self.speed() * self.spec.runner.draw.lead_secs / self.spec.runner.obstacle_track.gap)
            .ceil() as i32
            + 1)
        .max(self.spec.runner.draw.min_obstacles_ahead)
    }

    /// Tunnel floor/walls reach past the farthest drawn obstacle.
    fn tunnel_reach(&self) -> f32 {
        (self.speed() * self.spec.runner.draw.lead_secs + self.spec.runner.obstacle_track.gap * 2.0)
            .max(44.0)
    }

    fn speed(&self) -> f32 {
        (self.spec.runner.speed.start + self.time * self.spec.runner.speed.ramp)
            .min(self.spec.runner.speed.cap)
    }

    fn smooth_cam(&mut self, dt: f32) {
        if self.classic {
            let k = 1.0 - (-dt * 4.5).exp();
            self.cam_x += (self.x * 0.72 - self.cam_x) * k;
            return;
        }
        let want_y = self.spec.camera.chase_lift + self.y * 0.42;
        // Held steer keeps the bank after the rail zeros vx. A bare
        // lateral velocity still banks, which is what the tests set.
        let steer = if self.axis_x().abs() > 0.2 {
            -self.axis_x()
        } else {
            (self.vx / self.spec.runner.lanes.steer).clamp(-1.0, 1.0)
        };
        let want_roll = steer * self.spec.camera.roll;
        self.eye_x.step(self.x, 12.0, dt);
        self.eye_y.step(want_y, 8.0, dt);
        self.look_x.step(self.x * 0.9, 8.0, dt);
        self.bank.step(want_roll, 5.5, dt);
        self.fov_spring.step(self.fov_for_speed(), 2.4, dt);
    }

    fn touch_obstacles(&mut self) {
        let i0 = near_index(
            self.z,
            self.spec.runner.obstacle_track.z0,
            self.spec.runner.obstacle_track.gap,
        )
        .saturating_sub(1);
        for i in i0..i0 + 3 {
            if i < 0 {
                continue;
            }
            if !self.spawned.contains(&i) {
                continue;
            }
            let shape = obstacle_shape(&self.spec, i);
            if shape.feet_hit(self.x, self.y, self.z) {
                self.crash(&shape.kind);
                return;
            }
        }
    }

    fn collect_rings(&mut self) {
        let i0 = near_index(
            self.z,
            self.spec.runner.rings.track.z0,
            self.spec.runner.rings.track.gap,
        )
        .saturating_sub(1);
        for i in i0..i0 + 3 {
            if i < 0 || self.got.contains(&i) {
                continue;
            }
            let dx = self.x - ring_x(&self.spec, i);
            let dz = self.z - self.spec.runner.rings.track.z(i);
            if dx * dx + dz * dz > 1.15 * 1.15 {
                continue;
            }
            self.got.insert(i);
            self.core.stats.pickup(self.spec.runner.rings.score);
            self.core.juice.shake = self.core.juice.shake.max(0.06);
            self.burst(self.x, self.y + 0.6, self.z, 14, [80, 255, 255], 3.4, 0.12);
            crate::mixer::cue_sfx("card");
        }
    }

    fn crash(&mut self, reason: &str) {
        if !self.core.stats.die(reason) {
            return;
        }
        self.core.juice.hit(0.46, 0.11);
        self.vy = 0.0;
        self.burst(self.x, self.y + 0.4, self.z, 18, [255, 60, 170], 4.6, 0.16);
        self.burst(self.x, self.y + 0.5, self.z, 10, [90, 240, 255], 3.2, 0.12);
        crate::mixer::cue_sfx("fire");
    }

    #[allow(clippy::too_many_arguments)]
    fn burst(&mut self, x: f32, y: f32, z: f32, n: u32, color: [u32; 3], speed: f32, size: f32) {
        self.core.juice.burst(x, y, z, n, color, speed, size);
    }

    fn rebuild(&mut self) {
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: 48.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some(self.spec.style.sky),
            hemi_sky: Some([0.035, 0.028, 0.07]),
            hemi_ground: Some([0.012, 0.008, 0.02]),
            env_map: Some("preset:studio".into()),
            shadow_softness: Some(1.0),
            // Thin lines carry the glow. A low intensity and a high
            // threshold keep the stone, the wood, and the sky from hazing.
            bloom: Some(0.16),
            bloom_threshold: Some(1.4),
            vignette: Some(0.28),
            lut: Some("off".into()),
            lut_strength: Some(0.0),
            quality: self.quality.clone(),
            fxaa: Some(true),
            exposure: Some(0.70),
            ibl: Some(0.06),
            tonemap: Some(true),
            cull: Some(true),
            fog_start: Some(self.spec.runner.draw.fog_start),
            fog_end: Some(self.spec.runner.draw.fog_end),
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
            [eye[0] - 1.2, eye[1] + 5.5, eye[2] - 3.0],
            [0.12, -0.78, 0.55],
            [0.32, 0.28, 0.48],
            0.42,
            36.0,
            true,
        ));
        // Tight lights on the cat. The floor albedo is near black, so they
        // do not paint a streak. Radius stays under 1.6 m.
        doc.lights.push(light(
            1,
            "fill",
            [self.x, self.y + 0.46, self.z - 0.28],
            [0.0, 0.0, 0.0],
            [0.85, 0.62, 1.0],
            2.2,
            0.48,
            false,
        ));
        doc.lights.push(light(
            2,
            "rim",
            [self.x + 0.22, self.y + 0.62, self.z - 0.12],
            [0.0, 0.0, 0.0],
            [1.0, 0.25, 0.75],
            2.0,
            0.42,
            false,
        ));
        doc.lights.push(light(
            3,
            "edge",
            [self.x - 0.24, self.y + 0.55, self.z + 0.08],
            [0.0, 0.0, 0.0],
            [0.3, 0.9, 1.0],
            1.8,
            0.4,
            false,
        ));
        push_mesh_anchors(&self.spec, &mut doc);
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
        let z0 = self.z - 6.0;
        let z1 = self.z + self.tunnel_reach();
        let zmid = (z0 + z1) * 0.5;
        let depth = z1 - z0;
        let tile = 1.6;
        let x0 = -3.2;
        let iz0 = (z0 / tile).floor() as i32;
        let iz1 = (z1 / tile).ceil() as i32;
        for ix in 0..4 {
            for iz in iz0..iz1 {
                let even = (ix + iz).rem_euclid(2) == 0;
                let col = if even { [6, 4, 12] } else { [14, 9, 24] };
                let x = x0 + (ix as f32 + 0.5) * tile;
                let z = (iz as f32 + 0.5) * tile;
                doc.props.push(matte(
                    &format!("tile-{ix}-{iz}"),
                    [x, -0.01, z],
                    [tile * 0.995, 0.03, tile * 0.995],
                    col,
                ));
            }
        }
        for (name, side) in [("wall-l", -1.0_f32), ("wall-r", 1.0)] {
            doc.props.push(matte(
                name,
                [side * 2.78, 3.45, zmid],
                [0.05, 6.9, depth],
                [4, 3, 10],
            ));
        }
        doc.props.push(matte(
            "ceiling",
            [0.0, 6.95, zmid],
            [5.7, 0.05, depth],
            [3, 2, 8],
        ));
        // Close the far end in black so the open tunnel mouth does not read
        // as a flat maroon panel, with a faint violet glow at the vanishing point.
        doc.props.push(matte(
            "end-cap",
            [0.0, 3.45, z1],
            [5.7, 6.9, 0.05],
            [0, 0, 0],
        ));
        doc.props.push(glow(
            "vanish",
            "box",
            "vanish",
            [0.0, 2.2, z1 - 0.1],
            [1.6, 0.04, 0.02],
            [120, 60, 255],
            0.6,
        ));
        self.push_grid_lines(doc, z0, z1);
    }

    /// Screen-space strokes. A long line is chopped so a segment behind the
    /// camera does not drop the rest of the rail.
    fn push_grid_lines(&self, doc: &mut WorldDoc, z0: f32, z1: f32) {
        let mag = [255, 46, 176, 255];
        let vio = [176, 72, 255, 245];
        let wall = [118, 86, 255, 210];
        let wall_dim = [64, 150, 255, 180];
        for (i, x) in [-2.4_f32, -1.6, -0.8, 0.0, 0.8, 1.6, 2.4]
            .into_iter()
            .enumerate()
        {
            let col = if i % 2 == 0 { mag } else { vio };
            stroke(
                doc,
                &format!("fl-{i}"),
                run_z(x, 0.045, z0, z1, 8.0),
                col,
                2.0,
            );
        }
        let step = 0.7;
        let i0 = (z0 / step).floor() as i32;
        let i1 = (z1 / step).ceil() as i32;
        for i in i0..i1 {
            let z = i as f32 * step;
            let col = if i.rem_euclid(2) == 0 { vio } else { mag };
            stroke(
                doc,
                &format!("fc-{i}"),
                vec![[-2.4, 0.045, z], [2.4, 0.045, z]],
                col,
                2.0,
            );
        }
        for (name, x) in [("rail-l", -2.32_f32), ("rail-r", 2.32)] {
            stroke(
                doc,
                name,
                run_z(x, 0.08, z0, z1, 8.0),
                [255, 20, 154, 255],
                3.4,
            );
        }
        for (side, x) in [("l", -2.68_f32), ("r", 2.68)] {
            for (k, y) in [0.05_f32, 1.4, 2.8, 4.2, 5.6, 6.85].into_iter().enumerate() {
                let col = if k % 2 == 0 { wall } else { wall_dim };
                stroke(
                    doc,
                    &format!("wh-{side}-{k}"),
                    run_z(x, y, z0, z1, 8.0),
                    col,
                    1.25,
                );
            }
            let vz = 4.0;
            let j0 = (z0 / vz).floor() as i32;
            let j1 = (z1 / vz).ceil() as i32;
            for j in j0..j1 {
                let z = j as f32 * vz;
                stroke(
                    doc,
                    &format!("wv-{side}-{j}"),
                    vec![[x, 0.05, z], [x, 6.85, z]],
                    wall_dim,
                    1.15,
                );
            }
        }
        let rib = 4.0;
        let r0 = (z0 / rib).floor() as i32;
        let r1 = (z1 / rib).ceil() as i32;
        for i in r0..r1 {
            let z = i as f32 * rib;
            let col = if i.rem_euclid(2) == 0 { mag } else { wall };
            stroke(
                doc,
                &format!("rib-{i}"),
                vec![[-2.4, 6.9, z], [2.4, 6.9, z]],
                col,
                1.2,
            );
        }
    }

    fn push_gates(&self, doc: &mut WorldDoc) {
        let i0 = near_index(
            self.z,
            self.spec.runner.gates.z0,
            self.spec.runner.gates.gap,
        )
        .saturating_sub(1);
        for i in i0..i0 + 5 {
            if i < 0 {
                continue;
            }
            let z = self.spec.runner.gates.z(i);
            if z < self.z + 3.0 {
                continue;
            }
            // Dark stone. The neon read is the outline, not a glowing block.
            // Baked arch is about 4.1 wide and 4.0 tall; this scale fits the
            // tunnel and leaves the opening dark.
            if let Some(mut gate) = glb_prop(
                &format!("gate-{i}"),
                "gate",
                &self.spec.runner.gates.glb,
                [0.0, 0.0, z],
                // Opening is |x| < 1.12 at scale 1. 2.15 puts the pillars at
                // |x| > 2.41, outside the lane clamp plus the cat, and they
                // hide behind the walls. The arch stays pure decor.
                [self.spec.runner.gates.scale_x, 0.68, 0.36],
                0.0,
                0.0,
                [8, 5, 14],
                0.0,
                [0.02, 0.01, 0.04],
            ) {
                gate.roughness = 0.96;
                gate.metallic = 0.0;
                doc.props.push(gate);
            }
            push_arch_frame(doc, i, z - 0.42);
        }
    }

    fn push_rings(&self, doc: &mut WorldDoc) {
        let scale = 1.12;
        let i0 = near_index(
            self.z,
            self.spec.runner.rings.track.z0,
            self.spec.runner.rings.track.gap,
        )
        .saturating_sub(1);
        for i in i0..i0 + 5 {
            if i < 0 || self.got.contains(&i) {
                continue;
            }
            let z = self.spec.runner.rings.track.z(i);
            if z < self.z - 1.5 {
                continue;
            }
            // Mesh center sits about +0.5 x and +0.47 y in the file.
            let x = ring_x(&self.spec, i) - 0.50 * scale;
            let y = 0.05;
            if let Some(mut ring) = glb_prop(
                &format!("ring-{i}"),
                "ring",
                &self.spec.runner.rings.glb,
                [x, y, z + 0.16 * scale],
                [scale, scale, scale],
                0.0,
                0.0,
                [14, 32, 40],
                0.72,
                [0.1, 0.95, 1.0],
            ) {
                ring.roughness = 0.45;
                doc.props.push(ring);
            }
        }
    }

    fn push_obstacles(&self, doc: &mut WorldDoc) -> HashSet<i32> {
        let mut spawned = HashSet::new();
        let i0 = near_index(
            self.z,
            self.spec.runner.obstacle_track.z0,
            self.spec.runner.obstacle_track.gap,
        )
        .saturating_sub(1);
        for i in i0..i0 + 2 + self.obstacle_lead_count() {
            if i < 0 {
                continue;
            }
            let shape = obstacle_shape(&self.spec, i);
            if shape.origin[2] < self.z - 2.0 {
                continue;
            }
            if !obstacle_drawable(&shape.path) {
                continue;
            }
            if let Some(mut obstacle) = glb_prop(
                &format!("obs-{i}"),
                "obstacle",
                &shape.path,
                shape.origin,
                shape.scale,
                0.0,
                0.0,
                self.spec.style.obstacle_color,
                0.0,
                [0.04, 0.02, 0.06],
            ) {
                // Dim neon fill in the edge hue so a crate reads against the
                // black tunnel. Low enough that bloom (threshold 1.4) skips it.
                let cyan = i.rem_euclid(2) == 0;
                obstacle.emissive = Some(self.spec.style.fill_emissive);
                obstacle.emissive_color = Some(if cyan {
                    self.spec.style.fill_a
                } else {
                    self.spec.style.fill_b
                });
                obstacle.roughness = 0.94;
                obstacle.metallic = 0.04;
                doc.props.push(obstacle);
                let proud = 0.045;
                let col = if i.rem_euclid(2) == 0 {
                    self.spec.style.edge_a
                } else {
                    self.spec.style.edge_b
                };
                push_neon_box(
                    doc,
                    &format!("edge-{i}"),
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
                    col,
                    2.6,
                );
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
                [side * self.spec.runner.lanes.limit, 0.2, z],
                [0.03, 0.55, 36.0],
                [255, 210, 40],
                2.2,
            ));
        }
        let i0 = near_index(
            self.z,
            self.spec.runner.obstacle_track.z0,
            self.spec.runner.obstacle_track.gap,
        )
        .saturating_sub(1);
        for i in i0..i0 + 2 + self.obstacle_lead_count() {
            if i < 0 {
                continue;
            }
            let shape = obstacle_shape(&self.spec, i);
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
        } else if self.core.juice.land_squash > 0.0 {
            stretch = 0.78;
        }
        let mut cat = WorldWalker {
            id: "walker:cat".into(),
            kind: "walker".into(),
            name: "cat".into(),
            position: [
                self.x,
                self.y + self.spec.player.foot * self.spec.player.scale,
                self.z,
            ],
            yaw: self.yaw,
            on_ground: self.y <= 0.02,
            model: String::new(),
            gltf: None,
            clip: self.anim_t,
            anim: "run".into(),
            anim_blend: 1.0,
            scale: Some([
                self.spec.player.scale,
                self.spec.player.scale * stretch,
                self.spec.player.scale,
            ]),
            color: Some(self.spec.player.tint),
            roughness: Some(0.9),
            ..WorldWalker::default()
        };
        if glb_ready(&self.spec.player.glb) {
            cat.gltf = Some(self.spec.player.glb.clone());
        }
        doc.walkers.push(cat.clone());
        doc.player = Some(cat);
    }

    /// Sparks are short fading streaks along their velocity, to match the
    /// thin line art (not glowing balls).
    fn push_sparks(&self, doc: &mut WorldDoc) {
        for (i, s) in self.core.juice.sparks.iter().enumerate() {
            let fade = 1.0 - (s.life / s.max_life).clamp(0.0, 1.0);
            let k = 0.035;
            let tail = [s.x - s.vx * k, (s.y - s.vy * k).max(0.03), s.z - s.vz * k];
            let a = (255.0 * fade) as u8;
            let c = [s.color[0] as u8, s.color[1] as u8, s.color[2] as u8, a];
            stroke(
                doc,
                &format!("spark-{i}"),
                vec![tail, [s.x, s.y, s.z]],
                c,
                1.3,
            );
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
        if self.flow.phase == FlowPhase::Play || self.core.juice.hitstop > 0.0 {
            self.core.stats.paint_score(&mut list, self.spec.hud_top);
        }
        list
    }
}

impl Kit for NeonCat {
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

    /// One substep never moves further than `speed.max_step`. At the cap a
    /// 50 ms frame moves 1 m, which skipped a 0.4 m crate.
    fn substeps(&self, dt: f32) -> u32 {
        ((self.speed() * dt) / self.spec.runner.speed.max_step)
            .ceil()
            .max(1.0) as u32
    }

    fn step(&mut self, dt: f32) {
        self.play_step(dt);
    }

    fn attract(&mut self, dt: f32) {
        self.attract += dt;
        self.z = self.attract * 8.0;
        self.x = (self.attract * 0.7).sin() * 0.65;
        self.y = 0.0;
        self.anim_t += dt;
    }

    fn hitstop_frame(&mut self, dt: f32) {
        self.anim_t += dt * 0.35;
    }

    fn follow(&mut self, dt: f32) {
        self.smooth_cam(dt);
    }

    fn blocks_spark(&self, x: f32, y: f32, z: f32) -> bool {
        spark_inside_obstacle(&self.spec, &self.spawned, x, y, z)
    }

    fn rebuild(&mut self) {
        NeonCat::rebuild(self);
    }
}

type ObstacleShape = Hitbox;

/// Local AABB after the glTF node bake (`mesh.bounds()`), times the prop
/// scale. Y stays on the floor. XZ is centered on `(obs_x, obs_z)`.
/// The kill box is inset so it sits inside that mesh.
fn obstacle_shape(s: &GameSpec, index: i32) -> ObstacleShape {
    let kind = s.obstacle_kind(index);
    Hitbox::place(
        s,
        kind,
        s.runner.obstacle_track.x(index),
        s.runner.obstacle_track.z(index),
    )
}

/// A spark inside a drawn obstacle's mesh box would glow through it.
fn spark_inside_obstacle(s: &GameSpec, spawned: &HashSet<i32>, x: f32, y: f32, z: f32) -> bool {
    let i0 =
        near_index(z, s.runner.obstacle_track.z0, s.runner.obstacle_track.gap).saturating_sub(1);
    (i0..i0 + 3).any(|i| {
        if !spawned.contains(&i) {
            return false;
        }
        obstacle_shape(s, i).contains_padded(x, y, z, 0.08)
    })
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

/// Pin the renderer's glTF slot table.
///
/// `WorldDoc::gltf_slots` lists unique specs in prop order, and the renderer
/// re-parses and re-uploads every mesh whenever that list changes. Obstacle
/// kinds scroll in and out of the window, so without anchors the table
/// changed every few seconds and the page froze while it rebuilt. One tiny
/// buried prop per GLB, first in the list, keeps the key constant.
fn push_mesh_anchors(s: &GameSpec, doc: &mut WorldDoc) {
    for (i, path) in s.static_glbs().into_iter().enumerate() {
        if let Some(prop) = glb_prop(
            &format!("anchor:{i}"),
            "anchor",
            path,
            [0.0, -60.0, -60.0],
            [1e-4, 1e-4, 1e-4],
            0.0,
            0.0,
            [0, 0, 0],
            0.0,
            [0.0, 0.0, 0.0],
        ) {
            doc.props.push(prop);
        }
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

fn matte(id: &str, pos: [f32; 3], scale: [f32; 3], color: [u32; 3]) -> WorldProp {
    let mut prop = glow(id, "box", "grid", pos, scale, color, 0.0);
    prop.roughness = 0.96;
    prop.metallic = 0.0;
    prop
}

fn stroke(doc: &mut WorldDoc, id: &str, points: Vec<[f32; 3]>, color: [u8; 4], width: f32) {
    doc.lines.push(crate::strokes::FatLine {
        id: id.into(),
        points,
        color,
        width,
    });
}

fn run_z(x: f32, y: f32, z0: f32, z1: f32, step: f32) -> Vec<[f32; 3]> {
    let mut pts = Vec::new();
    let mut z = z0;
    let step = step.max(0.5);
    while z < z1 - 0.05 {
        pts.push([x, y, z]);
        z += step;
    }
    pts.push([x, y, z1]);
    pts
}

/// Outer and inner neon arch. The stone mesh stays dark inside the pair.
fn push_arch_frame(doc: &mut WorldDoc, index: i32, z: f32) {
    stroke(
        doc,
        &format!("arch-out-{index}"),
        arch_polyline(z, 2.62, 1.48, 1.28, 10),
        [255, 48, 186, 255],
        2.4,
    );
    stroke(
        doc,
        &format!("arch-in-{index}"),
        arch_polyline(z + 0.02, 2.45, 1.36, 1.02, 10),
        [196, 90, 255, 240],
        1.5,
    );
}

fn arch_polyline(z: f32, half_w: f32, spring_y: f32, rise: f32, steps: i32) -> Vec<[f32; 3]> {
    let mut pts = Vec::with_capacity(steps as usize + 3);
    pts.push([-half_w, 0.05, z]);
    let steps = steps.max(2);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let ang = std::f32::consts::PI * (1.0 - t);
        pts.push([half_w * ang.cos(), spring_y + rise * ang.sin(), z]);
    }
    pts.push([half_w, 0.05, z]);
    pts
}

fn push_neon_box(
    doc: &mut WorldDoc,
    id: &str,
    min: [f32; 3],
    max: [f32; 3],
    color: [u8; 4],
    width: f32,
) {
    let c = [
        [min[0], min[1], min[2]],
        [max[0], min[1], min[2]],
        [max[0], min[1], max[2]],
        [min[0], min[1], max[2]],
        [min[0], max[1], min[2]],
        [max[0], max[1], min[2]],
        [max[0], max[1], max[2]],
        [min[0], max[1], max[2]],
    ];
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for (n, (a, b)) in edges.iter().enumerate() {
        stroke(doc, &format!("{id}-{n}"), vec![c[*a], c[*b]], color, width);
    }
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

    #[test]
    fn crate_still_kills_at_top_speed_with_a_long_frame() {
        for &dt in &[0.05_f32, 0.2] {
            let mut run = NeonCat::fresh();
            run.rebuild();
            run.confirm();
            run.time = 1000.0;
            assert_eq!(run.speed(), SPEED_CAP);
            let i = 30;
            assert_eq!(obstacle_shape(i).kind, "CRATE");
            let shape = obstacle_shape(i);
            run.x = shape.hit_cx;
            run.z = shape.hit_cz - 0.6;
            run.y = 0.0;
            run.vx = 0.0;
            run.rebuild();
            assert!(run.spawned.contains(&i));
            for _ in 0..4 {
                run.set_steer(0.0, 0.0, false);
                run.x = shape.hit_cx;
                run.tick(dt);
                if !run.core.stats.alive {
                    break;
                }
            }
            assert!(
                !run.core.stats.alive,
                "dt {dt}: a lane crate at the cap must kill"
            );
        }
    }

    #[test]
    fn obstacles_show_early_enough_at_the_cap_and_fog_keeps_them() {
        let mut run = NeonCat::fresh();
        run.confirm();
        run.time = 1000.0;
        run.z = 500.0;
        run.rebuild();
        let far = run
            .spawned
            .iter()
            .map(|&i| obstacle_shape(i).hit_cz)
            .fold(f32::MIN, f32::max);
        let lead = (far - run.z) / run.speed();
        assert!(lead >= 1.2, "only {lead:.2}s of warning at the cap");
        assert!(
            far - run.z < run.doc.fog_end.unwrap(),
            "fog hides the farthest"
        );
    }

    #[test]
    fn sparks_stay_above_the_floor_and_out_of_crates() {
        let mut run = NeonCat::fresh();
        run.rebuild();
        run.burst(0.0, 0.2, 5.0, 30, [255, 255, 255], 5.0, 0.12);
        for _ in 0..40 {
            let mut juice = std::mem::take(&mut run.core.juice);
            juice.tick_sparks(1.0 / 60.0, |x, y, z| run.blocks_spark(x, y, z));
            run.core.juice = juice;
            for s in &run.core.juice.sparks {
                assert!(s.y >= s.size * 0.5 - 1e-4, "spark under floor at {}", s.y);
                assert!(!spark_inside_obstacle(&run.spawned, s.x, s.y, s.z));
            }
        }
    }

    #[test]
    fn arch_pillars_sit_outside_the_lane() {
        // Arch.glb inner opening half-width at scale 1.
        let inner = 1.12 * GATE_SCALE_X;
        assert!(
            inner > LANE_LIMIT + 0.2,
            "pillar at {inner} inside the lane"
        );
    }

    /// Fixed-input runs. Returns (score, rings, death frame, death reason).
    fn golden_run(run: &mut NeonCat, steer_rate: f32, jump_every: u32) -> (u32, u32, u32, String) {
        run.confirm();
        let mut frame = 0u32;
        while run.core.stats.alive && frame < 60 * 180 {
            run.set_steer(
                (frame as f32 * steer_rate).sin(),
                0.0,
                frame.is_multiple_of(jump_every),
            );
            run.tick(1.0 / 60.0);
            frame += 1;
        }
        (run.score(), run.rings(), frame, run.death().to_string())
    }

    #[test]
    fn golden_runs_match_the_pre_port_numbers() {
        // Recorded on master ffd6ee7, before constants moved into GameSpec.
        type Golden = (u32, u32, u32, &'static str);
        let cases: [(f32, u32, Golden); 4] = [
            (0.013, 47, (4252, 13, 4642, "BARREL")),
            (0.031, 23, (3004, 15, 2667, "BARRIER")),
            (0.0, 1000, (156, 1, 144, "CRATE")),
            (0.007, 11, (657, 2, 1010, "BARREL")),
        ];
        // The Python-baked spec must be Neon Cat exactly, and must play the same.
        let baked = include_str!("../tests/fixtures/neon_cat_world.json");
        let loaded = NeonCat::from_json(baked).unwrap();
        assert_eq!(loaded.spec(), &GameSpec::default());
        for doc in [r#"{"version":1,"genre":"neon_cat"}"#, baked] {
            for (rate, every, want) in cases {
                let mut run = NeonCat::from_json(doc).unwrap();
                let (score, rings, frame, death) = golden_run(&mut run, rate, every);
                assert_eq!(
                    (score, rings, frame, death.as_str()),
                    want,
                    "steer {rate} jump/{every}"
                );
            }
        }
    }

    #[test]
    fn second_game_is_just_a_spec() {
        let doc = include_str!("../tests/fixtures/meteor_cat_world.json");
        let play = || {
            let mut run = NeonCat::from_json(doc).unwrap();
            let out = golden_run(&mut run, 0.013, 47);
            (out, run.spec().clone())
        };
        let ((score, _, frame, death), spec) = play();
        assert_eq!(spec.id, "meteor_cat");
        assert_eq!(spec.runner.speed.cap, 24.0);
        assert!(
            ["METEOR", "CRYSTALS", "ROCK"].contains(&death.as_str()),
            "{death}"
        );
        assert!(score > 0 && frame > 60);
        assert_eq!(play().0, play().0, "fixed inputs replay exactly");
    }

    #[test]
    fn spec_changes_reach_the_sim() {
        let doc =
            r#"{"game":{"runner":{"speed":{"start":14.0,"cap":14.0},"rings":{"score":250}}}}"#;
        let mut run = NeonCat::from_json(doc).unwrap();
        run.confirm();
        run.tick(1.0 / 60.0);
        assert_eq!(run.speed(), 14.0);
        assert_eq!(run.spec().runner.rings.score, 250);
        assert!(NeonCat::from_json(r#"{"game":{"obstacles":[]}}"#).is_err());
    }

    #[test]
    fn fixed_dt_runs_are_deterministic() {
        let play = || {
            let mut run = NeonCat::fresh();
            run.confirm();
            let mut frames = 0;
            while run.core.stats.alive && frames < 60 * 120 {
                run.set_steer(((frames as f32) * 0.013).sin(), 0.0, frames % 47 == 0);
                run.tick(1.0 / 60.0);
                frames += 1;
            }
            (run.score(), run.rings(), frames, run.death().to_string())
        };
        assert_eq!(play(), play());
    }

    #[test]
    fn gpu_mesh_key_stays_put_through_a_long_run() {
        let mut run = NeonCat::fresh();
        run.rebuild();
        let first = run.document().gpu_mesh_key();
        run.confirm();
        for frame in 0..6000 {
            run.set_steer(((frame as f32) * 0.01).sin(), 0.0, frame % 90 == 0);
            run.tick(1.0 / 60.0);
            if !run.playing() {
                run.confirm();
            }
            assert_eq!(
                run.document().gpu_mesh_key(),
                first,
                "frame {frame}: a changed key re-uploads every mesh (the hitch)"
            );
        }
    }

    use super::*;

    // Pre-port names for the default spec, so the tests read as before.
    fn spec() -> GameSpec {
        GameSpec::default()
    }
    fn obstacle_shape(index: i32) -> ObstacleShape {
        super::obstacle_shape(&spec(), index)
    }
    fn spark_inside_obstacle(spawned: &HashSet<i32>, x: f32, y: f32, z: f32) -> bool {
        super::spark_inside_obstacle(&spec(), spawned, x, y, z)
    }
    fn obs_z(index: i32) -> f32 {
        spec().runner.obstacle_track.z(index)
    }
    fn obs_x(index: i32) -> f32 {
        spec().runner.obstacle_track.x(index)
    }
    fn gate_z(index: i32) -> f32 {
        spec().runner.gates.z(index)
    }
    const SPEED_CAP: f32 = 20.0;
    const LANE_LIMIT: f32 = 2.15;
    const GATE_SCALE_X: f32 = 2.15;
    const GRACE_SECS: f32 = 1.6;
    const CHASE_BACK: f32 = 5.9;
    const CHASE_LIFT: f32 = 2.35;
    const CHASE_LOOK: f32 = 13.0;
    const CHASE_FOV: f32 = 50.0;
    const CHASE_FOV_FAST: f32 = 58.0;
    const CAM_BACK: f32 = 7.4;
    const CAM_LIFT: f32 = 1.72;
    const CAM_FOV: f32 = 50.0;
    const OBS_FILL_EMISSIVE: f32 = 0.35;
    const CAT_SCALE: f32 = 0.50;
    const CAT_TINT: [u32; 3] = [226, 206, 255];
    const CAT_GLB: &str = "assets/models/kenney/cube-pets/animal-cat.glb";
    const GATE_GLB: &str = "assets/models/quaternius/modular-dungeons-pack/Arch.glb";
    const RING_GLB: &str = "assets/models/kenney/racing-kit/overheadRound.glb";

    #[test]
    fn test_shims_match_the_default_spec() {
        let s = spec();
        assert_eq!(s.runner.speed.cap, SPEED_CAP);
        assert_eq!(s.runner.lanes.limit, LANE_LIMIT);
        assert_eq!(s.runner.gates.scale_x, GATE_SCALE_X);
        assert_eq!(s.runner.jump.grace_secs, GRACE_SECS);
        assert_eq!(
            [
                s.camera.chase_back,
                s.camera.chase_lift,
                s.camera.chase_look
            ],
            [CHASE_BACK, CHASE_LIFT, CHASE_LOOK]
        );
        assert_eq!(
            [s.camera.fov, s.camera.fov_fast],
            [CHASE_FOV, CHASE_FOV_FAST]
        );
        assert_eq!(
            [
                s.camera.classic_back,
                s.camera.classic_lift,
                s.camera.classic_fov
            ],
            [CAM_BACK, CAM_LIFT, CAM_FOV]
        );
        assert_eq!(s.style.fill_emissive, OBS_FILL_EMISSIVE);
        assert_eq!((s.player.scale, s.player.tint), (CAT_SCALE, CAT_TINT));
        assert_eq!(
            [&s.player.glb, &s.runner.gates.glb, &s.runner.rings.glb],
            [CAT_GLB, GATE_GLB, RING_GLB]
        );
    }
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
            run.core.stats.alive,
            "a timed jump should clear the crate at z {}",
            obs_z(0)
        );
        assert!(run.z() > obs_z(0));

        let mut crash = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        crash.confirm();
        for _ in 0..60 * 8 {
            crash.set_steer(0.0, 0.0, false);
            crash.tick(1.0 / 60.0);
            if !crash.core.stats.alive {
                break;
            }
        }
        assert!(
            !crash.core.stats.alive,
            "staying on the ground hits the crate"
        );
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
            let file = crate::assets::resolve_asset(&shape.path)
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
        assert!(!crate_shape.feet_hit(obs_x(0) + 0.40, 0.0, obs_z(0)));
        assert!(crate_shape.feet_hit(obs_x(0), 0.0, obs_z(0)));
        assert!(!crate_shape.feet_hit(obs_x(0), crate_shape.hit_top, obs_z(0)));

        let mut graze = place_on(0, obs_x(0) + 0.40, 0.0);
        graze.tick(1.0 / 60.0);
        assert!(
            graze.core.stats.alive,
            "a point beside the crate is not a hit"
        );
        assert!(graze.death().is_empty());

        let mut hit = place_on(0, obs_x(0), 0.0);
        hit.tick(1.0 / 60.0);
        assert!(!hit.core.stats.alive);
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
        assert!(jumped.core.stats.alive, "feet above the crate clear it");
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
            spawn.core.stats.alive,
            "grace skips the obstacle under the spawn stretch"
        );

        let mut rail = NeonCat::from_json(r#"{"version":1,"genre":"neon_cat"}"#).unwrap();
        rail.confirm();
        rail.set_steer(1.0, 0.0, false);
        for _ in 0..60 * 4 {
            rail.tick(1.0 / 60.0);
        }
        assert!(
            rail.core.stats.alive,
            "full steer used to die on the empty wall"
        );
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
            arch.core.stats.alive,
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
        assert!(run.core.stats.alive, "no drawn mesh, no kill");
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
                assert_eq!(prop.gltf.as_deref(), Some(shape.path.as_str()));
                let file = crate::assets::resolve_asset(&shape.path).unwrap();
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
        let bump = run.core.juice.land.offset();
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
        let still = classic.core.juice.land.offset();
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
        assert!(run.core.stats.alive);
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
            cat.y < -0.2 && cat.y > -0.98,
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

    #[test]
    fn portrait_cat_is_a_small_dark_figure() {
        let mut run =
            NeonCat::from_json(r#"{"version":1,"genre":"neon_cat","quality":"mobile"}"#).unwrap();
        run.confirm();
        let doc = run.document();
        assert!(
            doc.exposure.unwrap() < 0.85,
            "exposure {}",
            doc.exposure.unwrap()
        );
        assert!(doc.bloom.unwrap() < 0.25, "bloom {}", doc.bloom.unwrap());
        assert!(
            doc.bloom_threshold.unwrap() > 1.1,
            "threshold {}",
            doc.bloom_threshold.unwrap()
        );
        assert!(doc.fog_start.unwrap() > 28.0);
        assert!(doc.lines.len() > 24, "grid lines {}", doc.lines.len());
        assert!(
            doc.lines.len() < crate::strokes::MAX_SEGMENTS,
            "segment budget"
        );
        let gate = doc
            .props
            .iter()
            .find(|prop| prop.name == "gate")
            .expect("gate");
        assert!(gate.emissive.unwrap_or(1.0) < 0.05);
        assert!(gate.color.unwrap()[0] < 40);
        let obs = doc
            .props
            .iter()
            .find(|prop| prop.id == "prop:obs-0")
            .expect("crate");
        assert!(obs.color.unwrap()[0] < 48);
        assert!(obs.emissive.unwrap_or(1.0) <= OBS_FILL_EMISSIVE);
        assert!(doc.lines.iter().any(|line| line.id.starts_with("edge-0")));
        assert!(doc.lines.iter().any(|line| line.id.starts_with("arch-out")));
        assert!(doc.lines.iter().any(|line| line.id.starts_with("fl-")));
        let cat = doc.walkers.first().expect("cat");
        assert_eq!(cat.color, Some(CAT_TINT));
        assert!(cat.scale.unwrap()[0] < 0.58);
        assert!(cat.roughness.unwrap() > 0.8);
        for light in &doc.lights {
            if light.kind == "point" {
                assert!(
                    light.radius < 1.6,
                    "point lights stay on the cat, {} r {} i {}",
                    light.name,
                    light.radius,
                    light.intensity
                );
            }
        }
        let scene = doc.compile_scene(412.0 / 915.0);
        let tint = [CAT_TINT[0] as u8, CAT_TINT[1] as u8, CAT_TINT[2] as u8, 255];
        assert!(
            scene
                .batches
                .iter()
                .any(|batch| batch.instances.iter().any(|inst| inst.color == tint)),
            "the cat tint has to reach the textured draw"
        );

        let feet = portrait_ndc(&run, [0.0, 0.02, 0.0]);
        let head = portrait_ndc(&run, [0.0, 1.70 * CAT_SCALE, 0.0]);
        let fraction = (head.y - feet.y).abs() * 0.5;
        assert!(
            (0.11..0.19).contains(&fraction),
            "cat should be about 15% of the portrait frame, fraction {fraction} feet {} head {}",
            feet.y,
            head.y
        );
        assert!(feet.y > -0.98, "feet cropped, ndc {}", feet.y);
        assert!(head.y < -0.28, "head left the lower third, ndc {}", head.y);
        let mid = (feet.y + head.y) * 0.5;
        assert!(
            mid < -0.33,
            "cat center ndc {mid} should sit in the lower third"
        );
    }
}
