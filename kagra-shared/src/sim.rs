//! A meter ticks while the player stands in a zone as an M3 slice on play_world.
//!
//! Sibling of sports / puzzle. Player walks a capsule; a box is the zone.
//! Standing in the zone fills `coins` (the meter). Full meter enables the
//! flag. Title -> play -> result reuses `WorldPlay` / `GamePhase`. Dump-visible
//! name (player/filling/full) and flag + coins. Capsules/boxes, not VRM.
//! Indoor lights stay 4 slots. Zone stays readable (contact blob plus metal
//! GGX inherited). Chase camera follows play. Does not rewrite other genre
//! loops. No Unity/Godot API, no enemy.chase, no avatar.state, no world.spawn
//! (those names are not in docs/API_INDEX.md). No Rapier, inventory, or ECS.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};

pub const GAME_ID: &str = "sim_meter";
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const NEED: u32 = 8;
pub const FILL_PER_SEC: f32 = 5.0;
pub const CAM_BACK: f32 = 7.2;
pub const CAM_UP: f32 = 4.8;
pub const CAM_LOOK: f32 = 0.55;
pub const NAME_PLAYER: &str = "player";
pub const NAME_FILLING: &str = "filling";
pub const NAME_FULL: &str = "full";

const FLAG_FULL_COLOR: [u32; 3] = [70, 180, 110];
const FILL_PIP: [u8; 4] = [240, 196, 72, 255];
const FULL_PIP: [u8; 4] = [70, 180, 110, 255];

/// Live sim around a dump. Filling/full stay here; `name` + flag enable
/// and `coins` in the dump are the query source of truth. Meter is time
/// in the zone, not Rapier.
#[derive(Clone, Debug, Default)]
pub struct SimGame {
    pub filling: bool,
    pub full: bool,
    pub done: bool,
    pub held_s: f32,
    pub coins: u32,
}

impl SimGame {
    pub fn from_doc(_doc: &WorldDoc) -> Self {
        Self::default()
    }
}

pub fn is_sim(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "zone")
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

/// Sit capsules/boxes on the floor, keep the flag off, chase cam.
pub fn seed(doc: &mut WorldDoc) {
    if !is_sim(doc) {
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
    place_chase_camera(doc);
}

/// Chase cam behind the player looking at the zone so the pad reads.
pub fn place_chase_camera(doc: &mut WorldDoc) {
    let Some(w) = player_ref(doc) else {
        return;
    };
    let px = w.position[0];
    let py = w.position[1];
    let pz = w.position[2];
    let yaw = w.yaw;
    let (s, c) = yaw.sin_cos();
    let (zx, zy, zz) = named_prop(doc, "zone")
        .map(|p| (p.position[0], p.position[1], p.position[2]))
        .unwrap_or((px, py, pz - 2.0));
    let eye = [px - s * CAM_BACK, py + CAM_UP, pz - c * CAM_BACK];
    let target = [
        zx * 0.55 + px * 0.45,
        zy.max(CAM_LOOK),
        zz * 0.55 + pz * 0.45,
    ];
    let fov = doc.cameras.first().map(|cam| cam.fov).unwrap_or(52.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye;
        cam.target = target;
        cam.name = "chase".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "chase".into(),
            position: eye,
            target,
            fov,
        });
    }
}

/// Meter ticks while the player stands in the zone. Caller already walked;
/// chase cam wins.
pub fn tick(doc: &mut WorldDoc, game: &mut SimGame, _input: WalkInput, dt: f32) {
    if game.done {
        write_beat(doc, game);
        place_chase_camera(doc);
        return;
    }
    let inside = player_in_zone(doc);
    game.filling = inside && !game.full;
    if inside {
        game.held_s += dt;
        let coins = ((game.held_s * FILL_PER_SEC).floor() as u32).min(NEED);
        game.coins = coins;
        if coins >= NEED {
            game.full = true;
            game.done = true;
            game.filling = false;
            game.held_s = NEED as f32 / FILL_PER_SEC;
            game.coins = NEED;
        }
    }
    write_beat(doc, game);
    place_chase_camera(doc);
}

fn beat_name(game: &SimGame) -> &'static str {
    if game.done || game.full {
        NAME_FULL
    } else if game.filling {
        NAME_FILLING
    } else {
        NAME_PLAYER
    }
}

fn write_beat(doc: &mut WorldDoc, game: &SimGame) {
    sit_named(doc, beat_name(game));
    doc.coins = if game.done || game.full {
        NEED
    } else {
        game.coins
    };
    if game.done || game.full {
        set_flag_prop(doc);
    }
}

fn set_flag_prop(doc: &mut WorldDoc) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = true;
            p.color = Some(FLAG_FULL_COLOR);
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
            "zone" | "flag" | "floor" | "post" => p.scale[1].abs() * 0.5,
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

fn player_in_zone(doc: &WorldDoc) -> bool {
    let Some(player) = player_ref(doc) else {
        return false;
    };
    let Some(zone) = named_prop(doc, "zone") else {
        return false;
    };
    let hx = zone.scale[0].abs() * 0.5 + PLAYER_R;
    let hz = zone.scale[2].abs() * 0.5 + PLAYER_R;
    let dx = player.position[0] - zone.position[0];
    let dz = player.position[2] - zone.position[2];
    dx.abs() <= hx && dz.abs() <= hz
}

pub fn build_hud(game: &SimGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let mut quads = Vec::new();
    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [12, 14, 18, 160]));
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
                [240, 196, 72, 255],
            ));
        }
        GamePhase::Playing => {
            let frac = (game.coins as f32 / NEED as f32).clamp(0.0, 1.0);
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                160.0 * scale,
                18.0 * scale,
                [28, 24, 20, 160],
            ));
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                (160.0 * scale * frac).max(if frac > 0.0 { 2.0 } else { 0.0 }),
                18.0 * scale,
                if game.filling {
                    FILL_PIP
                } else {
                    [80, 84, 78, 200]
                },
            ));
            quads.push(Quad::new(
                184.0 * scale,
                16.0 * scale,
                22.0 * scale,
                22.0 * scale,
                if game.full {
                    FULL_PIP
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
        clear: [24, 28, 34, 255],
        quads,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const YARD: &str = include_str!("../tests/fixtures/sim_meter_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const TOWN: &str = include_str!("../tests/fixtures/rpg_town_world.json");
    const PAGES: &str = include_str!("../tests/fixtures/novel_pages_world.json");
    const HIDE: &str = include_str!("../tests/fixtures/stealth_hide_world.json");
    const RING: &str = include_str!("../tests/fixtures/fight_hitstun_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");
    const ROOM: &str = include_str!("../tests/fixtures/puzzle_pad_world.json");
    const HOP: &str = include_str!("../tests/fixtures/box_hop_world.json");
    const PITCH: &str = include_str!("../tests/fixtures/sports_goal_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(YARD).unwrap();
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
    fn dump_is_sim_not_other_genres() {
        let doc = WorldDoc::from_json(YARD).unwrap();
        assert!(is_sim(&doc));
        assert_eq!(GAME_ID, "sim_meter");
        assert!(!crate::puzzle::is_puzzle(&doc));
        assert!(!crate::stealth::is_stealth(&doc));
        assert!(!crate::novel::is_novel(&doc));
        assert!(!crate::rpg::is_rpg(&doc));
        assert!(!crate::fight::is_fight(&doc));
        assert!(!crate::action::is_action(&doc));
        assert!(!crate::fps::is_fps(&doc));
        assert!(!crate::td::is_td(&doc));
        assert!(!crate::race::is_race(&doc));
        assert!(!crate::platformer::is_platformer(&doc));
        assert!(!crate::sports::is_sports(&doc));
        assert!(!crate::survival::is_survival(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_sim(&crest));
        let hop = WorldDoc::from_json(HOP).unwrap();
        assert!(crate::platformer::is_platformer(&hop));
        assert!(!is_sim(&hop));
        let room = WorldDoc::from_json(ROOM).unwrap();
        assert!(!is_sim(&room));
        let pitch = WorldDoc::from_json(PITCH).unwrap();
        assert!(crate::sports::is_sports(&pitch));
        assert!(!is_sim(&pitch), "sports goal must not count as sim");
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "zone" && p.model == "box"));
        assert!(doc.props.iter().any(|p| p.name == "flag" && !p.enabled));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "floor" && p.model == "box"));
        let zone = doc.props.iter().find(|p| p.name == "zone").unwrap();
        assert!(
            zone.metallic >= 0.5,
            "zone rim should read metal GGX, metallic={}",
            zone.metallic
        );
        assert_eq!(doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("zone"));
        assert!(json.contains("flag"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 3,
            "floor + zone must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(YARD).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_sim());
        assert!(!play.is_puzzle());
        assert!(!play.is_stealth());
        assert!(!play.is_sports());
        assert!(!play.is_platformer());
        assert!(!play.is_collectathon());
        let start = play.doc.player.as_ref().unwrap().position;
        play.input.lx = 1.0;
        play.input.lz = 1.0;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.sim.done, "title must not fill");
        assert_eq!(play.doc.player.as_ref().unwrap().position, start);
        assert_eq!(play.doc.coins, 0);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "chase");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn stand_in_zone_ticks_coins() {
        let mut play = play_started();
        let zone = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "zone")
            .unwrap()
            .position;
        put_player(&mut play, zone[0], zone[2]);
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        assert!(play.sim.filling, "standing in zone must fill");
        assert!(!play.sim.done);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_FILLING);
        assert!(
            play.doc.coins >= 1,
            "meter should tick, coins={}",
            play.doc.coins
        );
        assert_eq!(play.sim.coins, play.doc.coins);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("filling"), "filling must be dump-visible");
        assert_eq!(play.doc.cameras[0].name, "chase");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "filling meter overlay");
    }

    #[test]
    fn leave_zone_holds_meter() {
        let mut play = play_started();
        let zone = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "zone")
            .unwrap()
            .position;
        put_player(&mut play, zone[0], zone[2]);
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        let held = play.doc.coins;
        assert!(held >= 1);
        put_player(&mut play, zone[0], zone[2] + 6.0);
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.sim.filling, "outside zone must not fill");
        assert_eq!(play.doc.coins, held, "meter holds when leaving");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert!(!play.sim.done);
    }

    #[test]
    fn fill_meter_enables_flag() {
        let mut play = play_started();
        let zone = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "zone")
            .unwrap()
            .position;
        put_player(&mut play, zone[0], zone[2]);
        for _ in 0..150 {
            play.tick(1.0 / 60.0);
            if play.sim.done {
                break;
            }
        }
        assert!(play.sim.full);
        assert!(play.sim.done);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_FULL);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "full flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_FULL_COLOR));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("full"));
        assert_eq!(play.doc.coins, NEED);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "full overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.sim.done);
        assert!(!play.sim.full);
        assert!(!play.sim.filling);
        assert_eq!(play.doc.coins, 0);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "retry restores flag off");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
    }

    #[test]
    fn walk_forward_fills_from_spawn() {
        let mut play = play_started();
        let cam0 = play.doc.cameras[0].position;
        play.input.lz = 1.0;
        for _ in 0..240 {
            play.tick(1.0 / 60.0);
            if play.sim.done {
                break;
            }
        }
        assert!(
            play.sim.full && play.sim.done,
            "W from spawn should enter zone and fill, filling={} coins={} pos={:?}",
            play.sim.filling,
            play.doc.coins,
            play.doc.player.as_ref().unwrap().position
        );
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_FULL);
        assert_eq!(play.doc.cameras[0].name, "chase");
        let cam1 = play.doc.cameras[0].position;
        let d = (cam1[0] - cam0[0]).abs() + (cam1[2] - cam0[2]).abs();
        assert!(d > 0.2, "camera should follow play, delta={d}");
    }

    #[test]
    fn other_genres_still_own_their_dumps() {
        let town = WorldPlay::from_json(TOWN).unwrap();
        assert!(town.is_rpg());
        assert!(!town.is_sim());
        let novel = WorldPlay::from_json(PAGES).unwrap();
        assert!(novel.is_novel());
        assert!(!novel.is_sim());
        let stealth = WorldPlay::from_json(HIDE).unwrap();
        assert!(stealth.is_stealth());
        assert!(!stealth.is_sim());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_sim());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_sim());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_sim());
        let puzzle = WorldPlay::from_json(ROOM).unwrap();
        assert!(puzzle.is_puzzle());
        assert!(!puzzle.is_sim());
        let hop = WorldPlay::from_json(HOP).unwrap();
        assert!(hop.is_platformer());
        assert!(!hop.is_sim());
        let sports = WorldPlay::from_json(PITCH).unwrap();
        assert!(sports.is_sports());
        assert!(!sports.is_sim());
    }
}
