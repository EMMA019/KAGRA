//! Hunger/stamina meter ticks down as an M3 slice on play_world.
//!
//! Sibling of sim. Player walks a capsule; a box is the camp; a box is
//! the ration. Meter (`coins`) drains over time. Standing in the camp or
//! picking the ration fills it. Empty meter is dump-visible `starve`;
//! refill to full is dump-visible `ok`. Title -> play -> result reuses
//! `WorldPlay` / `GamePhase`. Capsules/boxes, not VRM. Indoor lights stay
//! 4 slots. Camp/ration stay readable (contact blob plus metal GGX
//! inherited). Chase camera follows play. Does not rewrite other genre
//! loops. No Rapier, SSAO, GI, inventory, or ECS. Picture slice stays.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};

pub const GAME_ID: &str = "survival_meter";
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const NEED: u32 = 8;
pub const DRAIN_PER_SEC: f32 = 3.2;
pub const FILL_PER_SEC: f32 = 6.0;
pub const RATION_FILL: f32 = 4.0;
pub const PICK_REACH: f32 = 1.35;
pub const CAM_BACK: f32 = 7.2;
pub const CAM_UP: f32 = 4.8;
pub const CAM_LOOK: f32 = 0.55;
pub const NAME_PLAYER: &str = "player";
pub const NAME_FILLING: &str = "filling";
pub const NAME_OK: &str = "ok";
pub const NAME_STARVE: &str = "starve";

const FLAG_OK_COLOR: [u32; 3] = [70, 180, 110];
const FILL_PIP: [u8; 4] = [240, 196, 72, 255];
const OK_PIP: [u8; 4] = [70, 180, 110, 255];
const LOW_PIP: [u8; 4] = [196, 64, 54, 255];

/// Live survival around a dump. Filling/ok/starve stay here; `name` + flag
/// enable and `coins` in the dump are the query source of truth. Meter is
/// time, not Rapier.
#[derive(Clone, Debug)]
pub struct SurvivalGame {
    pub filling: bool,
    pub ok: bool,
    pub starve: bool,
    pub done: bool,
    pub hunger: f32,
    pub coins: u32,
    pub picked: bool,
}

impl Default for SurvivalGame {
    fn default() -> Self {
        Self {
            filling: false,
            ok: false,
            starve: false,
            done: false,
            hunger: NEED as f32,
            coins: NEED,
            picked: false,
        }
    }
}

impl SurvivalGame {
    pub fn from_doc(_doc: &WorldDoc) -> Self {
        Self::default()
    }
}

pub fn is_survival(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "camp")
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

/// Sit capsules/boxes on the floor, keep the flag off, meter full, chase cam.
pub fn seed(doc: &mut WorldDoc) {
    if !is_survival(doc) {
        return;
    }
    sit_named(doc, NAME_PLAYER);
    sit_props(doc);
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = false;
        }
        if p.name == "ration" {
            p.enabled = true;
        }
    }
    doc.coins = NEED;
    place_chase_camera(doc);
}

/// Chase cam behind the player looking at the camp so the pad reads.
pub fn place_chase_camera(doc: &mut WorldDoc) {
    let Some(w) = player_ref(doc) else {
        return;
    };
    let px = w.position[0];
    let py = w.position[1];
    let pz = w.position[2];
    let yaw = w.yaw;
    let (s, c) = yaw.sin_cos();
    let (zx, zy, zz) = named_prop(doc, "camp")
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

/// Meter drains unless the player stands in camp or picks a ration.
/// Caller already walked; chase cam wins.
pub fn tick(doc: &mut WorldDoc, game: &mut SurvivalGame, _input: WalkInput, dt: f32) {
    if game.done {
        write_beat(doc, game);
        place_chase_camera(doc);
        return;
    }
    let inside = player_in_camp(doc);
    let picked_now = pick_ration(doc);
    if picked_now {
        game.picked = true;
    }
    let restoring = inside || picked_now;
    game.filling = restoring && !game.ok && !game.starve;
    let was = game.hunger;
    if restoring {
        let mut add = 0.0;
        if inside {
            add += FILL_PER_SEC * dt;
        }
        if picked_now {
            add += RATION_FILL;
        }
        game.hunger = (game.hunger + add).min(NEED as f32);
        if game.hunger >= NEED as f32 - 1e-3 && was < NEED as f32 - 1e-3 {
            game.ok = true;
            game.done = true;
            game.filling = false;
            game.hunger = NEED as f32;
        }
    } else {
        game.hunger = (game.hunger - DRAIN_PER_SEC * dt).max(0.0);
        if game.hunger <= 1e-3 {
            game.starve = true;
            game.done = true;
            game.filling = false;
            game.hunger = 0.0;
        }
    }
    game.coins = game.hunger.round().clamp(0.0, NEED as f32) as u32;
    write_beat(doc, game);
    place_chase_camera(doc);
}

fn beat_name(game: &SurvivalGame) -> &'static str {
    if game.starve {
        NAME_STARVE
    } else if game.ok {
        NAME_OK
    } else if game.filling {
        NAME_FILLING
    } else {
        NAME_PLAYER
    }
}

fn write_beat(doc: &mut WorldDoc, game: &SurvivalGame) {
    sit_named(doc, beat_name(game));
    doc.coins = game.coins;
    if game.ok {
        set_flag_prop(doc);
    }
}

fn set_flag_prop(doc: &mut WorldDoc) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = true;
            p.color = Some(FLAG_OK_COLOR);
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
            "camp" | "ration" | "flag" | "floor" | "post" => p.scale[1].abs() * 0.5,
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

fn player_in_camp(doc: &WorldDoc) -> bool {
    let Some(player) = player_ref(doc) else {
        return false;
    };
    let Some(camp) = named_prop(doc, "camp") else {
        return false;
    };
    let hx = camp.scale[0].abs() * 0.5 + PLAYER_R;
    let hz = camp.scale[2].abs() * 0.5 + PLAYER_R;
    let dx = player.position[0] - camp.position[0];
    let dz = player.position[2] - camp.position[2];
    dx.abs() <= hx && dz.abs() <= hz
}

fn pick_ration(doc: &mut WorldDoc) -> bool {
    let Some(player) = player_ref(doc) else {
        return false;
    };
    let px = player.position[0];
    let pz = player.position[2];
    let mut picked = false;
    for p in &mut doc.props {
        if !p.enabled || p.name != "ration" {
            continue;
        }
        let dx = px - p.position[0];
        let dz = pz - p.position[2];
        if (dx * dx + dz * dz).sqrt() <= PICK_REACH {
            p.enabled = false;
            picked = true;
        }
    }
    picked
}

pub fn build_hud(game: &SurvivalGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
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
            let frac = (game.hunger / NEED as f32).clamp(0.0, 1.0);
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                160.0 * scale,
                18.0 * scale,
                [28, 24, 20, 160],
            ));
            let pip = if game.filling {
                FILL_PIP
            } else if frac < 0.34 {
                LOW_PIP
            } else {
                [80, 84, 78, 200]
            };
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                (160.0 * scale * frac).max(if frac > 0.0 { 2.0 } else { 0.0 }),
                18.0 * scale,
                pip,
            ));
            quads.push(Quad::new(
                184.0 * scale,
                16.0 * scale,
                22.0 * scale,
                22.0 * scale,
                if game.ok { OK_PIP } else { [36, 48, 40, 160] },
            ));
        }
        GamePhase::Complete => {
            if game.starve {
                quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [28, 12, 12, 210]));
                quads.push(Quad::new(
                    w * 0.32,
                    h * 0.62,
                    w * 0.36,
                    48.0 * scale,
                    [196, 64, 54, 240],
                ));
            } else {
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
    }
    DrawList {
        clear: [24, 28, 34, 255],
        quads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

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
        let mut play = WorldPlay::from_json(CAMP).unwrap();
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
    fn dump_is_survival_not_other_genres() {
        let doc = WorldDoc::from_json(CAMP).unwrap();
        assert!(is_survival(&doc));
        assert_eq!(GAME_ID, "survival_meter");
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
        assert!(!is_survival(&crest));
        let yard = WorldDoc::from_json(YARD).unwrap();
        assert!(crate::sim::is_sim(&yard));
        assert!(!is_survival(&yard), "sim zone must not count as survival");
        let side = WorldDoc::from_json(SIDE).unwrap();
        assert!(crate::action2d::is_action2d(&side));
        assert!(!is_survival(&side));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "camp" && p.model == "box"));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "ration" && p.model == "box" && p.enabled));
        assert!(doc.props.iter().any(|p| p.name == "flag" && !p.enabled));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "floor" && p.model == "box"));
        let camp = doc.props.iter().find(|p| p.name == "camp").unwrap();
        assert!(
            camp.metallic >= 0.5,
            "camp rim should read metal GGX, metallic={}",
            camp.metallic
        );
        let ration = doc.props.iter().find(|p| p.name == "ration").unwrap();
        assert!(
            ration.metallic >= 0.5,
            "ration should read metal GGX, metallic={}",
            ration.metallic
        );
        assert_eq!(doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("camp"));
        assert!(json.contains("ration"));
        assert!(json.contains("flag"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 4,
            "floor + camp + ration must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(CAMP).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_survival());
        assert!(!play.is_sim());
        assert!(!play.is_action2d());
        assert!(!play.is_sports());
        assert!(!play.is_collectathon());
        let start = play.doc.player.as_ref().unwrap().position;
        let start_coins = play.doc.coins;
        play.input.lx = 1.0;
        play.input.lz = 1.0;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.survival.done, "title must not drain");
        assert_eq!(play.doc.player.as_ref().unwrap().position, start);
        assert_eq!(play.doc.coins, start_coins);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "chase");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn idle_ticks_down_then_starve() {
        let mut play = play_started();
        assert_eq!(play.doc.coins, NEED);
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.survival.done);
        assert!(
            play.doc.coins < NEED,
            "meter should drain, coins={}",
            play.doc.coins
        );
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        for _ in 0..240 {
            play.tick(1.0 / 60.0);
            if play.survival.done {
                break;
            }
        }
        assert!(play.survival.starve);
        assert!(play.survival.done);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_STARVE);
        assert_eq!(play.doc.coins, 0);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("starve"), "starve must be dump-visible");
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "starve does not raise the ok flag");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "starve overlay");
    }

    #[test]
    fn stand_in_camp_fills_to_ok() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 6.5);
        for _ in 0..60 {
            play.tick(1.0 / 60.0);
        }
        assert!(play.doc.coins < NEED, "drain away from camp first");
        let camp = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "camp")
            .unwrap()
            .position;
        put_player(&mut play, camp[0], camp[2]);
        for _ in 0..180 {
            play.tick(1.0 / 60.0);
            if play.survival.done {
                break;
            }
        }
        assert!(play.survival.ok);
        assert!(play.survival.done);
        assert!(!play.survival.starve);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_OK);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "ok flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_OK_COLOR));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("ok"), "ok must be dump-visible");
        assert_eq!(play.doc.coins, NEED);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "ok overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.survival.done);
        assert!(!play.survival.ok);
        assert!(!play.survival.starve);
        assert_eq!(play.doc.coins, NEED);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "retry restores flag off");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        let ration = play.doc.props.iter().find(|p| p.name == "ration").unwrap();
        assert!(ration.enabled, "retry restores ration");
    }

    #[test]
    fn pick_ration_fills_meter() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 6.5);
        for _ in 0..90 {
            play.tick(1.0 / 60.0);
        }
        let before = play.doc.coins;
        assert!(before <= NEED - 2, "need room to see a pick fill, coins={before}");
        let ration = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "ration")
            .unwrap()
            .position;
        put_player(&mut play, ration[0], ration[2]);
        play.tick(1.0 / 60.0);
        assert!(play.survival.picked);
        let ration = play.doc.props.iter().find(|p| p.name == "ration").unwrap();
        assert!(!ration.enabled, "picked ration disables in the dump");
        assert!(
            play.doc.coins > before,
            "pick should fill, before={before} after={}",
            play.doc.coins
        );
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("filling") || dump.contains("ok"));
    }

    #[test]
    fn walk_forward_oks_from_spawn() {
        let mut play = play_started();
        let cam0 = play.doc.cameras[0].position;
        play.input.lz = 1.0;
        for _ in 0..300 {
            play.tick(1.0 / 60.0);
            if play.survival.done {
                break;
            }
        }
        assert!(
            play.survival.ok && play.survival.done,
            "W from spawn should pick/camp and ok, starve={} filling={} coins={} pos={:?}",
            play.survival.starve,
            play.survival.filling,
            play.doc.coins,
            play.doc.player.as_ref().unwrap().position
        );
        assert!(!play.survival.starve);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_OK);
        assert_eq!(play.doc.cameras[0].name, "chase");
        let cam1 = play.doc.cameras[0].position;
        let d = (cam1[0] - cam0[0]).abs() + (cam1[2] - cam0[2]).abs();
        assert!(d > 0.2, "camera should follow play, delta={d}");
    }

    #[test]
    fn other_genres_still_own_their_dumps() {
        let town = WorldPlay::from_json(TOWN).unwrap();
        assert!(town.is_rpg());
        assert!(!town.is_survival());
        let novel = WorldPlay::from_json(PAGES).unwrap();
        assert!(novel.is_novel());
        assert!(!novel.is_survival());
        let stealth = WorldPlay::from_json(HIDE).unwrap();
        assert!(stealth.is_stealth());
        assert!(!stealth.is_survival());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_survival());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_survival());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_survival());
        let puzzle = WorldPlay::from_json(ROOM).unwrap();
        assert!(puzzle.is_puzzle());
        assert!(!puzzle.is_survival());
        let hop = WorldPlay::from_json(HOP).unwrap();
        assert!(hop.is_platformer());
        assert!(!hop.is_survival());
        let sports = WorldPlay::from_json(PITCH).unwrap();
        assert!(sports.is_sports());
        assert!(!sports.is_survival());
        let sim = WorldPlay::from_json(YARD).unwrap();
        assert!(sim.is_sim());
        assert!(!sim.is_survival());
        let side = WorldPlay::from_json(SIDE).unwrap();
        assert!(side.is_action2d());
        assert!(!side.is_survival());
    }
}
