//! Neon Cat's atmosphere, audio and missions: the data half.
//!
//! Three `GameSpec` sections, all plain JSON, all switchable:
//!
//! * `atmosphere`: night rain, neon signs and a concrete wall. Drawn by
//!   [`crate::atmosphere`]. Cheaper tiers (`low` / `mobile` / `high`) pick
//!   how much is drawn from the dump's `quality`.
//! * `audio`: which file plays for the BGM and for each SFX cue, and the
//!   volumes. The page (or a native shell) plays them; the sim only names
//!   the cue (`mixer::cue_sfx`).
//! * `missions`: a few small goals per run, counted from the run's own
//!   counters (coins, gates, perches).
//!
//! Python mirrors these in `kagra/game_spec.py`; `tests/test_runner_spec.py`
//! checks the field names and order against this file.
//! `GameSpec::legacy()` turns all three off, which keeps the golden runs and
//! the classic measurement baseline untouched.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Which picture tier the dump asked for. `quality` absent means desktop
/// (`High`): the page sets `mobile` on phones and `low` on request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Low,
    Mobile,
    High,
}

impl Tier {
    pub fn from_quality(q: Option<&str>) -> Self {
        match q.map(str::trim) {
            Some("low" | "0" | "off") => Tier::Low,
            Some("mobile") => Tier::Mobile,
            _ => Tier::High,
        }
    }
}

/// Run-time state of the three sections (not part of the spec).
#[derive(Clone, Debug, Default)]
pub struct RunFx {
    /// The wall PNG this tier uses, if it exists. None = plain walls.
    pub wall_texture: Option<String>,
    pub missions: crate::neon_missions::Tracker,
    /// The crash has happened; the game-over cue waits for the result.
    pub gameover_pending: bool,
    /// The finished mission's banner and seconds left.
    pub banner: String,
    pub banner_t: f32,
}

// ---------------------------------------------------------------- atmosphere

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AtmosphereSpec {
    /// Master switch for everything below.
    pub enabled: bool,
    pub rain: RainSpec,
    pub signs: SignsSpec,
    pub wall: WallSpec,
}

/// Falling rain: thin streaks (screen-space strokes), fixed in the world so
/// they stream past at running speed, plus a small ripple where one lands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RainSpec {
    pub enabled: bool,
    /// Streaks in view, per picture tier.
    pub count_low: u32,
    pub count_mobile: u32,
    pub count_high: u32,
    /// Metres ahead of the cat that carry rain.
    pub reach: f32,
    /// Metres behind the cat that carry rain.
    pub behind: f32,
    /// Fall speed, m/s.
    pub fall_speed: f32,
    /// Streak length, metres.
    pub length: f32,
    /// Sideways drift per metre of fall (wind).
    pub slant: f32,
    /// Stroke width in pixels.
    pub width: f32,
    /// RGBA of a streak.
    pub color: [u8; 4],
    /// A small ripple on the floor where a streak lands.
    pub splash: bool,
}

/// Neon signs on the tunnel walls: a few small outline drawings (cat, fish,
/// arrows, shop bars, a sign tower) with a dim glowing backing plate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SignsSpec {
    pub enabled: bool,
    /// Length of a sign cell along the track, metres. Each cell holds at most
    /// one sign (4 matches the wall's vertical rule).
    pub cell: f32,
    /// Chance a cell holds a sign, per picture tier (0..=1).
    pub density_low: f32,
    pub density_mobile: f32,
    pub density_high: f32,
    /// Sign colours (RGB). Chosen by hash. Keep them off the obstacle colour codes.
    pub palette: Vec<[u8; 3]>,
    /// Heights (metres) a sign's centre can sit at.
    pub rows: Vec<f32>,
    /// Sign width in metres (height is 60 % of it).
    pub size: f32,
    /// Stroke width in pixels.
    pub line_width: f32,
    /// Emissive strength of the backing plate and frame. 0 = strokes only.
    /// The low tier never draws the plate.
    pub glow: f32,
    /// Share of signs that flicker (0..=1).
    pub flicker: f32,
}

/// The side walls as a procedural concrete texture (`tools/gen_assets`).
/// Walls are tiled in `segment`-long boxes near the cat (each box is its own
/// texture slot) and plain black matte beyond `reach`, where fog hides them.
/// An empty or missing file keeps the old plain wall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WallSpec {
    pub enabled: bool,
    /// Texture per picture tier (repo-relative PNG). Empty = plain wall.
    pub texture_low: String,
    pub texture_mobile: String,
    pub texture_high: String,
    /// sRGB multiply on the texture (255 keeps it as drawn).
    pub tint: [u8; 3],
    /// Length of one textured wall box, metres.
    pub segment: f32,
    /// Metres ahead of the cat that get the texture.
    pub reach: f32,
}

impl Default for AtmosphereSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            rain: RainSpec::default(),
            signs: SignsSpec::default(),
            wall: WallSpec::default(),
        }
    }
}

impl Default for RainSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            count_low: 24,
            count_mobile: 56,
            count_high: 96,
            reach: 26.0,
            behind: 5.0,
            fall_speed: 11.0,
            length: 0.9,
            slant: 0.12,
            width: 1.1,
            color: [170, 205, 255, 125],
            splash: true,
        }
    }
}

impl Default for SignsSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            cell: 4.0,
            density_low: 0.2,
            density_mobile: 0.4,
            density_high: 0.55,
            palette: vec![
                [70, 235, 255],
                [60, 255, 170],
                [130, 110, 255],
                [255, 90, 200],
            ],
            rows: vec![3.5, 4.9],
            size: 1.7,
            line_width: 2.4,
            glow: 1.6,
            flicker: 0.3,
        }
    }
}

impl Default for WallSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            texture_low: String::new(),
            texture_mobile: "assets/textures/neon_cat/concrete_wall_s.png".into(),
            texture_high: "assets/textures/neon_cat/concrete_wall.png".into(),
            tint: [255, 255, 255],
            segment: 12.0,
            reach: 48.0,
        }
    }
}

impl RainSpec {
    pub fn count(&self, tier: Tier) -> u32 {
        match tier {
            Tier::Low => self.count_low,
            Tier::Mobile => self.count_mobile,
            Tier::High => self.count_high,
        }
    }
}

impl SignsSpec {
    pub fn density(&self, tier: Tier) -> f32 {
        match tier {
            Tier::Low => self.density_low,
            Tier::Mobile => self.density_mobile,
            Tier::High => self.density_high,
        }
    }
}

impl WallSpec {
    pub fn texture(&self, tier: Tier) -> &str {
        match tier {
            Tier::Low => &self.texture_low,
            Tier::Mobile => &self.texture_mobile,
            Tier::High => &self.texture_high,
        }
    }
}

/// A repo-relative asset path: no scheme, no `..`, not absolute.
fn plain_path(p: &str) -> bool {
    !(p.starts_with('/') || p.starts_with('\\') || p.contains("..") || p.contains(':'))
}

impl AtmosphereSpec {
    pub fn validate(&self) -> Result<(), String> {
        let r = &self.rain;
        for (name, n) in [
            ("count_low", r.count_low),
            ("count_mobile", r.count_mobile),
            ("count_high", r.count_high),
        ] {
            if n > 400 {
                return Err(format!("game spec: atmosphere.rain.{name} must be 0..=400"));
            }
        }
        let range = |v: f32, lo: f32, hi: f32| v.is_finite() && (lo..=hi).contains(&v);
        if !range(r.reach, 4.0, 80.0)
            || !range(r.behind, 0.0, 20.0)
            || !range(r.fall_speed, 1.0, 40.0)
            || !range(r.length, 0.1, 4.0)
            || !range(r.slant, -1.0, 1.0)
            || !range(r.width, 0.3, 6.0)
        {
            return Err("game spec: atmosphere.rain values are out of range".into());
        }
        let s = &self.signs;
        if !range(s.cell, 2.0, 16.0)
            || !range(s.size, 0.5, 2.4)
            || !range(s.line_width, 0.5, 8.0)
            || !range(s.glow, 0.0, 6.0)
            || !range(s.flicker, 0.0, 1.0)
            || ![s.density_low, s.density_mobile, s.density_high]
                .iter()
                .all(|d| range(*d, 0.0, 1.0))
        {
            return Err("game spec: atmosphere.signs values are out of range".into());
        }
        if s.enabled && (s.palette.is_empty() || s.palette.len() > 16) {
            return Err("game spec: atmosphere.signs.palette needs 1..=16 colours".into());
        }
        // The wall carries horizontal rules at 1.4 m steps; a sign has to sit
        // inside the wall, clear of the floor and the ceiling.
        let half = s.size * 0.3;
        if s.rows.len() > 6 || s.rows.iter().any(|y| !range(*y, 1.0 + half, 6.8 - half)) {
            return Err(
                "game spec: atmosphere.signs.rows must hold up to 6 heights inside the wall".into(),
            );
        }
        let w = &self.wall;
        if !range(w.segment, 2.0, 20.0) || !range(w.reach, 8.0, 100.0) {
            return Err("game spec: atmosphere.wall.segment 2..=20 and reach 8..=100".into());
        }
        // Each segment is a texture slot on the GPU; bound the count.
        if (w.reach / w.segment).ceil() as u32 > 16 {
            return Err("game spec: atmosphere.wall needs reach / segment <= 16".into());
        }
        for t in [&w.texture_low, &w.texture_mobile, &w.texture_high] {
            if !plain_path(t) || (!t.is_empty() && !t.to_ascii_lowercase().ends_with(".png")) {
                return Err(
                    "game spec: atmosphere.wall textures are repo-relative PNG paths".into(),
                );
            }
        }
        Ok(())
    }
}

// --------------------------------------------------------------------- audio

/// Audio files and volumes. Names in `sfx` are the sim's cues:
/// `jump roll coin card item meow perch knock robot_on robot_off hit gameover mission`
/// (`coin` = a coin landed, `card` = a gate passed, `item` = a power-up).
/// A cue with no entry is silent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AudioSpec {
    pub enabled: bool,
    /// 0..=1 gain on everything.
    pub master: f32,
    /// 0..=1 gain on the BGM group.
    pub bgm_volume: f32,
    /// 0..=1 gain on the SFX group.
    pub sfx_volume: f32,
    pub bgm: BgmSpec,
    pub sfx: BTreeMap<String, SfxSpec>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BgmSpec {
    /// Repo-relative `.wav` / `.ogg` / `.mp3`. Empty = no BGM.
    pub file: String,
    /// 0..=1, on top of the group volume.
    pub volume: f32,
    /// Fade in and out, seconds.
    pub fade_secs: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SfxSpec {
    /// Repo-relative `.wav` / `.ogg` / `.mp3`. Empty = silent.
    pub file: String,
    /// 0..=1, on top of the group volume.
    pub volume: f32,
    /// Playback rate (1 = as recorded; pitch moves with it).
    pub rate: f32,
}

/// The sim's SFX cue names. `AudioSpec` entries should be a subset.
pub const SFX_CUES: [&str; 13] = [
    "jump",
    "roll",
    "coin",
    "card",
    "item",
    "meow",
    "perch",
    "knock",
    "robot_on",
    "robot_off",
    "hit",
    "gameover",
    "mission",
];

pub const AUDIO_DIR: &str = "assets/audio/neon_cat";

impl Default for AudioSpec {
    fn default() -> Self {
        let vol = |name: &str, v: f32| {
            (
                name.to_string(),
                SfxSpec {
                    file: format!("{AUDIO_DIR}/{name}.wav"),
                    volume: v,
                    rate: 1.0,
                },
            )
        };
        Self {
            enabled: true,
            master: 0.8,
            bgm_volume: 0.7,
            sfx_volume: 1.0,
            bgm: BgmSpec::default(),
            sfx: SFX_CUES
                .iter()
                .map(|n| {
                    vol(
                        n,
                        match *n {
                            "coin" | "card" => 0.6,
                            "roll" => 0.5,
                            "knock" => 0.8,
                            _ => 0.9,
                        },
                    )
                })
                .collect(),
        }
    }
}

impl Default for BgmSpec {
    fn default() -> Self {
        Self {
            file: format!("{AUDIO_DIR}/bgm.wav"),
            volume: 0.55,
            fade_secs: 1.2,
        }
    }
}

impl Default for SfxSpec {
    fn default() -> Self {
        Self {
            file: String::new(),
            volume: 1.0,
            rate: 1.0,
        }
    }
}

fn audio_path_ok(p: &str) -> bool {
    if p.is_empty() {
        return true;
    }
    let l = p.to_ascii_lowercase();
    plain_path(p) && (l.ends_with(".wav") || l.ends_with(".ogg") || l.ends_with(".mp3"))
}

impl AudioSpec {
    pub fn validate(&self) -> Result<(), String> {
        let unit = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
        if !unit(self.master) || !unit(self.bgm_volume) || !unit(self.sfx_volume) {
            return Err("game spec: audio master, bgm_volume and sfx_volume must be 0..=1".into());
        }
        if !unit(self.bgm.volume) || !(0.0..=10.0).contains(&self.bgm.fade_secs) {
            return Err("game spec: audio.bgm volume 0..=1 and fade_secs 0..=10".into());
        }
        if !audio_path_ok(&self.bgm.file) {
            return Err(
                "game spec: audio.bgm.file must be a repo-relative .wav, .ogg or .mp3".into(),
            );
        }
        if self.sfx.len() > 32 {
            return Err("game spec: audio.sfx has at most 32 cues".into());
        }
        for (name, s) in &self.sfx {
            if name.is_empty() || name.len() > 32 {
                return Err("game spec: audio.sfx cue names are 1..=32 characters".into());
            }
            if !unit(s.volume) || !(0.25..=4.0).contains(&s.rate) {
                return Err(format!(
                    "game spec: audio.sfx.{name} volume must be 0..=1 and rate 0.25..=4"
                ));
            }
            if !audio_path_ok(&s.file) {
                return Err(format!(
                    "game spec: audio.sfx.{name}.file must be a repo-relative .wav, .ogg or .mp3"
                ));
            }
        }
        Ok(())
    }

    /// Every audio file the spec names, for bundling.
    pub fn files(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for f in std::iter::once(self.bgm.file.as_str())
            .chain(self.sfx.values().map(|s| s.file.as_str()))
        {
            if !f.is_empty() && !out.contains(&f) {
                out.push(f);
            }
        }
        out
    }
}

// ------------------------------------------------------------------ missions

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MissionsSpec {
    pub enabled: bool,
    /// The goals of every run. At most 5.
    pub list: Vec<MissionSpec>,
    /// Score added each time one is completed. 0 keeps scores comparable
    /// with runs that have no missions.
    pub bonus: u32,
    /// Draw the goals (and the best score) on the in-play HUD.
    pub hud: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MissionKind {
    /// Coins picked up this run.
    #[default]
    Coins,
    /// Stone arches run through this run.
    Gates,
    /// Different ledges landed on this run.
    Perch,
}

impl MissionKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Coins => "coins",
            Self::Gates => "gates",
            Self::Perch => "perch",
        }
    }

    pub fn default_label(self) -> &'static str {
        match self {
            Self::Coins => "COINS",
            Self::Gates => "GATES",
            Self::Perch => "PERCH",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MissionSpec {
    pub kind: MissionKind,
    /// How many to reach (1..=9999).
    pub target: u32,
    /// HUD wording. Empty uses the kind's own (`COINS`, `GATES`, `PERCH`).
    pub label: String,
}

impl Default for MissionSpec {
    fn default() -> Self {
        Self {
            kind: MissionKind::Coins,
            target: 1,
            label: String::new(),
        }
    }
}

impl MissionSpec {
    pub fn text(&self) -> &str {
        if self.label.is_empty() {
            self.kind.default_label()
        } else {
            &self.label
        }
    }
}

impl Default for MissionsSpec {
    fn default() -> Self {
        let m = |kind, target| MissionSpec {
            kind,
            target,
            label: String::new(),
        };
        Self {
            enabled: true,
            list: vec![
                m(MissionKind::Coins, 40),
                m(MissionKind::Gates, 5),
                m(MissionKind::Perch, 1),
            ],
            bonus: 0,
            hud: true,
        }
    }
}

impl MissionsSpec {
    pub fn validate(&self) -> Result<(), String> {
        if self.list.len() > 5 {
            return Err("game spec: missions.list has at most 5 goals".into());
        }
        if self.bonus > 100_000 {
            return Err("game spec: missions.bonus must be 0..=100000".into());
        }
        for m in &self.list {
            if !(1..=9999).contains(&m.target) {
                return Err("game spec: a mission target must be 1..=9999".into());
            }
            if m.label.chars().count() > 16 {
                return Err("game spec: a mission label has at most 16 characters".into());
            }
        }
        Ok(())
    }

    /// Switched off, as `GameSpec::legacy()` wants it.
    pub fn off() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }
}

impl AtmosphereSpec {
    pub fn off() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }
}

impl AudioSpec {
    pub fn off() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate_and_cover_every_cue() {
        AtmosphereSpec::default().validate().unwrap();
        AudioSpec::default().validate().unwrap();
        MissionsSpec::default().validate().unwrap();
        let a = AudioSpec::default();
        for cue in SFX_CUES {
            assert!(a.sfx.contains_key(cue), "no entry for {cue}");
        }
        for cue in [
            "knock",
            "coin",
            "perch",
            "robot_on",
            "robot_off",
            "jump",
            "hit",
            "item",
            "meow",
            "gameover",
        ] {
            assert!(SFX_CUES.contains(&cue));
        }
    }

    #[test]
    fn tiers_follow_quality() {
        assert_eq!(Tier::from_quality(None), Tier::High);
        assert_eq!(Tier::from_quality(Some("mobile")), Tier::Mobile);
        assert_eq!(Tier::from_quality(Some("low")), Tier::Low);
        assert_eq!(Tier::from_quality(Some("off")), Tier::Low);
        assert_eq!(Tier::from_quality(Some("high")), Tier::High);
        assert!(Tier::Low < Tier::Mobile && Tier::Mobile < Tier::High);
        let r = RainSpec::default();
        assert!(
            r.count(Tier::Low) < r.count(Tier::Mobile)
                && r.count(Tier::Mobile) < r.count(Tier::High)
        );
        let w = WallSpec::default();
        assert!(w.texture(Tier::Low).is_empty() && !w.texture(Tier::Mobile).is_empty());
    }

    #[test]
    fn bad_values_are_rejected() {
        let mut a = AtmosphereSpec::default();
        a.rain.count_high = 9999;
        assert!(a.validate().is_err());
        let mut a = AtmosphereSpec::default();
        a.wall.texture_high = "../secret.png".into();
        assert!(a.validate().is_err());
        let mut a = AtmosphereSpec::default();
        a.wall.segment = 2.0;
        a.wall.reach = 100.0;
        assert!(a.validate().is_err(), "too many texture slots");
        let mut a = AtmosphereSpec::default();
        a.signs.rows = vec![0.2];
        assert!(a.validate().is_err());
        let s = AudioSpec {
            master: 1.5,
            ..AudioSpec::default()
        };
        assert!(s.validate().is_err());
        let mut s = AudioSpec::default();
        s.bgm.file = "https://example.com/a.mp3".into();
        assert!(s.validate().is_err());
        let mut s = AudioSpec::default();
        s.sfx.get_mut("jump").unwrap().file = "assets/audio/x.flac".into();
        assert!(s.validate().is_err());
        let mut s = AudioSpec::default();
        s.sfx.get_mut("jump").unwrap().rate = 9.0;
        assert!(s.validate().is_err());
        let mut m = MissionsSpec::default();
        m.list[0].target = 0;
        assert!(m.validate().is_err());
        let m = MissionsSpec {
            list: vec![MissionSpec::default(); 6],
            ..MissionsSpec::default()
        };
        assert!(m.validate().is_err());
    }

    #[test]
    fn mp3_ogg_wav_and_silence_are_accepted() {
        let mut s = AudioSpec::default();
        for f in ["a.mp3", "a.OGG", "dir/a.wav", ""] {
            s.bgm.file = f.into();
            s.validate().unwrap();
        }
        assert!(!AudioSpec::default().files().is_empty());
        let mut s = AudioSpec::default();
        s.bgm.file.clear();
        s.sfx.clear();
        assert!(s.files().is_empty());
    }

    #[test]
    fn partial_json_keeps_the_other_defaults() {
        let a: AtmosphereSpec = serde_json::from_str(r#"{"rain":{"count_high":10}}"#).unwrap();
        assert_eq!(a.rain.count_high, 10);
        assert_eq!(a.rain.count_low, RainSpec::default().count_low);
        assert!(a.signs.enabled);
        assert!(serde_json::from_str::<AtmosphereSpec>(r#"{"rane":{}}"#).is_err());
        let m: MissionsSpec =
            serde_json::from_str(r#"{"list":[{"kind":"gates","target":2}]}"#).unwrap();
        assert_eq!(m.list.len(), 1);
        assert_eq!(m.list[0].text(), "GATES");
    }
}
