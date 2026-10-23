//! BGM loop, one-shot SFX, volume groups, and mute.
//!
//! The state machine is pure and always compiled. Desktop playback (rodio) is
//! the `playback` feature, opened lazily. No device, wasm, Android, and
//! `cargo test` leave the speakers alone: cues stay in the queue for the
//! shell (Android SoundPool) or a test assert.
//!
//! Clips are original CC0 tones in `assets/audio/` (see `LICENSE.txt`).

use std::cell::RefCell;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeGroup {
    Master,
    Bgm,
    Sfx,
}

impl VolumeGroup {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "master" => Some(Self::Master),
            "bgm" => Some(Self::Bgm),
            "sfx" => Some(Self::Sfx),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MixerState {
    pub master: f32,
    pub bgm: f32,
    pub sfx: f32,
    pub muted: bool,
    pub bgm_name: String,
    pub bgm_gain: f32,
    pub bgm_target: f32,
    pub loop_bgm: bool,
    pub oneshots: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Mixer {
    pub master: f32,
    pub bgm: f32,
    pub sfx: f32,
    pub muted: bool,
    bgm_name: String,
    bgm_gain: f32,
    bgm_target: f32,
    /// Gain units per second toward `bgm_target`. Huge means a snap.
    fade_rate: f32,
    loop_bgm: bool,
    queue: Vec<String>,
    log: Vec<String>,
}

impl Default for Mixer {
    fn default() -> Self {
        Self {
            master: 0.8,
            bgm: 0.7,
            sfx: 1.0,
            muted: false,
            bgm_name: String::new(),
            bgm_gain: 0.0,
            bgm_target: 0.0,
            fade_rate: 0.0,
            loop_bgm: true,
            queue: Vec::new(),
            log: Vec::new(),
        }
    }
}

impl Mixer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_group(&mut self, group: VolumeGroup, volume: f32) {
        let v = volume.clamp(0.0, 1.0);
        match group {
            VolumeGroup::Master => self.master = v,
            VolumeGroup::Bgm => self.bgm = v,
            VolumeGroup::Sfx => self.sfx = v,
        }
    }

    pub fn set_mute(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub fn play_bgm(&mut self, name: &str, fade_secs: f32, loop_bgm: bool) {
        self.bgm_name = name.to_string();
        self.loop_bgm = loop_bgm;
        self.bgm_target = 1.0;
        if fade_secs <= 1.0e-4 {
            self.bgm_gain = 1.0;
            self.fade_rate = 0.0;
        } else {
            self.fade_rate = 1.0 / fade_secs;
        }
        self.log.push(format!("bgm:{name}"));
    }

    pub fn stop_bgm(&mut self, fade_secs: f32) {
        self.bgm_target = 0.0;
        if fade_secs <= 1.0e-4 {
            self.bgm_gain = 0.0;
            self.fade_rate = 0.0;
            self.bgm_name.clear();
        } else {
            self.fade_rate = 1.0 / fade_secs;
        }
    }

    /// One-shot. Muted / zero groups still record a skipped cue so tests can see it.
    pub fn play_sfx(&mut self, name: &str) {
        if name.is_empty() {
            return;
        }
        self.log.push(name.to_string());
        if self.sfx_audible() && self.queue.len() < 8 {
            self.queue.push(name.to_string());
        }
    }

    pub fn sfx_audible(&self) -> bool {
        !self.muted && self.master > 0.0 && self.sfx > 0.0
    }

    pub fn bgm_audible_gain(&self) -> f32 {
        if self.muted || self.bgm_name.is_empty() {
            return 0.0;
        }
        (self.master * self.bgm * self.bgm_gain).clamp(0.0, 1.0)
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 1.0);
        if dt <= 0.0 || self.fade_rate <= 0.0 {
            return;
        }
        let step = self.fade_rate * dt;
        if self.bgm_gain < self.bgm_target {
            self.bgm_gain = (self.bgm_gain + step).min(self.bgm_target);
        } else if self.bgm_gain > self.bgm_target {
            self.bgm_gain = (self.bgm_gain - step).max(self.bgm_target);
        }
        if self.bgm_gain <= 1.0e-4 && self.bgm_target <= 0.0 {
            self.bgm_gain = 0.0;
            self.bgm_name.clear();
            self.fade_rate = 0.0;
        }
    }

    pub fn take_sfx(&mut self) -> Vec<String> {
        std::mem::take(&mut self.queue)
    }

    pub fn log(&self) -> &[String] {
        &self.log
    }

    pub fn state(&self) -> MixerState {
        MixerState {
            master: self.master,
            bgm: self.bgm,
            sfx: self.sfx,
            muted: self.muted,
            bgm_name: self.bgm_name.clone(),
            bgm_gain: self.bgm_gain,
            bgm_target: self.bgm_target,
            loop_bgm: self.loop_bgm,
            oneshots: self.queue.clone(),
        }
    }
}

thread_local! {
    static LOCAL: RefCell<Mixer> = RefCell::new(Mixer::new());
    /// FFI sizes the buffer with a null pointer, then writes. The first call
    /// must not drop the cues before the second call copies them out.
    static HELD_CUES: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn with_mixer<R>(f: impl FnOnce(&mut Mixer) -> R) -> R {
    LOCAL.with(|m| f(&mut m.borrow_mut()))
}

pub fn reset_mixer() {
    with_mixer(|m| *m = Mixer::new());
}

pub fn cue_sfx(name: &str) {
    with_mixer(|m| m.play_sfx(name));
    // Drain into the device on this thread. Android reads the queue after.
    crate::playback::submit_sfx(name);
}

pub fn play_bgm(name: &str, fade_secs: f32) {
    with_mixer(|m| m.play_bgm(name, fade_secs, true));
    crate::playback::sync_bgm();
}

pub fn stop_bgm(fade_secs: f32) {
    with_mixer(|m| m.stop_bgm(fade_secs));
    crate::playback::sync_bgm();
}

pub fn tick(dt: f32) {
    with_mixer(|m| m.tick(dt));
    crate::playback::sync_bgm();
}

pub fn set_group(name: &str, volume: f32) -> Result<(), String> {
    let group = VolumeGroup::parse(name).ok_or_else(|| format!("unknown volume group {name}"))?;
    with_mixer(|m| m.set_group(group, volume));
    crate::playback::sync_bgm();
    Ok(())
}

pub fn set_mute(muted: bool) {
    with_mixer(|m| m.set_mute(muted));
    crate::playback::sync_bgm();
}

pub fn state_json() -> String {
    with_mixer(|m| serde_json::to_string(&m.state()).unwrap_or_else(|_| "{}".into()))
}

/// JSON the Android shell plays with SoundPool / MediaPlayer.
pub fn hold_cues(json: String) {
    HELD_CUES.with(|slot| *slot.borrow_mut() = Some(json));
}

pub fn cues_json() -> String {
    if let Some(held) = HELD_CUES.with(|slot| slot.borrow_mut().take()) {
        return held;
    }
    with_mixer(|m| {
        let state = m.state();
        let shots = m.take_sfx();
        serde_json::json!({
            "muted": state.muted,
            "master": state.master,
            "bgm": state.bgm,
            "sfx": state.sfx,
            "bgm_name": state.bgm_name,
            "bgm_gain": state.bgm_gain,
            "oneshots": shots,
        })
        .to_string()
    })
}

pub fn clip_wav(name: &str) -> Option<&'static [u8]> {
    match name {
        "jump" => Some(include_bytes!("../../assets/audio/jump.wav")),
        "fire" => Some(include_bytes!("../../assets/audio/fire.wav")),
        "card" => Some(include_bytes!("../../assets/audio/card.wav")),
        "bgm" => Some(include_bytes!("../../assets/audio/bgm.wav")),
        _ => None,
    }
}

/// Bundled clip, or a procedural preset (`hit`, `coin`, `explode`, …).
/// A bundled name never falls through to the synth.
pub fn sfx_wav(name: &str) -> Option<Vec<u8>> {
    if let Some(bytes) = clip_wav(name) {
        return Some(bytes.to_vec());
    }
    crate::sfx::synth_wav(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_reaches_target_and_stop_clears_the_name() {
        let mut m = Mixer::new();
        m.play_bgm("bgm", 0.4, true);
        assert!(m.bgm_gain < 0.1);
        m.tick(0.2);
        assert!((m.bgm_gain - 0.5).abs() < 0.05, "gain {}", m.bgm_gain);
        m.tick(0.2);
        assert!((m.bgm_gain - 1.0).abs() < 1.0e-3);
        m.stop_bgm(0.2);
        m.tick(0.2);
        assert!(m.bgm_name.is_empty());
        assert_eq!(m.bgm_gain, 0.0);
    }

    #[test]
    fn mute_and_zero_sfx_drop_the_queue_but_keep_the_log() {
        let mut m = Mixer::new();
        m.play_sfx("jump");
        assert_eq!(m.take_sfx(), vec!["jump".to_string()]);
        m.set_mute(true);
        m.play_sfx("fire");
        assert!(m.take_sfx().is_empty());
        assert!(m.log().iter().any(|s| s == "fire"));
        m.set_mute(false);
        m.set_group(VolumeGroup::Sfx, 0.0);
        m.play_sfx("card");
        assert!(m.take_sfx().is_empty());
    }

    #[test]
    fn clips_are_riff_wavs() {
        for name in ["jump", "fire", "card", "bgm"] {
            let bytes = clip_wav(name).unwrap();
            assert_eq!(&bytes[..4], b"RIFF", "{name}");
            assert!(bytes.len() > 44, "{name}");
        }
        assert!(clip_wav("nope").is_none());
    }

    #[test]
    fn groups_reject_unknown_names() {
        assert!(set_group("voice", 1.0).is_err());
        set_group("master", 0.25).unwrap();
        let state: MixerState = serde_json::from_str(&state_json()).unwrap();
        assert!((state.master - 0.25).abs() < 1.0e-4);
        reset_mixer();
    }
}
