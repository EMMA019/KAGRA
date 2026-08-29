//! Action close on play_world: hit, dodge, kill, die, retry.
//!
//! Sibling of `collectathon`. Capsule player vs capsule / box foes on a
//! World.dump. Title -> play -> result reuses `WorldPlay` / `GamePhase`.
//! Hit flash and death overlay are `DrawList` quads on shared wgpu 30.
//! No Rapier, no VRM skin, no RendererV2, no new ECS.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};
use glam::Vec3;
use std::collections::{HashMap, HashSet};

pub const GAME_ID: &str = "action_arena";
pub const PLAYER_HP: u32 = 3;
pub const FOE_HP: u32 = 2;
pub const PLAYER_R: f32 = 0.46;
pub const FOE_CAPSULE_R: f32 = 0.50;
pub const ATTACK_REACH: f32 = 1.45;
pub const ATTACK_R: f32 = 0.70;
pub const ATTACK_TIME: f32 = 0.22;
pub const DODGE_TIME: f32 = 0.26;
pub const DODGE_IFRAME: f32 = 0.38;
pub const DODGE_SPEED: f32 = 13.0;
pub const HIT_FLASH: f32 = 0.20;
pub const CONTACT_CD: f32 = 0.55;
pub const FOE_SPEED: f32 = 2.2;
pub const BODY_H: f32 = 0.95;

/// Live combat around a dump. HP / swing / i-frames stay here; foes in the
/// dump (`name == "foe"`) are the query/dump source of truth when killed.
#[derive(Clone, Debug)]
pub struct ActionGame {
    pub hp: u32,
    pub hits: u32,
    pub kills: u32,
    pub attack_t: f32,
    pub dodge_t: f32,
    pub iframe_t: f32,
    pub flash_t: f32,
    pub hurt_flash: bool,
    pub contact_cd: f32,
    pub dead: bool,
    pub won: bool,
    foe_hp: HashMap<String, u32>,
    swing_hit: HashSet<String>,
}

impl Default for ActionGame {
    fn default() -> Self {
        Self {
            hp: PLAYER_HP,
            hits: 0,
            kills: 0,
            attack_t: 0.0,
            dodge_t: 0.0,
            iframe_t: 0.0,
            flash_t: 0.0,
            hurt_flash: false,
            contact_cd: 0.0,
            dead: false,
            won: false,
            foe_hp: HashMap::new(),
            swing_hit: HashSet::new(),
        }
    }
}

impl ActionGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let mut g = Self::default();
        g.rebind_foes(doc);
        g
    }

    fn rebind_foes(&mut self, doc: &WorldDoc) {
        self.foe_hp.clear();
        for p in &doc.props {
            if is_foe(p) && p.enabled {
                self.foe_hp.insert(p.id.clone(), FOE_HP);
            }
        }
    }
}

pub fn is_action(doc: &WorldDoc) -> bool {
    doc.props.iter().any(is_foe)
}

fn is_foe(p: &WorldProp) -> bool {
    p.name == "foe"
}

fn is_box_foe(p: &WorldProp) -> bool {
    let m = p.model.to_ascii_lowercase();
    m == "box" || m == "cube"
}

fn foe_radius(p: &WorldProp) -> f32 {
    if is_box_foe(p) {
        0.5 * p.scale[0].abs().max(p.scale[2].abs()).max(0.4)
    } else {
        FOE_CAPSULE_R * p.scale[0].abs().max(0.4)
    }
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

fn set_player_name(doc: &mut WorldDoc, name: &str) {
    let Some(mut w) = player_ref(doc).cloned() else {
        return;
    };
    w.name = name.into();
    write_player(doc, w);
}

/// Sit foes on the floor. Does not spawn extra foes (dump is source of truth).
pub fn seed(doc: &mut WorldDoc) {
    if !is_action(doc) {
        return;
    }
    let mut ys = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        if !is_foe(p) || !p.enabled {
            continue;
        }
        let extra = if is_box_foe(p) {
            0.5 * p.scale[1].abs().max(0.4)
        } else {
            BODY_H
        };
        let y = doc.height_at(p.position[0], p.position[2]) + extra;
        ys.push((i, y));
    }
    for (i, y) in ys {
        if let Some(p) = doc.props.get_mut(i) {
            p.position[1] = y;
        }
    }
}

/// Combat + foe chase. Caller already stepped the walker and chase camera.
pub fn tick(doc: &mut WorldDoc, game: &mut ActionGame, input: WalkInput, look_yaw: f32, dt: f32) {
    if game.dead || game.won {
        return;
    }
    game.attack_t = (game.attack_t - dt).max(0.0);
    game.dodge_t = (game.dodge_t - dt).max(0.0);
    game.iframe_t = (game.iframe_t - dt).max(0.0);
    game.flash_t = (game.flash_t - dt).max(0.0);
    game.contact_cd = (game.contact_cd - dt).max(0.0);
    if game.flash_t <= 0.0 && !game.dead {
        set_player_name(doc, "player");
        game.hurt_flash = false;
    }

    apply_dodge(doc, game, input, look_yaw, dt);
    apply_attack(doc, game, input);
    chase_foes(doc, game, dt);
    apply_contact(doc, game);

    if game.hp == 0 && !game.dead {
        die(doc, game);
    } else if live_foes(doc) == 0 {
        game.won = true;
    }
}

fn live_foes(doc: &WorldDoc) -> usize {
    doc.props.iter().filter(|p| is_foe(p) && p.enabled).count()
}

fn apply_dodge(
    doc: &mut WorldDoc,
    game: &mut ActionGame,
    input: WalkInput,
    look_yaw: f32,
    dt: f32,
) {
    if input.dodge && game.dodge_t <= 0.0 {
        game.dodge_t = DODGE_TIME;
        game.iframe_t = DODGE_IFRAME.max(game.iframe_t);
    }
    if game.dodge_t <= 0.0 {
        return;
    }
    let Some(mut w) = player_ref(doc).cloned() else {
        return;
    };
    let (s, c) = look_yaw.sin_cos();
    let fwd = Vec3::new(s, 0.0, c);
    // Chase cam looks along +fwd from behind: screen-right = fwd × up = (-c, 0, s).
    let right = Vec3::new(-c, 0.0, s);
    let mut wish = right * input.lx + fwd * input.lz;
    if wish.length() < 0.08 {
        wish = Vec3::new(w.yaw.sin(), 0.0, w.yaw.cos());
    } else {
        wish = wish.normalize();
    }
    let half = doc.half.max(4.0);
    let pad = 2.0;
    w.position[0] = (w.position[0] + wish.x * DODGE_SPEED * dt).clamp(-half + pad, half - pad);
    w.position[2] = (w.position[2] + wish.z * DODGE_SPEED * dt).clamp(-half + pad, half - pad);
    let ground = doc.height_at(w.position[0], w.position[2]) + BODY_H;
    if w.position[1] < ground {
        w.position[1] = ground;
        w.on_ground = true;
    }
    write_player(doc, w);
}

fn apply_attack(doc: &mut WorldDoc, game: &mut ActionGame, input: WalkInput) {
    if input.attack && game.attack_t <= 0.0 {
        game.attack_t = ATTACK_TIME;
        game.swing_hit.clear();
    }
    if game.attack_t <= 0.0 {
        return;
    }
    let Some(w) = player_ref(doc) else {
        return;
    };
    let (s, c) = w.yaw.sin_cos();
    let ax = w.position[0] + s * ATTACK_REACH * 0.62;
    let az = w.position[2] + c * ATTACK_REACH * 0.62;
    let mut hits: Vec<String> = Vec::new();
    for p in &doc.props {
        if !is_foe(p) || !p.enabled {
            continue;
        }
        if game.swing_hit.contains(&p.id) {
            continue;
        }
        let r = foe_radius(p) + ATTACK_R;
        let dx = ax - p.position[0];
        let dz = az - p.position[2];
        if dx * dx + dz * dz <= r * r {
            hits.push(p.id.clone());
        }
    }
    for id in hits {
        game.swing_hit.insert(id.clone());
        let hp = game.foe_hp.entry(id.clone()).or_insert(FOE_HP);
        *hp = hp.saturating_sub(1);
        game.hits += 1;
        game.flash_t = HIT_FLASH;
        game.hurt_flash = false;
        set_player_name(doc, "hurt");
        if *hp == 0 {
            if let Some(p) = doc.props.iter_mut().find(|p| p.id == id) {
                p.enabled = false;
            }
            game.kills += 1;
        }
    }
}

fn chase_foes(doc: &mut WorldDoc, game: &ActionGame, dt: f32) {
    if game.dead {
        return;
    }
    let Some(w) = player_ref(doc) else {
        return;
    };
    let px = w.position[0];
    let pz = w.position[2];
    let half = doc.half.max(4.0);
    let mut moves: Vec<(usize, f32, f32, f32)> = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        if !is_foe(p) || !p.enabled {
            continue;
        }
        let dx = px - p.position[0];
        let dz = pz - p.position[2];
        let dist = (dx * dx + dz * dz).sqrt();
        let stop = foe_radius(p) + PLAYER_R + 0.08;
        if dist <= stop + 0.02 {
            continue;
        }
        let nx = dx / dist.max(1e-4);
        let nz = dz / dist.max(1e-4);
        let x = (p.position[0] + nx * FOE_SPEED * dt).clamp(-half + 1.0, half - 1.0);
        let z = (p.position[2] + nz * FOE_SPEED * dt).clamp(-half + 1.0, half - 1.0);
        let extra = if is_box_foe(p) {
            0.5 * p.scale[1].abs().max(0.4)
        } else {
            BODY_H
        };
        let y = doc.height_at(x, z) + extra;
        moves.push((i, x, y, z));
    }
    for (i, x, y, z) in moves {
        if let Some(p) = doc.props.get_mut(i) {
            p.position = [x, y, z];
            p.yaw = (px - x).atan2(pz - z);
        }
    }
}

fn apply_contact(doc: &mut WorldDoc, game: &mut ActionGame) {
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
        if !is_foe(p) || !p.enabled {
            continue;
        }
        let r = foe_radius(p) + PLAYER_R;
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
    game.iframe_t = DODGE_IFRAME * 0.7;
    game.contact_cd = CONTACT_CD;
    game.flash_t = HIT_FLASH;
    game.hurt_flash = true;
    set_player_name(doc, "hurt");
}

fn die(doc: &mut WorldDoc, game: &mut ActionGame) {
    game.dead = true;
    game.hp = 0;
    set_player_name(doc, "dead");
    let Some(mut w) = player_ref(doc).cloned() else {
        return;
    };
    // Keep the body on the floor; camera still has a target.
    let ground = doc.height_at(w.position[0], w.position[2]);
    w.position[1] = ground + 0.28;
    w.on_ground = true;
    write_player(doc, w);
}

/// Font-free HUD: HP pips, hit flash, death / clear overlay. Result = retry.
pub fn build_hud(game: &ActionGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let pad = 16.0 * scale;
    let mut quads = Vec::new();

    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [18, 8, 8, 160]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [36, 14, 14, 230],
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.58,
                w * 0.36,
                52.0 * scale,
                [220, 70, 70, 255],
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
            if game.dodge_t > 0.0 {
                quads.push(Quad::new(
                    pad,
                    h - pad - 10.0 * scale,
                    80.0 * scale * (game.dodge_t / DODGE_TIME),
                    8.0 * scale,
                    [80, 180, 220, 220],
                ));
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
                let bar = (game.kills.max(1) as f32 / 4.0).clamp(0.15, 1.0);
                quads.push(Quad::new(
                    w * 0.22,
                    h * 0.40,
                    w * 0.56 * bar,
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
        clear: [96, 78, 70, 255],
        quads,
        ..Default::default()
    }
}

/// Camera still looks at the body after death (dump target == player xz).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(ARENA).unwrap();
        play.start();
        play
    }

    fn put_player(play: &mut WorldPlay, x: f32, z: f32, yaw: f32) {
        let y = play.doc.height_at(x, z) + BODY_H;
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [x, y, z];
            p.yaw = yaw;
            p.face = yaw;
            p.on_ground = true;
            p.name = "player".into();
        }
        let w = play.doc.player.clone().unwrap();
        write_player(&mut play.doc, w);
    }

    #[test]
    fn dump_is_action_not_collectathon() {
        let doc = WorldDoc::from_json(ARENA).unwrap();
        assert!(is_action(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_action(&crest));
        let foes: Vec<_> = doc
            .props
            .iter()
            .filter(|p| is_foe(p) && p.enabled)
            .collect();
        assert!(
            foes.len() >= 2,
            "need capsule + box foe, got {}",
            foes.len()
        );
        assert!(foes.iter().any(|p| p.model == "capsule"));
        assert!(foes.iter().any(|p| is_box_foe(p)));
        assert_eq!(doc.player.as_ref().unwrap().name, "player");
        assert!(doc.player.as_ref().unwrap().on_ground);
        let json = doc.to_json().unwrap();
        assert!(json.contains("foe"));
        assert!(json.contains("walker:player"));
    }

    #[test]
    fn title_blocks_combat_until_confirm() {
        let mut play = WorldPlay::from_json(ARENA).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_action());
        let z = play.doc.player.as_ref().unwrap().position[2];
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: true,
            dodge: false,
        };
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert_eq!(play.doc.player.as_ref().unwrap().position[2], z);
        assert_eq!(play.action.hits, 0);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.action.hp, PLAYER_HP);
    }

    #[test]
    fn attack_hits_capsule_foe_and_flash_is_in_dump() {
        let mut play = play_started();
        // Face +Z, stand in reach of the capsule foe at (0, ~2.2).
        put_player(&mut play, 0.0, 1.0, 0.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.action.hits >= 1, "hits {}", play.action.hits);
        assert!(play.action.flash_t > 0.0);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "hurt");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("hurt"), "hit must be dump-visible");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 3, "HP pips + flash overlay");
    }

    #[test]
    fn two_hits_kill_foe_and_dump_disables_it() {
        let mut play = play_started();
        let foe_id = play
            .doc
            .props
            .iter()
            .find(|p| is_foe(p) && p.model == "capsule" && p.enabled)
            .unwrap()
            .id
            .clone();
        put_player(&mut play, 0.0, 1.0, 0.0);
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
        assert!(play.action.kills >= 1);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains(&foe_id));
    }

    #[test]
    fn dodge_avoids_contact_damage() {
        let mut play = play_started();
        let foe = play
            .doc
            .props
            .iter()
            .find(|p| is_foe(p) && p.enabled)
            .unwrap()
            .clone();
        put_player(&mut play, foe.position[0], foe.position[2], 0.0);
        play.input.dodge = true;
        play.tick(1.0 / 60.0);
        assert!(play.action.iframe_t > 0.0 || play.action.dodge_t > 0.0);
        let hp = play.action.hp;
        play.input.dodge = false;
        play.tick(1.0 / 60.0);
        assert_eq!(play.action.hp, hp, "i-frames must ignore contact");
        assert!(!play.action.dead);
    }

    #[test]
    fn contact_kills_then_retry_restores_dump() {
        let mut play = play_started();
        let foe = play
            .doc
            .props
            .iter()
            .find(|p| is_foe(p) && p.enabled)
            .unwrap()
            .clone();
        put_player(&mut play, foe.position[0], foe.position[2], 0.0);
        let mut n = 0;
        while play.game.phase == GamePhase::Playing && n < 400 {
            play.input = WalkInput::default();
            play.tick(1.0 / 60.0);
            n += 1;
        }
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert!(play.action.dead, "player must die from contact");
        assert_eq!(play.doc.player.as_ref().unwrap().name, "dead");
        assert!(
            camera_tracks_body(&play.doc),
            "camera must keep the body, cam={:?} player={:?}",
            play.doc.cameras.first().map(|c| c.target),
            play.doc.player.as_ref().map(|p| p.position)
        );
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "death overlay");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("dead"));

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.action.dead);
        assert_eq!(play.action.hp, PLAYER_HP);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "player");
        let live = play
            .doc
            .props
            .iter()
            .filter(|p| is_foe(p) && p.enabled)
            .count();
        assert!(live >= 2, "retry must restore foes, live={live}");
        assert!(!play.doc.player.as_ref().unwrap().name.contains("dead"));
    }

    #[test]
    fn crest_collectathon_still_starts_on_title() {
        let play = WorldPlay::from_json(CREST).unwrap();
        assert!(play.is_collectathon());
        assert!(!play.is_action());
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.doc.props.iter().any(|p| p.name == "star"));
    }
}
