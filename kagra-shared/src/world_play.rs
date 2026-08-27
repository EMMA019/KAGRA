//! Live `WorldDoc` tick: WASD / look → walker + chase camera.
//!
//! Shared-side. Matches collectathon `WalkInput` (camera-relative wish, sit
//! on heightfield, optional jump). Python `Walk.wish` / `CharacterController`
//! (accel 14 / decel 22 / 8-point foot ring / step-up) is the leftover VRM
//! motor — documented, not copied, and not Rapier.

use crate::collectathon::{
    WalkInput, BODY_H, CAM_DISTANCE, CAM_HEIGHT, CAM_LOOK_Y, GRAVITY, JUMP_V, PLAYER_SPEED,
};
use crate::world_doc::{WorldDoc, WorldWalker};
use glam::Vec3;

/// Running play state around a dump document. `doc` is the JSON source of
/// truth after each tick (walker position/yaw + camera).
#[derive(Clone, Debug)]
pub struct WorldPlay {
    pub doc: WorldDoc,
    pub input: WalkInput,
    pub look_yaw: f32,
    pub look_pitch: f32,
    vy: f32,
}

impl WorldPlay {
    pub fn new(doc: WorldDoc) -> Self {
        let look_yaw = look_yaw_from_doc(&doc);
        Self {
            doc,
            input: WalkInput::default(),
            look_yaw,
            look_pitch: 0.0,
            vy: 0.0,
        }
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        Ok(Self::new(WorldDoc::from_json(json)?))
    }

    /// Mouse / arrow look. Pitch is clamped.
    pub fn add_look(&mut self, dyaw: f32, dpitch: f32) {
        self.look_yaw += dyaw;
        self.look_pitch = (self.look_pitch + dpitch).clamp(-0.7, 0.55);
    }

    /// Advance walker + chase camera. `dt` is seconds (clamped).
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        if dt <= 0.0 {
            return;
        }
        let input = self.input.clamped();
        self.step_walker(input, dt);
        self.follow_camera();
        self.input.jump = false;
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
    use crate::collectathon::BODY_H;

    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const ORB: &str = include_str!("../tests/fixtures/orb_rush_world.json");

    #[test]
    fn wasd_tick_moves_walker_on_heightfield() {
        let mut play = WorldPlay::from_json(CREST).unwrap();
        let start = play.doc.player.as_ref().unwrap().position;
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
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
}
