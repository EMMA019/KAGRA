//! Declarative feel presets.
//!
//! A game names whole experiences instead of tuning dozens of numbers:
//!
//! ```json
//! "feel": {"preset": "snappy_runner",
//!          "movement": {"jump": "forgiving", "lane_hop_secs": 0.14},
//!          "camera": {"preset": "soft_follow", "look_ahead": true},
//!          "feedback": {"hit_stop": "light", "landing_effect": "medium"}}
//! ```
//!
//! `preset` picks a bundle (input response, jump arc, squash, camera,
//! hit-stop, landing shake, particles, SFX hook). Each section can swap
//! one part by name, and any number can still be overridden. [`resolve`]
//! turns it into concrete [`FeelParams`]. Unknown names are errors.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeelSpec {
    /// Bundle: `snappy_runner` (default), `classic`, `arcade_punchy`, `chill`.
    pub preset: String,
    pub movement: MovementFeel,
    pub camera: CameraFeel,
    pub feedback: FeedbackFeel,
}

impl Default for FeelSpec {
    fn default() -> Self {
        Self {
            preset: "snappy_runner".into(),
            movement: MovementFeel::default(),
            camera: CameraFeel::default(),
            feedback: FeedbackFeel::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MovementFeel {
    /// `snappy_runner`, `smooth_runner`, or `free_steer` (held keys steer).
    pub preset: Option<String>,
    /// `standard`, `forgiving`, `snappy`, `floaty`.
    pub jump: Option<String>,
    /// `squash_and_recover`, `stiff`, `bouncy`.
    pub landing: Option<String>,
    pub lane_hop_secs: Option<f32>,
    pub gravity_scale: Option<f32>,
    pub velocity_scale: Option<f32>,
    pub jump_buffer_secs: Option<f32>,
    pub land_squash_secs: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CameraFeel {
    /// `soft_follow`, `tight`, `cinematic`.
    pub preset: Option<String>,
    pub look_ahead: Option<bool>,
    /// Lateral follow stiffness (spring rate).
    pub follow: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeedbackFeel {
    /// `none`, `light`, `heavy`.
    pub hit_stop: Option<String>,
    /// `none`, `light`, `medium`, `heavy`.
    pub landing_effect: Option<String>,
    pub hit_stop_secs: Option<f32>,
    /// Spark count multiplier.
    pub particles: Option<f32>,
    /// Fire the kit's SFX cues (jump, land, hit, pickup).
    pub sfx: Option<bool>,
}

/// Resolved numbers a kit reads every frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FeelParams {
    /// Eased hop per lane. `None` keeps free analog steering only.
    pub lane_hop_secs: Option<f32>,
    /// Arrow / A-D presses hop one lane (else they steer while held).
    pub key_lanes: bool,
    pub gravity_scale: f32,
    pub velocity_scale: f32,
    pub jump_buffer_secs: Option<f32>,
    pub land_squash_secs: f32,
    pub cam_follow: f32,
    pub cam_lift: f32,
    pub cam_look: f32,
    pub cam_bank: f32,
    pub look_ahead: bool,
    pub hit_shake: f32,
    pub hit_stop_secs: f32,
    pub land_shake: (f32, f32),
    pub particles: f32,
    pub sfx: bool,
}

struct Bundle {
    movement: &'static str,
    jump: &'static str,
    landing: &'static str,
    camera: &'static str,
    look_ahead: bool,
    hit_stop: &'static str,
    landing_effect: &'static str,
}

pub const BUNDLES: [&str; 4] = ["snappy_runner", "classic", "arcade_punchy", "chill"];

fn bundle(name: &str) -> Result<Bundle, String> {
    let b = |movement, jump, landing, camera, look_ahead, hit_stop, landing_effect| Bundle {
        movement,
        jump,
        landing,
        camera,
        look_ahead,
        hit_stop,
        landing_effect,
    };
    Ok(match name {
        "snappy_runner" => b(
            "snappy_runner",
            "standard",
            "squash_and_recover",
            "soft_follow",
            true,
            "light",
            "medium",
        ),
        "classic" => b(
            "free_steer",
            "standard",
            "squash_and_recover",
            "soft_follow",
            false,
            "light",
            "medium",
        ),
        "arcade_punchy" => b(
            "snappy_runner",
            "snappy",
            "bouncy",
            "tight",
            true,
            "heavy",
            "heavy",
        ),
        "chill" => b(
            "smooth_runner",
            "floaty",
            "squash_and_recover",
            "cinematic",
            false,
            "none",
            "light",
        ),
        _ => return Err(format!("feel.preset {name:?} not in {BUNDLES:?}")),
    })
}

fn bad(field: &str, name: &str, ok: &[&str]) -> String {
    format!("feel.{field} {name:?} not in {ok:?}")
}

pub fn resolve(spec: &FeelSpec) -> Result<FeelParams, String> {
    let b = bundle(&spec.preset)?;
    let m = &spec.movement;
    let (hop, key_lanes) = match m.preset.as_deref().unwrap_or(b.movement) {
        "snappy_runner" => (Some(0.17), true),
        "smooth_runner" => (Some(0.2), true),
        "free_steer" => (None, false),
        n => {
            return Err(bad(
                "movement.preset",
                n,
                &["snappy_runner", "smooth_runner", "free_steer"],
            ))
        }
    };
    // Gravity and launch scale together so the apex stays near the spec's.
    let (g, v, buf) = match m.jump.as_deref().unwrap_or(b.jump) {
        "standard" => (1.0, 1.0, None),
        "forgiving" => (0.9, 1.0, Some(0.2)),
        "snappy" => (1.4, 1.18, None),
        "floaty" => (0.75, 0.87, None),
        n => {
            return Err(bad(
                "movement.jump",
                n,
                &["standard", "forgiving", "snappy", "floaty"],
            ))
        }
    };
    let squash = match m.landing.as_deref().unwrap_or(b.landing) {
        "squash_and_recover" => 0.12,
        "stiff" => 0.0,
        "bouncy" => 0.2,
        n => {
            return Err(bad(
                "movement.landing",
                n,
                &["squash_and_recover", "stiff", "bouncy"],
            ))
        }
    };
    let c = &spec.camera;
    let (follow, lift, look, bank) = match c.preset.as_deref().unwrap_or(b.camera) {
        "soft_follow" => (12.0, 8.0, 8.0, 5.5),
        "tight" => (20.0, 14.0, 14.0, 8.0),
        "cinematic" => (6.0, 4.0, 4.0, 3.0),
        n => {
            return Err(bad(
                "camera.preset",
                n,
                &["soft_follow", "tight", "cinematic"],
            ))
        }
    };
    let f = &spec.feedback;
    let (shake, stop) = match f.hit_stop.as_deref().unwrap_or(b.hit_stop) {
        "none" => (0.3, 0.02),
        "light" => (0.46, 0.11),
        "heavy" => (0.7, 0.22),
        n => return Err(bad("feedback.hit_stop", n, &["none", "light", "heavy"])),
    };
    let (land, particles) = match f.landing_effect.as_deref().unwrap_or(b.landing_effect) {
        "none" => ((0.0, 0.0), 0.5),
        "light" => ((0.04, 0.12), 0.75),
        "medium" => ((0.08, 0.18), 1.0),
        "heavy" => ((0.14, 0.26), 1.4),
        n => {
            return Err(bad(
                "feedback.landing_effect",
                n,
                &["none", "light", "medium", "heavy"],
            ))
        }
    };
    Ok(FeelParams {
        lane_hop_secs: m.lane_hop_secs.or(hop).map(|s| s.max(0.02)),
        key_lanes,
        gravity_scale: m.gravity_scale.unwrap_or(g),
        velocity_scale: m.velocity_scale.unwrap_or(v),
        jump_buffer_secs: m.jump_buffer_secs.or(buf),
        land_squash_secs: m.land_squash_secs.unwrap_or(squash),
        cam_follow: c.follow.unwrap_or(follow),
        cam_lift: lift,
        cam_look: look,
        cam_bank: bank,
        look_ahead: c.look_ahead.unwrap_or(b.look_ahead),
        hit_shake: shake,
        hit_stop_secs: f.hit_stop_secs.unwrap_or(stop),
        land_shake: land,
        particles: f.particles.unwrap_or(particles),
        sfx: f.sfx.unwrap_or(true),
    })
}

/// Ease-out cubic on `0..=1`: fast start, soft settle.
pub fn ease_out(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundle_resolves_and_default_is_snappy() {
        for b in BUNDLES {
            let s = FeelSpec {
                preset: b.into(),
                ..Default::default()
            };
            resolve(&s).unwrap();
        }
        let p = resolve(&FeelSpec::default()).unwrap();
        assert_eq!(p.lane_hop_secs, Some(0.17));
        assert!(p.key_lanes && p.look_ahead);
        assert_eq!((p.hit_shake, p.hit_stop_secs), (0.46, 0.11));
    }

    #[test]
    fn parts_swap_by_name_and_numbers_override() {
        let s: FeelSpec = serde_json::from_str(
            r#"{"preset":"classic","movement":{"jump":"forgiving","lane_hop_secs":0.15},"feedback":{"hit_stop":"heavy"}}"#,
        )
        .unwrap();
        let p = resolve(&s).unwrap();
        assert_eq!(p.lane_hop_secs, Some(0.15));
        assert_eq!(p.jump_buffer_secs, Some(0.2));
        assert_eq!(p.hit_stop_secs, 0.22);
        assert!(!p.key_lanes);
        let typo = FeelSpec {
            preset: "snapy".into(),
            ..Default::default()
        };
        assert!(resolve(&typo).is_err());
        assert!(serde_json::from_str::<FeelSpec>(r#"{"movment":{}}"#).is_err());
    }
}
