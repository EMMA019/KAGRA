//! Fighting round on play_world: attack, hitstun, guard, 2-hit combo, KO / retry.
//!
//! Sibling of collectathon / action / race. Two capsules on a World.dump ring
//! (player walker + opponent prop). J/click attack, Shift/C/K guard, facing,
//! stun frames. Incoming hit while guarding is blocked (no damage / no KO).
//! Two attacks in a combo window register as a combo when the first landed.
//! KO/retry is dump-visible (`name` stun/hurt/block/combo/ko/win + opponent
//! enable). Title -> play -> result reuses `WorldPlay` / `GamePhase`. Dual-body
//! camera keeps both in view. Hit flash overlay on shared wgpu 30. Capsules,
//! not VRM, not Rapier. No combo editor, specials, net, or new ECS.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};
use glam::Vec3;

pub const GAME_ID: &str = "fight_hitstun";
pub const PLAYER_HP: u32 = 3;
pub const OPP_HP: u32 = 3;
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const OPP_R: f32 = 0.50;
pub const MOVE_SPEED: f32 = 4.4;
pub const ATTACK_REACH: f32 = 1.55;
pub const ATTACK_R: f32 = 0.62;
pub const ATTACK_TIME: f32 = 0.22;
pub const STUN_TIME: f32 = 0.40;
pub const HIT_FLASH: f32 = 0.22;
pub const COMBO_TIME: f32 = 0.55;
pub const BLOCK_TIME: f32 = 0.30;
pub const OPP_SPEED: f32 = 2.6;
pub const OPP_ATTACK_CD: f32 = 0.90;
pub const RING_Z: f32 = 1.6;

const OPP_COLOR: [u32; 3] = [200, 64, 72];
const OPP_STUN_COLOR: [u32; 3] = [255, 220, 80];
const OPP_KO_COLOR: [u32; 3] = [48, 14, 16];

/// Live round around a dump. HP / stun stay here; names + opponent enable in
/// the dump are the query/dump source of truth on hit / KO / retry.
#[derive(Clone, Debug)]
pub struct FightGame {
    pub hp: u32,
    pub opp_hp: u32,
    pub hits: u32,
    pub px: f32,
    pub pz: f32,
    pub pyaw: f32,
    pub ox: f32,
    pub oz: f32,
    pub oyaw: f32,
    pub attack_t: f32,
    pub stun_t: f32,
    pub opp_attack_t: f32,
    pub opp_stun_t: f32,
    pub opp_cd: f32,
    pub flash_t: f32,
    pub hurt_flash: bool,
    pub combo: u32,
    pub guarding: bool,
    pub ko: bool,
    pub won: bool,
    pub done: bool,
    combo_t: f32,
    block_t: f32,
    swing_hit: bool,
    opp_swing_hit: bool,
}

impl Default for FightGame {
    fn default() -> Self {
        Self {
            hp: PLAYER_HP,
            opp_hp: OPP_HP,
            hits: 0,
            px: 0.0,
            pz: 0.0,
            pyaw: std::f32::consts::FRAC_PI_2,
            ox: 0.0,
            oz: 0.0,
            oyaw: -std::f32::consts::FRAC_PI_2,
            attack_t: 0.0,
            stun_t: 0.0,
            opp_attack_t: 0.0,
            opp_stun_t: 0.0,
            opp_cd: 0.0,
            flash_t: 0.0,
            hurt_flash: false,
            combo: 0,
            guarding: false,
            ko: false,
            won: false,
            done: false,
            combo_t: 0.0,
            block_t: 0.0,
            swing_hit: false,
            opp_swing_hit: false,
        }
    }
}

impl FightGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let mut g = Self::default();
        g.rebind(doc);
        g
    }

    fn rebind(&mut self, doc: &WorldDoc) {
        *self = Self::default();
        if let Some(w) = player_ref(doc) {
            self.px = w.position[0];
            self.pz = w.position[2];
            self.pyaw = w.yaw;
        }
        if let Some(p) = doc.props.iter().find(|p| is_opponent(p)) {
            self.ox = p.position[0];
            self.oz = p.position[2];
            self.oyaw = p.yaw;
        }
    }
}

pub fn is_fight(doc: &WorldDoc) -> bool {
    doc.props.iter().any(is_opponent)
}

fn is_opponent(p: &WorldProp) -> bool {
    p.name == "opponent"
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

fn heading(yaw: f32) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    Vec3::new(s, 0.0, c)
}

fn face_toward(from_x: f32, from_z: f32, to_x: f32, to_z: f32) -> f32 {
    (to_x - from_x).atan2(to_z - from_z)
}

/// Sit capsules on the floor. Does not spawn extra opponents.
pub fn seed(doc: &mut WorldDoc) {
    if !is_fight(doc) {
        return;
    }
    let mut ys = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        if !is_opponent(p) || !p.enabled {
            continue;
        }
        let extra = BODY_H * p.scale[1].abs().max(0.6);
        let y = doc.height_at(p.position[0], p.position[2]) + extra;
        ys.push((i, y));
    }
    for (i, y) in ys {
        if let Some(p) = doc.props.get_mut(i) {
            p.position[1] = y;
            if p.color.is_none() {
                p.color = Some(OPP_COLOR);
            }
        }
    }
    if let Some(p) = player_ref(doc) {
        let mut w = p.clone();
        w.position[1] = doc.height_at(w.position[0], w.position[2]) + BODY_H;
        w.on_ground = true;
        write_player(doc, w);
    }
    doc.coins = 0;
    place_dual_camera(doc);
}

/// Side camera that keeps both capsules in frame.
pub fn place_dual_camera(doc: &mut WorldDoc) {
    let (px, pz) = player_ref(doc)
        .map(|w| (w.position[0], w.position[2]))
        .unwrap_or((0.0, 0.0));
    let (ox, oz) = doc
        .props
        .iter()
        .find(|p| is_opponent(p))
        .map(|p| (p.position[0], p.position[2]))
        .unwrap_or((px + 3.0, pz));
    let mx = (px + ox) * 0.5;
    let mz = (pz + oz) * 0.5;
    let sep = ((px - ox).hypot(pz - oz)).max(1.0);
    let back = (sep * 1.2 + 7.5).clamp(8.0, 18.0);
    let height = 3.6 + sep * 0.18;
    let eye = [mx, height, mz + back];
    let target = [mx, 1.15, mz];
    let fov = doc.cameras.first().map(|c| c.fov).unwrap_or(48.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye;
        cam.target = target;
        cam.name = "dual".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "dual".into(),
            position: eye,
            target,
            fov,
        });
    }
}

/// Round tick. Caller may have walked; this pose / camera wins.
pub fn tick(doc: &mut WorldDoc, game: &mut FightGame, input: WalkInput, dt: f32) {
    if game.done {
        write_pose(doc, game);
        place_dual_camera(doc);
        return;
    }
    let input = input.clamped();
    tick_timers(game, dt);
    game.guarding = input.dodge && game.stun_t <= 0.0;
    face_each_other(game);
    drive_player(doc, game, input, dt);
    player_attack(doc, game, input);
    drive_opponent(doc, game, dt);
    opponent_attack(game);
    separate(game);
    sync_opponent(doc, game);
    write_pose(doc, game);
    check_ko(doc, game);
    place_dual_camera(doc);
}

fn tick_timers(game: &mut FightGame, dt: f32) {
    game.attack_t = (game.attack_t - dt).max(0.0);
    game.stun_t = (game.stun_t - dt).max(0.0);
    game.opp_attack_t = (game.opp_attack_t - dt).max(0.0);
    game.opp_stun_t = (game.opp_stun_t - dt).max(0.0);
    game.opp_cd = (game.opp_cd - dt).max(0.0);
    game.flash_t = (game.flash_t - dt).max(0.0);
    game.combo_t = (game.combo_t - dt).max(0.0);
    game.block_t = (game.block_t - dt).max(0.0);
    if game.combo_t <= 0.0 {
        game.combo = 0;
    }
    if game.attack_t <= 0.0 {
        game.swing_hit = false;
    }
    if game.opp_attack_t <= 0.0 {
        game.opp_swing_hit = false;
    }
}

fn face_each_other(game: &mut FightGame) {
    game.pyaw = face_toward(game.px, game.pz, game.ox, game.oz);
    game.oyaw = face_toward(game.ox, game.oz, game.px, game.pz);
}

fn drive_player(doc: &WorldDoc, game: &mut FightGame, input: WalkInput, dt: f32) {
    if game.stun_t > 0.0 || game.attack_t > 0.0 || game.done {
        return;
    }
    let half = doc.half.max(4.0);
    let pad = 2.0;
    game.px = (game.px + input.lx * MOVE_SPEED * dt).clamp(-half + pad, half - pad);
    game.pz = (game.pz + input.lz * MOVE_SPEED * dt).clamp(-RING_Z, RING_Z);
}

fn player_attack(doc: &mut WorldDoc, game: &mut FightGame, input: WalkInput) {
    if game.done || game.stun_t > 0.0 {
        return;
    }
    if input.attack && game.attack_t <= 0.0 && !game.guarding {
        game.attack_t = ATTACK_TIME;
        game.swing_hit = false;
    }
    if game.attack_t <= 0.0 || game.swing_hit {
        return;
    }
    if !hit_test(game.px, game.pz, game.pyaw, game.ox, game.oz, OPP_R) {
        return;
    }
    game.swing_hit = true;
    game.opp_hp = game.opp_hp.saturating_sub(1);
    game.hits = game.hits.saturating_add(1);
    if game.combo_t > 0.0 {
        game.combo = game.combo.saturating_add(1);
    } else {
        game.combo = 1;
    }
    game.combo_t = COMBO_TIME;
    game.opp_stun_t = STUN_TIME;
    game.opp_attack_t = 0.0;
    game.flash_t = HIT_FLASH;
    game.hurt_flash = false;
    doc.coins = game.hits;
    if let Some(p) = doc.props.iter_mut().find(|p| is_opponent(p)) {
        p.color = Some(OPP_STUN_COLOR);
    }
}

fn drive_opponent(doc: &WorldDoc, game: &mut FightGame, dt: f32) {
    if game.done || game.opp_stun_t > 0.0 || game.opp_attack_t > 0.0 {
        return;
    }
    let dx = game.px - game.ox;
    let dz = game.pz - game.oz;
    let dist = dx.hypot(dz);
    let stop = PLAYER_R + OPP_R + 0.12;
    if dist <= stop {
        return;
    }
    let nx = dx / dist.max(1e-4);
    let nz = dz / dist.max(1e-4);
    let half = doc.half.max(4.0);
    let pad = 2.0;
    game.ox = (game.ox + nx * OPP_SPEED * dt).clamp(-half + pad, half - pad);
    game.oz = (game.oz + nz * OPP_SPEED * dt).clamp(-RING_Z, RING_Z);
}

fn opponent_attack(game: &mut FightGame) {
    if game.done || game.opp_stun_t > 0.0 {
        return;
    }
    let dist = (game.px - game.ox).hypot(game.pz - game.oz);
    let reach = ATTACK_REACH + PLAYER_R;
    if game.opp_attack_t <= 0.0 && game.opp_cd <= 0.0 && dist <= reach {
        game.opp_attack_t = ATTACK_TIME;
        game.opp_cd = OPP_ATTACK_CD;
        game.opp_swing_hit = false;
    }
    if game.opp_attack_t <= 0.0 || game.opp_swing_hit {
        return;
    }
    if !hit_test(game.ox, game.oz, game.oyaw, game.px, game.pz, PLAYER_R) {
        return;
    }
    game.opp_swing_hit = true;
    if game.guarding {
        game.block_t = BLOCK_TIME;
        game.flash_t = HIT_FLASH;
        game.hurt_flash = false;
        return;
    }
    game.hp = game.hp.saturating_sub(1);
    game.stun_t = STUN_TIME;
    game.attack_t = 0.0;
    game.flash_t = HIT_FLASH;
    game.hurt_flash = true;
    game.combo = 0;
    game.combo_t = 0.0;
}

fn hit_test(ax: f32, az: f32, yaw: f32, tx: f32, tz: f32, target_r: f32) -> bool {
    let fwd = heading(yaw);
    let hx = ax + fwd.x * ATTACK_REACH * 0.62;
    let hz = az + fwd.z * ATTACK_REACH * 0.62;
    let r = target_r + ATTACK_R;
    let dx = hx - tx;
    let dz = hz - tz;
    dx * dx + dz * dz <= r * r
}

fn separate(game: &mut FightGame) {
    let dx = game.px - game.ox;
    let dz = game.pz - game.oz;
    let dist = dx.hypot(dz);
    let min = PLAYER_R + OPP_R + 0.04;
    if dist >= min || dist < 1e-5 {
        return;
    }
    let nx = dx / dist;
    let nz = dz / dist;
    let push = (min - dist) * 0.5;
    game.px += nx * push;
    game.pz += nz * push;
    game.ox -= nx * push;
    game.oz -= nz * push;
}

fn sync_opponent(doc: &mut WorldDoc, game: &FightGame) {
    let y = doc.height_at(game.ox, game.oz) + BODY_H;
    if let Some(p) = doc.props.iter_mut().find(|p| is_opponent(p)) {
        p.position = [game.ox, y, game.oz];
        p.yaw = game.oyaw;
        if game.done && game.won {
            p.enabled = false;
            p.color = Some(OPP_KO_COLOR);
        } else if game.opp_stun_t > 0.0 {
            p.enabled = true;
            p.color = Some(OPP_STUN_COLOR);
        } else {
            p.enabled = true;
            p.color = Some(OPP_COLOR);
        }
    }
}

fn write_pose(doc: &mut WorldDoc, game: &FightGame) {
    let Some(mut w) = player_ref(doc).cloned() else {
        return;
    };
    let y = doc.height_at(game.px, game.pz) + BODY_H;
    w.position = [game.px, y, game.pz];
    w.yaw = game.pyaw;
    w.face = game.pyaw;
    w.on_ground = true;
    if game.done && game.ko {
        w.name = "ko".into();
    } else if game.done && game.won {
        w.name = "win".into();
    } else if game.stun_t > 0.0 {
        w.name = "stun".into();
    } else if game.block_t > 0.0 || game.guarding {
        w.name = "block".into();
    } else if game.combo >= 2 {
        w.name = "combo".into();
    } else if game.flash_t > 0.0 && !game.hurt_flash {
        w.name = "hurt".into();
    } else {
        w.name = "player".into();
    }
    write_player(doc, w);
    doc.coins = game.hits;
}

fn check_ko(doc: &mut WorldDoc, game: &mut FightGame) {
    if game.done {
        return;
    }
    if game.hp == 0 {
        game.ko = true;
        game.done = true;
        game.won = false;
        if let Some(mut w) = player_ref(doc).cloned() {
            w.name = "ko".into();
            write_player(doc, w);
        }
        return;
    }
    if game.opp_hp == 0 {
        game.won = true;
        game.done = true;
        game.ko = false;
        if let Some(p) = doc.props.iter_mut().find(|p| is_opponent(p)) {
            p.enabled = false;
            p.color = Some(OPP_KO_COLOR);
        }
        if let Some(mut w) = player_ref(doc).cloned() {
            w.name = "win".into();
            write_player(doc, w);
        }
    }
}

pub fn build_hud(game: &FightGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let pad = 16.0 * scale;
    let mut quads = Vec::new();
    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [12, 10, 14, 150]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [28, 16, 20, 230],
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
            let pip = 22.0 * scale;
            let gap = 6.0 * scale;
            for i in 0..PLAYER_HP {
                let got = i < game.hp;
                quads.push(Quad::new(
                    pad + i as f32 * (pip + gap),
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
            for i in 0..OPP_HP {
                let got = i < game.opp_hp;
                quads.push(Quad::new(
                    w - pad - pip - i as f32 * (pip + gap),
                    pad,
                    pip,
                    pip,
                    if got {
                        [72, 140, 220, 255]
                    } else {
                        [16, 20, 28, 160]
                    },
                ));
            }
            if game.flash_t > 0.0 {
                let a = (90.0 + 140.0 * (game.flash_t / HIT_FLASH)) as u8;
                let col = if game.block_t > 0.0 {
                    [80, 180, 255, a.min(140)]
                } else if game.hurt_flash {
                    [210, 30, 30, a]
                } else {
                    [255, 220, 80, a.min(120)]
                };
                quads.push(Quad::new(0.0, 0.0, w, h, col));
            }
            if game.stun_t > 0.0 {
                quads.push(Quad::new(
                    pad,
                    h - pad - 10.0 * scale,
                    100.0 * scale * (game.stun_t / STUN_TIME),
                    8.0 * scale,
                    [240, 180, 70, 220],
                ));
            }
        }
        GamePhase::Complete => {
            if game.ko {
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
                let bar = (game.hits.max(1) as f32 / OPP_HP.max(1) as f32).clamp(0.15, 1.0);
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
                if game.ko {
                    [200, 80, 70, 240]
                } else {
                    [70, 160, 110, 240]
                },
            ));
        }
    }
    DrawList {
        clear: [78, 52, 54, 255],
        quads,
        ..Default::default()
    }
}

/// Midpoint target; both bodies sit near it so the dual cam keeps them.
pub fn camera_keeps_both(doc: &WorldDoc) -> bool {
    let Some(w) = player_ref(doc) else {
        return false;
    };
    let Some(opp) = doc.props.iter().find(|p| is_opponent(p)) else {
        return false;
    };
    let Some(cam) = doc.cameras.first() else {
        return false;
    };
    let mx = (w.position[0] + opp.position[0]) * 0.5;
    let mz = (w.position[2] + opp.position[2]) * 0.5;
    let td = (cam.target[0] - mx).hypot(cam.target[2] - mz);
    let pd = (cam.target[0] - w.position[0]).hypot(cam.target[2] - w.position[2]);
    let od = (cam.target[0] - opp.position[0]).hypot(cam.target[2] - opp.position[2]);
    td < 1.5 && pd < 8.0 && od < 8.0 && cam.name == "dual"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const RING: &str = include_str!("../tests/fixtures/fight_hitstun_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");
    const TRACK: &str = include_str!("../tests/fixtures/race_drive_world.json");
    const RANGE: &str = include_str!("../tests/fixtures/fps_range_world.json");
    const LANE: &str = include_str!("../tests/fixtures/td_lane_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(RING).unwrap();
        play.confirm();
        play
    }

    fn put_bodies(play: &mut WorldPlay, px: f32, pz: f32, ox: f32, oz: f32) {
        play.fight.px = px;
        play.fight.pz = pz;
        play.fight.ox = ox;
        play.fight.oz = oz;
        play.fight.pyaw = face_toward(px, pz, ox, oz);
        play.fight.oyaw = face_toward(ox, oz, px, pz);
        write_pose(&mut play.doc, &play.fight);
        sync_opponent(&mut play.doc, &play.fight);
        place_dual_camera(&mut play.doc);
    }

    #[test]
    fn dump_is_fight_not_action_or_race() {
        let doc = WorldDoc::from_json(RING).unwrap();
        assert!(is_fight(&doc));
        assert_eq!(GAME_ID, "fight_hitstun");
        assert!(!crate::action::is_action(&doc));
        assert!(!crate::race::is_race(&doc));
        assert!(!crate::fps::is_fps(&doc));
        assert!(!crate::td::is_td(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_fight(&crest));
        let arena = WorldDoc::from_json(ARENA).unwrap();
        assert!(!is_fight(&arena));
        let opp: Vec<_> = doc
            .props
            .iter()
            .filter(|p| is_opponent(p) && p.enabled)
            .collect();
        assert_eq!(opp.len(), 1);
        assert_eq!(opp[0].model, "capsule");
        assert_eq!(doc.player.as_ref().unwrap().name, "player");
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("opponent"));
        assert!(json.contains("capsule"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 3,
            "ring + two capsules must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(RING).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_fight());
        assert!(!play.is_action());
        assert!(!play.is_race());
        assert!(!play.is_collectathon());
        let start = play.doc.player.as_ref().unwrap().position;
        play.input = WalkInput {
            lx: 1.0,
            lz: 0.0,
            jump: false,
            attack: true,
            dodge: false,
        };
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        let now = play.doc.player.as_ref().unwrap().position;
        assert_eq!(now, start, "title must not fight");
        assert_eq!(play.fight.hits, 0);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "dual");
    }

    #[test]
    fn attack_hits_and_stun_is_dump_visible() {
        let mut play = play_started();
        put_bodies(&mut play, -0.8, 0.0, 0.8, 0.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.fight.hits >= 1, "hits {}", play.fight.hits);
        assert!(play.fight.opp_stun_t > 0.0, "opponent must enter hitstun");
        assert!(play.fight.flash_t > 0.0);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "hurt");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("hurt"), "hit must be dump-visible");
        let opp = play.doc.props.iter().find(|p| is_opponent(p)).unwrap();
        assert_eq!(opp.color, Some(OPP_STUN_COLOR));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 4, "HP pips + flash overlay");
        assert!(camera_keeps_both(&play.doc));
    }

    #[test]
    fn hits_ko_opponent_then_retry_restores() {
        let mut play = play_started();
        put_bodies(&mut play, -0.8, 0.0, 0.8, 0.0);
        for _ in 0..OPP_HP {
            play.input.attack = true;
            play.tick(1.0 / 60.0);
            // Recover swing only; stay inside opponent hitstun so they cannot swing back.
            for _ in 0..18 {
                play.input.attack = false;
                play.tick(1.0 / 60.0);
            }
        }
        assert!(play.fight.won, "opponent must KO");
        assert!(play.fight.done);
        assert_eq!(play.game.phase, GamePhase::Complete);
        let opp = play.doc.props.iter().find(|p| is_opponent(p)).unwrap();
        assert!(!opp.enabled, "KO opponent must be disabled in the dump");
        assert_eq!(play.doc.player.as_ref().unwrap().name, "win");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("win"));
        assert!(dump.contains("opponent"));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "result overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.fight.done);
        assert_eq!(play.fight.hp, PLAYER_HP);
        assert_eq!(play.fight.opp_hp, OPP_HP);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "player");
        let live = play
            .doc
            .props
            .iter()
            .filter(|p| is_opponent(p) && p.enabled)
            .count();
        assert_eq!(live, 1, "retry must restore opponent");
    }

    #[test]
    fn opponent_attack_stuns_then_ko_is_dump_visible() {
        let mut play = play_started();
        put_bodies(&mut play, -0.7, 0.0, 0.7, 0.0);
        let mut n = 0;
        while play.game.phase == GamePhase::Playing && n < 600 {
            play.input = WalkInput::default();
            play.tick(1.0 / 60.0);
            n += 1;
        }
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert!(play.fight.ko, "player must KO from opponent swings");
        assert_eq!(play.doc.player.as_ref().unwrap().name, "ko");
        assert!(
            camera_keeps_both(&play.doc),
            "camera must keep both, cam={:?} player={:?} opp={:?}",
            play.doc
                .cameras
                .first()
                .map(|c| (c.target, c.name.as_str())),
            play.doc.player.as_ref().map(|p| p.position),
            play.doc
                .props
                .iter()
                .find(|p| is_opponent(p))
                .map(|p| p.position)
        );
        let dump = play.doc.to_json().unwrap();
        assert!(
            dump.contains("\"ko\"") || dump.contains("ko"),
            "KO must be dump-visible"
        );
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "KO overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.fight.ko);
        assert_eq!(play.fight.hp, PLAYER_HP);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "player");
    }

    #[test]
    fn stun_locks_movement() {
        let mut play = play_started();
        put_bodies(&mut play, -0.8, 0.0, 0.8, 0.0);
        play.fight.stun_t = STUN_TIME;
        let x0 = play.fight.px;
        play.input = WalkInput {
            lx: 1.0,
            lz: 0.0,
            jump: false,
            attack: true,
            dodge: false,
        };
        play.tick(1.0 / 60.0);
        assert!(
            (play.fight.px - x0).abs() < 0.02,
            "stun frames must lock walk, x0={x0} now={}",
            play.fight.px
        );
        assert_eq!(play.fight.hits, 0, "stun frames must lock attack");
        assert_eq!(play.doc.player.as_ref().unwrap().name, "stun");
    }

    #[test]
    fn dual_camera_tracks_both_after_walk() {
        let mut play = play_started();
        let cam0 = play.doc.cameras[0].position;
        play.input = WalkInput {
            lx: -1.0,
            lz: 0.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..40 {
            play.tick(1.0 / 60.0);
        }
        let cam = play.doc.cameras[0].position;
        let d = (cam[0] - cam0[0]).abs() + (cam[2] - cam0[2]).abs();
        assert!(d > 0.15, "dual cam should follow the pair, d={d}");
        assert_eq!(play.doc.cameras[0].name, "dual");
        assert!(camera_keeps_both(&play.doc));
    }

    #[test]
    fn guard_blocks_incoming_hit_dump_visible() {
        let mut play = play_started();
        put_bodies(&mut play, -0.7, 0.0, 0.7, 0.0);
        play.input.dodge = true;
        play.tick(1.0 / 60.0);
        assert!(play.fight.guarding);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "block");
        let hp0 = play.fight.hp;
        let mut blocked = play.fight.block_t > 0.0;
        let mut n = 0;
        while !blocked && n < 90 {
            play.input.dodge = true;
            play.input.attack = false;
            play.tick(1.0 / 60.0);
            blocked = play.fight.block_t > 0.0;
            n += 1;
        }
        assert!(blocked, "held guard must register a blocked hit, n={n}");
        assert_eq!(play.fight.hp, hp0, "guard must prevent damage");
        assert!(!play.fight.ko);
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "block");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("block"), "block must be dump-visible");
    }

    #[test]
    fn two_hit_combo_is_dump_visible() {
        let mut play = play_started();
        put_bodies(&mut play, -0.8, 0.0, 0.8, 0.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.fight.hits >= 1, "first hit must land");
        assert_eq!(play.fight.combo, 1);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "hurt");
        let recover = (ATTACK_TIME * 60.0).ceil() as u32 + 1;
        for _ in 0..recover {
            play.input.attack = false;
            play.tick(1.0 / 60.0);
        }
        assert_eq!(play.fight.combo, 1, "combo window must still be open");
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(
            play.fight.hits >= 2,
            "second hit must connect if first landed, hits={}",
            play.fight.hits
        );
        assert!(play.fight.combo >= 2, "two hits in window are a combo");
        assert!(!play.fight.won, "two hits must not KO yet");
        assert_eq!(play.doc.coins, play.fight.hits);
        assert_eq!(play.doc.player.as_ref().unwrap().name, "combo");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("combo"), "combo must be dump-visible");
    }

    #[test]
    fn crest_action_race_still_own_their_dumps() {
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_fight());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_fight());
        let race = WorldPlay::from_json(TRACK).unwrap();
        assert!(race.is_race());
        assert!(!race.is_fight());
        let fps = WorldPlay::from_json(RANGE).unwrap();
        assert!(fps.is_fps());
        assert!(!fps.is_fight());
        let td = WorldPlay::from_json(LANE).unwrap();
        assert!(td.is_td());
        assert!(!td.is_fight());
    }
}
