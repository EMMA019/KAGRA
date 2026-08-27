//! Buy with coins as an M3 slice on play_world.
//!
//! Sibling of fish / rpg. Player walks a capsule; a box is the stall.
//! Stand at the stall; J / click (`WalkInput.attack`) spends `coins` and
//! lands dump-visible `bought` (`flag` / `name`, coins decreased). Title
//! -> play -> result reuses `WorldPlay` / `GamePhase`. Capsules/boxes, not
//! VRM. Indoor lights stay 4 slots. Stall stays readable (contact blob
//! plus metal GGX inherited). Stall camera looks at the counter so the
//! picture holds. Does not rewrite RPG / inventory (there is no inventory
//! API ? a single bought flag is enough). No Rapier, SSAO, GI, net, or
//! ECS. Picture slice stays.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};

pub const GAME_ID: &str = "shop_buy";
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const START: u32 = 8;
pub const PRICE: u32 = 5;
pub const SHOP_REACH: f32 = 2.2;
pub const NAME_PLAYER: &str = "player";
pub const NAME_BOUGHT: &str = "bought";

const FLAG_BOUGHT_COLOR: [u32; 3] = [70, 180, 110];
const COIN_PIP: [u8; 4] = [240, 196, 72, 255];
const BOUGHT_PIP: [u8; 4] = [70, 180, 110, 255];

/// Live shop around a dump. Bought stays here; `name` + flag enable and
/// `coins` in the dump are the query source of truth. Spend is J/click at
/// the stall, not Rapier, not an inventory grid.
#[derive(Clone, Debug)]
pub struct ShopGame {
    pub bought: bool,
    pub done: bool,
    pub coins: u32,
}

impl Default for ShopGame {
    fn default() -> Self {
        Self {
            bought: false,
            done: false,
            coins: 0,
        }
    }
}

impl ShopGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        if is_shop(doc) {
            Self {
                bought: false,
                done: false,
                coins: START,
            }
        } else {
            Self::default()
        }
    }
}

pub fn is_shop(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "stall")
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

/// Sit capsules/boxes, keep flag off, coins = START, stall cam.
pub fn seed(doc: &mut WorldDoc) {
    if !is_shop(doc) {
        return;
    }
    sit_named(doc, NAME_PLAYER);
    sit_props(doc);
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = false;
        }
    }
    doc.coins = START;
    place_stall_camera(doc);
}

/// Stall cam looks at the counter so metal + goods read.
pub fn place_stall_camera(doc: &mut WorldDoc) {
    let (sx, sy, sz) = named_prop_any(doc, "stall")
        .map(|p| (p.position[0], p.position[1], p.position[2]))
        .unwrap_or((0.0, 0.45, -1.2));
    let target = [sx, sy.max(0.4), sz + 0.4];
    let eye = [sx, (sy + 5.6).max(6.0), sz + 8.4];
    let fov = doc.cameras.first().map(|cam| cam.fov).unwrap_or(52.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye;
        cam.target = target;
        cam.name = "stall".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "stall".into(),
            position: eye,
            target,
            fov,
        });
    }
}

/// J/click at the stall spends coins and lands bought. Caller may have
/// walked; stall cam wins so the counter stays readable.
pub fn tick(doc: &mut WorldDoc, game: &mut ShopGame, input: WalkInput, _dt: f32) {
    if game.done {
        write_beat(doc, game);
        place_stall_camera(doc);
        return;
    }
    if input.attack && player_at_stall(doc) && !game.bought && game.coins >= PRICE {
        game.bought = true;
        game.done = true;
        game.coins -= PRICE;
    }
    write_beat(doc, game);
    place_stall_camera(doc);
}

fn beat_name(game: &ShopGame) -> &'static str {
    if game.bought {
        NAME_BOUGHT
    } else {
        NAME_PLAYER
    }
}

fn write_beat(doc: &mut WorldDoc, game: &ShopGame) {
    sit_named(doc, beat_name(game));
    doc.coins = game.coins;
    if game.bought {
        set_flag_prop(doc);
    }
}

fn set_flag_prop(doc: &mut WorldDoc) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = true;
            p.color = Some(FLAG_BOUGHT_COLOR);
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
            "stall" | "flag" | "floor" => p.scale[1].abs() * 0.5,
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

fn player_at_stall(doc: &WorldDoc) -> bool {
    let Some(player) = player_ref(doc) else {
        return false;
    };
    let Some(stall) = named_prop_any(doc, "stall") else {
        return false;
    };
    if !stall.enabled {
        return false;
    }
    let dx = player.position[0] - stall.position[0];
    let dz = player.position[2] - stall.position[2];
    (dx * dx + dz * dz).sqrt() <= SHOP_REACH
}

pub fn build_hud(game: &ShopGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
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
            let frac = (game.coins as f32 / START as f32).clamp(0.0, 1.0);
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
                COIN_PIP,
            ));
            quads.push(Quad::new(
                184.0 * scale,
                16.0 * scale,
                22.0 * scale,
                22.0 * scale,
                if game.bought {
                    BOUGHT_PIP
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
                BOUGHT_PIP,
            ));
        }
    }
    DrawList {
        clear: [28, 24, 22, 255],
        quads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

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
        let mut play = WorldPlay::from_json(STALL).unwrap();
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

    #[test]
    fn dump_is_shop_stall_not_other_genres() {
        let mut doc = WorldDoc::from_json(STALL).unwrap();
        seed(&mut doc);
        assert!(is_shop(&doc));
        assert_eq!(GAME_ID, "shop_buy");
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
        assert!(!is_shop(&crest));
        let dock = WorldDoc::from_json(DOCK).unwrap();
        assert!(crate::fish::is_fish(&dock));
        assert!(!is_shop(&dock), "fish dock must not count as shop");
        let town = WorldDoc::from_json(TOWN).unwrap();
        assert!(crate::rpg::is_rpg(&town));
        assert!(!is_shop(&town), "RPG town must not count as shop");
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "stall" && p.model == "box"));
        assert!(doc.props.iter().any(|p| p.name == "flag" && !p.enabled));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "floor" && p.model == "box"));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "goods" && p.model == "box"));
        let stall = doc.props.iter().find(|p| p.name == "stall").unwrap();
        assert!(
            stall.metallic >= 0.5,
            "stall should read metal GGX, metallic={}",
            stall.metallic
        );
        assert_eq!(doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.coins, START);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("stall"));
        assert!(json.contains("flag"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 3,
            "floor + stall must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(STALL).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_shop());
        assert!(!play.is_fish());
        assert!(!play.is_rpg());
        assert!(!play.is_sim());
        assert!(!play.is_collectathon());
        let start = play.doc.player.as_ref().unwrap().position;
        let coins = play.doc.coins;
        assert_eq!(coins, START, "player has coins on the title dump");
        play.input.attack = true;
        play.input.lx = 1.0;
        play.input.lz = 1.0;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.shop.bought, "title must not buy");
        assert!(!play.shop.done);
        assert_eq!(play.doc.player.as_ref().unwrap().position, start);
        assert_eq!(play.doc.coins, START);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "stall");
        assert_eq!(play.doc.coins, START);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn attack_at_stall_buys() {
        let mut play = play_started();
        assert!(player_at_stall(&play.doc), "spawn stands at the stall");
        assert_eq!(play.doc.coins, START);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.shop.bought);
        assert!(play.shop.done);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_BOUGHT);
        assert_eq!(play.doc.coins, START - PRICE);
        assert!(
            play.doc.coins < START,
            "coins must decrease, coins={}",
            play.doc.coins
        );
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "bought flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_BOUGHT_COLOR));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("bought"), "bought name must be dump-visible");
        assert!(dump.contains("flag"));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "bought overlay");
        assert_eq!(play.doc.cameras[0].name, "stall");
    }

    #[test]
    fn attack_away_from_stall_is_ignored() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 6.0);
        assert!(!player_at_stall(&play.doc));
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(!play.shop.bought, "off-stall J is not a buy");
        assert!(!play.shop.done);
        assert_eq!(play.doc.coins, START);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled);
    }

    #[test]
    fn buy_without_coins_is_ignored() {
        let mut play = play_started();
        play.shop.coins = PRICE - 1;
        play.doc.coins = PRICE - 1;
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(!play.shop.bought, "not enough coins");
        assert_eq!(play.doc.coins, PRICE - 1);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled);
    }

    #[test]
    fn second_buy_does_not_spend_again() {
        let mut play = play_started();
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.shop.bought);
        let coins = play.doc.coins;
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(
            play.doc.coins,
            coins,
            "bought flag is enough; no second spend"
        );
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_BOUGHT);
    }

    #[test]
    fn confirm_retry_restores_coins_and_flag() {
        let mut play = play_started();
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.shop.done);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.shop.done);
        assert!(!play.shop.bought);
        assert_eq!(play.doc.coins, START);
        assert_eq!(play.shop.coins, START);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "retry restores flag off");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
    }

    #[test]
    fn held_attack_buys_from_spawn() {
        let mut play = play_started();
        play.input.attack = true;
        for _ in 0..8 {
            play.tick(1.0 / 60.0);
            play.input.attack = true;
            if play.shop.done {
                break;
            }
        }
        assert!(
            play.shop.bought && play.shop.done,
            "held J/click at spawn should buy; coins={} name={:?}",
            play.doc.coins,
            play.doc.player.as_ref().unwrap().name
        );
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_BOUGHT);
        assert_eq!(play.doc.coins, START - PRICE);
    }

    #[test]
    fn other_genres_still_own_their_dumps() {
        let town = WorldPlay::from_json(TOWN).unwrap();
        assert!(town.is_rpg());
        assert!(!town.is_shop());
        let novel = WorldPlay::from_json(PAGES).unwrap();
        assert!(novel.is_novel());
        assert!(!novel.is_shop());
        let stealth = WorldPlay::from_json(HIDE).unwrap();
        assert!(stealth.is_stealth());
        assert!(!stealth.is_shop());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_shop());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_shop());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_shop());
        let puzzle = WorldPlay::from_json(ROOM).unwrap();
        assert!(puzzle.is_puzzle());
        assert!(!puzzle.is_shop());
        let hop = WorldPlay::from_json(HOP).unwrap();
        assert!(hop.is_platformer());
        assert!(!hop.is_shop());
        let sports = WorldPlay::from_json(PITCH).unwrap();
        assert!(sports.is_sports());
        assert!(!sports.is_shop());
        let sim = WorldPlay::from_json(YARD).unwrap();
        assert!(sim.is_sim());
        assert!(!sim.is_shop());
        let side = WorldPlay::from_json(SIDE).unwrap();
        assert!(side.is_action2d());
        assert!(!side.is_shop());
        let camp = WorldPlay::from_json(CAMP).unwrap();
        assert!(camp.is_survival());
        assert!(!camp.is_shop());
        let stage = WorldPlay::from_json(STAGE).unwrap();
        assert!(stage.is_rhythm());
        assert!(!stage.is_shop());
        let dock = WorldPlay::from_json(DOCK).unwrap();
        assert!(dock.is_fish());
        assert!(!dock.is_shop());
    }
}
