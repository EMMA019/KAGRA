//! Stove cook as an M3 slice on play_world.
//!
//! Sibling of shop / fish. Player walks a capsule; a box is the stove.
//! Stand at the stove; after a short cook time, J / click (`WalkInput.attack`)
//! lands dump-visible `cooked` (`flag` / `name`, coins). Title -> play ->
//! result reuses `WorldPlay` / `GamePhase`. Capsules/boxes, not VRM. Indoor
//! lights stay 4 slots. Stove stays readable (contact blob plus metal GGX
//! inherited). Stove camera looks at the hob so the picture holds. Does not
//! rewrite shop / RPG / survival. No Rapier, SSAO, GI, net, or ECS. Picture
//! slice stays.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};

pub const GAME_ID: &str = "cook_stove";
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const NEED: u32 = 1;
pub const COOK_S: f32 = 0.8;
pub const COOK_REACH: f32 = 2.2;
pub const NAME_PLAYER: &str = "player";
pub const NAME_COOK: &str = "cook";
pub const NAME_READY: &str = "ready";
pub const NAME_COOKED: &str = "cooked";

const FLAG_COOKED_COLOR: [u32; 3] = [70, 180, 110];
const PAN_IDLE: [u32; 3] = [72, 76, 82];
const PAN_COOK: [u32; 3] = [240, 140, 48];
const PAN_READY: [u32; 3] = [255, 196, 72];
const PAN_COOKED: [u32; 3] = [70, 180, 110];
const COOK_PIP: [u8; 4] = [240, 140, 48, 255];
const READY_PIP: [u8; 4] = [255, 196, 72, 255];
const COOKED_PIP: [u8; 4] = [70, 180, 110, 255];

/// Live cook around a dump. Cook/ready/cooked stay here; `name` + flag
/// enable and `coins` in the dump are the query source of truth. Wait is
/// time at the stove, not Rapier, not a sim.
#[derive(Clone, Debug)]
pub struct CookGame {
    pub cooking: bool,
    pub ready: bool,
    pub cooked: bool,
    pub done: bool,
    pub wait: f32,
    pub coins: u32,
}

impl Default for CookGame {
    fn default() -> Self {
        Self {
            cooking: false,
            ready: false,
            cooked: false,
            done: false,
            wait: 0.0,
            coins: 0,
        }
    }
}

impl CookGame {
    pub fn from_doc(_doc: &WorldDoc) -> Self {
        Self::default()
    }
}

pub fn is_cook(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "stove")
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

/// Sit capsules/boxes, keep flag/meal off, pan idle, coins 0, stove cam.
pub fn seed(doc: &mut WorldDoc) {
    if !is_cook(doc) {
        return;
    }
    sit_named(doc, NAME_PLAYER);
    sit_props(doc);
    for p in &mut doc.props {
        if p.name == "flag" || p.name == "meal" {
            p.enabled = false;
        }
        if p.name == "pan" {
            p.enabled = true;
            p.color = Some(PAN_IDLE);
        }
    }
    doc.coins = 0;
    place_stove_camera(doc);
}

/// Stove cam looks at the hob so metal + pan read.
pub fn place_stove_camera(doc: &mut WorldDoc) {
    let (sx, sy, sz) = named_prop_any(doc, "stove")
        .map(|p| (p.position[0], p.position[1], p.position[2]))
        .unwrap_or((0.0, 0.45, -1.0));
    let target = [sx, sy.max(0.4), sz + 0.35];
    let eye = [sx, (sy + 5.6).max(6.0), sz + 8.4];
    let fov = doc.cameras.first().map(|cam| cam.fov).unwrap_or(52.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye;
        cam.target = target;
        cam.name = "stove".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "stove".into(),
            position: eye,
            target,
            fov,
        });
    }
}

/// Stand at the stove cooks; after `COOK_S` the hob is ready; J/click
/// lands cooked. Away from the stove resets the wait. Caller may have
/// walked; stove cam wins so the hob stays readable.
pub fn tick(doc: &mut WorldDoc, game: &mut CookGame, input: WalkInput, dt: f32) {
    if game.done {
        write_beat(doc, game);
        place_stove_camera(doc);
        return;
    }
    let at = player_at_stove(doc);
    if !at {
        if !game.cooked {
            game.cooking = false;
            game.ready = false;
            game.wait = 0.0;
            paint_pan(doc, PAN_IDLE);
        }
    } else if game.ready {
        if input.attack {
            game.ready = false;
            game.cooked = true;
            game.done = true;
            game.coins = NEED;
            paint_pan(doc, PAN_COOKED);
            show_meal(doc);
        }
    } else {
        game.cooking = true;
        game.wait = (game.wait + dt).min(COOK_S);
        if game.wait >= COOK_S - 1e-4 {
            game.cooking = false;
            game.ready = true;
            paint_pan(doc, PAN_READY);
        } else {
            paint_pan(doc, PAN_COOK);
        }
    }
    write_beat(doc, game);
    place_stove_camera(doc);
}

fn beat_name(game: &CookGame) -> &'static str {
    if game.cooked {
        NAME_COOKED
    } else if game.ready {
        NAME_READY
    } else if game.cooking {
        NAME_COOK
    } else {
        NAME_PLAYER
    }
}

fn write_beat(doc: &mut WorldDoc, game: &CookGame) {
    sit_named(doc, beat_name(game));
    doc.coins = game.coins;
    if game.cooked {
        set_flag_prop(doc);
        show_meal(doc);
    }
}

fn set_flag_prop(doc: &mut WorldDoc) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = true;
            p.color = Some(FLAG_COOKED_COLOR);
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
            "stove" | "flag" | "floor" | "hood" => p.scale[1].abs() * 0.5,
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

fn named_prop_any<'a>(doc: &'a WorldDoc, name: &str) -> Option<&'a WorldProp> {
    doc.props.iter().find(|p| p.name == name)
}

fn named_prop_mut<'a>(doc: &'a mut WorldDoc, name: &str) -> Option<&'a mut WorldProp> {
    doc.props.iter_mut().find(|p| p.name == name)
}

fn player_at_stove(doc: &WorldDoc) -> bool {
    let Some(player) = player_ref(doc) else {
        return false;
    };
    let Some(stove) = named_prop_any(doc, "stove") else {
        return false;
    };
    if !stove.enabled {
        return false;
    }
    let dx = player.position[0] - stove.position[0];
    let dz = player.position[2] - stove.position[2];
    (dx * dx + dz * dz).sqrt() <= COOK_REACH
}

fn paint_pan(doc: &mut WorldDoc, color: [u32; 3]) {
    if let Some(p) = named_prop_mut(doc, "pan") {
        p.enabled = true;
        p.color = Some(color);
    }
}

fn show_meal(doc: &mut WorldDoc) {
    if let Some(p) = named_prop_mut(doc, "meal") {
        p.enabled = true;
    }
}

pub fn build_hud(game: &CookGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let mut quads = Vec::new();
    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [16, 12, 10, 160]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [32, 22, 18, 230],
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.58,
                w * 0.36,
                52.0 * scale,
                [240, 140, 48, 255],
            ));
        }
        GamePhase::Playing => {
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                160.0 * scale,
                18.0 * scale,
                [28, 22, 18, 160],
            ));
            let frac = if game.ready || game.cooked {
                1.0
            } else if game.cooking {
                (game.wait / COOK_S).clamp(0.0, 1.0)
            } else {
                0.04
            };
            let pip = if game.cooked {
                COOKED_PIP
            } else if game.ready {
                READY_PIP
            } else if game.cooking {
                COOK_PIP
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
            if game.ready {
                quads.push(Quad::new(
                    w * 0.42,
                    h * 0.72,
                    w * 0.16,
                    22.0 * scale,
                    READY_PIP,
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
                COOKED_PIP,
            ));
        }
    }
    DrawList {
        clear: [28, 22, 20, 255],
        quads,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const STOVE: &str = include_str!("../tests/fixtures/cook_stove_world.json");
    const STALL: &str = include_str!("../tests/fixtures/shop_buy_world.json");
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
        let mut play = WorldPlay::from_json(STOVE).unwrap();
        play.confirm();
        play
    }

    fn put_player(play: &mut WorldPlay, x: f32, z: f32) {
        let y = play.doc.height_at(x, z) + BODY_H;
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [x, y, z];
        }
        if let Some(p) = play.doc.player.clone() {
            write_player(&mut play.doc, p);
        }
    }

    fn wait_until_ready(play: &mut WorldPlay) {
        for _ in 0..80 {
            play.input.attack = false;
            play.tick(1.0 / 60.0);
            if play.cook.ready {
                break;
            }
        }
    }

    #[test]
    fn dump_is_cook_stove_not_other_genres() {
        let mut doc = WorldDoc::from_json(STOVE).unwrap();
        seed(&mut doc);
        assert!(is_cook(&doc));
        assert_eq!(GAME_ID, "cook_stove");
        assert!(!crate::shop::is_shop(&doc));
        assert!(!crate::fish::is_fish(&doc));
        assert!(!crate::rhythm::is_rhythm(&doc));
        assert!(!crate::survival::is_survival(&doc));
        assert!(!crate::sim::is_sim(&doc));
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
        assert!(!crate::action2d::is_action2d(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_cook(&crest));
        let stall = WorldDoc::from_json(STALL).unwrap();
        assert!(crate::shop::is_shop(&stall));
        assert!(!is_cook(&stall), "shop stall must not count as cook");
        let dock = WorldDoc::from_json(DOCK).unwrap();
        assert!(crate::fish::is_fish(&dock));
        assert!(!is_cook(&dock), "fish dock must not count as cook");
        let town = WorldDoc::from_json(TOWN).unwrap();
        assert!(crate::rpg::is_rpg(&town));
        assert!(!is_cook(&town), "RPG town must not count as cook");
        let camp = WorldDoc::from_json(CAMP).unwrap();
        assert!(crate::survival::is_survival(&camp));
        assert!(!is_cook(&camp), "survival camp must not count as cook");
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "stove" && p.model == "box"));
        assert!(doc.props.iter().any(|p| p.name == "flag" && !p.enabled));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "floor" && p.model == "box"));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "pan" && p.model == "box" && p.enabled));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "meal" && p.model == "box" && !p.enabled));
        let stove = doc.props.iter().find(|p| p.name == "stove").unwrap();
        assert!(
            stove.metallic >= 0.5,
            "stove should read metal GGX, metallic={}",
            stove.metallic
        );
        assert_eq!(doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.coins, 0);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("stove"));
        assert!(json.contains("flag"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 3,
            "floor + stove must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(STOVE).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_cook());
        assert!(!play.is_shop());
        assert!(!play.is_fish());
        assert!(!play.is_rpg());
        assert!(!play.is_survival());
        assert!(!play.is_collectathon());
        let start = play.doc.player.as_ref().unwrap().position;
        assert_eq!(play.doc.coins, 0, "cook dump starts with no coins");
        play.input.attack = true;
        play.input.lx = 1.0;
        play.input.lz = 1.0;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.cook.cooking, "title must not cook");
        assert!(!play.cook.ready);
        assert!(!play.cook.done);
        assert_eq!(play.doc.player.as_ref().unwrap().position, start);
        assert_eq!(play.doc.coins, 0);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "stove");
        assert_eq!(play.doc.coins, 0);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn stand_cooks_then_attack_lands_cooked() {
        let mut play = play_started();
        assert!(player_at_stove(&play.doc), "spawn stands at the stove");
        assert_eq!(play.doc.coins, 0);
        play.tick(1.0 / 60.0);
        assert!(play.cook.cooking, "standing at the stove starts the wait");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_COOK);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("cook"), "cook name must be dump-visible");
        wait_until_ready(&mut play);
        assert!(play.cook.ready, "short cook time must ready the hob");
        assert!(!play.cook.cooking);
        assert!(!play.cook.cooked);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_READY);
        assert_eq!(play.doc.coins, 0);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("ready"), "ready must be dump-visible");
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "ready does not raise the cooked flag");
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.cook.cooked);
        assert!(play.cook.done);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_COOKED);
        assert_eq!(play.doc.coins, NEED);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "cooked flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_COOKED_COLOR));
        let meal = play.doc.props.iter().find(|p| p.name == "meal").unwrap();
        assert!(meal.enabled, "cooked meal must be dump-visible");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("cooked"), "cooked name must be dump-visible");
        assert!(dump.contains("flag"));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "cooked overlay");
        assert_eq!(play.doc.cameras[0].name, "stove");
    }

    #[test]
    fn attack_before_cook_time_is_ignored() {
        let mut play = play_started();
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.cook.cooking);
        assert!(!play.cook.ready);
        assert!(!play.cook.cooked, "J before cook time is not cooked");
        assert_eq!(play.doc.coins, 0);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_COOK);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled);
    }

    #[test]
    fn attack_away_from_stove_is_ignored() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 6.0);
        assert!(!player_at_stove(&play.doc));
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(!play.cook.cooking, "off-stove J is not a cook");
        assert!(!play.cook.ready);
        assert!(!play.cook.done);
        assert_eq!(play.doc.coins, 0);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled);
    }

    #[test]
    fn walking_away_resets_the_wait() {
        let mut play = play_started();
        play.tick(1.0 / 60.0);
        assert!(play.cook.cooking);
        put_player(&mut play, 0.0, 6.0);
        play.tick(1.0 / 60.0);
        assert!(!play.cook.cooking);
        assert!(!play.cook.ready);
        assert_eq!(play.cook.wait, 0.0);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
    }

    #[test]
    fn confirm_retry_restores_flag_and_coins() {
        let mut play = play_started();
        wait_until_ready(&mut play);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.cook.done);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.cook.done);
        assert!(!play.cook.cooked);
        assert!(!play.cook.ready);
        assert_eq!(play.doc.coins, 0);
        assert_eq!(play.cook.coins, 0);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "retry restores flag off");
        let meal = play.doc.props.iter().find(|p| p.name == "meal").unwrap();
        assert!(!meal.enabled, "retry hides meal");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert_eq!(play.doc.cameras[0].name, "stove");
    }

    #[test]
    fn held_attack_cooks_from_spawn() {
        let mut play = play_started();
        play.input.attack = true;
        for _ in 0..180 {
            play.tick(1.0 / 60.0);
            play.input.attack = true;
            if play.cook.done {
                break;
            }
        }
        assert!(
            play.cook.cooked && play.cook.done,
            "held J/click at spawn should wait then cook; ready={} coins={} name={:?}",
            play.cook.ready,
            play.doc.coins,
            play.doc.player.as_ref().unwrap().name
        );
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_COOKED);
        assert_eq!(play.doc.coins, NEED);
        assert_eq!(play.doc.cameras[0].name, "stove");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn other_genres_still_own_their_dumps() {
        let stall = WorldPlay::from_json(STALL).unwrap();
        assert!(stall.is_shop());
        assert!(!stall.is_cook());
        let dock = WorldPlay::from_json(DOCK).unwrap();
        assert!(dock.is_fish());
        assert!(!dock.is_cook());
        let town = WorldPlay::from_json(TOWN).unwrap();
        assert!(town.is_rpg());
        assert!(!town.is_cook());
        let novel = WorldPlay::from_json(PAGES).unwrap();
        assert!(novel.is_novel());
        assert!(!novel.is_cook());
        let stealth = WorldPlay::from_json(HIDE).unwrap();
        assert!(stealth.is_stealth());
        assert!(!stealth.is_cook());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_cook());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_cook());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_cook());
        let puzzle = WorldPlay::from_json(ROOM).unwrap();
        assert!(puzzle.is_puzzle());
        assert!(!puzzle.is_cook());
        let hop = WorldPlay::from_json(HOP).unwrap();
        assert!(hop.is_platformer());
        assert!(!hop.is_cook());
        let sports = WorldPlay::from_json(PITCH).unwrap();
        assert!(sports.is_sports());
        assert!(!sports.is_cook());
        let sim = WorldPlay::from_json(YARD).unwrap();
        assert!(sim.is_sim());
        assert!(!sim.is_cook());
        let side = WorldPlay::from_json(SIDE).unwrap();
        assert!(side.is_action2d());
        assert!(!side.is_cook());
        let camp = WorldPlay::from_json(CAMP).unwrap();
        assert!(camp.is_survival());
        assert!(!camp.is_cook());
        let stage = WorldPlay::from_json(STAGE).unwrap();
        assert!(stage.is_rhythm());
        assert!(!stage.is_cook());
    }
}
