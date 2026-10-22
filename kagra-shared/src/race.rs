//! Racing on play_world: kinematic drive, lap / finish.
//!
//! Sibling of collectathon / action / fps / td. WASD / arrows steer+throttle a
//! capsule/box car on a World.dump track (road boxes, not a flat empty plane).
//! Finish or lap is dump-visible (`name` + `flag` + `coins` count). Title ->
//! play -> result reuses `WorldPlay` / `GamePhase`. Chase camera follows the
//! car. Capsule/box, not VRM, not Rapier. Overlay count on shared wgpu 30.
//! No drifting physics, net, fighting, novel, or new ECS.
//!
//! A dump with `waypoint` props (4+) drives the bicycle model in `vehicle.rs`
//! on a `road::RoadPath`, held on the ribbon by the same road bounds
//! `driving.rs` uses. `laps` scale.x is the lap count (default [`LAP_NEED`]).
//! Each finish stores a lap time; the best lap is replayed on a `ghost` prop.

use crate::collectathon::WalkInput;
use crate::driving::practical_top_speed;
use crate::game::GamePhase;
use crate::road::RoadPath;
use crate::scene::{DrawList, Quad};
use crate::vehicle::{DriveInput, Truck, TruckSpec};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};
use glam::Vec3;

pub const GAME_ID: &str = "race_drive";
pub const LAP_NEED: u32 = 1;
pub const ACCEL: f32 = 16.0;
pub const BRAKE: f32 = 22.0;
pub const DRAG: f32 = 2.8;
pub const MAX_SPEED: f32 = 14.0;
pub const REV_MAX: f32 = 5.0;
pub const STEER: f32 = 2.15;
pub const BODY_H: f32 = 0.55;
pub const CAR_HALF_H: f32 = 0.22;
pub const CAM_BACK: f32 = 11.0;
pub const CAM_UP: f32 = 4.8;
pub const CAM_LOOK: f32 = 0.7;
/// Half-width of a waypoint course (m). Matches a small oval, not `driving::ROAD_WIDTH`.
const COURSE_HALF: f32 = 2.35;
const GHOST_DT: f32 = 1.0 / 25.0;
const GHOST_CAP: usize = 4000;

/// One sample of the car while a lap is open. Replayed as the ghost.
#[derive(Clone, Copy, Debug)]
struct GhostSample {
    t: f32,
    x: f32,
    z: f32,
    yaw: f32,
}

/// Live race around a dump. Pose / speed stay here; finish, split, flag, and
/// the car in the dump are the query/dump source of truth.
#[derive(Clone, Debug)]
pub struct RaceGame {
    pub speed: f32,
    pub yaw: f32,
    pub x: f32,
    pub z: f32,
    /// Front-wheel angle (radians, right positive). Vehicle courses only.
    pub steer: f32,
    pub laps: u32,
    /// Laps required to win. Dump prop `laps` scale.x, else [`LAP_NEED`].
    pub laps_need: u32,
    /// Seconds on the current lap.
    pub lap_time: f32,
    /// Best completed lap. 0 until the first finish.
    pub best_lap: f32,
    /// Seconds since the green flag.
    pub race_time: f32,
    pub armed: bool,
    pub in_finish: bool,
    pub in_split: bool,
    pub won: bool,
    pub done: bool,
    recording: Vec<GhostSample>,
    best_line: Vec<GhostSample>,
}

impl Default for RaceGame {
    fn default() -> Self {
        Self {
            speed: 0.0,
            yaw: 0.0,
            x: 0.0,
            z: 0.0,
            steer: 0.0,
            laps: 0,
            laps_need: LAP_NEED,
            lap_time: 0.0,
            best_lap: 0.0,
            race_time: 0.0,
            armed: false,
            in_finish: false,
            in_split: false,
            won: false,
            done: false,
            recording: Vec::new(),
            best_line: Vec::new(),
        }
    }
}

impl RaceGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let mut g = Self::default();
        g.rebind(doc);
        g
    }

    pub fn has_ghost(&self) -> bool {
        !self.best_line.is_empty()
    }

    fn rebind(&mut self, doc: &WorldDoc) {
        *self = Self::default();
        self.laps_need = laps_need_of(doc);
        if let Some(w) = player_ref(doc) {
            self.x = w.position[0];
            self.z = w.position[2];
            self.yaw = w.yaw;
        }
    }
}

pub fn is_race(doc: &WorldDoc) -> bool {
    doc.genre_is(GAME_ID)
}

fn is_finish(p: &WorldProp) -> bool {
    p.name == "finish"
}

fn is_split(p: &WorldProp) -> bool {
    p.name == "split"
}

fn is_flag(p: &WorldProp) -> bool {
    p.name == "flag"
}

fn is_car(p: &WorldProp) -> bool {
    p.name == "car"
}

fn is_wall(p: &WorldProp) -> bool {
    p.name == "wall"
}

fn is_boxish(p: &WorldProp) -> bool {
    matches!(p.model.to_ascii_lowercase().as_str(), "box" | "cube")
}

fn player_ref(doc: &WorldDoc) -> Option<&WorldWalker> {
    doc.player.as_ref().or(doc.walkers.first())
}

fn write_player(doc: &mut WorldDoc, walker: WorldWalker) {
    if let Some(existing) = doc.player.as_mut() {
        *existing = walker.clone();
    } else {
        doc.player = Some(walker.clone());
    }
    let mut found = false;
    for w in &mut doc.walkers {
        if w.id == walker.id {
            *w = walker.clone();
            found = true;
        }
    }
    if !found {
        if let Some(first) = doc.walkers.first_mut() {
            *first = walker;
        } else {
            doc.walkers.push(walker);
        }
    }
}

fn set_player_name(doc: &mut WorldDoc, name: &str) {
    let Some(mut w) = player_ref(doc).cloned() else {
        return;
    };
    w.name = name.into();
    write_player(doc, w);
}

fn overlaps_xz(pos: [f32; 3], prop: &WorldProp, extra: f32) -> bool {
    let hx = 0.5 * prop.scale[0].abs() + extra;
    let hz = 0.5 * prop.scale[2].abs() + extra;
    (pos[0] - prop.position[0]).abs() <= hx && (pos[2] - prop.position[2]).abs() <= hz
}

fn heading(yaw: f32) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    Vec3::new(s, 0.0, c)
}

/// Sit car / finish / split / flag on the floor. Does not spawn extra cars.
pub fn seed(doc: &mut WorldDoc) {
    if !is_race(doc) {
        return;
    }
    let mut ys = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        if !(is_car(p) || is_finish(p) || is_split(p) || is_flag(p)) || !p.enabled {
            continue;
        }
        let extra = if is_flag(p) {
            0.5 * p.scale[1].abs().max(0.4)
        } else if is_boxish(p) {
            0.5 * p.scale[1].abs().max(0.08)
        } else {
            BODY_H * p.scale[1].abs().max(0.6)
        };
        let y = doc.height_at(p.position[0], p.position[2]) + extra;
        ys.push((i, y));
    }
    for (i, y) in ys {
        if let Some(p) = doc.props.get_mut(i) {
            p.position[1] = y;
        }
    }
    if let Some(p) = player_ref(doc) {
        let mut w = p.clone();
        w.position[1] = doc.height_at(w.position[0], w.position[2]) + BODY_H;
        w.on_ground = true;
        write_player(doc, w);
    }
    doc.coins = 0;
    place_chase_camera(doc);
}

/// Chase cam behind the car heading so the track stays readable.
pub fn place_chase_camera(doc: &mut WorldDoc) {
    let Some(w) = player_ref(doc) else {
        return;
    };
    let pos = Vec3::new(w.position[0], w.position[1], w.position[2]);
    let fwd = heading(w.yaw);
    let eye = pos - fwd * CAM_BACK + Vec3::Y * CAM_UP;
    let target = pos + fwd * 2.2 + Vec3::Y * CAM_LOOK;
    let fov = doc.cameras.first().map(|c| c.fov).unwrap_or(52.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye.to_array();
        cam.target = target.to_array();
        cam.name = "chase".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "chase".into(),
            position: eye.to_array(),
            target: target.to_array(),
            fov,
        });
    }
}

/// Steer + throttle. Caller may have walked; this pose wins.
///
/// Waypoint courses use [`Truck`] on a [`RoadPath`]. Other dumps keep the
/// original kinematic body so the 1-lap box track stays the same.
pub fn tick(doc: &mut WorldDoc, game: &mut RaceGame, input: WalkInput, dt: f32) {
    if game.done {
        place_chase_camera(doc);
        return;
    }
    let input = input.clamped();
    game.race_time += dt;
    game.lap_time += dt;
    if course_points(doc).len() >= 4 {
        drive_vehicle(doc, game, input, dt);
    } else {
        drive(doc, game, input, dt);
    }
    sync_car(doc, game);
    write_pose(doc, game);
    record_sample(game);
    register_lap(doc, game);
    sync_ghost(doc, game);
    sync_lap_clock(doc, game);
    place_chase_camera(doc);
}

fn laps_need_of(doc: &WorldDoc) -> u32 {
    doc.props
        .iter()
        .find(|p| p.name == "laps")
        .map(|p| (p.scale[0].round() as u32).max(1))
        .unwrap_or(LAP_NEED)
}

fn course_points(doc: &WorldDoc) -> Vec<Vec3> {
    let mut props: Vec<&WorldProp> = doc.props.iter().filter(|p| p.name == "waypoint").collect();
    props.sort_by(|a, b| a.id.cmp(&b.id));
    let mut pts: Vec<Vec3> = props
        .into_iter()
        .map(|p| Vec3::new(p.position[0], 0.0, p.position[2]))
        .collect();
    if pts.len() >= 2 {
        let first = pts[0];
        if pts.last().is_some_and(|p| p.distance(first) > 0.25) {
            pts.push(first);
        }
    }
    pts
}

fn arcade_spec() -> TruckSpec {
    let mut spec = TruckSpec {
        wheelbase: 2.05,
        max_steer: 40f32.to_radians(),
        steer_rate: 0.10,
        accel: 11.0,
        brake: 16.0,
        drag: 0.09,
        rolling: 0.6,
        max_speed: 16.0,
        max_reverse: 3.0,
        allow_reverse: true,
        size: Vec3::new(1.7, 1.3, 3.4),
    };
    spec.max_speed = practical_top_speed(&spec).min(spec.max_speed);
    spec
}

/// Bicycle model + road ribbon. `lx` is steer (right positive), `lz` is
/// throttle / brake, same signs as the kinematic body.
fn drive_vehicle(doc: &WorldDoc, game: &mut RaceGame, input: WalkInput, dt: f32) {
    let pts = course_points(doc);
    if pts.len() < 3 {
        drive(doc, game, input, dt);
        return;
    }
    let path = RoadPath::from_waypoints(&pts, 8);
    let mut truck = Truck {
        spec: arcade_spec(),
        pos: Vec3::new(game.x, 0.0, game.z),
        heading: game.yaw,
        speed: game.speed,
        steer_angle: game.steer,
    };
    truck.update(
        DriveInput {
            steer: input.lx,
            throttle: input.lz.max(0.0),
            brake: (-input.lz).max(0.0),
        },
        dt,
    );
    let nearest = path.nearest(truck.pos);
    crate::collide::apply_road_bounds(&mut truck, nearest.pos, nearest.right, COURSE_HALF);
    let (mut x, mut z) = push_walls(doc, truck.pos.x, truck.pos.z);
    let half = doc.half.max(4.0);
    let pad = 2.2;
    x = x.clamp(-half + pad, half - pad);
    z = z.clamp(-half + pad, half - pad);
    game.x = x;
    game.z = z;
    game.yaw = truck.heading;
    game.speed = truck.speed;
    game.steer = truck.steer_angle;
}

fn drive(doc: &WorldDoc, game: &mut RaceGame, input: WalkInput, dt: f32) {
    let throttle = input.lz;
    if throttle > 0.04 {
        game.speed += ACCEL * throttle * dt;
    } else if throttle < -0.04 {
        game.speed += BRAKE * throttle * dt;
    }
    game.speed -= game.speed * DRAG * dt;
    game.speed = game.speed.clamp(-REV_MAX, MAX_SPEED);

    let grip = (game.speed.abs() / MAX_SPEED).clamp(0.28, 1.0);
    game.yaw -= input.lx * STEER * grip * dt;

    let fwd = heading(game.yaw);
    let mut x = game.x + fwd.x * game.speed * dt;
    let mut z = game.z + fwd.z * game.speed * dt;
    let half = doc.half.max(4.0);
    let pad = 2.2;
    x = x.clamp(-half + pad, half - pad);
    z = z.clamp(-half + pad, half - pad);
    let (x, z) = push_walls(doc, x, z);
    game.x = x;
    game.z = z;
}

fn push_walls(doc: &WorldDoc, mut x: f32, mut z: f32) -> (f32, f32) {
    let r = 0.55;
    for p in &doc.props {
        if !is_wall(p) || !p.enabled {
            continue;
        }
        let hx = 0.5 * p.scale[0].abs() + r;
        let hz = 0.5 * p.scale[2].abs() + r;
        let dx = x - p.position[0];
        let dz = z - p.position[2];
        if dx.abs() > hx || dz.abs() > hz {
            continue;
        }
        let ox = hx - dx.abs();
        let oz = hz - dz.abs();
        if ox < oz {
            x = p.position[0] + dx.signum() * hx;
        } else {
            z = p.position[2] + dz.signum() * hz;
        }
    }
    (x, z)
}

fn write_pose(doc: &mut WorldDoc, game: &RaceGame) {
    let Some(mut w) = player_ref(doc).cloned() else {
        return;
    };
    let y = doc.height_at(game.x, game.z) + BODY_H;
    w.position = [game.x, y, game.z];
    w.yaw = game.yaw;
    w.face = game.yaw;
    w.on_ground = true;
    if game.done && game.won {
        w.name = "finish".into();
    } else if game.laps > 0 {
        w.name = "lap".into();
    } else {
        w.name = "player".into();
    }
    write_player(doc, w);
}

fn sync_car(doc: &mut WorldDoc, game: &RaceGame) {
    let y = doc.height_at(game.x, game.z) + CAR_HALF_H;
    if let Some(p) = doc.props.iter_mut().find(|p| is_car(p)) {
        p.position = [game.x, y, game.z];
        p.yaw = game.yaw;
        p.enabled = true;
    }
}

fn register_lap(doc: &mut WorldDoc, game: &mut RaceGame) {
    let pos = [game.x, 0.0, game.z];
    let split_hit = doc
        .props
        .iter()
        .any(|p| is_split(p) && p.enabled && overlaps_xz(pos, p, 0.35));
    let finish_hit = doc
        .props
        .iter()
        .any(|p| is_finish(p) && p.enabled && overlaps_xz(pos, p, 0.35));

    if split_hit && !game.in_split {
        game.armed = true;
    }
    game.in_split = split_hit;

    if finish_hit && !game.in_finish && game.armed {
        game.laps = game.laps.saturating_add(1);
        game.armed = false;
        doc.coins = game.laps;
        if let Some(f) = doc.props.iter_mut().find(|p| is_flag(p)) {
            if let Some(c) = f.color.as_mut() {
                *c = [240, 196, 72];
            }
        }
        commit_lap(game);
        if game.laps >= game.laps_need.max(1) {
            game.won = true;
            game.done = true;
            set_player_name(doc, "finish");
        } else {
            set_player_name(doc, "lap");
        }
    }
    game.in_finish = finish_hit;
    if game.laps > 0 {
        doc.coins = game.laps;
    }
}

fn commit_lap(game: &mut RaceGame) {
    let t = game.lap_time.max(0.0);
    if t > 0.05 && !game.recording.is_empty() && (game.best_lap <= 0.0 || t < game.best_lap) {
        game.best_lap = t;
        game.best_line = std::mem::take(&mut game.recording);
    } else {
        game.recording.clear();
    }
    game.lap_time = 0.0;
}

fn record_sample(game: &mut RaceGame) {
    if game.recording.len() >= GHOST_CAP {
        return;
    }
    if let Some(prev) = game.recording.last() {
        if game.lap_time - prev.t < GHOST_DT {
            return;
        }
    }
    game.recording.push(GhostSample {
        t: game.lap_time,
        x: game.x,
        z: game.z,
        yaw: game.yaw,
    });
}

fn ghost_pose(line: &[GhostSample], t: f32) -> Option<(f32, f32, f32)> {
    let first = *line.first()?;
    if t <= first.t {
        return Some((first.x, first.z, first.yaw));
    }
    let last = *line.last().unwrap_or(&first);
    if t >= last.t {
        return Some((last.x, last.z, last.yaw));
    }
    let mut lo = 0usize;
    let mut hi = line.len() - 1;
    while lo + 1 < hi {
        let mid = (lo + hi) / 2;
        if line[mid].t <= t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let a = line[lo];
    let b = line[hi];
    let span = (b.t - a.t).max(1e-4);
    let u = ((t - a.t) / span).clamp(0.0, 1.0);
    let yaw = a.yaw + crate::vehicle::wrap_angle(b.yaw - a.yaw) * u;
    Some((a.x + (b.x - a.x) * u, a.z + (b.z - a.z) * u, yaw))
}

fn sync_ghost(doc: &mut WorldDoc, game: &RaceGame) {
    let Some((x, z, yaw)) = ghost_pose(&game.best_line, game.lap_time) else {
        if let Some(g) = doc.props.iter_mut().find(|p| p.name == "ghost") {
            g.enabled = false;
        }
        return;
    };
    let y = doc.height_at(x, z) + CAR_HALF_H + 0.08;
    if let Some(g) = doc.props.iter_mut().find(|p| p.name == "ghost") {
        g.position = [x, y, z];
        g.yaw = yaw;
        g.enabled = true;
    }
}

fn sync_lap_clock(doc: &mut WorldDoc, game: &RaceGame) {
    let Some(timer) = doc.timers.iter_mut().find(|t| t.name == "lap") else {
        return;
    };
    timer.seconds = game.best_lap;
    timer.remaining = game.lap_time;
    timer.active = false;
}

pub fn build_hud(game: &RaceGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let pad = 16.0 * scale;
    let mut quads = Vec::new();

    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [10, 12, 14, 150]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [16, 22, 28, 230],
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.58,
                w * 0.36,
                52.0 * scale,
                [240, 196, 72, 255],
            ));
        }
        GamePhase::Playing => {
            let pip = 18.0 * scale;
            let gap = 6.0 * scale;
            let need = game.laps_need.max(1);
            for i in 0..need {
                let filled = i < game.laps;
                let color = if filled {
                    [240, 196, 72, 255]
                } else {
                    [40, 48, 56, 220]
                };
                quads.push(Quad::new(
                    pad + i as f32 * (pip + gap),
                    pad,
                    pip,
                    pip,
                    color,
                ));
            }
            // throttle bar
            let t = (game.speed.abs() / MAX_SPEED).clamp(0.0, 1.0);
            quads.push(Quad::new(
                pad,
                h - pad - 10.0 * scale,
                120.0 * scale * t.max(0.04),
                10.0 * scale,
                [72, 160, 220, 230],
            ));
        }
        GamePhase::Complete => {
            quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 16, 18, 210]));
            let bar = (game.laps.max(1) as f32 / game.laps_need.max(1) as f32).clamp(0.15, 1.0);
            quads.push(Quad::new(
                w * 0.22,
                h * 0.40,
                w * 0.56 * bar,
                18.0 * scale,
                [240, 196, 72, 255],
            ));
        }
    }
    DrawList {
        clear: [48, 56, 62, 255],
        quads,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const TRACK: &str = include_str!("../tests/fixtures/race_drive_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const RANGE: &str = include_str!("../tests/fixtures/fps_range_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");
    const LANE: &str = include_str!("../tests/fixtures/td_lane_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(TRACK).unwrap();
        play.confirm();
        play
    }

    fn park(play: &mut WorldPlay, x: f32, z: f32) {
        play.race.x = x;
        play.race.z = z;
        play.race.speed = 0.0;
        let y = play.doc.height_at(x, z) + BODY_H;
        if let Some(w) = play.doc.player.as_mut() {
            w.position = [x, y, z];
        }
        let walker = play.doc.player.clone().unwrap();
        write_player(&mut play.doc, walker);
        sync_car(&mut play.doc, &play.race);
    }

    #[test]
    fn dump_is_race_not_td_or_fps() {
        let doc = WorldDoc::from_json(TRACK).unwrap();
        assert!(is_race(&doc));
        assert_eq!(GAME_ID, "race_drive");
        assert!(!crate::td::is_td(&doc));
        assert!(!crate::fps::is_fps(&doc));
        assert!(!crate::action::is_action(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_race(&crest));
        let fps = WorldDoc::from_json(RANGE).unwrap();
        assert!(!is_race(&fps));
        let td = WorldDoc::from_json(LANE).unwrap();
        assert!(!is_race(&td));
        let finish: Vec<_> = doc
            .props
            .iter()
            .filter(|p| is_finish(p) && p.enabled)
            .collect();
        assert_eq!(finish.len(), 1);
        assert!(is_boxish(finish[0]));
        let roads: Vec<_> = doc
            .props
            .iter()
            .filter(|p| p.name == "road" && p.enabled)
            .collect();
        assert!(
            roads.len() >= 4,
            "need a readable track, got {}",
            roads.len()
        );
        let car = doc.props.iter().find(|p| is_car(p) && p.enabled).unwrap();
        assert!(is_boxish(car) || car.model == "capsule");
        let flag = doc.props.iter().find(|p| is_flag(p) && p.enabled);
        assert!(flag.is_some(), "flag in dump");
        assert_eq!(doc.player.as_ref().unwrap().name, "player");
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("finish"));
        assert!(json.contains("split"));
        assert!(json.contains("car"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 10,
            "track + car must read, n={}",
            scene.instance_count()
        );
        assert!(scene.local_lights.iter().all(|l| l.intensity > 0.0));
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_drive_until_confirm() {
        let mut play = WorldPlay::from_json(TRACK).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_race());
        assert!(!play.is_td());
        assert!(!play.is_fps());
        assert!(!play.is_collectathon());
        let start = play.doc.player.as_ref().unwrap().position;
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..40 {
            play.tick(1.0 / 60.0);
        }
        let now = play.doc.player.as_ref().unwrap().position;
        assert_eq!(now, start, "title must not drive");
        assert_eq!(play.race.laps, 0);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "chase");
    }

    #[test]
    fn throttle_moves_car_on_track() {
        let mut play = play_started();
        let start = play.doc.player.as_ref().unwrap().position;
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..90 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap();
        let dx = p.position[0] - start[0];
        let dz = p.position[2] - start[2];
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(
            dist > 1.5,
            "throttle should move the car, dist={dist} start={start:?} now={:?}",
            p.position
        );
        assert!(p.on_ground);
        let car = play.doc.props.iter().find(|p| is_car(p)).unwrap();
        assert!(
            (car.position[0] - p.position[0]).abs() < 0.05
                && (car.position[2] - p.position[2]).abs() < 0.05,
            "box car follows walker"
        );
        assert_eq!(play.doc.cameras[0].name, "chase");
    }

    #[test]
    fn steer_changes_yaw() {
        let mut play = play_started();
        play.race.speed = 8.0;
        let yaw0 = play.race.yaw;
        play.input = WalkInput {
            lx: 1.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        let yaw = play.doc.player.as_ref().unwrap().yaw;
        assert!(
            (yaw - yaw0).abs() > 0.15,
            "steer should change heading, yaw0={yaw0} now={yaw}"
        );
    }

    #[test]
    fn split_then_finish_counts_lap_in_dump() {
        let mut play = play_started();
        let split = play
            .doc
            .props
            .iter()
            .find(|p| is_split(p))
            .unwrap()
            .position;
        park(&mut play, split[0], split[2]);
        play.tick(1.0 / 60.0);
        assert!(play.race.armed, "split should arm a lap");
        park(&mut play, split[0] + 3.0, split[2]);
        play.tick(1.0 / 60.0);
        let finish = play
            .doc
            .props
            .iter()
            .find(|p| is_finish(p))
            .unwrap()
            .position;
        park(&mut play, finish[0], finish[2]);
        play.tick(1.0 / 60.0);
        assert!(play.race.laps >= 1, "laps {}", play.race.laps);
        assert_eq!(play.doc.coins, play.race.laps);
        let dump = play.doc.to_json().unwrap();
        assert!(
            dump.contains("finish") || dump.contains("\"lap\""),
            "lap/finish must be dump-visible"
        );
        assert!(dump.contains("flag"));
        assert!(play.race.won);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "finish");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "overlay count");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.race.won);
        assert_eq!(play.race.laps, 0);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "player");
        assert_eq!(play.doc.coins, 0);
    }

    #[test]
    fn chase_camera_follows_the_car() {
        let mut play = play_started();
        let cam0 = play.doc.cameras[0].position;
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..50 {
            play.tick(1.0 / 60.0);
        }
        let cam = play.doc.cameras[0].position;
        let d = (cam[0] - cam0[0]).abs() + (cam[2] - cam0[2]).abs();
        assert!(
            d > 0.4,
            "chase cam should follow, d={d} cam={cam:?} was={cam0:?}"
        );
        assert_eq!(play.doc.cameras[0].name, "chase");
        let p = play.doc.player.as_ref().unwrap().position;
        let tgt = play.doc.cameras[0].target;
        let td = (tgt[0] - p[0]).abs() + (tgt[2] - p[2]).abs();
        assert!(td < 3.5, "camera looks near the car, td={td}");
    }

    #[test]
    fn crest_fps_td_still_own_their_dumps() {
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_race());
        let fps = WorldPlay::from_json(RANGE).unwrap();
        assert!(fps.is_fps());
        assert!(!fps.is_race());
        let td = WorldPlay::from_json(LANE).unwrap();
        assert!(td.is_td());
        assert!(!td.is_race());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_race());
    }
}
