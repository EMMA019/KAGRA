//! Cast + catch as an M3 slice on play_world.
//!
//! Sibling of rhythm. Player stands a capsule on a dock; J / click
//! (`WalkInput.attack`) casts; after a short wait a bite; J / click lands
//! dump-visible `catch` (`coins` / flag / `name`). Title -> play -> result
//! reuses `WorldPlay` / `GamePhase`. Capsules/boxes, not VRM. Indoor lights
//! stay 4 slots. Official `WorldDoc.water_y` plane + metal dock stay
//! readable (contact blob plus metal GGX inherited). Dock camera looks at
//! water + dock so the picture holds. Does not rewrite other genre loops.
//! No Rapier, SSAO, GI, inventory, net, or ECS. Picture slice stays.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};

pub const GAME_ID: &str = "fish_cast";
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const NEED: u32 = 1;
pub const WAIT_S: f32 = 0.8;
pub const NAME_PLAYER: &str = "player";
pub const NAME_CAST: &str = "cast";
pub const NAME_BITE: &str = "bite";
pub const NAME_CATCH: &str = "catch";

const FLAG_CATCH_COLOR: [u32; 3] = [70, 180, 110];
const BOBBER_CAST: [u32; 3] = [240, 196, 72];
const BOBBER_BITE: [u32; 3] = [255, 140, 64];
const BOBBER_CATCH: [u32; 3] = [70, 180, 110];
const CAST_PIP: [u8; 4] = [240, 196, 72, 255];
const BITE_PIP: [u8; 4] = [255, 140, 64, 255];
const CATCH_PIP: [u8; 4] = [70, 180, 110, 255];

/// Live fishing around a dump. Cast/bite/catch stay here; `name` + flag
/// enable and `coins` in the dump are the query source of truth. Wait is
/// time, not Rapier, not a sim.
#[derive(Clone, Debug)]
pub struct FishGame {
    pub waiting: bool,
    pub biting: bool,
    pub caught: bool,
    pub done: bool,
    pub wait: f32,
    pub coins: u32,
}

impl Default for FishGame {
    fn default() -> Self {
        Self {
            waiting: false,
            biting: false,
            caught: false,
            done: false,
            wait: 0.0,
            coins: 0,
        }
    }
}

impl FishGame {
    pub fn from_doc(_doc: &WorldDoc) -> Self {
        Self::default()
    }
}

pub fn is_fish(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "dock")
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

/// Sit capsules/boxes, keep flag off, bobber hidden, coins 0, dock cam.
pub fn seed(doc: &mut WorldDoc) {
    if !is_fish(doc) {
        return;
    }
    sit_named(doc, NAME_PLAYER);
    sit_props(doc);
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = false;
        }
        if p.name == "bobber" {
            p.enabled = false;
            p.color = Some(BOBBER_CAST);
        }
    }
    doc.coins = 0;
    place_dock_camera(doc);
}

/// Dock cam looks at water + dock so the plane and metal pad read.
pub fn place_dock_camera(doc: &mut WorldDoc) {
    let (dx, dy, dz) = named_prop_any(doc, "dock")
        .map(|p| (p.position[0], p.position[1], p.position[2]))
        .unwrap_or((0.0, 0.2, 3.0));
    let target = [dx, dy.max(0.25), dz - 2.4];
    let eye = [dx, (dy + 5.8).max(6.2), dz + 7.6];
    let fov = doc.cameras.first().map(|cam| cam.fov).unwrap_or(52.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye;
        cam.target = target;
        cam.name = "dock".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "dock".into(),
            position: eye,
            target,
            fov,
        });
    }
}

/// J/click on the dock casts; after `WAIT_S` a bite; J/click lands catch.
/// Caller may have walked; dock cam wins so water + dock stay readable.
pub fn tick(doc: &mut WorldDoc, game: &mut FishGame, input: WalkInput, dt: f32) {
    if game.done {
        write_beat(doc, game);
        place_dock_camera(doc);
        return;
    }
    if game.waiting {
        game.wait = (game.wait + dt).min(WAIT_S);
        if game.wait >= WAIT_S - 1e-4 {
            game.waiting = false;
            game.biting = true;
            paint_bobber(doc, BOBBER_BITE);
        }
    } else if game.biting {
        if input.attack {
            game.biting = false;
            game.caught = true;
            game.done = true;
            game.coins = NEED;
            paint_bobber(doc, BOBBER_CATCH);
        }
    } else if input.attack && player_on_dock(doc) {
        game.waiting = true;
        game.wait = 0.0;
        show_bobber(doc, BOBBER_CAST);
    }
    write_beat(doc, game);
    place_dock_camera(doc);
}

fn beat_name(game: &FishGame) -> &'static str {
    if game.caught {
        NAME_CATCH
    } else if game.biting {
        NAME_BITE
    } else if game.waiting {
        NAME_CAST
    } else {
        NAME_PLAYER
    }
}

fn write_beat(doc: &mut WorldDoc, game: &FishGame) {
    sit_named(doc, beat_name(game));
    doc.coins = game.coins;
    if game.caught {
        set_flag_prop(doc);
    }
}

fn set_flag_prop(doc: &mut WorldDoc) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = true;
            p.color = Some(FLAG_CATCH_COLOR);
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
    let water_y = doc.water_y.unwrap_or(0.0);
    let mut updates: Vec<(usize, f32)> = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        let y = match p.name.as_str() {
            "dock" | "flag" | "bank" => {
                doc.height_at(p.position[0], p.position[2]) + p.scale[1].abs() * 0.5
            }
            "bobber" => water_y + p.scale[1].abs() * 0.5 + 0.04,
            _ => continue,
        };
        updates.push((i, y));
    }
    for (i, y) in updates {
        if let Some(p) = doc.props.get_mut(i) {
            p.position[1] = y;
        }
    }
}

fn named_prop_any<'a>(doc: &'a WorldDoc, name: &str) -> Option<&'a WorldProp> {
    doc.props.iter().find(|p| p.name == name)
}

fn named_prop_mut<'a>(doc: &'a mut WorldDoc, name: &str) -> Option<&'a mut WorldProp> {
    doc.props.iter_mut().find(|p| p.name == name)
}

fn player_on_dock(doc: &WorldDoc) -> bool {
    let Some(player) = player_ref(doc) else {
        return false;
    };
    let Some(dock) = named_prop_any(doc, "dock") else {
        return false;
    };
    let hx = dock.scale[0].abs() * 0.5 + PLAYER_R;
    let hz = dock.scale[2].abs() * 0.5 + PLAYER_R;
    let dx = player.position[0] - dock.position[0];
    let dz = player.position[2] - dock.position[2];
    dx.abs() <= hx && dz.abs() <= hz
}

fn show_bobber(doc: &mut WorldDoc, color: [u32; 3]) {
    sit_props(doc);
    if let Some(p) = named_prop_mut(doc, "bobber") {
        p.enabled = true;
        p.color = Some(color);
    }
}

fn paint_bobber(doc: &mut WorldDoc, color: [u32; 3]) {
    if let Some(p) = named_prop_mut(doc, "bobber") {
        p.enabled = true;
        p.color = Some(color);
    }
}

pub fn build_hud(game: &FishGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
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
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                160.0 * scale,
                18.0 * scale,
                [28, 24, 20, 160],
            ));
            let frac = if game.biting || game.caught {
                1.0
            } else if game.waiting {
                (game.wait / WAIT_S).clamp(0.0, 1.0)
            } else {
                0.04
            };
            let pip = if game.caught {
                CATCH_PIP
            } else if game.biting {
                BITE_PIP
            } else if game.waiting {
                CAST_PIP
            } else {
                [80, 84, 78, 200]
            };
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                160.0 * scale * frac.max(0.04),
                18.0 * scale,
                pip,
            ));
            if game.biting {
                quads.push(Quad::new(
                    w * 0.42,
                    h * 0.72,
                    w * 0.16,
                    22.0 * scale,
                    BITE_PIP,
                ));
            }
        }
        GamePhase::Complete => {
            quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 28, 18, 210]));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.62,
                w * 0.36,
                48.0 * scale,
                CATCH_PIP,
            ));
        }
    }
    DrawList {
        clear: [18, 32, 42, 255],
        quads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const DOCK: &str = include_str!("../tests/fixtures/fish_cast_world.json");
    const STAGE: &str = include_str!("../tests/fixtures/rhythm_beat_world.json");
    const CAMP: &str = include_str!("../tests/fixtures/survival_meter_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const TOWN: &str = include_str!("../tests/fixtures/rpg_town_world.json");
    const PAGES: &str = include_str!("../tests/fixtures/novel_pages_world.json");
    const HIDE: &str = include_str!("../tests/fixtures/stealth_hide_world.json");
    const RING: &str = include_str!("../tests/fixtures/fight_hitstun_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");
    const ROOM: &str = include_str!("../tests/fixtures/puzzle_pad_world.json");
    const HOP: &str = include_str!("../tests/fixtures/box_hop_world.json");
    const PITCH: &str = include_str!("../tests/fixtures/sports_goal_world.json");
    const YARD: &str = include_str!("../tests/fixtures/sim_meter_world.json");
    const SIDE: &str = include_str!("../tests/fixtures/action_side_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(DOCK).unwrap();
        play.confirm();
        play
    }

    fn put_player(play: &mut WorldPlay, x: f32, z: f32) {
        let y = play.doc.height_at(x, z) + BODY_H;
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [x, y, z];
        }
        if let Some(p) = play.doc.player.clone() {
            let mut found = false;
            for w in &mut play.doc.walkers {
                if w.id == p.id {
                    *w = p.clone();
                    found = true;
                }
            }
            if !found {
                play.doc.walkers.push(p);
            }
        }
    }

    #[test]
    fn dump_is_fish_dock_not_other_genres() {
        let mut doc = WorldDoc::from_json(DOCK).unwrap();
        seed(&mut doc);
        assert!(is_fish(&doc));
        assert_eq!(GAME_ID, "fish_cast");
        assert!(!crate::rhythm::is_rhythm(&doc));
        assert!(!crate::survival::is_survival(&doc));
        assert!(!crate::sim::is_sim(&doc));
        assert!(!crate::puzzle::is_puzzle(&doc));
        assert!(!crate::stealth::is_stealth(&doc));
        assert!(!crate::novel::is_novel(&doc));
        assert!(!crate::rpg::is_rpg(&doc));
        assert!(!crate::fight::is_fight(&doc));
        assert!(!crate::action::is_action(&doc));
        assert!(!crate::action2d::is_action2d(&doc));
        assert!(!crate::fps::is_fps(&doc));
        assert!(!crate::td::is_td(&doc));
        assert!(!crate::race::is_race(&doc));
        assert!(!crate::platformer::is_platformer(&doc));
        assert!(!crate::sports::is_sports(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_fish(&crest), "crest water_y must not count as fish");
        let camp = WorldDoc::from_json(CAMP).unwrap();
        assert!(crate::survival::is_survival(&camp));
        assert!(!is_fish(&camp));
        let stage = WorldDoc::from_json(STAGE).unwrap();
        assert!(crate::rhythm::is_rhythm(&stage));
        assert!(!is_fish(&stage), "rhythm stage must not count as fish");
        let yard = WorldDoc::from_json(YARD).unwrap();
        assert!(crate::sim::is_sim(&yard));
        assert!(!is_fish(&yard));
        assert_eq!(doc.water_y, Some(0.0));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "dock" && p.model == "box"));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "bobber" && p.model == "box" && !p.enabled));
        assert!(doc.props.iter().any(|p| p.name == "flag" && !p.enabled));
        let dock = doc.props.iter().find(|p| p.name == "dock").unwrap();
        assert!(
            dock.metallic >= 0.5,
            "dock should read metal GGX, metallic={}",
            dock.metallic
        );
        let bobber = doc.props.iter().find(|p| p.name == "bobber").unwrap();
        assert!(
            bobber.metallic >= 0.5,
            "bobber should read metal GGX, metallic={}",
            bobber.metallic
        );
        assert_eq!(doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("dock"));
        assert!(json.contains("bobber"));
        assert!(json.contains("flag"));
        assert!(json.contains("water"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 3,
            "water plane + dock + player must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(DOCK).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_fish());
        assert!(!play.is_rhythm());
        assert!(!play.is_survival());
        assert!(!play.is_sim());
        assert!(!play.is_collectathon());
        let start_coins = play.doc.coins;
        let bobber0 = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "bobber")
            .unwrap()
            .enabled;
        play.input.attack = true;
        play.input.lz = 1.0;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.fish.waiting, "title must not cast");
        assert!(!play.fish.done);
        assert_eq!(play.doc.coins, start_coins);
        let bobber1 = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "bobber")
            .unwrap()
            .enabled;
        assert_eq!(bobber1, bobber0);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "dock");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn attack_on_dock_casts() {
        let mut play = play_started();
        assert!(player_on_dock(&play.doc), "spawn stands on the dock");
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.fish.waiting);
        assert!(!play.fish.biting);
        assert!(!play.fish.caught);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_CAST);
        let bobber = play.doc.props.iter().find(|p| p.name == "bobber").unwrap();
        assert!(bobber.enabled, "cast shows the bobber");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("cast"), "cast must be dump-visible");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn wait_then_bite() {
        let mut play = play_started();
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        play.input.attack = false;
        assert!(play.fish.waiting);
        for _ in 0..80 {
            play.tick(1.0 / 60.0);
            if play.fish.biting {
                break;
            }
        }
        assert!(play.fish.biting, "short wait must produce a bite");
        assert!(!play.fish.waiting);
        assert!(!play.fish.caught);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_BITE);
        assert_eq!(play.doc.coins, 0);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("bite"), "bite must be dump-visible");
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "bite does not raise the catch flag");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "bite overlay");
    }

    #[test]
    fn attack_on_bite_lands_catch() {
        let mut play = play_started();
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        play.input.attack = false;
        for _ in 0..80 {
            play.tick(1.0 / 60.0);
            if play.fish.biting {
                break;
            }
        }
        assert!(play.fish.biting);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.fish.caught);
        assert!(play.fish.done);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_CATCH);
        assert_eq!(play.doc.coins, NEED);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "catch flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_CATCH_COLOR));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("catch"), "catch must be dump-visible");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "catch overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.fish.done);
        assert!(!play.fish.caught);
        assert!(!play.fish.waiting);
        assert_eq!(play.doc.coins, 0);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "retry restores flag off");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        let bobber = play.doc.props.iter().find(|p| p.name == "bobber").unwrap();
        assert!(!bobber.enabled, "retry hides bobber");
        assert_eq!(play.doc.cameras[0].name, "dock");
    }

    #[test]
    fn attack_off_dock_is_ignored() {
        let mut play = play_started();
        put_player(&mut play, 6.0, 6.0);
        assert!(!player_on_dock(&play.doc));
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(!play.fish.waiting, "off-dock J is not a cast");
        assert!(!play.fish.biting);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        let bobber = play.doc.props.iter().find(|p| p.name == "bobber").unwrap();
        assert!(!bobber.enabled);
    }

    #[test]
    fn held_attack_catches_from_spawn() {
        let mut play = play_started();
        play.input.attack = true;
        for _ in 0..180 {
            play.tick(1.0 / 60.0);
            play.input.attack = true;
            if play.fish.done {
                break;
            }
        }
        assert!(
            play.fish.caught && play.fish.done,
            "held J/click should cast, wait, bite, catch; waiting={} biting={} coins={} name={:?}",
            play.fish.waiting,
            play.fish.biting,
            play.doc.coins,
            play.doc.player.as_ref().unwrap().name
        );
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_CATCH);
        assert_eq!(play.doc.cameras[0].name, "dock");
        assert_eq!(play.doc.coins, NEED);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn other_genres_still_own_their_dumps() {
        let town = WorldPlay::from_json(TOWN).unwrap();
        assert!(town.is_rpg());
        assert!(!town.is_fish());
        let novel = WorldPlay::from_json(PAGES).unwrap();
        assert!(novel.is_novel());
        assert!(!novel.is_fish());
        let stealth = WorldPlay::from_json(HIDE).unwrap();
        assert!(stealth.is_stealth());
        assert!(!stealth.is_fish());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_fish());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_fish());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_fish());
        let puzzle = WorldPlay::from_json(ROOM).unwrap();
        assert!(puzzle.is_puzzle());
        assert!(!puzzle.is_fish());
        let hop = WorldPlay::from_json(HOP).unwrap();
        assert!(hop.is_platformer());
        assert!(!hop.is_fish());
        let sports = WorldPlay::from_json(PITCH).unwrap();
        assert!(sports.is_sports());
        assert!(!sports.is_fish());
        let sim = WorldPlay::from_json(YARD).unwrap();
        assert!(sim.is_sim());
        assert!(!sim.is_fish());
        let side = WorldPlay::from_json(SIDE).unwrap();
        assert!(side.is_action2d());
        assert!(!side.is_fish());
        let camp = WorldPlay::from_json(CAMP).unwrap();
        assert!(camp.is_survival());
        assert!(!camp.is_fish());
        let stage = WorldPlay::from_json(STAGE).unwrap();
        assert!(stage.is_rhythm());
        assert!(!stage.is_fish());
    }
}
