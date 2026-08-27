//! 2D action on play_world: side-view sprite walk, hit, hurt, kill.
//!
//! Sibling of 3D `action` / `sprite`. Player card and foe card are the same
//! `model: "sprite"` / `"quad"` WorldDoc path (`MESH_QUAD` in `compile_scene`).
//! Walk along X on a back wall + floor; J hits the foe sprite. Hurt / kill
//! are dump-visible (`name` + foe `enabled`). Title -> play -> result reuses
//! `WorldPlay` / `GamePhase`. Overlay flash is `DrawList` quads on shared
//! wgpu 30. Does not rewrite 3D `action.rs`. No new ECS, no RendererV2, no
//! Rapier, no VRM, no billboards, no net. No `enemy.chase` (not in
//! docs/API_INDEX.md).

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
    foe_hp: u32,
    swing_hit: bool,
    facing: f32,
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
            foe_hp: FOE_HP,
            swing_hit: false,
            facing: 1.0,
        }
    }
}

impl Action2dGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let mut g = Self::default();
        if live_foes(doc) == 0 {
            g.foe_hp = 0;
        }
        g
    }
}

pub fn is_action2d(doc: &WorldDoc) -> bool {
    doc.props.iter().any(is_sprite_foe)
}

fn is_sprite_foe(p: &WorldProp) -> bool {
    p.name == "foe" && sprite::is_sprite_prop(p)
}

fn is_hero_card(p: &WorldProp) -> bool {
    sprite::is_sprite_prop(p) && p.name != "foe" && p.name != "wall" && p.name != "floor"
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
    sit_plane(doc);
    place_side_camera(doc);
}

pub fn place_side_camera(doc: &mut WorldDoc) {
    let (px, py) = player_ref(doc)
        .map(|w| (w.position[0], w.position[1]))
        .unwrap_or((0.0, BODY_H));
    let eye = [px, py.max(CAM_Y), CAM_Z];
    let target = [px, CAM_LOOK_Y.max(0.4), PLANE_Z];
    let fov = doc
        .cameras
        .first()
        .map(|cam| cam.fov)
        .unwrap_or(CAM_FOV);
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
    if game.flash_t <= 0.0 && !game.dead {
        game.hurt_flash = false;
    }

    apply_walk(doc, game, input, dt);
    apply_attack(doc, game, input);
    apply_contact(doc, game);
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
        },
    );
}

fn apply_attack(doc: &mut WorldDoc, game: &mut Action2dGame, input: WalkInput) {
    if input.attack && game.attack_t <= 0.0 {
        game.attack_t = ATTACK_TIME;
        game.swing_hit = false;
    }
    if game.attack_t <= 0.0 || game.swing_hit {
        return;
    }
    let Some(w) = player_ref(doc) else {
        return;
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
        return;
    };
    game.swing_hit = true;
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
        assert!(doc.props.iter().any(is_hero_card));
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
}
