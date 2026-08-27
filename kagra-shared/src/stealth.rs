//! Hide volume + guard facing cone + clear/caught on play_world.
//!
//! Sibling of action / novel. Player walks a capsule; a box/prop is a hide
//! volume; a guard capsule has a facing cone. Unseen reach of the exit is
//! clear; seen is caught. Title -> play -> result reuses `WorldPlay` /
//! `GamePhase`. Dump-visible `name` (player/hidden/clear/caught) + flag.
//! Capsules/boxes, not VRM. Indoor lights stay 4 slots. Does not rewrite
//! other genre loops. No patrol editor, noise meter, net, inventory, or VN.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldWalker};

pub const GAME_ID: &str = "stealth_hide";
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const CONE_RANGE: f32 = 6.0;
pub const CONE_HALF: f32 = 0.68;
pub const FLAG_CLEAR: &str = "clear";
pub const FLAG_CAUGHT: &str = "caught";
pub const NAME_HIDDEN: &str = "hidden";
pub const NAME_PLAYER: &str = "player";

const FLAG_CLEAR_COLOR: [u32; 3] = [70, 180, 110];
const FLAG_CAUGHT_COLOR: [u32; 3] = [200, 64, 56];
const HIDDEN_PIP: [u8; 4] = [90, 70, 48, 255];

/// Live stealth around a dump. Hidden/seen/clear stay here; `name` + flag
/// enable in the dump are the query source of truth.
#[derive(Clone, Debug, Default)]
pub struct StealthGame {
    pub hidden: bool,
    pub seen: bool,
    pub done: bool,
    pub clear: bool,
}

impl StealthGame {
    pub fn from_doc(_doc: &WorldDoc) -> Self {
        Self::default()
    }
}

pub fn is_stealth(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "hide")
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

/// Fixed room camera so the hide box and guard stay readable (not chase).
pub fn place_room_camera(doc: &mut WorldDoc) {
    let eye = [0.15, 5.1, 8.2];
    let target = [0.0, 0.9, -0.2];
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

/// Hide / cone / exit tick. Caller already walked; room camera wins.
pub fn tick(doc: &mut WorldDoc, game: &mut StealthGame, _input: WalkInput, _dt: f32) {
    if game.done {
        write_beat(doc, game);
        place_room_camera(doc);
        return;
    }
    game.hidden = in_hide(doc);
    if !game.hidden && in_guard_cone(doc) {
        game.seen = true;
        game.done = true;
        game.clear = false;
    } else if in_exit(doc) && !game.seen {
        game.done = true;
        game.clear = true;
    }
    write_beat(doc, game);
    place_room_camera(doc);
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
    sit_named(doc, beat_name(game));
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
            ..Default::default()
        },
    );
}

fn sit_props(doc: &mut WorldDoc) {
    let mut updates: Vec<(usize, f32)> = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        let extra = match p.name.as_str() {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const ROOM: &str = include_str!("../tests/fixtures/stealth_hide_world.json");
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
}
