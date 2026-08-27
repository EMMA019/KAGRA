//! Tower defense on play_world: path, spawn, hit.
//!
//! Sibling of collectathon / action / fps. Creeps walk waypoint boxes on a
//! World.dump. One tower damages in range. Leak or clear is dump-visible
//! (`name` + `coins` count). Title -> play -> result reuses `WorldPlay` /
//! `GamePhase`. Capsules/boxes, not VRM. Overlay count on shared wgpu 30.
//! No player-placed towers, waves editor, Rapier, RendererV2, or new ECS.

use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};
use glam::Vec3;
use std::collections::HashMap;

pub const GAME_ID: &str = "td_lane";
pub const CREEP_HP: u32 = 3;
pub const CREEP_SPEED: f32 = 3.0;
pub const SPAWN_GAP: f32 = 1.25;
pub const TOWER_RANGE: f32 = 8.5;
pub const TOWER_COOLDOWN: f32 = 0.40;
pub const HIT_FLASH: f32 = 0.16;
pub const BODY_H: f32 = 0.95;

/// Live lane around a dump. HP / path t stay here; creeps and the tower in
/// the dump (`name == "creep"` / `"tower"`) are the query/dump source of truth.
#[derive(Clone, Debug)]
pub struct TdGame {
    pub hits: u32,
    pub kills: u32,
    pub leaks: u32,
    pub fire_t: f32,
    pub flash_t: f32,
    pub won: bool,
    pub done: bool,
    creep_hp: HashMap<String, u32>,
    creep_seg: HashMap<String, (usize, f32)>,
    spawned: HashMap<String, bool>,
    spawn_order: Vec<String>,
    spawn_t: f32,
}

impl Default for TdGame {
    fn default() -> Self {
        Self {
            hits: 0,
            kills: 0,
            leaks: 0,
            fire_t: 0.0,
            flash_t: 0.0,
            won: false,
            done: false,
            creep_hp: HashMap::new(),
            creep_seg: HashMap::new(),
            spawned: HashMap::new(),
            spawn_order: Vec::new(),
            spawn_t: 0.0,
        }
    }
}

impl TdGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let mut g = Self::default();
        g.rebind(doc);
        g
    }

    fn rebind(&mut self, doc: &WorldDoc) {
        self.creep_hp.clear();
        self.creep_seg.clear();
        self.spawned.clear();
        self.spawn_order.clear();
        self.spawn_t = 0.0;
        self.hits = 0;
        self.kills = 0;
        self.leaks = 0;
        self.fire_t = 0.0;
        self.flash_t = 0.0;
        self.won = false;
        self.done = false;
        let mut creeps: Vec<&WorldProp> = doc.props.iter().filter(|p| is_creep(p)).collect();
        creeps.sort_by(|a, b| a.id.cmp(&b.id));
        for p in creeps {
            self.creep_hp.insert(p.id.clone(), CREEP_HP);
            self.creep_seg.insert(p.id.clone(), (0, 0.0));
            self.spawned.insert(p.id.clone(), false);
            self.spawn_order.push(p.id.clone());
        }
    }
}

pub fn is_td(doc: &WorldDoc) -> bool {
    doc.props.iter().any(is_tower)
}

fn is_tower(p: &WorldProp) -> bool {
    p.name == "tower" || p.name == "fire"
}

fn is_waypoint(p: &WorldProp) -> bool {
    p.name == "waypoint"
}

fn is_creep(p: &WorldProp) -> bool {
    matches!(p.name.as_str(), "creep" | "hurt" | "dead" | "leaked") || p.id.contains("creep")
}

fn is_boxish(p: &WorldProp) -> bool {
    matches!(p.model.to_ascii_lowercase().as_str(), "box" | "cube")
}

fn sit_extra(p: &WorldProp) -> f32 {
    if is_boxish(p) {
        0.5 * p.scale[1].abs().max(0.08)
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

fn waypoints(doc: &WorldDoc) -> Vec<[f32; 3]> {
    let mut pts: Vec<&WorldProp> = doc.props.iter().filter(|p| is_waypoint(p)).collect();
    pts.sort_by(|a, b| a.id.cmp(&b.id));
    pts.iter().map(|p| p.position).collect()
}

fn live_creeps(doc: &WorldDoc) -> usize {
    doc.props
        .iter()
        .filter(|p| is_creep(p) && p.enabled && p.name != "leaked" && p.name != "dead")
        .count()
}

/// Sit path / tower / creeps on the floor. Does not spawn extra creeps.
pub fn seed(doc: &mut WorldDoc) {
    if !is_td(doc) {
        return;
    }
    let mut ys = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        if !(is_tower(p) || is_waypoint(p) || is_creep(p)) || !p.enabled {
            continue;
        }
        let y = doc.height_at(p.position[0], p.position[2]) + sit_extra(p);
        ys.push((i, y));
    }
    for (i, y) in ys {
        if let Some(p) = doc.props.get_mut(i) {
            p.position[1] = y;
        }
    }
    if let Some(p) = player_ref(doc) {
        let mut w = p.clone();
        let extra = BODY_H;
        w.position[1] = doc.height_at(w.position[0], w.position[2]) + extra;
        w.on_ground = true;
        write_player(doc, w);
    }
    doc.coins = 0;
    place_overview_camera(doc);
}

/// Fixed overview so the path and tower stay readable (not a chase cam).
pub fn place_overview_camera(doc: &mut WorldDoc) {
    let pts = waypoints(doc);
    let mut sum = Vec3::ZERO;
    let n = pts.len().max(1) as f32;
    for p in &pts {
        sum += Vec3::from_array(*p);
    }
    let target = if pts.is_empty() {
        Vec3::new(0.0, 0.4, 0.0)
    } else {
        sum / n + Vec3::new(0.0, 0.4, 0.0)
    };
    let eye = target + Vec3::new(0.0, 18.0, -16.0);
    let fov = doc.cameras.first().map(|c| c.fov).unwrap_or(50.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye.to_array();
        cam.target = target.to_array();
        cam.name = "overview".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "overview".into(),
            position: eye.to_array(),
            target: target.to_array(),
            fov,
        });
    }
}

/// Advance spawn, path walk, tower hit. Caller already stepped the walker.
pub fn tick(doc: &mut WorldDoc, game: &mut TdGame, dt: f32) {
    if game.done {
        return;
    }
    game.fire_t = (game.fire_t - dt).max(0.0);
    game.flash_t = (game.flash_t - dt).max(0.0);
    if game.flash_t <= 0.0 {
        if let Some(t) = doc.props.iter_mut().find(|p| is_tower(p)) {
            t.name = "tower".into();
        }
        for p in doc.props.iter_mut().filter(|p| p.name == "hurt") {
            p.name = "creep".into();
        }
    }

    spawn_creeps(game, dt);
    step_creeps(doc, game, dt);
    apply_tower(doc, game);
    doc.coins = game.leaks;
    finish_if_resolved(doc, game);
}

fn spawn_creeps(game: &mut TdGame, dt: f32) {
    game.spawn_t += dt;
    let mut i = 0;
    while i < game.spawn_order.len() {
        let id = game.spawn_order[i].clone();
        let due = i as f32 * SPAWN_GAP;
        if game.spawn_t + 1e-4 >= due {
            game.spawned.insert(id, true);
        }
        i += 1;
    }
}

fn step_creeps(doc: &mut WorldDoc, game: &mut TdGame, dt: f32) {
    let path = waypoints(doc);
    if path.len() < 2 {
        return;
    }
    let ids: Vec<String> = game.spawn_order.clone();
    for id in ids {
        if game.spawned.get(&id) != Some(&true) {
            continue;
        }
        let Some(prop) = doc.props.iter().find(|p| p.id == id) else {
            continue;
        };
        if !prop.enabled || prop.name == "leaked" || prop.name == "dead" {
            continue;
        }
        let (mut seg, mut t) = game.creep_seg.get(&id).copied().unwrap_or((0, 0.0));
        if seg + 1 >= path.len() {
            leak_creep(doc, game, &id);
            continue;
        }
        let a = Vec3::from_array(path[seg]);
        let b = Vec3::from_array(path[seg + 1]);
        let delta = Vec3::new(b.x - a.x, 0.0, b.z - a.z);
        let dist = delta.length().max(0.05);
        t += CREEP_SPEED * dt / dist;
        while t >= 1.0 && seg + 1 < path.len() {
            t -= 1.0;
            seg += 1;
            if seg + 1 >= path.len() {
                leak_creep(doc, game, &id);
                t = 1.0;
                break;
            }
        }
        if doc
            .props
            .iter()
            .any(|p| p.id == id && (p.name == "leaked" || !p.enabled))
        {
            continue;
        }
        let a = Vec3::from_array(path[seg.min(path.len() - 1)]);
        let b = Vec3::from_array(path[(seg + 1).min(path.len() - 1)]);
        let pos = a.lerp(b, t.clamp(0.0, 1.0));
        let yaw = (b.x - a.x).atan2(b.z - a.z);
        let extra = doc
            .props
            .iter()
            .find(|p| p.id == id)
            .map(sit_extra)
            .unwrap_or(BODY_H);
        let y = doc.height_at(pos.x, pos.z) + extra;
        if let Some(p) = doc.props.iter_mut().find(|p| p.id == id) {
            p.position[0] = pos.x;
            p.position[2] = pos.z;
            p.position[1] = y;
            p.yaw = yaw;
        }
        game.creep_seg.insert(id, (seg, t.clamp(0.0, 1.0)));
    }
}

fn leak_creep(doc: &mut WorldDoc, game: &mut TdGame, id: &str) {
    let already = doc.props.iter().any(|p| p.id == id && p.name == "leaked");
    if already {
        return;
    }
    if let Some(p) = doc.props.iter_mut().find(|p| p.id == id) {
        p.name = "leaked".into();
        p.enabled = true;
        if let Some(c) = p.color.as_mut() {
            *c = [196, 72, 54];
        }
    }
    game.leaks = game.leaks.saturating_add(1);
    set_player_name(doc, "leak");
    doc.coins = game.leaks;
}

fn apply_tower(doc: &mut WorldDoc, game: &mut TdGame) {
    if game.fire_t > 0.0 {
        return;
    }
    let Some(tower) = doc.props.iter().find(|p| is_tower(p) && p.enabled) else {
        return;
    };
    let origin = Vec3::from_array(tower.position);
    let mut best: Option<(f32, String)> = None;
    for p in &doc.props {
        if !is_creep(p) || !p.enabled || p.name == "leaked" || p.name == "dead" {
            continue;
        }
        if game.spawned.get(&p.id) != Some(&true) {
            continue;
        }
        let d = Vec3::new(p.position[0] - origin.x, 0.0, p.position[2] - origin.z).length();
        if d <= TOWER_RANGE && best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
            best = Some((d, p.id.clone()));
        }
    }
    let Some((_, id)) = best else {
        return;
    };
    game.fire_t = TOWER_COOLDOWN;
    game.flash_t = HIT_FLASH;
    game.hits = game.hits.saturating_add(1);
    if let Some(t) = doc.props.iter_mut().find(|p| is_tower(p)) {
        t.name = "fire".into();
    }
    let hp = game.creep_hp.entry(id.clone()).or_insert(CREEP_HP);
    *hp = hp.saturating_sub(1);
    if *hp == 0 {
        if let Some(p) = doc.props.iter_mut().find(|p| p.id == id) {
            p.name = "dead".into();
            p.enabled = false;
        }
        game.kills = game.kills.saturating_add(1);
    } else if let Some(p) = doc.props.iter_mut().find(|p| p.id == id) {
        p.name = "hurt".into();
    }
}

fn finish_if_resolved(doc: &mut WorldDoc, game: &mut TdGame) {
    if game.spawn_order.is_empty() {
        return;
    }
    let spawned_all = game
        .spawn_order
        .iter()
        .all(|id| game.spawned.get(id) == Some(&true));
    if !spawned_all {
        return;
    }
    if live_creeps(doc) > 0 {
        return;
    }
    game.done = true;
    if game.leaks == 0 {
        game.won = true;
        set_player_name(doc, "clear");
    } else {
        set_player_name(doc, "leak");
    }
    doc.coins = game.leaks;
}

pub fn build_hud(game: &TdGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let pad = 16.0 * scale;
    let mut quads = Vec::new();

    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [10, 12, 16, 150]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [18, 22, 28, 230],
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.58,
                w * 0.36,
                52.0 * scale,
                [72, 196, 168, 255],
            ));
        }
        GamePhase::Playing => {
            let pip = 18.0 * scale;
            let gap = 6.0 * scale;
            let live = game
                .spawn_order
                .len()
                .saturating_sub(game.kills as usize)
                .saturating_sub(game.leaks as usize);
            for i in 0..live.min(12) {
                quads.push(Quad::new(
                    pad + i as f32 * (pip + gap),
                    pad,
                    pip,
                    pip,
                    [72, 196, 168, 255],
                ));
            }
            for i in 0..game.leaks.min(8) {
                quads.push(Quad::new(
                    pad + i as f32 * (pip + gap),
                    pad + pip + gap,
                    pip,
                    pip,
                    [196, 72, 54, 255],
                ));
            }
            if game.flash_t > 0.0 {
                let a = (50.0 + 90.0 * (game.flash_t / HIT_FLASH)) as u8;
                quads.push(Quad::new(
                    w * 0.42,
                    h * 0.78,
                    w * 0.16,
                    22.0 * scale,
                    [255, 200, 80, a.min(160)],
                ));
            }
        }
        GamePhase::Complete => {
            quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 16, 18, 210]));
            let color = if game.won {
                [72, 196, 168, 255]
            } else {
                [196, 72, 54, 255]
            };
            let bar = if game.won {
                1.0
            } else {
                (game.leaks.max(1) as f32 / 3.0).clamp(0.2, 1.0)
            };
            quads.push(Quad::new(
                w * 0.22,
                h * 0.40,
                w * 0.56 * bar,
                18.0 * scale,
                color,
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.58,
                w * 0.36,
                40.0 * scale,
                [240, 196, 72, 255],
            ));
        }
    }

    DrawList {
        clear: [48, 52, 58, 255],
        quads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectathon::WalkInput;
    use crate::game::GamePhase;
    use crate::world_play::WorldPlay;

    const LANE: &str = include_str!("../tests/fixtures/td_lane_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const RANGE: &str = include_str!("../tests/fixtures/fps_range_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(LANE).unwrap();
        play.start();
        play
    }

    fn park_creep(play: &mut WorldPlay, id: &str, x: f32, z: f32) {
        let extra = play
            .doc
            .props
            .iter()
            .find(|p| p.id == id)
            .map(sit_extra)
            .unwrap_or(BODY_H);
        let y = play.doc.height_at(x, z) + extra;
        if let Some(p) = play.doc.props.iter_mut().find(|p| p.id == id) {
            p.position = [x, y, z];
            p.enabled = true;
            if p.name == "dead" || p.name == "leaked" {
                p.name = "creep".into();
            }
        }
        play.td.spawned.insert(id.into(), true);
        play.td.creep_seg.insert(id.into(), (0, 0.0));
        play.td.creep_hp.entry(id.into()).or_insert(CREEP_HP);
    }

    #[test]
    fn dump_is_td_not_fps_or_collectathon() {
        let doc = WorldDoc::from_json(LANE).unwrap();
        assert!(is_td(&doc));
        assert_eq!(GAME_ID, "td_lane");
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_td(&crest));
        let fps = WorldDoc::from_json(RANGE).unwrap();
        assert!(!is_td(&fps));
        let action = WorldDoc::from_json(ARENA).unwrap();
        assert!(!is_td(&action));
        let towers: Vec<_> = doc
            .props
            .iter()
            .filter(|p| is_tower(p) && p.enabled)
            .collect();
        assert_eq!(towers.len(), 1);
        assert!(is_boxish(towers[0]) || towers[0].model == "capsule");
        let wps: Vec<_> = doc.props.iter().filter(|p| is_waypoint(p)).collect();
        assert!(wps.len() >= 3, "need a path, got {}", wps.len());
        let creeps: Vec<_> = doc
            .props
            .iter()
            .filter(|p| is_creep(p) && p.enabled)
            .collect();
        assert!(
            creeps.len() >= 2,
            "need creeps in the dump, got {}",
            creeps.len()
        );
        assert!(creeps.iter().any(|p| p.model == "capsule"));
        assert_eq!(doc.player.as_ref().unwrap().name, "player");
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let slots: Vec<u32> = {
            let mut s: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
            s.sort();
            s
        };
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("tower"));
        assert!(json.contains("waypoint"));
        assert!(json.contains("creep"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 8,
            "path + tower + creeps must read, n={}",
            scene.instance_count()
        );
        assert!(scene.local_lights.iter().all(|l| l.intensity > 0.0));
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_spawn_until_confirm() {
        let mut play = WorldPlay::from_json(LANE).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_td());
        assert!(!play.is_fps());
        assert!(!play.is_action());
        assert!(!play.is_collectathon());
        let z0: Vec<_> = play
            .doc
            .props
            .iter()
            .filter(|p| is_creep(p))
            .map(|p| (p.id.clone(), p.position[2]))
            .collect();
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: true,
            dodge: false,
        };
        for _ in 0..40 {
            play.tick(1.0 / 60.0);
        }
        for (id, z) in &z0 {
            let p = play.doc.props.iter().find(|p| p.id == *id).unwrap();
            assert_eq!(p.position[2], *z, "title must not walk creeps");
        }
        assert_eq!(play.td.hits, 0);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "overview");
    }

    #[test]
    fn creeps_walk_the_path_after_start() {
        let mut play = play_started();
        let first = play
            .doc
            .props
            .iter()
            .filter(|p| is_creep(p) && p.enabled)
            .min_by(|a, b| a.id.cmp(&b.id))
            .unwrap();
        let id = first.id.clone();
        let start = first.position;
        for _ in 0..90 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.props.iter().find(|p| p.id == id).unwrap();
        let dx = p.position[0] - start[0];
        let dz = p.position[2] - start[2];
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(
            dist > 1.2,
            "creep should walk the path, dist={dist} start={start:?} now={:?}",
            p.position
        );
        assert_eq!(play.doc.cameras[0].name, "overview");
    }

    #[test]
    fn tower_damages_creep_and_hurt_is_in_dump() {
        let mut play = play_started();
        let tower = play
            .doc
            .props
            .iter()
            .find(|p| is_tower(p))
            .unwrap()
            .position;
        let id = play
            .doc
            .props
            .iter()
            .find(|p| is_creep(p) && p.enabled)
            .unwrap()
            .id
            .clone();
        park_creep(&mut play, &id, tower[0] + 1.2, tower[2]);
        play.td.fire_t = 0.0;
        play.tick(1.0 / 60.0);
        assert!(play.td.hits >= 1, "hits {}", play.td.hits);
        let dump = play.doc.to_json().unwrap();
        assert!(
            dump.contains("hurt") || dump.contains("dead") || dump.contains("fire"),
            "hit must be dump-visible"
        );
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "overlay count");
    }

    #[test]
    fn killing_all_creeps_clears_and_name_is_in_dump() {
        let mut play = play_started();
        let tower = play
            .doc
            .props
            .iter()
            .find(|p| is_tower(p))
            .unwrap()
            .position;
        let ids: Vec<_> = play
            .doc
            .props
            .iter()
            .filter(|p| is_creep(p))
            .map(|p| p.id.clone())
            .collect();
        assert!(ids.len() >= 2);
        for id in &ids {
            play.td.creep_hp.insert(id.clone(), 1);
            park_creep(&mut play, id, tower[0] + 1.0, tower[2] + 0.4);
        }
        play.td.spawn_t = SPAWN_GAP * ids.len() as f32;
        for id in &ids {
            play.td.spawned.insert(id.clone(), true);
        }
        for _ in 0..40 {
            play.td.fire_t = 0.0;
            play.tick(1.0 / 60.0);
            if play.td.done {
                break;
            }
        }
        assert!(
            play.td.won,
            "clear, kills={} leaks={}",
            play.td.kills, play.td.leaks
        );
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "clear");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("clear"), "clear must be dump-visible");
        assert_eq!(play.doc.coins, 0);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.td.won);
        assert_eq!(play.td.hits, 0);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "player");
        let live = play
            .doc
            .props
            .iter()
            .filter(|p| is_creep(p) && p.enabled && p.name != "leaked")
            .count();
        assert!(live >= 2, "retry must restore creeps, live={live}");
    }

    #[test]
    fn leak_at_path_end_sets_name_and_count() {
        let mut play = play_started();
        if let Some(t) = play.doc.props.iter_mut().find(|p| is_tower(p)) {
            t.enabled = false;
        }
        let path = waypoints(&play.doc);
        let end = *path.last().unwrap();
        let id = play
            .doc
            .props
            .iter()
            .find(|p| is_creep(p) && p.enabled)
            .unwrap()
            .id
            .clone();
        park_creep(&mut play, &id, end[0], end[2]);
        play.td.creep_seg.insert(id.clone(), (path.len() - 2, 0.95));
        play.td.spawned.insert(id.clone(), true);
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
            let p = play.doc.props.iter().find(|p| p.id == id).unwrap();
            if p.name == "leaked" {
                break;
            }
        }
        let p = play.doc.props.iter().find(|p| p.id == id).unwrap();
        assert_eq!(p.name, "leaked");
        assert!(play.td.leaks >= 1);
        assert_eq!(play.doc.coins, play.td.leaks);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "leak");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("leaked") || dump.contains("\"leak\""));
        assert!(dump.contains(&id));
    }

    #[test]
    fn crest_fps_action_still_own_their_dumps() {
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_td());
        let fps = WorldPlay::from_json(RANGE).unwrap();
        assert!(fps.is_fps());
        assert!(!fps.is_td());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_td());
    }

    #[test]
    fn overview_camera_stays_put_while_creeps_move() {
        let mut play = play_started();
        let cam0 = play.doc.cameras[0].position;
        play.input.lz = 1.0;
        for _ in 0..45 {
            play.tick(1.0 / 60.0);
        }
        let cam = play.doc.cameras[0].position;
        assert!(
            (cam[0] - cam0[0]).abs() < 0.05 && (cam[2] - cam0[2]).abs() < 0.05,
            "overview must not chase, cam={cam:?} was={cam0:?}"
        );
        assert_eq!(play.doc.cameras[0].name, "overview");
    }
}
