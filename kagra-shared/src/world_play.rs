//! Live `WorldDoc` tick: WASD / look → walker + chase camera + collectathon loop.
//!
//! Shared-side. Matches collectathon `WalkInput` (camera-relative wish, sit
//! on heightfield, optional jump). Title → play → result lives here so
//! `python -m kagra.play_world` is one complete loop. Python `Walk.wish` /
//! `CharacterController` is the leftover VRM motor — documented, not copied,
//! and not Rapier.

use crate::action::{self, ActionGame};
use crate::collectathon::{
    spawn_coins, spawn_stars, won, IsleGame, WalkInput, BODY_H, CAM_DISTANCE, CAM_HEIGHT,
    CAM_LOOK_Y, GRAVITY, JUMP_V, PICK_REACH, PLAYER_SPEED, STAR_XZ,
};
use crate::game::GamePhase;
use crate::platformer::{self, PlatformGame};
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};
use glam::Vec3;

/// Running play state around a dump document. `doc` is the JSON source of
/// truth after each tick (walker position/yaw + camera + live pickups).
#[derive(Clone, Debug)]
pub struct WorldPlay {
    pub doc: WorldDoc,
    pub input: WalkInput,
    pub look_yaw: f32,
    pub look_pitch: f32,
    pub game: IsleGame,
    pub action: ActionGame,
    pub platform: PlatformGame,
    seed: WorldDoc,
    vy: f32,
}

impl WorldPlay {
    pub fn new(doc: WorldDoc) -> Self {
        let mut doc = doc;
        seed_collectathon_pickups(&mut doc);
        action::seed(&mut doc);
        platformer::seed(&mut doc);
        refresh_coin_count(&mut doc);
        let look_yaw = look_yaw_from_doc(&doc);
        let action = ActionGame::from_doc(&doc);
        let platform = PlatformGame::from_doc(&doc);
        let game = if is_collectathon(&doc)
            || action::is_action(&doc)
            || platformer::is_platformer(&doc)
        {
            IsleGame::default()
        } else {
            let mut g = IsleGame::default();
            g.start();
            g
        };
        Self {
            seed: doc.clone(),
            doc,
            input: WalkInput::default(),
            look_yaw,
            look_pitch: 0.0,
            game,
            action,
            platform,
            vy: 0.0,
        }
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        Ok(Self::new(WorldDoc::from_json(json)?))
    }

    /// Title / result: Space or Enter. Playing ignores this (Space is jump).
    pub fn confirm(&mut self) {
        match self.game.phase {
            GamePhase::Title | GamePhase::Complete => self.start(),
            GamePhase::Playing => {}
        }
    }

    pub fn start(&mut self) {
        let best = self.game.best_score;
        self.doc = self.seed.clone();
        self.input = WalkInput::default();
        self.look_yaw = look_yaw_from_doc(&self.doc);
        self.look_pitch = 0.0;
        self.vy = 0.0;
        self.game = IsleGame::default();
        self.game.best_score = best;
        self.game.start();
        self.action = ActionGame::from_doc(&self.doc);
        let ckpt = self.platform.checkpoint;
        self.platform = PlatformGame::from_doc(&self.doc);
        if ckpt.is_some() {
            self.platform.checkpoint = ckpt;
            platformer::restore_checkpoint(&mut self.doc, &self.platform);
        }
        refresh_coin_count(&mut self.doc);
    }

    pub fn is_collectathon(&self) -> bool {
        is_collectathon(&self.doc) || is_collectathon(&self.seed)
    }

    pub fn is_action(&self) -> bool {
        action::is_action(&self.doc) || action::is_action(&self.seed)
    }

    pub fn is_platformer(&self) -> bool {
        platformer::is_platformer(&self.doc) || platformer::is_platformer(&self.seed)
    }

    /// Mouse / arrow look. Pitch is clamped.
    pub fn add_look(&mut self, dyaw: f32, dpitch: f32) {
        self.look_yaw += dyaw;
        self.look_pitch = (self.look_pitch + dpitch).clamp(-0.7, 0.55);
    }

    /// Advance walker + chase camera + pickups. `dt` is seconds (clamped).
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        if dt <= 0.0 {
            return;
        }
        if !self.game.is_playing() {
            return;
        }
        let input = self.input.clamped();
        self.step_walker(input, dt);
        self.follow_camera();
        if self.is_action() {
            action::tick(&mut self.doc, &mut self.action, input, self.look_yaw, dt);
            self.game.time_s += dt;
            self.input.jump = false;
            self.input.attack = false;
            self.input.dodge = false;
            if self.action.dead || self.action.won {
                self.game.phase = GamePhase::Complete;
                self.game.score = self.action.kills * 250 + self.action.hits * 20;
            }
            return;
        }
        if self.is_platformer() {
            platformer::tick(&mut self.doc, &mut self.platform, &mut self.vy, input, dt);
            self.follow_camera();
            self.game.time_s += dt;
            self.input.jump = false;
            if self.platform.dead || self.platform.won {
                self.game.phase = GamePhase::Complete;
                self.game.score = self.platform.landed * 50;
            }
            return;
        }
        self.collect_pickups();
        self.game.time_s += dt;
        self.input.jump = false;
        if won(self.game.stars) {
            self.game
                .finish(self.game.stars, self.game.coins, self.game.time_s);
        }
    }

    /// Font-free HUD: title band / star+coin pips / result band.
    pub fn build_hud(&self, width: u32, height: u32) -> DrawList {
        if self.is_action() {
            return action::build_hud(&self.action, self.game.phase, width, height);
        }
        if self.is_platformer() {
            return platformer::build_hud(&self.platform, self.game.phase, width, height);
        }
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
        let pad = 16.0 * scale;
        let mut quads = Vec::new();

        match self.game.phase {
            GamePhase::Title => {
                quads.push(Quad::new(0.0, 0.0, w, h, [10, 14, 12, 150]));
                quads.push(Quad::new(
                    w * 0.18,
                    h * 0.28,
                    w * 0.64,
                    h * 0.18,
                    [18, 24, 18, 230],
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
                for i in 0..STAR_XZ.len() {
                    let x = pad + i as f32 * (pip + gap);
                    let got = (i as u32) < self.game.stars;
                    quads.push(Quad::new(
                        x,
                        pad,
                        pip,
                        pip,
                        if got {
                            [240, 196, 72, 255]
                        } else {
                            [20, 24, 18, 150]
                        },
                    ));
                }
                let coin_w = 8.0 * scale;
                for i in 0..self.game.coins.min(24) {
                    quads.push(Quad::new(
                        pad + i as f32 * (coin_w + 3.0 * scale),
                        pad + pip + 8.0 * scale,
                        coin_w,
                        coin_w,
                        [255, 210, 70, 255],
                    ));
                }
            }
            GamePhase::Complete => {
                quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 16, 12, 210]));
                let bar = (self.game.score.min(2400) as f32 / 2400.0).clamp(0.08, 1.0);
                quads.push(Quad::new(
                    w * 0.22,
                    h * 0.40,
                    w * 0.56 * bar,
                    18.0 * scale,
                    [240, 196, 72, 255],
                ));
                quads.push(Quad::new(
                    w * 0.32,
                    h * 0.62,
                    w * 0.36,
                    48.0 * scale,
                    [70, 160, 110, 240],
                ));
            }
        }

        DrawList {
            clear: [130, 165, 205, 255],
            quads,
        }
    }

    fn step_walker(&mut self, input: WalkInput, dt: f32) {
        let half = self.doc.half.max(4.0);
        let (s, c) = self.look_yaw.sin_cos();
        let fwd = Vec3::new(s, 0.0, c);
        let right = Vec3::new(c, 0.0, -s);
        let wish = right * input.lx + fwd * input.lz;
        let wish_len = wish.length();

        let (id, mut x, mut y, mut z, mut yaw, mut on_ground) = {
            let Some(w) = player_ref(&self.doc) else {
                return;
            };
            (
                w.id.clone(),
                w.position[0],
                w.position[1],
                w.position[2],
                w.yaw,
                w.on_ground,
            )
        };

        if wish_len > 0.08 {
            let dir = wish / wish_len;
            let speed = PLAYER_SPEED * wish_len.min(1.0);
            x += dir.x * speed * dt;
            z += dir.z * speed * dt;
            yaw = dir.x.atan2(dir.z);
        }
        let pad = 2.0;
        x = x.clamp(-half + pad, half - pad);
        z = z.clamp(-half + pad, half - pad);

        if input.jump && on_ground {
            self.vy = JUMP_V;
        }
        self.vy -= GRAVITY * dt;
        y += self.vy * dt;
        let ground = self.doc.height_at(x, z) + BODY_H;
        if y <= ground {
            y = ground;
            self.vy = 0.0;
            on_ground = true;
        } else {
            on_ground = false;
        }

        let updated = WorldWalker {
            id: id.clone(),
            kind: "walker".into(),
            name: "player".into(),
            position: [x, y, z],
            yaw,
            face: yaw,
            on_ground,
        };
        write_player(&mut self.doc, updated);
    }

    fn follow_camera(&mut self) {
        let Some(w) = player_ref(&self.doc) else {
            return;
        };
        let look = Vec3::new(w.position[0], w.position[1] + CAM_LOOK_Y, w.position[2]);
        let (s, c) = self.look_yaw.sin_cos();
        let pitch = self.look_pitch;
        let dist = CAM_DISTANCE;
        let height = CAM_HEIGHT - CAM_LOOK_Y + pitch * 4.0;
        let eye = look + Vec3::new(-s * dist, height, -c * dist);
        let fov = self.doc.cameras.first().map(|c| c.fov).unwrap_or(54.0);
        if let Some(cam) = self.doc.cameras.first_mut() {
            cam.position = eye.to_array();
            cam.target = look.to_array();
        } else {
            self.doc.cameras.push(crate::world_doc::WorldCamera {
                id: "camera:main".into(),
                kind: "camera".into(),
                name: "main".into(),
                position: eye.to_array(),
                target: look.to_array(),
                fov,
            });
        }
    }

    fn collect_pickups(&mut self) {
        let Some(w) = player_ref(&self.doc) else {
            return;
        };
        let px = w.position[0];
        let pz = w.position[2];
        for prop in &mut self.doc.props {
            if !prop.enabled {
                continue;
            }
            if prop.name != "coin" && prop.name != "star" {
                continue;
            }
            let dx = px - prop.position[0];
            let dz = pz - prop.position[2];
            if (dx * dx + dz * dz).sqrt() <= PICK_REACH {
                prop.enabled = false;
            }
        }
        self.game.stars = self
            .doc
            .props
            .iter()
            .filter(|p| p.name == "star" && !p.enabled)
            .count() as u32;
        self.game.coins = self
            .doc
            .props
            .iter()
            .filter(|p| p.name == "coin" && !p.enabled)
            .count() as u32;
        refresh_coin_count(&mut self.doc);
    }
}

fn is_collectathon(doc: &WorldDoc) -> bool {
    doc.heightfield.as_ref().and_then(|h| h.fn_name.as_deref()) == Some("open_world_height")
}

fn seed_collectathon_pickups(doc: &mut WorldDoc) {
    if !is_collectathon(doc) {
        return;
    }
    let stars = doc
        .props
        .iter()
        .filter(|p| p.name == "star" && p.enabled)
        .count();
    if stars >= STAR_XZ.len() {
        sit_pickups(doc);
        return;
    }
    doc.props.retain(|p| p.name != "star" && p.name != "coin");
    for (i, p) in spawn_stars().into_iter().enumerate() {
        let y = doc.height_at(p.x, p.z) + 1.55;
        let color = if i + 1 == STAR_XZ.len() {
            [255, 214, 70]
        } else if i % 2 == 0 {
            [220, 70, 70]
        } else {
            [70, 170, 90]
        };
        doc.props.push(WorldProp {
            id: format!("prop:star-{i}"),
            kind: "prop".into(),
            name: "star".into(),
            position: [p.x, y, p.z],
            model: "box".into(),
            scale: [0.55, 0.85, 0.12],
            enabled: true,
            color: Some(color),
            ..Default::default()
        });
    }
    for (i, p) in spawn_coins().into_iter().enumerate() {
        let y = doc.height_at(p.x, p.z) + 0.55;
        doc.props.push(WorldProp {
            id: format!("prop:coin-{i}"),
            kind: "prop".into(),
            name: "coin".into(),
            position: [p.x, y, p.z],
            yaw: i as f32 * 0.35,
            model: "sphere".into(),
            scale: [0.42, 0.08, 0.42],
            enabled: true,
            color: Some([255, 208, 64]),
            metallic: 1.0,
            roughness: 0.12,
            ..Default::default()
        });
    }
}

fn sit_pickups(doc: &mut WorldDoc) {
    let mut updates: Vec<(usize, f32, bool)> = Vec::new();
    for (i, prop) in doc.props.iter().enumerate() {
        if !prop.enabled {
            continue;
        }
        if prop.name != "star" && prop.name != "coin" {
            continue;
        }
        let extra = if prop.name == "star" { 1.55 } else { 0.55 };
        let y = doc.height_at(prop.position[0], prop.position[2]) + extra;
        updates.push((i, y, prop.name == "coin" && prop.metallic < 0.5));
    }
    for (i, y, metal) in updates {
        if let Some(prop) = doc.props.get_mut(i) {
            prop.position[1] = y;
            if metal {
                prop.metallic = 1.0;
                prop.roughness = 0.12;
            }
        }
    }
}

fn refresh_coin_count(doc: &mut WorldDoc) {
    doc.coins = doc
        .props
        .iter()
        .filter(|p| p.name == "coin" && p.enabled)
        .count() as u32;
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

fn look_yaw_from_doc(doc: &WorldDoc) -> f32 {
    let Some(cam) = doc.cameras.first() else {
        return 0.0;
    };
    let dx = cam.position[0] - cam.target[0];
    let dz = cam.position[2] - cam.target[2];
    // collectathon: eye = look + (-sin(yaw)*dist, …, -cos(yaw)*dist)
    (-dx).atan2(-dz)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectathon::{coin_path, BODY_H, STAR_NEED};

    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const ORB: &str = include_str!("../tests/fixtures/orb_rush_world.json");

    #[test]
    fn wasd_tick_moves_walker_on_heightfield() {
        let mut play = WorldPlay::from_json(CREST).unwrap();
        play.start();
        let start = play.doc.player.as_ref().unwrap().position;
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..45 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap();
        let dx = p.position[0] - start[0];
        let dz = p.position[2] - start[2];
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(
            dist > 1.5,
            "WASD forward should move walker in WorldDoc, dist={dist} pos={:?}",
            p.position
        );
        assert!(p.on_ground, "tick sits on the named height fn");
        let ground = play.doc.height_at(p.position[0], p.position[2]) + BODY_H;
        assert!(
            (p.position[1] - ground).abs() < 0.05,
            "foot y {} vs ground {}",
            p.position[1],
            ground
        );
        let twin = play
            .doc
            .walkers
            .iter()
            .find(|w| w.id == "walker:player")
            .unwrap();
        assert_eq!(twin.position, p.position);
    }

    #[test]
    fn look_updates_camera_in_world_doc() {
        let mut play = WorldPlay::from_json(CREST).unwrap();
        play.start();
        let yaw0 = play.look_yaw;
        let eye0 = play.doc.cameras[0].position;
        play.add_look(0.6, 0.0);
        play.tick(1.0 / 60.0);
        assert!((play.look_yaw - yaw0 - 0.6).abs() < 1e-4);
        let eye = play.doc.cameras[0].position;
        let d = (eye[0] - eye0[0]).abs() + (eye[2] - eye0[2]).abs();
        assert!(d > 0.2, "chase camera should orbit, delta={d}");
        let tgt = play.doc.cameras[0].target;
        let p = play.doc.player.as_ref().unwrap().position;
        assert!((tgt[0] - p[0]).abs() < 0.05);
        assert!((tgt[2] - p[2]).abs() < 0.05);
    }

    #[test]
    fn strafe_and_idle_tick_orb_rush_floor() {
        let mut play = WorldPlay::from_json(ORB).unwrap();
        let start = play.doc.player.as_ref().unwrap().position;
        play.input = WalkInput {
            lx: 1.0,
            lz: 0.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap();
        assert!(
            (p.position[0] - start[0]).abs() > 0.4,
            "strafe should move x, got {:?}",
            p.position
        );
        play.input = WalkInput::default();
        let mid = p.position;
        play.tick(1.0 / 60.0);
        let p2 = play.doc.player.as_ref().unwrap();
        let drift = (p2.position[0] - mid[0]).abs() + (p2.position[2] - mid[2]).abs();
        assert!(drift < 0.02, "idle tick must not drift, {drift}");
    }

    #[test]
    fn title_does_not_walk_until_confirm() {
        let mut play = WorldPlay::from_json(CREST).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        let z = play.doc.player.as_ref().unwrap().position[2];
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        assert_eq!(play.doc.player.as_ref().unwrap().position[2], z);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap().position;
        let dist = ((p[0]).powi(2) + (p[2] - z).powi(2)).sqrt();
        assert!(
            dist > 1.0,
            "after start, WASD should move, dist={dist} pos={p:?}"
        );
    }

    #[test]
    fn picking_a_star_counts_and_six_finishes() {
        let mut play = WorldPlay::from_json(CREST).unwrap();
        play.start();
        let stars: Vec<_> = play
            .doc
            .props
            .iter()
            .filter(|p| p.name == "star" && p.enabled)
            .map(|p| (p.position[0], p.position[2]))
            .collect();
        assert!(
            stars.len() >= STAR_NEED as usize,
            "collectathon layout needs {} stars, got {}",
            STAR_NEED,
            stars.len()
        );
        assert!(
            play.doc.coins >= 8,
            "coins live in the dump, got {}",
            play.doc.coins
        );
        assert!(coin_path().len() >= 20);
        let first = stars[0];
        let y0 = play.doc.height_at(first.0, first.1) + BODY_H;
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [first.0, y0, first.1];
        }
        let walker = play.doc.player.clone().unwrap();
        write_player(&mut play.doc, walker);
        play.tick(1.0 / 60.0);
        assert_eq!(play.game.stars, 1);
        assert!(play
            .doc
            .props
            .iter()
            .any(|p| p.name == "star" && !p.enabled));
        let live_coins = play
            .doc
            .props
            .iter()
            .filter(|p| p.name == "coin" && p.enabled)
            .count();
        assert_eq!(play.doc.coins, live_coins as u32);

        let rest: Vec<_> = play
            .doc
            .props
            .iter()
            .filter(|p| p.name == "star" && p.enabled)
            .map(|p| (p.position[0], p.position[2]))
            .take(STAR_NEED as usize - 1)
            .collect();
        for (x, z) in rest {
            let y = play.doc.height_at(x, z) + BODY_H;
            if let Some(p) = play.doc.player.as_mut() {
                p.position = [x, y, z];
            }
            let walker = play.doc.player.clone().unwrap();
            write_player(&mut play.doc, walker);
            play.tick(1.0 / 60.0);
        }
        assert!(won(play.game.stars), "stars {}", play.game.stars);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert!(play.game.score > 0);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn crest_seed_sits_coins_on_heightfield() {
        let play = WorldPlay::from_json(CREST).unwrap();
        let coins: Vec<_> = play
            .doc
            .props
            .iter()
            .filter(|p| p.name == "coin" && p.enabled)
            .collect();
        assert!(coins.len() >= 8);
        for c in &coins {
            let ground = play.doc.height_at(c.position[0], c.position[2]);
            assert!(
                (c.position[1] - ground - 0.55).abs() < 0.05,
                "coin y {} vs ground {}",
                c.position[1],
                ground
            );
            assert!(c.metallic >= 0.5);
        }
        let player = play.doc.player.as_ref().unwrap();
        assert!(player.on_ground);
    }
}
