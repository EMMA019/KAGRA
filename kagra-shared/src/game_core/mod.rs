//! Genre-agnostic game core shared by every kit.
//!
//! - [`juice`]: springs, camera shake, hitstop, spark bursts.
//! - [`collide`]: entity hitboxes from a spec row (glb AABB × scale, inset).
//! - [`run`]: score / pickups / death, HUD score panel, result hand-off.
//! - [`lives`], [`missions`], [`result`]: the run rules every kit shares
//!   (lives and the blink after a hit, per-run goals, game-over and banner),
//!   driven by the `lives` and `missions` sections of the GameSpec JSON.
//! - [`items`]: the running pick-up effects (timers), the weighted draw of which
//!   pick-up a slot holds, and the spec lookups for an effect's colour and length.
//! - [`Kit`] + [`tick_frame`]: the fixed-step frame driver. A kit supplies
//!   one sim step; the core owns substepping, hitstop, title attract, and
//!   juice decay in a fixed order so runs replay exactly.
//!
//! The runner kit (`neon_cat.rs`) is the only kit today.

pub mod collide;
pub mod feel;
pub mod items;
pub mod juice;
pub mod lives;
pub mod missions;
pub mod result;
pub mod run;

use crate::flow::{FlowPhase, GameFlow};

pub use collide::Hitbox;
pub use items::{Effects, Timed};
pub use juice::{Juice, JuiceShake, Spark, Spring1};
pub use lives::{Hit, Lives, LivesSpec};
pub use missions::{Counters, MissionKind, MissionSpec, MissionsSpec, Tracker};
pub use result::ResultState;
pub use run::RunStats;

/// Shared per-run state every kit carries.
#[derive(Debug, Default)]
pub struct Core {
    pub stats: RunStats,
    pub juice: Juice,
    /// Hits left and the blink after one ([`lives`]).
    pub lives: Lives,
    /// Which goals are done ([`missions`]).
    pub missions: Tracker,
    /// The game-over cue and the banner ([`result`]).
    pub result: ResultState,
    /// Which timed pick-up effects are running ([`items`]).
    pub effects: Effects,
}

/// One genre's sim, driven by [`tick_frame`].
pub trait Kit {
    fn core(&self) -> &Core;
    fn core_mut(&mut self) -> &mut Core;
    fn flow(&self) -> &GameFlow;
    fn flow_mut(&mut self) -> &mut GameFlow;
    /// Substeps for a frame of `dt` (anti-tunnelling). At least 1.
    fn substeps(&self, dt: f32) -> u32;
    /// One fixed sim step while playing.
    fn step(&mut self, dt: f32);
    /// Title-screen attract motion.
    fn attract(&mut self, dt: f32);
    /// Frozen-frame animation during hitstop.
    fn hitstop_frame(&mut self, _dt: f32) {}
    /// Camera follow after a played frame.
    fn follow(&mut self, dt: f32);
    /// Sparks inside solid geometry are dropped.
    fn blocks_spark(&self, x: f32, y: f32, z: f32) -> bool;
    /// Rebuild the drawable world.
    fn rebuild(&mut self);
}

/// Advance one frame. `dt` is clamped to 50 ms.
pub fn tick_frame<K: Kit>(kit: &mut K, dt: f32) {
    let dt = dt.clamp(0.0, 0.05);
    if dt > 0.0 {
        crate::mixer::tick(dt);
    }
    kit.core_mut().stats.clear_events();
    if dt <= 0.0 {
        kit.rebuild();
        return;
    }
    if kit.core().juice.hitstop > 0.0 {
        let juice = &mut kit.core_mut().juice;
        juice.hitstop = (juice.hitstop - dt).max(0.0);
        finish_juice(kit, dt);
        kit.hitstop_frame(dt);
        let core = kit.core();
        if core.juice.hitstop == 0.0 && !core.stats.alive && kit.flow().phase == FlowPhase::Play {
            let (score, rings, line) = (
                core.stats.score,
                core.stats.rings,
                format!("HIT {}", core.stats.death),
            );
            kit.flow_mut().finish(score, rings, &line);
        }
        kit.rebuild();
        return;
    }
    match kit.flow().phase {
        FlowPhase::Title => {
            kit.attract(dt);
            kit.follow(dt);
        }
        FlowPhase::Play if kit.core().stats.alive => {
            let steps = kit.substeps(dt).max(1);
            let h = dt / steps as f32;
            for _ in 0..steps {
                kit.step(h);
                if !kit.core().stats.alive {
                    break;
                }
            }
            kit.follow(dt);
        }
        _ => {}
    }
    finish_juice(kit, dt);
    kit.rebuild();
}

fn finish_juice<K: Kit>(kit: &mut K, dt: f32) {
    let mut juice = std::mem::take(&mut kit.core_mut().juice);
    juice.tick_sparks(dt, |x, y, z| kit.blocks_spark(x, y, z));
    juice.decay(dt);
    kit.core_mut().juice = juice;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_spec::GameSpec;

    #[test]
    fn stats_roll_fractional_points_and_die_once() {
        let mut s = RunStats::default();
        s.add_points(0.6);
        s.add_points(0.6);
        assert_eq!(s.score, 1);
        s.pickup(100);
        assert_eq!((s.score, s.rings, s.just_rings), (101, 1, 1));
        assert!(s.die("CRATE"));
        assert!(!s.die("BARREL"));
        assert_eq!(s.death, "CRATE");
    }

    #[test]
    fn seeded_sparks_replay_and_respect_blockers() {
        let run = |blocked: bool| {
            let mut j = Juice::seeded(7);
            j.burst(0.0, 0.5, 0.0, 50, [255, 0, 0], 3.0, 0.1);
            assert_eq!(j.sparks.len(), juice::MAX_SPARKS);
            j.tick_sparks(1.0 / 60.0, |_, _, _| blocked);
            j.sparks.iter().map(|s| s.x).collect::<Vec<_>>()
        };
        assert_eq!(run(false), run(false));
        assert!(run(true).is_empty());
    }

    #[test]
    fn hitbox_is_floor_seated_and_inset() {
        let spec = GameSpec::default();
        let crate_box = Hitbox::place(&spec, &spec.obstacles[0], 1.0, 20.0);
        assert!(crate_box.skin_min[1].abs() < 1e-6);
        assert!(crate_box.feet_hit(1.0, 0.0, 20.0));
        assert!(!crate_box.feet_hit(1.0, crate_box.hit_top + 0.01, 20.0));
        assert!(!crate_box.feet_hit(1.0 + crate_box.hit_hx + 0.01, 0.0, 20.0));
    }
}
