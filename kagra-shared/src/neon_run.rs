//! Lumen — endless neon flyer. Not a WorldPlay genre.
//!
//! The ship, gate obstacles, crystal pickups, and side dishes are Kenney
//! Space Kit GLBs when the file is mounted or on disk. A missing file keeps
//! the sphere / box primitive. Ribs, rails, rings, and stars stay procedural.
//!
//! Phone and desktop wasm load a dump with `"genre": "neon_run"` and this
//! session ticks it. Python calls the same type for the window build.
//! Glow is dump `emissive` (the shared HDR add). Reflections are
//! `hemi_sky` / `hemi_ground` plus `env_map: preset:studio`.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use serde_json::json;

use crate::flow::{FlowAction, FlowPhase, GameFlow};
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc, WorldLight, WorldProp};

pub const GAME_ID: &str = "neon_run";

/// First seconds ignore a miss and hold the ship inside the corridor.
const GRACE_SECS: f32 = 2.4;
/// Pass radius. The old hole was 1.15 the whole run.
const HOLE_R_EARLY: f32 = 1.46;
const HOLE_R_LATE: f32 = 1.28;
const GATE0: f32 = 50.0;
const WALL_X: f32 = 2.58;
const WALL_Y: f32 = 1.90;
/// Cruise at t=0. The previous start was 22, so this is about 35% slower.
const SPEED0: f32 = 14.3;
/// Units per second added to cruise. The previous ramp was 0.62.
const SPEED_RAMP: f32 = 0.32;
const SPEED_CAP: f32 = 46.0;

/// Kenney Space Kit, CC0. See `assets/models/kenney/space-kit/LICENSE.txt`.
const SHIP_GLB: &str = "assets/models/kenney/space-kit/craft_speederC.glb";
/// The mesh is heavier toward local +Z (the tail). Yaw π puts that tail
/// toward the chase camera, which looks along world +Z.
const SHIP_YAW: f32 = std::f32::consts::PI;
/// About 0.78× the old 0.46 hull, so the next ring stays in frame.
const SHIP_SCALE: f32 = 0.36;
/// Side posts only. A top beam, even a thin one, stacks into cyan and
/// pink bars across the upper half of a portrait frame.
const RIB_EMISSIVE: f32 = 0.12;
/// Bottom-origin mesh, so this lifts the visual center onto the ship.
const SHIP_CENTER_Y: f32 = 0.30;
/// Chase camera sits behind the ship. Boost eases this back a little more.
const CAM_BACK: f32 = 11.4;
const CAM_LIFT: f32 = 1.62;
const CAM_FOV: f32 = 50.0;

struct KitMesh {
    path: &'static str,
    center_y: f32,
    radius: f32,
}

const OBSTACLE_KITS: &[KitMesh] = &[
    KitMesh {
        path: "assets/models/kenney/space-kit/meteor.glb",
        center_y: 0.37,
        radius: 0.43,
    },
    KitMesh {
        path: "assets/models/kenney/space-kit/meteor_detailed.glb",
        center_y: 0.37,
        radius: 0.43,
    },
    KitMesh {
        path: "assets/models/kenney/space-kit/rock_largeA.glb",
        center_y: 0.25,
        radius: 0.46,
    },
    KitMesh {
        path: "assets/models/kenney/space-kit/turret_single.glb",
        center_y: 0.28,
        radius: 0.35,
    },
];
const PICKUP_GLB: &str = "assets/models/kenney/space-kit/rock_crystals.glb";
const PICKUP_CENTER_Y: f32 = 0.17;
const DISH_GLB: &str = "assets/models/kenney/space-kit/satelliteDish.glb";
const DISH_CENTER_Y: f32 = 0.31;

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
    let (x, y) = P[index.rem_euclid(5) as usize];
    // The first rings stay near the middle. The full weave arrives later.
    let t = (index as f32 / 10.0).clamp(0.0, 1.0);
    let k = 0.22 + 0.78 * t * t;
    (x * k, y * k)
}

pub fn hole_r(index: i32) -> f32 {
    let t = (index.max(0) as f32 / 12.0).clamp(0.0, 1.0);
    HOLE_R_EARLY + (HOLE_R_LATE - HOLE_R_EARLY) * t
}

/// Gap from gate `index - 1` to gate `index`. Early gaps are wide; the late
/// run tightens past the old constant 26.
pub fn gate_gap(index: i32) -> f32 {
    if index <= 0 {
        return 0.0;
    }
    let t = ((index - 1) as f32 / 9.0).clamp(0.0, 1.0);
    44.0 + (23.0 - 44.0) * t
}

pub fn gate_z(index: i32) -> f32 {
    if index <= 0 {
        return GATE0 + index as f32 * 44.0;
    }
    let mut z = GATE0;
    let n = index.min(4_000);
    for i in 1..=n {
        z += gate_gap(i);
    }
    z
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

fn attach_glb(prop: &mut WorldProp, path: &str) -> bool {
    if !glb_ready(path) {
        return false;
    }
    prop.gltf = Some(path.to_string());
    prop.color = Some([255, 255, 255]);
    prop.emissive_map = None;
    true
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
    /// 0..1 eased while boost is held, for FOV, camera, and speed lines.
    boost_vis: f32,
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
    sparks: Vec<Spark>,
    doc: WorldDoc,
    /// Dump `quality`, copied onto every rebuild. None keeps the historical
    /// neon picture (one cascade, no SSAO). Gameplay does not read it.
    picture_quality: Option<String>,
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
        if let Some(q) = value.get("quality").and_then(|v| v.as_str()) {
            let q = q.trim();
            if !q.is_empty() {
                run.picture_quality = Some(q.to_string());
            }
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
            boost_vis: 0.0,
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
            sparks: Vec::new(),
            doc: WorldDoc::default(),
            picture_quality: None,
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

    /// World dump plus a flat `sim` object.
    ///
    /// Call this once per tick. `sim` carries phase, the ship, the next gate,
    /// and the chase camera's forward, so a host does not also parse
    /// `state_json` or round-trip the dump through JSON to edit a copy.
    /// Wasm `render` draws `document()` directly and does not call this.
    /// `state_json` stays for the page's small `gameJson` status blob.
    pub fn dump_json(&self) -> String {
        let body = self.doc.to_json().unwrap_or_else(|_| "{}".into());
        splice_json_key(&body, "sim", &self.sim_value())
    }

    fn sim_value(&self) -> serde_json::Value {
        let next = self.passed + 1;
        let (hx, hy) = hole_center(next);
        json!({
            "phase": self.flow.phase.name(),
            "alive": self.alive,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "next_x": hx,
            "next_y": hy,
            "next_z": gate_z(next),
            "cam_forward": self.cam_forward(),
        })
    }

    /// Unit forward of the chase camera (target − eye). Listener aim uses this
    /// instead of a hard-coded world +Z.
    fn cam_forward(&self) -> [f32; 3] {
        let fwd =
            glam::Vec3::from_array(self.chase_target()) - glam::Vec3::from_array(self.chase_eye());
        if fwd.length_squared() > 1e-8 {
            fwd.normalize().to_array()
        } else {
            [0.0, 0.0, 1.0]
        }
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

    /// Chase cam looks along +Z, so this is about world -X.
    fn screen_right(&self) -> glam::Vec3 {
        let eye = self.chase_eye();
        let target = self.chase_target();
        crate::scene3d::screen_right(glam::Vec3::from_array(eye), glam::Vec3::from_array(target))
    }

    fn chase_target(&self) -> [f32; 3] {
        [self.x, self.y + 0.35, self.z + 16.0]
    }

    fn chase_eye(&self) -> [f32; 3] {
        let shake = (self.time * 47.0).sin() * self.shake;
        let back = CAM_BACK + self.boost_vis * 2.6;
        [
            self.cam_x + shake,
            self.cam_y + CAM_LIFT + shake * 0.35,
            self.z - back,
        ]
    }

    fn chase_fov(&self) -> f32 {
        CAM_FOV + self.boost_vis * 8.0
    }

    fn tick_boost_vis(&mut self, dt: f32) {
        let target = if self.boost > 0.02 { 1.0 } else { 0.0 };
        let k = 1.0 - (-dt * 6.0).exp();
        self.boost_vis += (target - self.boost_vis) * k;
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
            "screen_right": self.screen_right().to_array(),
            "screen_left": (-self.screen_right()).to_array(),
            "next_x": hx,
            "next_y": hy,
            "next_z": gate_z(next),
            "cam_forward": self.cam_forward(),
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
        self.boost_vis = 0.0;
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
            self.decay_shake(dt);
            self.tick_boost_vis(dt);
            self.rebuild();
            return;
        }
        if self.flow.phase != FlowPhase::Play || !self.alive {
            self.tick_sparks(dt);
            self.decay_shake(dt);
            self.tick_boost_vis(dt);
            self.rebuild();
            return;
        }
        self.time += dt;
        self.boost = (self.boost - dt).max(0.0);
        let (ax, ay) = self.axis();
        // +ax is D / stick-right. The chase camera looks along +Z, so
        // screen-right is world -X (right-handed look-at). +Y stays up.
        let steer_x = -ax;
        let grace = self.time < GRACE_SECS;
        let auth = if grace { 0.55 } else { 1.0 };
        let response = 1.0 - (-dt * 7.0).exp();
        self.vx += (steer_x * 6.4 * auth - self.vx) * response;
        self.vy += (ay * 5.2 * auth - self.vy) * response;
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        if grace {
            // Open the hold as the grace runs out so the walls don't snap.
            let hold = (1.0 - self.time / GRACE_SECS).clamp(0.0, 1.0);
            let limit_x = (WALL_X - 0.40 - hold * 0.85).max(0.8);
            let limit_y = (WALL_Y - 0.30 - hold * 0.60).max(0.55);
            self.x = self.x.clamp(-limit_x, limit_x);
            self.y = self.y.clamp(-limit_y, limit_y);
        }
        let speed = self.speed();
        self.z += speed * dt;
        self.score_acc += speed * dt * 2.0;
        let whole = self.score_acc.floor();
        if whole >= 1.0 {
            self.score += whole as u32;
            self.score_acc -= whole;
        }
        if !grace && (self.x.abs() > WALL_X || self.y.abs() > WALL_Y) {
            self.crash();
        } else {
            self.cross_gates();
        }
        self.smooth_cam(dt);
        self.tick_sparks(dt);
        self.decay_shake(dt);
        self.tick_boost_vis(dt);
        self.rebuild();
    }

    fn speed(&self) -> f32 {
        let base = (SPEED0 + self.time * SPEED_RAMP).min(SPEED_CAP);
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
            let hole = hole_r(next);
            let d = (self.x - hx).hypot(self.y - hy);
            if d > hole {
                // A miss in the opening seconds is a free pass, not a crash.
                if self.time < GRACE_SECS {
                    self.passed = next;
                    continue;
                }
                self.crash();
                break;
            }
            self.passed = next;
            self.score += 100;
            if d <= hole * 0.40 {
                self.score += 80;
                self.orbs += 1;
                self.just_orbs += 1;
                self.burst(hx, hy, gz, 20, [255, 244, 180], 4.8, 0.18);
                crate::mixer::cue_sfx("card");
                self.shake = self.shake.max(0.08);
            } else if d >= hole * 0.72 {
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
        self.burst(self.x, self.y, self.z, 22, [255, 64, 140], 4.8, 0.18);
        self.burst(self.x, self.y, self.z, 12, [120, 240, 255], 3.6, 0.16);
        crate::mixer::cue_sfx("fire");
        self.flow.finish(self.score, self.orbs, "CRASH");
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

    #[allow(clippy::too_many_arguments)]
    fn burst(&mut self, x: f32, y: f32, z: f32, n: u32, color: [u32; 3], speed: f32, size: f32) {
        for _ in 0..n {
            if self.sparks.len() >= 64 {
                self.sparks.remove(0);
            }
            let ang = self.rng() * std::f32::consts::TAU;
            let rad = self.rng();
            let spd = speed * (0.35 + self.rng());
            let vy = 0.8 + self.rng() * speed * 0.45;
            let max_life = 0.34 + self.rng() * 0.32;
            let size = size * (0.65 + self.rng() * 0.7);
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
            contact_blobs: Some(false),
            // Night grade kept under the bloom. The shared "night" preset is
            // brighter on purpose and is not applied here.
            sky: Some([22, 12, 52, 255]),
            hemi_sky: Some([0.16, 0.14, 0.42]),
            hemi_ground: Some([0.08, 0.04, 0.10]),
            env_map: Some("preset:studio".into()),
            shadow_softness: Some(1.0),
            bloom: Some(0.14),
            bloom_threshold: Some(1.45),
            vignette: Some(0.55),
            lut: Some("cool".into()),
            lut_strength: Some(0.22),
            fxaa: Some(true),
            exposure: Some(0.88),
            ibl: Some(0.28),
            tonemap: Some(true),
            cull: Some(true),
            prop_lod: Some(80.0),
            fog_start: Some(36.0),
            fog_end: Some(130.0),
            ..WorldDoc::default()
        };
        doc.quality = self.picture_quality.clone();
        let eye = self.chase_eye();
        let target = self.chase_target();
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "chase".into(),
            position: eye,
            target,
            fov: self.chase_fov(),
        });
        // Slot 0 is the sun shadow. It must sit behind the camera: a spot
        // parked on the eye puts the shadow frustum through the ribs and
        // the compare comes back as NaN, which wipes emissive and bloom.
        doc.lights.push(light(
            0,
            "key",
            [eye[0] - 1.5, eye[1] + 7.5, eye[2] - 5.0],
            [0.12, -0.82, 0.5],
            [0.72, 0.82, 1.0],
            1.05,
            48.0,
            true,
        ));
        doc.lights.push(light(
            1,
            "ship",
            [self.x, self.y, self.z],
            [0.0, 0.0, 0.0],
            [0.35, 0.90, 1.0],
            1.55,
            9.0,
            false,
        ));
        doc.lights.push(light(
            2,
            "rim",
            [self.x, self.y + 1.2, self.z + 6.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.28, 0.65],
            1.15,
            13.0,
            false,
        ));
        self.push_tunnel(&mut doc);
        self.push_floor(&mut doc);
        self.push_gates(&mut doc);
        self.push_islands(&mut doc);
        self.push_stars(&mut doc);
        self.push_ship(&mut doc);
        self.push_speed_lines(&mut doc);
        self.push_spark_props(&mut doc);
        self.doc = doc;
    }

    fn push_tunnel(&self, doc: &mut WorldDoc) {
        let origin = (self.z / 7.0).floor() * 7.0;
        for i in 0..10 {
            let z = origin + i as f32 * 7.0;
            // The nearest rib used to cross the lens and read as a bar.
            if z < self.z + 14.0 {
                continue;
            }
            let cyan = (i & 1) == 0;
            let col = if cyan { [40, 220, 255] } else { [255, 50, 180] };
            let gain = RIB_EMISSIVE;
            let w = 8.2;
            doc.props.push(beam(
                &format!("rib-l-{i}"),
                [-w * 0.5, 1.4, z],
                [0.02, 2.2, 0.04],
                col,
                gain,
            ));
            doc.props.push(beam(
                &format!("rib-r-{i}"),
                [w * 0.5, 1.4, z],
                [0.02, 2.2, 0.04],
                col,
                gain,
            ));
        }
        // Short rails in front of the camera. A 70 m beam centered ahead
        // runs back through the near plane and can fill a band of the frame.
        doc.props.push(beam(
            "rail-l",
            [-3.4, -2.15, self.z + 28.0],
            [0.035, 0.02, 32.0],
            [80, 255, 240],
            0.22,
        ));
        doc.props.push(beam(
            "rail-r",
            [3.4, -2.15, self.z + 28.0],
            [0.035, 0.02, 32.0],
            [255, 60, 180],
            0.22,
        ));
    }

    fn push_floor(&self, doc: &mut WorldDoc) {
        let mut floor = glow(
            "floor",
            "plane",
            "floor",
            [0.0, -2.42, self.z + 20.0],
            [18.0, 1.0, 90.0],
            0.0,
            0.0,
            0.0,
            [10, 6, 22],
            0.04,
        );
        floor.roughness = 0.32;
        floor.metallic = 0.42;
        doc.props.push(floor);
        let origin = (self.z / 4.0).floor() * 4.0;
        for i in 0..18 {
            let z = origin + i as f32 * 4.0;
            if z < self.z - 4.0 {
                continue;
            }
            let col = if i % 2 == 0 {
                [40, 220, 255]
            } else {
                [255, 50, 180]
            };
            doc.props.push(beam(
                &format!("grid-x-{i}"),
                [0.0, -2.30, z],
                [14.0, 0.012, 0.04],
                col,
                0.26,
            ));
        }
        for k in -4..=4 {
            let col = if k % 2 == 0 {
                [40, 220, 255]
            } else {
                [255, 50, 180]
            };
            doc.props.push(beam(
                &format!("grid-z-{k}"),
                [k as f32 * 1.7, -2.29, self.z + 18.0],
                [0.025, 0.01, 40.0],
                col,
                if k == 0 { 0.18 } else { 0.28 },
            ));
        }
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
            // Pass rings stay cyan. Hazards (rocks, turrets) carry the pink.
            let col = [40, 230, 255];
            // Torus major 0.5, tube 0.075. Scale so the visual hole matches the pass radius.
            let ring = hole_r(index) / 0.425;
            doc.props.push(glow(
                &format!("ring-{index}"),
                "ring",
                "gate",
                [hx, hy, z],
                [ring, ring, ring],
                0.0,
                0.0,
                0.0,
                col,
                0.9,
            ));
            let inner = ring * 0.76;
            doc.props.push(glow(
                &format!("ring-b-{index}"),
                "ring",
                "gate",
                [hx, hy, z + 0.55],
                [inner, inner, inner],
                0.0,
                0.0,
                0.0,
                [180, 255, 255],
                0.7,
            ));
            self.push_obstacles(doc, index, hx, hy, z);
            if index > self.passed {
                let spin = self.time * 1.4;
                let s = 0.62;
                let mut orb = glow(
                    &format!("orb-{index}"),
                    "sphere",
                    "orb",
                    [hx, hy - PICKUP_CENTER_Y * s, z],
                    [s, s, s],
                    spin,
                    0.0,
                    0.0,
                    [255, 236, 170],
                    1.6,
                );
                if attach_glb(&mut orb, PICKUP_GLB) {
                    orb.metallic = 0.1;
                    orb.roughness = 0.35;
                    orb.emissive = Some(1.7);
                    orb.emissive_color = Some([1.0, 0.82, 0.35]);
                }
                doc.props.push(orb);
            }
        }
    }

    fn push_obstacles(&self, doc: &mut WorldDoc, index: i32, hx: f32, hy: f32, z: f32) {
        let hole = hole_r(index);
        let late = (index as f32 / 10.0).clamp(0.0, 1.0);
        let scale = 1.22 + 0.48 * late;
        let count = if index < 5 { 4 } else { 8 };
        for k in 0..count {
            let ang = k as f32 * std::f32::consts::TAU / count as f32 + index as f32 * 0.17;
            let kit = &OBSTACLE_KITS[(index.rem_euclid(16) as usize + k) % OBSTACLE_KITS.len()];
            let dist = hole + kit.radius * scale + 0.18;
            let ox = hx + ang.cos() * dist;
            let oy = hy + ang.sin() * dist * 0.72;
            let mut prop = glow(
                &format!("obs-{index}-{k}"),
                "sphere",
                "obstacle",
                [ox, oy - kit.center_y * scale, z],
                [scale, scale, scale],
                ang,
                0.15,
                0.2 * k as f32,
                [200, 120, 90],
                0.35,
            );
            if attach_glb(&mut prop, kit.path) {
                // Pink multiply so the rock base color lands in the neon palette.
                // A stronger emissive add flattened the Kenney normals, so the
                // sun term (and the shadow on it) can still shape the rock.
                prop.color = Some([255, 128, 168]);
                prop.roughness = 0.62;
                prop.metallic = 0.06;
                prop.emissive = Some(0.06);
                prop.emissive_color = Some([1.0, 0.28, 0.55]);
            } else {
                prop.color = Some([220, 70, 140]);
                prop.emissive = Some(0.7);
                prop.emissive_color = Some([1.0, 0.3, 0.6]);
            }
            doc.props.push(prop);
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
            let dish = 2.7;
            let mut isle = glow(
                &format!("isle-{i}"),
                "box",
                "island",
                [x, y - DISH_CENTER_Y * dish + 0.15, z],
                [dish, dish, dish],
                if side > 0.0 { 0.5 } else { -0.5 },
                0.0,
                0.0,
                [18, 10, 28],
                0.15,
            );
            if !attach_glb(&mut isle, DISH_GLB) {
                isle.position = [x, y, z];
                isle.scale = [2.4, 0.45, 1.6];
                isle.yaw = 0.2;
                isle.roll = 0.15;
                isle.emissive = None;
            } else {
                isle.roughness = 0.48;
                isle.metallic = 0.35;
                isle.emissive = Some(0.28);
                isle.emissive_color = Some([0.45, 0.75, 1.0]);
            }
            doc.props.push(isle);
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
                1.05,
            ));
        }
    }

    fn push_stars(&self, doc: &mut WorldDoc) {
        for i in 0..64 {
            let z = self.z + ((i * 17) % 140) as f32 - 12.0;
            let ang = i as f32 * 2.399;
            let rad = 8.5 + (i % 8) as f32 * 1.35;
            let size = 0.09 + (i % 5) as f32 * 0.035;
            let col = if i % 3 == 0 {
                [255, 110, 210]
            } else if i % 3 == 1 {
                [150, 250, 255]
            } else {
                [255, 240, 210]
            };
            let y = ang.sin().abs() * rad * 0.55 + 1.6;
            doc.props.push(glow(
                &format!("star-{i}"),
                "sphere",
                "star",
                [ang.cos() * rad, y, z],
                [size, size, size],
                0.0,
                0.0,
                0.0,
                col,
                2.1 + (i % 3) as f32 * 0.25,
            ));
        }
    }

    fn push_ship(&self, doc: &mut WorldDoc) {
        let bank = (-self.vx * 0.14).clamp(-0.6, 0.6);
        let nose = (self.vy * -0.08).clamp(-0.35, 0.35);
        let (x, y, z) = (self.x, self.y, self.z);
        if glb_ready(SHIP_GLB) {
            let mut prop = glow(
                "ship-core",
                "sphere",
                "ship-core",
                [x, y - SHIP_CENTER_Y * SHIP_SCALE, z],
                [SHIP_SCALE, SHIP_SCALE, SHIP_SCALE],
                SHIP_YAW,
                nose,
                bank,
                // Cool multiply on the Kenney base colors (red metal, gray, dark).
                [176, 220, 255],
                0.12,
            );
            prop.gltf = Some(SHIP_GLB.into());
            prop.emissive_map = None;
            prop.emissive_color = Some([0.20, 0.62, 1.0]);
            prop.metallic = 0.22;
            prop.roughness = 0.72;
            doc.props.push(prop);
            self.push_ship_lights(doc, "prop:ship-core");
            return;
        }
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

    /// Cyan canopy toward the nose, pink engines toward the chase camera.
    /// Local offsets live in the hull's space (yaw π already applied).
    fn push_ship_lights(&self, doc: &mut WorldDoc, parent: &str) {
        let mut canopy = glow(
            "cockpit",
            "sphere",
            "cockpit",
            [0.0, 0.52, -0.2],
            [0.55, 0.28, 0.42],
            0.0,
            0.0,
            0.0,
            [140, 255, 255],
            2.6,
        );
        canopy.parent = Some(parent.into());
        canopy.local = true;
        canopy.emissive_color = Some([0.15, 0.95, 1.0]);
        canopy.roughness = 0.2;
        canopy.metallic = 0.05;
        doc.props.push(canopy);
        for (i, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
            let mut engine = glow(
                &format!("engine-{i}"),
                "sphere",
                "engine",
                [side * 0.55, 0.22, 1.25],
                [0.38, 0.38, 0.55],
                0.0,
                0.0,
                0.0,
                [255, 70, 180],
                1.7,
            );
            engine.parent = Some(parent.into());
            engine.local = true;
            engine.emissive_color = Some([1.0, 0.18, 0.55]);
            engine.roughness = 0.25;
            engine.metallic = 0.05;
            doc.props.push(engine);
        }
    }

    fn push_speed_lines(&self, doc: &mut WorldDoc) {
        let k = self.boost_vis;
        if k < 0.08 {
            return;
        }
        for i in 0..12 {
            let ang = i as f32 * std::f32::consts::TAU / 12.0 + self.time * 2.4;
            let rad = 0.85 + (i % 3) as f32 * 0.28;
            let len = 1.1 + 2.4 * k;
            let col = if i % 2 == 0 {
                [80, 240, 255]
            } else {
                [255, 70, 190]
            };
            doc.props.push(glow(
                &format!("streak-{i}"),
                "box",
                "streak",
                [
                    self.x + ang.cos() * rad,
                    self.y + ang.sin() * rad * 0.62,
                    self.z - 0.8 - (i % 4) as f32 * 0.35,
                ],
                [0.02, 0.02, len],
                0.0,
                0.0,
                0.0,
                col,
                0.5 + 1.3 * k,
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
                2.2 * fade + 0.55,
            ));
        }
    }

    pub fn hud_json(&self, width: u32, height: u32) -> String {
        let mut value: serde_json::Value =
            serde_json::from_str(&crate::flow::draw_list_json(&self.hud(width, height)))
                .unwrap_or_else(|_| json!({"quads": [], "texts": []}));
        if let Some(obj) = value.as_object_mut() {
            let pause = self
                .flow
                .button_rect(FlowAction::Pause, width, height)
                .map(|rect| json!({"x": rect[0], "y": rect[1], "w": rect[2], "h": rect[3]}))
                .unwrap_or(serde_json::Value::Null);
            obj.insert("pause".into(), pause);
        }
        value.to_string()
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
            list.quads
                .push(Quad::new(12.0, 8.0, 196.0, 46.0, [6, 4, 16, 150]));
            label(
                &mut list,
                &format!("SCORE {}", self.score),
                22.0,
                14.0,
                16.0,
                [150, 245, 255, 255],
                TextAlign::Left,
            );
            label(
                &mut list,
                &format!("{} m   {} ORBS", self.z.max(0.0) as u32, self.orbs),
                22.0,
                36.0,
                12.0,
                [120, 210, 230, 230],
                TextAlign::Left,
            );
            if self.boost_vis > 0.08 {
                let k = self.boost_vis;
                for i in 0..7 {
                    let y = h * (0.18 + i as f32 * 0.09);
                    let len = 28.0 + 84.0 * k;
                    let a = (90.0 + 120.0 * k) as u8;
                    list.quads
                        .push(Quad::new(6.0, y, len, 1.4, [80, 240, 255, a]));
                    list.quads.push(Quad::new(
                        w - 6.0 - len,
                        y + 14.0,
                        len,
                        1.4,
                        [255, 70, 190, a],
                    ));
                }
            }
            if self.alive && self.time < GRACE_SECS {
                label(
                    &mut list,
                    "EASY START",
                    w * 0.5,
                    h * 0.18,
                    18.0,
                    [255, 220, 255, 230],
                    TextAlign::Center,
                );
            }
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

/// Insert `key` after the opening brace. `from_json` ignores unknown keys,
/// same as the coordinate note on `WorldDoc::to_json`.
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
    use crate::profile::FrameProfile;
    use crate::session::{SceneKind, SharedSession};
    use crate::world_doc::{self, WorldDoc};

    #[test]
    fn straight_flight_scores_and_a_wall_ends_the_run() {
        let mut run = NeonRun::from_json(r#"{"version":1,"genre":"neon_run"}"#).unwrap();
        assert_eq!(run.phase_name(), "title");
        run.tick(1.0 / 60.0);
        assert!(
            run.document().props.iter().any(|p| {
                p.name == "ship-core"
                    && p.emissive.unwrap_or(0.0) > 0.05
                    && p.emissive.unwrap_or(0.0) < 0.5
            }),
            "hull emissive stays under the bloom"
        );
        assert!(
            run.document()
                .props
                .iter()
                .any(|p| p.name == "engine" && p.emissive.unwrap_or(0.0) > 1.0),
            "engine glow is emissive"
        );
        assert_eq!(run.document().contact_blobs, Some(false));
        assert!(
            run.document()
                .props
                .iter()
                .all(|p| { !(p.name == "rib" && p.scale[0] > 1.0 && p.position[1] > 0.5) }),
            "no ceiling beam across the portrait frame"
        );
        let scene_fog = run.document().compile_scene(9.0 / 16.0);
        assert!((scene_fog.fog_start - 36.0).abs() < 1e-3);
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
        assert!((scene.hemi_sky[2] - 0.42).abs() < 1e-4);
        assert_eq!(run.document().cull, Some(true));
        assert_eq!(run.document().prop_lod, Some(80.0));
        assert!(scene.env_pixels.is_some(), "studio env strip");
        run.confirm();
        assert_eq!(run.phase_name(), "play");
        let ship = run
            .document()
            .props
            .iter()
            .find(|p| p.name == "ship-core")
            .expect("ship");
        assert_eq!(ship.gltf.as_deref(), Some(SHIP_GLB));
        assert!((ship.scale[0] - SHIP_SCALE).abs() < 1e-4);
        assert_eq!(ship.model, "sphere", "missing glb still names a primitive");
        let ring = run
            .document()
            .props
            .iter()
            .find(|p| p.name == "gate" && p.id.contains("ring-") && !p.id.contains("ring-b"))
            .expect("cyan gate");
        let col = ring.color.expect("ring color");
        assert!(
            col[2] > 200 && col[0] < 80,
            "pass rings are cyan, got {col:?}"
        );
        assert!(
            run.document()
                .props
                .iter()
                .any(|p| p.name == "obstacle" && p.gltf.as_deref() == Some(OBSTACLE_KITS[0].path)),
            "gate obstacles are glb meshes"
        );
        for _ in 0..300 {
            run.tick(1.0 / 60.0);
        }
        assert!(run.score() >= 100, "score {}", run.score());
        assert!(run.alive);
        assert!(
            run.document().props.iter().all(|p| p.name != "trail"),
            "the staircase ribbon is gone"
        );
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
    fn opening_is_slower_and_a_grace_miss_does_not_crash() {
        let early = gate_gap(1);
        let late = gate_gap(14);
        assert!(early > 40.0, "early gap {early}");
        assert!(late < 26.0, "late gap {late}");
        assert!(early > late);
        assert!(gate_z(0) > 40.0);
        assert!(hole_r(0) > 1.4);
        assert!(hole_r(20) < hole_r(0));
        assert!(hole_center(0).0.abs() < 0.05);

        let mut run = NeonRun::from_json(r#"{"version":1,"genre":"neon_run"}"#).unwrap();
        run.confirm();
        run.set_steer(1.0, 0.0, false);
        for _ in 0..60 {
            run.tick(1.0 / 60.0);
        }
        assert!(run.alive, "grace holds the ship");
        let state: serde_json::Value = serde_json::from_str(&run.state_json()).unwrap();
        let z = state["z"].as_f64().unwrap();
        assert!(z > 13.0 && z < 17.0, "start speed z={z}");
        assert!(!glb_ready(
            "assets/models/kenney/space-kit/missing-lumen.glb"
        ));
        for _ in 0..240 {
            run.set_steer(1.0, 0.0, false);
            run.tick(1.0 / 60.0);
        }
        assert!(!run.alive, "walls still end a run after the grace");
        assert_eq!(run.phase_name(), "result");
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

    #[test]
    fn one_dump_carries_sim_and_hud_names_the_pause_rect() {
        let mut run = NeonRun::from_json(r#"{"version":1,"genre":"neon_run"}"#).unwrap();
        let title: serde_json::Value = serde_json::from_str(&run.hud_json(1280, 720)).unwrap();
        assert!(title["pause"].is_null());
        run.confirm();
        run.tick(1.0 / 60.0);
        let dump: serde_json::Value = serde_json::from_str(&run.dump_json()).unwrap();
        assert!(
            WorldDoc::from_json(&run.dump_json()).is_ok(),
            "sim is ignored by from_json"
        );
        let sim = &dump["sim"];
        let state: serde_json::Value = serde_json::from_str(&run.state_json()).unwrap();
        assert_eq!(sim["phase"], "play");
        assert_eq!(sim["alive"], true);
        for key in ["x", "y", "z", "next_x", "next_y", "next_z"] {
            let a = sim[key].as_f64().unwrap();
            let b = state[key].as_f64().unwrap();
            assert!((a - b).abs() < 1e-4, "{key} dump {a} state {b}");
        }
        let fwd = sim["cam_forward"].as_array().unwrap();
        let len = fwd
            .iter()
            .map(|v| v.as_f64().unwrap())
            .map(|v| v * v)
            .sum::<f64>()
            .sqrt();
        assert!((len - 1.0).abs() < 1e-3, "cam forward {fwd:?}");
        assert!(fwd[2].as_f64().unwrap() > 0.9, "chase looks along +Z");
        let hud: serde_json::Value = serde_json::from_str(&run.hud_json(1280, 720)).unwrap();
        let pause = &hud["pause"];
        let rect = [
            pause["x"].as_f64().unwrap() as f32,
            pause["y"].as_f64().unwrap() as f32,
            pause["w"].as_f64().unwrap() as f32,
            pause["h"].as_f64().unwrap() as f32,
        ];
        assert!(rect[2] > 20.0 && rect[3] > 10.0, "{rect:?}");
        assert_eq!(
            run.flow
                .hit(rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5, 1280, 720),
            Some(FlowAction::Pause)
        );
        if std::env::var_os("KAGRA_BENCH").is_some() {
            let frames = 80;
            let dump_once = std::time::Instant::now();
            for _ in 0..frames {
                let _ = run.dump_json();
            }
            let dump_once = dump_once.elapsed();
            let both = std::time::Instant::now();
            for _ in 0..frames {
                let _ = run.doc.to_json();
                let _ = run.state_json();
            }
            let both = both.elapsed();
            // Host side of the old picture path: parse the dump, then serialize
            // it again. The new path parses once and deep-copies in Python.
            let text = run.dump_json();
            let round = std::time::Instant::now();
            for _ in 0..frames {
                let value: serde_json::Value = serde_json::from_str(&text).unwrap();
                let _ = serde_json::to_string(&value);
            }
            let round = round.elapsed();
            let parse = std::time::Instant::now();
            for _ in 0..frames {
                let _: serde_json::Value = serde_json::from_str(&text).unwrap();
            }
            let parse = parse.elapsed();
            let cloned = std::time::Instant::now();
            for _ in 0..frames {
                let _ = run.document().clone();
            }
            let cloned = cloned.elapsed();
            eprintln!(
                "neon json {frames} frames bytes {} dump {dump_once:?} dump+state {both:?} parse {parse:?} parse+reserialize {round:?} doc_clone {cloned:?}",
                text.len()
            );
        }
    }

    #[test]
    fn picture_quality_survives_rebuild_without_replacing_the_grade() {
        let plain = NeonRun::from_json(r#"{"version":1,"genre":"neon_run"}"#).unwrap();
        assert!(plain.document().quality.is_none());
        assert!((plain.document().bloom_intensity() - 0.14).abs() < 1e-6);
        if world_doc::heavy_post_allowed() && !world_doc::quality_is_high() {
            assert_eq!(plain.document().shadow_cascade_count(), 1);
            assert_eq!(plain.document().ssao_radius(), 0.0);
        }
        let mobile =
            NeonRun::from_json(r#"{"version":1,"genre":"neon_run","quality":"mobile"}"#).unwrap();
        let doc = mobile.document();
        assert_eq!(doc.quality.as_deref(), Some("mobile"));
        assert!((doc.bloom.unwrap() - 0.14).abs() < 1e-6);
        assert!(doc.fxaa_enabled());
        assert_eq!(doc.tonemap, Some(true));
        if world_doc::heavy_post_allowed() {
            assert!((doc.ssao_radius() - 4.0).abs() < 1e-6);
            assert!(!doc.taa_enabled());
            assert_eq!(doc.shadow_cascade_count(), 2);
            assert!(!doc.shadow_pcss_enabled());
        }
        let low = FrameProfile::from_world(
            NeonRun::from_json(r#"{"version":1,"genre":"neon_run","quality":"low"}"#)
                .unwrap()
                .document(),
        );
        let phone = FrameProfile::from_world(doc);
        assert_eq!(low.draws, phone.draws);
        assert_eq!(low.triangles, phone.triangles);
        assert_eq!(low.texture_bytes, phone.texture_bytes);
        assert!(
            phone.draws <= crate::profile::BUDGET_DRAWS,
            "draws {}",
            phone.draws
        );
        assert!(phone.texture_bytes <= crate::profile::BUDGET_TEXTURE_BYTES);
        let tri_warn = phone.warnings().iter().any(|w| w.contains("triangles"));
        if tri_warn {
            eprintln!(
                "neon already has {} triangles; the mobile tier did not add any",
                phone.triangles
            );
        }
    }
}
