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

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use serde_json::json;

use crate::flow::{FlowAction, FlowPhase, GameFlow};
use crate::game_core::{self, Core, Hitbox, Juice, JuiceShake, Kit, Spring1};
use crate::game_spec::{
    Behavior, GameSpec, ItemEffect, ItemIcon, MischiefKind, PartRole, PerchKind, SectionKind,
};
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc, WorldLight, WorldProp, WorldWalker};

/// How far the lens stays from any obstacle surface.
pub const CAMERA_MARGIN: f32 = 0.25;

pub const GAME_ID: &str = "neon_cat";

use crate::game_core::feel::{self, FeelParams};
use crate::game_spec::InputScheme;
use crate::gesture::{DragEvent, DragFlick, Gesture, InputBuffer, Swipe, SwipeRecognizer};

/// How long a swipe waits for the cat to be able to act on it.
pub const SWIPE_BUFFER_SECS: f32 = 0.15;
/// `hurt_t` value that means no recent hit.
const HURT_IDLE: f32 = 99.0;
const FX_MAGNET: usize = 0;
const FX_BOOST: usize = 1;
const FX_JUMP: usize = 2;
const FX_DOUBLE: usize = 3;

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
    /// Resolved `spec.feel`.
    feel: FeelParams,
    /// Eased lane hop in flight: (from, to, elapsed, duration).
    hop: Option<(f32, f32, f32, f32)>,
    /// Seconds of roll left. While > 0 the body is `roll_height` tall.
    rolling: f32,
    /// Seconds of standing back up left after a roll.
    recover: f32,
    /// Smoothed pose scale `[x, y, z]` of the cat (crouch, stretch, squash).
    pose: [f32; 3],
    /// When the last down flick happened (for the reverse guard).
    last_down: f32,
    /// A fast-fall asked for a roll on touchdown.
    roll_on_land: bool,
    /// Height of the surface under the feet (a train top or ramp), else 0.
    ground: f32,
    /// Coins picked up this run, and which ones (`index * 32 + k`).
    coins: u32,
    /// Stone arches run through this run.
    gates: u32,
    /// Seconds since the crash, while it plays out (drives the flash and shake).
    dying: f32,
    /// Hits left. The run ends when the last one goes.
    lives: u32,
    /// Seconds of protection left after a hit (the cat blinks).
    invuln: f32,
    /// Seconds since the last hit (drives the paw pop and the edge flash).
    hurt_t: f32,
    /// Seconds left of each timed pick-up: magnet, boost, super jump, 2x.
    fx: [f32; 4],
    /// Pick-up slots taken this run.
    got_items: HashSet<i32>,
    /// Knocked-over props by slot: play time of the knock and which way it flew.
    knocked: HashMap<i32, (f32, f32)>,
    streak: u32,
    streak_t: f32,
    knocks: u32,
    /// A line of text over the play view for a moment (a pick-up's name).
    toast: String,
    toast_t: f32,
    toast_c: [u8; 3],
    got_coins: HashSet<i32>,
    /// Coins the magnet has taken hold of: position and speed on the way in.
    pulls: HashMap<i32, ([f32; 3], f32)>,
    /// A score pop under the coin chip: seconds left and the running sum.
    pop_t: f32,
    pop_sum: u32,
    /// Route slots the cat has landed on, and reached the top of.
    perch_landed: HashSet<i32>,
    perch_routed: HashSet<i32>,
    /// Fraction of a point owed for time on a ledge.
    perch_acc: f32,
    /// Standing on a ledge right now.
    on_perch: bool,
    /// The cleaning robot: seconds of chase left (0 = none), seconds of
    /// running away left after a meow, and its own sideways position.
    robot_t: f32,
    robot_flee: f32,
    robot_age: f32,
    robot_x: f32,
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
    next_ahead_where(z, origin, gap, at, |_| true)
}

fn next_ahead_where(
    z: f32,
    origin: f32,
    gap: f32,
    at: impl Fn(i32) -> f32,
    keep: impl Fn(i32) -> bool,
) -> i32 {
    let mut i = near_index(z, origin, gap).max(0);
    if at(i) < z + 0.35 {
        i += 1;
    }
    // Skip slots a section left empty (bounded: sections cannot be all-empty forever).
    for _ in 0..4096 {
        if keep(i) {
            break;
        }
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

    /// One hit ends the run, as before lives. The geometry measurements
    /// (reachability, fairness) depend on a hit being a death.
    pub fn one_life(mut self) -> Self {
        std::sync::Arc::make_mut(&mut self.spec).lives.count = 1;
        self.lives = 1;
        self
    }

    #[cfg(test)]
    fn fresh() -> Self {
        Self::with_spec(GameSpec::legacy())
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
            feel: feel::resolve(&GameSpec::default().feel).expect("default feel"),
            hop: None,
            rolling: 0.0,
            recover: 0.0,
            pose: [1.0; 3],
            last_down: -10.0,
            roll_on_land: false,
            ground: 0.0,
            coins: 0,
            gates: 0,
            dying: 0.0,
            lives: 0,
            invuln: 0.0,
            hurt_t: HURT_IDLE,
            fx: [0.0; 4],
            got_items: HashSet::new(),
            knocked: HashMap::new(),
            streak: 0,
            streak_t: 0.0,
            knocks: 0,
            toast: String::new(),
            toast_t: 0.0,
            toast_c: [255, 245, 215],
            got_coins: HashSet::new(),
            pulls: HashMap::new(),
            pop_t: 0.0,
            pop_sum: 0,
            perch_landed: HashSet::new(),
            perch_routed: HashSet::new(),
            perch_acc: 0.0,
            on_perch: false,
            robot_t: 0.0,
            robot_flee: 0.0,
            robot_age: 0.0,
            robot_x: 0.0,
        };
        run.lives = run.spec.lives.count;
        if let Ok(p) = feel::resolve(&run.spec.feel) {
            run.feel = p;
        }
        run.drag.flick_dist = run.spec.input.flick_dist;
        run.drag.flick_angle_deg = run.spec.input.flick_angle_deg;
        run.swiper.min_dist = run.spec.input.swipe_dist;
        run.swiper.cooldown = run.spec.input.swipe_cooldown_secs;
        run.swipes.window = run
            .feel
            .jump_buffer_secs
            .unwrap_or(run.spec.input.buffer_secs);
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

    /// The line the result screen shows for the obstacle that ended the run.
    pub fn death_text(&self) -> String {
        let r = &self.spec.hud.result;
        let d = &self.core.stats.death;
        if d.is_empty() {
            return String::new();
        }
        r.deaths.get(d).unwrap_or(&r.death_fallback).clone()
    }

    /// What the result screen needs to rank this run, and how: the ranking key
    /// (game id and course seed), the columns and N, and this run's values.
    pub fn result_json(&self) -> serde_json::Value {
        let b = &self.spec.hud.result.board;
        let values = |c: &str| -> serde_json::Value {
            match c {
                "time" => json!((self.time as f64 * 10.0).round() / 10.0),
                "gates" => json!(self.gates),
                "coins" => json!(self.coins),
                _ => json!(self.core.stats.score),
            }
        };
        let run: serde_json::Map<String, serde_json::Value> = crate::game_spec::BOARD_COLUMNS
            .iter()
            .map(|c| (c.to_string(), values(c)))
            .collect();
        json!({
            "key": format!("{}:{}", self.spec.id, self.spec.seed),
            "title": self.spec.title,
            "enabled": b.enabled,
            "top_n": b.top_n,
            "columns": b.columns,
            "sort": b.sort,
            "run": run,
        })
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
        let oi = next_ahead_where(
            self.z,
            self.spec.runner.obstacle_track.z0,
            self.spec.runner.obstacle_track.gap,
            |i| self.spec.runner.obstacle_track.z(i),
            |i| self.spec.obstacle_active(i),
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
            "coins": self.coins,
            "gates": self.gates,
            "lives": self.lives,
            "fx": {
                "magnet": self.fx[FX_MAGNET],
                "boost": self.fx[FX_BOOST],
                "superjump": self.fx[FX_JUMP],
                "double": self.fx[FX_DOUBLE],
            },
            "perch": { "landed": self.perch_landed.len(), "routes": self.perch_routed.len(), "on": self.on_perch },
            "robot": { "secs": self.robot_t, "fleeing": self.robot_flee > 0.0 },
            "mischief": { "knocked": self.knocks, "streak": if self.streak_t > 0.0 { self.streak } else { 0 } },
            "toast": if self.toast_t > 0.0 { self.toast.as_str() } else { "" },
            "lives_max": self.spec.lives.count,
            "invulnerable": self.invuln > 0.0,
            "rolling": self.rolling > 0.0,
            "ground": self.ground,
            "alive": self.core.stats.alive,
            "death": self.core.stats.death,
            "death_text": self.death_text(),
            "time": self.time,
            "result": self.result_json(),
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

    /// The eye the rig asked for, then lifted out of any obstacle volume
    /// (plus a margin), so the lens never sits inside a mesh and floods the
    /// screen with its colour. A safety net for custom obstacle shapes.
    pub fn chase_eye(&self) -> [f32; 3] {
        let mut eye = self.raw_chase_eye();
        if self.classic {
            return eye;
        }
        let track = &self.spec.runner.obstacle_track;
        let i0 = near_index(eye[2], track.z0, track.gap) - 1;
        for i in i0..i0 + 4 {
            if i < 0 || !self.spec.obstacle_active(i) {
                continue;
            }
            for p in obstacle_parts(&self.spec, i) {
                if matches!(p.role, PartRole::Decor) {
                    continue;
                }
                let m = CAMERA_MARGIN;
                let inside =
                    (0..3).all(|a| eye[a] > p.skin_min[a] - m && eye[a] < p.skin_max[a] + m);
                if inside {
                    eye[1] = eye[1].max(p.skin_max[1] + m);
                }
            }
        }
        eye
    }

    /// The chase eye before the obstacle guard (the harness checks this one,
    /// so a bad spec shows up instead of being hidden).
    pub fn raw_chase_eye(&self) -> [f32; 3] {
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
        let (lx, jump, _) = self.script_plan();
        (lx, jump)
    }

    /// `script_steer` plus a roll, and an answer for every obstacle kind:
    /// hop crates, go around solids, roll under beams and through gaps, and
    /// line up with a ramp to ride a train.
    pub fn script_plan(&self) -> (f32, bool, bool) {
        let ri = next_ahead(
            self.z,
            self.spec.runner.rings.track.z0,
            self.spec.runner.rings.track.gap,
            |i| self.spec.runner.rings.track.z(i),
        );
        let dx = ring_x(&self.spec, ri) - self.x;
        let mut lx = (-dx * 1.4).clamp(-1.0, 1.0);
        let oi = next_ahead_where(
            self.z,
            self.spec.runner.obstacle_track.z0,
            self.spec.runner.obstacle_track.gap,
            |i| self.spec.runner.obstacle_track.z(i),
            |i| self.spec.obstacle_active(i),
        );
        let parts = obstacle_parts(&self.spec, oi);
        let kind = self.spec.obstacle_kind(oi);
        let (ox, oz) = obstacle_anchor(&self.spec, oi);
        let front = parts
            .iter()
            .map(|p| p.skin_min[2])
            .fold(f32::INFINITY, f32::min);
        let lim = self.spec.runner.lanes.limit;
        let steer_to = |target: f32| (-(target - self.x) * 3.0).clamp(-1.0, 1.0);
        let (mut jump, mut roll) = (false, false);
        match kind.behavior {
            Behavior::JumpOver => {
                let base = &parts[0];
                let ahead = base.hit_cz - self.z;
                jump = (0.15..2.5).contains(&ahead) && (self.x - base.hit_cx).abs() < 0.95;
            }
            Behavior::Solid => {
                if oz - self.z < 12.0 {
                    let free = game_core::collide::free_between(
                        &game_core::collide::blocked_spans(
                            &parts,
                            0.0,
                            self.spec.runner.body.stand_height,
                            CAT_HALF_W,
                        ),
                        lim,
                    );
                    // The free strip nearest the cat, held a hand from its edges.
                    let near = free.iter().min_by(|a, b| {
                        let d = |s: &&(f32, f32)| (self.x.clamp(s.0, s.1) - self.x).abs();
                        d(a).partial_cmp(&d(b)).unwrap()
                    });
                    if let Some(&(lo, hi)) = near {
                        let m = ((hi - lo) * 0.5).min(0.2);
                        lx = steer_to(self.x.clamp(lo + m, hi - m));
                    }
                }
            }
            Behavior::RollUnder => {
                roll = (0.0..1.6).contains(&(front - self.z)) && self.rolling <= 0.0;
            }
            Behavior::CatGap => {
                if oz - self.z < 12.0 {
                    lx = steer_to(ox);
                }
                roll = (0.0..1.6).contains(&(front - self.z)) && self.rolling <= 0.0;
            }
            Behavior::Rideable => {
                if oz - self.z < 12.0 {
                    lx = steer_to(ox);
                }
            }
        }
        (lx, jump, roll)
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
        self.reset_run();
        self.attract = 0.0;
        self.jump_held = false;
        self.key_left = false;
        self.key_right = false;
        self.key_jump = false;
        self.pad_x = 0.0;
        self.pad_up = 0.0;
        self.rebuild();
    }

    /// Everything that belongs to one run, back to its first-frame value.
    /// Title and retry both go through here, so a counter cannot be left
    /// behind by one of them.
    fn reset_run(&mut self) {
        self.reset_motion();
        let stats = &mut self.core.stats;
        stats.alive = true;
        stats.death.clear();
        stats.score = 0;
        stats.score_acc = 0.0;
        stats.rings = 0;
        self.got.clear();
        self.got_coins.clear();
        self.pulls.clear();
        self.pop_t = 0.0;
        self.pop_sum = 0;
        self.perch_landed.clear();
        self.perch_routed.clear();
        self.perch_acc = 0.0;
        self.on_perch = false;
        self.robot_t = 0.0;
        self.robot_flee = 0.0;
        self.robot_age = 0.0;
        self.coins = 0;
        self.gates = 0;
        self.dying = 0.0;
        self.lives = self.spec.lives.count;
        self.invuln = 0.0;
        self.hurt_t = HURT_IDLE;
        self.fx = [0.0; 4];
        self.got_items.clear();
        self.knocked.clear();
        self.streak = 0;
        self.streak_t = 0.0;
        self.knocks = 0;
        self.toast.clear();
        self.toast_t = 0.0;
        self.toast_c = [255, 245, 215];
        self.rolling = 0.0;
        self.recover = 0.0;
        self.pose = [1.0; 3];
        self.last_down = -10.0;
        self.roll_on_land = false;
        self.ground = 0.0;
        self.core.juice.sparks.clear();
        self.time = 0.0;
        self.anim_t = 0.05;
        self.swipes.clear();
        self.lane_target = None;
        self.drag_target = None;
        self.hop = None;
        self.drag.reset();
        self.swiper.reset();
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
            "a" | "arrowleft" | "left" if self.feel.key_lanes => {
                if down {
                    self.swipe(Swipe::Left);
                }
                return true;
            }
            "d" | "arrowright" | "right" if self.feel.key_lanes => {
                if down {
                    self.swipe(Swipe::Right);
                }
                return true;
            }
            "s" | "arrowdown" | "down" if self.feel.key_lanes => {
                if down {
                    self.swipe(Swipe::Down);
                }
                return true;
            }
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
        self.reset_run();
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
            Swipe::Up => {
                // The finger coming back up after a down flick is not a jump.
                if self.time - self.last_down >= self.spec.input.reverse_guard_secs {
                    self.swipes.press(Swipe::Up, self.time);
                }
            }
            Swipe::Down => {
                self.last_down = self.time;
                if self.y > self.ground + 0.05 {
                    // Fast-fall, and roll the moment the paws touch down.
                    self.vy = self
                        .vy
                        .min(-self.spec.runner.jump.velocity * self.feel.velocity_scale * 1.2);
                    self.roll_on_land = true;
                } else {
                    // On the ground: roll now (buffered a hair for a press
                    // that lands just before the cat does).
                    self.swipes.press(Swipe::Down, self.time);
                }
            }
        }
    }

    pub fn tick(&mut self, dt: f32) {
        game_core::tick_frame(self, dt);
    }

    /// One sim step. A 2x pick-up doubles whatever the step scored, whichever
    /// rule scored it (distance, coin, ring, gate).
    fn play_step(&mut self, dt: f32) {
        let before = self.core.stats.score;
        self.play_step_inner(dt);
        if self.fx[FX_DOUBLE] > 0.0 && self.core.stats.alive {
            let gained = self.core.stats.score.saturating_sub(before);
            self.core.stats.score += gained;
        }
        for t in &mut self.fx {
            *t = (*t - dt).max(0.0);
        }
        self.update_robot(dt);
        self.toast_t = (self.toast_t - dt).max(0.0);
        self.streak_t = (self.streak_t - dt).max(0.0);
        self.pop_t = (self.pop_t - dt).max(0.0);
        if self.pop_t <= 0.0 {
            self.pop_sum = 0;
        }
    }

    fn play_step_inner(&mut self, dt: f32) {
        self.time += dt;
        self.invuln = (self.invuln - dt).max(0.0);
        self.hurt_t = (self.hurt_t + dt).min(HURT_IDLE);
        let ax = self.axis_x();
        // +ax is screen-right. This camera looks along +Z, so that is world -X.
        let steer = -ax;
        let response = 1.0 - (-dt * 8.0).exp();
        self.vx += (steer * self.spec.runner.lanes.steer - self.vx) * response;
        if let Some(t) = self.drag_target {
            let k = 1.0 - (-dt / self.spec.input.drag_smooth_secs.max(1e-3)).exp();
            // Capped so a fast drag still reads as motion, not a teleport.
            let cap = self.spec.runner.lanes.steer * 3.0;
            self.vx = ((t - self.x) * k / dt.max(1e-4)).clamp(-cap, cap);
        } else if let Some(t) = self.lane_target {
            // Ease into the lane centre without overshoot, capped at steer speed.
            let held = (self.key_right as i32 - self.key_left as i32) as f32 + self.pad_x;
            if held.abs() < 0.2 {
                if let Some(per_lane) = self.feel.lane_hop_secs {
                    // Eased hop: duration scales with the distance in lanes.
                    let w = self.spec.runner.lanes.width.max(0.1);
                    let (from, to, el, dur) = match self.hop {
                        Some(h) if h.1 == t => h,
                        _ => (self.x, t, 0.0, per_lane * ((t - self.x).abs() / w).max(0.5)),
                    };
                    let el = (el + dt).min(dur);
                    let nx = from + (to - from) * feel::ease_out(el / dur);
                    self.vx = (nx - self.x) / dt.max(1e-4);
                    self.hop = if el >= dur {
                        None
                    } else {
                        Some((from, to, el, dur))
                    };
                } else {
                    let k = 1.0 - (-dt / 0.06).exp();
                    let cap = self.spec.runner.lanes.steer * 1.5;
                    self.vx = ((t - self.x) * k / dt.max(1e-4)).clamp(-cap, cap);
                }
            }
        }
        let g0 = self.ground_at(self.x, self.z);
        self.ground = g0;
        let grounded = self.y <= g0 + 0.001 && self.vy <= 0.0;
        let was_rolling = self.rolling > 0.0;
        self.rolling = (self.rolling - dt).max(0.0);
        self.recover = (self.recover - dt).max(0.0);
        if was_rolling && self.rolling <= 0.0 {
            // Stand back up: the body and the pose rise together.
            self.recover = self.spec.runner.body.recover_secs;
            if grounded {
                self.burst(
                    self.x,
                    self.y + 0.06,
                    self.z - 0.1,
                    5,
                    [170, 210, 255],
                    1.6,
                    0.06,
                );
            }
        }
        if let Some(t) = self.lane_target {
            let held = (self.key_right as i32 - self.key_left as i32) as f32 + self.pad_x;
            if held.abs() >= 0.2 {
                self.lane_target = None;
                self.hop = None;
            } else if (self.x - t).abs() < 0.02 {
                // Arrived: land on the centre and stop, no drift.
                self.x = t;
                self.vx = 0.0;
                self.lane_target = None;
                self.hop = None;
            }
        }
        let want_jump = self.jump_held || self.key_jump || self.pad_up > 0.55;
        if grounded && (want_jump || self.swipes.take(Swipe::Up, self.time)) {
            let boost = if self.fx[FX_JUMP] > 0.0 {
                self.spec.item_rules.jump_scale
            } else {
                1.0
            };
            let vy = self.spec.runner.jump.velocity * self.feel.velocity_scale * boost;
            self.vy = self.perch_hop(g0 + 0.02, vy);
            self.y = g0 + 0.02;
            self.rolling = 0.0;
            self.recover = 0.0;
            self.roll_on_land = false;
            self.core.stats.just_jump = true;
            self.sfx("jump");
        } else if grounded && (self.roll_on_land || self.swipes.take(Swipe::Down, self.time)) {
            self.roll_on_land = false;
            // One roll at a time: a repeated press never stretches it.
            if self.rolling <= 0.0 {
                self.start_roll();
            }
        }
        let pull = self.perch_fall_pull(dt);
        self.vy += self.spec.runner.jump.gravity * self.feel.gravity_scale * pull * dt;
        self.y += self.vy * dt;
        if self.y <= g0 {
            if self.vy < -1.5 {
                self.core.juice.land_squash = self.feel.land_squash_secs;
                if !self.classic {
                    // Same defaults as `kagra.juice.Juice.shake`.
                    let (a, d) = self.feel.land_shake;
                    if a > 0.0 {
                        self.core.juice.land.shake(a, d);
                    }
                }
            }
            self.y = g0;
            self.vy = 0.0;
        }
        self.core.juice.land_squash = (self.core.juice.land_squash - dt).max(0.0);
        let (x0, z0) = (self.x, self.z);
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
        self.core
            .stats
            .add_points(speed * dt * self.spec.runner.score.per_meter);
        self.count_gates();
        self.anim_t += dt * if self.y > 0.05 { 0.7 } else { 1.15 };
        let target = self.pose_target();
        let k = 1.0 - (-dt * 22.0).exp();
        for (p, t) in self.pose.iter_mut().zip(target) {
            *p += (t - *p) * k;
        }
        // Grace covers the spawn stretch. The rail is not a kill plane,
        // so steering into it during grace cannot strand the cat outside.
        if self.time >= self.spec.runner.jump.grace_secs {
            self.touch_obstacles_swept(x0, z0);
        }
        if self.core.stats.alive {
            // Step up onto a ramp or a train top that came under the feet.
            let g1 = self.ground_at(self.x, self.z);
            if self.y < g1 && self.vy <= 0.0 {
                self.y = g1;
                self.vy = 0.0;
            } else if self.y < g1 && self.vy > 0.0 {
                // Jumping up into a ledge that is within reach lands on it
                // (a late jump no longer rises through the front of a step).
                if let Some((_, _, top)) = self.perch_support(self.x, self.z) {
                    if top >= g1 {
                        self.y = top;
                        self.vy = 0.0;
                        self.core.juice.land_squash = self.feel.land_squash_secs;
                    }
                }
            }
            self.ground = g1;
            self.collect_rings();
            self.collect_coins(dt);
            self.collect_items();
            self.collect_mischief();
            self.track_perch(dt);
        }
    }

    /// Standing height now: the low hitbox while rolling.
    fn body_height(&self) -> f32 {
        let b = &self.spec.runner.body;
        if self.rolling > 0.0 {
            b.roll_height
        } else if self.recover > 0.0 && b.recover_secs > 0.0 {
            let u = 1.0 - (self.recover / b.recover_secs).clamp(0.0, 1.0);
            let e = u * u * (3.0 - 2.0 * u);
            b.roll_height + (b.stand_height - b.roll_height) * e
        } else {
            b.stand_height
        }
    }

    /// Where the pose wants to be: crouched while rolling, rising during the
    /// recovery, stretched on the way up and down, squashed on touchdown.
    fn pose_target(&self) -> [f32; 3] {
        let b = &self.spec.runner.body;
        let crouch = if self.rolling > 0.0 {
            1.0
        } else if self.recover > 0.0 && b.recover_secs > 0.0 {
            let k = (self.recover / b.recover_secs).clamp(0.0, 1.0);
            k * k
        } else {
            0.0
        };
        let mut t = [
            1.0 + (b.crouch[0] - 1.0) * crouch,
            1.0 + (b.crouch[1] - 1.0) * crouch,
            1.0 + (b.crouch[2] - 1.0) * crouch,
        ];
        if crouch <= 0.0 {
            if self.y > self.ground + 0.15 {
                let up = (self.vy
                    / (self.spec.runner.jump.velocity * self.feel.velocity_scale).max(0.1))
                .abs()
                .clamp(0.0, 1.3);
                let ys = 1.0 + 0.14 * up;
                let xz = 1.0 / ys.sqrt();
                t = [xz, ys, xz];
            } else if self.core.juice.land_squash > 0.0 {
                let k = (self.core.juice.land_squash / self.feel.land_squash_secs.max(1e-3))
                    .clamp(0.0, 1.0);
                t = [1.0 + 0.12 * k, 1.0 - 0.22 * k, 1.0 + 0.12 * k];
            }
        }
        t
    }

    fn start_roll(&mut self) {
        self.rolling = self.spec.runner.body.roll_secs;
        self.core.juice.shake = self.core.juice.shake.max(0.03);
        self.burst(
            self.x,
            self.y + 0.08,
            self.z - 0.2,
            6,
            [150, 200, 255],
            2.2,
            0.08,
        );
        self.sfx("roll");
    }

    /// Height of the highest ramp or ride top under the feet at `(x, z)`.
    fn ground_at(&self, x: f32, z: f32) -> f32 {
        let ledge = self.perch_support(x, z).map_or(0.0, |p| p.2);
        if !self.spec.has_rideables() {
            return ledge;
        }
        let i0 = near_index(
            z,
            self.spec.runner.obstacle_track.z0,
            self.spec.runner.obstacle_track.gap,
        )
        .saturating_sub(1);
        let mut best = 0.0f32;
        for i in i0..i0 + 3 {
            if i < 0 || !self.spawned.contains(&i) {
                continue;
            }
            for part in obstacle_parts(&self.spec, i) {
                if matches!(part.role, PartRole::Ride | PartRole::Ramp) {
                    best = best.max(part.support(x, z, CAT_HALF_W));
                }
            }
        }
        best.max(ledge)
    }

    /// The perch ledge under `(x, z)` that can hold the cat: the route slot,
    /// the step and its top. A ledge more than `step_up` above the feet is
    /// walked through (a jump gets onto it), one within reach is stepped onto.
    fn perch_support(&self, x: f32, z: f32) -> Option<(i32, usize, f32)> {
        if self.spec.perch_route.is_empty() {
            return None;
        }
        let track = &self.spec.runner.obstacle_track;
        let i0 = near_index(z, track.z0, track.gap).saturating_sub(1);
        let up = self.spec.perch_rules.step_up;
        let mut best: Option<(i32, usize, f32)> = None;
        for i in i0..i0 + 3 {
            let Some(steps) = perch_route_at(&self.spec, i) else {
                continue;
            };
            for (n, b) in steps.iter().enumerate() {
                if (x - b.x).abs() <= b.half_width
                    && z >= b.z0
                    && z < b.z1
                    && self.y >= b.top - up
                    && best.is_none_or(|q| b.top > q.2)
                {
                    best = Some((i, n, b.top));
                }
            }
        }
        best
    }

    /// The first route step ahead that a jump with take-off speed `vy` from
    /// `y0` can get onto: it starts ahead of the feet and rises above them,
    /// but not above the top of the arc. `(z0, z1, top)`.
    fn perch_target(&self, y0: f32, vy: f32) -> Option<(f32, f32, f32)> {
        let g = self.spec.runner.jump.gravity * self.feel.gravity_scale;
        let v = self.speed();
        if g >= 0.0 || v <= 0.1 {
            return None;
        }
        let apex = y0 + vy * vy / (2.0 * -g);
        let reach = v * (2.0 * vy / -g).max(0.0);
        let track = &self.spec.runner.obstacle_track;
        let i0 = near_index(self.z, track.z0, track.gap).saturating_sub(1);
        let i1 = near_index(self.z + reach, track.z0, track.gap) + 1;
        let mut best: Option<(f32, f32, f32)> = None;
        for i in i0..=i1 {
            let Some(steps) = perch_route_at(&self.spec, i) else {
                continue;
            };
            for b in &steps {
                if (self.x - b.x).abs() > b.half_width
                    || b.z1 <= self.z
                    || b.top <= y0 + 0.05
                    || b.top > apex - 0.1
                    || best.is_some_and(|q| q.0 <= b.z0)
                {
                    continue;
                }
                best = Some((b.z0, b.z1, b.top));
            }
        }
        best
    }

    /// Whether the arc `(y0, vy)` comes down on `step` short of its far end.
    fn perch_arc_lands(&self, y0: f32, vy: f32, step: (f32, f32, f32)) -> bool {
        let g = self.spec.runner.jump.gravity * self.feel.gravity_scale;
        let disc = vy * vy - 2.0 * g * (y0 - step.2);
        if g >= 0.0 || disc < 0.0 {
            return false;
        }
        let z = self.z + self.speed() * (vy + disc.sqrt()) / -g;
        z >= step.0 && z < step.1 - 0.2
    }

    /// Take-off speed for a jump from height `y0`: the full jump, unless that
    /// arc would clear the first ledge ahead and a slightly lower hop comes
    /// down on it (a late jump at running speed). Never lower than clears it.
    fn perch_hop(&self, y0: f32, vy: f32) -> f32 {
        if self.spec.perch_route.is_empty() {
            return vy;
        }
        let Some(step) = self.perch_target(y0, vy) else {
            return vy;
        };
        if self.perch_arc_lands(y0, vy, step) {
            return vy;
        }
        let mut v = vy - 0.05;
        while v * v
            > 2.0 * -(self.spec.runner.jump.gravity * self.feel.gravity_scale) * (step.2 - y0)
        {
            if self.perch_arc_lands(y0, v, step) {
                return v;
            }
            v -= 0.05;
        }
        vy
    }

    /// Gravity multiplier that brings a falling cat down onto the ledge it is
    /// over before the ledge ends. A jump over a short step at running speed
    /// lasts longer than the step is long, so without this the cat flies over
    /// the first step and lands on the next. 1.0 when it lands anyway, when
    /// there is no ledge below, or when no firm pull could make it.
    fn perch_fall_pull(&self, dt: f32) -> f32 {
        const MAX_PULL: f32 = 6.0;
        if self.spec.perch_route.is_empty() || self.vy >= 0.0 {
            return 1.0;
        }
        let g = self.spec.runner.jump.gravity * self.feel.gravity_scale;
        let v = self.speed();
        if g >= 0.0 || v <= 0.1 {
            return 1.0;
        }
        let track = &self.spec.runner.obstacle_track;
        let i0 = near_index(self.z, track.z0, track.gap).saturating_sub(1);
        let mut pull = 1.0f32;
        for i in i0..i0 + 3 {
            let Some(steps) = perch_route_at(&self.spec, i) else {
                continue;
            };
            for b in &steps {
                if (self.x - b.x).abs() > b.half_width
                    || self.z < b.z0
                    || self.z >= b.z1
                    || self.y <= b.top + 0.01
                {
                    continue;
                }
                // Land by a little before the ledge ends, within this tick at the latest.
                let t = ((b.z1 - 0.2 - self.z) / v).max(dt);
                let need = 2.0 * (b.top - self.y - self.vy * t) / (t * t);
                let k = need / g;
                if k > 1.0 && k <= MAX_PULL {
                    pull = pull.max(k);
                }
            }
        }
        pull
    }

    /// Score and toast for landing on a route, time spent on a ledge and
    /// reaching the top step.
    fn track_perch(&mut self, dt: f32) {
        self.on_perch = false;
        let Some((slot, step, top)) = self.perch_support(self.x, self.z) else {
            return;
        };
        if (self.y - top).abs() > 0.03 {
            return;
        }
        self.on_perch = true;
        let rules = self.spec.perch_rules.clone();
        let c = self.spec.style.codes.rideable;
        self.perch_acc += rules.run_score as f32 * dt;
        let whole = self.perch_acc.floor();
        self.perch_acc -= whole;
        self.core.stats.score += whole as u32;
        if self.perch_landed.insert(slot) {
            self.core.stats.score += rules.land_score;
            self.toast = "PERCH".into();
            self.toast_t = 1.0;
            self.toast_c = c;
            self.burst(
                self.x,
                top + 0.1,
                self.z,
                8,
                [c[0] as u32, c[1] as u32, c[2] as u32],
                2.6,
                0.08,
            );
            self.sfx("perch");
        }
        if step + 1 == self.spec.perch_route.len() && self.perch_routed.insert(slot) {
            self.core.stats.score += rules.route_bonus;
            self.toast = format!("ROUTE  +{}", rules.route_bonus);
            self.toast_t = 1.4;
            self.toast_c = [255, 245, 215];
            self.burst(self.x, top + 0.2, self.z, 18, [255, 240, 150], 3.6, 0.1);
            self.sfx("coin");
        }
    }

    /// The robot's clock: it runs down faster on a ledge (or under a boost),
    /// and a meow sends it away.
    fn update_robot(&mut self, dt: f32) {
        if self.robot_t <= 0.0 || !self.core.stats.alive {
            return;
        }
        self.robot_age += dt;
        if self.robot_flee > 0.0 {
            self.robot_flee = (self.robot_flee - dt).max(0.0);
            if self.robot_flee <= 0.0 {
                self.robot_t = 0.0;
            }
            return;
        }
        let fast = self.on_perch || self.fx[FX_BOOST] > 0.0;
        let k = if fast {
            self.spec.chaser.perch_drain
        } else {
            1.0
        };
        self.robot_t = (self.robot_t - dt * k).max(0.0);
        let follow = 1.0 - (-dt * 3.0).exp();
        self.robot_x += (self.x - self.robot_x) * follow;
        if self.robot_t <= 0.0 {
            let (rx, rz) = (self.robot_x, self.z - self.spec.chaser.far_gap);
            self.burst(rx, 0.3, rz, 8, [200, 210, 230], 2.0, 0.08);
            self.sfx("robot_off");
        }
    }

    /// A hit calls the robot (once). It starts behind the cat, in sight.
    fn call_robot(&mut self) {
        if !self.spec.chaser.enabled || self.robot_t > 0.0 {
            return;
        }
        self.robot_t = self.spec.chaser.secs;
        self.robot_flee = 0.0;
        self.robot_age = 0.0;
        self.robot_x = self.x;
        self.sfx("robot_on");
    }

    /// How far behind the cat the robot is now: right on its tail when it is
    /// called, falling back as its clock runs down, and gone fast after a meow.
    fn robot_gap(&self) -> f32 {
        let c = &self.spec.chaser;
        let used = 1.0 - (self.robot_t / c.secs.max(0.1)).clamp(0.0, 1.0);
        let gap = c.near_gap + (c.far_gap - c.near_gap) * used;
        if self.robot_flee > 0.0 {
            let t = (1.0 - self.robot_flee).max(0.0);
            gap + t * t * 40.0
        } else {
            gap
        }
    }

    /// Obstacles drawn ahead. At least `self.spec.runner.draw.lead_secs` of travel, never fewer than 4.
    fn obstacle_lead_count(&self) -> i32 {
        ((self.speed() * self.spec.runner.draw.lead_secs / self.spec.runner.obstacle_track.gap)
            .ceil() as i32
            + 1)
        .max(self.spec.runner.draw.min_obstacles_ahead)
    }

    /// Fog and line fade `(start, end)`. The far end never passes the first
    /// obstacle that is not drawn yet, so nothing pops in at the horizon.
    fn fog_range(&self) -> (f32, f32) {
        let d = &self.spec.runner.draw;
        let gap = self.spec.runner.obstacle_track.gap;
        let undrawn = self.obstacle_lead_count() as f32 * gap - 6.0;
        let end = d.fog_end.min(undrawn).max(20.0);
        (d.fog_start.min(end * 0.4), end)
    }

    /// Tunnel floor/walls reach past the farthest drawn obstacle.
    fn tunnel_reach(&self) -> f32 {
        (self.speed() * self.spec.runner.draw.lead_secs + self.spec.runner.obstacle_track.gap * 2.0)
            .max(44.0)
    }

    fn speed(&self) -> f32 {
        let base = (self.spec.runner.speed.start + self.time * self.spec.runner.speed.ramp)
            .min(self.spec.runner.speed.cap);
        if self.fx[FX_BOOST] > 0.0 {
            base * self.spec.item_rules.boost_speed
        } else {
            base
        }
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
        let f = &self.feel;
        let (follow, lift, look, bank) = (f.cam_follow, f.cam_lift, f.cam_look, f.cam_bank);
        // Look-ahead leads the aim toward where the cat is heading.
        let lead = if f.look_ahead { self.vx * 0.06 } else { 0.0 };
        self.eye_x.step(self.x, follow, dt);
        self.eye_y.step(want_y, lift, dt);
        self.look_x.step(self.x * 0.9 + lead, look, dt);
        self.bank.step(want_roll, bank, dt);
        self.fov_spring.step(self.fov_for_speed(), 2.4, dt);
    }

    #[cfg(test)]
    fn touch_obstacles(&mut self) {
        self.touch_obstacles_swept(self.x, self.z);
    }

    /// Full x/z sweep of this substep's feet path against nearby boxes.
    fn touch_obstacles_swept(&mut self, x0: f32, z0: f32) {
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
            let h = self.body_height();
            for part in obstacle_parts(&self.spec, i) {
                if part.sweep_hit_body(x0, z0, self.x, self.z, self.y, h, CAT_HALF_W) {
                    self.crash(&part.kind);
                    return;
                }
            }
        }
    }

    /// Each stone arch the cat has passed counts once and pays its bonus.
    fn count_gates(&mut self) {
        let g = &self.spec.runner.gates;
        if self.z < g.z0 {
            return;
        }
        let passed = ((self.z - g.z0) / g.gap.max(0.1)).floor() as u32 + 1;
        while self.gates < passed {
            self.gates += 1;
            self.core.stats.score += self.spec.runner.score.per_gate;
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
            self.sfx("card");
        }
    }

    /// Coins within reach of the paws (a body-sized box), by `index * 32 + k`.
    fn collect_coins(&mut self, dt: f32) {
        let c = &self.spec.runner.coins;
        if !c.enabled {
            return;
        }
        let magnet = self.fx[FX_MAGNET] > 0.0;
        let rules = self.spec.item_rules.clone();
        let (score, pad) = (c.score, 0.55);
        let track = &self.spec.runner.obstacle_track;
        let i0 = near_index(self.z, track.z0, track.gap).saturating_sub(1);
        let mid = self.y + self.body_height() * 0.5;
        let mut taken = Vec::new();
        let mut grabbed = Vec::new();
        for i in i0..i0 + 3 {
            if i < 0 {
                continue;
            }
            for (k, p) in coin_line(&self.spec, i).into_iter().enumerate() {
                let id = i * 32 + k as i32;
                if self.got_coins.contains(&id) {
                    continue;
                }
                let near = (self.x - p[0]).abs() < pad
                    && (self.z - p[2]).abs() < pad
                    && (mid - p[1]).abs() < 0.75;
                if near {
                    taken.push((id, p));
                    continue;
                }
                // The field is a ball with a longer reach ahead of the cat.
                let in_field = magnet && {
                    let (dx, dy, dz) = (p[0] - self.x, p[1] - mid, p[2] - self.z);
                    let k = if dz > 0.0 {
                        rules.magnet_radius / (rules.magnet_radius + rules.magnet_lead)
                    } else {
                        1.0
                    };
                    dx * dx + dy * dy + (dz * k) * (dz * k)
                        < rules.magnet_radius * rules.magnet_radius
                };
                if in_field {
                    grabbed.push((id, p));
                }
            }
        }
        for (id, p) in taken {
            self.got_coins.insert(id);
            self.coin_landed(p, score);
        }
        for (id, p) in grabbed {
            self.got_coins.insert(id);
            self.pulls.insert(id, (p, rules.pull_speed));
        }
        // Pulled coins accelerate toward the cat and land when they reach it.
        let target = [self.x, mid, self.z];
        let ids: Vec<i32> = self.pulls.keys().copied().collect();
        for id in ids {
            let (mut p, mut v) = self.pulls[&id];
            let d = [target[0] - p[0], target[1] - p[1], target[2] - p[2]];
            let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            v += rules.pull_accel * dt;
            let step = v * dt;
            if dist <= step + 0.4 {
                self.pulls.remove(&id);
                self.coin_landed(target, score);
            } else {
                for k in 0..3 {
                    p[k] += d[k] / dist * step;
                }
                self.pulls.insert(id, (p, v));
            }
        }
    }

    /// One coin has arrived: count it, spark, and add to the score pop.
    fn coin_landed(&mut self, at: [f32; 3], score: u32) {
        self.coins += 1;
        self.core.stats.score += score;
        self.pop_sum += score;
        self.pop_t = 0.6;
        self.burst(at[0], at[1], at[2], 4, [255, 214, 90], 2.0, 0.07);
        self.sfx("coin");
    }

    /// Knock over the props the cat runs into (jumping clears them).
    fn collect_mischief(&mut self) {
        let r = self.spec.mischief_rules.radius;
        let track = &self.spec.runner.obstacle_track;
        let i0 = near_index(self.z, track.z0, track.gap).saturating_sub(1);
        let mut hit = Vec::new();
        for i in i0..i0 + 3 {
            if i < 0 || self.knocked.contains_key(&i) {
                continue;
            }
            if let Some((kind, p)) = mischief_at(&self.spec, i) {
                let (_, h) = mischief_shape(kind);
                if (self.x - p[0]).abs() < r && (self.z - p[2]).abs() < 0.6 && self.y < h * 0.8 {
                    hit.push((i, kind, p));
                }
            }
        }
        for (i, kind, p) in hit {
            let rules = self.spec.mischief_rules.clone();
            self.streak = if self.streak_t > 0.0 {
                self.streak + 1
            } else {
                1
            };
            self.streak_t = rules.streak_secs;
            self.knocks += 1;
            let step = self.streak.min(rules.streak_cap) - 1;
            self.core.stats.score += rules.score + rules.streak_bonus * step;
            let dir = if p[0] >= self.x { 1.0 } else { -1.0 };
            self.knocked.insert(i, (self.time, dir));
            self.toast = if self.streak > 1 {
                format!("{} x{}", mischief_label(kind), self.streak)
            } else {
                mischief_label(kind).to_string()
            };
            self.toast_t = 1.1;
            let c = mischief_color(kind);
            self.toast_c = [255, 245, 215];
            self.burst(
                p[0],
                p[1] + 0.3,
                p[2],
                10,
                [c[0] as u32, c[1] as u32, c[2] as u32],
                3.0,
                0.09,
            );
            self.sfx("knock");
        }
    }

    /// The props ahead, standing or flying: a fixed arc from the knock time.
    fn push_mischief(&self, doc: &mut WorldDoc) {
        let track = &self.spec.runner.obstacle_track;
        let i0 = near_index(self.z, track.z0, track.gap).saturating_sub(1);
        let reach = self.z + 6.0 + self.obstacle_lead_count() as f32 * track.gap;
        for i in i0..i0 + 2 + self.obstacle_lead_count() {
            if i < 0 {
                continue;
            }
            let Some((kind, p)) = mischief_at(&self.spec, i) else {
                continue;
            };
            if p[2] < self.z - 4.0 || p[2] > reach {
                continue;
            }
            let (parts, _) = mischief_shape(kind);
            let c = mischief_color(kind);
            let (mut pos, mut tilt, mut spin) = (p, 0.0f32, 0.0f32);
            if let Some(&(t0, d)) = self.knocked.get(&i) {
                let age = (self.time - t0).clamp(0.0, 0.8);
                let land = 0.8;
                pos[0] += d * 2.6 * age;
                pos[1] += 3.0 * age - 0.5 * 7.5 * age * age;
                pos[2] += 2.0 * age;
                pos[1] = pos[1].max(p[1] + 0.12 * (age / land));
                tilt = d * (age / land) * std::f32::consts::FRAC_PI_2;
                spin = d * age * 5.0;
            }
            for (n, part) in parts.iter().enumerate() {
                // The part's offset along the prop's own up axis, tipped by `tilt`.
                let off = [tilt.sin() * part.dy, tilt.cos() * part.dy];
                let col = [c[0] as u32, c[1] as u32, c[2] as u32];
                let mut q = glow(
                    &format!("mis-{i}-{n}"),
                    part.model,
                    "mischief",
                    [pos[0] + off[0], pos[1] + off[1], pos[2]],
                    part.scale,
                    if part.light {
                        [
                            (c[0] as u32 + 255) / 2,
                            (c[1] as u32 + 255) / 2,
                            (c[2] as u32 + 255) / 2,
                        ]
                    } else {
                        col
                    },
                    0.7,
                );
                q.roll = -tilt;
                q.yaw = spin;
                if part.model == "torus" {
                    q.pitch = 1.2;
                }
                doc.props.push(q);
            }
        }
    }

    /// Take the pick-ups the cat touches this step.
    fn collect_items(&mut self) {
        let r = self.spec.item_rules.radius;
        let track = &self.spec.runner.obstacle_track;
        let i0 = near_index(self.z, track.z0, track.gap).saturating_sub(1);
        let mid = self.y + self.body_height() * 0.5;
        let mut hit = Vec::new();
        for i in i0..i0 + 3 {
            if i < 0 || self.got_items.contains(&i) {
                continue;
            }
            if let Some((effect, p)) = item_at(&self.spec, i) {
                if (self.x - p[0]).abs() < r
                    && (self.z - p[2]).abs() < r
                    && (mid - p[1]).abs() < r + 0.3
                {
                    hit.push((i, effect, p));
                }
            }
        }
        for (i, effect, p) in hit {
            self.got_items.insert(i);
            self.take_item(effect, p);
        }
    }

    /// The colour of an effect in this spec: orb, toast, label and floor ring.
    fn item_color(&self, effect: ItemEffect) -> [f32; 3] {
        let c = self
            .spec
            .items
            .iter()
            .find(|it| it.effect == effect)
            .map(|it| it.rgb())
            .unwrap_or_else(|| effect.default_color());
        [c[0] as f32, c[1] as f32, c[2] as f32]
    }

    fn take_item(&mut self, effect: ItemEffect, at: [f32; 3]) {
        let secs = self
            .spec
            .items
            .iter()
            .find(|it| it.effect == effect)
            .map(|it| it.secs())
            .unwrap_or_else(|| effect.default_secs());
        let c = self.item_color(effect);
        let color = [c[0] as u32, c[1] as u32, c[2] as u32];
        self.toast_c = [c[0] as u8, c[1] as u8, c[2] as u8];
        match effect {
            ItemEffect::Magnet => self.fx[FX_MAGNET] = secs,
            ItemEffect::Boost => self.fx[FX_BOOST] = secs,
            ItemEffect::Superjump => self.fx[FX_JUMP] = secs,
            ItemEffect::Double => self.fx[FX_DOUBLE] = secs,
            ItemEffect::Meow => {
                if self.lives < self.spec.lives.count {
                    self.lives += 1;
                    self.toast = "MEOW  +1 LIFE".into();
                } else {
                    self.core.stats.score += 100;
                    self.toast = "MEOW  +100".into();
                }
                self.toast_t = 1.4;
                if self.robot_t > 0.0 && self.robot_flee <= 0.0 {
                    self.robot_flee = 1.0;
                    self.core.stats.score += self.spec.chaser.scare_score;
                    self.toast = format!("{}  SHOO!", self.toast.trim_end());
                }
                self.burst(self.x, self.y + 0.5, self.z, 16, color, 3.4, 0.12);
                self.sfx("meow");
                return;
            }
        }
        self.toast = effect_label(effect).into();
        self.toast_t = 1.4;
        self.burst(at[0], at[1], at[2], 14, color, 3.0, 0.11);
        self.sfx("coin");
    }

    /// Glowing pick-ups ahead of the cat: a bobbing orb in the effect's colour
    /// inside a slow ring.
    fn push_items(&self, doc: &mut WorldDoc) {
        let track = &self.spec.runner.obstacle_track;
        let i0 = near_index(self.z, track.z0, track.gap).saturating_sub(1);
        let reach = self.z + 6.0 + self.obstacle_lead_count() as f32 * track.gap;
        for i in i0..i0 + 2 + self.obstacle_lead_count() {
            if i < 0 || self.got_items.contains(&i) {
                continue;
            }
            let Some((effect, p)) = item_at(&self.spec, i) else {
                continue;
            };
            if p[2] < self.z - 1.0 || p[2] > reach {
                continue;
            }
            let c = self.item_color(effect);
            let col = [c[0] as u32, c[1] as u32, c[2] as u32];
            let lin = [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0];
            let icon = self
                .spec
                .items
                .iter()
                .find(|it| it.effect == effect)
                .map(|it| it.shape())
                .unwrap_or_else(|| effect.default_icon());
            let bob = (self.time * 3.0 + i as f32).sin() * 0.07;
            // A slow sway rather than a full turn, so the picture is never edge-on.
            let sway = (self.time * 1.7 + i as f32 * 0.9).sin() * 0.75;
            let centre = [p[0], p[1] + bob, p[2]];
            for (n, part) in icon_props(icon, centre, sway).iter().enumerate() {
                let mut q = glow(
                    &format!("item-{i}-{n}"),
                    part.model,
                    "item",
                    part.pos,
                    part.scale,
                    if part.pale {
                        [(col[0] + 255) / 2, (col[1] + 255) / 2, (col[2] + 255) / 2]
                    } else {
                        col
                    },
                    2.4,
                );
                q.emissive_color = Some(if part.pale {
                    [
                        (lin[0] + 1.0) / 2.0,
                        (lin[1] + 1.0) / 2.0,
                        (lin[2] + 1.0) / 2.0,
                    ]
                } else {
                    lin
                });
                q.yaw = part.yaw;
                q.roll = part.roll;
                doc.props.push(q);
            }
            let mut ring = glow(
                &format!("item-ring-{i}"),
                "torus",
                "item_ring",
                [p[0], p[1] + bob, p[2]],
                [1.05, 1.05, 1.05],
                col,
                1.2,
            );
            ring.emissive_color = Some(lin);
            ring.yaw = self.time * 1.6 + i as f32;
            ring.pitch = 1.2;
            doc.props.push(ring);
        }
    }

    /// A ring on the ground under the cat for each effect that is running.
    fn push_auras(&self, doc: &mut WorldDoc) {
        let effects = [
            (FX_MAGNET, ItemEffect::Magnet, 1.5),
            (FX_BOOST, ItemEffect::Boost, 1.15),
            (FX_JUMP, ItemEffect::Superjump, 1.3),
            (FX_DOUBLE, ItemEffect::Double, 1.0),
        ];
        for (slot, effect, size) in effects {
            if self.fx[slot] <= 0.0 {
                continue;
            }
            // Blink for the last stretch so the end is not a surprise.
            if self.fx[slot] < self.spec.item_rules.warn_secs
                && (self.fx[slot] * 8.0).floor() as i64 % 2 == 0
            {
                continue;
            }
            let c = self.item_color(effect);
            let lin = [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0];
            let mut ring = glow(
                &format!("aura-{slot}"),
                "torus",
                "aura",
                [self.x, self.ground + 0.03, self.z],
                [size, size * 0.4, size],
                [c[0] as u32, c[1] as u32, c[2] as u32],
                1.6,
            );
            ring.emissive_color = Some(lin);
            ring.yaw = self.time * 2.0;
            doc.props.push(ring);
            if slot == FX_MAGNET {
                // Rings sweeping in from the edge of the field toward the cat.
                let rad = self.spec.item_rules.magnet_radius;
                for n in 0..2 {
                    let phase = (self.time * 0.9 + n as f32 * 0.5).fract();
                    let r = rad * (1.0 - 0.75 * phase);
                    let mut f = glow(
                        &format!("field-{n}"),
                        "torus",
                        "magnet_field",
                        [self.x, self.ground + 0.06 + 0.5 * phase, self.z],
                        [r * 2.0, r * 0.3, r * 2.0],
                        [c[0] as u32, c[1] as u32, c[2] as u32],
                        0.2 + 1.6 * phase,
                    );
                    f.emissive_color = Some(lin);
                    doc.props.push(f);
                }
            }
        }
    }

    fn push_coins(&self, doc: &mut WorldDoc) {
        let c = &self.spec.runner.coins;
        if !c.enabled || !glb_ready(&c.glb) {
            return;
        }
        let track = &self.spec.runner.obstacle_track;
        let i0 = near_index(self.z, track.z0, track.gap).saturating_sub(1);
        // Same cap as obstacles: coins past the fog never reach the GPU.
        let reach = self.z + 6.0 + self.obstacle_lead_count() as f32 * track.gap;
        for i in i0..i0 + 2 + self.obstacle_lead_count() {
            if i < 0 {
                continue;
            }
            for (k, p) in coin_line(&self.spec, i).into_iter().enumerate() {
                if self.got_coins.contains(&(i * 32 + k as i32))
                    || p[2] < self.z - 1.0
                    || p[2] > reach
                {
                    continue;
                }
                // Mesh is 0.4 square, base at y 0: centre it on the point.
                if let Some(mut coin) = glb_prop(
                    &format!("coin-{i}-{k}"),
                    "coin",
                    &c.glb,
                    [p[0], p[1] - 0.2, p[2]],
                    [1.0, 1.0, 1.0],
                    self.time * 3.2 + k as f32 * 0.5,
                    0.0,
                    [255, 205, 70],
                    0.0,
                    [1.0, 0.72, 0.18],
                ) {
                    coin.emissive = Some(0.55);
                    coin.metallic = 0.6;
                    coin.roughness = 0.35;
                    doc.props.push(coin);
                }
            }
        }
        // Coins on their way to the cat: faster spin and a short streak behind.
        let mid = self.y + self.body_height() * 0.5;
        let mut ids: Vec<_> = self.pulls.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let (p, v) = self.pulls[&id];
            let d = [self.x - p[0], mid - p[1], self.z - p[2]];
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-3);
            let u = [d[0] / len, d[1] / len, d[2] / len];
            if let Some(mut coin) = glb_prop(
                &format!("pull-{id}"),
                "coin",
                &c.glb,
                [p[0], p[1] - 0.2, p[2]],
                [1.0, 1.0, 1.0],
                self.time * 12.0 + id as f32,
                0.0,
                [255, 205, 70],
                0.0,
                [1.0, 0.72, 0.18],
            ) {
                coin.emissive = Some(1.2);
                doc.props.push(coin);
            }
            // The faster it goes the longer the streak.
            let tail = (v * 0.025).clamp(0.15, 0.7);
            for n in 1..=3 {
                let f = n as f32 / 3.0;
                let mut s = glow(
                    &format!("pull-{id}-t{n}"),
                    "box",
                    "coin_trail",
                    [
                        p[0] - u[0] * tail * f,
                        p[1] - u[1] * tail * f,
                        p[2] - u[2] * tail * f,
                    ],
                    [0.1 * (1.0 - f * 0.6); 3],
                    [255, 214, 90],
                    2.2 * (1.0 - f * 0.7),
                );
                s.emissive_color = Some([1.0, 0.8, 0.3]);
                doc.props.push(s);
            }
        }
    }

    /// A hit that does not end the run: one life, a blink of protection.
    fn hurt(&mut self) {
        self.lives = self.lives.saturating_sub(1);
        self.invuln = self.spec.lives.invuln_secs;
        self.hurt_t = 0.0;
        self.call_robot();
        self.core.juice.hit(self.feel.hit_shake * 0.6, 0.07);
        self.burst(self.x, self.y + 0.4, self.z, 12, [255, 60, 170], 3.6, 0.14);
        self.burst(self.x, self.y + 0.5, self.z, 6, [90, 240, 255], 2.6, 0.1);
        self.sfx("fire");
    }

    fn crash(&mut self, reason: &str) {
        if self.core.stats.alive {
            if self.invuln > 0.0 || self.fx[FX_BOOST] > 0.0 {
                return;
            }
            // A second hit with the robot on your tail: it has caught you.
            if self.robot_t > 0.0 && self.robot_flee <= 0.0 {
                self.lives = 1;
            }
            if self.lives > 1 {
                self.hurt();
                return;
            }
            self.lives = 0;
        }
        if !self.core.stats.die(reason) {
            return;
        }
        // The crash plays out (shake, flash, sparks) for `beat_secs` before the
        // result card; the freeze is the longer of that and the feel's own.
        let beat = self.feel.hit_stop_secs.max(self.spec.hud.result.beat_secs);
        self.core.juice.hit(self.feel.hit_shake, beat);
        self.dying = 0.0;
        self.vy = 0.0;
        self.drag_target = None;
        self.lane_target = None;
        self.hop = None;
        self.burst(self.x, self.y + 0.4, self.z, 18, [255, 60, 170], 4.6, 0.16);
        self.burst(self.x, self.y + 0.5, self.z, 10, [90, 240, 255], 3.2, 0.12);
        self.sfx("fire");
    }

    #[allow(clippy::too_many_arguments)]
    fn burst(&mut self, x: f32, y: f32, z: f32, n: u32, color: [u32; 3], speed: f32, size: f32) {
        let n = (n as f32 * self.feel.particles).round() as u32;
        self.core.juice.burst(x, y, z, n, color, speed, size);
    }

    fn sfx(&self, name: &str) {
        if self.feel.sfx {
            crate::mixer::cue_sfx(name);
        }
    }

    fn rebuild(&mut self) {
        let fog = self.fog_range();
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
            fog_start: Some(fog.0),
            fog_end: Some(fog.1),
            line_fade: Some([fog.0, fog.1]),
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
        // Rim: one light each side, outside the cat's flanks (a light inside
        // the mesh lights nothing). +x is screen-left on this camera.
        let rim = &self.spec.player.rim;
        let rim_y = self.y + rim.height;
        doc.lights.push(light(
            2,
            "rim",
            [self.x + rim.side, rim_y, self.z],
            [0.0, 0.0, 0.0],
            rim.left,
            rim.intensity,
            rim.radius,
            false,
        ));
        doc.lights.push(light(
            3,
            "edge",
            [self.x - rim.side, rim_y, self.z],
            [0.0, 0.0, 0.0],
            rim.right,
            rim.intensity,
            rim.radius,
            false,
        ));
        push_mesh_anchors(&self.spec, &mut doc);
        self.push_tunnel(&mut doc);
        self.push_gates(&mut doc);
        self.push_rings(&mut doc);
        self.push_coins(&mut doc);
        self.push_items(&mut doc);
        self.push_mischief(&mut doc);
        self.push_perches(&mut doc);
        self.push_robot(&mut doc);
        self.spawned = self.push_obstacles(&mut doc);
        if self.debug {
            self.push_hitboxes(&mut doc);
        }
        self.push_cat(&mut doc);
        self.push_auras(&mut doc);
        self.push_sparks(&mut doc);
        self.doc = doc;
    }

    /// Ledges ahead: a dark body with a lit top so the landing reads, in the
    /// colour the spec gives to things you can ride.
    fn push_perches(&self, doc: &mut WorldDoc) {
        if self.spec.perch_route.is_empty() {
            return;
        }
        let track = &self.spec.runner.obstacle_track;
        let i0 = near_index(self.z, track.z0, track.gap).saturating_sub(1);
        let reach = self.z + 6.0 + self.obstacle_lead_count() as f32 * track.gap;
        let c = self.spec.style.codes.rideable;
        let col = [c[0] as u32, c[1] as u32, c[2] as u32];
        let lin = [
            c[0] as f32 / 255.0,
            c[1] as f32 / 255.0,
            c[2] as f32 / 255.0,
        ];
        for i in i0..i0 + 2 + self.obstacle_lead_count() {
            let Some(steps) = perch_route_at(&self.spec, i) else {
                continue;
            };
            for (n, b) in steps.iter().enumerate() {
                if b.z1 < self.z - 6.0 || b.z0 > reach {
                    continue;
                }
                let id = format!("perch-{i}-{n}");
                let len = b.z1 - b.z0;
                let w = b.half_width * 2.0;
                let zc = (b.z0 + b.z1) * 0.5;
                let mut add = |part: &str,
                               model: &str,
                               pos: [f32; 3],
                               scale: [f32; 3],
                               color: [u32; 3],
                               em: f32,
                               pitch: f32| {
                    let mut q = glow(
                        &format!("{id}-{part}"),
                        model,
                        "perch",
                        pos,
                        scale,
                        color,
                        em,
                    );
                    q.pitch = pitch;
                    if em > 0.0 {
                        q.emissive_color = Some(lin);
                    }
                    doc.props.push(q);
                };
                match b.kind {
                    PerchKind::AcUnit => {
                        add(
                            "body",
                            "box",
                            [b.x, b.top * 0.5, zc],
                            [w * 0.92, b.top, len * 0.94],
                            [58, 66, 86],
                            0.0,
                            0.0,
                        );
                        for (k, f) in [-0.25f32, 0.25].into_iter().enumerate() {
                            add(
                                &format!("fan{k}"),
                                "cylinder",
                                [b.x, b.top + 0.02, zc + len * f],
                                [w * 0.6, 0.05, w * 0.6],
                                [30, 34, 46],
                                0.0,
                                0.0,
                            );
                        }
                    }
                    PerchKind::Pipe => {
                        let r = (w * 0.5).min(0.5);
                        add(
                            "body",
                            "cylinder",
                            [b.x, b.top - r, zc],
                            [r * 2.0, len * 0.98, r * 2.0],
                            [96, 104, 118],
                            0.0,
                            std::f32::consts::FRAC_PI_2,
                        );
                        for (k, f) in [-0.4f32, 0.4].into_iter().enumerate() {
                            add(
                                &format!("leg{k}"),
                                "box",
                                [b.x, (b.top - r * 2.0) * 0.5, zc + len * f],
                                [0.16, b.top - r * 2.0, 0.16],
                                [50, 56, 70],
                                0.0,
                                0.0,
                            );
                        }
                    }
                    PerchKind::Sign => {
                        add(
                            "board",
                            "box",
                            [b.x, b.top - 0.1, zc],
                            [w * 0.92, 0.2, len * 0.94],
                            [44, 40, 66],
                            0.0,
                            0.0,
                        );
                        for (k, f) in [-0.4f32, 0.4].into_iter().enumerate() {
                            add(
                                &format!("post{k}"),
                                "box",
                                [b.x, (b.top - 0.2) * 0.5, zc + len * f],
                                [0.14, b.top - 0.2, 0.14],
                                [50, 56, 70],
                                0.0,
                                0.0,
                            );
                        }
                    }
                }
                // The lit top: a thin plate where the paws land, and its two edges.
                add(
                    "top",
                    "box",
                    [b.x, b.top + 0.012, zc],
                    [w * 0.96, 0.025, len * 0.96],
                    col,
                    0.9,
                    0.0,
                );
                for (k, sx) in [-1.0f32, 1.0].into_iter().enumerate() {
                    add(
                        &format!("edge{k}"),
                        "box",
                        [b.x + sx * (b.half_width - 0.02), b.top + 0.03, zc],
                        [0.05, 0.05, len * 0.96],
                        col,
                        2.4,
                        0.0,
                    );
                }
            }
        }
    }

    /// The cleaning robot behind the cat: a squat disc with a beacon, a lit
    /// bumper ring and two spinning brushes. After a meow it speeds off to the side.
    fn push_robot(&self, doc: &mut WorldDoc) {
        if self.robot_t <= 0.0 || !self.core.stats.alive && self.robot_age == 0.0 {
            return;
        }
        let c = self.spec.chaser.color;
        let col = [c[0] as u32, c[1] as u32, c[2] as u32];
        let lin = [
            c[0] as f32 / 255.0,
            c[1] as f32 / 255.0,
            c[2] as f32 / 255.0,
        ];
        let fl = self.robot_flee;
        let away = if fl > 0.0 { 1.0 - fl } else { 0.0 };
        let side = if self.robot_x >= 0.0 { 1.0 } else { -1.0 };
        let x = self.robot_x + side * away * away * 5.0;
        let z = self.z - self.robot_gap();
        let bob = (self.robot_age * 9.0).sin() * 0.012;
        let yaw = away * away * 6.0;
        let (sin, cos) = yaw.sin_cos();
        // Parts are laid out around the centre and turned with the whole robot.
        let mut add = |part: &str,
                       model: &str,
                       off: [f32; 3],
                       scale: [f32; 3],
                       color: [u32; 3],
                       em: f32,
                       spin: f32| {
            let mut q = glow(
                &format!("robot-{part}"),
                model,
                "robot",
                [
                    x + off[0] * cos + off[2] * sin,
                    off[1] + bob,
                    z - off[0] * sin + off[2] * cos,
                ],
                scale,
                color,
                em,
            );
            q.yaw = yaw + spin;
            if em > 0.0 {
                q.emissive_color = Some(lin);
            }
            doc.props.push(q);
        };
        add(
            "base",
            "cylinder",
            [0.0, 0.18, 0.0],
            [1.15, 0.26, 1.15],
            [38, 44, 58],
            0.0,
            0.0,
        );
        add(
            "dome",
            "sphere",
            [0.0, 0.36, 0.0],
            [0.62, 0.34, 0.62],
            [78, 88, 108],
            0.0,
            0.0,
        );
        add(
            "ring",
            "torus",
            [0.0, 0.2, 0.0],
            [1.24, 1.24, 1.24],
            col,
            1.6,
            0.0,
        );
        add(
            "tail",
            "box",
            [0.0, 0.3, -0.58],
            [0.5, 0.07, 0.05],
            col,
            2.6,
            0.0,
        );
        let lit = (self.robot_age * 5.0).floor() as i64 % 2 == 1;
        add(
            "beacon",
            "sphere",
            [0.0, 0.62, 0.0],
            [0.17, 0.17, 0.17],
            col,
            if lit { 3.2 } else { 0.4 },
            0.0,
        );
        for (k, sx) in [-1.0f32, 1.0].into_iter().enumerate() {
            add(
                &format!("brush{k}"),
                "box",
                [sx * 0.62, 0.07, 0.42],
                [0.5, 0.03, 0.07],
                [205, 208, 225],
                0.0,
                self.robot_age * 16.0 * sx,
            );
        }
        // A pulse on the street so the danger reads from a distance.
        let pulse = (self.robot_age * 4.0).fract();
        let r = 1.5 + 0.9 * pulse;
        let mut ring = glow(
            "robot-pulse",
            "torus",
            "robot",
            [x, 0.04, z],
            [r * 2.0, 1.0, r * 2.0],
            col,
            0.9 * (1.0 - pulse),
        );
        ring.emissive_color = Some(lin);
        doc.props.push(ring);
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
            push_arch_frame(doc, i, z - 0.42, self.spec.style.codes.pass_through);
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
            if i < 0 || !self.spec.obstacle_active(i) {
                continue;
            }
            let parts = obstacle_parts(&self.spec, i);
            if parts.iter().all(|p| p.skin_max[2] < self.z - 2.0) {
                continue;
            }
            if parts.iter().any(|p| !obstacle_drawable(&p.path)) {
                continue;
            }
            // One colour per thing to do (the spec's colour code), so the
            // shape reads before the player has worked it out.
            let rgb = self
                .spec
                .obstacle_kind(i)
                .edge_color(&self.spec.style.codes);
            let col = [rgb[0], rgb[1], rgb[2], 255];
            let fill = [
                rgb[0] as f32 / 255.0 * 0.8,
                rgb[1] as f32 / 255.0 * 0.8,
                rgb[2] as f32 / 255.0 * 0.8,
            ];
            for (k, shape) in parts.iter().enumerate() {
                let Some(mut obstacle) = glb_prop(
                    &part_id("obs", i, k),
                    "obstacle",
                    &shape.path,
                    shape.origin,
                    shape.scale,
                    0.0,
                    0.0,
                    self.spec.style.obstacle_color,
                    0.0,
                    [0.04, 0.02, 0.06],
                ) else {
                    continue;
                };
                // Dim neon fill in the edge hue so a crate reads against the
                // black tunnel. Low enough that bloom (threshold 1.4) skips it.
                obstacle.emissive = Some(self.spec.style.fill_emissive);
                obstacle.emissive_color = Some(fill);
                if shape.glow > 0.0 {
                    // A lit opening: bright enough to bloom against the dark.
                    obstacle.emissive = Some(shape.glow);
                    obstacle.emissive_color = Some([
                        rgb[0] as f32 / 255.0,
                        rgb[1] as f32 / 255.0,
                        rgb[2] as f32 / 255.0,
                    ]);
                }
                obstacle.roughness = 0.94;
                obstacle.metallic = 0.04;
                doc.props.push(obstacle);
                // Ramps and decor are not dangerous: no neon edge.
                if shape.kills() {
                    let proud = 0.045;
                    push_neon_box(
                        doc,
                        &part_id("edge", i, k),
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
                }
            }
            spawned.insert(i);
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
            if i < 0 || !self.spec.obstacle_active(i) {
                continue;
            }
            for (k, shape) in obstacle_parts(&self.spec, i).iter().enumerate() {
                if shape.skin_max[2] < self.z - 2.0 || !shape.kills() {
                    continue;
                }
                // The kill box is inset inside the mesh, so a cage on that box
                // disappears into the crate. Draw the skin a little proud, in
                // cyan, so the phone overlay shows which prop can kill.
                let proud = 0.1;
                push_aabb_cage(
                    doc,
                    &part_id("hit", i, k),
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
    }

    fn push_cat(&mut self, doc: &mut WorldDoc) {
        // The blink: hidden on the off half of each cycle while protected.
        let hz = self.spec.lives.blink_hz;
        let shown =
            self.invuln <= 0.0 || hz <= 0.0 || (self.invuln * hz * 2.0).floor() as i64 % 2 == 0;
        let k = if shown { 1.0 } else { 0.001 };
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
            on_ground: self.y <= self.ground + 0.02,
            model: String::new(),
            gltf: None,
            clip: self.anim_t,
            anim: "run".into(),
            anim_blend: 1.0,
            scale: Some([
                self.spec.player.scale * self.pose[0] * k,
                self.spec.player.scale * self.pose[1] * k,
                self.spec.player.scale * self.pose[2] * k,
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
        if shown {
            self.push_eyes(doc);
        }
    }

    /// The cat's glow marks (eyes, ear glints, collar, tail tip). Offsets are model units, so they follow
    /// the scale and the crouch / stretch pose. Feet are at `self.y`.
    fn push_eyes(&self, doc: &mut WorldDoc) {
        let e = &self.spec.player.eyes;
        let sc = self.spec.player.scale;
        let (sin, cos) = self.yaw.sin_cos();
        // Model-space offset (sideways, up, forward) to the world.
        let at = |x: f32, y: f32, z: f32| {
            let lx = x * sc * self.pose[0];
            let lz = z * sc * self.pose[2];
            [
                self.x + lx * cos + lz * sin,
                self.y + y * sc * self.pose[1],
                self.z - lx * sin + lz * cos,
            ]
        };
        let mut put = |mut p: WorldProp, color: [f32; 3]| {
            p.emissive_color = Some(color);
            p.yaw = self.yaw;
            doc.props.push(p);
        };
        for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            if e.enabled {
                let eye = glow(
                    &format!("cat-eye-{i}"),
                    "box",
                    "cat_eye",
                    at(side * e.spread, e.height, e.forward),
                    [
                        e.size * sc * self.pose[0],
                        e.size * 1.3 * sc * self.pose[1],
                        e.size * 0.4 * sc * self.pose[2],
                    ],
                    [255, 255, 255],
                    e.glow,
                );
                put(eye, e.color);
            }
            if e.ears {
                // A glint on each ear tip, which shows over the back.
                let k = e.size * 0.8 * sc;
                let ear = glow(
                    &format!("cat-ear-{i}"),
                    "box",
                    "cat_ear",
                    at(side * e.ear_x, e.ear_y, e.ear_z),
                    [k * self.pose[0], k * self.pose[1], k * self.pose[2]],
                    [255, 255, 255],
                    e.glow,
                );
                put(ear, e.color);
            }
        }
        if e.collar {
            // Four thin strips just outside the body block (half 0.625).
            let (h, t) = (0.64f32, 0.05f32);
            let strips = [
                ("back", 0.0, -h, h * 2.0, t),
                ("left", -h, 0.0, t, h * 2.0),
                ("right", h, 0.0, t, h * 2.0),
            ];
            for (name, x, z, w, d) in strips {
                let strip = glow(
                    &format!("cat-collar-{name}"),
                    "box",
                    "cat_collar",
                    at(x, e.collar_y, z),
                    [
                        w * sc * self.pose[0],
                        0.07 * sc * self.pose[1],
                        d * sc * self.pose[2],
                    ],
                    [255, 255, 255],
                    e.glow * 0.8,
                );
                put(strip, e.collar_color);
            }
        }
        if e.tail_tip {
            let k = 0.16 * sc;
            let tip = glow(
                "cat-tail-tip",
                "box",
                "cat_tail",
                at(e.tail_x, e.tail_y, e.tail_z),
                [k * self.pose[0], k * 1.4 * self.pose[1], k * self.pose[2]],
                [255, 255, 255],
                e.glow,
            );
            put(tip, e.tail_color);
        }
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
        let phase = self.flow.phase;
        // The in-canvas result column has its own panels; only the host's card sits over the HUD.
        if phase == FlowPhase::Play
            || self.core.juice.hitstop > 0.0
            || (phase == FlowPhase::Result && !menu)
        {
            self.paint_play_hud(&mut list, width, height);
        }
        list
    }
}

impl NeonCat {
    /// Score and coin panels, plus the crash flash. After a crash the panels
    /// shake once, dim, and stay dimmed under the result card.
    fn paint_play_hud(&self, list: &mut DrawList, width: u32, height: u32) {
        use crate::hud_kit::{counter_chip, fade, nine_slice, paw_icon, ui_scale};
        let hud = &self.spec.hud;
        let ui = ui_scale(width, height, hud.scale);
        let (w, h) = (width as f32, height as f32);
        let beat = hud.result.beat_secs.max(0.05);
        let dead = !self.core.stats.alive;
        // 1 while playing, easing to 0.35 across the beat, then held.
        let keep = if dead {
            1.0 - 0.65 * (self.dying / beat).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let shake = if dead {
            (self.dying * 70.0).sin() * 7.0 * ui * (1.0 - self.dying / 0.3).max(0.0)
        } else {
            0.0
        };
        let flash = if dead {
            (1.0 - self.dying / 0.4).max(0.0)
        } else {
            0.0
        };
        if flash > 0.0 {
            list.quads.push(Quad::new(
                0.0,
                0.0,
                w,
                h,
                [255, 40, 110, (flash * 120.0) as u8],
            ));
            // Red edges, heavier than the middle: the hit lands at the rim of the view.
            let e = (14.0 * ui).max(8.0);
            let edge = [255, 20, 70, (flash * 150.0) as u8];
            list.quads.push(Quad::new(0.0, 0.0, w, e, edge));
            list.quads.push(Quad::new(0.0, h - e, w, e, edge));
            list.quads.push(Quad::new(0.0, e, e, h - 2.0 * e, edge));
            list.quads.push(Quad::new(w - e, e, e, h - 2.0 * e, edge));
        }
        // A hit that did not end the run: thin red edges, gone in a third of a second.
        let hurt = if dead {
            0.0
        } else {
            (1.0 - self.hurt_t / 0.35).max(0.0)
        };
        if hurt > 0.0 {
            let e = (10.0 * ui).max(6.0);
            let edge = [255, 20, 70, (hurt * 120.0) as u8];
            list.quads.push(Quad::new(0.0, 0.0, w, e, edge));
            list.quads.push(Quad::new(0.0, h - e, w, e, edge));
            list.quads.push(Quad::new(0.0, e, e, h - 2.0 * e, edge));
            list.quads.push(Quad::new(w - e, e, e, h - 2.0 * e, edge));
        }
        let top = self.spec.hud_top * ui * 0.5 + shake;
        let m = 14.0 * ui;
        let p = &hud.panel;
        let (fill, border) = (fade(p.fill, keep), fade(p.border, keep));
        // Score: small caption over a big number.
        let (sw, sh) = (160.0 * ui, 56.0 * ui);
        nine_slice(
            list,
            m + shake,
            top,
            sw,
            sh,
            p.slice * ui,
            fill,
            border,
            (2.0 * ui, 3.0 * ui, 90),
        );
        list.texts.push(
            TextQuad::new(
                "SCORE",
                m + 12.0 * ui + shake,
                top + 6.0 * ui,
                11.0 * ui,
                fade([150, 200, 215, 255], keep),
            )
            .aligned(TextAlign::Left),
        );
        list.texts.push(
            TextQuad::new(
                &format!("{}", self.core.stats.score),
                m + 12.0 * ui + shake,
                top + 20.0 * ui,
                30.0 * ui,
                fade(p.score, keep),
            )
            .aligned(TextAlign::Left),
        );
        // Lives: a paw each, under the score. The one just lost pops red.
        let max = self.spec.lives.count;
        if max > 1 {
            let r = 6.5 * ui;
            let step = 17.0 * ui;
            let (x0, y0) = (m + 12.0 * ui + r + shake, top + sh + 6.0 * ui + r);
            let pop = (1.0 - self.hurt_t / 0.6).clamp(0.0, 1.0);
            for k in 0..max {
                let cx = x0 + k as f32 * step;
                if k < self.lives {
                    paw_icon(list, cx, y0, r, fade([255, 150, 205, 255], keep));
                } else if k == self.lives && pop > 0.0 && !dead {
                    let c = [
                        255,
                        (60.0 + 90.0 * (1.0 - pop)) as u8,
                        90,
                        (pop * 255.0) as u8,
                    ];
                    paw_icon(list, cx, y0, r * (1.0 + 0.9 * pop), fade(c, keep));
                } else {
                    paw_icon(list, cx, y0, r, fade([70, 60, 100, 130], keep));
                }
            }
        }
        // Running pick-ups: a label and a draining bar each, under the paws.
        let mut row_y = top + sh + 6.0 * ui + if max > 1 { 20.0 * ui } else { 0.0 };
        let fx_rows = [
            (FX_MAGNET, ItemEffect::Magnet),
            (FX_BOOST, ItemEffect::Boost),
            (FX_JUMP, ItemEffect::Superjump),
            (FX_DOUBLE, ItemEffect::Double),
        ];
        for (slot, effect) in fx_rows {
            let left = self.fx[slot];
            if left <= 0.0 {
                continue;
            }
            let full = self
                .spec
                .items
                .iter()
                .find(|i| i.effect == effect)
                .map(|i| i.secs())
                .unwrap_or_else(|| effect.default_secs())
                .max(0.1);
            let c = self.item_color(effect);
            let warn = left < self.spec.item_rules.warn_secs;
            let on = !warn || (left * 6.0).floor() as i64 % 2 == 1;
            let col = [
                c[0] as u8,
                c[1] as u8,
                c[2] as u8,
                if on { 255 } else { 90 },
            ];
            let x0 = m + 12.0 * ui + shake;
            list.texts.push(
                TextQuad::new(effect_label(effect), x0, row_y, 11.0 * ui, fade(col, keep))
                    .aligned(TextAlign::Left),
            );
            let bw = 96.0 * ui;
            list.quads.push(Quad::new(
                x0,
                row_y + 13.0 * ui,
                bw,
                4.0 * ui,
                fade([20, 14, 40, 180], keep),
            ));
            list.quads.push(Quad::new(
                x0,
                row_y + 13.0 * ui,
                bw * (left / full).clamp(0.0, 1.0),
                4.0 * ui,
                fade(col, keep),
            ));
            row_y += 22.0 * ui;
        }
        // The cleaning robot's clock, in its own colour.
        if self.robot_t > 0.0 && !dead {
            let full = self.spec.chaser.secs.max(0.1);
            let c = self.spec.chaser.color;
            let warn = self.robot_t < self.spec.item_rules.warn_secs && self.robot_flee <= 0.0;
            let on = !warn || (self.robot_t * 6.0).floor() as i64 % 2 == 1;
            let col = [c[0], c[1], c[2], if on { 255 } else { 90 }];
            let x0 = m + 12.0 * ui + shake;
            let label = if self.robot_flee > 0.0 {
                "SHOO!"
            } else {
                "ROBOT"
            };
            list.texts.push(
                TextQuad::new(label, x0, row_y, 11.0 * ui, fade(col, keep))
                    .aligned(TextAlign::Left),
            );
            let bw = 96.0 * ui;
            list.quads.push(Quad::new(
                x0,
                row_y + 13.0 * ui,
                bw,
                4.0 * ui,
                fade([20, 14, 40, 180], keep),
            ));
            list.quads.push(Quad::new(
                x0,
                row_y + 13.0 * ui,
                bw * (self.robot_t / full).clamp(0.0, 1.0),
                4.0 * ui,
                fade(col, keep),
            ));
        }
        // A pick-up's name over the view for a moment.
        if self.toast_t > 0.0 && !dead {
            let a = (self.toast_t / 0.4).clamp(0.0, 1.0);
            let rise = (1.4 - self.toast_t).min(0.4) * 30.0 * ui;
            list.texts.push(
                TextQuad::new(
                    &self.toast,
                    w * 0.5,
                    h * 0.30 - rise,
                    26.0 * ui,
                    [
                        self.toast_c[0],
                        self.toast_c[1],
                        self.toast_c[2],
                        (a * 255.0) as u8,
                    ],
                )
                .aligned(TextAlign::Center),
            );
        }
        if self.spec.runner.coins.enabled {
            let (cw, ch) = (112.0 * ui, 40.0 * ui);
            if self.pop_t > 0.0 && !dead {
                let a = (self.pop_t / 0.3).clamp(0.0, 1.0);
                list.texts.push(
                    TextQuad::new(
                        &format!("+{}", self.pop_sum),
                        w - m - cw * 0.5 + shake,
                        top + ch + 6.0 * ui + (0.6 - self.pop_t) * 18.0 * ui,
                        15.0 * ui,
                        [255, 224, 110, (a * 255.0) as u8],
                    )
                    .aligned(TextAlign::Center),
                );
            }
            counter_chip(
                list,
                w - m - cw + shake,
                top,
                cw,
                ch,
                ui,
                fill,
                border,
                fade(p.coin, keep),
                &format!("{}", self.coins),
            );
        }
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
        // Sized by the larger of forward and sideways travel.
        let lateral = self
            .vx
            .abs()
            .max(self.spec.runner.lanes.steer * 3.0 * self.drag_target.is_some() as u8 as f32);
        ((self.speed().max(lateral) * dt) / self.spec.runner.speed.max_step)
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
        self.dying += dt;
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

/// The cat's body half-width. Feet were a point, so a side touch let the
/// body sink into a crate before it counted.
const CAT_HALF_W: f32 = 0.1;

/// Local AABB after the glTF node bake (`mesh.bounds()`), times the prop
/// scale. Y stays on the floor. XZ is centered on `(obs_x, obs_z)`.
/// The kill box is inset so it sits inside that mesh.
fn obstacle_shape(s: &GameSpec, index: i32) -> ObstacleShape {
    obstacle_parts(s, index).swap_remove(0)
}

/// Where obstacle `index` is anchored. Lane-crossing kinds (a beam, a wall
/// with one gap) stay centred so the wall always reaches the lane limit and
/// cannot be walked around.
fn obstacle_anchor(s: &GameSpec, index: i32) -> (f32, f32) {
    let t = &s.runner.obstacle_track;
    let x = match s.obstacle_kind(index).behavior {
        Behavior::RollUnder | Behavior::CatGap => 0.0,
        _ => t.x(index),
    };
    (x, t.z(index))
}

/// Every mesh of obstacle `index`: the base mesh first (if any), then parts.
fn obstacle_parts(s: &GameSpec, index: i32) -> Vec<Hitbox> {
    let kind = s.obstacle_kind(index);
    let (x, z) = obstacle_anchor(s, index);
    let mut out = Vec::with_capacity(1 + kind.parts.len());
    if !kind.glb.is_empty() {
        out.push(Hitbox::place(s, kind, x, z));
    }
    for part in &kind.parts {
        out.push(Hitbox::place_part(s, kind, part, x, z));
    }
    out
}

/// A spark inside a drawn obstacle's mesh box would glow through it.
fn spark_inside_obstacle(s: &GameSpec, spawned: &HashSet<i32>, x: f32, y: f32, z: f32) -> bool {
    let i0 =
        near_index(z, s.runner.obstacle_track.z0, s.runner.obstacle_track.gap).saturating_sub(1);
    (i0..i0 + 3).any(|i| {
        if !spawned.contains(&i) {
            return false;
        }
        obstacle_parts(s, i)
            .iter()
            .any(|p| p.contains_padded(x, y, z, 0.08))
    })
}

/// Coin positions `[x, y, z]` that belong to slot `index`. The shape follows
/// what the slot asks: an arc over a hop, a low line under a beam, a line
/// along a train top, and a weave through rest and reward stretches.
/// The pick-up of obstacle slot `index`, if any: only free slots of rest and
/// reward sections carry one, every n-th by the spec, halfway to the next slot
/// so it never sits in the coin line. Which one is a weighted hash of the slot.
/// Where a slot sits in its section: the section kind, how many free slots
/// came before it in the same section, and a well-mixed hash of the slot.
fn free_slot(s: &GameSpec, index: i32) -> Option<(SectionKind, u32, u32)> {
    if index < 0 || s.obstacle_active(index) {
        return None;
    }
    let t = &s.runner.obstacle_track;
    let z = t.z(index);
    let (_, sec) = s.section_at(z)?;
    let mut h = (index as u32).wrapping_add(0x9e37_79b9);
    h = (h ^ (h >> 16)).wrapping_mul(0x85eb_ca6b);
    h = (h ^ (h >> 13)).wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    let mut i = index;
    while i > 0
        && !s.obstacle_active(i - 1)
        && s.section_at(t.z(i - 1)).map(|(k, _)| k) == s.section_at(z).map(|(k, _)| k)
    {
        i -= 1;
    }
    Some((sec.kind, (index - i) as u32, h))
}

/// One ledge of a perch route in world terms.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PerchBox {
    pub kind: PerchKind,
    pub x: f32,
    pub half_width: f32,
    pub z0: f32,
    pub z1: f32,
    pub top: f32,
}

/// The steps of the perch route standing in a free slot (None when the slot
/// has no route): end to end along the track, off to one side by hash.
pub(crate) fn perch_route_at(s: &GameSpec, index: i32) -> Option<Vec<PerchBox>> {
    if s.perch_route.is_empty() {
        return None;
    }
    let (kind, ordinal, h) = free_slot(s, index)?;
    let r = &s.perch_rules;
    let every = match kind {
        SectionKind::Rest => r.rest_every,
        SectionKind::Reward => r.reward_every,
        _ => r.other_every,
    };
    if every == 0 || ordinal % every != 0 {
        return None;
    }
    let t = &s.runner.obstacle_track;
    let side = if (h >> 1) & 1 == 0 { -1.0 } else { 1.0 };
    let reach = (s.runner.lanes.limit - r.half_width + 0.1).max(0.0);
    let x = (t.x(index) + side * r.side).clamp(-reach, reach);
    let len = s.perch_route.len() as f32 * r.step_len;
    let start = t.z(index) - len * 0.5;
    Some(
        s.perch_route
            .iter()
            .enumerate()
            .map(|(n, step)| PerchBox {
                kind: step.kind,
                x,
                half_width: r.half_width,
                z0: start + n as f32 * r.step_len,
                z1: start + (n + 1) as f32 * r.step_len,
                top: step.top(),
            })
            .collect(),
    )
}

/// A knockable prop standing in a free slot, off to one side of the track.
pub(crate) fn mischief_at(s: &GameSpec, index: i32) -> Option<(MischiefKind, [f32; 3])> {
    if s.mischief.is_empty() {
        return None;
    }
    let (kind, ordinal, h) = free_slot(s, index)?;
    // A route has the slot's side: the prop makes way.
    if perch_route_at(s, index).is_some() {
        return None;
    }
    let r = &s.mischief_rules;
    let every = match kind {
        SectionKind::Rest => r.rest_every,
        SectionKind::Reward => r.reward_every,
        _ => r.other_every,
    };
    if every == 0 || ordinal % every != every - 1 {
        return None;
    }
    let total: u32 = s.mischief.iter().map(|m| m.weight).sum();
    if total == 0 {
        return None;
    }
    let mut pick = (h >> 4) % total;
    let mut which = s.mischief[0].kind;
    for m in &s.mischief {
        if pick < m.weight {
            which = m.kind;
            break;
        }
        pick -= m.weight;
    }
    let t = &s.runner.obstacle_track;
    let side = if h & 1 == 0 { -1.0 } else { 1.0 };
    let lim = s.runner.lanes.limit - 0.15;
    Some((
        which,
        [
            (t.x(index) + side * r.side).clamp(-lim, lim),
            0.0,
            t.z(index),
        ],
    ))
}

struct MischiefPart {
    model: &'static str,
    /// Offset along the prop's own up axis.
    dy: f32,
    scale: [f32; 3],
    /// Drawn in a paler shade of the prop's colour.
    light: bool,
}

/// The shapes of a prop (all from primitives) and its height.
fn mischief_shape(kind: MischiefKind) -> (Vec<MischiefPart>, f32) {
    let part = |model, dy, scale, light| MischiefPart {
        model,
        dy,
        scale,
        light,
    };
    match kind {
        MischiefKind::Vase => (
            vec![
                part("sphere", 0.30, [0.46, 0.56, 0.46], false),
                part("cylinder", 0.64, [0.22, 0.28, 0.22], true),
            ],
            0.78,
        ),
        MischiefKind::Bottle => (
            vec![
                part("cylinder", 0.24, [0.22, 0.46, 0.22], false),
                part("cylinder", 0.56, [0.1, 0.22, 0.1], true),
            ],
            0.68,
        ),
        MischiefKind::Can => (vec![part("cylinder", 0.2, [0.3, 0.4, 0.3], false)], 0.4),
        MischiefKind::Yarn => (
            vec![
                part("sphere", 0.21, [0.42, 0.42, 0.42], false),
                part("torus", 0.21, [0.5, 0.5, 0.5], true),
            ],
            0.42,
        ),
    }
}

fn mischief_color(kind: MischiefKind) -> [f32; 3] {
    match kind {
        MischiefKind::Vase => [90.0, 210.0, 225.0],
        MischiefKind::Bottle => [120.0, 235.0, 150.0],
        MischiefKind::Can => [255.0, 170.0, 90.0],
        MischiefKind::Yarn => [255.0, 110.0, 190.0],
    }
}

fn mischief_label(kind: MischiefKind) -> &'static str {
    match kind {
        MischiefKind::Vase => "CRASH",
        MischiefKind::Bottle => "CLINK",
        MischiefKind::Can => "CLANG",
        MischiefKind::Yarn => "BOING",
    }
}

pub(crate) fn item_at(s: &GameSpec, index: i32) -> Option<(ItemEffect, [f32; 3])> {
    if s.items.is_empty() {
        return None;
    }
    let (kind, ordinal, h) = free_slot(s, index)?;
    let every = match kind {
        SectionKind::Rest => s.item_rules.rest_every,
        SectionKind::Reward => s.item_rules.reward_every,
        _ => return None,
    };
    if every == 0 || ordinal % every != every - 1 {
        return None;
    }
    let t = &s.runner.obstacle_track;
    let z = t.z(index);
    let total: u32 = s.items.iter().map(|i| i.weight).sum();
    if total == 0 {
        return None;
    }
    let mut pick = h % total;
    let mut effect = s.items[0].effect;
    for it in &s.items {
        if pick < it.weight {
            effect = it.effect;
            break;
        }
        pick -= it.weight;
    }
    Some((effect, [t.x(index), s.item_rules.hover, z + t.gap * 0.5]))
}

struct IconPart {
    model: &'static str,
    /// Centre in the icon's plane: sideways, then up.
    at: [f32; 2],
    scale: [f32; 3],
    /// Turn in the picture's plane, radians (positive lifts the right end,
    /// right as the player sees it from the chase camera).
    roll: f32,
    /// A paler shade of the item colour.
    pale: bool,
}

/// One drawn piece of an icon, in world terms (what goes into the prop).
struct IconProp {
    model: &'static str,
    pos: [f32; 3],
    scale: [f32; 3],
    yaw: f32,
    roll: f32,
    pale: bool,
}

/// An icon's pieces placed around `centre`, the picture swayed by `sway`
/// about the vertical axis. The chase camera looks along +Z, so screen-right
/// is world -X: the picture's right is mirrored into that, and a roll that
/// lifts the picture's right end is a negative prop roll (which lifts +X).
fn icon_props(icon: ItemIcon, centre: [f32; 3], sway: f32) -> Vec<IconProp> {
    let (sin, cos) = sway.sin_cos();
    icon_parts(icon)
        .into_iter()
        .map(|part| {
            // Rotating (-x, y, 0) by `sway` about Y: +Z turns toward +X.
            let x = -part.at[0];
            IconProp {
                model: part.model,
                pos: [
                    centre[0] + x * cos,
                    centre[1] + part.at[1],
                    centre[2] - x * sin,
                ],
                scale: part.scale,
                yaw: sway,
                roll: -part.roll,
                pale: part.pale,
            }
        })
        .collect()
}

/// An orb's picture as a few thick primitives, about 0.7 m across.
fn icon_parts(icon: ItemIcon) -> Vec<IconPart> {
    const Q: f32 = std::f32::consts::FRAC_PI_4;
    let bar = |x: f32, y: f32, w: f32, h: f32, roll: f32, pale: bool| IconPart {
        model: "box",
        at: [x, y],
        scale: [w, h, 0.1],
        roll,
        pale,
    };
    let ball = |x: f32, y: f32| IconPart {
        model: "sphere",
        at: [x, y],
        scale: [0.32, 0.32, 0.2],
        roll: 0.0,
        pale: false,
    };
    match icon {
        ItemIcon::Horseshoe => vec![
            bar(-0.28, -0.02, 0.14, 0.5, 0.0, false),
            bar(0.28, -0.02, 0.14, 0.5, 0.0, false),
            bar(0.0, -0.28, 0.7, 0.15, 0.0, false),
            bar(-0.28, 0.27, 0.16, 0.13, 0.0, true),
            bar(0.28, 0.27, 0.16, 0.13, 0.0, true),
        ],
        ItemIcon::Chevron => {
            let mut v = Vec::new();
            for cx in [-0.2f32, 0.15] {
                v.push(bar(cx, 0.13, 0.4, 0.12, -Q, false));
                v.push(bar(cx, -0.13, 0.4, 0.12, Q, false));
            }
            v
        }
        ItemIcon::Arrow => vec![
            bar(0.0, -0.12, 0.15, 0.46, 0.0, false),
            bar(-0.14, 0.2, 0.4, 0.12, Q, false),
            bar(0.14, 0.2, 0.4, 0.12, -Q, false),
        ],
        ItemIcon::TwoX => vec![
            bar(-0.24, 0.27, 0.26, 0.08, 0.0, false),
            bar(-0.13, 0.14, 0.08, 0.22, 0.0, false),
            bar(-0.24, 0.0, 0.26, 0.08, 0.0, false),
            bar(-0.35, -0.14, 0.08, 0.22, 0.0, false),
            bar(-0.24, -0.27, 0.26, 0.08, 0.0, false),
            bar(0.24, 0.0, 0.5, 0.09, Q, true),
            bar(0.24, 0.0, 0.5, 0.09, -Q, true),
        ],
        ItemIcon::Heart => vec![
            ball(-0.14, 0.1),
            ball(0.14, 0.1),
            IconPart {
                model: "cone",
                at: [0.0, -0.1],
                scale: [0.44, 0.4, 0.2],
                roll: std::f32::consts::PI,
                pale: false,
            },
        ],
    }
}

fn effect_label(effect: ItemEffect) -> &'static str {
    match effect {
        ItemEffect::Magnet => "MAGNET",
        ItemEffect::Boost => "BOOST",
        ItemEffect::Superjump => "HIGH JUMP",
        ItemEffect::Double => "2X SCORE",
        ItemEffect::Meow => "MEOW",
    }
}

/// Ids are `index * 32 + k`, so a line never exceeds 32 coins.
pub(crate) fn coin_line(s: &GameSpec, index: i32) -> Vec<[f32; 3]> {
    let c = &s.runner.coins;
    if !c.enabled || c.per_line == 0 || index < 0 {
        return Vec::new();
    }
    let t = &s.runner.obstacle_track;
    let lim = s.runner.lanes.limit - 0.2;
    let z = t.z(index);
    let sp = c.spacing;
    let strip = |n: u32, x: &dyn Fn(f32) -> f32, y: &dyn Fn(f32) -> f32| -> Vec<[f32; 3]> {
        let n = n.min(32);
        (0..n)
            .map(|k| {
                let u = k as f32 - (n as f32 - 1.0) * 0.5;
                let f = if n > 1 {
                    k as f32 / (n as f32 - 1.0)
                } else {
                    0.5
                };
                [x(u).clamp(-lim, lim), y(f), z + u * sp]
            })
            .collect()
    };
    if !s.obstacle_active(index) {
        let kind = s.section_at(z).map(|(_, sec)| sec.kind);
        let calm = matches!(kind, Some(SectionKind::Rest | SectionKind::Reward));
        let n = if calm { c.free_line } else { c.per_line };
        let weave = match kind {
            Some(SectionKind::Reward) => 0.9,
            Some(SectionKind::Rest) => 0.4,
            _ => 0.0,
        };
        let cx = t.x(index);
        let mut line = strip(n, &|u| cx + weave * (u * 0.9).sin(), &|_| 0.3);
        // Coins along the top of each ledge of a route in this slot.
        if let Some(steps) = perch_route_at(s, index) {
            let per = s.perch_rules.coins_per_step;
            for b in &steps {
                for k in 0..per {
                    let f = (k as f32 + 0.5) / per as f32;
                    line.push([b.x, b.top + 0.6, b.z0 + (b.z1 - b.z0) * f]);
                }
            }
            line.truncate(32);
        }
        return line;
    }
    let kind = s.obstacle_kind(index);
    let (ax, _) = obstacle_anchor(s, index);
    let n = c.per_line;
    match kind.behavior {
        Behavior::Solid => Vec::new(),
        Behavior::JumpOver => strip(n, &|_| ax, &|f| {
            0.3 + c.arc_height * (f * std::f32::consts::PI).sin()
        }),
        Behavior::RollUnder => strip(n, &|_| ax, &|_| 0.2),
        // Through the opening, below the lintel.
        Behavior::CatGap => strip(n, &|_| ax, &|_| 0.2),
        Behavior::Rideable => {
            let top = obstacle_parts(s, index)
                .iter()
                .filter(|p| p.role == PartRole::Ride)
                .map(|p| p.skin_max[1])
                .fold(0.0, f32::max);
            let len = obstacle_parts(s, index)
                .iter()
                .find(|p| p.role == PartRole::Ride)
                .map(|p| (p.skin_max[2] - p.skin_min[2]).max(sp))
                .unwrap_or(sp);
            let m = ((len / sp) as u32).clamp(2, n + 2);
            strip(m, &|_| ax, &|_| top + 0.3)
        }
    }
}

/// `obs-3` for the main mesh of slot 3, `obs-3-1` for its extra parts.
fn part_id(prefix: &str, i: i32, k: usize) -> String {
    if k == 0 {
        format!("{prefix}-{i}")
    } else {
        format!("{prefix}-{i}-{k}")
    }
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
/// Coloured as the spec's pass-through code: a gate is never a threat.
fn push_arch_frame(doc: &mut WorldDoc, index: i32, z: f32, rgb: [u8; 3]) {
    stroke(
        doc,
        &format!("arch-out-{index}"),
        arch_polyline(z, 2.62, 1.48, 1.28, 10),
        [rgb[0], rgb[1], rgb[2], 255],
        2.4,
    );
    let soft = |c: u8| (c as f32 * 0.7) as u8;
    stroke(
        doc,
        &format!("arch-in-{index}"),
        arch_polyline(z + 0.02, 2.45, 1.36, 1.02, 10),
        [soft(rgb[0]), soft(rgb[1]), soft(rgb[2]), 240],
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
    /// The three-crate course the older tests were written against.
    const CLASSIC: &str = include_str!("../tests/fixtures/neon_cat_classic_world.json");

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
        // Scores re-recorded when coins arrived (10 each); frames, rings and
        // deaths are unchanged, so the physics did not move.
        // Run 2 re-recorded when the cat got a body half-width (CAT_HALF_W):
        // it used to slide past a barrier edge with its body inside it.
        type Golden = (u32, u32, u32, &'static str);
        let cases: [(f32, u32, Golden); 4] = [
            (0.013, 47, (4582, 13, 4642, "BARREL")),
            (0.031, 23, (1240, 5, 1240, "BARRIER")),
            (0.0, 1000, (176, 1, 144, "CRATE")),
            (0.007, 11, (717, 2, 1010, "BARREL")),
        ];
        // The Python-baked classic course (three crates, no sections) is the
        // spec these numbers were recorded on, and must play the same.
        let baked = include_str!("../tests/fixtures/neon_cat_classic_world.json");
        let loaded = NeonCat::from_json(baked).unwrap();
        assert_eq!(loaded.spec(), &GameSpec::legacy());
        // The Python-baked default is the full Subway course.
        let full = include_str!("../tests/fixtures/neon_cat_world.json");
        assert_eq!(
            NeonCat::from_json(full).unwrap().spec(),
            &GameSpec::default()
        );
        for doc in [baked] {
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
            let mut run = NeonCat::from_json(doc).unwrap().one_life();
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
        GameSpec::legacy()
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
    const OBS_FILL_EMISSIVE: f32 = 0.8;
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
        let mut run = NeonCat::from_json(CLASSIC).unwrap();
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
        let mut run = NeonCat::from_json(CLASSIC).unwrap();
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

        let mut crash = NeonCat::from_json(CLASSIC).unwrap();
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
        for _ in 0..60 {
            crash.tick(1.0 / 60.0);
        }
        assert_eq!(crash.phase_name(), "result");
    }

    fn pev(id: u32, phase: crate::input::PointerPhase, x: f32) -> crate::input::PointerEvent {
        crate::input::PointerEvent {
            id,
            x,
            y: 1600.0,
            phase,
            pressure: 0.5,
        }
    }

    #[test]
    fn drag_keeps_working_over_five_retries_with_lost_pointerups() {
        use crate::input::PointerPhase as P;
        let mut run = NeonCat::from_json(CLASSIC).unwrap();
        run.confirm();
        for attempt in 0..5u32 {
            let id = attempt + 1;
            for _ in 0..30 {
                run.tick(1.0 / 60.0);
            }
            let x0 = run.x();
            // Drag one lane screen-right (world -X) and hold.
            run.touch(pev(id, P::Begin, 540.0), 1080, 2340, attempt as f32 * 100.0);
            for f in 1..=12 {
                let x = 540.0 + 194.4 * (f as f32 / 12.0);
                run.touch(
                    pev(id, P::Move, x),
                    1080,
                    2340,
                    attempt as f32 * 100.0 + f as f32 / 60.0,
                );
                run.tick(1.0 / 60.0);
            }
            for _ in 0..20 {
                run.tick(1.0 / 60.0);
            }
            assert!(
                x0 - run.x() > 0.9,
                "run {attempt}: a 1-lane drag moved {:.2} m",
                x0 - run.x()
            );
            // The pointerup is lost (result overlay swallowed it): die, retry.
            for _ in 0..60 * 30 {
                run.tick(1.0 / 60.0);
                if run.phase_name() == "result" {
                    break;
                }
            }
            assert_eq!(run.phase_name(), "result", "run {attempt} should end");
            run.confirm();
            for _ in 0..5 {
                run.tick(1.0 / 60.0);
                if run.phase_name() != "play" {
                    run.confirm();
                }
            }
            assert!(!run.touch_active(), "retry clears stale fingers");
        }
    }

    #[test]
    fn sideways_drag_into_a_crate_beside_the_cat_kills() {
        // Every frame offset: the cat drags from the next lane across the
        // crate while their z ranges overlap. Swept x/z must catch it.
        let box0 = obstacle_shape(0);
        let mut kills = 0;
        for lead in 0..24 {
            for top_speed in [false, true] {
                let mut run = NeonCat::from_json(CLASSIC).unwrap();
                run.confirm();
                run.time = 10.0;
                if top_speed {
                    run.time = 1e4;
                }
                let w = run.spec.runner.lanes.width;
                run.x = box0.hit_cx + w;
                run.z = box0.hit_cz - box0.hit_hz - run.speed() * lead as f32 / 120.0;
                run.drag_target = Some(box0.hit_cx - w);
                let mut path_hits = false;
                for _ in 0..40 {
                    let (x0, z0) = (run.x, run.z);
                    run.tick(1.0 / 60.0);
                    if !run.core.stats.alive {
                        break;
                    }
                    path_hits |= box0.sweep_hit(x0, z0, run.x, run.z, 0.0, CAT_HALF_W);
                }
                assert!(
                    !path_hits,
                    "lead {lead}: path crossed the box but the cat lived"
                );
                if !run.core.stats.alive {
                    assert_eq!(run.death(), "CRATE");
                    kills += 1;
                }
            }
        }
        assert!(kills > 4, "side entries should kill, got {kills}");
    }

    #[test]
    fn swept_hitbox_catches_side_entry_and_tunnelling() {
        let b = obstacle_shape(0);
        let (cx, cz) = (b.hit_cx, b.hit_cz);
        // Sideways only, z already overlapping.
        assert!(b.sweep_hit(cx + 1.2, cz, cx - 1.2, cz, 0.0, 0.0));
        // Diagonal across the box in one big step.
        assert!(b.sweep_hit(cx + 1.0, cz - 1.0, cx - 1.0, cz + 1.0, 0.0, 0.0));
        // Beside, never inside.
        assert!(!b.sweep_hit(cx + 1.2, cz - 1.0, cx + 1.2, cz + 1.0, 0.0, 0.0));
        // Over the top.
        assert!(!b.sweep_hit(cx + 1.2, cz, cx - 1.2, cz, b.hit_top + 0.1, 0.0));
    }

    #[test]
    fn running_through_the_first_ring_scores() {
        let mut run = NeonCat::from_json(CLASSIC).unwrap();
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
        let run = NeonCat::from_json(CLASSIC).unwrap();
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
            .load_game_json(r#"{"version":1,"genre":"neon_cat","game":{"lives":{"count":1}}}"#)
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
        let mut run = NeonCat::from_json(CLASSIC).unwrap();
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
        for _ in 0..60 {
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
        let mut spawn = NeonCat::from_json(CLASSIC).unwrap();
        spawn.confirm();
        spawn.x = obs_x(0);
        spawn.z = obs_z(0);
        spawn.time = 0.2;
        spawn.tick(1.0 / 60.0);
        assert!(
            spawn.core.stats.alive,
            "grace skips the obstacle under the spawn stretch"
        );

        let mut rail = NeonCat::from_json(CLASSIC).unwrap();
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

        let mut arch = NeonCat::from_json(CLASSIC).unwrap();
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
        let mut run = NeonCat::from_json(CLASSIC).unwrap();
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
            let mut world: serde_json::Value = serde_json::from_str(CLASSIC).unwrap();
            world["quality"] = quality.into();
            let json = world.to_string();
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
        let mut run = NeonCat::from_json(CLASSIC).unwrap();
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
        let mut run = NeonCat::from_json(CLASSIC).unwrap();
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

    // ---- Subway mechanics (PR B1) ----

    fn full() -> NeonCat {
        full_lives().one_life()
    }

    /// The shipped course with all its lives.
    fn full_lives() -> NeonCat {
        let mut run =
            NeonCat::from_json(include_str!("../tests/fixtures/neon_cat_world.json")).unwrap();
        run.confirm();
        run.time = GRACE_SECS + 0.2;
        run
    }

    /// First slot whose kind is `name`.
    fn slot_of(spec: &GameSpec, name: &str) -> i32 {
        (0..4000)
            .find(|&i| spec.obstacle_active(i) && spec.obstacle_kind(i).name == name)
            .unwrap_or_else(|| panic!("no {name} in the course"))
    }

    fn start_before(run: &mut NeonCat, i: i32, x: f32) {
        let front = obstacle_parts(&run.spec, i)
            .iter()
            .map(|p| p.skin_min[2])
            .fold(f32::INFINITY, f32::min);
        run.x = x;
        run.y = 0.0;
        run.vy = 0.0;
        run.z = front - 3.0;
        run.rebuild();
    }

    #[test]
    fn roll_lowers_the_body_and_ends_on_time() {
        let mut run = full();
        run.swipe(Swipe::Down);
        run.tick(1.0 / 60.0);
        assert!(run.rolling > 0.0);
        assert_eq!(run.body_height(), run.spec.runner.body.roll_height);
        for _ in 0..40 {
            run.tick(1.0 / 60.0);
        }
        assert_eq!(
            run.rolling, 0.0,
            "{} s roll is over",
            run.spec.runner.body.roll_secs
        );
        // Standing back up takes `recover_secs`, then the body is full height.
        let mid = run.body_height();
        assert!(mid > run.spec.runner.body.roll_height && mid < run.spec.runner.body.stand_height);
        for _ in 0..20 {
            run.tick(1.0 / 60.0);
        }
        assert_eq!(run.body_height(), run.spec.runner.body.stand_height);
        assert!(
            run.pose[1] > 0.97,
            "pose is back to standing: {:?}",
            run.pose
        );
    }

    #[test]
    fn fog_ends_before_the_first_obstacle_that_is_not_drawn_yet() {
        let mut run = full();
        for t in [0.0, 30.0, 200.0] {
            run.time = t;
            run.z = 0.0;
            run.rebuild();
            let (start, end) = run.fog_range();
            let first_undrawn =
                run.obstacle_lead_count() as f32 * run.spec.runner.obstacle_track.gap;
            assert!(end < first_undrawn, "fog {end} vs {first_undrawn}");
            assert!(start < end);
            assert_eq!(run.doc.line_fade, Some([start, end]));
        }
    }

    #[test]
    fn the_overpass_is_a_sign_on_posts_and_the_gap_is_lit() {
        let run = full();
        let spec = run.spec();
        let over = spec
            .obstacles
            .iter()
            .find(|k| k.name == "OVERPASS")
            .unwrap();
        // One solid panel with 0.5 m under it, two posts that never kill.
        assert_eq!(over.parts.len(), 3);
        assert!(over.parts[0].offset[1] >= spec.runner.body.stand_height * 0.8);
        assert!(over.parts[1..].iter().all(|p| p.role == PartRole::Decor));
        let wall = spec.obstacles.iter().find(|k| k.name == "WALL").unwrap();
        assert!(wall.parts.iter().any(|p| p.glow > 0.0));
        // The eye stays above the panel's top by the camera margin.
        let top = over.parts[0].offset[1] + over.parts[0].scale[1];
        assert!(spec.camera.chase_lift - top >= CAMERA_MARGIN);
    }

    #[test]
    fn held_or_repeated_down_never_stretches_a_roll() {
        let mut run = full();
        let secs = run.spec.runner.body.roll_secs;
        run.swipe(Swipe::Down);
        let mut t = 0.0;
        let mut ended = None;
        // A jittery finger fires a down flick every 0.1 s for a full second.
        for i in 0..90 {
            if i % 6 == 0 && t < 1.0 {
                run.swipe(Swipe::Down);
            }
            run.tick(1.0 / 60.0);
            t += 1.0 / 60.0;
            if ended.is_none() && run.rolling <= 0.0 && t > 0.1 {
                ended = Some(t);
            }
        }
        // A fresh roll may begin after the first one ends, but no single roll
        // runs longer than its spec: the first one ended on time.
        let ended = ended.expect("the first roll ended");
        assert!(ended <= secs + 0.1, "roll lasted {ended} s, spec {secs}");
    }

    #[test]
    fn an_up_flick_right_after_a_down_flick_is_the_finger_coming_back() {
        let mut run = full();
        run.swipe(Swipe::Down);
        run.tick(1.0 / 60.0);
        run.swipe(Swipe::Up);
        for _ in 0..10 {
            run.tick(1.0 / 60.0);
        }
        assert!(run.y <= run.ground + 0.001, "no jump out of the roll");
        assert!(run.rolling > 0.0);
        // Later it is a jump again.
        for _ in 0..40 {
            run.tick(1.0 / 60.0);
        }
        run.swipe(Swipe::Up);
        for _ in 0..8 {
            run.tick(1.0 / 60.0);
        }
        assert!(run.y > 0.2);
    }

    #[test]
    fn fast_fall_rolls_on_touchdown() {
        let mut run = full();
        run.swipe(Swipe::Up);
        for _ in 0..8 {
            run.tick(1.0 / 60.0);
        }
        assert!(run.y > 0.3);
        run.swipe(Swipe::Down);
        assert!(run.roll_on_land);
        let mut rolled = false;
        for _ in 0..60 {
            run.tick(1.0 / 60.0);
            rolled |= run.rolling > 0.0;
        }
        assert!(rolled && run.core.stats.alive);
    }

    #[test]
    fn a_beam_kills_a_standing_cat_and_not_a_rolling_one() {
        let spec = full().spec.clone();
        let i = slot_of(&spec, "OVERPASS");
        let mut stand = full();
        start_before(&mut stand, i, 0.0);
        for _ in 0..120 {
            stand.tick(1.0 / 60.0);
        }
        assert_eq!(stand.death(), "OVERPASS");
        let mut roll = full();
        start_before(&mut roll, i, 0.0);
        for f in 0..120 {
            if f == 8 {
                roll.swipe(Swipe::Down);
            }
            roll.tick(1.0 / 60.0);
        }
        assert!(roll.core.stats.alive, "{}", roll.death());
        assert!(roll.z > obstacle_parts(&spec, i)[0].skin_max[2]);
    }

    #[test]
    fn cat_gap_needs_a_roll_and_cannot_be_jumped_or_walked() {
        let spec = full().spec.clone();
        let i = slot_of(&spec, "WALL");
        let mut jump = full();
        start_before(&mut jump, i, 0.0);
        for f in 0..120 {
            jump.set_steer(0.0, 0.0, (20..30).contains(&f));
            jump.tick(1.0 / 60.0);
        }
        assert_eq!(jump.death(), "WALL");
        let mut side = full();
        start_before(&mut side, i, 1.5);
        for _ in 0..120 {
            side.tick(1.0 / 60.0);
        }
        assert_eq!(side.death(), "WALL", "the wall reaches the rails");
        let mut roll = full();
        start_before(&mut roll, i, 0.0);
        for f in 0..120 {
            if f == 8 {
                roll.swipe(Swipe::Down);
            }
            roll.tick(1.0 / 60.0);
        }
        assert!(roll.core.stats.alive, "{}", roll.death());
    }

    #[test]
    fn a_ramp_lifts_the_cat_onto_a_train_and_it_drops_off_the_end() {
        let spec = full().spec.clone();
        let i = slot_of(&spec, "TRAIN");
        let parts = obstacle_parts(&spec, i);
        let top = parts[0].skin_max[1];
        let mut run = full();
        let (ax, _) = obstacle_anchor(&spec, i);
        start_before(&mut run, i, ax);
        let mut peak = 0.0f32;
        for _ in 0..200 {
            if run.z > parts[0].skin_max[2] + 6.0 {
                break;
            }
            run.tick(1.0 / 60.0);
            peak = peak.max(run.y);
            if run.z > parts[0].skin_min[2] + 1.0 && run.z < parts[0].skin_max[2] - 0.5 {
                assert!((run.y - top).abs() < 0.02, "riding at {} not {top}", run.y);
            }
        }
        assert!(run.core.stats.alive, "{}", run.death());
        assert!((peak - top).abs() < 0.05, "peak {peak}");
        assert!(run.y < 0.01 && run.ground == 0.0, "back on the street");
    }

    #[test]
    fn a_train_without_its_ramp_is_a_wall() {
        let spec = full().spec.clone();
        let i = slot_of(&spec, "TRAIN");
        let parts = obstacle_parts(&spec, i);
        let mut run = full();
        // Alongside the car on the street, then a hard turn into its flank.
        run.x = parts[0].hit_cx + parts[0].hit_hx + 0.4;
        run.y = 0.0;
        run.z = parts[0].skin_min[2] + 1.0;
        run.rebuild();
        run.set_steer(1.0, 0.0, false);
        for _ in 0..30 {
            run.tick(1.0 / 60.0);
        }
        assert_eq!(run.death(), "TRAIN", "the flank is a wall below the step");
    }

    #[test]
    fn coins_score_without_counting_as_rings() {
        let spec = full().spec.clone();
        let line = coin_line(&spec, 0);
        assert_eq!(line, coin_line(&spec, 0), "deterministic");
        assert!(line.len() >= 3);
        let mut run = full();
        run.x = line[0][0];
        run.y = (line[0][1] - 0.3).max(0.0);
        run.z = line[0][2] - 0.2;
        run.rebuild();
        let before = run.core.stats.score;
        run.tick(1.0 / 60.0);
        run.tick(1.0 / 60.0);
        assert!(run.coins >= 1);
        assert_eq!(run.core.stats.rings, 0, "a coin is not a ring");
        assert!(run.core.stats.score >= before + spec.runner.coins.score);
        let got = run.coins;
        run.z = line[0][2] - 0.2;
        run.tick(1.0 / 60.0);
        assert_eq!(run.coins, got, "each coin once");
    }

    #[test]
    fn retry_starts_every_counter_from_zero() {
        let mut run = full_lives();
        assert_eq!(run.lives, 3);
        // Collect a coin and a ring, run past some gates, then crash.
        let spec = run.spec.clone();
        let line = coin_line(&spec, 0);
        run.x = line[0][0];
        run.y = (line[0][1] - 0.3).max(0.0);
        run.z = line[0][2] - 0.2;
        run.rebuild();
        run.tick(1.0 / 60.0);
        run.tick(1.0 / 60.0);
        assert!(run.coins >= 1);
        run.got.insert(3);
        run.core.stats.rings = 2;
        run.z = spec.runner.gates.z0 + spec.runner.gates.gap * 2.0 + 0.5;
        run.tick(1.0 / 60.0);
        assert!(run.gates >= 3, "gates {}", run.gates);
        assert!(run.core.stats.score > 0);
        // Two hits cost two lives; the run goes on.
        for _ in 0..2 {
            run.robot_t = 0.0; // the robot has its own tests
            run.invuln = 0.0;
            run.crash("CRATE");
            assert!(run.core.stats.alive);
        }
        assert_eq!(run.lives, 1);
        assert!(run.invuln > 0.0);
        run.invuln = 0.0;
        run.lives = 1;
        run.crash("CRATE");
        assert!(!run.core.stats.alive);
        assert_eq!(run.lives, 0);
        for _ in 0..90 {
            run.tick(1.0 / 60.0);
        }
        assert_eq!(run.flow.phase, FlowPhase::Result);
        // Retry through the on-screen action, then through the key.
        for via_key in [false, true] {
            if via_key {
                run.got.insert(1);
                run.coins = 4;
                run.gates = 2;
                run.lives = 1;
                run.crash("BARREL");
                for _ in 0..90 {
                    run.tick(1.0 / 60.0);
                }
                assert_eq!(run.flow.phase, FlowPhase::Result);
            }
            if via_key {
                run.on_key("r", true);
            } else {
                run.apply_action(FlowAction::Retry);
            }
            assert_eq!(run.flow.phase, FlowPhase::Play);
            assert_eq!(run.coins, 0, "coins");
            assert_eq!(run.gates, 0, "gates");
            assert_eq!(run.core.stats.score, 0, "score");
            assert_eq!(run.core.stats.rings, 0, "rings");
            assert_eq!(run.time, 0.0, "time");
            assert_eq!(run.z, 0.0, "distance");
            assert!(run.got.is_empty() && run.got_coins.is_empty());
            assert!(run.core.stats.alive && run.core.stats.death.is_empty());
            assert_eq!(run.rolling, 0.0);
            assert_eq!(run.dying, 0.0, "crash timer");
            assert_eq!(run.pose, [1.0; 3]);
            assert_eq!(run.lives, run.spec.lives.count, "every life is back");
            assert_eq!(run.lives, 3);
            assert_eq!(run.invuln, 0.0, "no leftover protection");
            assert!(run.hurt_t > 10.0, "no leftover hurt flash");
            assert_eq!(run.sim_value()["lives"], 3);
            // The collected coin is back in the world and the sim json shows it.
            let v = run.sim_value();
            assert_eq!(v["coins"], 0);
            assert_eq!(v["gates"], 0);
            // Collected pickups of the first run can be collected again.
            run.time = GRACE_SECS + 0.2;
            run.x = line[0][0];
            run.y = (line[0][1] - 0.3).max(0.0);
            run.z = line[0][2] - 0.2;
            run.rebuild();
            run.tick(1.0 / 60.0);
            run.tick(1.0 / 60.0);
            assert!(run.coins >= 1, "the same coin pays again after a retry");
            run.lives = 1;
            run.crash("CRATE");
            for _ in 0..90 {
                run.tick(1.0 / 60.0);
            }
        }
    }

    #[test]
    fn a_hit_costs_a_life_then_protects_then_the_last_one_ends_the_run() {
        let mut run = full_lives();
        let fx = run.spec.lives.clone();
        assert_eq!((fx.count, run.lives), (3, 3));
        run.crash("CRATE");
        assert!(run.core.stats.alive, "a hit is survivable");
        assert_eq!(run.lives, 2);
        assert!((run.invuln - fx.invuln_secs).abs() < 1e-6);
        // Protected: the same obstacle cannot hit again.
        run.crash("CRATE");
        run.crash("BARREL");
        assert_eq!(run.lives, 2, "protected");
        run.robot_t = 0.0; // this one is about lives, not the robot
                           // Protection runs out with play time.
        let mut frames = 0;
        while run.invuln > 0.0 && frames < 600 {
            run.tick(1.0 / 60.0);
            frames += 1;
        }
        assert!(
            (frames as f32 / 60.0 - fx.invuln_secs).abs() < 0.2,
            "{frames} frames"
        );
        assert_eq!(run.invuln, 0.0);
        run.crash("CRATE");
        assert_eq!(run.lives, 1);
        // Down to the last life: the next hit is the end.
        run.invuln = 0.0;
        run.lives = 1;
        run.crash("BARREL");
        assert!(!run.core.stats.alive);
        assert_eq!(run.core.stats.death, "BARREL");
        assert_eq!(run.lives, 0);
    }

    #[test]
    fn one_life_is_the_old_rule() {
        let mut run = full();
        assert_eq!(run.lives, 1);
        run.crash("CRATE");
        assert!(!run.core.stats.alive);
    }

    #[test]
    fn a_real_obstacle_hit_costs_one_life_not_several() {
        let mut run = full_lives();
        let spec = run.spec.clone();
        let i = slot_of(&spec, "CRATE");
        let z = spec.runner.obstacle_track.z(i);
        let (x, _) = (spec.runner.obstacle_track.x(i), 0.0);
        run.x = x;
        run.z = z - 1.2;
        run.rebuild();
        for _ in 0..120 {
            run.tick(1.0 / 60.0);
        }
        assert!(run.core.stats.alive);
        assert_eq!(run.lives, 2, "one box, one life, however long it overlaps");
    }

    #[test]
    fn the_paw_row_shows_each_life_and_the_lost_one_pops() {
        let mut run = full_lives();
        let paws = |l: &DrawList| l.quads.len();
        let nine = hud_of(&run, 824, 1800);
        run.crash("CRATE");
        // Right after the hit: two filled paws, one popping, plus edge flashes.
        let hit = hud_of(&run, 824, 1800);
        assert!(paws(&hit) > 0);
        let bright = |l: &DrawList| {
            l.quads
                .iter()
                .filter(|q| {
                    q.color[0] >= 250 && (140..=160).contains(&q.color[1]) && q.color[2] >= 200
                })
                .count()
        };
        assert!(bright(&nine) > bright(&hit), "a filled paw went out");
        // Long after, the pop is over and the row stays one short.
        run.hurt_t = 5.0;
        let calm = hud_of(&run, 824, 1800);
        assert!(bright(&calm) < bright(&nine));
        // A one-life spec draws no row.
        let one = full();
        let one_row = hud_of(&one, 824, 1800);
        assert!(bright(&one_row) == 0);
    }

    #[test]
    fn the_cat_blinks_while_protected_and_wears_neon_eyes() {
        let mut run = full_lives();
        let eyes = |run: &NeonCat| run.doc.props.iter().filter(|p| p.name == "cat_eye").count();
        assert_eq!(eyes(&run), 2);
        let e = run.doc.props.iter().find(|p| p.name == "cat_eye").unwrap();
        assert!(e.emissive.unwrap() > 1.0, "the eyes glow");
        let sc = |run: &NeonCat| run.doc.walkers[0].scale.unwrap()[0];
        let full_scale = sc(&run);
        // Eyes sit on the cat, in front and above the feet.
        let (a, b) = run
            .doc
            .props
            .iter()
            .filter(|p| p.name == "cat_eye")
            .map(|p| p.position)
            .fold(([0.0; 3], [0.0; 3]), |(_, last), p| (last, p));
        assert!((a[1] - b[1]).abs() < 1e-5, "same height");
        assert!(a[0] != b[0], "apart");
        assert!(a[1] > run.y && a[1] < run.y + 1.0);
        assert!(a[2] > run.z, "on the front of the face");
        run.crash("CRATE");
        let mut shown = 0;
        let mut hidden = 0;
        for _ in 0..40 {
            run.tick(1.0 / 60.0);
            if sc(&run) > full_scale * 0.5 {
                shown += 1;
                assert_eq!(eyes(&run), 2);
            } else {
                hidden += 1;
                assert_eq!(eyes(&run), 0, "no eyes floating where the cat is not");
            }
        }
        assert!(shown > 5 && hidden > 5, "blinks: {shown} on, {hidden} off");
        run.invuln = 0.0;
        run.rebuild();
        assert!(sc(&run) > full_scale * 0.5 && eyes(&run) == 2);
    }

    #[test]
    fn rim_lights_sit_outside_the_cat_and_stay_tight() {
        let run = full_lives();
        let rim = &run.spec.player.rim;
        let lights: Vec<_> = run
            .doc
            .lights
            .iter()
            .filter(|l| l.name == "rim" || l.name == "edge")
            .collect();
        assert_eq!(lights.len(), 2);
        let half_w = 0.625 * run.spec.player.scale; // the model half width
        for l in &lights {
            assert!(
                (l.position[0] - run.x).abs() > half_w,
                "a light inside the body lights nothing"
            );
            assert!(l.radius < 1.6 && l.intensity > 1.0);
        }
        assert!(lights[0].position[0] > run.x && lights[1].position[0] < run.x);
        assert_eq!(lights[0].color, Some(rim.left));
        assert_eq!(lights[1].color, Some(rim.right));
    }

    /// First slot of the shipped course that carries `effect`.
    fn item_slot(spec: &GameSpec, effect: ItemEffect) -> (i32, [f32; 3]) {
        (0..400)
            .find_map(|i| {
                item_at(spec, i)
                    .filter(|(e, _)| *e == effect)
                    .map(|(_, p)| (i, p))
            })
            .unwrap_or_else(|| panic!("no {effect:?} in 400 slots"))
    }

    /// The cat on its way into a pick-up.
    fn at_item(run: &mut NeonCat, i: i32, p: [f32; 3]) {
        run.got_items.remove(&i);
        run.x = p[0];
        run.y = 0.0;
        run.vy = 0.0;
        run.z = p[2] - 0.3;
        run.rebuild();
        run.tick(1.0 / 60.0);
        run.tick(1.0 / 60.0);
    }

    #[test]
    fn items_sit_in_free_rest_and_reward_slots_only_clear_of_the_coin_line() {
        let spec = full().spec.clone();
        let mut kinds = std::collections::BTreeSet::new();
        let mut count = 0;
        for i in 0..400 {
            let z = spec.runner.obstacle_track.z(i);
            let sec = spec.section_at(z).unwrap().1.kind;
            match item_at(&spec, i) {
                None => {}
                Some((e, p)) => {
                    count += 1;
                    kinds.insert(format!("{e:?}"));
                    assert!(!spec.obstacle_active(i), "slot {i} has an obstacle");
                    assert!(
                        matches!(sec, SectionKind::Rest | SectionKind::Reward),
                        "slot {i} in {sec:?}"
                    );
                    let line = coin_line(&spec, i);
                    let last = line.iter().map(|c| c[2]).fold(f32::MIN, f32::max);
                    assert!(p[2] > last + 1.0, "item {} behind coins {}", p[2], last);
                    assert_eq!(item_at(&spec, i).unwrap().0, e, "deterministic");
                }
            }
        }
        assert!(count >= 4, "{count} items on the first 400 slots");
        assert!(kinds.len() >= 4, "{kinds:?}");
        // No items in the old course.
        let legacy = GameSpec::legacy();
        assert!((0..200).all(|i| item_at(&legacy, i).is_none()));
    }

    #[test]
    fn magnet_boost_superjump_and_double_each_do_their_one_thing() {
        let spec = full().spec.clone();
        let secs = |e: ItemEffect| spec.items.iter().find(|i| i.effect == e).unwrap().secs();
        // Magnet: coins beyond the normal reach fly in.
        let mut run = full_lives();
        let (i, p) = item_slot(&spec, ItemEffect::Magnet);
        at_item(&mut run, i, p);
        assert!(
            run.fx[FX_MAGNET] > secs(ItemEffect::Magnet) - 0.2,
            "{:?}",
            run.fx
        );
        assert!(run.got_items.contains(&i));
        let before = run.coins;
        let j = i + 1;
        let line = coin_line(&spec, j);
        assert!(!line.is_empty());
        run.x = line[0][0] + 1.6;
        run.z = line[0][2] - 1.5;
        run.rebuild();
        run.tick(1.0 / 60.0);
        assert_eq!(run.coins, before, "it flies in, it does not snap");
        assert!(!run.pulls.is_empty(), "the coin was taken hold of");
        for _ in 0..60 {
            run.tick(1.0 / 60.0);
        }
        assert!(
            run.coins > before,
            "the magnet pulled a coin from beside the lane"
        );
        assert!(run.pulls.is_empty());
        let plain = {
            let mut r = full_lives();
            r.x = line[0][0] + 1.6;
            r.z = line[0][2] - 1.5;
            r.rebuild();
            r.tick(1.0 / 60.0);
            r.coins
        };
        assert_eq!(plain, 0, "without the magnet it stays");
        // Boost: faster, and a hit does nothing.
        let mut run = full_lives();
        let base = run.speed();
        let (i, p) = item_slot(&spec, ItemEffect::Boost);
        at_item(&mut run, i, p);
        assert!(run.speed() > base * 1.3, "{} vs {}", run.speed(), base);
        let lives = run.lives;
        run.crash("CRATE");
        assert!(
            run.core.stats.alive && run.lives == lives,
            "nothing hurts a boosting cat"
        );
        run.fx[FX_BOOST] = 0.0;
        run.invuln = 0.0;
        run.crash("CRATE");
        assert_eq!(run.lives, lives - 1);
        // Super jump: a much higher arc.
        let apex = |boosted: bool| {
            let mut r = full_lives();
            if boosted {
                r.fx[FX_JUMP] = 5.0;
            }
            r.rebuild();
            r.on_key("w", true);
            let mut top = 0.0f32;
            for _ in 0..180 {
                r.tick(1.0 / 60.0);
                top = top.max(r.y);
                r.on_key("w", false);
            }
            top
        };
        let (low, high) = (apex(false), apex(true));
        assert!(high > low * 1.8, "apex {low} -> {high}");
        // 2x: a coin pays twice.
        let pay = |on: bool| {
            let mut r = full_lives();
            if on {
                r.fx[FX_DOUBLE] = 5.0;
            }
            let line = coin_line(&spec, 0);
            r.x = line[0][0];
            r.y = (line[0][1] - 0.3).max(0.0);
            r.z = line[0][2] - 0.05;
            r.rebuild();
            let s0 = r.core.stats.score;
            r.tick(1.0 / 60.0);
            r.core.stats.score - s0
        };
        let (one, two) = (pay(false), pay(true));
        assert!(
            one >= spec.runner.coins.score && two >= 2 * one - 2,
            "{one} -> {two}"
        );
    }

    #[test]
    fn effects_run_out_and_a_retry_clears_them() {
        let mut run = full_lives();
        run.fx = [1.0, 1.0, 1.0, 1.0];
        let mut frames = 0;
        while run.fx.iter().any(|&t| t > 0.0) && frames < 200 {
            run.tick(1.0 / 60.0);
            frames += 1;
        }
        assert!((55..80).contains(&frames), "{frames} frames");
        let spec = run.spec.clone();
        let (i, p) = item_slot(&spec, ItemEffect::Double);
        at_item(&mut run, i, p);
        assert_eq!(
            run.sim_value()["fx"]["double"].as_f64().map(|v| v > 5.0),
            Some(true)
        );
        assert!(run.sim_value()["toast"].as_str().unwrap().contains("2X"));
        run.lives = 1;
        run.invuln = 0.0;
        run.fx[FX_BOOST] = 0.0;
        run.crash("CRATE");
        for _ in 0..90 {
            run.tick(1.0 / 60.0);
        }
        run.apply_action(FlowAction::Retry);
        assert_eq!(run.fx, [0.0; 4]);
        assert!(run.got_items.is_empty());
        assert_eq!(run.sim_value()["toast"], "");
    }

    /// Slots of the shipped course that carry a prop, with the prop.
    fn prop_slots(spec: &GameSpec, n: i32) -> Vec<(i32, MischiefKind, [f32; 3])> {
        (0..n)
            .filter_map(|i| mischief_at(spec, i).map(|(k, p)| (i, k, p)))
            .collect()
    }

    fn at_prop(run: &mut NeonCat, p: [f32; 3], y: f32) {
        run.x = p[0];
        run.y = y;
        run.vy = 0.0;
        run.z = p[2] - 0.3;
        run.rebuild();
        run.tick(1.0 / 60.0);
        run.tick(1.0 / 60.0);
    }

    #[test]
    fn props_stand_beside_the_track_in_free_slots_and_never_in_the_old_course() {
        let spec = full().spec.clone();
        let list = prop_slots(&spec, 400);
        assert!(list.len() >= 12, "{} props", list.len());
        let kinds: std::collections::BTreeSet<_> = list.iter().map(|(_, k, _)| *k as u32).collect();
        assert_eq!(kinds.len(), 4, "all four kinds show up");
        let lim = spec.runner.lanes.limit;
        for (i, _, p) in &list {
            assert!(!spec.obstacle_active(*i), "slot {i} has an obstacle");
            assert!(p[0].abs() <= lim, "inside the lanes: {}", p[0]);
            let centre = spec.runner.obstacle_track.x(*i);
            assert!((p[0] - centre).abs() > 0.3, "off the centre line");
            assert_eq!(p[2], spec.runner.obstacle_track.z(*i));
            // Never on the same spot as a pick-up.
            assert!(item_at(&spec, *i)
                .map(|(_, q)| (q[2] - p[2]).abs() > 5.0)
                .unwrap_or(true));
        }
        // Rest sections are denser than the rest of the course.
        let in_kind = |k: SectionKind| {
            let all = (0..400).filter(|&i| {
                !spec.obstacle_active(i)
                    && spec
                        .section_at(spec.runner.obstacle_track.z(i))
                        .map(|(_, s)| s.kind)
                        == Some(k)
            });
            let n = all.clone().count().max(1);
            let m = all.filter(|&i| mischief_at(&spec, i).is_some()).count();
            m as f32 / n as f32
        };
        assert!(in_kind(SectionKind::Rest) > in_kind(SectionKind::Challenge));
        assert!(prop_slots(&GameSpec::legacy(), 200).is_empty());
    }

    #[test]
    fn running_into_a_prop_knocks_it_over_once_and_pays() {
        let mut run = full_lives();
        let spec = run.spec.clone();
        let (i, _, p) = prop_slots(&spec, 200)[0];
        let rules = spec.mischief_rules.clone();
        let s0 = run.core.stats.score;
        at_prop(&mut run, p, 0.0);
        assert!(run.knocked.contains_key(&i));
        assert_eq!(run.knocks, 1);
        assert!(run.core.stats.score >= s0 + rules.score);
        assert!(["CRASH", "CLINK", "CLANG", "BOING"]
            .contains(&run.sim_value()["toast"].as_str().unwrap()));
        assert!(
            run.core.stats.alive && run.lives == 3,
            "knocking costs nothing"
        );
        // Each prop once.
        let after = run.core.stats.score;
        run.z = p[2] - 0.3;
        run.tick(1.0 / 60.0);
        assert_eq!(run.knocks, 1);
        assert!(run.core.stats.score < after + rules.score);
        // A jump clears it.
        let mut hop = full_lives();
        at_prop(&mut hop, [p[0], 0.0, p[2]], 2.0);
        assert_eq!(hop.knocks, 0, "over the top");
        // A retry puts them back.
        run.lives = 1;
        run.invuln = 0.0;
        run.fx[FX_BOOST] = 0.0;
        run.crash("CRATE");
        for _ in 0..90 {
            run.tick(1.0 / 60.0);
        }
        run.apply_action(FlowAction::Retry);
        assert!(run.knocked.is_empty() && run.knocks == 0 && run.streak == 0);
        assert_eq!(run.sim_value()["mischief"]["knocked"], 0);
    }

    #[test]
    fn a_quick_second_knock_extends_the_streak_and_a_slow_one_does_not() {
        let spec = full().spec.clone();
        let list = prop_slots(&spec, 300);
        let r = spec.mischief_rules.clone();
        let (a, b) = (list[0].2, list[1].2);
        let mut run = full_lives();
        let s0 = run.core.stats.score;
        at_prop(&mut run, a, 0.0);
        let first = run.core.stats.score - s0;
        assert_eq!(run.streak, 1);
        let s1 = run.core.stats.score;
        at_prop(&mut run, b, 0.0);
        assert_eq!(run.streak, 2);
        assert!(run.sim_value()["toast"].as_str().unwrap().ends_with("x2"));
        assert!(run.core.stats.score - s1 >= r.score + r.streak_bonus);
        assert!(first >= r.score);
        // Let the streak run out, then knock the next one.
        let mut slow = full_lives();
        at_prop(&mut slow, a, 0.0);
        for _ in 0..((r.streak_secs + 0.5) * 60.0) as i32 {
            slow.tick(1.0 / 60.0);
        }
        assert_eq!(slow.sim_value()["mischief"]["streak"], 0);
        at_prop(&mut slow, b, 0.0);
        assert_eq!(slow.streak, 1, "starts over");
    }

    #[test]
    fn a_knocked_prop_flies_aside_and_lies_down() {
        let mut run = full_lives();
        let spec = run.spec.clone();
        let (i, _, p) = prop_slots(&spec, 200)[0];
        let id = format!("prop:mis-{i}-0");
        run.x = p[0];
        run.z = p[2] - 4.0;
        run.rebuild();
        let before = run
            .doc
            .props
            .iter()
            .find(|q| q.id == id)
            .expect("standing")
            .clone();
        assert!((before.position[0] - p[0]).abs() < 1e-4 && before.roll == 0.0);
        at_prop(&mut run, p, 0.0);
        for _ in 0..8 {
            run.tick(1.0 / 60.0);
        }
        let mid = run.doc.props.iter().find(|q| q.id == id);
        // Keep it in view: the cat is right beside it.
        let mid = mid.expect("still in the dump").clone();
        assert!(mid.position[1] > before.position[1] + 0.05, "in the air");
        for _ in 0..90 {
            run.tick(1.0 / 60.0);
            run.x = p[0];
            run.z = p[2] - 1.0;
            run.invuln = 1.0;
        }
        run.rebuild();
        let end = run
            .doc
            .props
            .iter()
            .find(|q| q.id == id)
            .expect("lying")
            .clone();
        let side = end.position[0] - p[0];
        assert!(side.abs() > 1.5, "flew {side} m aside");
        assert!(end.position[1] < before.position[1] + 0.8, "back down");
        assert!(end.roll.abs() > 1.2, "tipped over");
    }

    #[test]
    fn meow_gives_a_life_back_or_score_when_full() {
        let spec = full().spec.clone();
        let (i, p) = item_slot(&spec, ItemEffect::Meow);
        let mut run = full_lives();
        run.lives = 2;
        at_item(&mut run, i, p);
        assert_eq!(run.lives, 3);
        assert!(run.sim_value()["toast"].as_str().unwrap().contains("LIFE"));
        let mut run = full_lives();
        let s0 = run.core.stats.score;
        at_item(&mut run, i, p);
        assert_eq!(run.lives, 3, "never above the row");
        assert!(run.core.stats.score >= s0 + 100);
    }

    #[test]
    fn running_effects_show_in_the_hud_and_the_world() {
        let mut run = full_lives();
        run.rebuild();
        let calm = hud_of(&run, 824, 1800);
        assert!(!calm.texts.iter().any(|t| t.text == "MAGNET"));
        run.fx[FX_MAGNET] = 4.0;
        run.fx[FX_DOUBLE] = 2.0;
        run.toast = "MAGNET".into();
        run.toast_t = 1.0;
        run.rebuild();
        let hud = hud_of(&run, 824, 1800);
        assert!(
            hud.texts.iter().filter(|t| t.text == "MAGNET").count() >= 2,
            "row and toast"
        );
        assert!(hud.texts.iter().any(|t| t.text == "2X SCORE"));
        assert!(run.doc.props.iter().any(|p| p.id == "prop:aura-0"));
        // Items ahead glow in their own colour.
        let spec = run.spec.clone();
        let (i, p) = item_slot(&spec, ItemEffect::Boost);
        run.x = p[0];
        run.z = p[2] - 8.0;
        run.rebuild();
        let orb = run
            .doc
            .props
            .iter()
            .find(|q| q.id == format!("prop:item-{i}-0"))
            .expect("orb in the dump");
        assert!(orb.emissive.unwrap() > 1.0);
    }

    #[test]
    fn the_magnet_pulls_coins_in_with_a_speed_up_a_trail_a_pop_and_a_field() {
        let mut run = full_lives();
        let spec = run.spec.clone();
        let rules = spec.item_rules.clone();
        let line = (0..40)
            .map(|j| coin_line(&spec, j))
            .find(|l| !l.is_empty())
            .expect("a coin line");
        let c = line[0];
        // The cat stands still enough: park it 3 m short of a coin, to the side.
        run.fx[FX_MAGNET] = 5.0;
        run.x = c[0] + 1.5;
        run.z = c[2] - 2.6;
        run.rebuild();
        let start = run.coins;
        let mid = |r: &NeonCat| r.y + r.body_height() * 0.5;
        let d_of = |r: &NeonCat, p: [f32; 3]| {
            let (dx, dy, dz) = (r.x - p[0], mid(r) - p[1], r.z - p[2]);
            (dx * dx + dy * dy + dz * dz).sqrt()
        };
        run.tick(1.0 / 60.0);
        let (id, &(_, v0)) = run.pulls.iter().next().expect("a coin is flying");
        let id = *id;
        assert!(run.pulls.contains_key(&id) && v0 >= rules.pull_speed);
        // A trail and the spinning coin are in the world while it flies.
        assert!(run.doc.props.iter().any(|p| p.name == "coin_trail"));
        assert!(run
            .doc
            .props
            .iter()
            .any(|p| p.id == format!("prop:pull-{id}")));
        // Field rings sized to the radius.
        let rings: Vec<_> = run
            .doc
            .props
            .iter()
            .filter(|p| p.name == "magnet_field")
            .collect();
        assert_eq!(rings.len(), 2);
        assert!(rings
            .iter()
            .all(|r| r.scale[0] <= rules.magnet_radius * 2.0 + 1e-3));
        // Each frame it is faster, until it lands once and pays.
        let mut last_v = v0;
        let mut frames = 0;
        while let Some(&(_, v)) = run.pulls.get(&id) {
            assert!(v >= last_v, "ease-in: the speed only grows");
            last_v = v;
            run.tick(1.0 / 60.0);
            frames += 1;
            assert!(frames < 90, "it never arrived");
        }
        assert!(
            frames >= 4,
            "a visible flight, not a snap ({frames} frames)"
        );
        assert!(run.coins > start);
        assert!(run.pop_t > 0.0 && run.pop_sum >= spec.runner.coins.score);
        let hud = hud_of(&run, 824, 1800);
        assert!(hud
            .texts
            .iter()
            .any(|t| t.text == format!("+{}", run.pop_sum)));
        run.fx[FX_MAGNET] = 0.0; // no new coin comes in while it fades
        for _ in 0..60 {
            run.tick(1.0 / 60.0);
        }
        assert_eq!(run.pop_sum, 0, "the pop fades");
        // Ahead of the cat the field reaches further than behind it.
        let mut r = full_lives();
        r.fx[FX_MAGNET] = 5.0;
        let ahead = [
            r.x,
            r.y + r.body_height() * 0.5,
            r.z + rules.magnet_radius + rules.magnet_lead * 0.5,
        ];
        assert!(
            d_of(&r, ahead) > rules.magnet_radius,
            "outside the plain ball"
        );
        let _ = ahead;
    }

    #[test]
    fn the_last_seconds_of_an_effect_blink() {
        let mut run = full_lives();
        let warn = run.spec.item_rules.warn_secs;
        run.fx[FX_MAGNET] = warn + 1.0;
        run.rebuild();
        let steady = hud_of(&run, 824, 1800);
        let alpha = |h: &DrawList| {
            h.texts
                .iter()
                .find(|t| t.text == "MAGNET")
                .map(|t| t.color[3])
        };
        assert_eq!(alpha(&steady), Some(255));
        let mut seen = std::collections::HashSet::new();
        let mut auras = std::collections::HashSet::new();
        for k in 0..40 {
            run.fx[FX_MAGNET] = warn * (1.0 - k as f32 / 40.0) + 0.01;
            run.rebuild();
            seen.insert(alpha(&hud_of(&run, 824, 1800)));
            auras.insert(run.doc.props.iter().any(|p| p.id == "prop:aura-0"));
        }
        assert!(seen.len() >= 2, "the label blinks: {seen:?}");
        assert!(auras.len() == 2, "the aura blinks too");
    }

    #[test]
    fn items_have_their_own_colour_shape_and_toast() {
        let run = full_lives();
        let spec = run.spec.clone();
        let rgbs: Vec<_> = spec
            .items
            .iter()
            .map(|i| (i.effect, i.rgb(), i.shape()))
            .collect();
        for (a, ca, sa) in &rgbs {
            for (b, cb, sb) in &rgbs {
                if a == b {
                    continue;
                }
                let d: i32 = (0..3).map(|k| (ca[k] as i32 - cb[k] as i32).abs()).sum();
                assert!(d >= 90, "{a:?} and {b:?} look alike");
                assert_ne!(sa, sb, "{a:?} and {b:?} share a shape");
            }
        }
        let mut shapes = rgbs.iter().map(|r| r.2).collect::<Vec<_>>();
        shapes.dedup();
        assert_eq!(shapes.len(), rgbs.len());
        // Each icon is drawn from its own parts.
        for a in ItemIcon::ALL {
            for b in ItemIcon::ALL {
                if a != b {
                    let pa: Vec<_> = icon_parts(a)
                        .iter()
                        .map(|p| (p.model, p.at, p.roll))
                        .collect();
                    let pb: Vec<_> = icon_parts(b)
                        .iter()
                        .map(|p| (p.model, p.at, p.roll))
                        .collect();
                    assert_ne!(pa, pb);
                }
            }
        }
        // The toast, the chip and the aura use the same colour as the item.
        for it in &spec.items {
            let mut r = full_lives();
            r.take_item(it.effect, [r.x, 0.5, r.z]);
            assert_eq!(r.toast_c, it.rgb(), "{:?}", it.effect);
        }
    }

    /// The icon as the player sees it from the chase camera: a grid of ink,
    /// row 0 on top, column 0 on the left, plus a second grid for the pale parts.
    fn icon_picture(icon: ItemIcon, sway: f32) -> (Vec<Vec<bool>>, Vec<Vec<bool>>) {
        use glam::{Quat, Vec3};
        const W: usize = 28;
        const H: usize = 14;
        let eye = Vec3::new(0.0, 1.0, -5.9);
        let (_, right, up) = crate::scene3d::screen_basis(eye, Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
        let mut all = vec![vec![false; W]; H];
        let mut pale = vec![vec![false; W]; H];
        let centre = [0.0, 1.0, 0.0];
        for part in icon_props(icon, centre, sway) {
            let rot = crate::world_doc::prop_rotation(&WorldProp {
                yaw: part.yaw,
                roll: part.roll,
                ..WorldProp::default()
            });
            let n = 48;
            for a in 0..=n {
                for b in 0..=n {
                    for c in 0..=2 {
                        let l = Vec3::new(
                            a as f32 / n as f32 - 0.5,
                            b as f32 / n as f32 - 0.5,
                            c as f32 / 2.0 - 0.5,
                        );
                        // Primitive outlines: the cone narrows toward +Y, the sphere is round.
                        let inside = match part.model {
                            "cone" => (l.x * l.x + l.z * l.z).sqrt() <= (0.5 - l.y) * 0.5 + 1e-3,
                            "sphere" => l.length() <= 0.5,
                            _ => true,
                        };
                        if !inside {
                            continue;
                        }
                        let w = rot * (l * Vec3::from(part.scale)) + Vec3::from(part.pos)
                            - Vec3::from(centre);
                        let (sx, sy) = (w.dot(right), w.dot(up));
                        // 0.04 m per cell, centred.
                        let col = (sx / 0.04 + W as f32 / 2.0).floor();
                        let row = (H as f32 / 2.0 - sy / 0.04 * 0.5).floor();
                        if col >= 0.0 && (col as usize) < W && row >= 0.0 && (row as usize) < H {
                            all[row as usize][col as usize] = true;
                            if part.pale {
                                pale[row as usize][col as usize] = true;
                            }
                        }
                    }
                }
            }
        }
        let _ = Quat::IDENTITY;
        (all, pale)
    }

    fn show(name: &str, g: &[Vec<bool>]) {
        println!("{name}");
        for r in g {
            println!(
                "  {}",
                r.iter()
                    .map(|&b| if b { '#' } else { '.' })
                    .collect::<String>()
            );
        }
    }

    fn ink_cols(row: &[bool]) -> Vec<usize> {
        row.iter()
            .enumerate()
            .filter(|(_, &b)| b)
            .map(|(i, _)| i)
            .collect()
    }

    #[test]
    fn item_icons_read_the_right_way_round_from_the_chase_camera() {
        use glam::Vec3;
        // The chase camera looks along +Z: screen-right is world -X.
        let (_, right, up) = crate::scene3d::screen_basis(
            Vec3::new(0.0, 1.0, -5.9),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::Y,
        );
        assert!((right - Vec3::NEG_X).length() < 1e-4 && (up - Vec3::Y).length() < 1e-4);
        for sway in [0.0f32, 0.75, -0.75] {
            // Boost: two chevrons, tips on the right (the way the cat runs), open end on the left.
            let (g, _) = icon_picture(ItemIcon::Chevron, sway);
            show(&format!("chevron sway {sway}"), &g);
            let ink: Vec<usize> = g.iter().flat_map(|r| ink_cols(r)).collect();
            let (lo, hi) = (*ink.iter().min().unwrap(), *ink.iter().max().unwrap());
            let rows_at = |c: usize| -> Vec<usize> { (0..g.len()).filter(|&r| g[r][c]).collect() };
            let mid = (g.iter().position(|r| r.iter().any(|&b| b)).unwrap()
                + g.iter().rposition(|r| r.iter().any(|&b| b)).unwrap())
                as f32
                / 2.0;
            assert!(
                rows_at(hi).iter().all(|&r| (r as f32 - mid).abs() <= 1.5),
                "the tips are in the middle on the right (sway {sway})"
            );
            assert!(
                rows_at(lo).iter().all(|&r| (r as f32 - mid).abs() >= 2.0),
                "the open ends are top and bottom on the left (sway {sway})"
            );
            // Super jump: an arrow, head above its stem.
            let (g, _) = icon_picture(ItemIcon::Arrow, sway);
            show(&format!("arrow sway {sway}"), &g);
            let widths: Vec<usize> = g.iter().map(|r| ink_cols(r).len()).collect();
            let top = widths.iter().position(|&w| w > 0).unwrap();
            let bottom = widths.iter().rposition(|&w| w > 0).unwrap();
            let widest = widths.iter().copied().max().unwrap();
            let widest_row = widths.iter().position(|&w| w == widest).unwrap();
            assert!(widths[top] < widest && widths[bottom] < widest);
            assert!(
                widest_row - top < bottom - widest_row,
                "the head is above the stem (sway {sway})"
            );
            // 2x: the 2 is on the left, the pale x on the right.
            let (g, pale) = icon_picture(ItemIcon::TwoX, sway);
            show(&format!("2x sway {sway}"), &g);
            let mean = |grid: &[Vec<bool>], want: bool| {
                let (mut sum, mut n) = (0.0, 0.0);
                for r in 0..grid.len() {
                    for c in 0..grid[r].len() {
                        if g[r][c] && (pale[r][c] == want) {
                            sum += c as f32;
                            n += 1.0;
                        }
                    }
                }
                sum / n
            };
            assert!(
                mean(&g, false) < mean(&g, true),
                "the 2 is left of the x (sway {sway})"
            );
            // Magnet: a U that opens upward, pale tips on top.
            let (g, pale) = icon_picture(ItemIcon::Horseshoe, sway);
            show(&format!("horseshoe sway {sway}"), &g);
            let top = g.iter().position(|r| r.iter().any(|&b| b)).unwrap();
            let bottom = g.iter().rposition(|r| r.iter().any(|&b| b)).unwrap();
            let runs = |row: &[bool]| {
                let c = ink_cols(row);
                1 + c.windows(2).filter(|w| w[1] > w[0] + 1).count()
            };
            assert_eq!(runs(&g[top]), 2, "two prongs at the top (sway {sway})");
            assert_eq!(runs(&g[bottom]), 1, "joined at the bottom (sway {sway})");
            assert!(pale[top].iter().any(|&b| b), "pale tips on top");
            // Meow: a heart, two lobes on top and a point below.
            let (g, _) = icon_picture(ItemIcon::Heart, sway);
            show(&format!("heart sway {sway}"), &g);
            let top = g.iter().position(|r| r.iter().any(|&b| b)).unwrap();
            let bottom = g.iter().rposition(|r| r.iter().any(|&b| b)).unwrap();
            assert_eq!(runs(&g[top]), 2, "two lobes (sway {sway})");
            assert!(
                ink_cols(&g[bottom]).len() < ink_cols(&g[(top + bottom) / 2]).len(),
                "a point at the bottom (sway {sway})"
            );
        }
    }

    #[test]
    fn ear_collar_and_tail_glints_show_from_behind() {
        let run = full_lives();
        let ears: Vec<_> = run
            .doc
            .props
            .iter()
            .filter(|p| p.name == "cat_ear")
            .collect();
        assert_eq!(ears.len(), 2);
        let eyes_y = run
            .doc
            .props
            .iter()
            .find(|p| p.name == "cat_eye")
            .unwrap()
            .position[1];
        assert!(
            ears.iter().all(|e| e.position[1] > eyes_y + 0.2),
            "above the face"
        );
        assert!(ears.iter().all(|e| e.emissive.unwrap() > 1.0));
        assert!(ears[0].position[0] != ears[1].position[0]);
        // A collar band across the back and a tail tip, behind and above the body.
        let p = |n: &str| {
            run.doc
                .props
                .iter()
                .filter(|q| q.name == n)
                .collect::<Vec<_>>()
        };
        assert!(p("cat_collar").is_empty(), "the collar is off by default");
        let tail = p("cat_tail");
        assert_eq!(tail.len(), 1);
        assert!(tail[0].position[2] < run.z && tail[0].position[1] > 1.5 * run.spec.player.scale);
        assert!(tail[0].emissive.unwrap() > 1.0);
        // Each mark has its own switch, and the eyes can be off without losing them.
        let mut spec = (*run.spec).clone();
        spec.player.eyes.enabled = false;
        spec.player.eyes.collar = true;
        let mut r2 = NeonCat::with_spec(spec);
        r2.confirm();
        r2.rebuild();
        assert!(r2.doc.props.iter().all(|q| q.name != "cat_eye"));
        let back = r2
            .doc
            .props
            .iter()
            .find(|q| q.id.ends_with("collar-back"))
            .expect("collar when asked for");
        assert!(back.position[2] < r2.z, "the strip is behind the origin");
        assert_eq!(
            r2.doc
                .props
                .iter()
                .filter(|q| q.name == "cat_collar")
                .count(),
            3
        );
        assert_eq!(
            r2.doc.props.iter().filter(|q| q.name == "cat_ear").count(),
            2
        );
    }

    #[test]
    fn gates_pay_a_bonus_and_the_formula_is_the_specs() {
        let mut run = full();
        let g = run.spec.runner.gates.clone();
        let per_gate = run.spec.runner.score.per_gate;
        assert!(per_gate > 0);
        run.z = g.z0 - 0.5;
        run.rebuild();
        run.tick(1.0 / 60.0);
        assert_eq!(run.gates, 0);
        let before = run.core.stats.score;
        run.z = g.z0 + 0.2;
        run.tick(1.0 / 60.0);
        assert_eq!(run.gates, 1);
        assert!(run.core.stats.score >= before + per_gate);
        run.tick(1.0 / 60.0);
        assert_eq!(run.gates, 1, "each gate once");
        // Zero bonus and zero distance pay: the score stays put.
        let mut spec = (*run.spec).clone();
        spec.runner.score.per_gate = 0;
        spec.runner.score.per_meter = 0.0;
        let mut flat = NeonCat::with_spec(spec);
        flat.confirm();
        flat.time = GRACE_SECS + 0.2;
        flat.z = g.z0 + 0.2;
        flat.tick(1.0 / 60.0);
        assert_eq!(flat.gates, 1);
        assert_eq!(flat.core.stats.score, 0);
    }

    #[test]
    fn coin_lines_follow_the_obstacle_that_asks() {
        let spec = full().spec.clone();
        let arc = coin_line(&spec, slot_of(&spec, "CRATE"));
        assert!(arc.iter().any(|c| c[1] > 0.9), "an arc over a hop");
        let low = coin_line(&spec, slot_of(&spec, "OVERPASS"));
        assert!(low.iter().all(|c| c[1] < 0.3), "a low line under a beam");
        assert!(coin_line(&spec, slot_of(&spec, "GENERATOR")).is_empty());
        let train = coin_line(&spec, slot_of(&spec, "TRAIN"));
        assert!(train.iter().all(|c| c[1] > 0.8), "along the roof");
        // Rest stretches are the long calm lines.
        let calm = (0..200)
            .filter(|&i| !spec.obstacle_active(i) && perch_route_at(&spec, i).is_none())
            .map(|i| coin_line(&spec, i).len())
            .max()
            .unwrap();
        assert_eq!(calm as u32, spec.runner.coins.free_line);
    }

    #[test]
    fn sections_narrow_which_kinds_spawn() {
        let spec = full().spec.clone();
        for i in 0..400 {
            if !spec.obstacle_active(i) {
                continue;
            }
            let z = spec.runner.obstacle_track.z(i);
            let (_, sec) = spec.section_at(z).unwrap();
            if !sec.allow.is_empty() {
                assert!(
                    sec.allow.contains(&spec.obstacle_kind(i).behavior),
                    "slot {i} {:?} in {:?}",
                    spec.obstacle_kind(i).name,
                    sec.kind
                );
            }
        }
    }

    #[test]
    fn obstacle_edges_follow_the_colour_codes_and_gates_stay_friendly() {
        let mut run = full();
        let codes = run.spec.style.codes.clone();
        let mut checked = std::collections::BTreeSet::new();
        for name in ["CRATE", "GENERATOR", "OVERPASS", "TRAIN", "WALL"] {
            let i = slot_of(&run.spec, name);
            start_before(&mut run, i, 0.0);
            let doc = run.document();
            let kind = run.spec.obstacle_kind(i);
            let want = kind.edge_color(&codes);
            let own = format!("edge-{i}-");
            let lines: Vec<_> = doc
                .lines
                .iter()
                .filter(|l| l.id.starts_with(&own))
                .collect();
            assert!(!lines.is_empty(), "{name} has no edges");
            for l in lines {
                assert_eq!(l.color[..3], want, "{name}");
            }
            assert_ne!(want, codes.pass_through);
            checked.insert(want);
        }
        assert_eq!(checked.len(), 5, "each behavior has its own colour");
        let doc = run.document();
        // The arch uses the pass-through code.
        let arch = doc
            .lines
            .iter()
            .find(|l| l.id.starts_with("arch-out-"))
            .expect("arch");
        assert_eq!(arch.color[..3], codes.pass_through);
        // A kind can override its code.
        let mut spec = (*run.spec).clone();
        spec.obstacles[0].color = Some([1, 2, 3]);
        assert_eq!(spec.obstacles[0].edge_color(&spec.style.codes), [1, 2, 3]);
        // Colours too alike to tell apart are refused.
        let mut bad = (*run.spec).clone();
        bad.style.codes.solid = [255, 61, 70];
        bad.style.codes.jump_over = [255, 60, 71];
        assert!(bad.validate().is_err());
    }

    fn hud_of(run: &NeonCat, w: u32, h: u32) -> DrawList {
        run.paint_hud(w, h, false)
    }

    #[test]
    fn crash_plays_out_before_the_result_and_flashes() {
        let mut run = full();
        for _ in 0..10 {
            run.tick(1.0 / 60.0);
        }
        let calm = hud_of(&run, 824, 1800);
        assert!(
            !calm.quads.iter().any(|q| q.w >= 824.0 && q.h >= 1800.0),
            "no flash while alive"
        );
        run.crash("CRATE");
        run.tick(1.0 / 60.0);
        assert_eq!(
            run.flow.phase,
            FlowPhase::Play,
            "the crash is still playing"
        );
        let hit = hud_of(&run, 824, 1800);
        assert!(
            hit.quads.iter().any(|q| q.w >= 824.0 && q.h >= 1800.0),
            "full-screen flash on the hit"
        );
        let mut frames = 1;
        while run.flow.phase == FlowPhase::Play && frames < 200 {
            run.tick(1.0 / 60.0);
            frames += 1;
        }
        let secs = frames as f32 / 60.0;
        let beat = run.spec.hud.result.beat_secs;
        assert!(
            (beat - 0.05..beat + 0.1).contains(&secs),
            "result after {secs}s, beat {beat}"
        );
        assert_eq!(run.flow.phase, FlowPhase::Result);
        let after = hud_of(&run, 824, 1800);
        assert!(!after.quads.iter().any(|q| q.w >= 824.0 && q.h >= 1800.0));
        // The HUD stays on under the host's card, dimmed.
        let alpha = |l: &DrawList| l.quads.iter().map(|q| q.color[3]).max().unwrap_or(0);
        assert!(!after.quads.is_empty());
        assert!(alpha(&after) < alpha(&calm), "dimmed under the result");
        // The in-canvas menu column does not get the HUD on top.
        assert!(run
            .paint_hud(824, 1800, true)
            .texts
            .iter()
            .all(|t| t.text != "SCORE"));
    }

    #[test]
    fn hud_is_big_on_a_phone_and_inside_the_screen() {
        let mut run = full();
        run.coins = 12;
        run.core.stats.score = 3456;
        run.tick(1.0 / 60.0);
        for (w, h) in [(412u32, 915u32), (824, 1800), (1080, 2340), (1800, 824)] {
            let l = hud_of(&run, w, h);
            let ui = crate::hud_kit::ui_scale(w, h, 1.0);
            let score = l
                .texts
                .iter()
                .find(|t| t.text == "3456")
                .expect("score text");
            // At least 26 CSS px on a 2x phone: 52 buffer px.
            assert!(
                score.size >= 13.0 * ui,
                "{w}x{h}: score size {}",
                score.size
            );
            assert!(l.texts.iter().any(|t| t.text == "12"), "coin count");
            for q in &l.quads {
                assert!(
                    q.x >= -1.0
                        && q.y >= -1.0
                        && q.x + q.w <= w as f32 + 1.0
                        && q.y + q.h <= h as f32 + 1.0,
                    "{w}x{h}: {q:?} off screen"
                );
            }
            // A panel behind the score and a coin icon: more than bare text.
            assert!(l.quads.len() > 20, "{w}x{h}: panels and coin icon");
        }
        let small = hud_of(&run, 412, 915);
        let big = hud_of(&run, 824, 1800);
        let size = |l: &DrawList| l.texts.iter().find(|t| t.text == "3456").unwrap().size;
        assert!(
            (size(&big) / size(&small) - 2.0).abs() < 0.01,
            "scales with the screen"
        );
    }

    #[test]
    fn death_lines_are_the_specs_and_fall_back() {
        let mut run = full();
        run.tick(1.0 / 60.0);
        assert_eq!(run.death_text(), "");
        run.crash("CRATE");
        assert_eq!(run.death_text(), run.spec.hud.result.deaths["CRATE"]);
        assert!(!run.death_text().contains("HIT"), "not the raw name");
        assert_eq!(run.sim_value()["death_text"], run.death_text());
        let mut spec = (*run.spec).clone();
        spec.hud.result.deaths.clear();
        spec.hud.result.death_fallback = "oops".into();
        let mut other = NeonCat::with_spec(spec);
        other.start_run();
        other.tick(1.0 / 60.0);
        other.crash("CRATE");
        assert_eq!(other.death_text(), "oops");
    }

    #[test]
    fn result_json_carries_the_ranking_config_and_this_run() {
        let mut run = full();
        run.tick(1.0 / 60.0);
        run.time = 12.34;
        run.coins = 7;
        run.gates = 3;
        run.core.stats.score = 900;
        let r = run.result_json();
        assert_eq!(r["key"], format!("{}:{}", run.spec.id, run.spec.seed));
        assert_eq!(r["top_n"], 5);
        assert_eq!(
            r["columns"],
            serde_json::json!(["time", "gates", "coins", "score"])
        );
        assert_eq!(r["sort"], "score");
        assert_eq!(r["run"]["time"].as_f64(), Some(12.3));
        assert_eq!(r["run"]["gates"], 3);
        assert_eq!(r["run"]["coins"], 7);
        assert_eq!(r["run"]["score"], 900);
        assert_eq!(run.sim_value()["result"], r);
        assert!((run.sim_value()["time"].as_f64().unwrap() - 12.34).abs() < 1e-3);
    }

    /// First slot with a perch route.
    fn route_slot(spec: &GameSpec) -> (i32, Vec<PerchBox>) {
        (0..400)
            .find_map(|i| perch_route_at(spec, i).map(|r| (i, r)))
            .expect("a route in 400 slots")
    }

    /// The cat standing on the track at `(x, y, z)`.
    fn put(run: &mut NeonCat, x: f32, y: f32, z: f32) {
        run.x = x;
        run.y = y;
        run.vy = 0.0;
        run.z = z;
        run.rebuild();
    }

    #[test]
    fn perch_routes_stand_in_free_slots_end_to_end_and_the_props_make_way() {
        let spec = full().spec.clone();
        let mut n = 0;
        for i in 0..400 {
            let Some(steps) = perch_route_at(&spec, i) else {
                continue;
            };
            n += 1;
            assert!(!spec.obstacle_active(i), "slot {i} has an obstacle");
            assert!(mischief_at(&spec, i).is_none(), "slot {i}: prop on a route");
            assert_eq!(steps.len(), spec.perch_route.len());
            let t = &spec.runner.obstacle_track;
            for w in steps.windows(2) {
                assert_eq!(w[0].z1, w[1].z0, "end to end");
            }
            assert!(steps[0].z0 > t.z(i) - t.gap * 0.5);
            assert!(steps.last().unwrap().z1 < t.z(i) + t.gap * 0.5);
            let lim = spec.runner.lanes.limit;
            assert!(steps[0].x.abs() <= lim, "inside the rails");
            assert_eq!(perch_route_at(&spec, i).unwrap()[0].x, steps[0].x);
            // Heights are the spec's, rising step by step.
            let tops: Vec<f32> = steps.iter().map(|b| b.top).collect();
            assert!(tops.windows(2).all(|w| w[1] > w[0]), "{tops:?}");
        }
        assert!(n >= 8, "{n} routes in 400 slots");
        // None in the old course, and none in a spec without a route.
        let legacy = GameSpec::legacy();
        assert!((0..200).all(|i| perch_route_at(&legacy, i).is_none()));
        let mut bare = (*spec).clone();
        bare.perch_route.clear();
        assert!((0..200).all(|i| perch_route_at(&bare, i).is_none()));
        // Props still stand where there is no route.
        assert!((0..400).any(|i| mischief_at(&spec, i).is_some()));
        // The route's coins lie along its top.
        let (i, steps) = route_slot(&spec);
        let line = coin_line(&spec, i);
        let up = line.iter().filter(|c| c[1] > 0.8).count();
        assert_eq!(up, steps.len() * spec.perch_rules.coins_per_step as usize);
        assert!(line.len() <= 32);
    }

    #[test]
    fn a_ledge_needs_a_jump_then_steps_up_and_ends_in_a_drop() {
        let spec = full().spec.clone();
        let (i, steps) = route_slot(&spec);
        let up = spec.perch_rules.step_up;
        let b0 = steps[0];
        // On the street the first step is too high to walk onto.
        let mut r = full_lives();
        put(&mut r, b0.x, 0.0, b0.z0 + 0.3);
        assert_eq!(r.ground_at(b0.x, b0.z0 + 0.3), 0.0, "walked through");
        assert!(b0.top - up > 0.0, "the first step needs a jump");
        // A cat above it lands on it.
        put(&mut r, b0.x, b0.top - up + 0.01, b0.z0 + 0.3);
        assert_eq!(r.ground_at(b0.x, b0.z0 + 0.3), b0.top);
        // Each next step is within step_up of the one before.
        for w in steps.windows(2) {
            put(&mut r, w[0].x, w[0].top, w[1].z0 + 0.2);
            assert_eq!(r.ground_at(w[0].x, w[1].z0 + 0.2), w[1].top);
        }
        // Past the last step it is the street again, and beside the ledge too.
        let last = steps.last().unwrap();
        put(&mut r, last.x, last.top, last.z1 + 0.2);
        assert_eq!(r.ground_at(last.x, last.z1 + 0.2), 0.0);
        put(&mut r, b0.x + b0.half_width + 0.3, b0.top, b0.z0 + 0.3);
        assert_eq!(r.ground_at(b0.x + b0.half_width + 0.3, b0.z0 + 0.3), 0.0);
        let _ = i;
    }

    /// The route steps the cat stands on after one jump `lead` metres before
    /// the first step at `speed`, running on to the end of the route.
    fn steps_stood_on(speed: f32, lead: f32) -> Vec<usize> {
        let mut run = full_lives();
        {
            let sp = std::sync::Arc::make_mut(&mut run.spec);
            sp.runner.speed.start = speed;
            sp.runner.speed.ramp = 0.0;
            sp.runner.speed.cap = speed;
        }
        let (_, steps) = route_slot(&run.spec.clone());
        let b0 = steps[0];
        run.invuln = 999.0;
        put(&mut run, b0.x, 0.0, b0.z0 - lead);
        run.swipe(Swipe::Up);
        let mut stood: Vec<usize> = Vec::new();
        for _ in 0..400 {
            run.tick(1.0 / 60.0);
            if let Some((_, n, top)) = run.perch_support(run.x, run.z) {
                if (run.y - top).abs() < 0.02 && !stood.contains(&n) {
                    stood.push(n);
                }
            }
            if run.z > steps.last().unwrap().z1 + 2.0 {
                break;
            }
        }
        stood
    }

    #[test]
    fn a_jump_onto_the_first_step_lands_at_every_speed_and_timing() {
        // The bot jumps from the street at every timing from "on the front
        // edge" (a late jump) to "a long way ahead", at the start speed and
        // up to the cap. Wherever the jump could get onto the first step it
        // lands on it, and the run then goes up steps 2 and 3 in order.
        // (Before, a late jump at running speed rose through the front of
        // the AC unit and came down on the pipe, so the first step never
        // paid a landing.)
        for speed in [8.0f32, 10.5, 12.0, 14.0, 16.0, 18.0, 20.0] {
            for lead4 in 0..=12 {
                let lead = lead4 as f32 * 0.25;
                let stood = steps_stood_on(speed, lead);
                assert_eq!(stood, vec![0, 1, 2], "speed {speed} lead {lead}: {stood:?}");
            }
        }
    }

    #[test]
    fn a_late_jump_at_the_start_speed_lands_on_the_ac_unit_and_pays_the_perch() {
        let mut run = full_lives();
        let (i, steps) = route_slot(&run.spec.clone());
        let b0 = steps[0];
        run.invuln = 999.0;
        // Normal speed, the jump pressed at the front edge of the first step.
        put(&mut run, b0.x, 0.0, b0.z0 - 0.05);
        let score0 = run.core.stats.score;
        run.swipe(Swipe::Up);
        let mut on_first = false;
        for _ in 0..30 {
            run.tick(1.0 / 60.0);
            on_first |= run.on_perch && (run.y - b0.top).abs() < 0.02 && run.z < b0.z1;
        }
        assert!(on_first, "never stood on the first step");
        assert!(run.perch_landed.contains(&i));
        assert!(run.core.stats.score >= score0 + run.spec.perch_rules.land_score);
    }

    #[test]
    fn running_a_route_pays_the_landing_the_time_and_the_bonus_once() {
        let spec = full_lives().spec.clone();
        let (i, steps) = route_slot(&spec);
        let mut run = full_lives();
        run.invuln = 99.0;
        let b0 = steps[0];
        put(&mut run, b0.x, b0.top + 0.05, b0.z0 - 0.4);
        let score0 = run.core.stats.score;
        let mut seen = false;
        let mut top_y: f32 = 0.0;
        for _ in 0..240 {
            run.tick(1.0 / 60.0);
            seen |= run.on_perch;
            top_y = top_y.max(run.y);
            if run.z > steps.last().unwrap().z1 + 1.0 && run.y == 0.0 {
                break;
            }
        }
        assert!(seen, "stood on a ledge");
        assert!(
            top_y >= steps.last().unwrap().top - 0.01,
            "climbed to the top"
        );
        assert_eq!(run.perch_landed.len(), 1);
        assert!(run.perch_landed.contains(&i));
        assert_eq!(run.perch_routed.len(), 1, "reached the last step");
        let gained = run.core.stats.score - score0;
        let r = &spec.perch_rules;
        assert!(gained >= r.land_score + r.route_bonus, "gained {gained}");
        assert_eq!(run.y, 0.0, "back on the street at the end of the route");
        // A second pass over the same slot pays nothing for landing or the route.
        let before = run.perch_landed.len();
        run.perch_landed.insert(i);
        run.z = b0.z0 - 0.4;
        run.y = b0.top + 0.05;
        run.x = b0.x;
        for _ in 0..30 {
            run.tick(1.0 / 60.0);
        }
        assert_eq!(run.perch_landed.len(), before);
        assert_eq!(run.perch_routed.len(), 1);
        assert_eq!(run.sim_value()["perch"]["routes"], 1);
        // Classic and legacy have no ledges at all.
        let classic = NeonCat::from_json(include_str!(
            "../tests/fixtures/neon_cat_classic_world.json"
        ))
        .unwrap();
        assert!(classic.spec.perch_route.is_empty() && !classic.spec.chaser.enabled);
    }

    #[test]
    fn a_hit_calls_the_robot_and_a_second_one_while_it_is_there_is_fatal() {
        let mut run = full_lives();
        assert_eq!(run.robot_t, 0.0);
        run.crash("CRATE");
        assert!(run.core.stats.alive);
        assert_eq!(run.lives, 2);
        assert!(run.robot_t > 9.0, "robot called: {}", run.robot_t);
        assert_eq!(run.sim_value()["robot"]["fleeing"], false);
        run.invuln = 0.0;
        run.crash("CRATE");
        assert!(!run.core.stats.alive, "caught by the robot");
        // One life: no robot at all, the run just ends.
        let mut one = full();
        one.crash("CRATE");
        assert!(!one.core.stats.alive);
        assert_eq!(one.robot_t, 0.0);
        // The old course never calls it.
        let mut classic = NeonCat::from_json(include_str!(
            "../tests/fixtures/neon_cat_classic_world.json"
        ))
        .unwrap();
        classic.lives = 3;
        classic.confirm();
        classic.crash("CRATE");
        assert_eq!(classic.robot_t, 0.0);
    }

    #[test]
    fn the_robot_gives_up_a_meow_scares_it_and_a_ledge_wears_it_out_faster() {
        // Gives up: after its time the next hit is a plain one again.
        let mut run = full_lives();
        run.crash("CRATE");
        for _ in 0..800 {
            run.update_robot(1.0 / 60.0);
        }
        assert_eq!(run.robot_t, 0.0);
        run.invuln = 0.0;
        run.crash("CRATE");
        assert!(run.core.stats.alive && run.lives == 1, "a plain hit");
        // A meow scares it off, pays, and the next hit is plain.
        let mut run = full_lives();
        run.crash("CRATE");
        let score = run.core.stats.score;
        run.take_item(ItemEffect::Meow, [run.x, 0.5, run.z]);
        assert!(run.robot_flee > 0.0);
        assert_eq!(run.sim_value()["robot"]["fleeing"], true);
        assert!(run.core.stats.score >= score + run.spec.chaser.scare_score);
        run.invuln = 0.0;
        run.crash("CRATE");
        assert!(run.core.stats.alive, "the scared robot cannot catch");
        for _ in 0..90 {
            run.update_robot(1.0 / 60.0);
        }
        assert_eq!(run.robot_t, 0.0, "gone");
        // Time on a ledge or under a boost drains it faster.
        let left = |perch: bool, boost: bool| {
            let mut r = full_lives();
            r.crash("CRATE");
            r.on_perch = perch;
            if boost {
                r.fx[FX_BOOST] = 5.0;
            }
            for _ in 0..60 {
                r.update_robot(1.0 / 60.0);
            }
            r.robot_t
        };
        let plain = left(false, false);
        assert!((plain - (r_secs() - 1.0)).abs() < 0.05, "{plain}");
        assert!(left(true, false) < plain - 1.0);
        assert!(left(false, true) < plain - 1.0);
    }

    fn r_secs() -> f32 {
        full_lives().spec.chaser.secs
    }

    #[test]
    fn the_robot_is_inside_the_chase_camera_all_the_way_back() {
        let mut run = full_lives();
        run.crash("CRATE");
        let spec = run.spec.clone();
        let c = &spec.chaser;
        // From just called to just before it gives up, and across the lanes.
        for lane_x in [-2.1f32, 0.0, 2.1] {
            for used in [0.0f32, 0.5, 0.99] {
                let mut r = full_lives();
                r.crash("CRATE");
                r.x = lane_x;
                r.robot_x = lane_x;
                r.robot_t = c.secs * (1.0 - used);
                r.rebuild();
                let cam = &r.document().cameras[0];
                let at = [r.robot_x, 0.35, r.z - r.robot_gap()];
                let f = norm(sub(cam.target, cam.position));
                let right = norm(cross(f, [0.0, 1.0, 0.0]));
                let up = cross(right, f);
                let d = sub(at, cam.position);
                let depth = dot(d, f);
                assert!(depth > 0.5, "in front of the camera: {depth}");
                let ty = (cam.fov.to_radians() * 0.5).tan();
                let ny = dot(d, up) / depth / ty;
                let nx = dot(d, right) / depth / ty; // times the aspect: x / (ty * aspect)
                assert!(
                    ny.abs() < 0.95,
                    "robot off the screen vertically: ny {ny} (gap {})",
                    r.robot_gap()
                );
                // A 16:9 screen has room for every lane. A 9:16 phone is the
                // narrowest we play on: the middle lane must fit there too.
                assert!(
                    (nx * 9.0 / 16.0).abs() < 0.95,
                    "off the side (16:9): nx {nx}"
                );
                if lane_x == 0.0 {
                    assert!(
                        (nx * 16.0 / 9.0).abs() < 0.95,
                        "off the side (9:16): nx {nx}"
                    );
                }
            }
        }
        // And the doc really draws it, with its light.
        run.rebuild();
        assert!(run
            .document()
            .props
            .iter()
            .any(|p| p.id.contains("robot-beacon")));
        let mut none = full_lives();
        none.rebuild();
        assert!(!none
            .document()
            .props
            .iter()
            .any(|p| p.id.contains("robot-")));
    }

    fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }
    fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
    fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    fn norm(a: [f32; 3]) -> [f32; 3] {
        let l = dot(a, a).sqrt().max(1e-6);
        [a[0] / l, a[1] / l, a[2] / l]
    }
}
