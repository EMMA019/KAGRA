//! Named-clip player with crossfades for skinned walkers.
//!
//! Idea after three.js `AnimationMixer` / `AnimationAction.crossFadeTo`
//! (three.js, MIT License, Copyright 2010-2024 three.js authors): the new
//! action fades in while the old one fades out over `duration` seconds, both
//! clocks keep running, and the pose is a per-bone weighted blend.
//! The mixer only tracks names, times and weights; `gltf_load` does the pose
//! math from the dump fields this writes (`clip_name`, `clip`, `fade_from`,
//! `fade_from_clip`, `fade_from_weight`).

/// One running clip clock.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipClock {
    pub name: String,
    /// Seconds since this clip started (wrapped by the sampler).
    pub time: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimMixer {
    pub current: Option<ClipClock>,
    pub previous: Option<ClipClock>,
    /// Fade length in seconds (0 = hard cut).
    pub fade_duration: f32,
    /// Seconds into the current fade.
    pub fade_elapsed: f32,
    /// Clip playback speed (1 = authored).
    pub time_scale: f32,
}

impl AnimMixer {
    pub fn new() -> Self {
        Self {
            time_scale: 1.0,
            ..Self::default()
        }
    }

    /// Hard switch to `name` (no fade). Re-playing the current clip is a no-op.
    pub fn play(&mut self, name: &str) {
        if self.current.as_ref().is_some_and(|c| c.name == name) {
            return;
        }
        self.current = Some(ClipClock {
            name: name.to_string(),
            time: 0.0,
        });
        self.previous = None;
        self.fade_duration = 0.0;
        self.fade_elapsed = 0.0;
    }

    /// Fade from the current clip to `name` over `duration` seconds.
    pub fn cross_fade_to(&mut self, name: &str, duration: f32) {
        if self.current.as_ref().is_some_and(|c| c.name == name) {
            return;
        }
        if duration <= 0.0 || self.current.is_none() {
            self.play(name);
            return;
        }
        self.previous = self.current.take();
        self.current = Some(ClipClock {
            name: name.to_string(),
            time: 0.0,
        });
        self.fade_duration = duration;
        self.fade_elapsed = 0.0;
    }

    /// Advance clocks and the fade by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        let step = dt.max(0.0) * self.time_scale;
        if let Some(c) = self.current.as_mut() {
            c.time += step;
        }
        if let Some(p) = self.previous.as_mut() {
            p.time += step;
        }
        if self.previous.is_some() {
            self.fade_elapsed += dt.max(0.0);
            if self.fade_elapsed >= self.fade_duration {
                self.previous = None;
                self.fade_elapsed = 0.0;
                self.fade_duration = 0.0;
            }
        }
    }

    /// Weight of the old clip, 1 → 0 across the fade (linear, like three.js).
    pub fn previous_weight(&self) -> f32 {
        if self.previous.is_none() || self.fade_duration <= 0.0 {
            return 0.0;
        }
        (1.0 - self.fade_elapsed / self.fade_duration).clamp(0.0, 1.0)
    }

    /// Weight of the current clip (`1 - previous_weight`).
    pub fn current_weight(&self) -> f32 {
        if self.current.is_none() {
            return 0.0;
        }
        1.0 - self.previous_weight()
    }

    /// Write the dump fields a walker reads.
    pub fn apply(&self, w: &mut crate::world_doc::WorldWalker) {
        if let Some(c) = &self.current {
            w.clip_name = Some(c.name.clone());
            // `clip == 0` means rest pose, so nudge the very first frame.
            w.clip = c.time.max(1e-4);
        }
        match &self.previous {
            Some(p) if self.previous_weight() > 0.0 => {
                w.fade_from = Some(p.name.clone());
                w.fade_from_clip = p.time;
                w.fade_from_weight = self.previous_weight();
            }
            _ => {
                w.fade_from = None;
                w.fade_from_clip = 0.0;
                w.fade_from_weight = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossfade_weights_ramp_and_sum_to_one() {
        let mut m = AnimMixer::new();
        m.play("Walk");
        m.update(0.5);
        m.cross_fade_to("Run", 0.4);
        assert_eq!(m.previous_weight(), 1.0);
        m.update(0.1);
        assert!((m.previous_weight() - 0.75).abs() < 1e-5);
        assert!((m.previous_weight() + m.current_weight() - 1.0).abs() < 1e-6);
        assert!((m.previous.as_ref().unwrap().time - 0.6).abs() < 1e-5);
        assert!((m.current.as_ref().unwrap().time - 0.1).abs() < 1e-5);
        m.update(0.3);
        assert!(m.previous.is_none(), "fade finished");
        assert_eq!(m.current_weight(), 1.0);
        assert_eq!(m.current.as_ref().unwrap().name, "Run");
    }

    #[test]
    fn play_is_a_hard_cut_and_same_clip_is_noop() {
        let mut m = AnimMixer::new();
        m.cross_fade_to("Idle", 0.5); // nothing playing: hard start
        assert!(m.previous.is_none());
        m.update(1.0);
        m.cross_fade_to("Idle", 0.5);
        assert!((m.current.as_ref().unwrap().time - 1.0).abs() < 1e-6);
        m.play("Jump");
        assert_eq!(m.previous_weight(), 0.0);
        assert_eq!(m.current.as_ref().unwrap().time, 0.0);
    }

    #[test]
    fn apply_writes_walker_dump_fields() {
        let mut m = AnimMixer::new();
        m.play("Walk");
        m.update(0.2);
        m.cross_fade_to("Run", 1.0);
        m.update(0.25);
        let mut w = crate::world_doc::WorldWalker::default();
        m.apply(&mut w);
        assert_eq!(w.clip_name.as_deref(), Some("Run"));
        assert_eq!(w.fade_from.as_deref(), Some("Walk"));
        assert!((w.fade_from_weight - 0.75).abs() < 1e-5);
        assert!((w.fade_from_clip - 0.45).abs() < 1e-5);
    }
}
