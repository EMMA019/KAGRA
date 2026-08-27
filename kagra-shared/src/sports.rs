//! Kick a ball into a goal as M3 genre on play_world.
//!
//! Sibling of puzzle / stealth. Player walks a capsule; a sphere is a
//! kinematic ball; a goal box is the volume. Overlap applies an impulse
//! (no Rapier). Ball entering the goal is scored. Title -> play -> result
//! reuses `WorldPlay` / `GamePhase`. Dump-visible name (player/kicking/scored)
//! and flag + coins. Capsules/spheres/boxes, not VRM. Outdoor lights stay
//! 4 slots. Pitch and goal stay readable (contact blob plus metal GGX
//! inherited). Chase camera follows play. Does not rewrite other genre
//! loops. No FIFA, net, inventory, vehicles, or sokoban editor.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};

pub const GAME_ID: &str = "sports_goal";
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const KICK_SPEED: f32 = 12.0;
pub const BALL_DAMP: f32 = 0.85;
pub const CAM_BACK: f32 = 8.4;
pub const CAM_UP: f32 = 5.1;
pub const CAM_LOOK: f32 = 0.55;
pub const NAME_PLAYER: &str = "player";
pub const NAME_KICKING: &str = "kicking";
pub const NAME_SCORED: &str = "scored";

const FLAG_SCORED_COLOR: [u32; 3] = [70, 180, 110];
const KICK_PIP: [u8; 4] = [240, 196, 72, 255];
const SCORED_PIP: [u8; 4] = [70, 180, 110, 255];

/// Live sports around a dump. Kicking/scored stay here; `name` + flag
/// enable in the dump are the query source of truth. Ball velocity is
/// kinematic (impulse + damp), not Rapier.
#[derive(Clone, Debug, Default)]
pub struct SportsGame {
    pub kicking: bool,
    pub scored: bool,
    pub done: bool,
    pub ball_vx: f32,
    pub ball_vz: f32,
}

impl SportsGame {
    pub fn from_doc(_doc: &WorldDoc) -> Self {
        Self::default()
    }
}

pub fn is_sports(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "ball")
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

/// Sit capsules/spheres/boxes on the pitch, keep the flag off, chase cam.
pub fn seed(doc: &mut WorldDoc) {
    if !is_sports(doc) {
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

/// Chase cam behind the player looking at the ball so pitch + goal read.
pub fn place_chase_camera(doc: &mut WorldDoc) {
    let Some(w) = player_ref(doc) else {
        return;
    };
    let px = w.position[0];
    let py = w.position[1];
    let pz = w.position[2];
    let yaw = w.yaw;
    let (s, c) = yaw.sin_cos();
    let (bx, by, bz) = named_prop(doc, "ball")
        .map(|p| (p.position[0], p.position[1], p.position[2]))
        .unwrap_or((px, py, pz - 2.0));
    let eye = [px - s * CAM_BACK, py + CAM_UP, pz - c * CAM_BACK];
    let target = [bx * 0.7 + px * 0.3, by.max(CAM_LOOK), bz * 0.7 + pz * 0.3];
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

/// Kinematic kick + roll + goal tick. Caller already walked; chase cam wins.
pub fn tick(doc: &mut WorldDoc, game: &mut SportsGame, _input: WalkInput, dt: f32) {
    if game.done {
        write_beat(doc, game);
        place_chase_camera(doc);
        return;
    }
    game.kicking = kick_ball(doc, game);
    roll_ball(doc, game, dt);
    if ball_in_goal(doc) {
        game.scored = true;
        game.done = true;
        game.kicking = false;
        game.ball_vx = 0.0;
        game.ball_vz = 0.0;
    }
    write_beat(doc, game);
    place_chase_camera(doc);
}

fn beat_name(game: &SportsGame) -> &'static str {
    if game.done || game.scored {
        NAME_SCORED
    } else if game.kicking {
        NAME_KICKING
    } else {
        NAME_PLAYER
    }
}

fn write_beat(doc: &mut WorldDoc, game: &SportsGame) {
    sit_named(doc, beat_name(game));
    doc.coins = if game.done || game.scored {
        10
    } else if game.kicking {
        1
    } else {
        0
    };
    if game.done || game.scored {
        set_flag_prop(doc);
    }
}

fn set_flag_prop(doc: &mut WorldDoc) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = true;
            p.color = Some(FLAG_SCORED_COLOR);
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
            "ball" => p.scale[1].abs() * 0.5,
            "goal" | "post" | "bar" | "flag" | "wall" => p.scale[1].abs() * 0.5,
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

fn ball_radius(p: &WorldProp) -> f32 {
    p.scale[0].abs().max(p.scale[2].abs()) * 0.5
}

/// Circle (player) vs ball circle on XZ. Impulse along player?ball.
/// Returns true when an impulse was applied this tick.
fn kick_ball(doc: &WorldDoc, game: &mut SportsGame) -> bool {
    let Some(player) = player_ref(doc) else {
        return false;
    };
    let px = player.position[0];
    let pz = player.position[2];
    let yaw = player.yaw;
    let Some(ball) = named_prop(doc, "ball") else {
        return false;
    };
    let bx = ball.position[0];
    let bz = ball.position[2];
    let r = PLAYER_R + ball_radius(ball);
    let dx = bx - px;
    let dz = bz - pz;
    let dist_sq = dx * dx + dz * dz;
    if dist_sq >= r * r {
        return false;
    }
    let (nx, nz) = if dist_sq < 1e-8 {
        let (s, c) = yaw.sin_cos();
        (s, c)
    } else {
        let dist = dist_sq.sqrt();
        (dx / dist, dz / dist)
    };
    game.ball_vx = nx * KICK_SPEED;
    game.ball_vz = nz * KICK_SPEED;
    true
}

fn roll_ball(doc: &mut WorldDoc, game: &mut SportsGame, dt: f32) {
    let Some(idx) = named_prop_index(doc, "ball") else {
        return;
    };
    let r = ball_radius(&doc.props[idx]);
    let mut x = doc.props[idx].position[0] + game.ball_vx * dt;
    let mut z = doc.props[idx].position[2] + game.ball_vz * dt;
    let damp = (1.0 - BALL_DAMP * dt).clamp(0.0, 1.0);
    game.ball_vx *= damp;
    game.ball_vz *= damp;
    let half = doc.half.max(4.0);
    if x < -half + r {
        x = -half + r;
        game.ball_vx = game.ball_vx.abs();
    } else if x > half - r {
        x = half - r;
        game.ball_vx = -game.ball_vx.abs();
    }
    if z > half - r {
        z = half - r;
        game.ball_vz = -game.ball_vz.abs();
    } else if z < -half + r {
        z = -half + r;
        game.ball_vz = game.ball_vz.abs();
    }
    bounce_walls(doc, &mut x, &mut z, &mut game.ball_vx, &mut game.ball_vz, r);
    let y = doc.height_at(x, z) + r;
    if let Some(p) = doc.props.get_mut(idx) {
        p.position[0] = x;
        p.position[1] = y;
        p.position[2] = z;
    }
}

fn bounce_walls(doc: &WorldDoc, x: &mut f32, z: &mut f32, vx: &mut f32, vz: &mut f32, r: f32) {
    for w in &doc.props {
        if w.name != "wall" || !w.enabled {
            continue;
        }
        let hx = w.scale[0].abs() * 0.5 + r;
        let hz = w.scale[2].abs() * 0.5 + r;
        let dx = *x - w.position[0];
        let dz = *z - w.position[2];
        if dx.abs() > hx || dz.abs() > hz {
            continue;
        }
        if (hx - dx.abs()) <= (hz - dz.abs()) {
            let sx = if dx >= 0.0 { 1.0 } else { -1.0 };
            *x = w.position[0] + hx * sx;
            *vx = -*vx;
        } else {
            let sz = if dz >= 0.0 { 1.0 } else { -1.0 };
            *z = w.position[2] + hz * sz;
            *vz = -*vz;
        }
    }
}

fn ball_in_goal(doc: &WorldDoc) -> bool {
    let Some(ball) = named_prop(doc, "ball") else {
        return false;
    };
    let Some(goal) = named_prop(doc, "goal") else {
        return false;
    };
    let hx = goal.scale[0].abs() * 0.5;
    let hy = goal.scale[1].abs() * 0.5;
    let hz = goal.scale[2].abs() * 0.5;
    (ball.position[0] - goal.position[0]).abs() <= hx * 0.95
        && (ball.position[2] - goal.position[2]).abs() <= hz * 0.95
        && (ball.position[1] - goal.position[1]).abs() <= hy + ball_radius(ball)
}

pub fn build_hud(game: &SportsGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let mut quads = Vec::new();
    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [10, 18, 12, 160]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [18, 32, 22, 230],
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
                if game.kicking {
                    KICK_PIP
                } else {
                    [28, 24, 20, 160]
                },
            ));
            quads.push(Quad::new(
                44.0 * scale,
                16.0 * scale,
                22.0 * scale,
                22.0 * scale,
                if game.scored {
                    SCORED_PIP
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
        clear: [28, 42, 32, 255],
        quads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const PITCH: &str = include_str!("../tests/fixtures/sports_goal_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const TOWN: &str = include_str!("../tests/fixtures/rpg_town_world.json");
    const PAGES: &str = include_str!("../tests/fixtures/novel_pages_world.json");
    const HIDE: &str = include_str!("../tests/fixtures/stealth_hide_world.json");
    const RING: &str = include_str!("../tests/fixtures/fight_hitstun_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");
    const ROOM: &str = include_str!("../tests/fixtures/puzzle_pad_world.json");
    const HOP: &str = include_str!("../tests/fixtures/box_hop_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(PITCH).unwrap();
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

    fn ball_xz(play: &WorldPlay) -> (f32, f32) {
        let p = play.doc.props.iter().find(|p| p.name == "ball").unwrap();
        (p.position[0], p.position[2])
    }

    fn put_ball(play: &mut WorldPlay, x: f32, z: f32) {
        let y = play.doc.height_at(x, z);
        for p in &mut play.doc.props {
            if p.name == "ball" {
                p.position[0] = x;
                p.position[2] = z;
                p.position[1] = y + p.scale[1].abs() * 0.5;
            }
        }
    }

    #[test]
    fn dump_is_sports_not_other_genres() {
        let doc = WorldDoc::from_json(PITCH).unwrap();
        assert!(is_sports(&doc));
        assert_eq!(GAME_ID, "sports_goal");
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
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_sports(&crest));
        let hop = WorldDoc::from_json(HOP).unwrap();
        assert!(crate::platformer::is_platformer(&hop));
        assert!(!is_sports(&hop), "platformer goal must not count as sports");
        let room = WorldDoc::from_json(ROOM).unwrap();
        assert!(!is_sports(&room));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "ball" && p.model == "sphere"));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "goal" && p.model == "box"));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "pitch" && p.model == "box"));
        let ball = doc.props.iter().find(|p| p.name == "ball").unwrap();
        assert!(
            ball.metallic >= 0.5,
            "ball should read metal GGX, metallic={}",
            ball.metallic
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
        assert!(json.contains("ball"));
        assert!(json.contains("goal"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 6,
            "pitch + goal must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(PITCH).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_sports());
        assert!(!play.is_puzzle());
        assert!(!play.is_stealth());
        assert!(!play.is_platformer());
        assert!(!play.is_collectathon());
        let start = play.doc.player.as_ref().unwrap().position;
        let ball0 = ball_xz(&play);
        play.input.lx = 1.0;
        play.input.lz = 1.0;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.sports.done, "title must not score");
        assert_eq!(play.doc.player.as_ref().unwrap().position, start);
        assert_eq!(ball_xz(&play), ball0);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "chase");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn walk_into_ball_kicks_it() {
        let mut play = play_started();
        let (bx, bz) = ball_xz(&play);
        put_player(&mut play, bx, bz + 0.7);
        play.tick(1.0 / 60.0);
        assert!(play.sports.kicking, "overlap must kick");
        assert!(!play.sports.done);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_KICKING);
        let (bx2, bz2) = ball_xz(&play);
        assert!(
            (bz2 - bz).abs() > 0.02 || (bx2 - bx).abs() > 0.02,
            "ball should move, before=({bx},{bz}) after=({bx2},{bz2})"
        );
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("kicking"), "kicking must be dump-visible");
        assert_eq!(play.doc.coins, 1);
        assert_eq!(play.doc.cameras[0].name, "chase");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "kicking pip overlay");
    }

    #[test]
    fn ball_in_goal_is_scored() {
        let mut play = play_started();
        let goal = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "goal")
            .unwrap()
            .position;
        put_ball(&mut play, goal[0], goal[2]);
        play.tick(1.0 / 60.0);
        assert!(play.sports.scored);
        assert!(play.sports.done);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_SCORED);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "scored flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_SCORED_COLOR));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("scored"));
        assert_eq!(play.doc.coins, 10);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "scored overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.sports.done);
        assert!(!play.sports.scored);
        assert!(!play.sports.kicking);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "retry restores flag off");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
    }

    #[test]
    fn walk_forward_kicks_ball_into_goal() {
        let mut play = play_started();
        let cam0 = play.doc.cameras[0].position;
        play.input.lz = 1.0;
        for _ in 0..240 {
            play.tick(1.0 / 60.0);
            if play.sports.done {
                break;
            }
        }
        assert!(
            play.sports.scored && play.sports.done,
            "W from spawn should kick ball into goal, kicking={} ball={:?}",
            play.sports.kicking,
            ball_xz(&play)
        );
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_SCORED);
        assert_eq!(play.doc.cameras[0].name, "chase");
        let cam1 = play.doc.cameras[0].position;
        let d = (cam1[0] - cam0[0]).abs() + (cam1[2] - cam0[2]).abs();
        assert!(d > 0.2, "camera should follow play, delta={d}");
    }

    #[test]
    fn other_genres_still_own_their_dumps() {
        let town = WorldPlay::from_json(TOWN).unwrap();
        assert!(town.is_rpg());
        assert!(!town.is_sports());
        let novel = WorldPlay::from_json(PAGES).unwrap();
        assert!(novel.is_novel());
        assert!(!novel.is_sports());
        let stealth = WorldPlay::from_json(HIDE).unwrap();
        assert!(stealth.is_stealth());
        assert!(!stealth.is_sports());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_sports());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_sports());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_sports());
        let puzzle = WorldPlay::from_json(ROOM).unwrap();
        assert!(puzzle.is_puzzle());
        assert!(!puzzle.is_sports());
        let hop = WorldPlay::from_json(HOP).unwrap();
        assert!(hop.is_platformer());
        assert!(!hop.is_sports());
    }
}
