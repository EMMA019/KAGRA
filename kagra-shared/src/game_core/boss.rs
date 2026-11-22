//! Boss presentation: the entrance beat and the named HP bar, as a
//! shared part.
//!
//! The design is city_action's UI polish round (2026-11-17), lifted out
//! of the kit: at spawn the world holds its breath — slow motion on a
//! short clock, a camera push-in on an ease, the boss's name posted big
//! at centre screen — and the fight then reads on a wide top-centre HP
//! bar with the boss's weakness named under it in its own tint. The
//! part owns the beat's timing and the two drawings; the game owns the
//! names, the weakness choice and the damage math (city_action's
//! affinity, Dusk Horde's rotating weapon weakness), so the same part
//! presents a boss in a KitHost kit and in a WorldPlay loop alike.
//!
//! The beat is runtime-only state (no serde): it starts at spawn,
//! advances on real time (a slow-motion world must not slow the
//! beat itself), and expires on its own. Games with a level-up /
//! reward freeze apply the slow scale to their sim step only, never
//! to the beat's own step.

use crate::scene::{DrawList, Quad, TextAlign, TextFace, TextQuad};

/// The entrance beat: slow motion, camera push-in and name banner.
///
/// Lifetimes (city_action's constants, kept): the slow breath lasts
/// [`EntranceBeat::INTRO_SECS`] at [`EntranceBeat::SLOW_SCALE`], the
/// push-in reaches [`EntranceBeat::CAM_ZOOM`] at the bottom of the
/// ease and releases back over [`EntranceBeat::RELEASE_SECS`], and
/// the name stays posted for [`EntranceBeat::BANNER_SECS`].
#[derive(Clone, Copy, Debug)]
pub struct EntranceBeat {
    /// Seconds since `start` (real time). `f32::MAX` = never started.
    t: f32,
}

impl Default for EntranceBeat {
    fn default() -> Self {
        Self { t: f32::MAX }
    }
}

impl EntranceBeat {
    /// The slow-motion window (s, real time): the world holds its
    /// breath for this long after the boss walks in.
    pub const INTRO_SECS: f32 = 0.5;
    /// The sim's time scale during the intro (the ult opening's
    /// own clock in city_action).
    pub const SLOW_SCALE: f32 = 0.35;
    /// The camera push-in's depth: the eye-distance multiplier at
    /// the bottom of the ease.
    pub const CAM_ZOOM: f32 = 0.85;
    /// Seconds over which the push-in eases back out after the
    /// intro window.
    pub const RELEASE_SECS: f32 = 0.9;
    /// How long the entrance name stays posted (real time).
    pub const BANNER_SECS: f32 = 1.8;

    pub fn new() -> Self {
        Self::default()
    }

    /// Start the beat at spawn.
    pub fn start(&mut self) {
        self.t = 0.0;
    }

    /// Advance on real time. A slow-motion world must not slow this
    /// step: the beat is the clock the slow motion hangs from.
    pub fn step(&mut self, dt_real: f32) {
        if self.t < f32::MAX {
            self.t = (self.t + dt_real.max(0.0)).min(Self::BANNER_SECS);
        }
    }

    /// The sim's time scale while the beat runs: slow during the
    /// intro window, full speed after it (the banner may still be
    /// posted — the fight is already live).
    pub fn time_scale(&self) -> f32 {
        if self.t < Self::INTRO_SECS {
            Self::SLOW_SCALE
        } else {
            1.0
        }
    }

    /// The camera eye-distance multiplier: eases from 1.0 down to
    /// [`EntranceBeat::CAM_ZOOM`] across the intro, then back to 1.0
    /// over [`EntranceBeat::RELEASE_SECS`].
    pub fn cam_scale(&self) -> f32 {
        if self.t >= Self::BANNER_SECS || self.t == f32::MAX {
            return 1.0;
        }
        let push = if self.t < Self::INTRO_SECS {
            smoothstep(self.t / Self::INTRO_SECS)
        } else {
            1.0 - smoothstep((self.t - Self::INTRO_SECS) / Self::RELEASE_SECS)
        };
        1.0 - (1.0 - Self::CAM_ZOOM) * push
    }

    /// Seconds of the name banner left, while it is posted.
    pub fn banner_left(&self) -> Option<f32> {
        if self.t < Self::BANNER_SECS {
            Some(Self::BANNER_SECS - self.t)
        } else {
            None
        }
    }

    /// True while the banner is posted (the beat's whole life).
    pub fn active(&self) -> bool {
        self.banner_left().is_some()
    }
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// What the bar draws: the boss's name, its HP fraction and its
/// weakness line (the label plus the tint it reads in, or None for
/// a boss with nothing to name).
#[derive(Clone, Copy, Debug)]
pub struct BossMeter<'a> {
    pub name: &'a str,
    pub hp: f32,
    pub hp_max: f32,
    pub weak: Option<(&'a str, [u8; 4])>,
}

/// The named HP bar (city_action's UI-round geometry, kept): name
/// centred above a wide centred bar, the fill heating gold → orange
/// → red as the boss's HP falls through its thirds, the weakness
/// line under the bar in its own tint. `top` is the name line's y;
/// the bar sits 30*s under it and the weakness 6*s under the bar.
/// Callers hide the bar while the entrance banner is posted — the
/// banner is already saying the name and the weakness.
pub fn draw_boss_bar(list: &mut DrawList, m: &BossMeter, w: f32, s: f32, top: f32) {
    let bw = (w * 0.74).min(470.0 * s);
    let bx = (w - bw) / 2.0;
    let by = top + 30.0 * s;
    let bh = 13.0 * s;
    let frac = if m.hp_max > 0.0 {
        (m.hp / m.hp_max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    list.texts.push(
        TextQuad::new(m.name, w * 0.5, top, 21.0 * s, [255, 190, 160, 255])
            .aligned(TextAlign::Center)
            .with_face(TextFace::Display),
    );
    list.quads
        .push(Quad::new(bx, by, bw, bh, [24, 16, 28, 220]));
    let fill = if frac > 2.0 / 3.0 {
        [232, 176, 75, 255]
    } else if frac > 1.0 / 3.0 {
        [235, 122, 52, 255]
    } else {
        [228, 52, 44, 255]
    };
    list.quads.push(Quad::new(bx, by, bw * frac, bh, fill));
    if let Some((label, tint)) = m.weak {
        list.texts.push(
            TextQuad::new(
                &format!("WEAK: {label}"),
                w * 0.5,
                by + bh + 6.0 * s,
                14.0 * s,
                [tint[0], tint[1], tint[2], 245],
            )
            .aligned(TextAlign::Center)
            .with_face(TextFace::Display),
        );
    }
}

/// The entrance banner: the name big at centre screen with the
/// weakness under it, the same stack the bar carries.
pub fn draw_entrance_banner(
    list: &mut DrawList,
    name: &str,
    weak: Option<(&str, [u8; 4])>,
    w: f32,
    h: f32,
    s: f32,
) {
    list.texts.push(
        TextQuad::new(name, w * 0.5, h * 0.30, 40.0 * s, [255, 190, 160, 255])
            .with_face(TextFace::Display)
            .aligned(TextAlign::Center),
    );
    if let Some((label, tint)) = weak {
        list.texts.push(
            TextQuad::new(
                &format!("WEAK: {label}"),
                w * 0.5,
                h * 0.30 + 54.0 * s,
                20.0 * s,
                tint,
            )
            .with_face(TextFace::Display)
            .aligned(TextAlign::Center),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::DrawList;

    #[test]
    fn the_beat_slowmos_the_intro_then_frees_the_fight() {
        let mut b = EntranceBeat::new();
        assert_eq!(b.time_scale(), 1.0);
        assert!(!b.active());
        b.start();
        assert_eq!(b.time_scale(), EntranceBeat::SLOW_SCALE);
        assert!(b.active());
        b.step(EntranceBeat::INTRO_SECS);
        assert_eq!(b.time_scale(), 1.0);
        assert!(b.active(), "the name outlives the slow breath");
        b.step(EntranceBeat::BANNER_SECS - EntranceBeat::INTRO_SECS);
        assert!(!b.active());
        assert_eq!(b.time_scale(), 1.0);
    }

    #[test]
    fn the_push_in_eases_down_and_back_out() {
        let mut b = EntranceBeat::new();
        b.start();
        assert!((b.cam_scale() - 1.0).abs() < 1e-6);
        b.step(EntranceBeat::INTRO_SECS);
        assert!((b.cam_scale() - EntranceBeat::CAM_ZOOM).abs() < 1e-6);
        b.step(EntranceBeat::RELEASE_SECS / 2.0);
        let mid = b.cam_scale();
        assert!(mid > EntranceBeat::CAM_ZOOM && mid < 1.0);
        b.step(EntranceBeat::RELEASE_SECS / 2.0);
        assert!((b.cam_scale() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_bar_draws_name_fill_and_weak_line() {
        let mut list = DrawList::default();
        draw_boss_bar(
            &mut list,
            &BossMeter {
                name: "GRAVE HULK",
                hp: 60.0,
                hp_max: 100.0,
                weak: Some(("LASER GUN", [110, 240, 255, 255])),
            },
            800.0,
            1.0,
            100.0,
        );
        assert_eq!(list.quads.len(), 2, "well + fill");
        assert_eq!(list.texts.len(), 2, "name + WEAK line");
        assert_eq!(list.texts[0].text, "GRAVE HULK");
        assert_eq!(list.texts[1].text, "WEAK: LASER GUN");
        // A two-thirds-full boss heats orange, not gold.
        assert_eq!(list.quads[1].color, [235, 122, 52, 255]);
    }

    #[test]
    fn the_banner_draws_the_name_and_nothing_when_weak_is_none() {
        let mut list = DrawList::default();
        draw_entrance_banner(&mut list, "DUSK REVENANT", None, 800.0, 600.0, 1.0);
        assert_eq!(list.texts.len(), 1);
        assert_eq!(list.texts[0].text, "DUSK REVENANT");
    }
}
