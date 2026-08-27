//! Push a box onto a pad as M3 genre on play_world.
//!
//! Sibling of stealth / novel. Player walks a capsule; a box/prop is a
//! kinematic crate; a pad box is the goal. Overlap pushes the crate (no
//! Rapier). Crate on pad is solved. Title -> play -> result reuses
//! `WorldPlay` / `GamePhase`. Dump-visible name (player/pushing/solved)
//! and flag. Capsules/boxes, not VRM. Indoor lights stay 4 slots. Pad and
//! crate stay readable (contact blob plus metal GGX inherited). Does not
//! rewrite other genre loops. No sokoban editor, joints, net, inventory,
//! or vehicles.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};

pub const GAME_ID: &str = "puzzle_pad";
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const NAME_PLAYER: &str = "player";
pub const NAME_PUSHING: &str = "pushing";
pub const NAME_SOLVED: &str = "solved";

const FLAG_SOLVED_COLOR: [u32; 3] = [70, 180, 110];
const PUSH_PIP: [u8; 4] = [240, 196, 72, 255];
const SOLVED_PIP: [u8; 4] = [70, 180, 110, 255];

/// Live puzzle around a dump. Pushing/solved stay here; `name` + flag
/// enable in the dump are the query source of truth.
#[derive(Clone, Debug, Default)]
pub struct PuzzleGame {
    pub pushing: bool,
    pub solved: bool,
    pub done: bool,
}

impl PuzzleGame {
    pub fn from_doc(_doc: &WorldDoc) -> Self {
        Self::default()
    }
}

pub fn is_puzzle(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "pad")
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
    if !is_puzzle(doc) {
        return;
    }
    sit_named(doc, NAME_PLAYER);
    sit_props(doc);
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = false;
        }
    }
    doc.coins = 0;
    place_room_camera(doc);
}

/// Fixed room camera so the pad and crate stay readable (not chase).
pub fn place_room_camera(doc: &mut WorldDoc) {
    let eye = [0.0, 5.2, 8.2];
    let target = [0.0, 0.7, -0.4];
    let fov = doc.cameras.first().map(|c| c.fov).unwrap_or(48.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye;
        cam.target = target;
        cam.name = "room".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "room".into(),
            position: eye,
            target,
            fov,
        });
    }
}

/// Kinematic push + pad tick. Caller already walked; room camera wins.
pub fn tick(doc: &mut WorldDoc, game: &mut PuzzleGame, _input: WalkInput, _dt: f32) {
    if game.done {
        write_beat(doc, game);
        place_room_camera(doc);
        return;
    }
    game.pushing = push_crate(doc);
    if crate_on_pad(doc) {
        game.solved = true;
        game.done = true;
        game.pushing = false;
    }
    write_beat(doc, game);
    place_room_camera(doc);
}

fn beat_name(game: &PuzzleGame) -> &'static str {
    if game.done || game.solved {
        NAME_SOLVED
    } else if game.pushing {
        NAME_PUSHING
    } else {
        NAME_PLAYER
    }
}

fn write_beat(doc: &mut WorldDoc, game: &PuzzleGame) {
    sit_named(doc, beat_name(game));
    doc.coins = if game.done || game.solved {
        10
    } else if game.pushing {
        1
    } else {
        0
    };
    if game.done || game.solved {
        set_flag_prop(doc);
    }
}

fn set_flag_prop(doc: &mut WorldDoc) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = true;
            p.color = Some(FLAG_SOLVED_COLOR);
        }
    }
}

fn sit_named(doc: &mut WorldDoc, name: &str) {
    let Some(p) = player_ref(doc) else {
        return;
    };
    let id = p.id.clone();
    let x = p.position[0];
    let z = p.position[2];
    let yaw = p.yaw;
    let y = doc.height_at(x, z) + BODY_H;
    write_player(
        doc,
        WorldWalker {
            id,
            kind: "walker".into(),
            name: name.into(),
            position: [x, y, z],
            yaw,
            face: yaw,
            on_ground: true,
        },
    );
}

fn sit_props(doc: &mut WorldDoc) {
    let mut updates: Vec<(usize, f32)> = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        let extra = match p.name.as_str() {
            "crate" | "pad" | "flag" => p.scale[1].abs() * 0.5,
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

fn named_prop<'a>(doc: &'a WorldDoc, name: &str) -> Option<&'a WorldProp> {
    doc.props.iter().find(|p| p.name == name && p.enabled)
}

fn named_prop_index(doc: &WorldDoc, name: &str) -> Option<usize> {
    doc.props.iter().position(|p| p.name == name && p.enabled)
}

/// Circle (player) vs crate AABB on XZ. Move the crate by the penetration.
/// Returns true when overlap was resolved this tick.
fn push_crate(doc: &mut WorldDoc) -> bool {
    let Some(player) = player_ref(doc) else {
        return false;
    };
    let px = player.position[0];
    let pz = player.position[2];
    let Some(idx) = named_prop_index(doc, "crate") else {
        return false;
    };
    let crate_prop = &doc.props[idx];
    let hx = crate_prop.scale[0].abs() * 0.5;
    let hz = crate_prop.scale[2].abs() * 0.5;
    let cx = crate_prop.position[0];
    let cz = crate_prop.position[2];
    let closest_x = px.clamp(cx - hx, cx + hx);
    let closest_z = pz.clamp(cz - hz, cz + hz);
    let dx = px - closest_x;
    let dz = pz - closest_z;
    let dist_sq = dx * dx + dz * dz;
    let r = PLAYER_R;
    if dist_sq >= r * r {
        return false;
    }
    let (nx, nz, pen) = if dist_sq < 1e-8 {
        let ax = cx - px;
        let az = cz - pz;
        let len = (ax * ax + az * az).sqrt();
        if len < 1e-6 {
            (0.0, -1.0, r)
        } else {
            (ax / len, az / len, r)
        }
    } else {
        let dist = dist_sq.sqrt();
        // Normal from crate surface toward player; crate moves the other way.
        (-dx / dist, -dz / dist, r - dist)
    };
    let pen = pen + 0.02;
    // Prefer a cardinal push so the crate stays on the pad line.
    let (nx, nz) = if nx.abs() >= nz.abs() {
        (nx.signum(), 0.0)
    } else {
        (0.0, nz.signum())
    };
    let (nx, nz) = if nx == 0.0 && nz == 0.0 {
        (0.0, -1.0)
    } else {
        (nx, nz)
    };
    let mut new_x = cx + nx * pen;
    let mut new_z = cz + nz * pen;
    let half = doc.half.max(4.0);
    let pad = 2.2;
    new_x = new_x.clamp(-half + pad, half - pad);
    new_z = new_z.clamp(-half + pad, half - pad);
    if overlaps_wall_at(doc, new_x, new_z, hx, hz) {
        if overlaps_wall_at(doc, cx, new_z, hx, hz) {
            new_z = cz;
        }
        if overlaps_wall_at(doc, new_x, cz, hx, hz) {
            new_x = cx;
        }
        if overlaps_wall_at(doc, new_x, new_z, hx, hz) {
            return true;
        }
    }
    let sy = doc.props[idx].scale[1].abs();
    let y = doc.height_at(new_x, new_z) + sy * 0.5;
    if let Some(p) = doc.props.get_mut(idx) {
        p.position[0] = new_x;
        p.position[1] = y;
        p.position[2] = new_z;
    }
    true
}

fn overlaps_wall_at(doc: &WorldDoc, x: f32, z: f32, hx: f32, hz: f32) -> bool {
    doc.props.iter().any(|w| {
        if w.name != "wall" || !w.enabled {
            return false;
        }
        let wx = w.scale[0].abs() * 0.5;
        let wz = w.scale[2].abs() * 0.5;
        (x - w.position[0]).abs() <= hx + wx && (z - w.position[2]).abs() <= hz + wz
    })
}

fn crate_on_pad(doc: &WorldDoc) -> bool {
    let Some(crate_prop) = named_prop(doc, "crate") else {
        return false;
    };
    let Some(pad) = named_prop(doc, "pad") else {
        return false;
    };
    let hx = pad.scale[0].abs() * 0.5;
    let hz = pad.scale[2].abs() * 0.5;
    (crate_prop.position[0] - pad.position[0]).abs() <= hx * 0.55
        && (crate_prop.position[2] - pad.position[2]).abs() <= hz * 0.55
}

pub fn build_hud(game: &PuzzleGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
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
                if game.pushing {
                    PUSH_PIP
                } else {
                    [28, 24, 20, 160]
                },
            ));
            quads.push(Quad::new(
                44.0 * scale,
                16.0 * scale,
                22.0 * scale,
                22.0 * scale,
                if game.solved {
                    SOLVED_PIP
                } else {
                    [36, 48, 40, 160]
                },
            ));
        }
        GamePhase::Complete => {
            quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 28, 18, 210]));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.62,
                w * 0.36,
                48.0 * scale,
                [70, 180, 110, 240],
            ));
        }
    }
    DrawList {
        clear: [28, 34, 36, 255],
        quads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const ROOM: &str = include_str!("../tests/fixtures/puzzle_pad_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const TOWN: &str = include_str!("../tests/fixtures/rpg_town_world.json");
    const PAGES: &str = include_str!("../tests/fixtures/novel_pages_world.json");
    const HIDE: &str = include_str!("../tests/fixtures/stealth_hide_world.json");
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

    fn crate_xz(play: &WorldPlay) -> (f32, f32) {
        let p = play.doc.props.iter().find(|p| p.name == "crate").unwrap();
        (p.position[0], p.position[2])
    }

    fn put_crate(play: &mut WorldPlay, x: f32, z: f32) {
        let y = play.doc.height_at(x, z);
        for p in &mut play.doc.props {
            if p.name == "crate" {
                p.position[0] = x;
                p.position[2] = z;
                p.position[1] = y + p.scale[1].abs() * 0.5;
            }
        }
    }

    #[test]
    fn dump_is_puzzle_not_other_genres() {
        let doc = WorldDoc::from_json(ROOM).unwrap();
        assert!(is_puzzle(&doc));
        assert_eq!(GAME_ID, "puzzle_pad");
        assert!(!crate::stealth::is_stealth(&doc));
        assert!(!crate::novel::is_novel(&doc));
        assert!(!crate::rpg::is_rpg(&doc));
        assert!(!crate::fight::is_fight(&doc));
        assert!(!crate::action::is_action(&doc));
        assert!(!crate::fps::is_fps(&doc));
        assert!(!crate::td::is_td(&doc));
        assert!(!crate::race::is_race(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_puzzle(&crest));
        let town = WorldDoc::from_json(TOWN).unwrap();
        assert!(!is_puzzle(&town));
        let pages = WorldDoc::from_json(PAGES).unwrap();
        assert!(!is_puzzle(&pages));
        let hide = WorldDoc::from_json(HIDE).unwrap();
        assert!(!is_puzzle(&hide));
        let ring = WorldDoc::from_json(RING).unwrap();
        assert!(!is_puzzle(&ring));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "pad" && p.model == "box"));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "crate" && p.model == "box"));
        let crate_prop = doc.props.iter().find(|p| p.name == "crate").unwrap();
        assert!(
            crate_prop.metallic >= 0.5,
            "crate should read metal GGX, metallic={}",
            crate_prop.metallic
        );
        assert!(doc.props.iter().any(|p| p.name == "flag" && !p.enabled));
        assert_eq!(doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("pad"));
        assert!(json.contains("crate"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 6,
            "pad + crate must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(ROOM).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_puzzle());
        assert!(!play.is_stealth());
        assert!(!play.is_novel());
        assert!(!play.is_rpg());
        assert!(!play.is_fight());
        assert!(!play.is_collectathon());
        let start = play.doc.player.as_ref().unwrap().position;
        let crate0 = crate_xz(&play);
        play.input.lx = 1.0;
        play.input.lz = 1.0;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.puzzle.done, "title must not solve");
        assert_eq!(play.doc.player.as_ref().unwrap().position, start);
        assert_eq!(crate_xz(&play), crate0);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "room");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn walk_into_crate_pushes_it() {
        let mut play = play_started();
        let (cx, cz) = crate_xz(&play);
        put_player(&mut play, cx, cz + 0.7);
        play.tick(1.0 / 60.0);
        assert!(play.puzzle.pushing, "overlap must push");
        assert!(!play.puzzle.done);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PUSHING);
        let (cx2, cz2) = crate_xz(&play);
        assert!(
            (cz2 - cz).abs() > 0.02 || (cx2 - cx).abs() > 0.02,
            "crate should move, before=({cx},{cz}) after=({cx2},{cz2})"
        );
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("pushing"), "pushing must be dump-visible");
        assert_eq!(play.doc.coins, 1);
        assert_eq!(play.doc.cameras[0].name, "room");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "pushing pip overlay");
    }

    #[test]
    fn crate_on_pad_is_solved() {
        let mut play = play_started();
        let pad = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "pad")
            .unwrap()
            .position;
        put_crate(&mut play, pad[0], pad[2]);
        play.tick(1.0 / 60.0);
        assert!(play.puzzle.solved);
        assert!(play.puzzle.done);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_SOLVED);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "solved flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_SOLVED_COLOR));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("solved"));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "solved overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.puzzle.done);
        assert!(!play.puzzle.solved);
        assert!(!play.puzzle.pushing);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "retry restores flag off");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
    }

    #[test]
    fn walk_forward_pushes_crate_onto_pad() {
        let mut play = play_started();
        play.input.lz = 1.0;
        for _ in 0..180 {
            play.tick(1.0 / 60.0);
            if play.puzzle.done {
                break;
            }
        }
        assert!(
            play.puzzle.solved && play.puzzle.done,
            "W from spawn should push crate onto pad, pushing={} crate={:?}",
            play.puzzle.pushing,
            crate_xz(&play)
        );
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_SOLVED);
        assert_eq!(play.doc.cameras[0].name, "room");
    }

    #[test]
    fn other_genres_still_own_their_dumps() {
        let town = WorldPlay::from_json(TOWN).unwrap();
        assert!(town.is_rpg());
        assert!(!town.is_puzzle());
        let novel = WorldPlay::from_json(PAGES).unwrap();
        assert!(novel.is_novel());
        assert!(!novel.is_puzzle());
        let stealth = WorldPlay::from_json(HIDE).unwrap();
        assert!(stealth.is_stealth());
        assert!(!stealth.is_puzzle());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_puzzle());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_puzzle());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_puzzle());
    }
}
