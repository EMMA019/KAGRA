//! Body motion for a crowd of static models. The engine does not skin a crowd, so each figure
//! gets a little motion from its transform: a bob, a squash on the beat, a sway, a lean, a
//! flinch when hit. Every figure has its own phase and tempo (from its id), so a crowd does not
//! move in lockstep. Picture only: a sim never reads it. First used by Moonlit Swarm's foes.
//!
//! Apply a [`Pose`] on top of the resting transform: `lift` to y, `wide`/`tall` to the scale,
//! `pitch` as a lean toward the facing direction (a `WorldProp`'s +pitch lifts its nose, so set
//! `prop.pitch = -pose.pitch`) and `roll` as is.

/// A style of motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    /// The ghost: floats, sways side to side, drifts up and down slowly.
    Float,
    /// The zombie: a heavy, uneven shuffle, rolling onto one leg.
    Shamble,
    /// The skeleton: quick little hops that squash on landing.
    Hop,
    /// The vampire: glides leaning forward, a slow cape-like sway.
    Glide,
    /// The keeper: a slow, heavy stomp that squashes on each step.
    Stomp,
}

impl Motion {
    /// The style a JSON row names: `float`, `shamble`, `hop`, `glide` or `stomp`.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "float" => Motion::Float,
            "shamble" => Motion::Shamble,
            "hop" => Motion::Hop,
            "glide" => Motion::Glide,
            "stomp" => Motion::Stomp,
            _ => return None,
        })
    }
}

/// Offsets on top of a foe's resting transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Metres up.
    pub lift: f32,
    /// Scale across (x and z) and up (y), 1 = rest.
    pub wide: f32,
    pub tall: f32,
    /// Radians: lean toward the hero (+) or back (-), and side to side.
    pub pitch: f32,
    pub roll: f32,
}

impl Pose {
    pub const REST: Pose = Pose {
        lift: 0.0,
        wide: 1.0,
        tall: 1.0,
        pitch: 0.0,
        roll: 0.0,
    };
}

/// A foe's own phase (0..TAU) and tempo (0.85..1.15) from its id, so neighbours differ.
pub fn phase_tempo(uid: u32) -> (f32, f32) {
    let h = uid.wrapping_mul(0x9E37_79B9) ^ (uid >> 7).wrapping_mul(0x85EB_CA6B);
    let a = (h & 0xFFFF) as f32 / 65535.0;
    let b = ((h >> 16) & 0xFFFF) as f32 / 65535.0;
    (a * std::f32::consts::TAU, 0.85 + 0.3 * b)
}

/// The pose at time `t` (seconds) for a foe with this phase and tempo. `amount` scales it
/// (0 = rest, 1 = as tuned). `hit` is the hit flash, 0..~0.1: a flinch back.
pub fn pose(m: Motion, t: f32, phase: f32, tempo: f32, amount: f32, hit: f32) -> Pose {
    if amount <= 0.0 {
        return Pose::REST;
    }
    let w = t * tempo;
    let s = |hz: f32| (w * hz * std::f32::consts::TAU + phase).sin();
    // A bounce: |sin| is a hop that lands hard; 1 - it peaks on the landing.
    let hop = |hz: f32| s(hz * 0.5).abs();
    let mut p = match m {
        Motion::Float => Pose {
            lift: 0.08 * s(0.6),
            wide: 1.0 + 0.03 * s(1.2),
            tall: 1.0 - 0.03 * s(1.2),
            pitch: 0.10,
            roll: 0.16 * s(0.45),
        },
        Motion::Shamble => {
            // One leg drags: a long roll onto it, a short dip off it.
            let step = s(0.9);
            Pose {
                lift: 0.03 * step.max(0.0),
                wide: 1.0 + 0.04 * (-step).max(0.0),
                tall: 1.0 - 0.05 * (-step).max(0.0),
                pitch: 0.14 + 0.05 * s(1.8),
                roll: 0.20 * step,
            }
        }
        Motion::Hop => {
            let up = hop(3.2);
            let land = (1.0 - up).powi(3);
            Pose {
                lift: 0.16 * up,
                wide: 1.0 + 0.12 * land,
                tall: 1.0 - 0.14 * land + 0.06 * up,
                pitch: 0.06,
                roll: 0.08 * s(1.6),
            }
        }
        Motion::Glide => Pose {
            lift: 0.05 * s(0.5),
            wide: 1.0,
            tall: 1.0 + 0.02 * s(1.0),
            pitch: 0.22 + 0.04 * s(0.5),
            roll: 0.10 * s(0.35),
        },
        Motion::Stomp => {
            let up = hop(1.4);
            let land = (1.0 - up).powi(4);
            Pose {
                lift: 0.06 * up,
                wide: 1.0 + 0.09 * land,
                tall: 1.0 - 0.10 * land,
                pitch: 0.08,
                roll: 0.14 * s(0.7),
            }
        }
    };
    let k = amount.min(2.0);
    p.lift *= k;
    p.wide = 1.0 + (p.wide - 1.0) * k;
    p.tall = 1.0 + (p.tall - 1.0) * k;
    p.pitch *= k;
    p.roll *= k;
    // The flinch: thrown back and squashed for the length of the hit flash.
    let f = (hit * 10.0).clamp(0.0, 1.0);
    p.pitch -= 0.45 * f;
    p.wide += 0.10 * f;
    p.tall -= 0.10 * f;
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Motion; 5] = [
        Motion::Float,
        Motion::Shamble,
        Motion::Hop,
        Motion::Glide,
        Motion::Stomp,
    ];

    #[test]
    fn off_is_the_rest_pose_and_every_motion_stays_small() {
        for m in ALL {
            assert_eq!(pose(m, 1.3, 0.4, 1.0, 0.0, 0.0), Pose::REST);
            for i in 0..400 {
                let p = pose(m, i as f32 * 0.017, 1.1, 1.0, 1.0, 0.0);
                assert!(p.lift.abs() <= 0.2, "{m:?} lift {}", p.lift);
                assert!((0.8..=1.2).contains(&p.wide) && (0.8..=1.2).contains(&p.tall));
                assert!(p.pitch.abs() <= 0.3 && p.roll.abs() <= 0.25, "{m:?} {p:?}");
            }
        }
    }

    #[test]
    fn neighbours_do_not_move_in_lockstep() {
        let (a, ta) = phase_tempo(7);
        let (b, tb) = phase_tempo(8);
        assert!((a - b).abs() > 0.1 || (ta - tb).abs() > 0.02);
        let mut differ = 0;
        for uid in 0..50u32 {
            let (p, t) = phase_tempo(uid);
            assert!((0.0..=std::f32::consts::TAU).contains(&p) && (0.85..=1.15).contains(&t));
            let (q, u) = phase_tempo(uid + 1);
            let x = pose(Motion::Hop, 2.0, p, t, 1.0, 0.0);
            let y = pose(Motion::Hop, 2.0, q, u, 1.0, 0.0);
            if (x.lift - y.lift).abs() > 0.01 {
                differ += 1;
            }
        }
        assert!(differ > 30, "only {differ} of 50 pairs differ");
    }

    #[test]
    fn a_hit_flinches_back() {
        for m in ALL {
            let calm = pose(m, 0.5, 0.0, 1.0, 1.0, 0.0);
            let hit = pose(m, 0.5, 0.0, 1.0, 1.0, 0.1);
            assert!(hit.pitch < calm.pitch - 0.3 && hit.tall < calm.tall);
        }
    }
}
