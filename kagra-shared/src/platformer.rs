//! Platformer close on play_world: jump, fall, land, checkpoint retry.
//!
//! Sibling of `collectathon` / `action`. Capsule player on box platforms.
//! Title -> play -> result reuses `WorldPlay`. Fall death is visible; retry
//! restores the last checkpoint in the dump. No sprites, no Rapier, no VRM.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};

pub const GAME_ID: &str = "box_hop";
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const SNAP: f32 = 0.55;
pub const CHECK_REACH: f32 = 1.15;
pub const KILL_Y: f32 = -3.0;

#[derive(Clone, Debug)]
pub struct PlatformGame {
    pub dead: bool,
    pub won: bool,
    pub landed: u32,
    pub checkpoint: Option<[f32; 3]>,
    pub air_s: f32,
}

impl Default for PlatformGame {
    fn default() -> Self {
        Self {
            dead: false,
            won: false,
            landed: 0,
            checkpoint: None,
            air_s: 0.0,
        }
    }
}

impl PlatformGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let mut g = Self::default();
        if let Some(w) = player_ref(doc) {
            g.checkpoint = Some(w.position);
        }
        g
    }
}

pub fn is_platformer(doc: &WorldDoc) -> bool {
    doc.props
        .iter()
        .any(|p| p.name == "platform" || p.name == "checkpoint")
}

fn is_named_platform(p: &WorldProp) -> bool {
    p.enabled && p.name == "platform"
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

fn box_top(p: &WorldProp) -> f32 {
    p.position[1] + 0.5 * p.scale[1].abs()
}

fn xz_on_box(px: f32, pz: f32, p: &WorldProp) -> bool {
    let hx = 0.5 * p.scale[0].abs() + PLAYER_R * 0.35;
    let hz = 0.5 * p.scale[2].abs() + PLAYER_R * 0.35;
    (px - p.position[0]).abs() <= hx && (pz - p.position[2]).abs() <= hz
}

/// Sit the player on the start platform. Dump platforms stay as authored.
pub fn seed(doc: &mut WorldDoc) {
    if !is_platformer(doc) {
        return;
    }
    let Some(plat) = doc.props.iter().find(|p| is_named_platform(p)) else {
        return;
    };
    let top = box_top(plat);
    let Some(mut w) = player_ref(doc).cloned() else {
        return;
    };
    if (w.position[1] - (top + BODY_H)).abs() > 0.2 {
        w.position[1] = top + BODY_H;
        w.on_ground = true;
        write_player(doc, w);
    }
}

/// Land on boxes, fall-kill, checkpoint, goal. Caller already stepped walker.
pub fn tick(doc: &mut WorldDoc, game: &mut PlatformGame, vy: &mut f32, _input: WalkInput, dt: f32) {
    if game.dead || game.won {
        return;
    }
    let Some(mut w) = player_ref(doc).cloned() else {
        return;
    };
    let landed = land_on_boxes(&mut w, doc, *vy);
    if landed {
        *vy = 0.0;
        if game.air_s > 0.04 {
            game.landed += 1;
        }
        game.air_s = 0.0;
        w.name = "player".into();
    } else {
        game.air_s += dt;
        if *vy < -1.0 {
            w.name = "hurt".into();
        }
    }
    write_player(doc, w.clone());
    touch_checkpoint(doc, game, &w);
    touch_goal(doc, game, &w);
    if w.position[1] - BODY_H < KILL_Y {
        die(doc, game);
    }
}

fn land_on_boxes(w: &mut WorldWalker, doc: &WorldDoc, vy: f32) -> bool {
    let feet = w.position[1] - BODY_H;
    let mut best: Option<f32> = None;
    for p in &doc.props {
        if !is_named_platform(p) {
            continue;
        }
        if !xz_on_box(w.position[0], w.position[2], p) {
            continue;
        }
        let top = box_top(p);
        if vy > 1.2 {
            continue;
        }
        if feet <= top + 0.12 && feet >= top - SNAP {
            best = Some(best.map_or(top, |b| b.max(top)));
        }
    }
    if let Some(top) = best {
        w.position[1] = top + BODY_H;
        w.on_ground = true;
        true
    } else {
        false
    }
}

fn touch_checkpoint(doc: &WorldDoc, game: &mut PlatformGame, w: &WorldWalker) {
    for p in &doc.props {
        if !p.enabled || p.name != "checkpoint" {
            continue;
        }
        let dx = w.position[0] - p.position[0];
        let dz = w.position[2] - p.position[2];
        if (dx * dx + dz * dz).sqrt() <= CHECK_REACH {
            game.checkpoint = Some(w.position);
        }
    }
}

fn touch_goal(doc: &WorldDoc, game: &mut PlatformGame, w: &WorldWalker) {
    for p in &doc.props {
        if !p.enabled || p.name != "goal" {
            continue;
        }
        let dx = w.position[0] - p.position[0];
        let dz = w.position[2] - p.position[2];
        if (dx * dx + dz * dz).sqrt() <= CHECK_REACH {
            game.won = true;
        }
    }
}

fn die(doc: &mut WorldDoc, game: &mut PlatformGame) {
    game.dead = true;
    let Some(mut w) = player_ref(doc).cloned() else {
        return;
    };
    w.name = "dead".into();
    w.on_ground = true;
    write_player(doc, w);
}

pub fn restore_checkpoint(doc: &mut WorldDoc, game: &PlatformGame) {
    let Some(pos) = game.checkpoint else {
        return;
    };
    let Some(mut w) = player_ref(doc).cloned() else {
        return;
    };
    w.position = pos;
    w.name = "player".into();
    w.on_ground = true;
    write_player(doc, w);
}

pub fn camera_tracks_body(doc: &WorldDoc) -> bool {
    let Some(w) = player_ref(doc) else {
        return false;
    };
    let Some(cam) = doc.cameras.first() else {
        return false;
    };
    let dx = cam.target[0] - w.position[0];
    let dz = cam.target[2] - w.position[2];
    (dx * dx + dz * dz).sqrt() < 1.25
}

pub fn build_hud(game: &PlatformGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let mut quads = Vec::new();
    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [8, 12, 22, 160]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [16, 28, 48, 230],
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.58,
                w * 0.36,
                52.0 * scale,
                [80, 170, 230, 255],
            ));
        }
        GamePhase::Playing => {
            let pip = 18.0 * scale;
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                pip,
                pip,
                [80, 170, 230, 255],
            ));
            if game.checkpoint.is_some() {
                quads.push(Quad::new(
                    16.0 * scale + pip + 8.0 * scale,
                    16.0 * scale,
                    pip,
                    pip,
                    [240, 196, 72, 255],
                ));
            }
            if game.air_s > 0.12 {
                quads.push(Quad::new(0.0, 0.0, w, 10.0 * scale, [255, 255, 255, 40]));
            }
        }
        GamePhase::Complete => {
            if game.dead {
                quads.push(Quad::new(0.0, 0.0, w, h, [20, 8, 28, 190]));
                quads.push(Quad::new(
                    w * 0.18,
                    h * 0.28,
                    w * 0.64,
                    h * 0.22,
                    [40, 16, 50, 230],
                ));
            } else {
                quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 16, 28, 210]));
            }
            quads.push(Quad::new(
                w * 0.32,
                h * 0.62,
                w * 0.36,
                48.0 * scale,
                if game.dead {
                    [180, 80, 160, 240]
                } else {
                    [70, 160, 110, 240]
                },
            ));
        }
    }
    DrawList {
        clear: [110, 150, 200, 255],
        quads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const HOP: &str = include_str!("../tests/fixtures/box_hop_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");

    fn started() -> WorldPlay {
        let mut play = WorldPlay::from_json(HOP).unwrap();
        play.start();
        play
    }

    fn put(play: &mut WorldPlay, x: f32, y: f32, z: f32) {
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [x, y, z];
            p.on_ground = true;
            p.name = "player".into();
        }
        let w = play.doc.player.clone().unwrap();
        write_player(&mut play.doc, w);
    }

    #[test]
    fn dump_is_platformer_not_other_genres() {
        let doc = WorldDoc::from_json(HOP).unwrap();
        assert!(is_platformer(&doc));
        assert!(!crate::action::is_action(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_platformer(&crest));
        let arena = WorldDoc::from_json(ARENA).unwrap();
        assert!(!is_platformer(&arena));
        assert!(doc.props.iter().any(|p| p.name == "platform"));
        assert!(doc.props.iter().any(|p| p.name == "checkpoint"));
        assert!(doc.player.as_ref().unwrap().on_ground);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(HOP).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_platformer());
        let z = play.doc.player.as_ref().unwrap().position[2];
        play.input.lz = 1.0;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert_eq!(play.doc.player.as_ref().unwrap().position[2], z);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
    }

    #[test]
    fn jump_leaves_box_and_lands() {
        let mut play = started();
        let y0 = play.doc.player.as_ref().unwrap().position[1];
        play.input.jump = true;
        play.tick(1.0 / 60.0);
        play.input.jump = false;
        assert!(
            !play.doc.player.as_ref().unwrap().on_ground
                || play.doc.player.as_ref().unwrap().position[1] > y0
        );
        let mut peaked = play.doc.player.as_ref().unwrap().position[1];
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
            peaked = peaked.max(play.doc.player.as_ref().unwrap().position[1]);
        }
        assert!(peaked > y0 + 0.4, "jump peak {peaked} from {y0}");
        for _ in 0..90 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap();
        assert!(p.on_ground, "should land on the start box");
        assert!(
            p.position[1] > 0.5,
            "must not fall to the pit, y={}",
            p.position[1]
        );
        assert!(play.platform.landed >= 1);
    }

    #[test]
    fn fall_into_pit_dies_camera_keeps_body() {
        let mut play = started();
        put(&mut play, 0.0, 2.0, -8.0);
        play.doc.player.as_mut().unwrap().on_ground = false;
        let w = play.doc.player.clone().unwrap();
        write_player(&mut play.doc, w);
        let mut n = 0;
        while play.game.phase == GamePhase::Playing && n < 240 {
            play.tick(1.0 / 60.0);
            n += 1;
        }
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert!(play.platform.dead);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "dead");
        assert!(camera_tracks_body(&play.doc));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("dead"));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn checkpoint_retry_restores_dump_at_flag() {
        let mut play = started();
        let ck = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "checkpoint")
            .unwrap()
            .clone();
        put(
            &mut play,
            ck.position[0],
            box_top(&ck) + BODY_H,
            ck.position[2],
        );
        play.tick(1.0 / 60.0);
        assert!(play.platform.checkpoint.is_some());
        put(&mut play, 0.0, 2.0, -8.0);
        play.doc.player.as_mut().unwrap().on_ground = false;
        let w = play.doc.player.clone().unwrap();
        write_player(&mut play.doc, w);
        let mut n = 0;
        while play.game.phase == GamePhase::Playing && n < 240 {
            play.tick(1.0 / 60.0);
            n += 1;
        }
        assert!(play.platform.dead);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.platform.dead);
        let p = play.doc.player.as_ref().unwrap().position;
        let dx = p[0] - ck.position[0];
        let dz = p[2] - ck.position[2];
        assert!(
            (dx * dx + dz * dz).sqrt() < 1.5,
            "retry must spawn at checkpoint, pos={p:?} ck={:?}",
            ck.position
        );
        assert_eq!(play.doc.player.as_ref().unwrap().name, "player");
    }

    #[test]
    fn action_and_collectathon_untouched() {
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_platformer());
        let arena = WorldPlay::from_json(ARENA).unwrap();
        assert!(arena.is_action());
        assert!(!arena.is_platformer());
    }
}
