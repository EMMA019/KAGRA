//! FPS on play_world: first-person eye camera, look, fire, hit.
//!
//! Sibling of collectathon / action. Hitscan (click or J) vs capsule / sprite
//! targets on a World.dump. Camera sits at capsule eye; the body stays in the
//! dump (local mesh hidden so the near plane does not clip a white interior).
//! Title -> play -> result reuses `WorldPlay` / `GamePhase`. Muzzle and hit
//! flash are `DrawList` quads on shared wgpu 30. No recoil, inventory, net,
//! Rapier, VRM skin, RendererV2, or new ECS.

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
pub const RANGE: f32 = 48.0;
pub const EYE_Y: f32 = 0.55;
pub const BODY_H: f32 = 0.95;
pub const CAPSULE_R: f32 = 0.50;

/// Live range around a dump. HP stays here; targets in the dump
/// (`name == "target"`) are the query/dump source of truth when killed.
#[derive(Clone, Debug)]
pub struct FpsGame {
    pub hits: u32,
    pub kills: u32,
    pub fire_t: f32,
    pub flash_t: f32,
    pub muzzle_t: f32,
    pub hurt_flash: bool,
    pub won: bool,
    target_hp: HashMap<String, u32>,
}

impl Default for FpsGame {
    fn default() -> Self {
        Self {
            hits: 0,
            kills: 0,
            fire_t: 0.0,
            flash_t: 0.0,
            muzzle_t: 0.0,
            hurt_flash: false,
            won: false,
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
            if is_target(p) && p.enabled {
                self.target_hp.insert(p.id.clone(), TARGET_HP);
            }
        }
    }
}

pub fn is_fps(doc: &WorldDoc) -> bool {
    doc.props.iter().any(is_target)
}

fn is_target(p: &WorldProp) -> bool {
    p.name == "target"
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
    if game.won {
        return;
    }
    game.fire_t = (game.fire_t - dt).max(0.0);
    game.flash_t = (game.flash_t - dt).max(0.0);
    game.muzzle_t = (game.muzzle_t - dt).max(0.0);
    if game.flash_t <= 0.0 && game.muzzle_t <= 0.0 {
        set_player_name(doc, "player");
        game.hurt_flash = false;
    }

    apply_fire(doc, game, input, look_yaw, look_pitch);

    if live_targets(doc) == 0 {
        game.won = true;
    }
}

fn live_targets(doc: &WorldDoc) -> usize {
    doc.props
        .iter()
        .filter(|p| is_target(p) && p.enabled)
        .count()
}

fn look_dir(yaw: f32, pitch: f32) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    Vec3::new(s * cp, sp, c * cp)
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

fn apply_fire(
    doc: &mut WorldDoc,
    game: &mut FpsGame,
    input: WalkInput,
    look_yaw: f32,
    look_pitch: f32,
) {
    if !(input.attack && game.fire_t <= 0.0) {
        return;
    }
    game.fire_t = FIRE_TIME;
    game.muzzle_t = MUZZLE_FLASH;
    set_player_name(doc, "fire");

    let Some(w) = player_ref(doc) else {
        return;
    };
    let origin = Vec3::new(w.position[0], w.position[1] + EYE_Y, w.position[2]);
    let dir = look_dir(look_yaw, look_pitch);
    let mut best: Option<(f32, String)> = None;
    for p in &doc.props {
        if !is_target(p) || !p.enabled {
            continue;
        }
        let center = Vec3::from_array(p.position);
        if let Some(t) = ray_hit_t(origin, dir, center, target_radius(p), target_half_h(p)) {
            if best.as_ref().map(|(bt, _)| t < *bt).unwrap_or(true) {
                best = Some((t, p.id.clone()));
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
        game.kills += 1;
    }
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
            for i in 0..game.kills.min(12) {
                quads.push(Quad::new(
                    pad + i as f32 * (pip + gap),
                    pad,
                    pip,
                    pip,
                    [240, 196, 72, 255],
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
            quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 16, 18, 210]));
            let bar = (game.kills.max(1) as f32 / 3.0).clamp(0.15, 1.0);
            quads.push(Quad::new(
                w * 0.22,
                h * 0.40,
                w * 0.56 * bar,
                18.0 * scale,
                [240, 196, 72, 255],
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.62,
                w * 0.36,
                48.0 * scale,
                [70, 160, 110, 240],
            ));
        }
    }

    DrawList {
        clear: [70, 86, 78, 255],
        quads,
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
}
