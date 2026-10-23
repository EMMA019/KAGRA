//! FPS on play_world: first-person eye camera, look, fire, hit, ammo, reload.
//!
//! Sibling of collectathon / action. Hitscan (click or J) vs capsule / sprite
//! targets and hostile walkers on a World.dump. Camera sits at capsule eye; the
//! body stays in the dump (local mesh hidden so the near plane does not clip a
//! white interior). Mag count is dump `coins`. Fire spends a round; empty click
//! is dump-visible `empty` and does not hitscan. R (`WalkInput.dodge`) reloads
//! after a short delay.
//!
//! Walkers named `enemy` chase and deal contact damage. `reserve` walkers are
//! the next wave (id `walker:enemy:<n>:<who>`); they rise when the current wave
//! is gone. Player HP lives here. Clearing every wave wins; HP 0 loses. Title
//! -> play -> result reuses `WorldPlay` / `GamePhase`. Retry restores the seed
//! dump. Muzzle and hit flash are `DrawList` quads on shared wgpu 30. No
//! recoil, inventory, net, Rapier, or RendererV2.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};
use glam::Vec3;
use std::collections::HashMap;

pub const GAME_ID: &str = "fps_range";
pub const TARGET_HP: u32 = 2;
pub const FIRE_TIME: f32 = 0.16;
pub const HIT_FLASH: f32 = 0.18;
pub const MUZZLE_FLASH: f32 = 0.10;
pub const MAG: u32 = 6;
pub const RELOAD_TIME: f32 = 0.70;
pub const RANGE: f32 = 48.0;
pub const EYE_Y: f32 = 0.55;
pub const BODY_H: f32 = 0.95;
pub const CAPSULE_R: f32 = 0.50;
pub const PLAYER_HP: u32 = 5;
pub const PLAYER_R: f32 = 0.46;
pub const ENEMY_R: f32 = 0.55;
pub const ENEMY_SPEED: f32 = 2.35;
pub const CONTACT_CD: f32 = 0.60;

/// Live range around a dump. HP stays here; targets in the dump
/// (`name == "target"`) are the query/dump source of truth when killed.
#[derive(Clone, Debug)]
pub struct FpsGame {
    pub hits: u32,
    pub kills: u32,
    pub ammo: u32,
    pub fire_t: f32,
    pub flash_t: f32,
    pub muzzle_t: f32,
    pub reload_t: f32,
    pub hurt_flash: bool,
    pub won: bool,
    pub lost: bool,
    pub hp: u32,
    pub wave: u32,
    pub contact_cd: f32,
    target_hp: HashMap<String, u32>,
}

impl Default for FpsGame {
    fn default() -> Self {
        Self {
            hits: 0,
            kills: 0,
            ammo: MAG,
            fire_t: 0.0,
            flash_t: 0.0,
            muzzle_t: 0.0,
            reload_t: 0.0,
            hurt_flash: false,
            won: false,
            lost: false,
            hp: PLAYER_HP,
            wave: 1,
            contact_cd: 0.0,
            target_hp: HashMap::new(),
        }
    }
}

impl FpsGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let mut g = Self::default();
        g.rebind_targets(doc);
        g
    }

    fn rebind_targets(&mut self, doc: &WorldDoc) {
        self.target_hp.clear();
        for p in &doc.props {
            if is_shootable(p) && p.enabled {
                self.target_hp.insert(p.id.clone(), TARGET_HP);
            }
        }
        for w in &doc.walkers {
            if is_enemy_walker(w) || is_reserve_walker(w) {
                self.target_hp.insert(w.id.clone(), TARGET_HP);
            }
        }
    }
}

/// Highest `walker:enemy:<n>:` index in the dump. Range targets with no
/// hostile walkers report 1.
pub fn wave_count(doc: &WorldDoc) -> u32 {
    let mut max_wave = 0u32;
    for w in &doc.walkers {
        if is_enemy_walker(w) || is_reserve_walker(w) || w.name == "down" {
            max_wave = max_wave.max(wave_index(&w.id));
        }
    }
    max_wave.max(1)
}

pub fn is_fps(doc: &WorldDoc) -> bool {
    doc.genre_is(GAME_ID)
}

fn is_target(p: &WorldProp) -> bool {
    p.name == "target"
}

fn is_shootable(p: &WorldProp) -> bool {
    is_target(p) || p.name == "enemy"
}

fn is_enemy_walker(w: &WorldWalker) -> bool {
    w.name == "enemy"
}

fn is_reserve_walker(w: &WorldWalker) -> bool {
    w.name == "reserve"
}

fn walker_spec(w: &WorldWalker) -> &str {
    w.gltf
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(w.model.as_str())
}

/// Feet on the floor. Same rest-pose offset the walker sit uses.
fn stand_y(doc: &WorldDoc, x: f32, z: f32, spec: &str) -> f32 {
    let ground = doc.height_at(x, z);
    let spec = spec.trim();
    if spec.is_empty() {
        ground + BODY_H
    } else {
        ground - crate::world_doc::walker_rest_min_y(spec)
    }
}

/// `walker:enemy:2:mage` -> 2. Anything else is wave 1.
fn wave_index(id: &str) -> u32 {
    let mut parts = id.split(':');
    let _kind = parts.next();
    let _role = parts.next();
    parts
        .next()
        .and_then(|n| n.parse::<u32>().ok())
        .filter(|n| *n >= 1)
        .unwrap_or(1)
}

fn is_sprite_target(p: &WorldProp) -> bool {
    matches!(p.model.to_ascii_lowercase().as_str(), "sprite" | "quad")
}

fn is_box_target(p: &WorldProp) -> bool {
    let m = p.model.to_ascii_lowercase();
    m == "box" || m == "cube"
}

fn target_radius(p: &WorldProp) -> f32 {
    if is_sprite_target(p) {
        0.5 * p.scale[0].abs().max(0.35)
    } else if is_box_target(p) {
        0.5 * p.scale[0].abs().max(p.scale[2].abs()).max(0.4)
    } else {
        CAPSULE_R * p.scale[0].abs().max(0.4)
    }
}

fn target_half_h(p: &WorldProp) -> f32 {
    if is_sprite_target(p) || is_box_target(p) {
        0.5 * p.scale[1].abs().max(0.4)
    } else {
        BODY_H * p.scale[1].abs().max(0.6)
    }
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

/// Sit targets on the floor. Does not spawn extra targets (dump is source of truth).
pub fn seed(doc: &mut WorldDoc) {
    if !is_fps(doc) {
        return;
    }
    doc.coins = MAG;
    let mut ys = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        if !is_target(p) || !p.enabled {
            continue;
        }
        let extra = if is_sprite_target(p) || is_box_target(p) {
            0.5 * p.scale[1].abs().max(0.4)
        } else {
            BODY_H
        };
        let y = doc.height_at(p.position[0], p.position[2]) + extra;
        ys.push((i, y));
    }
    for (i, y) in ys {
        if let Some(p) = doc.props.get_mut(i) {
            p.position[1] = y;
        }
    }
    // Active wave stands on the floor. Later waves wait under the arena so
    // they are not in the hitscan until `advance_waves` raises them.
    let seated: Vec<(usize, f32)> = doc
        .walkers
        .iter()
        .enumerate()
        .filter_map(|(i, w)| {
            if is_enemy_walker(w) {
                let spec = walker_spec(w).to_string();
                let y = stand_y(doc, w.position[0], w.position[2], &spec);
                Some((i, y))
            } else {
                None
            }
        })
        .collect();
    for w in &mut doc.walkers {
        if is_reserve_walker(w) {
            w.position[1] = -40.0;
        }
    }
    for (i, y) in seated {
        if let Some(w) = doc.walkers.get_mut(i) {
            w.position[1] = y;
            w.on_ground = true;
        }
    }
}

/// Hitscan fire. Caller already stepped the walker and placed the eye camera.
pub fn tick(
    doc: &mut WorldDoc,
    game: &mut FpsGame,
    input: WalkInput,
    look_yaw: f32,
    look_pitch: f32,
    dt: f32,
) {
    if game.won || game.lost {
        return;
    }
    game.fire_t = (game.fire_t - dt).max(0.0);
    game.flash_t = (game.flash_t - dt).max(0.0);
    game.muzzle_t = (game.muzzle_t - dt).max(0.0);
    game.contact_cd = (game.contact_cd - dt).max(0.0);
    let was_reloading = game.reload_t > 0.0;
    game.reload_t = (game.reload_t - dt).max(0.0);
    if was_reloading && game.reload_t <= 0.0 {
        game.ammo = MAG;
    }
    if game.reload_t > 0.0 {
        set_player_name(doc, "reload");
    } else if game.flash_t <= 0.0 && game.muzzle_t <= 0.0 {
        set_player_name(doc, if game.ammo == 0 { "empty" } else { "player" });
        game.hurt_flash = false;
    }

    apply_reload(doc, game, input);
    apply_fire(doc, game, input, look_yaw, look_pitch);
    doc.coins = game.ammo;
    advance_waves(doc, game);
    chase_enemies(doc, game, dt);
    apply_contact(doc, game);

    if arena_clear(doc) {
        game.won = true;
    } else if game.hp == 0 {
        game.lost = true;
        game.hp = 0;
        set_player_name(doc, "dead");
    }
}

fn live_shootables(doc: &WorldDoc) -> usize {
    let props = doc
        .props
        .iter()
        .filter(|p| is_shootable(p) && p.enabled)
        .count();
    let walkers = doc.walkers.iter().filter(|w| is_enemy_walker(w)).count();
    props + walkers
}

fn arena_clear(doc: &WorldDoc) -> bool {
    live_shootables(doc) == 0 && !doc.walkers.iter().any(is_reserve_walker)
}

fn look_dir(yaw: f32, pitch: f32) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    Vec3::new(s * cp, sp, c * cp)
}

/// Keep the gameplay capsule at eye height. A KayKit rig whose rest pose
/// starts at the feet would otherwise sit the camera on the ankles.
pub fn pin_player_capsule(doc: &mut WorldDoc) {
    let Some(mut w) = player_ref(doc).cloned() else {
        return;
    };
    let y = doc.height_at(w.position[0], w.position[2]) + BODY_H;
    if (w.position[1] - y).abs() < 0.001 && w.on_ground {
        return;
    }
    w.position[1] = y;
    w.on_ground = true;
    write_player(doc, w);
}

/// Sit the dump camera at capsule eye. Body stays in WorldDoc.
/// `name == "eye"` is the draw cue to skip the local mesh (no white interior).
pub fn place_eye_camera(doc: &mut WorldDoc, look_yaw: f32, look_pitch: f32) {
    let Some(w) = player_ref(doc) else {
        return;
    };
    let origin = Vec3::new(w.position[0], w.position[1] + EYE_Y, w.position[2]);
    let dir = look_dir(look_yaw, look_pitch);
    let target = origin + dir * 8.0;
    let fov = doc.cameras.first().map(|c| c.fov).unwrap_or(54.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = origin.to_array();
        cam.target = target.to_array();
        cam.name = "eye".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "eye".into(),
            position: origin.to_array(),
            target: target.to_array(),
            fov,
            roll: 0.0,
        });
    }
}

/// Ray vs vertical capsule (XZ circle + Y slab). `dir` should be unit length.
fn ray_hit_t(origin: Vec3, dir: Vec3, center: Vec3, radius: f32, half_h: f32) -> Option<f32> {
    let ox = origin.x - center.x;
    let oz = origin.z - center.z;
    let a = dir.x * dir.x + dir.z * dir.z;
    let y_lo = center.y - half_h;
    let y_hi = center.y + half_h;
    if a < 1e-8 {
        if ox * ox + oz * oz > radius * radius {
            return None;
        }
        if dir.y.abs() < 1e-8 {
            return None;
        }
        let t0 = (y_lo - origin.y) / dir.y;
        let t1 = (y_hi - origin.y) / dir.y;
        let t = t0.min(t1);
        if t > 0.02 && t <= RANGE {
            return Some(t);
        }
        return None;
    }
    let b = 2.0 * (ox * dir.x + oz * dir.z);
    let c = ox * ox + oz * oz - radius * radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let sqrt = disc.sqrt();
    let mut t = (-b - sqrt) / (2.0 * a);
    if t <= 0.02 {
        t = (-b + sqrt) / (2.0 * a);
    }
    if t <= 0.02 || t > RANGE {
        return None;
    }
    let y = origin.y + dir.y * t;
    if y < y_lo - 0.02 || y > y_hi + 0.02 {
        return None;
    }
    Some(t)
}

fn apply_reload(doc: &mut WorldDoc, game: &mut FpsGame, input: WalkInput) {
    if !input.dodge || game.reload_t > 0.0 || game.ammo >= MAG {
        return;
    }
    game.reload_t = RELOAD_TIME;
    set_player_name(doc, "reload");
}

fn apply_fire(
    doc: &mut WorldDoc,
    game: &mut FpsGame,
    input: WalkInput,
    look_yaw: f32,
    look_pitch: f32,
) {
    if !(input.attack && game.fire_t <= 0.0 && game.reload_t <= 0.0) {
        return;
    }
    if game.ammo == 0 {
        game.fire_t = FIRE_TIME;
        set_player_name(doc, "empty");
        return;
    }
    game.ammo = game.ammo.saturating_sub(1);
    game.fire_t = FIRE_TIME;
    game.muzzle_t = MUZZLE_FLASH;
    set_player_name(doc, "fire");
    crate::mixer::cue_sfx("fire");

    let Some(w) = player_ref(doc) else {
        return;
    };
    let origin = Vec3::new(w.position[0], w.position[1] + EYE_Y, w.position[2]);
    let dir = look_dir(look_yaw, look_pitch);
    let player_id = player_ref(doc).map(|w| w.id.clone());
    let mut best: Option<(f32, String)> = None;
    for p in &doc.props {
        if !is_shootable(p) || !p.enabled {
            continue;
        }
        let center = Vec3::from_array(p.position);
        if let Some(t) = ray_hit_t(origin, dir, center, target_radius(p), target_half_h(p)) {
            if best.as_ref().map(|(bt, _)| t < *bt).unwrap_or(true) {
                best = Some((t, p.id.clone()));
            }
        }
    }
    for w in &doc.walkers {
        if !is_enemy_walker(w) || player_id.as_deref() == Some(w.id.as_str()) {
            continue;
        }
        let ground = doc.height_at(w.position[0], w.position[2]);
        let center = Vec3::new(w.position[0], ground + BODY_H, w.position[2]);
        if let Some(t) = ray_hit_t(origin, dir, center, ENEMY_R, BODY_H) {
            if best.as_ref().map(|(bt, _)| t < *bt).unwrap_or(true) {
                best = Some((t, w.id.clone()));
            }
        }
    }
    let Some((_, id)) = best else {
        return;
    };
    let hp = game.target_hp.entry(id.clone()).or_insert(TARGET_HP);
    *hp = hp.saturating_sub(1);
    game.hits += 1;
    game.flash_t = HIT_FLASH;
    game.hurt_flash = true;
    set_player_name(doc, "hurt");
    if *hp == 0 {
        if let Some(p) = doc.props.iter_mut().find(|p| p.id == id) {
            p.enabled = false;
        }
        if let Some(w) = doc.walkers.iter_mut().find(|w| w.id == id) {
            w.name = "down".into();
            w.position[1] = -40.0;
            w.anim = "idle".into();
        }
        game.kills += 1;
    }
}

fn advance_waves(doc: &mut WorldDoc, game: &mut FpsGame) {
    let wave_alive = doc
        .walkers
        .iter()
        .any(|w| is_enemy_walker(w) && wave_index(&w.id) == game.wave);
    if wave_alive {
        return;
    }
    let tracked = doc
        .walkers
        .iter()
        .any(|w| is_enemy_walker(w) || is_reserve_walker(w) || w.name == "down");
    if !tracked {
        return;
    }
    let next = game.wave + 1;
    let rising: Vec<(usize, f32)> = doc
        .walkers
        .iter()
        .enumerate()
        .filter_map(|(i, w)| {
            if is_reserve_walker(w) && wave_index(&w.id) == next {
                let spec = walker_spec(w).to_string();
                let y = stand_y(doc, w.position[0], w.position[2], &spec);
                Some((i, y))
            } else {
                None
            }
        })
        .collect();
    if rising.is_empty() {
        return;
    }
    for (i, y) in rising {
        if let Some(w) = doc.walkers.get_mut(i) {
            w.name = "enemy".into();
            w.position[1] = y;
            w.on_ground = true;
        }
    }
    game.wave = next;
    let ids: Vec<String> = doc
        .walkers
        .iter()
        .filter(|w| is_enemy_walker(w))
        .map(|w| w.id.clone())
        .collect();
    for id in ids {
        game.target_hp.entry(id).or_insert(TARGET_HP);
    }
}

fn chase_enemies(doc: &mut WorldDoc, game: &FpsGame, dt: f32) {
    if game.lost || game.won {
        return;
    }
    let Some(player) = player_ref(doc) else {
        return;
    };
    let px = player.position[0];
    let pz = player.position[2];
    let player_id = player.id.clone();
    let half = doc.half.max(4.0);
    // (index, x, z, yaw, spec). Empty spec keeps the current feet height.
    let mut plans: Vec<(usize, f32, f32, f32, String)> = Vec::new();
    for (i, w) in doc.walkers.iter().enumerate() {
        if !is_enemy_walker(w) || w.id == player_id {
            continue;
        }
        let dx = px - w.position[0];
        let dz = pz - w.position[2];
        let dist = (dx * dx + dz * dz).sqrt();
        let stop = ENEMY_R + PLAYER_R + 0.08;
        let yaw = dx.atan2(dz);
        if dist <= stop + 0.02 {
            plans.push((i, w.position[0], w.position[2], yaw, String::new()));
            continue;
        }
        let nx = dx / dist.max(1e-4);
        let nz = dz / dist.max(1e-4);
        let x = (w.position[0] + nx * ENEMY_SPEED * dt).clamp(-half + 1.0, half - 1.0);
        let z = (w.position[2] + nz * ENEMY_SPEED * dt).clamp(-half + 1.0, half - 1.0);
        plans.push((i, x, z, yaw, walker_spec(w).to_string()));
    }
    let mut moves: Vec<(usize, f32, f32, f32, f32)> = Vec::new();
    for (i, x, z, yaw, spec) in plans {
        let y = if spec.is_empty() {
            doc.walkers.get(i).map(|w| w.position[1]).unwrap_or(0.0)
        } else {
            stand_y(doc, x, z, &spec)
        };
        moves.push((i, x, y, z, yaw));
    }
    separate_moves(&mut moves);
    for (i, x, y, z, yaw) in moves {
        if let Some(w) = doc.walkers.get_mut(i) {
            w.position = [x, y, z];
            w.yaw = yaw;
            w.face = yaw;
            w.on_ground = true;
            w.anim = "walk".into();
        }
    }
}

fn separate_moves(moves: &mut [(usize, f32, f32, f32, f32)]) {
    let min_d = ENEMY_R * 2.0;
    for i in 0..moves.len() {
        for j in (i + 1)..moves.len() {
            let dx = moves[j].1 - moves[i].1;
            let dz = moves[j].3 - moves[i].3;
            let dist = (dx * dx + dz * dz).sqrt();
            if dist >= min_d {
                continue;
            }
            let (nx, nz) = if dist < 1e-4 {
                (1.0, 0.0)
            } else {
                (dx / dist, dz / dist)
            };
            let push = (min_d - dist) * 0.5;
            moves[i].1 -= nx * push;
            moves[i].3 -= nz * push;
            moves[j].1 += nx * push;
            moves[j].3 += nz * push;
        }
    }
}

fn apply_contact(doc: &mut WorldDoc, game: &mut FpsGame) {
    if game.hp == 0 || game.contact_cd > 0.0 || game.lost || game.won {
        return;
    }
    let Some(player) = player_ref(doc) else {
        return;
    };
    let px = player.position[0];
    let pz = player.position[2];
    let player_id = player.id.clone();
    let hit = doc.walkers.iter().any(|w| {
        if !is_enemy_walker(w) || w.id == player_id {
            return false;
        }
        let dx = px - w.position[0];
        let dz = pz - w.position[2];
        let r = ENEMY_R + PLAYER_R;
        dx * dx + dz * dz <= r * r
    });
    if !hit {
        return;
    }
    game.hp = game.hp.saturating_sub(1);
    game.contact_cd = CONTACT_CD;
    game.flash_t = HIT_FLASH;
    game.hurt_flash = true;
    set_player_name(doc, "hurt");
}

pub fn build_hud(game: &FpsGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let pad = 16.0 * scale;
    let mut quads = Vec::new();

    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [8, 10, 14, 150]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [16, 20, 28, 230],
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
            for i in 0..PLAYER_HP {
                let filled = i < game.hp;
                quads.push(Quad::new(
                    pad + i as f32 * (pip + gap),
                    pad,
                    pip,
                    pip * 0.72,
                    if filled {
                        [220, 64, 72, 255]
                    } else {
                        [40, 24, 24, 170]
                    },
                ));
            }
            for i in 0..game.kills.min(12) {
                quads.push(Quad::new(
                    pad + i as f32 * (pip + gap),
                    pad + pip + 6.0 * scale,
                    pip * 0.62,
                    pip * 0.62,
                    [240, 196, 72, 255],
                ));
            }
            // Mag pips: dump `coins` is the same count.
            let mag_w = 8.0 * scale;
            let mag_h = 16.0 * scale;
            for i in 0..MAG {
                let filled = i < game.ammo;
                quads.push(Quad::new(
                    pad + i as f32 * (mag_w + gap),
                    h - pad - mag_h,
                    mag_w,
                    mag_h,
                    if filled {
                        [240, 220, 90, 255]
                    } else {
                        [36, 32, 24, 180]
                    },
                ));
            }
            if game.reload_t > 0.0 {
                let frac = (game.reload_t / RELOAD_TIME).clamp(0.0, 1.0);
                quads.push(Quad::new(
                    pad,
                    h - pad - mag_h - 8.0 * scale,
                    (mag_w + gap) * MAG as f32 * frac,
                    4.0 * scale,
                    [180, 180, 190, 220],
                ));
            }
            // Crosshair: look is already mouse/arrows; fire lands on this overlay.
            let cx = w * 0.5;
            let cy = h * 0.5;
            let arm = 10.0 * scale;
            let thick = 2.0 * scale;
            quads.push(Quad::new(
                cx - arm,
                cy - thick * 0.5,
                arm * 2.0,
                thick,
                [240, 240, 240, 220],
            ));
            quads.push(Quad::new(
                cx - thick * 0.5,
                cy - arm,
                thick,
                arm * 2.0,
                [240, 240, 240, 220],
            ));
            if game.muzzle_t > 0.0 {
                let a = (80.0 + 140.0 * (game.muzzle_t / MUZZLE_FLASH)) as u8;
                quads.push(Quad::new(
                    w * 0.42,
                    h * 0.78,
                    w * 0.16,
                    28.0 * scale,
                    [255, 220, 90, a.min(180)],
                ));
            }
            if game.flash_t > 0.0 {
                let a = (70.0 + 120.0 * (game.flash_t / HIT_FLASH)) as u8;
                quads.push(Quad::new(0.0, 0.0, w, h, [255, 80, 60, a.min(110)]));
            }
        }
        GamePhase::Complete => {
            let win = !game.lost;
            quads.push(Quad::new(
                0.0,
                h * 0.22,
                w,
                h * 0.36,
                if win {
                    [12, 16, 18, 210]
                } else {
                    [28, 10, 12, 220]
                },
            ));
            let bar = (game.kills.max(1) as f32 / 3.0).clamp(0.15, 1.0);
            quads.push(Quad::new(
                w * 0.22,
                h * 0.40,
                w * 0.56 * bar,
                18.0 * scale,
                if win {
                    [240, 196, 72, 255]
                } else {
                    [180, 64, 64, 255]
                },
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.62,
                w * 0.36,
                48.0 * scale,
                if win {
                    [70, 160, 110, 240]
                } else {
                    [170, 64, 64, 240]
                },
            ));
        }
    }

    DrawList {
        clear: [70, 86, 78, 255],
        quads,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const RANGE_JSON: &str = include_str!("../tests/fixtures/fps_range_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(RANGE_JSON).unwrap();
        play.start();
        play
    }

    fn put_player(play: &mut WorldPlay, x: f32, z: f32, yaw: f32) {
        let y = play.doc.height_at(x, z) + BODY_H;
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [x, y, z];
            p.yaw = yaw;
            p.face = yaw;
            p.on_ground = true;
            p.name = "player".into();
        }
        let w = play.doc.player.clone().unwrap();
        write_player(&mut play.doc, w);
        play.look_yaw = yaw;
        play.look_pitch = 0.0;
    }

    #[test]
    fn dump_is_fps_not_action_or_collectathon() {
        let doc = WorldDoc::from_json(RANGE_JSON).unwrap();
        assert!(is_fps(&doc));
        assert_eq!(GAME_ID, "fps_range");
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_fps(&crest));
        let action = WorldDoc::from_json(ARENA).unwrap();
        assert!(!is_fps(&action));
        let targets: Vec<_> = doc
            .props
            .iter()
            .filter(|p| is_target(p) && p.enabled)
            .collect();
        assert!(
            targets.len() >= 2,
            "need capsule + sprite target, got {}",
            targets.len()
        );
        assert!(targets.iter().any(|p| p.model == "capsule"));
        assert!(targets.iter().any(|p| is_sprite_target(p)));
        assert_eq!(doc.player.as_ref().unwrap().name, "player");
        assert!(doc.player.as_ref().unwrap().on_ground);
        let json = doc.to_json().unwrap();
        assert!(json.contains("target"));
        assert!(json.contains("walker:player"));
        let play = WorldPlay::from_json(RANGE_JSON).unwrap();
        assert_eq!(play.doc.coins, MAG, "mag must be dump-visible");
        assert_eq!(play.fps.ammo, MAG);
    }

    #[test]
    fn title_blocks_fire_until_confirm() {
        let mut play = WorldPlay::from_json(RANGE_JSON).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_fps());
        assert!(!play.is_action());
        assert!(!play.is_collectathon());
        let z = play.doc.player.as_ref().unwrap().position[2];
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: true,
            dodge: false,
        };
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert_eq!(play.doc.player.as_ref().unwrap().position[2], z);
        assert_eq!(play.fps.hits, 0);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.fps.kills, 0);
    }

    #[test]
    fn fire_hits_capsule_and_hurt_is_in_dump() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 0.0, 0.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.fps.hits >= 1, "hits {}", play.fps.hits);
        assert!(play.fps.flash_t > 0.0);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "hurt");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("hurt"), "hit must be dump-visible");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 3, "crosshair + muzzle/hit overlay");
    }

    #[test]
    fn two_hits_kill_target_and_dump_disables_it() {
        let mut play = play_started();
        let id = play
            .doc
            .props
            .iter()
            .find(|p| is_target(p) && p.model == "capsule" && p.enabled)
            .unwrap()
            .id
            .clone();
        put_player(&mut play, 0.0, 0.0, 0.0);
        for _ in 0..TARGET_HP {
            play.input.attack = true;
            play.tick(1.0 / 60.0);
            for _ in 0..20 {
                play.input.attack = false;
                play.tick(1.0 / 60.0);
            }
        }
        let t = play.doc.props.iter().find(|p| p.id == id).unwrap();
        assert!(!t.enabled, "killed target must be disabled in the dump");
        assert!(play.fps.kills >= 1);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains(&id));
    }

    #[test]
    fn look_away_misses_then_look_hits() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 0.0, 1.2);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.fps.hits, 0, "yaw 1.2 should miss the +Z capsule");
        assert_eq!(play.doc.player.as_ref().unwrap().name, "fire");
        for _ in 0..20 {
            play.input.attack = false;
            play.tick(1.0 / 60.0);
        }
        play.look_yaw = 0.0;
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(
            play.fps.hits >= 1,
            "yaw 0 should hit, hits {}",
            play.fps.hits
        );
    }

    #[test]
    fn clearing_targets_finishes_and_retry_restores_dump() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 0.0, 0.0);
        let ids: Vec<_> = play
            .doc
            .props
            .iter()
            .filter(|p| is_target(p) && p.enabled)
            .map(|p| (p.id.clone(), p.position[0], p.position[2]))
            .collect();
        assert!(ids.len() >= 2);
        for (id, x, z) in ids {
            let yaw = x.atan2(z);
            put_player(&mut play, 0.0, 0.0, yaw);
            for _ in 0..TARGET_HP {
                play.input.attack = true;
                play.tick(1.0 / 60.0);
                for _ in 0..16 {
                    play.input.attack = false;
                    play.tick(1.0 / 60.0);
                }
            }
            let t = play.doc.props.iter().find(|p| p.id == id).unwrap();
            assert!(!t.enabled, "{id} still live");
        }
        assert!(play.fps.won);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert!(play.game.score > 0);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.fps.won);
        assert_eq!(play.fps.hits, 0);
        assert_eq!(play.fps.ammo, MAG);
        assert_eq!(play.doc.coins, MAG);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "player");
        let live = play
            .doc
            .props
            .iter()
            .filter(|p| is_target(p) && p.enabled)
            .count();
        assert!(live >= 2, "retry must restore targets, live={live}");
    }

    #[test]
    fn crest_collectathon_and_action_still_own_their_dumps() {
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_fps());
        assert_eq!(crest.game.phase, GamePhase::Title);
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_fps());
    }

    #[test]
    fn fps_camera_is_eye_not_chase_and_tracks_body() {
        let chase = WorldDoc::from_json(RANGE_JSON).unwrap();
        let n_chase = chase.compile_scene(16.0 / 9.0).instance_count();
        let mut play = WorldPlay::from_json(RANGE_JSON).unwrap();
        assert!(play.is_fps());
        let p = play.doc.player.as_ref().unwrap().position;
        let cam = play.doc.cameras[0].position;
        assert!(
            (cam[0] - p[0]).abs() < 0.02 && (cam[2] - p[2]).abs() < 0.02,
            "eye xz must sit on the body, cam={cam:?} body={p:?}"
        );
        assert!(
            (cam[1] - (p[1] + EYE_Y)).abs() < 0.02,
            "eye y {} vs body {} + EYE_Y",
            cam[1],
            p[1]
        );
        assert_eq!(play.doc.cameras[0].name, "eye");
        assert_eq!(play.doc.player.as_ref().unwrap().id, "walker:player");
        assert_eq!(play.doc.player.as_ref().unwrap().name, "player");
        let n_eye = play.doc.compile_scene(16.0 / 9.0).instance_count();
        assert!(
            n_eye < n_chase,
            "hide local body/head so we do not clip, chase={n_chase} eye={n_eye}"
        );
        assert!(
            n_eye >= 3,
            "floor + targets must remain (not a white void), n_eye={n_eye}"
        );

        play.start();
        play.add_look(0.0, 0.4);
        play.tick(1.0 / 60.0);
        let cam = &play.doc.cameras[0];
        let p = play.doc.player.as_ref().unwrap().position;
        assert!((cam.position[0] - p[0]).abs() < 0.02);
        assert!((cam.position[2] - p[2]).abs() < 0.02);
        assert!(
            cam.target[1] > cam.position[1] + 0.2,
            "pitch up should raise look target, tgt={} eye={}",
            cam.target[1],
            cam.position[1]
        );

        play.input.lz = 1.0;
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap().position;
        let cam = play.doc.cameras[0].position;
        assert!(
            (cam[0] - p[0]).abs() < 0.02 && (cam[2] - p[2]).abs() < 0.02,
            "camera must not lose the body after walk, cam={cam:?} body={p:?}"
        );
        assert!(
            p[2] > 0.4,
            "WASD forward should move the dump body, z={}",
            p[2]
        );
        assert_eq!(play.doc.cameras[0].name, "eye");
    }

    #[test]
    fn fire_from_eye_still_hits_after_look() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 0.0, 0.0);
        play.add_look(0.0, 0.05);
        play.tick(1.0 / 60.0);
        let cam = play.doc.cameras[0].position;
        let p = play.doc.player.as_ref().unwrap().position;
        assert!((cam[1] - (p[1] + EYE_Y)).abs() < 0.02);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(
            play.fps.hits >= 1,
            "hitscan from eye forward, hits {}",
            play.fps.hits
        );
    }

    #[test]
    fn fire_spends_a_round_in_the_dump() {
        let mut play = play_started();
        assert_eq!(play.doc.coins, MAG);
        assert_eq!(play.fps.ammo, MAG);
        put_player(&mut play, 0.0, 0.0, 0.0);
        crate::mixer::reset_mixer();
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(crate::mixer::with_mixer(|m| m
            .log()
            .iter()
            .any(|s| s == "fire")));
        assert_eq!(play.fps.ammo, MAG - 1);
        assert_eq!(play.doc.coins, MAG - 1, "ammo must be dump-visible coins");
        assert!(play.fps.hits >= 1);
        let hud = play.build_hud(960, 540);
        assert!(
            hud.quads.len() as u32 >= MAG + 2,
            "crosshair + mag pips, n={}",
            hud.quads.len()
        );
    }

    #[test]
    fn empty_click_does_not_kill() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 0.0, 0.0);
        for _ in 0..MAG {
            play.input.attack = true;
            play.tick(1.0 / 60.0);
            for _ in 0..20 {
                play.input.attack = false;
                play.tick(1.0 / 60.0);
            }
        }
        assert_eq!(play.fps.ammo, 0);
        assert_eq!(play.doc.coins, 0);
        let sprite_id = play
            .doc
            .props
            .iter()
            .find(|p| is_target(p) && is_sprite_target(p) && p.enabled)
            .expect("sprite still live after draining at the +Z capsule")
            .id
            .clone();
        let (x, z) = {
            let p = play.doc.props.iter().find(|p| p.id == sprite_id).unwrap();
            (p.position[0], p.position[2])
        };
        put_player(&mut play, 0.0, 0.0, x.atan2(z));
        let hits = play.fps.hits;
        let kills = play.fps.kills;
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.fps.ammo, 0);
        assert_eq!(play.fps.hits, hits, "empty click must not hitscan");
        assert_eq!(play.fps.kills, kills);
        assert!(
            play.doc
                .props
                .iter()
                .find(|p| p.id == sprite_id)
                .unwrap()
                .enabled,
            "empty click must not kill"
        );
        assert_eq!(play.doc.player.as_ref().unwrap().name, "empty");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("empty"), "empty must be dump-visible");
        assert_eq!(play.doc.coins, 0);
    }

    #[test]
    fn reload_r_restores_ammo() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 0.0, 0.0);
        play.fps.ammo = 0;
        play.doc.coins = 0;
        play.input.dodge = true;
        play.tick(1.0 / 60.0);
        assert!(play.fps.reload_t > 0.0);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "reload");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("reload"), "reload must be dump-visible");
        assert_eq!(play.fps.ammo, 0, "mag fills when the delay ends");
        let steps = (RELOAD_TIME * 60.0).ceil() as u32 + 2;
        for _ in 0..steps {
            play.input.dodge = false;
            play.tick(1.0 / 60.0);
        }
        assert!(play.fps.reload_t <= 0.0);
        assert_eq!(play.fps.ammo, MAG);
        assert_eq!(play.doc.coins, MAG);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.fps.ammo, MAG - 1);
        assert!(
            play.fps.hits >= 1,
            "after reload, fire must hitscan, hits {}",
            play.fps.hits
        );
    }

    const ARENA_JSON: &str = include_str!("../tests/fixtures/fps_world.json");

    fn arena_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(ARENA_JSON).unwrap();
        play.start();
        play
    }

    fn nearest_enemy(play: &WorldPlay) -> Option<(f32, f32, f32)> {
        let p = play.doc.player.as_ref()?.position;
        let mut best: Option<(f32, f32, f32)> = None;
        for w in &play.doc.walkers {
            if w.name != "enemy" {
                continue;
            }
            let d = (w.position[0] - p[0]).powi(2) + (w.position[2] - p[2]).powi(2);
            if best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
                best = Some((d, w.position[0], w.position[2]));
            }
        }
        best
    }

    fn aim_and_hold(play: &mut WorldPlay, attack: bool, dodge: bool, lz: f32) {
        if let Some((_, x, z)) = nearest_enemy(play) {
            let p = play.doc.player.as_ref().unwrap().position;
            let desired = (x - p[0]).atan2(z - p[2]);
            let err = crate::vehicle::wrap_angle(desired - play.look_yaw);
            play.add_look(err, -play.look_pitch);
        }
        play.input.lx = 0.0;
        play.input.lz = lz;
        play.input.attack = attack;
        play.input.dodge = dodge;
        play.input.jump = false;
    }

    #[test]
    fn arena_enemies_chase_and_waves_stay_buried() {
        let mut play = arena_started();
        assert_eq!(play.fps.hp, PLAYER_HP);
        assert_eq!(play.fps.wave, 1);
        assert_eq!(wave_count(&play.doc), 3);
        let reserves: Vec<_> = play
            .doc
            .walkers
            .iter()
            .filter(|w| w.name == "reserve")
            .collect();
        assert!(reserves.len() >= 4, "later waves {}", reserves.len());
        assert!(reserves.iter().all(|w| w.position[1] < -10.0));
        let z0 = play
            .doc
            .walkers
            .iter()
            .find(|w| w.id == "walker:enemy:1:rogue")
            .unwrap()
            .position[2];
        for _ in 0..90 {
            aim_and_hold(&mut play, false, false, 0.0);
            play.tick(1.0 / 60.0);
        }
        let z1 = play
            .doc
            .walkers
            .iter()
            .find(|w| w.id == "walker:enemy:1:rogue")
            .unwrap()
            .position[2];
        assert!(
            z1 < z0 - 1.0,
            "enemy should close on the player, z0={z0} z1={z1}"
        );
        assert!(play.fps.hp <= PLAYER_HP);
        assert!(!play.fps.lost);
        assert_eq!(play.game.phase, GamePhase::Playing);
    }

    #[test]
    fn contact_drains_hp_and_retry_restores_waves() {
        let mut play = arena_started();
        let (x, z) = {
            let w = play.doc.walkers.iter().find(|w| w.name == "enemy").unwrap();
            (w.position[0], w.position[2])
        };
        put_player(&mut play, x, z, 0.0);
        for _ in 0..400 {
            if play.fps.lost {
                break;
            }
            play.input = WalkInput::default();
            play.tick(1.0 / 60.0);
        }
        assert!(play.fps.lost, "hp {}", play.fps.hp);
        assert_eq!(play.fps.hp, 0);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "dead");
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.fps.lost);
        assert!(!play.fps.won);
        assert_eq!(play.fps.hp, PLAYER_HP);
        assert_eq!(play.fps.ammo, MAG);
        assert_eq!(play.fps.wave, 1);
        let live = play
            .doc
            .walkers
            .iter()
            .filter(|w| w.name == "enemy")
            .count();
        let buried = play
            .doc
            .walkers
            .iter()
            .filter(|w| w.name == "reserve")
            .count();
        assert_eq!(live, 2, "wave 1 returns");
        assert!(buried >= 4, "later waves buried again, n={buried}");
    }

    #[test]
    fn clearing_wave_one_raises_the_next() {
        let mut play = arena_started();
        for _ in 0..900 {
            let enemies = play
                .doc
                .walkers
                .iter()
                .filter(|w| w.name == "enemy")
                .count();
            if enemies == 0 || play.fps.wave >= 2 || play.fps.lost {
                break;
            }
            let dist = nearest_enemy(&play)
                .map(|(d, _, _)| d.sqrt())
                .unwrap_or(99.0);
            let lz = if dist < 3.4 { -1.0 } else { 0.0 };
            let dodge = play.fps.ammo == 0;
            let attack = play.fps.ammo > 0 && play.fps.reload_t <= 0.0;
            aim_and_hold(&mut play, attack, dodge, lz);
            play.tick(1.0 / 60.0);
        }
        assert!(!play.fps.lost, "hp {}", play.fps.hp);
        assert!(play.fps.wave >= 2, "wave {}", play.fps.wave);
        assert!(
            play.doc
                .walkers
                .iter()
                .any(|w| w.name == "enemy" && w.id.contains(":2:")),
            "wave 2 should be standing"
        );
        assert!(
            play.doc
                .walkers
                .iter()
                .any(|w| w.name == "reserve" && w.position[1] < -10.0),
            "wave 3 stays buried"
        );
    }
}
