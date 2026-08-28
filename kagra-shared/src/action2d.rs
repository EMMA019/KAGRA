//! 2D action on play_world: side-view sprite walk, hit, hurt, kill,
//! projectile, and room switch.
//!
//! Sibling of 3D `action` / `sprite`. Player card and foe card are the same
//! `model: "sprite"` / `"quad"` WorldDoc path (`MESH_QUAD` in `compile_scene`).
//! Walk along X on a back wall + floor. J / click hits the foe sprite when
//! in reach; otherwise it spawns a dump-visible `shot` card that moves and
//! can hit/kill. Crossing a `trigger` swaps hall <-> den (dump scene / name
//! / flag like RPG town <-> dungeon). Sprite stays on WorldDoc. Hurt / kill
//! are dump-visible (`name` + foe `enabled`). Title -> play -> result reuses
//! `WorldPlay` / `GamePhase`. Overlay flash is `DrawList` quads on shared
//! wgpu 30. Does not rewrite 3D `action.rs`, RPG, FPS, fight, TD, puzzle, or
//! VRM. No new ECS, no RendererV2, no Rapier, no billboards, no net. No
//! `enemy.chase` (not in docs/API_INDEX.md). Empty / no-ammo is not this slice.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::sprite;
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};

pub const GAME_ID: &str = "action_side";
pub const PLAYER_HP: u32 = 3;
pub const FOE_HP: u32 = 2;
pub const BODY_H: f32 = 0.95;
pub const SPEED: f32 = 5.6;
pub const ATTACK_REACH: f32 = 1.55;
pub const ATTACK_TIME: f32 = 0.22;
pub const HIT_FLASH: f32 = 0.20;
pub const CONTACT_CD: f32 = 0.55;
pub const IFRAME: f32 = 0.28;
pub const PLANE_Z: f32 = 0.0;
pub const CAM_Z: f32 = 9.2;
pub const CAM_Y: f32 = 2.35;
pub const CAM_LOOK_Y: f32 = 1.15;
pub const CAM_FOV: f32 = 48.0;
pub const NAME_PLAYER: &str = "player";
pub const NAME_HURT: &str = "hurt";
pub const NAME_DEAD: &str = "dead";
pub const NAME_SHOT: &str = "shot";
pub const NAME_TRIGGER: &str = "trigger";
pub const SCENE_HALL: &str = "hall";
pub const SCENE_DEN: &str = "den";
pub const FLAG_DEN: &str = "den";
pub const SHOT_SPEED: f32 = 12.0;
pub const SHOT_REACH: f32 = 0.42;
pub const FIRE_OFFSET: f32 = 0.70;
pub const TRIGGER_REACH: f32 = 1.35;
pub const SWITCH_CD: f32 = 0.45;
pub const HALL_RETURN_X: f32 = -4.6;
pub const DEN_SPAWN_X: f32 = 3.0;
pub const ID_SHOT: &str = "prop:shot";
pub const ID_SCENE: &str = "prop:scene";
pub const ID_FLAG: &str = "prop:flag";
pub const ID_TRIGGER: &str = "prop:trigger";

const HALL_JSON: &str = include_str!("../tests/fixtures/action_side_world.json");
const DEN_JSON: &str = include_str!("../tests/fixtures/action_side_den_world.json");

const HERO_COLOR: [u32; 3] = [62, 176, 184];
const HERO_HURT: [u32; 3] = [220, 64, 64];
const HERO_DEAD: [u32; 3] = [70, 74, 82];
const FOE_COLOR: [u32; 3] = [196, 64, 54];

/// Live 2D combat around a dump. HP / swing stay here; sprite foe `enabled`
/// and walker `name` in the dump are the query source of truth.
#[derive(Clone, Debug)]
pub struct Action2dGame {
    pub hp: u32,
    pub hits: u32,
    pub kills: u32,
    pub attack_t: f32,
    pub flash_t: f32,
    pub hurt_flash: bool,
    pub contact_cd: f32,
    pub iframe_t: f32,
    pub dead: bool,
    pub won: bool,
    pub scene: String,
    pub flags: Vec<String>,
    foe_hp: u32,
    hall_foe_hp: u32,
    den_foe_hp: u32,
    swing_hit: bool,
    facing: f32,
    shot_vx: f32,
    switch_cd: f32,
    hall: WorldDoc,
    den: WorldDoc,
}

impl Default for Action2dGame {
    fn default() -> Self {
        Self {
            hp: PLAYER_HP,
            hits: 0,
            kills: 0,
            attack_t: 0.0,
            flash_t: 0.0,
            hurt_flash: false,
            contact_cd: 0.0,
            iframe_t: 0.0,
            dead: false,
            won: false,
            scene: SCENE_HALL.into(),
            flags: Vec::new(),
            foe_hp: FOE_HP,
            hall_foe_hp: FOE_HP,
            den_foe_hp: FOE_HP,
            swing_hit: false,
            facing: 1.0,
            shot_vx: 0.0,
            switch_cd: 0.0,
            hall: WorldDoc::from_json(HALL_JSON).unwrap_or_default(),
            den: WorldDoc::from_json(DEN_JSON).unwrap_or_default(),
        }
    }
}

impl Action2dGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let scene = if is_den(doc) { SCENE_DEN } else { SCENE_HALL };
        let foe_hp = if live_foes(doc) == 0 { 0 } else { FOE_HP };
        Self {
            scene: scene.into(),
            foe_hp,
            hall_foe_hp: if scene == SCENE_HALL { foe_hp } else { FOE_HP },
            den_foe_hp: if scene == SCENE_DEN { foe_hp } else { FOE_HP },
            ..Self::default()
        }
    }

    pub fn has_flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
}

pub fn is_action2d(doc: &WorldDoc) -> bool {
    doc.props.iter().any(is_sprite_foe)
}

fn is_sprite_foe(p: &WorldProp) -> bool {
    p.name == "foe" && sprite::is_sprite_prop(p)
}

fn is_hero_card(p: &WorldProp) -> bool {
    sprite::is_sprite_prop(p)
        && p.name != "foe"
        && p.name != NAME_SHOT
        && p.name != NAME_TRIGGER
        && p.name != "wall"
        && p.name != "floor"
        && p.name != "flag"
        && p.name != SCENE_HALL
        && p.name != SCENE_DEN
}

fn is_den(doc: &WorldDoc) -> bool {
    doc.props
        .iter()
        .any(|p| p.id == ID_SCENE && p.name == SCENE_DEN)
        || (doc.props.iter().any(|p| p.name == SCENE_DEN)
            && !doc.props.iter().any(|p| p.name == SCENE_HALL))
}

fn shot_live(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == NAME_SHOT && p.enabled)
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

/// Sit wall / floor / cards on the 2D plane. Camera looks at the cards.
pub fn seed(doc: &mut WorldDoc) {
    if !is_action2d(doc) {
        return;
    }
    ensure_markers(doc);
    sit_plane(doc);
    place_side_camera(doc);
}

pub fn place_side_camera(doc: &mut WorldDoc) {
    let (px, py) = player_ref(doc)
        .map(|w| (w.position[0], w.position[1]))
        .unwrap_or((0.0, BODY_H));
    let eye = [px, py.max(CAM_Y), CAM_Z];
    let target = [px, CAM_LOOK_Y.max(0.4), PLANE_Z];
    let fov = doc.cameras.first().map(|cam| cam.fov).unwrap_or(CAM_FOV);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye;
        cam.target = target;
        cam.name = "side".into();
        cam.fov = fov;
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "side".into(),
            position: eye,
            target,
            fov,
        });
    }
}

/// Side-view walk + hit. Owns movement so A/D and W (`--seconds`) share +X.
pub fn tick(doc: &mut WorldDoc, game: &mut Action2dGame, input: WalkInput, dt: f32) {
    if game.dead || game.won {
        write_beat(doc, game);
        place_side_camera(doc);
        return;
    }
    game.attack_t = (game.attack_t - dt).max(0.0);
    game.flash_t = (game.flash_t - dt).max(0.0);
    game.contact_cd = (game.contact_cd - dt).max(0.0);
    game.iframe_t = (game.iframe_t - dt).max(0.0);
    game.switch_cd = (game.switch_cd - dt).max(0.0);
    if game.flash_t <= 0.0 && !game.dead {
        game.hurt_flash = false;
    }

    apply_walk(doc, game, input, dt);
    apply_attack(doc, game, input);
    apply_shots(doc, game, dt);
    apply_contact(doc, game);
    apply_switch(doc, game);
    write_beat(doc, game);
    place_side_camera(doc);

    if game.hp == 0 && !game.dead {
        die(doc, game);
    } else if live_foes(doc) == 0 {
        game.won = true;
    }
}

fn live_foes(doc: &WorldDoc) -> usize {
    doc.props
        .iter()
        .filter(|p| is_sprite_foe(p) && p.enabled)
        .count()
}

fn apply_walk(doc: &mut WorldDoc, game: &mut Action2dGame, input: WalkInput, dt: f32) {
    let Some(w) = player_ref(doc).cloned() else {
        return;
    };
    let along = (input.lx + input.lz).clamp(-1.0, 1.0);
    if along.abs() > 0.08 {
        game.facing = along.signum();
    }
    let half = doc.half.max(4.0);
    let pad = 1.6;
    let x = (w.position[0] + along * SPEED * dt).clamp(-half + pad, half - pad);
    let z = PLANE_Z;
    let y = doc.height_at(x, z) + BODY_H;
    write_player(
        doc,
        WorldWalker {
            id: w.id,
            kind: "walker".into(),
            name: beat_name(game).into(),
            position: [x, y, z],
            yaw: if game.facing < 0.0 {
                std::f32::consts::PI
            } else {
                0.0
            },
            face: if game.facing < 0.0 {
                std::f32::consts::PI
            } else {
                0.0
            },
            on_ground: true,
            ..Default::default()
        },
    );
}

fn apply_attack(doc: &mut WorldDoc, game: &mut Action2dGame, input: WalkInput) {
    if input.attack && game.attack_t <= 0.0 {
        game.attack_t = ATTACK_TIME;
        game.swing_hit = false;
        if try_melee_hit(doc, game) {
            return;
        }
        spawn_shot(doc, game);
        return;
    }
    if game.attack_t <= 0.0 || game.swing_hit || shot_live(doc) {
        return;
    }
    try_melee_hit(doc, game);
}

fn try_melee_hit(doc: &mut WorldDoc, game: &mut Action2dGame) -> bool {
    let Some(w) = player_ref(doc) else {
        return false;
    };
    let ax = w.position[0] + game.facing * ATTACK_REACH * 0.55;
    let mut hit_id: Option<String> = None;
    for p in &doc.props {
        if !is_sprite_foe(p) || !p.enabled {
            continue;
        }
        let r = foe_half(p) + 0.55;
        let dx = ax - p.position[0];
        let dz = w.position[2] - p.position[2];
        if dx * dx + dz * dz <= r * r {
            hit_id = Some(p.id.clone());
            break;
        }
    }
    let Some(id) = hit_id else {
        return false;
    };
    game.swing_hit = true;
    apply_foe_hit(doc, game, &id);
    true
}

fn apply_foe_hit(doc: &mut WorldDoc, game: &mut Action2dGame, id: &str) {
    game.foe_hp = game.foe_hp.saturating_sub(1);
    game.hits += 1;
    game.flash_t = HIT_FLASH;
    game.hurt_flash = false;
    if game.foe_hp == 0 {
        if let Some(p) = doc.props.iter_mut().find(|p| p.id == id) {
            p.enabled = false;
        }
        game.kills += 1;
    }
    if game.scene == SCENE_DEN {
        game.den_foe_hp = game.foe_hp;
    } else {
        game.hall_foe_hp = game.foe_hp;
    }
}

fn spawn_shot(doc: &mut WorldDoc, game: &mut Action2dGame) {
    let Some(w) = player_ref(doc) else {
        return;
    };
    let x = w.position[0] + game.facing * FIRE_OFFSET;
    let y = w.position[1].max(BODY_H);
    ensure_shot(doc);
    if let Some(p) = doc
        .props
        .iter_mut()
        .find(|p| p.id == ID_SHOT || p.name == NAME_SHOT)
    {
        p.enabled = true;
        p.name = NAME_SHOT.into();
        p.model = "sprite".into();
        p.position = [x, y, PLANE_Z];
        p.scale = [0.50, 0.28, 1.0];
        p.color = Some([240, 220, 90]);
        p.yaw = if game.facing < 0.0 {
            std::f32::consts::PI
        } else {
            0.0
        };
    }
    game.shot_vx = game.facing.signum() * SHOT_SPEED;
}

fn apply_shots(doc: &mut WorldDoc, game: &mut Action2dGame, dt: f32) {
    if !shot_live(doc) {
        game.shot_vx = 0.0;
        return;
    }
    let half = doc.half.max(4.0);
    let (sx, sz) = if let Some(shot) = doc
        .props
        .iter_mut()
        .find(|p| p.name == NAME_SHOT && p.enabled)
    {
        shot.position[0] += game.shot_vx * dt;
        shot.position[2] = PLANE_Z;
        (shot.position[0], shot.position[2])
    } else {
        return;
    };
    let out = sx.abs() > half + 1.5;
    let mut hit_id: Option<String> = None;
    if !out {
        for p in &doc.props {
            if !is_sprite_foe(p) || !p.enabled {
                continue;
            }
            let r = foe_half(p) + SHOT_REACH;
            let dx = sx - p.position[0];
            let dz = sz - p.position[2];
            if dx * dx + dz * dz <= r * r {
                hit_id = Some(p.id.clone());
                break;
            }
        }
    }
    if out {
        disable_shots(doc);
        game.shot_vx = 0.0;
        return;
    }
    if let Some(id) = hit_id {
        apply_foe_hit(doc, game, &id);
        disable_shots(doc);
        game.shot_vx = 0.0;
    }
}

fn disable_shots(doc: &mut WorldDoc) {
    for p in &mut doc.props {
        if p.name == NAME_SHOT {
            p.enabled = false;
        }
    }
}

fn apply_switch(doc: &mut WorldDoc, game: &mut Action2dGame) {
    if game.switch_cd > 0.0 || game.dead || game.won {
        return;
    }
    if !near_named(doc, NAME_TRIGGER, TRIGGER_REACH) {
        return;
    }
    if game.scene == SCENE_HALL {
        game.hall_foe_hp = game.foe_hp;
        game.hall = doc.clone();
        *doc = game.den.clone();
        game.scene = SCENE_DEN.into();
        if !game.has_flag(FLAG_DEN) {
            game.flags.push(FLAG_DEN.into());
        }
        game.foe_hp = bind_room_hp(doc, game.den_foe_hp);
        park_player(doc, game, DEN_SPAWN_X);
    } else {
        game.den_foe_hp = game.foe_hp;
        game.den = doc.clone();
        *doc = game.hall.clone();
        game.scene = SCENE_HALL.into();
        game.foe_hp = bind_room_hp(doc, game.hall_foe_hp);
        park_player(doc, game, HALL_RETURN_X);
    }
    game.shot_vx = 0.0;
    game.switch_cd = SWITCH_CD;
    disable_shots(doc);
    seed(doc);
    write_beat(doc, game);
}

fn park_player(doc: &mut WorldDoc, game: &Action2dGame, x: f32) {
    let half = doc.half.max(4.0);
    let pad = 1.6;
    let x = x.clamp(-half + pad, half - pad);
    let y = doc.height_at(x, PLANE_Z) + BODY_H;
    let Some(w) = player_ref(doc).cloned() else {
        return;
    };
    write_player(
        doc,
        WorldWalker {
            id: w.id,
            kind: "walker".into(),
            name: beat_name(game).into(),
            position: [x, y, PLANE_Z],
            yaw: if game.facing < 0.0 {
                std::f32::consts::PI
            } else {
                0.0
            },
            face: if game.facing < 0.0 {
                std::f32::consts::PI
            } else {
                0.0
            },
            on_ground: true,
            ..Default::default()
        },
    );
}

fn bind_room_hp(doc: &WorldDoc, stored: u32) -> u32 {
    if live_foes(doc) == 0 {
        0
    } else if stored == 0 {
        FOE_HP
    } else {
        stored.min(FOE_HP)
    }
}

fn near_named(doc: &WorldDoc, name: &str, reach: f32) -> bool {
    let Some(w) = player_ref(doc) else {
        return false;
    };
    doc.props.iter().any(|p| {
        p.enabled && p.name == name && {
            let dx = w.position[0] - p.position[0];
            let dz = w.position[2] - p.position[2];
            dx * dx + dz * dz <= reach * reach
        }
    })
}

fn ensure_markers(doc: &mut WorldDoc) {
    let den = is_den(doc);
    ensure_prop(
        doc,
        WorldProp {
            id: ID_TRIGGER.into(),
            kind: "prop".into(),
            name: NAME_TRIGGER.into(),
            position: if den {
                [7.2, 1.2, PLANE_Z]
            } else {
                [-7.2, 1.2, PLANE_Z]
            },
            yaw: 0.0,
            model: "box".into(),
            gltf: None,
            scale: [0.5, 2.4, 1.2],
            enabled: true,
            parent: None,
            color: Some(if den { [180, 140, 88] } else { [160, 120, 72] }),
            metallic: 0.0,
            roughness: 0.85,
        },
    );
    ensure_shot(doc);
    ensure_prop(
        doc,
        tiny_prop(
            ID_SCENE,
            if den { SCENE_DEN } else { SCENE_HALL },
            [0.0, 0.2, 0.8],
            true,
            if den { [52, 44, 72] } else { [40, 36, 52] },
        ),
    );
    ensure_prop(
        doc,
        tiny_prop(
            ID_FLAG,
            "flag",
            if den {
                [7.2, 2.6, PLANE_Z]
            } else {
                [-7.2, 2.6, PLANE_Z]
            },
            false,
            [240, 196, 72],
        ),
    );
}

fn ensure_shot(doc: &mut WorldDoc) {
    ensure_prop(
        doc,
        WorldProp {
            id: ID_SHOT.into(),
            kind: "prop".into(),
            name: NAME_SHOT.into(),
            position: [0.0, BODY_H, PLANE_Z],
            yaw: 0.0,
            model: "sprite".into(),
            gltf: None,
            scale: [0.50, 0.28, 1.0],
            enabled: false,
            parent: None,
            color: Some([240, 220, 90]),
            metallic: 0.0,
            roughness: 0.7,
        },
    );
}

fn ensure_prop(doc: &mut WorldDoc, prop: WorldProp) {
    if doc.props.iter().any(|p| p.id == prop.id) {
        return;
    }
    doc.props.push(prop);
}

fn tiny_prop(
    id: &str,
    name: &str,
    position: [f32; 3],
    enabled: bool,
    color: [u32; 3],
) -> WorldProp {
    WorldProp {
        id: id.into(),
        kind: "prop".into(),
        name: name.into(),
        position,
        yaw: 0.0,
        model: "box".into(),
        gltf: None,
        scale: [0.22, 0.22, 0.22],
        enabled,
        parent: None,
        color: Some(color),
        metallic: 0.0,
        roughness: 1.0,
    }
}

fn apply_contact(doc: &mut WorldDoc, game: &mut Action2dGame) {
    if game.hp == 0 || game.iframe_t > 0.0 || game.contact_cd > 0.0 {
        return;
    }
    let Some(w) = player_ref(doc) else {
        return;
    };
    let px = w.position[0];
    let pz = w.position[2];
    let mut hit = false;
    for p in &doc.props {
        if !is_sprite_foe(p) || !p.enabled {
            continue;
        }
        let r = foe_half(p) + 0.46;
        let dx = px - p.position[0];
        let dz = pz - p.position[2];
        if dx * dx + dz * dz <= r * r {
            hit = true;
            break;
        }
    }
    if !hit {
        return;
    }
    game.hp = game.hp.saturating_sub(1);
    game.iframe_t = IFRAME;
    game.contact_cd = CONTACT_CD;
    game.flash_t = HIT_FLASH;
    game.hurt_flash = true;
}

fn die(doc: &mut WorldDoc, game: &mut Action2dGame) {
    game.dead = true;
    game.flash_t = HIT_FLASH;
    game.hurt_flash = true;
    write_beat(doc, game);
    place_side_camera(doc);
}

fn beat_name(game: &Action2dGame) -> &'static str {
    if game.dead {
        NAME_DEAD
    } else if game.flash_t > 0.0 {
        NAME_HURT
    } else {
        NAME_PLAYER
    }
}

fn write_beat(doc: &mut WorldDoc, game: &Action2dGame) {
    let Some(w) = player_ref(doc).cloned() else {
        return;
    };
    let name = beat_name(game);
    write_player(
        doc,
        WorldWalker {
            name: name.into(),
            ..w
        },
    );
    let hero_col = if game.dead {
        HERO_DEAD
    } else if game.flash_t > 0.0 {
        HERO_HURT
    } else {
        HERO_COLOR
    };
    let px = doc.player.as_ref().map(|w| w.position[0]).unwrap_or(0.0);
    let hy = doc.height_at(px, PLANE_Z);
    for p in &mut doc.props {
        if p.id == ID_SCENE {
            p.name = game.scene.clone();
            p.enabled = true;
        }
        if p.name == "flag" || p.id == ID_FLAG {
            p.enabled = game.has_flag(FLAG_DEN);
        }
        if is_hero_card(p) {
            p.enabled = true;
            p.position[0] = px;
            p.position[2] = PLANE_Z;
            p.position[1] = hy + p.scale[1].abs() * 0.5;
            p.color = Some(hero_col);
            let mag = p.scale[0].abs().max(0.4);
            p.scale[0] = if game.facing < 0.0 { -mag } else { mag };
        }
    }
}

fn sit_plane(doc: &mut WorldDoc) {
    let mut updates: Vec<(usize, [f32; 3])> = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        let y = match p.name.as_str() {
            "floor" => doc.floor_y - p.scale[1].abs() * 0.5,
            "wall" => doc.floor_y + p.scale[1].abs() * 0.5,
            "foe" if sprite::is_sprite_prop(p) => {
                doc.height_at(p.position[0], PLANE_Z) + p.scale[1].abs() * 0.5
            }
            NAME_SHOT if sprite::is_sprite_prop(p) => {
                doc.height_at(p.position[0], PLANE_Z) + p.scale[1].abs() * 0.5
            }
            NAME_TRIGGER => doc.floor_y + p.scale[1].abs() * 0.5,
            _ if is_hero_card(p) => doc.height_at(p.position[0], PLANE_Z) + p.scale[1].abs() * 0.5,
            _ => continue,
        };
        let z = if p.name == "wall" {
            p.position[2]
        } else {
            PLANE_Z
        };
        updates.push((i, [p.position[0], y, z]));
    }
    for (i, pos) in updates {
        if let Some(p) = doc.props.get_mut(i) {
            p.position = pos;
            if is_sprite_foe(p) {
                p.color = Some(FOE_COLOR);
            }
        }
    }
    if let Some(w) = player_ref(doc).cloned() {
        let x = w.position[0];
        let y = doc.height_at(x, PLANE_Z) + BODY_H;
        write_player(
            doc,
            WorldWalker {
                name: NAME_PLAYER.into(),
                position: [x, y, PLANE_Z],
                on_ground: true,
                ..w
            },
        );
    }
}

fn foe_half(p: &WorldProp) -> f32 {
    0.5 * p.scale[0].abs().max(0.4)
}

pub fn build_hud(game: &Action2dGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let pad = 16.0 * scale;
    let mut quads = Vec::new();
    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [10, 12, 18, 160]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [22, 24, 32, 230],
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.58,
                w * 0.36,
                52.0 * scale,
                [62, 176, 184, 255],
            ));
        }
        GamePhase::Playing => {
            let pip = 22.0 * scale;
            let gap = 6.0 * scale;
            for i in 0..PLAYER_HP {
                let x = pad + i as f32 * (pip + gap);
                let got = i < game.hp;
                quads.push(Quad::new(
                    x,
                    pad,
                    pip,
                    pip,
                    if got {
                        [220, 64, 64, 255]
                    } else {
                        [28, 16, 16, 160]
                    },
                ));
            }
            let kill_w = 10.0 * scale;
            for i in 0..game.kills.min(12) {
                quads.push(Quad::new(
                    pad + i as f32 * (kill_w + 4.0 * scale),
                    pad + pip + 8.0 * scale,
                    kill_w,
                    kill_w,
                    [240, 196, 72, 255],
                ));
            }
            if game.has_flag(FLAG_DEN) {
                quads.push(Quad::new(
                    pad,
                    pad + pip + 8.0 * scale + kill_w + 6.0 * scale,
                    16.0 * scale,
                    16.0 * scale,
                    [240, 196, 72, 255],
                ));
            }
            if game.scene == SCENE_DEN {
                quads.push(Quad::new(
                    w - pad - 18.0 * scale,
                    pad,
                    18.0 * scale,
                    18.0 * scale,
                    [72, 78, 118, 255],
                ));
            }
            if game.flash_t > 0.0 {
                let a = (90.0 + 140.0 * (game.flash_t / HIT_FLASH)) as u8;
                let col = if game.hurt_flash {
                    [210, 30, 30, a]
                } else {
                    [255, 220, 80, a.min(120)]
                };
                quads.push(Quad::new(0.0, 0.0, w, h, col));
            }
        }
        GamePhase::Complete => {
            if game.dead {
                quads.push(Quad::new(0.0, 0.0, w, h, [40, 6, 6, 190]));
                quads.push(Quad::new(
                    w * 0.18,
                    h * 0.28,
                    w * 0.64,
                    h * 0.22,
                    [70, 12, 12, 230],
                ));
            } else {
                quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 16, 12, 210]));
                quads.push(Quad::new(
                    w * 0.22,
                    h * 0.40,
                    w * 0.56,
                    18.0 * scale,
                    [240, 196, 72, 255],
                ));
            }
            quads.push(Quad::new(
                w * 0.32,
                h * 0.62,
                w * 0.36,
                48.0 * scale,
                if game.dead {
                    [200, 80, 70, 240]
                } else {
                    [70, 160, 110, 240]
                },
            ));
        }
    }
    DrawList {
        clear: [96, 88, 78, 255],
        quads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_doc::MESH_QUAD;
    use crate::world_play::WorldPlay;

    const SIDE: &str = include_str!("../tests/fixtures/action_side_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");
    const CARD: &str = include_str!("../tests/fixtures/sprite_card_world.json");
    const HOP: &str = include_str!("../tests/fixtures/box_hop_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const YARD: &str = include_str!("../tests/fixtures/sim_meter_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(SIDE).unwrap();
        play.confirm();
        play
    }

    fn put_player(play: &mut WorldPlay, x: f32) {
        let y = play.doc.height_at(x, PLANE_Z) + BODY_H;
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [x, y, PLANE_Z];
        }
        let walker = play.doc.player.clone().unwrap();
        write_player(&mut play.doc, walker);
        action2d_sync_for_test(play);
    }

    fn action2d_sync_for_test(play: &mut WorldPlay) {
        write_beat(&mut play.doc, &play.action2d);
    }

    #[test]
    fn dump_is_action2d_not_3d_action_or_sprite_only() {
        let doc = WorldDoc::from_json(SIDE).unwrap();
        assert!(is_action2d(&doc));
        assert_eq!(GAME_ID, "action_side");
        assert!(!crate::platformer::is_platformer(&doc));
        assert!(!crate::sim::is_sim(&doc));
        assert!(!crate::survival::is_survival(&doc));
        assert!(!crate::sports::is_sports(&doc));
        assert!(!crate::fps::is_fps(&doc));
        assert!(crate::sprite::is_sprite(&doc));
        let arena = WorldDoc::from_json(ARENA).unwrap();
        assert!(crate::action::is_action(&arena));
        assert!(!is_action2d(&arena), "3D capsule/box foes are not 2D cards");
        let card = WorldDoc::from_json(CARD).unwrap();
        assert!(crate::sprite::is_sprite(&card));
        assert!(!is_action2d(&card), "sprite card dump has no foe sprite");
        let hop = WorldDoc::from_json(HOP).unwrap();
        assert!(crate::platformer::is_platformer(&hop));
        assert!(!is_action2d(&hop));
        let yard = WorldDoc::from_json(YARD).unwrap();
        assert!(!is_action2d(&yard));
        assert!(doc.props.iter().any(is_sprite_foe));
        assert!(doc.props.iter().any(|p| p.name == "wall"));
        assert!(doc.props.iter().any(|p| p.name == "floor"));
        assert!(doc.props.iter().any(|p| p.name == NAME_TRIGGER));
        assert!(doc.props.iter().any(is_hero_card));
        assert!(
            !crate::rpg::is_rpg(&doc),
            "trigger must not look like an RPG door"
        );
        assert_eq!(doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.batches.iter().any(|b| b.mesh == MESH_QUAD),
            "player + foe cards must compile as MESH_QUAD"
        );
        assert!(scene.instance_count() >= 4, "wall + floor + two cards");
        let json = doc.to_json().unwrap();
        assert!(json.contains("foe"));
        assert!(json.contains("sprite"));
        assert!(json.contains("wall"));
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(SIDE).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_action2d());
        assert!(!play.is_action(), "2D dump must not run 3D action.rs");
        assert!(play.is_sprite());
        assert!(!play.is_platformer());
        assert!(!play.is_sim());
        let start = play.doc.player.as_ref().unwrap().position;
        play.input.lx = 1.0;
        play.input.lz = 1.0;
        play.input.attack = true;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert_eq!(play.doc.player.as_ref().unwrap().position, start);
        assert_eq!(play.action2d.hits, 0);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "side");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn walk_along_x_on_the_plane() {
        let mut play = play_started();
        let x0 = play.doc.player.as_ref().unwrap().position[0];
        play.input.lz = 1.0;
        for _ in 0..45 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap();
        assert!(
            p.position[0] > x0 + 1.2,
            "W / --seconds must walk +X, x0={x0} x={}",
            p.position[0]
        );
        assert!(
            (p.position[2] - PLANE_Z).abs() < 0.05,
            "side-view must stay on the plane, z={}",
            p.position[2]
        );
        assert_eq!(play.doc.cameras[0].name, "side");
        let cam = &play.doc.cameras[0];
        assert!(
            (cam.position[0] - p.position[0]).abs() < 0.2,
            "camera framed on the player card"
        );
        assert!(cam.position[2] > cam.target[2] + 4.0, "look at XY cards");
        let hero = play
            .doc
            .props
            .iter()
            .find(|p| is_hero_card(p))
            .expect("hero card");
        assert!(
            (hero.position[0] - p.position[0]).abs() < 0.05,
            "hero sprite tracks walker"
        );
    }

    #[test]
    fn attack_hits_foe_sprite_and_hurt_is_dump_visible() {
        let mut play = play_started();
        let foe = play
            .doc
            .props
            .iter()
            .find(|p| is_sprite_foe(p) && p.enabled)
            .unwrap()
            .clone();
        put_player(&mut play, foe.position[0] - 0.9);
        play.action2d.facing = 1.0;
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.action2d.hits >= 1, "hits {}", play.action2d.hits);
        assert!(play.action2d.flash_t > 0.0);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_HURT);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("hurt"), "hit must be dump-visible");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 3, "HP pips + flash overlay");
        let scene = play.doc.compile_scene(16.0 / 9.0);
        assert!(scene.batches.iter().any(|b| b.mesh == MESH_QUAD));
    }

    #[test]
    fn two_hits_kill_foe_and_dump_disables_it() {
        let mut play = play_started();
        let foe_id = play
            .doc
            .props
            .iter()
            .find(|p| is_sprite_foe(p) && p.enabled)
            .unwrap()
            .id
            .clone();
        let fx = play
            .doc
            .props
            .iter()
            .find(|p| p.id == foe_id)
            .unwrap()
            .position[0];
        put_player(&mut play, fx - 0.9);
        play.action2d.facing = 1.0;
        for _ in 0..FOE_HP {
            play.input.attack = true;
            play.tick(1.0 / 60.0);
            for _ in 0..20 {
                play.input.attack = false;
                play.tick(1.0 / 60.0);
            }
        }
        let foe = play.doc.props.iter().find(|p| p.id == foe_id).unwrap();
        assert!(!foe.enabled, "killed foe must be disabled in the dump");
        assert!(play.action2d.kills >= 1);
        assert_eq!(play.game.phase, GamePhase::Complete);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains(&foe_id));
    }

    #[test]
    fn contact_then_retry_restores_dump() {
        let mut play = play_started();
        let foe = play
            .doc
            .props
            .iter()
            .find(|p| is_sprite_foe(p) && p.enabled)
            .unwrap()
            .clone();
        put_player(&mut play, foe.position[0]);
        let mut n = 0;
        while play.game.phase == GamePhase::Playing && n < 400 {
            play.input = WalkInput::default();
            play.tick(1.0 / 60.0);
            n += 1;
        }
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert!(play.action2d.dead, "player must die from contact");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_DEAD);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("dead"));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "death overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.action2d.dead);
        assert_eq!(play.action2d.hp, PLAYER_HP);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        let live = play
            .doc
            .props
            .iter()
            .filter(|p| is_sprite_foe(p) && p.enabled)
            .count();
        assert!(live >= 1, "retry must restore foe, live={live}");
    }

    #[test]
    fn other_dumps_keep_their_genres() {
        let arena = WorldPlay::from_json(ARENA).unwrap();
        assert!(arena.is_action());
        assert!(!arena.is_action2d());
        let card = WorldPlay::from_json(CARD).unwrap();
        assert!(card.is_sprite());
        assert!(!card.is_action2d());
        assert!(!card.is_action());
        let hop = WorldPlay::from_json(HOP).unwrap();
        assert!(hop.is_platformer());
        assert!(!hop.is_action2d());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_action2d());
        let yard = WorldPlay::from_json(YARD).unwrap();
        assert!(yard.is_sim());
        assert!(!yard.is_action2d());
    }

    #[test]
    fn fire_from_range_spawns_dump_visible_shot_that_hits() {
        let mut play = play_started();
        let foe_x = play
            .doc
            .props
            .iter()
            .find(|p| is_sprite_foe(p) && p.enabled)
            .unwrap()
            .position[0];
        put_player(&mut play, foe_x - 4.2);
        play.action2d.facing = 1.0;
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        let shot = play
            .doc
            .props
            .iter()
            .find(|p| p.name == NAME_SHOT)
            .expect("shot prop");
        assert!(
            shot.enabled,
            "J/click from range must spawn a dump-visible shot"
        );
        assert!(sprite::is_sprite_prop(shot));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("shot"));
        let x0 = shot.position[0];
        play.input.attack = false;
        play.tick(1.0 / 60.0);
        let shot = play.doc.props.iter().find(|p| p.name == NAME_SHOT).unwrap();
        assert!(
            shot.position[0] > x0 + 0.05,
            "shot must move, x0={x0} x={}",
            shot.position[0]
        );
        let mut n = 0;
        while play.action2d.hits == 0 && n < 180 {
            play.input.attack = false;
            play.tick(1.0 / 60.0);
            n += 1;
        }
        assert!(play.action2d.hits >= 1, "moving shot must hit, n={n}");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_HURT);
        let shot = play.doc.props.iter().find(|p| p.name == NAME_SHOT).unwrap();
        assert!(!shot.enabled, "shot is consumed on hit");
    }

    #[test]
    fn crossing_trigger_switches_hall_and_den() {
        let mut play = play_started();
        assert_eq!(play.action2d.scene, SCENE_HALL);
        assert!(!play.is_rpg());
        let hall_wall = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "wall")
            .unwrap()
            .color;
        let tx = play
            .doc
            .props
            .iter()
            .find(|p| p.name == NAME_TRIGGER)
            .unwrap()
            .position[0];
        put_player(&mut play, tx);
        play.input = Default::default();
        play.tick(1.0 / 60.0);
        assert_eq!(play.action2d.scene, SCENE_DEN);
        assert!(play.is_action2d());
        assert!(play.doc.props.iter().any(is_hero_card));
        assert!(play.doc.props.iter().any(|p| is_sprite_foe(p) && p.enabled));
        assert_eq!(play.doc.cameras[0].name, "side");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("den"), "scene name must be dump-visible");
        assert!(play
            .doc
            .props
            .iter()
            .any(|p| p.id == ID_SCENE && p.name == SCENE_DEN));
        assert!(play.doc.props.iter().any(|p| p.name == "flag" && p.enabled));
        let den_wall = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "wall")
            .unwrap()
            .color;
        assert_ne!(den_wall, hall_wall, "rooms must look distinct");
        let px = play.doc.player.as_ref().unwrap().position[0];
        assert!(
            (px - DEN_SPAWN_X).abs() < 0.3,
            "park away from the return trigger, px={px}"
        );

        for _ in 0..40 {
            play.input = Default::default();
            play.tick(1.0 / 60.0);
        }
        let tx = play
            .doc
            .props
            .iter()
            .find(|p| p.name == NAME_TRIGGER)
            .unwrap()
            .position[0];
        put_player(&mut play, tx);
        play.tick(1.0 / 60.0);
        assert_eq!(play.action2d.scene, SCENE_HALL);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("hall"));
        assert!(play.doc.props.iter().any(|p| p.name == "flag" && p.enabled));
        assert_eq!(play.doc.cameras[0].name, "side");
        assert!(play.doc.props.iter().any(is_hero_card));
    }

    #[test]
    fn den_fixture_is_action2d_not_rpg() {
        const DEN: &str = include_str!("../tests/fixtures/action_side_den_world.json");
        let doc = WorldDoc::from_json(DEN).unwrap();
        assert!(is_action2d(&doc));
        assert!(is_den(&doc));
        assert!(!crate::rpg::is_rpg(&doc));
        let play = WorldPlay::from_json(DEN).unwrap();
        assert!(play.is_action2d());
        assert!(!play.is_rpg());
        assert!(
            !play.is_action(),
            "WorldPlay must keep 3D action.rs off sprite foes"
        );
        assert_eq!(play.action2d.scene, SCENE_DEN);
    }
}
