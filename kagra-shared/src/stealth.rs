//! Hide volume + guard facing cone + clear/caught on play_world.
//!
//! Sibling of action / novel. Player walks a capsule; a box/prop is a hide
//! volume; a guard has a facing cone. Unseen reach of the exit is clear;
//! seen is caught. Title -> play -> result reuses `WorldPlay` / `GamePhase`.
//! Dump-visible `name` (player/hidden/clear/caught) + flag.
//! Indoor lights stay 4 slots. Does not rewrite other genre loops.
//!
//! A dump with `patrol` props (parent = guard id) turns on the yard loop:
//! guards ping-pong those points, pause and look at a `scan` prop, and the
//! player has a noise radius plus an alert meter. Crouch (`WalkInput.dodge`)
//! is quieter and harder to spot. A `key` prop must be carried before the
//! exit counts. Cones named `cone` track each guard. Without patrol props
//! the original one-tick hide/cone room is unchanged.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};

pub const GAME_ID: &str = "stealth_hide";
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const CONE_RANGE: f32 = 6.0;
pub const CONE_HALF: f32 = 0.68;
pub const FLAG_CLEAR: &str = "clear";
pub const FLAG_CAUGHT: &str = "caught";
pub const NAME_HIDDEN: &str = "hidden";
pub const NAME_PLAYER: &str = "player";

/// Walk speed scale while crouching (`dodge`).
pub const CROUCH_SCALE: f32 = 0.55;
/// Vision range scale while crouching.
const CROUCH_SIGHT: f32 = 0.70;
const WALK_NOISE: f32 = 3.5;
const CROUCH_NOISE: f32 = 0.75;
const GUARD_SPEED: f32 = 0.90;
const SCAN_TIME: f32 = 1.15;
const SEE_RATE: f32 = 1.35;
const HEAR_RATE: f32 = 0.70;
const DECAY_RATE: f32 = 0.55;
const BODY_REACH: f32 = 0.72;
const PUSH_R: f32 = 0.36;

const FLAG_CLEAR_COLOR: [u32; 3] = [70, 180, 110];
const FLAG_CAUGHT_COLOR: [u32; 3] = [200, 64, 56];
const HIDDEN_PIP: [u8; 4] = [90, 70, 48, 255];

/// Per-guard patrol cursor. Not dump-visible; `from_doc` rebuilds it.
#[derive(Clone, Debug)]
struct GuardRt {
    /// Index of the waypoint currently being approached.
    index: usize,
    dir: i8,
    pause: f32,
}

/// Live stealth around a dump. Hidden/seen/clear stay here; `name` + flag
/// enable in the dump are the query source of truth. Alert / key / crouch
/// are session state (the key prop's `enabled` flag is the dump copy).
#[derive(Clone, Debug)]
pub struct StealthGame {
    pub hidden: bool,
    pub seen: bool,
    pub done: bool,
    pub clear: bool,
    /// 0 calm .. 1 caught. Fills from vision and noise when patrols exist.
    pub alert: f32,
    pub has_key: bool,
    pub crouching: bool,
    /// Noise radius emitted this frame (meters). 0 when still.
    pub noise: f32,
    cam_eye: [f32; 3],
    cam_target: [f32; 3],
    cam_fov: f32,
    guards: Vec<GuardRt>,
}

impl Default for StealthGame {
    fn default() -> Self {
        Self {
            hidden: false,
            seen: false,
            done: false,
            clear: false,
            alert: 0.0,
            has_key: false,
            crouching: false,
            noise: 0.0,
            cam_eye: [0.15, 5.1, 8.2],
            cam_target: [0.0, 0.9, -0.2],
            cam_fov: 48.0,
            guards: Vec::new(),
        }
    }
}

impl StealthGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let (cam_eye, cam_target, cam_fov) = doc
            .cameras
            .first()
            .map(|c| (c.position, c.target, c.fov))
            .unwrap_or(([0.15, 5.1, 8.2], [0.0, 0.9, -0.2], 48.0));
        let guards = doc
            .props
            .iter()
            .filter(|p| p.name == "guard")
            .map(|g| {
                let n = patrol_points(doc, &g.id).len();
                GuardRt {
                    index: if n > 1 { 1 } else { 0 },
                    dir: 1,
                    pause: 0.0,
                }
            })
            .collect();
        Self {
            cam_eye,
            cam_target,
            cam_fov,
            guards,
            ..Self::default()
        }
    }

    pub fn alert_state(&self) -> &'static str {
        if self.seen || self.alert >= 1.0 {
            "alert"
        } else if self.alert >= 0.18 {
            "suspicious"
        } else {
            "calm"
        }
    }
}

/// Crouch slows the wish. Other bits of input (including `dodge`) stay.
pub fn locomotion(input: WalkInput) -> WalkInput {
    if !input.dodge {
        return input;
    }
    WalkInput {
        lx: input.lx * CROUCH_SCALE,
        lz: input.lz * CROUCH_SCALE,
        ..input
    }
}

pub fn is_stealth(doc: &WorldDoc) -> bool {
    doc.genre_is(GAME_ID)
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

/// Sit capsules/boxes on the floor, keep the flag off, room camera.
pub fn seed(doc: &mut WorldDoc) {
    if !is_stealth(doc) {
        return;
    }
    sit_named(doc, NAME_PLAYER, false);
    sit_props(doc);
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = false;
        }
    }
    doc.coins = 0;
    place_room_camera(doc);
}

/// Keep an authored room camera. Insert the small-room default only when
/// the dump has none. Does not chase.
pub fn place_room_camera(doc: &mut WorldDoc) {
    if let Some(cam) = doc.cameras.first_mut() {
        if cam.name.is_empty() {
            cam.name = "room".into();
        }
        return;
    }
    let eye = [0.15, 5.1, 8.2];
    let target = [0.0, 0.9, -0.2];
    doc.cameras.push(crate::world_doc::WorldCamera {
        id: "camera:main".into(),
        kind: "camera".into(),
        name: "room".into(),
        position: eye,
        target,
        fov: 48.0,
    });
}

fn restore_room_camera(doc: &mut WorldDoc, game: &StealthGame) {
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = game.cam_eye;
        cam.target = game.cam_target;
        cam.fov = game.cam_fov;
        if cam.name.is_empty() {
            cam.name = "room".into();
        }
    } else {
        place_room_camera(doc);
    }
}

/// Hide / cone / exit tick. Caller already walked; room camera wins.
pub fn tick(doc: &mut WorldDoc, game: &mut StealthGame, input: WalkInput, dt: f32) {
    if game.done {
        write_beat(doc, game);
        restore_room_camera(doc, game);
        return;
    }
    game.crouching = input.dodge;
    game.noise = noise_radius(input);
    if patrol_mode(doc) {
        push_out_of_walls(doc);
        game.hidden = in_hide(doc);
        step_patrols(doc, game, dt);
        pickup_key(doc, game);
        step_alert(doc, game, dt);
        if !game.done && exit_ready(doc, game) {
            game.done = true;
            game.clear = true;
        }
    } else {
        game.hidden = in_hide(doc);
        if !game.hidden && in_guard_cone(doc) {
            game.seen = true;
            game.done = true;
            game.clear = false;
            game.alert = 1.0;
        } else if in_exit(doc) && !game.seen {
            game.done = true;
            game.clear = true;
        }
    }
    write_beat(doc, game);
    sync_cones(doc, game);
    restore_room_camera(doc, game);
}

fn patrol_mode(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "patrol")
}

fn noise_radius(input: WalkInput) -> f32 {
    let moving = input.lx * input.lx + input.lz * input.lz > 0.02;
    if !moving {
        0.0
    } else if input.dodge {
        CROUCH_NOISE
    } else {
        WALK_NOISE
    }
}

fn beat_name(game: &StealthGame) -> &'static str {
    if game.done {
        if game.clear {
            FLAG_CLEAR
        } else {
            FLAG_CAUGHT
        }
    } else if game.hidden {
        NAME_HIDDEN
    } else {
        NAME_PLAYER
    }
}

fn write_beat(doc: &mut WorldDoc, game: &StealthGame) {
    let crouch = game.crouching && patrol_mode(doc);
    sit_named(doc, beat_name(game), crouch);
    doc.coins = if game.done {
        if game.clear {
            10
        } else {
            11
        }
    } else if game.hidden {
        1
    } else {
        0
    };
    if game.done {
        set_flag_prop(doc, game.clear);
    }
}

fn set_flag_prop(doc: &mut WorldDoc, clear: bool) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = true;
            p.color = Some(if clear {
                FLAG_CLEAR_COLOR
            } else {
                FLAG_CAUGHT_COLOR
            });
        }
    }
}

fn sit_named(doc: &mut WorldDoc, name: &str, crouch: bool) {
    let Some(p) = player_ref(doc) else {
        return;
    };
    let mut walker = p.clone();
    let x = walker.position[0];
    let z = walker.position[2];
    let extra = if crouch { BODY_H * 0.62 } else { BODY_H };
    walker.kind = "walker".into();
    walker.name = name.into();
    walker.position = [x, doc.height_at(x, z) + extra, z];
    walker.face = walker.yaw;
    walker.on_ground = true;
    write_player(doc, walker);
}

fn prop_has_gltf(prop: &WorldProp) -> bool {
    prop.gltf
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty())
}

fn sit_props(doc: &mut WorldDoc) {
    let mut updates: Vec<(usize, f32)> = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        let extra = match p.name.as_str() {
            "guard" if prop_has_gltf(p) => 0.02,
            "guard" => BODY_H * p.scale[1].abs().max(0.6),
            "hide" | "exit" | "beam" | "flag" => p.scale[1].abs() * 0.5,
            _ => continue,
        };
        let y = doc.height_at(p.position[0], p.position[2]) + extra;
        updates.push((i, y));
    }
    for (i, y) in updates {
        if let Some(p) = doc.props.get_mut(i) {
            p.position[1] = y;
        }
    }
}

fn in_hide(doc: &WorldDoc) -> bool {
    let Some(p) = player_ref(doc) else {
        return false;
    };
    doc.props
        .iter()
        .any(|prop| prop.name == "hide" && prop.enabled && xz_in(p, prop, 0.2))
}

fn in_exit(doc: &WorldDoc) -> bool {
    let Some(p) = player_ref(doc) else {
        return false;
    };
    doc.props
        .iter()
        .any(|prop| prop.name == "exit" && prop.enabled && xz_in(p, prop, 0.35))
}

fn key_required(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "key")
}

fn pickup_key(doc: &mut WorldDoc, game: &mut StealthGame) {
    if game.has_key {
        return;
    }
    let Some(p) = player_ref(doc).cloned() else {
        return;
    };
    let hit = doc
        .props
        .iter()
        .any(|prop| prop.name == "key" && prop.enabled && xz_in(&p, prop, 0.5));
    if !hit {
        return;
    }
    game.has_key = true;
    for prop in &mut doc.props {
        if prop.name == "key" {
            prop.enabled = false;
        }
    }
}

fn exit_ready(doc: &WorldDoc, game: &StealthGame) -> bool {
    if game.seen || !in_exit(doc) {
        return false;
    }
    !key_required(doc) || game.has_key
}

fn xz_in(player: &WorldWalker, prop: &crate::world_doc::WorldProp, extra: f32) -> bool {
    let hx = prop.scale[0].abs() * 0.5 + extra;
    let hz = prop.scale[2].abs() * 0.5 + extra;
    (player.position[0] - prop.position[0]).abs() <= hx
        && (player.position[2] - prop.position[2]).abs() <= hz
}

fn in_guard_cone(doc: &WorldDoc) -> bool {
    let Some(player) = player_ref(doc) else {
        return false;
    };
    doc.props.iter().any(|g| {
        if g.name != "guard" || !g.enabled {
            return false;
        }
        let dx = player.position[0] - g.position[0];
        let dz = player.position[2] - g.position[2];
        let dist = (dx * dx + dz * dz).sqrt();
        if dist <= PLAYER_R + 0.35 {
            return true;
        }
        if dist > CONE_RANGE {
            return false;
        }
        let (s, c) = g.yaw.sin_cos();
        let ndx = dx / dist;
        let ndz = dz / dist;
        let cos = ndx * s + ndz * c;
        cos >= CONE_HALF.cos()
    })
}

fn patrol_points(doc: &WorldDoc, guard_id: &str) -> Vec<[f32; 2]> {
    let mut rows: Vec<(String, f32, f32)> = Vec::new();
    for p in &doc.props {
        if p.name == "patrol" && p.parent.as_deref() == Some(guard_id) {
            rows.push((p.id.clone(), p.position[0], p.position[2]));
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.into_iter().map(|(_, x, z)| [x, z]).collect()
}

fn scan_point(doc: &WorldDoc, guard_id: &str) -> Option<[f32; 2]> {
    doc.props.iter().find_map(|p| {
        if p.name == "scan" && p.parent.as_deref() == Some(guard_id) {
            Some([p.position[0], p.position[2]])
        } else {
            None
        }
    })
}

fn guard_ids(doc: &WorldDoc) -> Vec<(usize, String)> {
    doc.props
        .iter()
        .enumerate()
        .filter(|(_, p)| p.name == "guard" && p.enabled)
        .map(|(i, p)| (i, p.id.clone()))
        .collect()
}

fn step_patrols(doc: &mut WorldDoc, game: &mut StealthGame, dt: f32) {
    let guards = guard_ids(doc);
    if game.guards.len() != guards.len() {
        game.guards = guards
            .iter()
            .map(|(_, id)| {
                let n = patrol_points(doc, id).len();
                GuardRt {
                    index: if n > 1 { 1 } else { 0 },
                    dir: 1,
                    pause: 0.0,
                }
            })
            .collect();
    }
    for (gi, (idx, _)) in guards.iter().enumerate() {
        step_guard(doc, *idx, &mut game.guards[gi], dt);
    }
    face_aware_guards(doc, game);
}

fn step_guard(doc: &mut WorldDoc, idx: usize, rt: &mut GuardRt, dt: f32) {
    let Some(guard) = doc.props.get(idx) else {
        return;
    };
    let id = guard.id.clone();
    let points = patrol_points(doc, &id);
    if points.len() < 2 {
        return;
    }
    let target_i = rt.index % points.len();
    let target = points[target_i];
    let mut x = doc.props[idx].position[0];
    let mut z = doc.props[idx].position[2];
    let mut yaw = doc.props[idx].yaw;
    if rt.pause > 0.0 {
        rt.pause = (rt.pause - dt).max(0.0);
        if let Some(scan) = scan_point(doc, &id) {
            yaw = (scan[0] - x).atan2(scan[1] - z);
        }
        if rt.pause <= 0.0 {
            let n = points.len() as i32;
            let mut next = target_i as i32 + i32::from(rt.dir);
            if next < 0 || next >= n {
                rt.dir = -rt.dir;
                next = target_i as i32 + i32::from(rt.dir);
            }
            rt.index = next.clamp(0, n - 1) as usize;
        }
    } else {
        let dx = target[0] - x;
        let dz = target[1] - z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist > 1.0e-4 {
            yaw = dx.atan2(dz);
        }
        if dist <= GUARD_SPEED * dt + 0.05 {
            x = target[0];
            z = target[1];
            rt.pause = SCAN_TIME;
            rt.index = target_i;
        } else {
            let step = GUARD_SPEED * dt;
            x += dx / dist * step;
            z += dz / dist * step;
        }
    }
    let y = guard_feet_y(doc, idx, x, z);
    if let Some(p) = doc.props.get_mut(idx) {
        p.position = [x, y, z];
        p.yaw = yaw;
    }
}

fn guard_feet_y(doc: &WorldDoc, idx: usize, x: f32, z: f32) -> f32 {
    let Some(p) = doc.props.get(idx) else {
        return doc.height_at(x, z);
    };
    let extra = if prop_has_gltf(p) {
        0.02
    } else {
        BODY_H * p.scale[1].abs().max(0.6)
    };
    doc.height_at(x, z) + extra
}

fn face_aware_guards(doc: &mut WorldDoc, game: &StealthGame) {
    if game.hidden || game.alert < 0.18 {
        return;
    }
    let Some(player) = player_ref(doc).cloned() else {
        return;
    };
    let guards = guard_ids(doc);
    let mut turns: Vec<(usize, f32)> = Vec::new();
    for (idx, _) in guards {
        let Some(g) = doc.props.get(idx) else {
            continue;
        };
        let dx = player.position[0] - g.position[0];
        let dz = player.position[2] - g.position[2];
        let dist = (dx * dx + dz * dz).sqrt();
        let sees = guard_sees(doc, g, &player, game.crouching);
        let hears = hears(dist, game.noise, los_clear(doc, g, &player));
        if sees || hears {
            turns.push((idx, dx.atan2(dz)));
        }
    }
    for (idx, yaw) in turns {
        if let Some(p) = doc.props.get_mut(idx) {
            p.yaw = yaw;
        }
    }
}

fn los_clear(doc: &WorldDoc, guard: &WorldProp, player: &WorldWalker) -> bool {
    !segment_hits_occluder(
        doc,
        guard.position[0],
        guard.position[2],
        player.position[0],
        player.position[2],
    )
}

fn segment_hits_occluder(doc: &WorldDoc, ax: f32, az: f32, bx: f32, bz: f32) -> bool {
    doc.props.iter().any(|p| {
        if !p.enabled || !matches!(p.name.as_str(), "wall" | "hide" | "block") {
            return false;
        }
        let hx = p.scale[0].abs() * 0.5;
        let hz = p.scale[2].abs() * 0.5;
        segment_hits_rect(
            ax,
            az,
            bx,
            bz,
            [
                p.position[0] - hx,
                p.position[0] + hx,
                p.position[2] - hz,
                p.position[2] + hz,
            ],
        )
    })
}

fn segment_hits_rect(ax: f32, az: f32, bx: f32, bz: f32, rect: [f32; 4]) -> bool {
    let [minx, maxx, minz, maxz] = rect;
    let dx = bx - ax;
    let dz = bz - az;
    let mut t0 = 0.0;
    let mut t1 = 1.0;
    if !slab(ax, dx, minx, maxx, &mut t0, &mut t1) || !slab(az, dz, minz, maxz, &mut t0, &mut t1) {
        return false;
    }
    // A hit that only contains the player's endpoint still counts as cover
    // when the body is inside the volume. Grazes past the guard do not.
    t0 < 0.98 && t1 > 0.02
}

fn slab(p: f32, d: f32, min: f32, max: f32, t0: &mut f32, t1: &mut f32) -> bool {
    if d.abs() < 1.0e-8 {
        return p >= min && p <= max;
    }
    let inv = 1.0 / d;
    let (mut near, mut far) = ((min - p) * inv, (max - p) * inv);
    if near > far {
        std::mem::swap(&mut near, &mut far);
    }
    *t0 = t0.max(near);
    *t1 = t1.min(far);
    *t0 <= *t1
}

fn guard_sees(doc: &WorldDoc, guard: &WorldProp, player: &WorldWalker, crouch: bool) -> bool {
    if !los_clear(doc, guard, player) {
        return false;
    }
    let dx = player.position[0] - guard.position[0];
    let dz = player.position[2] - guard.position[2];
    let dist = (dx * dx + dz * dz).sqrt();
    if dist <= BODY_REACH {
        return true;
    }
    let range = CONE_RANGE * if crouch { CROUCH_SIGHT } else { 1.0 };
    if dist > range {
        return false;
    }
    let (s, c) = guard.yaw.sin_cos();
    let cos = dx / dist * s + dz / dist * c;
    let half = if crouch { CONE_HALF * 0.9 } else { CONE_HALF };
    cos >= half.cos()
}

fn hears(dist: f32, noise: f32, los: bool) -> bool {
    if noise <= 0.05 {
        return false;
    }
    if los && dist <= noise {
        return true;
    }
    dist <= noise * 0.35
}

fn step_alert(doc: &WorldDoc, game: &mut StealthGame, dt: f32) {
    let mut rate: f32 = 0.0;
    if !game.hidden {
        if let Some(player) = player_ref(doc).cloned() {
            for prop in &doc.props {
                if prop.name != "guard" || !prop.enabled {
                    continue;
                }
                let dx = player.position[0] - prop.position[0];
                let dz = player.position[2] - prop.position[2];
                let dist = (dx * dx + dz * dz).sqrt();
                if dist <= BODY_REACH {
                    game.alert = 1.0;
                    game.seen = true;
                    game.done = true;
                    game.clear = false;
                    return;
                }
                if guard_sees(doc, prop, &player, game.crouching) {
                    rate = rate.max(SEE_RATE);
                } else if hears(dist, game.noise, los_clear(doc, prop, &player)) {
                    rate = rate.max(HEAR_RATE);
                }
            }
        }
    }
    if rate > 0.0 {
        game.alert = (game.alert + rate * dt).min(1.05);
    } else {
        game.alert = (game.alert - DECAY_RATE * dt).max(0.0);
    }
    if game.alert >= 1.0 {
        game.seen = true;
        game.done = true;
        game.clear = false;
    }
}

fn push_out_of_walls(doc: &mut WorldDoc) {
    let Some(player) = player_ref(doc).cloned() else {
        return;
    };
    let mut x = player.position[0];
    let mut z = player.position[2];
    for _ in 0..3 {
        for prop in doc.props.iter().filter(|p| p.enabled && p.name == "wall") {
            let hx = prop.scale[0].abs() * 0.5 + PUSH_R;
            let hz = prop.scale[2].abs() * 0.5 + PUSH_R;
            let dx = x - prop.position[0];
            let dz = z - prop.position[2];
            if dx.abs() >= hx || dz.abs() >= hz {
                continue;
            }
            let px = hx - dx.abs();
            let pz = hz - dz.abs();
            if px < pz {
                x += if dx >= 0.0 { px } else { -px };
            } else {
                z += if dz >= 0.0 { pz } else { -pz };
            }
        }
    }
    if (x - player.position[0]).abs() < 1.0e-5 && (z - player.position[2]).abs() < 1.0e-5 {
        return;
    }
    let mut walker = player;
    walker.position[0] = x;
    walker.position[2] = z;
    write_player(doc, walker);
}

/// Wide end of the mesh sits `CONE_RANGE` along the guard's facing; the apex
/// (local +Y) points back at the guard. Pitch `+π/2` maps local +Y onto -Z
/// before yaw, which is -forward when yaw is 0.
pub fn vision_cone_pose(origin: [f32; 3], yaw: f32) -> ([f32; 3], f32, f32, [f32; 3]) {
    let (s, c) = yaw.sin_cos();
    let pos = [origin[0] + s * CONE_RANGE, 0.18, origin[2] + c * CONE_RANGE];
    let radius = CONE_RANGE * CONE_HALF.tan();
    let scale = [radius / 0.5, CONE_RANGE, 0.55];
    (pos, yaw, std::f32::consts::FRAC_PI_2, scale)
}

fn prop_xz(doc: &WorldDoc, name: &str) -> Option<(f32, f32)> {
    doc.props.iter().find_map(|p| {
        if p.name == name && p.enabled {
            Some((p.position[0], p.position[2]))
        } else {
            None
        }
    })
}

fn look_yaw_of(doc: &WorldDoc) -> f32 {
    let Some(cam) = doc.cameras.first() else {
        return 0.0;
    };
    let dx = cam.position[0] - cam.target[0];
    let dz = cam.position[2] - cam.target[2];
    (-dx).atan2(-dz)
}

fn stick_toward(doc: &WorldDoc, px: f32, pz: f32, tx: f32, tz: f32) -> (f32, f32) {
    let dx = tx - px;
    let dz = tz - pz;
    let dist = (dx * dx + dz * dz).sqrt();
    if dist < 0.22 {
        return (0.0, 0.0);
    }
    let dx = dx / dist;
    let dz = dz / dist;
    let (s, c) = look_yaw_of(doc).sin_cos();
    let lx = (dx * -c + dz * s).clamp(-1.0, 1.0);
    let lz = (dx * s + dz * c).clamp(-1.0, 1.0);
    (lx, lz)
}

/// Headless route for the yard fixture: crouch to the key, wait until the
/// hallway guard is walking away from the door, then crouch to the exit.
/// Interactive play does not call this.
pub fn demo_input(doc: &WorldDoc, game: &StealthGame) -> WalkInput {
    let Some(player) = player_ref(doc) else {
        return WalkInput {
            dodge: true,
            ..WalkInput::default()
        };
    };
    let px = player.position[0];
    let pz = player.position[2];
    let key = prop_xz(doc, "key").unwrap_or((-5.5, 1.6));
    let exit = prop_xz(doc, "exit").unwrap_or((-5.8, -5.5));
    let guard = doc.props.iter().find(|p| p.id == "prop:guard-a");
    let (gx, gyaw) = guard.map(|g| (g.position[0], g.yaw)).unwrap_or((0.0, 0.0));
    let safe = gx > 0.45 && gyaw > 0.8 && game.alert < 0.4;
    // Near the key until the hallway guard walks off to the right, then
    // cross the door. Once past the divider, commit to the exit.
    let target = if !game.has_key || (game.alert > 0.45 && pz > 0.6) {
        key
    } else if pz > 1.35 {
        if safe {
            (0.0, 1.15)
        } else {
            key
        }
    } else if pz > 0.45 {
        if safe {
            (0.0, -1.25)
        } else {
            (0.0, 1.2)
        }
    } else {
        exit
    };
    let (lx, lz) = stick_toward(doc, px, pz, target.0, target.1);
    WalkInput {
        lx,
        lz,
        jump: false,
        attack: false,
        dodge: true,
    }
}

fn cone_color(game: &StealthGame) -> [u32; 3] {
    if game.seen || game.alert >= 1.0 {
        [210, 48, 48]
    } else if game.alert >= 0.18 {
        [232, 120, 36]
    } else {
        [232, 186, 48]
    }
}

struct ConePose {
    guard_id: String,
    position: [f32; 3],
    yaw: f32,
    pitch: f32,
    scale: [f32; 3],
}

fn sync_cones(doc: &mut WorldDoc, game: &StealthGame) {
    let color = cone_color(game);
    let poses: Vec<ConePose> = doc
        .props
        .iter()
        .filter(|p| p.name == "guard" && p.enabled)
        .map(|g| {
            let (position, yaw, pitch, scale) = vision_cone_pose(g.position, g.yaw);
            ConePose {
                guard_id: g.id.clone(),
                position,
                yaw,
                pitch,
                scale,
            }
        })
        .collect();
    for pose in poses {
        if let Some(cone) = doc
            .props
            .iter_mut()
            .find(|p| p.name == "cone" && p.parent.as_deref() == Some(pose.guard_id.as_str()))
        {
            cone.position = pose.position;
            cone.yaw = pose.yaw;
            cone.pitch = pose.pitch;
            cone.scale = pose.scale;
            cone.color = Some(color);
            cone.model = "cone".into();
            cone.enabled = true;
        }
    }
}

pub fn build_hud(game: &StealthGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let mut quads = Vec::new();
    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [12, 16, 18, 160]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [22, 28, 32, 230],
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
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                22.0 * scale,
                22.0 * scale,
                if game.hidden {
                    HIDDEN_PIP
                } else {
                    [28, 24, 20, 160]
                },
            ));
            quads.push(Quad::new(
                44.0 * scale,
                16.0 * scale,
                22.0 * scale,
                22.0 * scale,
                if game.seen {
                    [200, 64, 56, 255]
                } else {
                    [36, 48, 40, 160]
                },
            ));
            let track_w = 140.0 * scale;
            quads.push(Quad::new(
                16.0 * scale,
                44.0 * scale,
                track_w,
                10.0 * scale,
                [24, 28, 32, 200],
            ));
            let fill = track_w * game.alert.clamp(0.0, 1.0);
            if fill > 1.0 {
                let col = if game.alert >= 1.0 {
                    [210, 48, 48, 255]
                } else if game.alert >= 0.18 {
                    [232, 120, 36, 255]
                } else {
                    [232, 186, 48, 255]
                };
                quads.push(Quad::new(
                    16.0 * scale,
                    44.0 * scale,
                    fill,
                    10.0 * scale,
                    col,
                ));
            }
        }
        GamePhase::Complete => {
            let clear = game.clear;
            quads.push(Quad::new(
                0.0,
                h * 0.22,
                w,
                h * 0.36,
                if clear {
                    [12, 28, 18, 210]
                } else {
                    [28, 12, 12, 210]
                },
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.62,
                w * 0.36,
                48.0 * scale,
                if clear {
                    [70, 180, 110, 240]
                } else {
                    [200, 64, 56, 240]
                },
            ));
        }
    }
    DrawList {
        clear: [28, 34, 36, 255],
        quads,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const ROOM: &str = include_str!("../tests/fixtures/stealth_hide_world.json");
    const YARD: &str = include_str!("../tests/fixtures/stealth_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const TOWN: &str = include_str!("../tests/fixtures/rpg_town_world.json");
    const PAGES: &str = include_str!("../tests/fixtures/novel_pages_world.json");
    const RING: &str = include_str!("../tests/fixtures/fight_hitstun_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(ROOM).unwrap();
        play.confirm();
        play
    }

    fn put_player(play: &mut WorldPlay, x: f32, z: f32) {
        let y = play.doc.height_at(x, z) + BODY_H;
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [x, y, z];
        }
        let walker = play.doc.player.clone().unwrap();
        write_player(&mut play.doc, walker);
    }

    #[test]
    fn dump_is_stealth_not_other_genres() {
        let doc = WorldDoc::from_json(ROOM).unwrap();
        assert!(is_stealth(&doc));
        assert_eq!(GAME_ID, "stealth_hide");
        assert!(!crate::novel::is_novel(&doc));
        assert!(!crate::rpg::is_rpg(&doc));
        assert!(!crate::fight::is_fight(&doc));
        assert!(!crate::action::is_action(&doc));
        assert!(!crate::fps::is_fps(&doc));
        assert!(!crate::td::is_td(&doc));
        assert!(!crate::race::is_race(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_stealth(&crest));
        let town = WorldDoc::from_json(TOWN).unwrap();
        assert!(!is_stealth(&town));
        let pages = WorldDoc::from_json(PAGES).unwrap();
        assert!(!is_stealth(&pages));
        let ring = WorldDoc::from_json(RING).unwrap();
        assert!(!is_stealth(&ring));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "hide" && p.model == "box"));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "guard" && p.model == "capsule"));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "exit" && p.model == "box"));
        assert!(doc.props.iter().any(|p| p.name == "flag" && !p.enabled));
        assert_eq!(doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("hide"));
        assert!(json.contains("guard"));
        assert!(json.contains("capsule"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 6,
            "hide box + guard must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(ROOM).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_stealth());
        assert!(!play.is_novel());
        assert!(!play.is_rpg());
        assert!(!play.is_fight());
        assert!(!play.is_collectathon());
        let start = play.doc.player.as_ref().unwrap().position;
        play.input.lx = 1.0;
        play.input.lz = 1.0;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.stealth.done, "title must not catch or clear");
        assert_eq!(play.doc.player.as_ref().unwrap().position, start);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "room");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn hide_volume_sets_hidden_dump_name() {
        let mut play = play_started();
        put_player(&mut play, -1.4, 0.5);
        play.tick(1.0 / 60.0);
        assert!(play.stealth.hidden, "crate is a hide volume");
        assert!(!play.stealth.seen);
        assert!(!play.stealth.done);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_HIDDEN);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("hidden"), "hidden must be dump-visible");
        assert_eq!(play.doc.coins, 1);
        assert_eq!(play.doc.cameras[0].name, "room");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "hidden pip overlay");
    }

    #[test]
    fn in_cone_not_hidden_is_caught() {
        let mut play = play_started();
        put_player(&mut play, 0.2, 0.8);
        play.tick(1.0 / 60.0);
        assert!(play.stealth.seen);
        assert!(play.stealth.done);
        assert!(!play.stealth.clear);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, FLAG_CAUGHT);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "caught flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_CAUGHT_COLOR));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("caught"));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "caught overlay");
    }

    #[test]
    fn hidden_then_exit_is_clear() {
        let mut play = play_started();
        put_player(&mut play, -1.4, 0.5);
        play.tick(1.0 / 60.0);
        assert!(play.stealth.hidden);
        assert!(!play.stealth.seen);
        put_player(&mut play, -3.4, -2.6);
        play.tick(1.0 / 60.0);
        assert!(play.stealth.done);
        assert!(play.stealth.clear);
        assert!(!play.stealth.seen);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, FLAG_CLEAR);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "clear flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_CLEAR_COLOR));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("clear"));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "clear overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.stealth.done);
        assert!(!play.stealth.seen);
        assert!(!play.stealth.hidden);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "retry restores flag off");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
    }

    #[test]
    fn other_genres_still_own_their_dumps() {
        let town = WorldPlay::from_json(TOWN).unwrap();
        assert!(town.is_rpg());
        assert!(!town.is_stealth());
        let novel = WorldPlay::from_json(PAGES).unwrap();
        assert!(novel.is_novel());
        assert!(!novel.is_stealth());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_stealth());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_stealth());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_stealth());
    }

    #[test]
    fn vision_cone_apex_sits_on_the_guard() {
        use glam::{EulerRot, Mat4, Quat, Vec3};
        for yaw in [0.0, std::f32::consts::FRAC_PI_2, -1.2] {
            let origin = [1.5, 0.9, -2.0];
            let (pos, cone_yaw, pitch, scale) = vision_cone_pose(origin, yaw);
            let q = Quat::from_euler(EulerRot::YXZ, cone_yaw, -pitch, 0.0);
            let m = Mat4::from_scale_rotation_translation(
                Vec3::from_array(scale),
                q,
                Vec3::from_array(pos),
            );
            let apex = m.transform_point3(Vec3::new(0.0, 1.0, 0.0));
            let (s, c) = yaw.sin_cos();
            assert!(
                (apex.x - origin[0]).abs() < 0.08 && (apex.z - origin[2]).abs() < 0.08,
                "apex {apex:?} yaw {yaw} should sit on the guard"
            );
            let base = m.transform_point3(Vec3::ZERO);
            let want_x = origin[0] + s * CONE_RANGE;
            let want_z = origin[2] + c * CONE_RANGE;
            assert!(
                (base.x - want_x).abs() < 0.08 && (base.z - want_z).abs() < 0.08,
                "base {base:?} should be the wide end along facing"
            );
        }
    }

    fn yard_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(YARD).unwrap();
        play.confirm();
        play
    }

    #[test]
    fn divider_blocks_sight_and_the_door_does_not() {
        let doc = WorldDoc::from_json(YARD).unwrap();
        assert!(segment_hits_occluder(&doc, 0.0, -3.6, -4.0, 2.0));
        assert!(!segment_hits_occluder(&doc, 0.0, -3.6, 0.0, 2.0));
    }

    #[test]
    fn yard_cone_fills_alert_and_a_bump_catches() {
        let mut play = yard_started();
        let guard = play
            .doc
            .props
            .iter()
            .find(|p| p.id == "prop:guard-a")
            .unwrap()
            .clone();
        put_player(&mut play, guard.position[0] + 2.4, guard.position[2]);
        for _ in 0..8 {
            play.tick(1.0 / 60.0);
        }
        assert!(
            play.stealth.alert > 0.05,
            "standing in the cone raises the meter, alert={}",
            play.stealth.alert
        );
        assert!(!play.stealth.done, "one short glance is not a catch");
        put_player(&mut play, guard.position[0] + 0.2, guard.position[2]);
        play.tick(1.0 / 60.0);
        assert!(play.stealth.seen && play.stealth.done && !play.stealth.clear);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.stealth.done);
        assert!(!play.stealth.has_key);
        assert!(play.doc.props.iter().any(|p| p.name == "key" && p.enabled));
    }

    #[test]
    fn yard_key_then_exit_clears_and_noise_hears() {
        let mut play = yard_started();
        put_player(&mut play, -5.5, 1.6);
        play.tick(1.0 / 60.0);
        assert!(play.stealth.has_key, "overlap picks up the key");
        assert!(play.doc.props.iter().any(|p| p.name == "key" && !p.enabled));
        put_player(&mut play, -5.8, -5.5);
        play.tick(1.0 / 60.0);
        assert!(play.stealth.clear && play.stealth.done);

        let mut loud = yard_started();
        let guard = loud
            .doc
            .props
            .iter()
            .find(|p| p.id == "prop:guard-b")
            .unwrap();
        let gx = guard.position[0];
        let gz = guard.position[2];
        for _ in 0..30 {
            put_player(&mut loud, gx - 2.3, gz);
            loud.input = WalkInput {
                lx: 1.0,
                lz: 0.0,
                jump: false,
                attack: false,
                dodge: false,
            };
            loud.tick(1.0 / 60.0);
        }
        assert!(
            loud.stealth.alert > 0.15,
            "walking beside a guard is noisy, alert={}",
            loud.stealth.alert
        );
        let loud_alert = loud.stealth.alert;
        let mut quiet = yard_started();
        for _ in 0..30 {
            put_player(&mut quiet, gx - 2.3, gz);
            quiet.input = WalkInput {
                lx: 1.0,
                lz: 0.0,
                jump: false,
                attack: false,
                dodge: true,
            };
            quiet.tick(1.0 / 60.0);
        }
        assert!(
            quiet.stealth.alert < loud_alert * 0.5,
            "crouch is quieter, quiet={} loud={loud_alert}",
            quiet.stealth.alert
        );
    }

    #[test]
    fn yard_demo_route_grabs_the_key_and_escapes() {
        let mut play = yard_started();
        let mut saw_key = false;
        for _ in 0..1800 {
            if play.stealth.clear {
                break;
            }
            if play.stealth.done && !play.stealth.clear {
                break;
            }
            let wish = demo_input(&play.doc, &play.stealth);
            if play.stealth.has_key {
                saw_key = true;
            }
            play.input = wish;
            play.tick(1.0 / 60.0);
        }
        let pos = play.doc.player.as_ref().unwrap().position;
        let guard = play
            .doc
            .props
            .iter()
            .find(|p| p.id == "prop:guard-a")
            .map(|g| (g.position[0], g.position[2], g.yaw));
        assert!(
            saw_key && play.stealth.clear && play.stealth.done,
            "demo route should clear, key={saw_key} clear={} alert={} state={} pos={pos:?} guard={guard:?}",
            play.stealth.clear,
            play.stealth.alert,
            play.stealth.alert_state(),
        );
        assert!(play.doc.props.iter().any(|p| p.name == "cone" && p.enabled));
        assert_eq!(play.game.phase, GamePhase::Complete);
    }
}
