//! Tower defense on play_world: path, spawn, hit, place.
//!
//! Sibling of collectathon / action / fps. Creeps walk waypoint boxes on a
//! World.dump. Towers damage in range. Player places extra towers with J /
//! click (`WalkInput.attack`) on a `slot` (not puzzle `pad`). Placement
//! spends `coins` (cost + remaining dump-visible). Leak or clear is
//! dump-visible (`name`). Title -> play -> result reuses `WorldPlay` /
//! `GamePhase`. Capsules/boxes, not VRM. Overlay count on shared wgpu 30.
//! No waves editor, Rapier, RendererV2, or new ECS.
//!
//! A dump whose creep ids look like `prop:creep-w3-1` is wave mode: five
//! (or however many) waves, three tower types, gold, and lives. Tap or
//! mouse (`pointer_play`) places the selected type on a slot. The plain
//! `td_lane` fixture stays one wave, one box tower, and the old coin cost.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};
use crate::scene3d::Camera;
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
pub const START: u32 = 10;
pub const COST: u32 = 5;
pub const PLACE_REACH: f32 = 2.2;
/// Lives on a wave-mode map. A leak spends one. Zero lives loses immediately.
pub const WAVE_LIVES: u32 = 15;
/// Starting gold when creep ids carry a wave tag. The plain lane stays [`START`].
pub const WAVE_GOLD: u32 = 80;
/// Pause between waves, seconds.
pub const WAVE_GAP: f32 = 2.0;
/// Gold added when a wave-mode tower kills a creep.
pub const KILL_GOLD: u32 = 2;
/// Ground pick radius around a slot (tap / mouse).
pub const SLOT_PICK: f32 = 1.85;

struct TowerKind {
    name: &'static str,
    flash: &'static str,
    cost: u32,
    range: f32,
    damage: u32,
    cooldown: f32,
    gltf: &'static str,
    model: &'static str,
    scale: [f32; 3],
    color: [u32; 3],
}

/// Arrow / cannon / mage. Bundled building aliases, not bare boxes.
const KINDS: [TowerKind; 3] = [
    TowerKind {
        name: "tower",
        flash: "fire",
        cost: 8,
        range: 9.0,
        damage: 1,
        cooldown: 0.35,
        gltf: "gazebo",
        model: "gazebo",
        scale: [0.55, 0.55, 0.55],
        color: [214, 198, 156],
    },
    TowerKind {
        name: "cannon",
        flash: "cannon_fire",
        cost: 18,
        range: 7.5,
        damage: 2,
        cooldown: 0.70,
        gltf: "mill",
        model: "mill",
        scale: [0.38, 0.38, 0.38],
        color: [176, 148, 112],
    },
    TowerKind {
        name: "mage",
        flash: "mage_fire",
        cost: 28,
        range: 12.0,
        damage: 1,
        cooldown: 0.22,
        gltf: "tower",
        model: "tower",
        scale: [0.32, 0.32, 0.32],
        color: [128, 156, 214],
    },
];

/// Live lane around a dump. HP / path t stay here; creeps, towers, slots,
/// and `coins` in the dump are the query/dump source of truth.
#[derive(Clone, Debug)]
pub struct TdGame {
    pub hits: u32,
    pub kills: u32,
    pub leaks: u32,
    pub coins: u32,
    pub fire_t: f32,
    pub flash_t: f32,
    pub won: bool,
    pub done: bool,
    /// Remaining leaks before a wave-mode loss. 0 on the plain lane.
    pub lives: u32,
    /// 0-based index into [`TdGame::waves`]. Plain lane stays 0.
    pub wave_index: usize,
    pub wave_gap: f32,
    /// 0 arrow, 1 cannon, 2 mage. Attack and tap both use this in wave mode.
    pub selected: u8,
    pub wave_mode: bool,
    waves: Vec<Vec<String>>,
    tower_cd: HashMap<String, f32>,
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
            coins: 0,
            fire_t: 0.0,
            flash_t: 0.0,
            won: false,
            done: false,
            lives: 0,
            wave_index: 0,
            wave_gap: 0.0,
            selected: 0,
            wave_mode: false,
            waves: Vec::new(),
            tower_cd: HashMap::new(),
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
        self.wave_mode = wave_mode(doc);
        self.lives = if self.wave_mode { WAVE_LIVES } else { 0 };
        self.wave_index = 0;
        self.wave_gap = 0.0;
        self.selected = 0;
        self.waves.clear();
        self.tower_cd.clear();
        self.coins = starting_coins(doc);
        let mut creeps: Vec<&WorldProp> = doc.props.iter().filter(|p| is_creep(p)).collect();
        creeps.sort_by(|a, b| a.id.cmp(&b.id));
        let mut grouped: Vec<(u32, String)> = Vec::new();
        for p in creeps {
            self.creep_hp.insert(p.id.clone(), CREEP_HP);
            self.creep_seg.insert(p.id.clone(), (0, 0.0));
            self.spawned.insert(p.id.clone(), false);
            if self.wave_mode {
                let wave = creep_wave(&p.id).unwrap_or(1);
                grouped.push((wave, p.id.clone()));
            } else {
                self.spawn_order.push(p.id.clone());
            }
        }
        if self.wave_mode {
            let max_w = grouped.iter().map(|(w, _)| *w).max().unwrap_or(1);
            self.waves = vec![Vec::new(); max_w as usize];
            for (w, id) in grouped {
                let idx = (w.saturating_sub(1) as usize).min(self.waves.len().saturating_sub(1));
                self.waves[idx].push(id);
            }
            for wave in &mut self.waves {
                wave.sort();
            }
            if let Some(idx) = self.waves.iter().position(|w| !w.is_empty()) {
                self.wave_index = idx;
                self.spawn_order = self.waves[idx].clone();
            }
        }
    }

    /// 1-based wave shown on the HUD. Plain lane is wave 1 of 1.
    pub fn wave_number(&self) -> u32 {
        if self.wave_mode {
            self.wave_index as u32 + 1
        } else {
            1
        }
    }

    pub fn wave_count(&self) -> u32 {
        if self.wave_mode {
            self.waves.len().max(1) as u32
        } else {
            1
        }
    }

    pub fn select(&mut self, kind: u8) {
        let last = KINDS.len().saturating_sub(1) as u8;
        self.selected = kind.min(last);
    }
}

pub fn is_td(doc: &WorldDoc) -> bool {
    doc.genre_is(GAME_ID)
}

/// Creep id contains `-wN` (`prop:creep-w2-0`). Plain lane ids do not.
pub fn wave_mode(doc: &WorldDoc) -> bool {
    doc.props
        .iter()
        .any(|p| is_creep(p) && creep_wave(&p.id).is_some())
}

pub fn starting_coins(doc: &WorldDoc) -> u32 {
    if wave_mode(doc) {
        WAVE_GOLD
    } else {
        START
    }
}

fn creep_wave(id: &str) -> Option<u32> {
    let bytes = id.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b'-' && bytes[i + 1].eq_ignore_ascii_case(&b'w') {
            let rest = &id[i + 2..];
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<u32>() {
                if n >= 1 {
                    return Some(n);
                }
            }
        }
        i += 1;
    }
    None
}

fn is_tower(p: &WorldProp) -> bool {
    matches!(
        p.name.as_str(),
        "tower" | "fire" | "cannon" | "cannon_fire" | "mage" | "mage_fire"
    )
}

fn kind_of(p: &WorldProp) -> usize {
    match p.name.as_str() {
        "cannon" | "cannon_fire" => 1,
        "mage" | "mage_fire" => 2,
        _ => 0,
    }
}

fn rested_name(name: &str) -> Option<&'static str> {
    match name {
        "fire" => Some("tower"),
        "cannon_fire" => Some("cannon"),
        "mage_fire" => Some("mage"),
        "hurt" => Some("creep"),
        _ => None,
    }
}

fn is_slot(p: &WorldProp) -> bool {
    p.name == "slot"
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
    } else if p.gltf.as_deref().is_some_and(|g| !g.is_empty()) {
        // Bundled meshes sit on their feet. Capsules keep the body offset.
        0.0
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
        if !(is_tower(p) || is_waypoint(p) || is_creep(p) || is_slot(p)) || !p.enabled {
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
    if wave_mode(doc) {
        for p in doc.props.iter_mut() {
            if is_creep(p) {
                p.enabled = false;
            }
        }
    }
    doc.coins = starting_coins(doc);
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
            roll: 0.0,
        });
    }
}

/// Advance spawn, path walk, tower hit, place. Caller already stepped the walker.
pub fn tick(doc: &mut WorldDoc, game: &mut TdGame, input: WalkInput, dt: f32) {
    if game.done {
        doc.coins = game.coins;
        return;
    }
    game.fire_t = (game.fire_t - dt).max(0.0);
    game.flash_t = (game.flash_t - dt).max(0.0);
    if game.wave_mode {
        for cd in game.tower_cd.values_mut() {
            *cd = (*cd - dt).max(0.0);
        }
    }
    if game.flash_t <= 0.0 {
        for p in doc.props.iter_mut() {
            if let Some(rest) = rested_name(&p.name) {
                p.name = rest.into();
            }
        }
    }

    if input.attack {
        try_place(doc, game);
    }
    spawn_creeps(doc, game, dt);
    step_creeps(doc, game, dt);
    if game.wave_mode {
        apply_towers_by_kind(doc, game);
    } else {
        apply_tower(doc, game);
    }
    doc.coins = game.coins;
    finish_if_resolved(doc, game);
}

fn player_at_slot(doc: &WorldDoc) -> Option<String> {
    let player = player_ref(doc)?;
    let mut best: Option<(f32, String)> = None;
    for p in &doc.props {
        if !is_slot(p) || !p.enabled {
            continue;
        }
        let dx = player.position[0] - p.position[0];
        let dz = player.position[2] - p.position[2];
        let d = (dx * dx + dz * dz).sqrt();
        if d <= PLACE_REACH && best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
            best = Some((d, p.id.clone()));
        }
    }
    best.map(|(_, id)| id)
}

fn try_place(doc: &mut WorldDoc, game: &mut TdGame) {
    let Some(slot_id) = player_at_slot(doc) else {
        return;
    };
    if game.wave_mode {
        place_kind_on_slot(doc, game, &slot_id, game.selected);
    } else {
        place_box_on_slot(doc, game, &slot_id);
    }
}

fn place_box_on_slot(doc: &mut WorldDoc, game: &mut TdGame, slot_id: &str) {
    if game.coins < COST {
        return;
    }
    let Some(slot) = doc
        .props
        .iter()
        .find(|p| p.id == slot_id && is_slot(p) && p.enabled)
    else {
        return;
    };
    let pos = slot.position;
    let yaw = slot.yaw;
    if let Some(p) = doc.props.iter_mut().find(|p| p.id == slot_id) {
        p.name = "used".into();
    }
    let n = doc.props.iter().filter(|p| is_tower(p)).count();
    let extra = 0.5 * 3.2_f32;
    let y = doc.height_at(pos[0], pos[2]) + extra;
    doc.props.push(WorldProp {
        id: format!("prop:tower-{n}"),
        kind: "prop".into(),
        name: "tower".into(),
        position: [pos[0], y, pos[2]],
        yaw,
        model: "box".into(),
        gltf: None,
        scale: [1.5, 3.2, 1.5],
        enabled: true,
        parent: None,
        color: Some([54, 92, 110]),
        metallic: 0.0,
        roughness: 1.0,
        interact: None,
        is_static: true,
        friction: 0.85,
        restitution: 0.0,
        texture: None,
        uv_scale: None,
        pitch: 0.0,
        roll: 0.0,
        local: false,
        ..Default::default()
    });
    game.coins = game.coins.saturating_sub(COST);
    doc.coins = game.coins;
}

fn place_kind_on_slot(doc: &mut WorldDoc, game: &mut TdGame, slot_id: &str, kind: u8) {
    let kind = (kind as usize).min(KINDS.len() - 1);
    let cost = KINDS[kind].cost;
    if game.coins < cost {
        return;
    }
    let Some(slot) = doc
        .props
        .iter()
        .find(|p| p.id == slot_id && is_slot(p) && p.enabled)
    else {
        return;
    };
    let pos = slot.position;
    let yaw = slot.yaw;
    if let Some(p) = doc.props.iter_mut().find(|p| p.id == slot_id) {
        p.name = "used".into();
    }
    let spec = &KINDS[kind];
    let n = doc.props.iter().filter(|p| is_tower(p)).count();
    let prefix = match kind {
        1 => "cannon",
        2 => "mage",
        _ => "tower",
    };
    let y = doc.height_at(pos[0], pos[2]);
    doc.props.push(WorldProp {
        id: format!("prop:{prefix}-{n}"),
        kind: "prop".into(),
        name: spec.name.into(),
        position: [pos[0], y, pos[2]],
        yaw,
        model: spec.model.into(),
        gltf: Some(spec.gltf.into()),
        scale: spec.scale,
        enabled: true,
        parent: None,
        color: Some(spec.color),
        metallic: 0.04,
        roughness: 0.72,
        interact: None,
        is_static: true,
        friction: 0.85,
        restitution: 0.0,
        texture: None,
        uv_scale: None,
        pitch: 0.0,
        roll: 0.0,
        local: false,
        ..Default::default()
    });
    game.coins = game.coins.saturating_sub(cost);
    doc.coins = game.coins;
}

fn spawn_creeps(doc: &mut WorldDoc, game: &mut TdGame, dt: f32) {
    if game.wave_mode && game.wave_gap > 0.0 {
        game.wave_gap = (game.wave_gap - dt).max(0.0);
        return;
    }
    game.spawn_t += dt;
    let mut i = 0;
    while i < game.spawn_order.len() {
        let id = game.spawn_order[i].clone();
        let due = i as f32 * SPAWN_GAP;
        if game.spawn_t + 1e-4 >= due {
            let fresh = game.spawned.get(&id) != Some(&true);
            game.spawned.insert(id.clone(), true);
            if fresh && game.wave_mode {
                if let Some(p) = doc.props.iter_mut().find(|p| p.id == id) {
                    if p.name != "leaked" && p.name != "dead" {
                        p.enabled = true;
                        p.name = "creep".into();
                    }
                }
            }
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
    if game.wave_mode {
        game.lives = game.lives.saturating_sub(1);
        if game.lives == 0 {
            game.done = true;
            game.won = false;
            set_player_name(doc, "leak");
        }
    } else {
        set_player_name(doc, "leak");
    }
}

fn apply_tower(doc: &mut WorldDoc, game: &mut TdGame) {
    if game.fire_t > 0.0 {
        return;
    }
    let towers: Vec<(String, Vec3)> = doc
        .props
        .iter()
        .filter(|p| is_tower(p) && p.enabled)
        .map(|p| (p.id.clone(), Vec3::from_array(p.position)))
        .collect();
    if towers.is_empty() {
        return;
    }
    let mut fire_ids = Vec::new();
    let mut hit_ids = Vec::new();
    for (tid, origin) in &towers {
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
        if let Some((_, id)) = best {
            fire_ids.push(tid.clone());
            hit_ids.push(id);
        }
    }
    if fire_ids.is_empty() {
        return;
    }
    game.fire_t = TOWER_COOLDOWN;
    game.flash_t = HIT_FLASH;
    for tid in &fire_ids {
        if let Some(t) = doc.props.iter_mut().find(|p| p.id == *tid) {
            t.name = "fire".into();
        }
    }
    for id in hit_ids {
        let hp = game.creep_hp.entry(id.clone()).or_insert(CREEP_HP);
        if *hp == 0 {
            continue;
        }
        *hp = hp.saturating_sub(1);
        game.hits = game.hits.saturating_add(1);
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
}

fn finish_if_resolved(doc: &mut WorldDoc, game: &mut TdGame) {
    if game.wave_mode {
        finish_wave(doc, game);
        return;
    }
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
}

fn finish_wave(doc: &mut WorldDoc, game: &mut TdGame) {
    if game.done || game.waves.is_empty() || game.wave_gap > 0.0 {
        return;
    }
    if game.spawn_order.is_empty() {
        return;
    }
    let spawned_all = game
        .spawn_order
        .iter()
        .all(|id| game.spawned.get(id) == Some(&true));
    if !spawned_all || live_creeps(doc) > 0 {
        return;
    }
    if advance_wave(game) {
        return;
    }
    game.done = true;
    game.won = game.lives > 0;
    set_player_name(doc, if game.won { "clear" } else { "leak" });
}

fn advance_wave(game: &mut TdGame) -> bool {
    let mut next = game.wave_index + 1;
    while next < game.waves.len() && game.waves[next].is_empty() {
        next += 1;
    }
    if next >= game.waves.len() {
        return false;
    }
    game.wave_index = next;
    game.spawn_order = game.waves[next].clone();
    game.spawn_t = 0.0;
    game.wave_gap = WAVE_GAP;
    for id in &game.spawn_order {
        game.spawned.insert(id.clone(), false);
    }
    true
}

fn apply_towers_by_kind(doc: &mut WorldDoc, game: &mut TdGame) {
    let towers: Vec<(String, Vec3, usize)> = doc
        .props
        .iter()
        .filter(|p| is_tower(p) && p.enabled)
        .map(|p| (p.id.clone(), Vec3::from_array(p.position), kind_of(p)))
        .collect();
    let mut flashes: Vec<(String, usize)> = Vec::new();
    let mut hits: Vec<(String, u32)> = Vec::new();
    for (tid, origin, kind) in &towers {
        let cd = game.tower_cd.get(tid).copied().unwrap_or(0.0);
        if cd > 0.0 {
            continue;
        }
        let range = KINDS[*kind].range;
        let mut best: Option<(f32, String)> = None;
        for p in &doc.props {
            if !is_creep(p) || !p.enabled || p.name == "leaked" || p.name == "dead" {
                continue;
            }
            if game.spawned.get(&p.id) != Some(&true) {
                continue;
            }
            let d = Vec3::new(p.position[0] - origin.x, 0.0, p.position[2] - origin.z).length();
            if d <= range && best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
                best = Some((d, p.id.clone()));
            }
        }
        if let Some((_, id)) = best {
            flashes.push((tid.clone(), *kind));
            hits.push((id, KINDS[*kind].damage));
            game.tower_cd.insert(tid.clone(), KINDS[*kind].cooldown);
        }
    }
    if flashes.is_empty() {
        return;
    }
    game.flash_t = HIT_FLASH;
    for (tid, kind) in &flashes {
        if let Some(t) = doc.props.iter_mut().find(|p| p.id == *tid) {
            t.name = KINDS[*kind].flash.into();
        }
    }
    for (id, dmg) in hits {
        let hp = game.creep_hp.entry(id.clone()).or_insert(CREEP_HP);
        if *hp == 0 {
            continue;
        }
        *hp = hp.saturating_sub(dmg);
        game.hits = game.hits.saturating_add(1);
        if *hp == 0 {
            if let Some(p) = doc.props.iter_mut().find(|p| p.id == id) {
                p.name = "dead".into();
                p.enabled = false;
            }
            game.kills = game.kills.saturating_add(1);
            game.coins = game.coins.saturating_add(KILL_GOLD);
        } else if let Some(p) = doc.props.iter_mut().find(|p| p.id == id) {
            p.name = "hurt".into();
        }
    }
}

pub fn build_hud(game: &TdGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let pad = 16.0 * scale;
    let mut quads = Vec::new();
    let mut texts = Vec::new();
    let ink = [236, 242, 236, 255];

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
            texts.push(text_line(
                "td_lane",
                w * 0.5,
                h * 0.34,
                28.0 * scale,
                ink,
                TextAlign::Center,
            ));
            texts.push(text_line(
                "tap to start",
                w * 0.5,
                h * 0.60,
                16.0 * scale,
                [16, 28, 26, 255],
                TextAlign::Center,
            ));
        }
        GamePhase::Playing if game.wave_mode => {
            quads.push(Quad::new(
                pad,
                pad,
                w * 0.72,
                36.0 * scale,
                [16, 22, 18, 210],
            ));
            texts.push(text_line(
                &status_line(game, phase),
                pad + 10.0 * scale,
                pad + 8.0 * scale,
                15.0 * scale,
                ink,
                TextAlign::Left,
            ));
            for (kind, bx, by, bw, bh) in kind_buttons(width, height) {
                let selected = game.selected == kind;
                let color = if selected {
                    [240, 196, 72, 240]
                } else {
                    [28, 36, 32, 220]
                };
                quads.push(Quad::new(bx, by, bw, bh, color));
                texts.push(text_line(
                    &kind_label(kind as usize),
                    bx + bw * 0.5,
                    by + bh * 0.28,
                    14.0 * scale,
                    if selected { [28, 22, 12, 255] } else { ink },
                    TextAlign::Center,
                ));
            }
            if game.flash_t > 0.0 {
                let a = (50.0 + 90.0 * (game.flash_t / HIT_FLASH)) as u8;
                quads.push(Quad::new(
                    w * 0.42,
                    h * 0.62,
                    w * 0.16,
                    18.0 * scale,
                    [255, 200, 80, a.min(160)],
                ));
            }
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
            for i in 0..game.coins.min(12) {
                quads.push(Quad::new(
                    pad + i as f32 * (pip + gap),
                    pad + (pip + gap) * 2.0,
                    pip,
                    pip,
                    [240, 196, 72, 255],
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
            texts.push(text_line(
                &status_line(game, phase),
                pad,
                pad + (18.0 * scale + 6.0 * scale) * 3.0,
                14.0 * scale,
                ink,
                TextAlign::Left,
            ));
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
            texts.push(text_line(
                &status_line(game, phase),
                w * 0.5,
                h * 0.30,
                22.0 * scale,
                ink,
                TextAlign::Center,
            ));
            texts.push(text_line(
                "tap to retry",
                w * 0.5,
                h * 0.60,
                15.0 * scale,
                [28, 22, 12, 255],
                TextAlign::Center,
            ));
        }
    }

    DrawList {
        clear: [48, 52, 58, 255],
        quads,
        texts,
        ..Default::default()
    }
}

fn text_line(text: &str, x: f32, y: f32, size: f32, color: [u8; 4], align: TextAlign) -> TextQuad {
    TextQuad::new(text, x, y, size, color).aligned(align)
}

fn status_line(game: &TdGame, phase: GamePhase) -> String {
    match phase {
        GamePhase::Title => "td_lane - tap to start".into(),
        GamePhase::Playing if game.wave_mode => format!(
            "td_lane - wave {}/{}  gold {}  lives {}",
            game.wave_number(),
            game.wave_count(),
            game.coins,
            game.lives
        ),
        GamePhase::Playing => format!("td_lane - gold {}", game.coins),
        GamePhase::Complete if game.won => "td_lane - clear".into(),
        GamePhase::Complete => "td_lane - lost".into(),
    }
}

fn kind_label(index: usize) -> String {
    let kind = &KINDS[index.min(KINDS.len() - 1)];
    let name = match index {
        0 => "ARROW",
        1 => "CANNON",
        2 => "MAGE",
        _ => "TOWER",
    };
    format!("{name} {}", kind.cost)
}

/// Bottom row of tower-type buttons. Same rects as the HUD and the tap target.
pub fn kind_buttons(width: u32, height: u32) -> [(u8, f32, f32, f32, f32); 3] {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let bw = 150.0 * scale;
    let bh = 44.0 * scale;
    let gap = 12.0 * scale;
    let y = h - bh - 16.0 * scale;
    let total = bw * 3.0 + gap * 2.0;
    let x0 = (w - total) * 0.5;
    [
        (0, x0, y, bw, bh),
        (1, x0 + bw + gap, y, bw, bh),
        (2, x0 + (bw + gap) * 2.0, y, bw, bh),
    ]
}

fn overview_camera(doc: &WorldDoc) -> Camera {
    let cam = doc.cameras.first();
    let eye = cam
        .map(|c| Vec3::from_array(c.position))
        .unwrap_or(Vec3::new(0.0, 18.0, -16.0));
    let target = cam
        .map(|c| Vec3::from_array(c.target))
        .unwrap_or(Vec3::new(0.0, 0.4, 0.0));
    let fov = cam.map(|c| c.fov).unwrap_or(50.0);
    Camera {
        eye,
        target,
        up: Vec3::Y,
        fov_y: fov.to_radians(),
        near: 0.2,
        far: 400.0,
    }
}

/// World point to screen pixels (top-left origin, y down). Overview camera.
pub fn world_to_screen(
    doc: &WorldDoc,
    world: [f32; 3],
    width: u32,
    height: u32,
) -> Option<(f32, f32)> {
    let cam = overview_camera(doc);
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    let clip = cam.view_projection(aspect) * glam::Vec4::new(world[0], world[1], world[2], 1.0);
    if clip.w.abs() < 1e-5 {
        return None;
    }
    let ndc_x = clip.x / clip.w;
    let ndc_y = clip.y / clip.w;
    if !ndc_x.is_finite() || !ndc_y.is_finite() {
        return None;
    }
    let sx = (ndc_x * 0.5 + 0.5) * width as f32;
    let sy = (0.5 - ndc_y * 0.5) * height as f32;
    Some((sx, sy))
}

fn screen_to_ground(doc: &WorldDoc, x: f32, y: f32, width: u32, height: u32) -> Option<Vec3> {
    let cam = overview_camera(doc);
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    let inv = cam.view_projection(aspect).inverse();
    if !inv.determinant().is_finite() {
        return None;
    }
    let ndc_x = (x / width.max(1) as f32) * 2.0 - 1.0;
    let ndc_y = 1.0 - (y / height.max(1) as f32) * 2.0;
    let near = project_point(inv, Vec3::new(ndc_x, ndc_y, 0.0));
    let far = project_point(inv, Vec3::new(ndc_x, ndc_y, 1.0));
    if !near.is_finite() || !far.is_finite() {
        return None;
    }
    let dir = far - near;
    if dir.y.abs() < 1e-6 {
        return None;
    }
    let t = (doc.floor_y - near.y) / dir.y;
    if t < 0.0 {
        return None;
    }
    Some(near + dir * t)
}

fn project_point(m: glam::Mat4, p: Vec3) -> Vec3 {
    let v = m * glam::Vec4::new(p.x, p.y, p.z, 1.0);
    if v.w.abs() < 1e-8 {
        return Vec3::NAN;
    }
    Vec3::new(v.x / v.w, v.y / v.w, v.z / v.w)
}

fn slot_at_screen(doc: &WorldDoc, x: f32, y: f32, width: u32, height: u32) -> Option<String> {
    let hit = screen_to_ground(doc, x, y, width, height)?;
    let mut best: Option<(f32, String)> = None;
    for p in &doc.props {
        if !is_slot(p) || !p.enabled {
            continue;
        }
        let dx = p.position[0] - hit.x;
        let dz = p.position[2] - hit.z;
        let d = (dx * dx + dz * dz).sqrt();
        if d <= SLOT_PICK && best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
            best = Some((d, p.id.clone()));
        }
    }
    best.map(|(_, id)| id)
}

/// Playing-phase tap / click. Title and result stay on `GenreSession::confirm`.
pub fn pointer_play(
    doc: &mut WorldDoc,
    game: &mut TdGame,
    x: f32,
    y: f32,
    width: u32,
    height: u32,
) {
    if game.done {
        return;
    }
    if game.wave_mode {
        for (kind, bx, by, bw, bh) in kind_buttons(width, height) {
            if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
                game.selected = kind;
                return;
            }
        }
    }
    let Some(slot_id) = slot_at_screen(doc, x, y, width, height) else {
        return;
    };
    if game.wave_mode {
        place_kind_on_slot(doc, game, &slot_id, game.selected);
    } else {
        place_box_on_slot(doc, game, &slot_id);
    }
}

/// HUD JSON (`quads` + ASCII `texts`) for `draw_world`.
pub fn hud_json(game: &TdGame, phase: GamePhase, width: u32, height: u32) -> String {
    let hud = build_hud(game, phase, width, height);
    let quads: Vec<_> = hud
        .quads
        .iter()
        .map(|q| {
            serde_json::json!({
                "x": q.x,
                "y": q.y,
                "w": q.w,
                "h": q.h,
                "color": q.color,
            })
        })
        .collect();
    let texts: Vec<_> = hud
        .texts
        .iter()
        .map(|t| {
            let align = match t.align {
                TextAlign::Left => "left",
                TextAlign::Center => "center",
                TextAlign::Right => "right",
            };
            serde_json::json!({
                "text": t.text,
                "x": t.x,
                "y": t.y,
                "size": t.size,
                "color": t.color,
                "align": align,
            })
        })
        .collect();
    serde_json::to_string(&serde_json::json!({ "quads": quads, "texts": texts }))
        .unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let pads: Vec<_> = doc
            .props
            .iter()
            .filter(|p| is_slot(p) && p.enabled)
            .collect();
        assert!(pads.len() >= 2, "need place slots, got {}", pads.len());
        assert!(pads.iter().all(|p| is_boxish(p)));
        assert_eq!(doc.coins, START);
        assert!(
            !crate::puzzle::is_puzzle(&doc),
            "slot must not be puzzle pad"
        );
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
        assert_eq!(play.doc.coins, START);
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
        assert_eq!(play.doc.coins, START, "leaks must not steal place-coins");
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

    fn put_player(play: &mut WorldPlay, x: f32, z: f32) {
        let y = play.doc.height_at(x, z) + BODY_H;
        if let Some(w) = play.doc.player.as_mut() {
            w.position = [x, y, z];
            w.on_ground = true;
        }
        if let Some(w) = play.doc.walkers.first_mut() {
            w.position = [x, y, z];
            w.on_ground = true;
        }
    }

    fn slot_xz(play: &WorldPlay) -> (f32, f32) {
        let p = play
            .doc
            .props
            .iter()
            .filter(|p| is_slot(p) && p.enabled)
            .min_by(|a, b| a.id.cmp(&b.id))
            .unwrap();
        (p.position[0], p.position[2])
    }

    #[test]
    fn attack_on_slot_places_tower_and_spends_coins() {
        let mut play = play_started();
        assert_eq!(play.doc.coins, START);
        let towers0 = play.doc.props.iter().filter(|p| is_tower(p)).count();
        let (x, z) = slot_xz(&play);
        put_player(&mut play, x, z);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        let towers1 = play.doc.props.iter().filter(|p| is_tower(p)).count();
        assert_eq!(towers1, towers0 + 1, "place must add a tower entity");
        assert_eq!(play.doc.coins, START - COST);
        assert_eq!(play.td.coins, START - COST);
        assert!(
            play.doc.props.iter().any(|p| p.name == "used"),
            "occupied slot must be dump-visible"
        );
        let dump = play.doc.to_json().unwrap();
        assert!(
            dump.contains("prop:tower-"),
            "new tower id must be dump-visible"
        );
        assert!(
            dump.contains(&format!("\"coins\": {0}", START - COST))
                || dump.contains(&format!("\"coins\":{0}", START - COST)),
            "remaining coins must be dump-visible, dump coins field missing {0}",
            START - COST
        );
        assert!(dump.contains("used"));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 3, "coin overlay");
        assert_eq!(play.doc.cameras[0].name, "overview");
    }

    #[test]
    fn attack_away_from_slot_does_not_place() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 14.0);
        let towers0 = play.doc.props.iter().filter(|p| is_tower(p)).count();
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        let towers1 = play.doc.props.iter().filter(|p| is_tower(p)).count();
        assert_eq!(towers1, towers0, "off-slot J is not a place");
        assert_eq!(play.doc.coins, START);
        assert!(!play.doc.props.iter().any(|p| p.name == "used"));
    }

    #[test]
    fn place_without_coins_is_ignored() {
        let mut play = play_started();
        play.td.coins = COST - 1;
        play.doc.coins = COST - 1;
        let (x, z) = slot_xz(&play);
        put_player(&mut play, x, z);
        let towers0 = play.doc.props.iter().filter(|p| is_tower(p)).count();
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        let towers1 = play.doc.props.iter().filter(|p| is_tower(p)).count();
        assert_eq!(towers1, towers0, "not enough coins");
        assert_eq!(play.doc.coins, COST - 1);
        assert!(!play.doc.props.iter().any(|p| p.name == "used"));
    }

    #[test]
    fn second_place_on_same_slot_does_not_spend() {
        let mut play = play_started();
        let (x, z) = slot_xz(&play);
        put_player(&mut play, x, z);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        let coins = play.doc.coins;
        let towers = play.doc.props.iter().filter(|p| is_tower(p)).count();
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.doc.coins, coins, "used slot must not spend again");
        assert_eq!(
            play.doc.props.iter().filter(|p| is_tower(p)).count(),
            towers
        );
    }

    #[test]
    fn placed_tower_auto_fires() {
        let mut play = play_started();
        let (x, z) = slot_xz(&play);
        put_player(&mut play, x, z);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        let placed = play
            .doc
            .props
            .iter()
            .find(|p| p.id.starts_with("prop:tower-"))
            .unwrap();
        let pos = placed.position;
        let id = play
            .doc
            .props
            .iter()
            .find(|p| is_creep(p) && p.enabled)
            .unwrap()
            .id
            .clone();
        park_creep(&mut play, &id, pos[0] + 1.2, pos[2]);
        play.td.fire_t = 0.0;
        play.tick(1.0 / 60.0);
        assert!(
            play.td.hits >= 1,
            "placed tower should auto-fire, hits {}",
            play.td.hits
        );
        let dump = play.doc.to_json().unwrap();
        assert!(
            dump.contains("hurt") || dump.contains("dead") || dump.contains("fire"),
            "placed-tower hit must be dump-visible"
        );
    }

    #[test]
    fn title_does_not_place() {
        let mut play = WorldPlay::from_json(LANE).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        let (x, z) = slot_xz(&play);
        put_player(&mut play, x, z);
        let towers0 = play.doc.props.iter().filter(|p| is_tower(p)).count();
        play.input.attack = true;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        let towers1 = play.doc.props.iter().filter(|p| is_tower(p)).count();
        assert_eq!(towers1, towers0, "title must not place");
        assert_eq!(play.doc.coins, START);
    }

    #[test]
    fn confirm_retry_restores_towers_and_coins() {
        let mut play = play_started();
        let (x, z) = slot_xz(&play);
        put_player(&mut play, x, z);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.doc.coins < START);
        play.start();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.coins, START);
        assert_eq!(play.td.coins, START);
        assert_eq!(
            play.doc
                .props
                .iter()
                .filter(|p| is_tower(p) && p.enabled)
                .count(),
            1
        );
        assert!(play.doc.props.iter().any(|p| is_slot(p) && p.enabled));
        assert!(!play.doc.props.iter().any(|p| p.name == "used"));
    }

    const WAVES: &str = include_str!("../tests/fixtures/td_world.json");

    fn assert_ascii(s: &str) {
        assert!(s.is_ascii(), "HUD must stay ASCII, got {s}");
    }

    #[test]
    fn wave_map_has_five_waves_ascii_hud() {
        let doc = WorldDoc::from_json(WAVES).unwrap();
        assert!(wave_mode(&doc));
        assert!(!wave_mode(&WorldDoc::from_json(LANE).unwrap()));
        assert_eq!(starting_coins(&doc), WAVE_GOLD);
        let mut seen = std::collections::BTreeSet::new();
        let mut n = 0;
        for p in &doc.props {
            if is_creep(p) {
                n += 1;
                seen.insert(creep_wave(&p.id).unwrap());
                assert!(p.gltf.is_some(), "creeps use bundled gltf");
            }
        }
        assert_eq!(n, 19);
        assert_eq!(
            seen.iter().copied().collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        let play = WorldPlay::from_json(WAVES).unwrap();
        assert_eq!(play.td.wave_count(), 5);
        assert_eq!(play.td.lives, WAVE_LIVES);
        assert_eq!(play.doc.coins, WAVE_GOLD);
        let hud = hud_json(&play.td, GamePhase::Playing, 960, 540);
        assert_ascii(&hud);
        assert!(hud.contains("wave 1/5"));
        assert!(hud.contains("gold"));
        assert!(hud.contains("lives"));
        assert!(hud.contains("ARROW"));
        assert!(hud.contains("CANNON"));
        assert!(hud.contains("MAGE"));
    }

    #[test]
    fn tap_and_button_place_two_tower_types() {
        let mut play = WorldPlay::from_json(WAVES).unwrap();
        play.start();
        let slot = play
            .doc
            .props
            .iter()
            .find(|p| p.id == "prop:slot-a")
            .unwrap();
        let xz = [slot.position[0], 0.0, slot.position[2]];
        let (sx, sy) = world_to_screen(&play.doc, xz, 960, 540).unwrap();
        let back = screen_to_ground(&play.doc, sx, sy, 960, 540).unwrap();
        assert!(
            (back.x - xz[0]).abs() < 0.25 && (back.z - xz[2]).abs() < 0.25,
            "ground pick drifted to {back:?} from {xz:?}"
        );
        pointer_play(&mut play.doc, &mut play.td, sx, sy, 960, 540);
        assert!(play
            .doc
            .props
            .iter()
            .any(|p| p.gltf.as_deref() == Some("gazebo")));
        assert_eq!(play.td.coins, WAVE_GOLD - KINDS[0].cost);
        let (_, bx, by, bw, bh) = kind_buttons(960, 540)[1];
        pointer_play(
            &mut play.doc,
            &mut play.td,
            bx + bw * 0.5,
            by + bh * 0.5,
            960,
            540,
        );
        assert_eq!(play.td.selected, 1);
        let slot = play
            .doc
            .props
            .iter()
            .find(|p| p.id == "prop:slot-b")
            .unwrap();
        let xz = [slot.position[0], 0.0, slot.position[2]];
        let (sx, sy) = world_to_screen(&play.doc, xz, 960, 540).unwrap();
        pointer_play(&mut play.doc, &mut play.td, sx, sy, 960, 540);
        assert!(play.doc.props.iter().any(|p| p.name == "cannon"));
        assert!(play.doc.props.iter().any(|p| p.name == "used"));
    }

    #[test]
    fn one_life_then_leak_loses_and_retry_restores() {
        let mut play = WorldPlay::from_json(WAVES).unwrap();
        play.start();
        play.td.lives = 1;
        let id = play.td.spawn_order[0].clone();
        let path = waypoints(&play.doc);
        let end = *path.last().unwrap();
        if let Some(p) = play.doc.props.iter_mut().find(|p| p.id == id) {
            p.enabled = true;
            p.name = "creep".into();
            p.position = end;
        }
        play.td.spawned.insert(id.clone(), true);
        play.td.creep_seg.insert(id.clone(), (path.len() - 2, 0.92));
        for _ in 0..50 {
            play.tick(1.0 / 60.0);
            if play.td.done {
                break;
            }
        }
        assert!(
            play.td.done && !play.td.won,
            "lives={} leaks={} phase={:?}",
            play.td.lives,
            play.td.leaks,
            play.game.phase
        );
        assert_eq!(play.td.lives, 0);
        assert_eq!(play.game.phase, GamePhase::Complete);
        let hud = hud_json(&play.td, play.game.phase, 960, 540);
        assert_ascii(&hud);
        assert!(hud.contains("lost"));
        assert!(hud.contains("tap to retry"));
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.td.lives, WAVE_LIVES);
        assert_eq!(play.doc.coins, WAVE_GOLD);
        assert!(!play.td.won && !play.td.done);
    }

    #[test]
    fn three_towers_clear_five_waves() {
        let mut play = WorldPlay::from_json(WAVES).unwrap();
        play.start();
        for (kind, id) in [(0u8, "prop:slot-a"), (1, "prop:slot-b"), (2, "prop:slot-c")] {
            play.td.select(kind);
            let slot = play.doc.props.iter().find(|p| p.id == id).unwrap();
            let xz = [slot.position[0], 0.0, slot.position[2]];
            let (sx, sy) = world_to_screen(&play.doc, xz, 960, 540).unwrap();
            pointer_play(&mut play.doc, &mut play.td, sx, sy, 960, 540);
        }
        assert_eq!(play.doc.props.iter().filter(|p| is_tower(p)).count(), 3);
        for _ in 0..9000 {
            if play.td.done {
                break;
            }
            play.tick(1.0 / 60.0);
        }
        assert!(
            play.td.won && play.td.lives > 0,
            "won={} lives={} leaks={} kills={} wave={}/{}",
            play.td.won,
            play.td.lives,
            play.td.leaks,
            play.td.kills,
            play.td.wave_number(),
            play.td.wave_count()
        );
        assert_eq!(play.game.phase, GamePhase::Complete);
        let hud = hud_json(&play.td, play.game.phase, 960, 540);
        assert_ascii(&hud);
        assert!(hud.contains("clear"));
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.td.lives, WAVE_LIVES);
        assert_eq!(
            play.doc.props.iter().filter(|p| is_tower(p)).count(),
            0,
            "retry drops placed towers"
        );
    }
}
