//! What a native shell plays this frame: files, volumes and rates.
//!
//! A kit only queues cue names (`mixer::cue_sfx("hit")`) and a BGM name. Which file a name
//! plays and how loud is the game's `audio` block in its world JSON, the same block the
//! browser page reads (`play.html`, `createAudio`). The page maps names to files in
//! JavaScript; a native shell (Android) gets the finished [`Plan`] from here and only has
//! to play it, so both sound the same and the mapping is tested once, in Rust.
//!
//! ```json
//! "audio": {
//!   "master": 0.8, "sfx_volume": 1.0, "bgm_volume": 0.5,
//!   "bgm": {"file": "assets/audio/x/theme.mp3", "volume": 0.45},
//!   "sfx": {"hit": {"file": "assets/audio/y/hit.wav", "volume": 0.6, "rate": 1.1}}
//! }
//! ```
//!
//! A game with no `audio` block keeps the old fixed clips (`assets/audio/<name>.wav`).

use serde::Serialize;
use serde_json::Value;

/// The mixer's own defaults ([`crate::mixer::Mixer`]). A player who never touched the
/// volume hears the game as its `audio` block says; the knobs scale from there.
const DEFAULT_MASTER: f32 = 0.8;
const DEFAULT_BGM: f32 = 0.7;
const DEFAULT_SFX: f32 = 1.0;

/// SoundPool plays between half and double speed.
const RATE_MIN: f32 = 0.5;
const RATE_MAX: f32 = 2.0;

/// The names the shells have always known (`mixer::clip_wav`).
const LEGACY_SFX: [&str; 3] = ["jump", "fire", "card"];

/// One sound to start now.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Shot {
    /// Path under the asset root.
    pub file: String,
    /// 0..1, the player's volume and the game's mix already applied.
    pub volume: f32,
    /// Playback speed (pitch), 0.5..2.
    pub rate: f32,
}

/// The looping music to hold at a volume.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Music {
    pub file: String,
    pub volume: f32,
}

/// What to play this frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Plan {
    /// `None`: no music (stopped, faded out or muted).
    pub bgm: Option<Music>,
    pub shots: Vec<Shot>,
}

/// The mixer's state for one frame (`mixer::cues_json`).
#[derive(Clone, Debug, PartialEq)]
pub struct Cues {
    pub muted: bool,
    pub master: f32,
    pub bgm: f32,
    pub sfx: f32,
    pub bgm_name: String,
    pub bgm_gain: f32,
    pub oneshots: Vec<String>,
}

impl Cues {
    /// From the JSON of [`crate::mixer::cues_json`].
    pub fn parse(json: &str) -> Option<Self> {
        let v: Value = serde_json::from_str(json).ok()?;
        let f = |k: &str, d: f32| v.get(k).and_then(Value::as_f64).map_or(d, |x| x as f32);
        Some(Self {
            muted: v.get("muted").and_then(Value::as_bool).unwrap_or(false),
            master: f("master", DEFAULT_MASTER),
            bgm: f("bgm", DEFAULT_BGM),
            sfx: f("sfx", DEFAULT_SFX),
            bgm_name: v
                .get("bgm_name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            bgm_gain: f("bgm_gain", 0.0),
            oneshots: v
                .get("oneshots")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }
}

fn unit(v: Option<&Value>, default: f32) -> f32 {
    v.and_then(Value::as_f64)
        .filter(|x| x.is_finite())
        .map_or(default, |x| x as f32)
        .clamp(0.0, 1.0)
}

fn file_of(v: Option<&Value>) -> Option<&str> {
    v.and_then(|c| c.get("file"))
        .and_then(Value::as_str)
        .filter(|f| safe_path(f))
}

/// A path that stays under the asset root.
fn safe_path(f: &str) -> bool {
    !f.is_empty() && !f.starts_with('/') && !f.contains('\\') && !f.split('/').any(|p| p == "..")
}

/// The game's `audio` block from its world JSON, if it has one and has not turned it off.
pub fn spec_of(world: &Value) -> Option<Value> {
    let a = world.get("game")?.get("audio")?;
    (a.is_object() && a.get("enabled").and_then(Value::as_bool) != Some(false)).then(|| a.clone())
}

/// The one-shot files to load before the first cue (the music is streamed, not loaded).
/// No two the same.
pub fn sfx_files(spec: Option<&Value>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    match spec {
        Some(spec) => {
            if let Some(map) = spec.get("sfx").and_then(Value::as_object) {
                for cue in map.values() {
                    if let Some(f) = file_of(Some(cue)) {
                        if !out.iter().any(|x| x == f) {
                            out.push(f.to_string());
                        }
                    }
                }
            }
        }
        None => out.extend(LEGACY_SFX.iter().map(|n| format!("assets/audio/{n}.wav"))),
    }
    out
}

/// What the shell plays for these cues.
pub fn plan(spec: Option<&Value>, cues: &Cues) -> Plan {
    if cues.muted {
        return Plan::default();
    }
    let Some(spec) = spec else {
        return legacy(cues);
    };
    // The player's knobs, as a share of their defaults.
    let knob = |v: f32, default: f32| (v / default).clamp(0.0, 1.0 / default);
    let master = unit(spec.get("master"), 0.8) * knob(cues.master, DEFAULT_MASTER);
    let sfx_mix = master * unit(spec.get("sfx_volume"), 1.0) * knob(cues.sfx, DEFAULT_SFX);
    let mut shots: Vec<Shot> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    if sfx_mix > 0.0 {
        for name in &cues.oneshots {
            // The same cue twice in one frame is one sound (a burst of coins).
            if seen.contains(&name.as_str()) {
                continue;
            }
            seen.push(name);
            let cue = spec.get("sfx").and_then(|m| m.get(name.as_str()));
            let Some(file) = file_of(cue) else { continue };
            let volume = (sfx_mix * unit(cue.and_then(|c| c.get("volume")), 1.0)).clamp(0.0, 1.0);
            if volume <= 0.0 {
                continue;
            }
            let rate = cue
                .and_then(|c| c.get("rate"))
                .and_then(Value::as_f64)
                .filter(|r| r.is_finite())
                .map_or(1.0, |r| r as f32)
                .clamp(RATE_MIN, RATE_MAX);
            shots.push(Shot {
                file: file.to_string(),
                volume,
                rate,
            });
        }
    }
    let bgm_spec = spec.get("bgm");
    let bgm = file_of(bgm_spec)
        .filter(|_| !cues.bgm_name.is_empty() && cues.bgm_gain > 0.001)
        .map(|file| Music {
            file: file.to_string(),
            volume: (master
                * unit(spec.get("bgm_volume"), 0.7)
                * unit(bgm_spec.and_then(|b| b.get("volume")), 0.55)
                * knob(cues.bgm, DEFAULT_BGM)
                * cues.bgm_gain.clamp(0.0, 1.0))
            .clamp(0.0, 1.0),
        })
        .filter(|m| m.volume > 0.0);
    Plan { bgm, shots }
}

/// A game with no `audio` block: the fixed clips by name, as the shells always played them.
fn legacy(cues: &Cues) -> Plan {
    let sfx = (cues.master * cues.sfx).clamp(0.0, 1.0);
    let shots = if sfx > 0.0 {
        cues.oneshots
            .iter()
            .filter(|n| LEGACY_SFX.contains(&n.as_str()))
            .map(|n| Shot {
                file: format!("assets/audio/{n}.wav"),
                volume: sfx,
                rate: 1.0,
            })
            .collect()
    } else {
        Vec::new()
    };
    let volume = (cues.master * cues.bgm * cues.bgm_gain).clamp(0.0, 1.0);
    let name_ok = !cues.bgm_name.is_empty()
        && cues
            .bgm_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let bgm = (name_ok && volume > 0.01).then(|| Music {
        file: format!("assets/audio/{}.wav", cues.bgm_name),
        volume,
    });
    Plan { bgm, shots }
}

/// [`plan`] as the JSON a shell reads: `{"bgm": {"file", "volume"} | null, "shots": [...]}`.
pub fn plan_json(spec: Option<&Value>, cues_json: &str) -> String {
    let plan = Cues::parse(cues_json)
        .map(|c| plan(spec, &c))
        .unwrap_or_default();
    serde_json::to_string(&plan).unwrap_or_else(|_| r#"{"bgm":null,"shots":[]}"#.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cues(shots: &[&str]) -> Cues {
        Cues {
            muted: false,
            master: DEFAULT_MASTER,
            bgm: DEFAULT_BGM,
            sfx: DEFAULT_SFX,
            bgm_name: "bgm".into(),
            bgm_gain: 1.0,
            oneshots: shots.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn spec() -> Value {
        json!({
            "enabled": true,
            "master": 0.8,
            "sfx_volume": 1.0,
            "bgm_volume": 0.5,
            "bgm": {"file": "assets/audio/a/theme.mp3", "volume": 0.45, "fade_secs": 0.5},
            "sfx": {
                "hit": {"file": "assets/audio/b/hit.wav", "volume": 0.6, "rate": 1.1},
                "card": {"file": "assets/audio/c/ui.wav", "volume": 0.4, "rate": 1.3},
                "step": {"file": "assets/audio/c/ui.wav", "volume": 0.2, "rate": 1.8},
                "deep": {"file": "assets/audio/b/hit.wav", "volume": 1.0, "rate": 0.2},
                "bad": {"file": "../secret.wav"},
                "nofile": {"volume": 1.0}
            }
        })
    }

    /// At the player's default volume the numbers are the page's: master x group x cue.
    #[test]
    fn a_cue_plays_its_file_at_the_pages_volume_and_rate() {
        let s = spec();
        let p = plan(Some(&s), &cues(&["hit", "card"]));
        assert_eq!(p.shots.len(), 2);
        assert_eq!(p.shots[0].file, "assets/audio/b/hit.wav");
        assert!((p.shots[0].volume - 0.8 * 1.0 * 0.6).abs() < 1e-5);
        assert!((p.shots[0].rate - 1.1).abs() < 1e-5);
        assert!((p.shots[1].volume - 0.8 * 0.4).abs() < 1e-5);
        let m = p.bgm.unwrap();
        assert_eq!(m.file, "assets/audio/a/theme.mp3");
        assert!((m.volume - 0.8 * 0.5 * 0.45).abs() < 1e-5);
    }

    #[test]
    fn unknown_unsafe_and_repeated_cues_play_nothing_extra() {
        let s = spec();
        let p = plan(
            Some(&s),
            &cues(&["hit", "hit", "nope", "bad", "nofile", "deep"]),
        );
        let files: Vec<_> = p.shots.iter().map(|x| x.file.as_str()).collect();
        assert_eq!(files, ["assets/audio/b/hit.wav", "assets/audio/b/hit.wav"]);
        // The second is "deep": its rate is held to what the device can play.
        assert_eq!(p.shots[1].rate, RATE_MIN);
        for shot in &p.shots {
            assert!((0.0..=1.0).contains(&shot.volume));
        }
    }

    #[test]
    fn the_players_knobs_scale_and_mute_silences() {
        let s = spec();
        let mut c = cues(&["hit"]);
        c.master = DEFAULT_MASTER * 0.5;
        let half = plan(Some(&s), &c);
        assert!((half.shots[0].volume - 0.8 * 0.6 * 0.5).abs() < 1e-5);
        assert!((half.bgm.unwrap().volume - 0.8 * 0.5 * 0.45 * 0.5).abs() < 1e-5);
        c.master = DEFAULT_MASTER;
        c.sfx = 0.0;
        let quiet = plan(Some(&s), &c);
        assert!(quiet.shots.is_empty());
        assert!(quiet.bgm.is_some(), "the music has its own knob");
        c.muted = true;
        assert_eq!(plan(Some(&s), &c), Plan::default());
    }

    #[test]
    fn music_follows_the_mixers_name_and_fade() {
        let s = spec();
        let mut c = cues(&[]);
        c.bgm_gain = 0.5;
        assert!((plan(Some(&s), &c).bgm.unwrap().volume - 0.8 * 0.5 * 0.45 * 0.5).abs() < 1e-5);
        c.bgm_gain = 0.0;
        assert!(plan(Some(&s), &c).bgm.is_none(), "faded out");
        c.bgm_gain = 1.0;
        c.bgm_name.clear();
        assert!(plan(Some(&s), &c).bgm.is_none(), "stopped");
    }

    #[test]
    fn files_to_load_are_the_one_shots_once_each() {
        let s = spec();
        let mut files = sfx_files(Some(&s));
        files.sort();
        assert_eq!(files, ["assets/audio/b/hit.wav", "assets/audio/c/ui.wav"]);
        assert_eq!(
            sfx_files(None),
            [
                "assets/audio/jump.wav",
                "assets/audio/fire.wav",
                "assets/audio/card.wav"
            ]
        );
    }

    #[test]
    fn a_game_with_no_block_keeps_the_fixed_clips() {
        let p = plan(None, &cues(&["jump", "hit", "card"]));
        let files: Vec<_> = p.shots.iter().map(|x| x.file.as_str()).collect();
        assert_eq!(files, ["assets/audio/jump.wav", "assets/audio/card.wav"]);
        assert_eq!(p.bgm.unwrap().file, "assets/audio/bgm.wav");
        let mut c = cues(&[]);
        c.bgm_name = "../x".into();
        assert!(plan(None, &c).bgm.is_none());
    }

    #[test]
    fn the_block_is_read_from_the_world_and_can_be_off() {
        let on = json!({"game": {"audio": spec()}});
        assert!(spec_of(&on).is_some());
        let off = json!({"game": {"audio": {"enabled": false}}});
        assert!(spec_of(&off).is_none());
        assert!(spec_of(&json!({"game": {}})).is_none());
    }

    #[test]
    fn the_json_a_shell_reads() {
        let s = spec();
        let cues = json!({
            "muted": false, "master": 0.8, "bgm": 0.7, "sfx": 1.0,
            "bgm_name": "bgm", "bgm_gain": 1.0, "oneshots": ["hit"]
        })
        .to_string();
        let v: Value = serde_json::from_str(&plan_json(Some(&s), &cues)).unwrap();
        assert_eq!(v["shots"][0]["file"], "assets/audio/b/hit.wav");
        assert_eq!(v["bgm"]["file"], "assets/audio/a/theme.mp3");
        let silent: Value = serde_json::from_str(&plan_json(Some(&s), "not json")).unwrap();
        assert!(silent["bgm"].is_null());
        assert_eq!(silent["shots"].as_array().unwrap().len(), 0);
    }

    /// Every shipped kit's own block gives files that exist in the repo.
    #[test]
    fn the_shipped_kits_name_files_that_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut checked = 0;
        for dump in [
            crate::diamond_days::DiamondDays::new(Default::default())
                .world_value()
                .to_string(),
            crate::crate_tower::CrateTower::new(Default::default())
                .world_value()
                .to_string(),
        ] {
            let world: Value = serde_json::from_str(&dump).unwrap();
            let Some(spec) = spec_of(&world) else {
                continue;
            };
            let files = sfx_files(Some(&spec));
            assert!(!files.is_empty());
            for f in files {
                assert!(root.join(&f).is_file(), "{f}");
                checked += 1;
            }
            if let Some(f) = file_of(spec.get("bgm")) {
                assert!(root.join(f).is_file(), "{f}");
            }
        }
        assert!(checked >= 4);
    }
}
