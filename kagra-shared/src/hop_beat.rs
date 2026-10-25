//! HOP-BEAT: a one-button rhythm hopper. The world is a row of stone slabs
//! laid out on the music's beat grid; the hero runs at one beat-length per
//! beat and hops the gaps and low blocks. A tap is a short hop (one beat in
//! the air), a hold is a long one (two beats). Pressing on the beat is a
//! PERFECT and builds the combo.
//!
//! The beat grid lives in the spec (`music.bpm`). The hero's x position is
//! `song_t * speed`, and `song_t` follows the page's audio clock when the page
//! reports one ([`HopBeat::set_music_clock`]), so the picture stays on the
//! music whatever the frame rate. Without a clock (tests, the playtest bot,
//! a muted tab) the sim runs on its own fixed steps.
//!
//! Everything a designer tunes is in [`HopSpec`] (JSON `game` of the dump).
//! The Python mirror is `kagra/hop_beat.py`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::flow::{FlowAction, FlowPhase, GameFlow};
use crate::game_core::{tick_frame, Core, Kit};
use crate::game_spec::LivesSpec;
use crate::neon_fx_spec::{BgmSpec, SfxSpec};
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc, WorldLight, WorldProp};

pub const GAME_ID: &str = "hop_beat";
/// The sim's own step when the page does not run it (and the harness).
pub const STEP: f32 = 1.0 / 60.0;
/// How much course is laid ahead of the hero, in metres.
const AHEAD_M: f32 = 44.0;
/// How much stays drawn behind the hero.
const BEHIND_M: f32 = 14.0;

pub const SFX_CUES: [&str; 12] = [
    "hop", "land", "perfect", "good", "combo", "hit", "fall", "stomp", "warn", "gameover",
    "revive", "newbest",
];

/// Loudness of each cue under the music (the music file is about -14 LUFS at 0.7):
/// the ones that repeat every beat sit low, the rewards and the damage stand out.
fn cue_volume(name: &str) -> f32 {
    match name {
        "hop" => 0.6,
        "land" => 0.4,
        "good" => 0.65,
        "perfect" => 0.85,
        "combo" => 0.8,
        "warn" => 0.55,
        "hit" => 0.95,
        _ => 0.9,
    }
}

/// Cues that are long music stingers (mp3 from the music pack); the rest are
/// short generated effects (wav).
pub const STINGERS: [&str; 3] = ["gameover", "revive", "newbest"];

pub fn is_hop_dump(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| {
            v.get("genre")
                .and_then(|g| g.as_str())
                .map(|s| s == GAME_ID)
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------- spec ----

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MusicSpec {
    /// Beats per minute of the music file (and of every gap in the course).
    pub bpm: f32,
    pub beats_per_bar: u32,
    /// Latency of the music file itself: positive when the first beat sits
    /// this many ms after the file starts (a silent lead-in), or to push the
    /// picture later. The player's own setting is added on top.
    pub offset_ms: f32,
    /// Bars of plain slab before the first obstacle.
    pub lead_in_bars: u32,
    /// Length of the BGM loop in bars (the loop file must be this long).
    pub loop_bars: u32,
}

impl Default for MusicSpec {
    fn default() -> Self {
        Self {
            bpm: 126.0,
            beats_per_bar: 4,
            offset_ms: 0.0,
            lead_in_bars: 2,
            loop_bars: 32,
        }
    }
}

/// One hop: how many beats it stays in the air and how high it goes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HopArc {
    pub air_beats: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HeroSpec {
    /// Metres the hero runs per beat. Speed follows the BPM.
    pub beat_len_m: f32,
    /// Press and keep holding.
    pub hold: HopArc,
    /// Press and let go within `tap_window_secs`.
    pub tap: HopArc,
    pub tap_window_secs: f32,
    /// A press this soon after running off an edge still hops.
    pub coyote_secs: f32,
    /// A press this soon before landing hops on landing.
    pub buffer_secs: f32,
    pub half_width: f32,
    /// Slack at a slab's edge (metres) before the hero counts as off it.
    pub edge_tol: f32,
    pub fall_gravity: f32,
}

impl Default for HeroSpec {
    fn default() -> Self {
        Self {
            beat_len_m: 3.0,
            hold: HopArc {
                air_beats: 2.0,
                height: 1.6,
            },
            tap: HopArc {
                air_beats: 1.0,
                height: 0.9,
            },
            tap_window_secs: 0.16,
            coyote_secs: 0.09,
            buffer_secs: 0.12,
            half_width: 0.24,
            edge_tol: 0.35,
            fall_gravity: 28.0,
        }
    }
}
impl Default for HopArc {
    fn default() -> Self {
        Self {
            air_beats: 1.0,
            height: 1.0,
        }
    }
}

/// One obstacle of a phrase. `beat` is when the player should press (the
/// ideal takeoff); the obstacle is centred under the arc of `hop`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub beat: f32,
    /// `gap`, `block` or `enemy`.
    pub kind: String,
    /// Width in beats.
    pub beats: f32,
    /// `tap` or `hold`: the hop this obstacle is written for.
    pub hop: String,
    /// For `kind: "enemy"`: the id of an entry of `enemies.types`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub enemy: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phrase {
    /// 0 = the first bars of a run, higher is harder.
    pub level: u32,
    pub bars: u32,
    pub items: Vec<Item>,
    /// How often it is picked among the phrases of its level (1..=16).
    #[serde(default = "one_weight")]
    pub weight: u32,
    /// The first phrase laid when a level starts is picked among the `debut`
    /// phrases of that level (if it has any), so a new enemy is introduced on
    /// its own.
    #[serde(default, skip_serializing_if = "is_false")]
    pub debut: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn one_weight() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CourseSpec {
    pub seed: u32,
    /// Bars until the hardest level is reached.
    pub ramp_bars: u32,
    /// A plain bar after this many bars of phrases.
    pub rest_every_bars: u32,
    pub block_height: f32,
    /// Every obstacle must leave at least this much timing window (ms, total
    /// width) or the spec is refused.
    pub min_window_ms: f32,
    /// Empty = the built-in phrase set.
    pub phrases: Vec<Phrase>,
}

impl Default for CourseSpec {
    fn default() -> Self {
        Self {
            seed: 7,
            ramp_bars: 40,
            rest_every_bars: 8,
            block_height: 0.5,
            min_window_ms: 110.0,
            phrases: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JudgeSpec {
    /// Press within this of the ideal beat: PERFECT.
    pub perfect_ms: f32,
    pub good_ms: f32,
    pub combo_cap: u32,
    pub score_perfect: u32,
    pub score_good: u32,
    pub score_ok: u32,
    /// Added per combo step (up to `combo_cap`) to a PERFECT.
    pub combo_bonus: u32,
    /// Points per beat survived.
    pub per_beat: f32,
    /// A life comes back after this many clears in a row without damage
    /// (never above `lives.count`). 0 = never.
    pub regain_clears: u32,
}

impl Default for JudgeSpec {
    fn default() -> Self {
        Self {
            perfect_ms: 70.0,
            good_ms: 130.0,
            combo_cap: 8,
            score_perfect: 100,
            score_good: 60,
            score_ok: 30,
            combo_bonus: 25,
            per_beat: 2.0,
            regain_clears: 12,
        }
    }
}

/// Continue after game over (a rewarded ad, or any other reason).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReviveSpec {
    pub enabled: bool,
    pub max_per_run: u32,
    /// Lives given back.
    pub lives: u32,
    pub invuln_secs: f32,
    /// Beats of empty slab laid after the restart bar.
    pub clear_beats: f32,
}

impl Default for ReviveSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            max_per_run: 1,
            lives: 2,
            invuln_secs: 2.0,
            clear_beats: 4.0,
        }
    }
}

/// The breather after a fall that costs a life, and after a Continue. The drop plays out,
/// the world holds for a few beats of READY, the music restarts on a bar line, and the
/// bunny blinks (cannot be hurt) for a while once play resumes. All of it is timing and
/// picture on the way back in; the sim before the fall is not touched.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RecoverSpec {
    /// Off = the old instant second wind (and an instant Continue).
    pub enabled: bool,
    /// After a fall that costs a life: seconds the drop plays out (slowed) before READY.
    pub drop_secs: f32,
    /// Beats of READY with the world held, after such a fall and after a Continue.
    pub ready_beats: f32,
    /// Seconds the bunny blinks and cannot be hurt once play resumes after a fall.
    pub invuln_secs: f32,
    /// Beats of empty slab laid ahead of the bunny where it comes back.
    pub clear_beats: f32,
}

impl Default for RecoverSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            drop_secs: 0.9,
            ready_beats: 3.0,
            invuln_secs: 2.4,
            clear_beats: 4.0,
        }
    }
}

/// Where the way back in is. `Drop`: the fall plays on. `Ready`: the world holds and the
/// beats count down; the music starts again when it reaches 0.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RecoverStage {
    Drop,
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Recover {
    stage: RecoverStage,
    /// Seconds left in this stage.
    left: f32,
    /// Seconds the READY stage lasts in all (0 during the drop).
    total: f32,
    /// Countdown beats already ticked.
    ticked: i32,
    /// Protection to give when play resumes.
    invuln: f32,
}

/// How a fall into a gap looks. All of it is picture and timing: the sim decides the
/// fall exactly as before, this only shapes what is drawn and how long the game over
/// card waits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FallSpec {
    /// Picture hold at the ledge (hit-stop; the sim and the music keep going).
    pub edge_stop_secs: f32,
    /// Somersaults per second once it is off the ledge.
    pub tumble_turns_per_sec: f32,
    /// Stretch along the fall at full speed (0 = none).
    pub stretch: f32,
    /// Share of the forward speed the bunny keeps while it drops out of frame.
    pub carry: f32,
    /// Share of its forward speed the camera does NOT follow (0 = rigid, 1 = stays put).
    pub cam_lag: f32,
    /// Time scale of the drop after the run has ended.
    pub slow_scale: f32,
    /// From the ledge to the game over card.
    pub card_delay_secs: f32,
    /// Strength of the dark screen edge (0 = off).
    pub vignette: f32,
    /// Dust puffs at the ledge (halved on the low tier).
    pub dust: u32,
}

impl Default for FallSpec {
    fn default() -> Self {
        Self {
            edge_stop_secs: 0.07,
            tumble_turns_per_sec: 1.1,
            stretch: 0.3,
            carry: 0.55,
            cam_lag: 0.6,
            slow_scale: 0.5,
            card_delay_secs: 1.0,
            vignette: 0.7,
            dust: 8,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlaySpec {
    /// Land in the run itself: no title card, the first tap starts the music.
    pub direct: bool,
}

use crate::game_core::enemies::{self as foes, HeroBox, MAX_WALK_M};
pub use crate::game_core::enemies::{EnemySpec, EnemyType, Touch};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LookSpec {
    pub hero: [u32; 3],
    pub slab: [u32; 3],
    pub glow: [u32; 3],
    pub accent: [u32; 3],
    /// The finishing bundle: one of [`LOOK_PRESETS`]. `post` and `atmo` override single values.
    pub preset: String,
    pub post: PostSpec,
    pub atmo: AtmoSpec,
}

/// Hero skins by id: a body colour laid over the look's own hero colour.
/// `classic` leaves `look.hero` alone, so a custom look keeps working.
pub const DEFAULT_SKIN: &str = "classic";
pub const SKINS: [(&str, Option<[u32; 3]>); 5] = [
    ("classic", None),
    ("mint", Some([140, 255, 205])),
    ("gold", Some([255, 205, 90])),
    ("ember", Some([255, 120, 90])),
    ("ghost", Some([220, 226, 255])),
];

/// Look presets other games can reuse by name.
pub const LOOK_PRESETS: [&str; 3] = ["neon_night", "dusk", "plain"];
pub const LUT_NAMES: [&str; 4] = ["off", "warm", "cool", "punch"];

/// Post-process overrides (the renderer's bloom, vignette, grade, tone map). `None` = the preset's value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PostSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bloom: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bloom_threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vignette: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lut: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lut_strength: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exposure: Option<f32>,
    /// `agx`, `neutral` or empty for the default ACES curve.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tonemap_curve: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ibl: Option<f32>,
}

/// Sky, haze and far-scenery overrides. `None` = the preset's value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AtmoSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sky: Option<[u32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fog_start: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fog_end: Option<f32>,
    /// Twinkling stars at the quality tier's full count (low draws a third).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stars: Option<u32>,
    /// Slow far clouds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clouds: Option<u32>,
    /// The glow band on the horizon (colour and strength).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub horizon: Option<[u32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub horizon_glow: Option<f32>,
    /// How much the tower windows flicker (0 = steady).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twinkle: Option<f32>,
}

/// A look with every value filled in.
#[derive(Clone, Debug)]
pub struct Finish {
    pub bloom: f32,
    pub bloom_threshold: f32,
    pub vignette: f32,
    pub lut: String,
    pub lut_strength: f32,
    pub exposure: f32,
    pub tonemap_curve: String,
    pub ibl: f32,
    pub sky: [u32; 3],
    pub hemi_sky: [f32; 3],
    pub hemi_ground: [f32; 3],
    pub fog_start: f32,
    pub fog_end: f32,
    pub stars: u32,
    pub clouds: u32,
    pub horizon: [u32; 3],
    pub horizon_glow: f32,
    pub twinkle: f32,
}

impl LookSpec {
    /// The preset's values with `post` and `atmo` laid over them.
    pub fn finish(&self) -> Finish {
        let mut f = match self.preset.as_str() {
            "dusk" => Finish {
                bloom: 0.2,
                bloom_threshold: 1.3,
                vignette: 0.55,
                lut: "warm".into(),
                lut_strength: 0.25,
                exposure: 0.95,
                tonemap_curve: String::new(),
                ibl: 0.3,
                sky: [40, 16, 58],
                hemi_sky: [0.34, 0.2, 0.4],
                hemi_ground: [0.14, 0.06, 0.12],
                fog_start: 30.0,
                fog_end: 105.0,
                stars: 16,
                clouds: 6,
                horizon: [255, 120, 90],
                horizon_glow: 2.0,
                twinkle: 0.5,
            },
            "plain" => Finish {
                bloom: 0.0,
                bloom_threshold: 1.4,
                vignette: 0.0,
                lut: "off".into(),
                lut_strength: 0.0,
                exposure: 1.0,
                tonemap_curve: String::new(),
                ibl: 0.3,
                sky: [14, 10, 40],
                hemi_sky: [0.20, 0.16, 0.42],
                hemi_ground: [0.10, 0.06, 0.14],
                fog_start: 34.0,
                fog_end: 92.0,
                stars: 0,
                clouds: 0,
                horizon: [0, 0, 0],
                horizon_glow: 0.0,
                twinkle: 0.0,
            },
            _ => Finish {
                bloom: 0.16,
                bloom_threshold: 1.4,
                vignette: 0.6,
                lut: "cool".into(),
                lut_strength: 0.2,
                exposure: 0.9,
                tonemap_curve: String::new(),
                ibl: 0.3,
                sky: [14, 10, 40],
                hemi_sky: [0.20, 0.16, 0.42],
                hemi_ground: [0.10, 0.06, 0.14],
                fog_start: 34.0,
                fog_end: 110.0,
                stars: 40,
                clouds: 4,
                horizon: [150, 50, 170],
                horizon_glow: 2.4,
                twinkle: 0.6,
            },
        };
        let (p, a) = (&self.post, &self.atmo);
        f.bloom = p.bloom.unwrap_or(f.bloom);
        f.bloom_threshold = p.bloom_threshold.unwrap_or(f.bloom_threshold);
        f.vignette = p.vignette.unwrap_or(f.vignette);
        if let Some(l) = &p.lut {
            f.lut = l.clone();
        }
        f.lut_strength = p.lut_strength.unwrap_or(f.lut_strength);
        f.exposure = p.exposure.unwrap_or(f.exposure);
        if let Some(c) = &p.tonemap_curve {
            f.tonemap_curve = c.clone();
        }
        f.ibl = p.ibl.unwrap_or(f.ibl);
        f.sky = a.sky.unwrap_or(f.sky);
        f.fog_start = a.fog_start.unwrap_or(f.fog_start);
        f.fog_end = a.fog_end.unwrap_or(f.fog_end);
        f.stars = a.stars.unwrap_or(f.stars);
        f.clouds = a.clouds.unwrap_or(f.clouds);
        f.horizon = a.horizon.unwrap_or(f.horizon);
        f.horizon_glow = a.horizon_glow.unwrap_or(f.horizon_glow);
        f.twinkle = a.twinkle.unwrap_or(f.twinkle);
        f
    }

    pub fn validate(&self) -> Result<(), String> {
        if !LOOK_PRESETS.contains(&self.preset.as_str()) {
            return Err(format!(
                "hop spec: look.preset is {}",
                LOOK_PRESETS.join("|")
            ));
        }
        let f = self.finish();
        finite_in("look.post.bloom", f.bloom, 0.0, 2.0)?;
        finite_in("look.post.bloom_threshold", f.bloom_threshold, 0.0, 2.0)?;
        finite_in("look.post.vignette", f.vignette, 0.0, 2.0)?;
        finite_in("look.post.lut_strength", f.lut_strength, 0.0, 1.0)?;
        finite_in("look.post.exposure", f.exposure, 0.3, 2.0)?;
        finite_in("look.post.ibl", f.ibl, 0.0, 2.0)?;
        if !LUT_NAMES.contains(&f.lut.as_str()) {
            return Err(format!(
                "hop spec: look.post.lut is {}",
                LUT_NAMES.join("|")
            ));
        }
        if !["", "agx", "neutral"].contains(&f.tonemap_curve.as_str()) {
            return Err("hop spec: look.post.tonemap_curve is agx|neutral or empty".into());
        }
        finite_in("look.atmo.fog_start", f.fog_start, 0.0, 400.0)?;
        finite_in("look.atmo.fog_end", f.fog_end, 1.0, 400.0)?;
        if f.fog_end <= f.fog_start {
            return Err("hop spec: look.atmo.fog_end must be above fog_start".into());
        }
        finite_in("look.atmo.horizon_glow", f.horizon_glow, 0.0, 4.0)?;
        finite_in("look.atmo.twinkle", f.twinkle, 0.0, 2.0)?;
        if f.stars > 200 || f.clouds > 12 {
            return Err("hop spec: look.atmo.stars 0..=200, clouds 0..=12".into());
        }
        for c in [f.sky, f.horizon] {
            if c.iter().any(|v| *v > 255) {
                return Err("hop spec: look colours are 0..=255".into());
            }
        }
        Ok(())
    }
}

impl Default for LookSpec {
    fn default() -> Self {
        Self {
            hero: [255, 214, 120],
            slab: [58, 52, 92],
            glow: [96, 232, 255],
            accent: [255, 92, 170],
            preset: "neon_night".into(),
            post: PostSpec::default(),
            atmo: AtmoSpec::default(),
        }
    }
}

/// Same shape as Neon Cat's `audio`, so the page plays both the same way.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HopAudio {
    pub enabled: bool,
    pub master: f32,
    pub bgm_volume: f32,
    pub sfx_volume: f32,
    /// The music. `file` is repo-relative; drop a real track in and set
    /// `music.bpm` / `music.offset_ms` to match.
    pub bgm: BgmSpec,
    pub sfx: BTreeMap<String, SfxSpec>,
}

pub const AUDIO_DIR: &str = "assets/audio/hop_beat";

impl Default for HopAudio {
    fn default() -> Self {
        Self {
            enabled: true,
            master: 0.8,
            bgm_volume: 0.7,
            sfx_volume: 1.0,
            bgm: BgmSpec {
                file: format!("{AUDIO_DIR}/hop_beat_loop.mp3"),
                volume: 0.8,
                fade_secs: 0.3,
            },
            sfx: SFX_CUES
                .iter()
                .map(|n| {
                    (
                        n.to_string(),
                        SfxSpec {
                            file: if STINGERS.contains(n) {
                                format!("{AUDIO_DIR}/hop_beat_{n}.mp3")
                            } else {
                                format!("{AUDIO_DIR}/{n}.wav")
                            },
                            volume: cue_volume(n),
                            rate: 1.0,
                        },
                    )
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HopSpec {
    pub id: String,
    pub title: String,
    pub music: MusicSpec,
    pub hero: HeroSpec,
    pub course: CourseSpec,
    pub judge: JudgeSpec,
    pub lives: LivesSpec,
    pub revive: ReviveSpec,
    pub recover: RecoverSpec,
    pub fall: FallSpec,
    pub play: PlaySpec,
    pub enemies: EnemySpec,
    pub look: LookSpec,
    /// The hero's skin: one of [`SKINS`]. The unlock system switches this (and `look.preset`).
    pub skin: String,
    pub audio: HopAudio,
}

impl Default for HopSpec {
    fn default() -> Self {
        Self {
            id: GAME_ID.into(),
            title: "Hop Beat".into(),
            music: MusicSpec::default(),
            hero: HeroSpec::default(),
            course: CourseSpec::default(),
            judge: JudgeSpec::default(),
            lives: LivesSpec::default(),
            revive: ReviveSpec::default(),
            recover: RecoverSpec::default(),
            fall: FallSpec::default(),
            play: PlaySpec::default(),
            enemies: EnemySpec::default(),
            look: LookSpec::default(),
            skin: DEFAULT_SKIN.into(),
            audio: HopAudio::default(),
        }
    }
}

fn finite_in(name: &str, v: f32, lo: f32, hi: f32) -> Result<(), String> {
    if v.is_finite() && (lo..=hi).contains(&v) {
        Ok(())
    } else {
        Err(format!("hop spec: {name} must be {lo}..={hi}"))
    }
}

impl HopSpec {
    pub fn from_value(v: &serde_json::Value) -> Result<Self, String> {
        let spec: HopSpec =
            serde_json::from_value(v.clone()).map_err(|e| format!("hop spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn beat_secs(&self) -> f32 {
        60.0 / self.music.bpm
    }

    /// Metres per second.
    pub fn speed(&self) -> f32 {
        self.hero.beat_len_m / self.beat_secs()
    }

    pub fn arc(&self, hop: Hop) -> &HopArc {
        match hop {
            Hop::Tap => &self.hero.tap,
            Hop::Hold => &self.hero.hold,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !SKINS.iter().any(|(id, _)| *id == self.skin) {
            let ids: Vec<&str> = SKINS.iter().map(|(id, _)| *id).collect();
            return Err(format!("hop spec: skin is {}", ids.join("|")));
        }
        let m = &self.music;
        finite_in("music.bpm", m.bpm, 60.0, 200.0)?;
        finite_in("music.offset_ms", m.offset_ms, -500.0, 500.0)?;
        if !(2..=8).contains(&m.beats_per_bar) {
            return Err("hop spec: music.beats_per_bar must be 2..=8".into());
        }
        if !(1..=16).contains(&m.lead_in_bars) || !(1..=64).contains(&m.loop_bars) {
            return Err("hop spec: music.lead_in_bars 1..=16, loop_bars 1..=64".into());
        }
        let h = &self.hero;
        finite_in("hero.beat_len_m", h.beat_len_m, 1.0, 8.0)?;
        for (n, a) in [("hold", &h.hold), ("tap", &h.tap)] {
            finite_in(&format!("hero.{n}.air_beats"), a.air_beats, 0.25, 4.0)?;
            finite_in(&format!("hero.{n}.height"), a.height, 0.1, 4.0)?;
        }
        if h.tap.air_beats >= h.hold.air_beats {
            return Err("hop spec: hero.tap.air_beats must be shorter than hold".into());
        }
        finite_in("hero.tap_window_secs", h.tap_window_secs, 0.02, 0.4)?;
        finite_in("hero.coyote_secs", h.coyote_secs, 0.0, 0.3)?;
        finite_in("hero.buffer_secs", h.buffer_secs, 0.0, 0.3)?;
        finite_in("hero.half_width", h.half_width, 0.05, 1.0)?;
        finite_in("hero.edge_tol", h.edge_tol, 0.0, 1.0)?;
        finite_in("hero.fall_gravity", h.fall_gravity, 5.0, 80.0)?;
        let c = &self.course;
        finite_in("course.block_height", c.block_height, 0.1, 2.0)?;
        finite_in("course.min_window_ms", c.min_window_ms, 0.0, 400.0)?;
        if c.ramp_bars == 0 || c.rest_every_bars == 0 {
            return Err("hop spec: course.ramp_bars and rest_every_bars must be >= 1".into());
        }
        let j = &self.judge;
        finite_in("judge.perfect_ms", j.perfect_ms, 5.0, 250.0)?;
        finite_in("judge.good_ms", j.good_ms, 5.0, 400.0)?;
        if j.perfect_ms >= j.good_ms {
            return Err("hop spec: judge.perfect_ms must be below good_ms".into());
        }
        if !(1..=64).contains(&j.combo_cap) {
            return Err("hop spec: judge.combo_cap must be 1..=64".into());
        }
        finite_in("judge.per_beat", j.per_beat, 0.0, 100.0)?;
        if !(1..=LivesSpec::MAX).contains(&self.lives.count)
            || !(0.0..=10.0).contains(&self.lives.invuln_secs)
        {
            return Err("hop spec: lives.count 1..=9, invuln_secs 0..=10".into());
        }
        let rc = &self.recover;
        finite_in("recover.drop_secs", rc.drop_secs, 0.2, 3.0)?;
        finite_in("recover.ready_beats", rc.ready_beats, 0.0, 8.0)?;
        finite_in("recover.invuln_secs", rc.invuln_secs, 0.0, 10.0)?;
        finite_in("recover.clear_beats", rc.clear_beats, 0.0, 32.0)?;
        let r = &self.revive;
        finite_in("revive.invuln_secs", r.invuln_secs, 0.0, 10.0)?;
        finite_in("revive.clear_beats", r.clear_beats, 0.0, 32.0)?;
        if !(1..=9).contains(&r.lives) || r.max_per_run > 9 {
            return Err("hop spec: revive.lives 1..=9, max_per_run 0..=9".into());
        }
        let f = &self.fall;
        for (n, v, lo, hi) in [
            ("fall.edge_stop_secs", f.edge_stop_secs, 0.0, 0.5),
            (
                "fall.tumble_turns_per_sec",
                f.tumble_turns_per_sec,
                0.0,
                4.0,
            ),
            ("fall.stretch", f.stretch, 0.0, 1.0),
            ("fall.carry", f.carry, 0.0, 1.0),
            ("fall.cam_lag", f.cam_lag, 0.0, 1.0),
            ("fall.slow_scale", f.slow_scale, 0.1, 1.0),
            ("fall.card_delay_secs", f.card_delay_secs, 0.3, 3.0),
            ("fall.vignette", f.vignette, 0.0, 1.0),
        ] {
            finite_in(n, v, lo, hi)?;
        }
        if f.dust > 32 {
            return Err("hop spec: fall.dust 0..=32".into());
        }
        let a = &self.audio;
        for (n, v) in [
            ("master", a.master),
            ("bgm_volume", a.bgm_volume),
            ("sfx_volume", a.sfx_volume),
        ] {
            finite_in(&format!("audio.{n}"), v, 0.0, 1.0)?;
        }
        finite_in("audio.bgm.volume", a.bgm.volume, 0.0, 1.0)?;
        finite_in("audio.bgm.fade_secs", a.bgm.fade_secs, 0.0, 10.0)?;
        for (n, s) in &a.sfx {
            if !SFX_CUES.contains(&n.as_str()) {
                return Err(format!(
                    "hop spec: audio.sfx.{n} is not a cue ({SFX_CUES:?})"
                ));
            }
            finite_in(&format!("audio.sfx.{n}.volume"), s.volume, 0.0, 1.0)?;
            finite_in(&format!("audio.sfx.{n}.rate"), s.rate, 0.25, 4.0)?;
        }
        self.look.validate()?;
        self.validate_enemies()?;
        let problems = self.course_problems();
        if let Some(first) = problems.first() {
            return Err(format!("hop spec: {first} ({} problem(s))", problems.len()));
        }
        Ok(())
    }

    fn validate_enemies(&self) -> Result<(), String> {
        self.enemies
            .validate("hop spec", &["slime", "beetle", "bat"])
    }

    /// How long a bounce that starts at song time `t` lasts: it comes down on
    /// the next half beat at least 0.2 s away.
    pub fn bounce_secs(&self, t: f32) -> f32 {
        let half = self.beat_secs() * 0.5;
        (((t + 0.2) / half).ceil() * half - t).max(0.2)
    }

    /// Index of the enemy type called `id`.
    pub fn enemy_index(&self, id: &str) -> Option<usize> {
        self.enemies.index(id)
    }

    /// The phrases in use: the spec's, or the built-in set.
    pub fn phrases(&self) -> Vec<Phrase> {
        if self.course.phrases.is_empty() {
            builtin_phrases()
        } else {
            self.course.phrases.clone()
        }
    }

    /// Does the player have to press for this one? A hovering enemy is only
    /// something to stay clear of.
    pub fn needs_press(&self, o: &Obst) -> bool {
        match o.kind {
            Kind::Enemy => o.y_lo <= 0.0,
            _ => true,
        }
    }

    /// Impossible or unmusical placements in the phrase set. Empty = fine.
    pub fn course_problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let bpb = self.music.beats_per_bar as f32;
        let phrases = self.phrases();
        if phrases.is_empty() {
            out.push("no phrases".into());
        }
        for (pi, p) in phrases.iter().enumerate() {
            if p.bars == 0 || p.bars > 16 {
                out.push(format!("phrase {pi}: bars must be 1..=16"));
                continue;
            }
            if !(1..=16).contains(&p.weight) {
                out.push(format!("phrase {pi}: weight must be 1..=16"));
            }
            let end = p.bars as f32 * bpb;
            let mut slots: Vec<foes::Slot> = Vec::new();
            let mut sound = true;
            for (ii, it) in p.items.iter().enumerate() {
                let where_ = format!("phrase {pi} item {ii}");
                let Some((kind, hop)) = parse_item(it) else {
                    out.push(format!("{where_}: kind gap|block|enemy and hop tap|hold"));
                    sound = false;
                    continue;
                };
                if !(0.1..=4.0).contains(&it.beats) || it.beat < 0.0 {
                    out.push(format!("{where_}: bad beat or width"));
                    sound = false;
                    continue;
                }
                let mut ety = None;
                if kind == Kind::Enemy {
                    match self.enemy_index(&it.enemy) {
                        None => {
                            out.push(format!("{where_}: unknown enemy {:?}", it.enemy));
                            sound = false;
                            continue;
                        }
                        Some(i) => ety = Some(i),
                    }
                } else if !it.enemy.is_empty() {
                    out.push(format!("{where_}: only kind enemy names an enemy"));
                }
                let probe = self.probe(kind, hop, it.beats, it.beat, ety);
                let air = self.arc(hop).air_beats;
                slots.push(foes::Slot {
                    item: ii,
                    enemy: ety,
                    at: it.beat,
                    len: air,
                });
                if !self.needs_press(&probe) {
                    // A flyer: the hops around it must stay clear (the sequence check).
                    continue;
                }
                let (lo, hi) = self.window_secs_obst(&probe);
                let width_ms = (hi - lo).max(0.0) * 1000.0;
                if width_ms < self.course.min_window_ms {
                    out.push(format!(
                        "{where_}: only {width_ms:.0} ms of timing window (too wide or too fast for a {} hop?)",
                        if hop == Hop::Tap { "tap" } else { "hold" }
                    ));
                }
            }
            out.extend(foes::spacing_problems(
                &self.enemies,
                &format!("phrase {pi}"),
                p.level,
                end,
                &slots,
            ));
            if sound {
                out.extend(self.flyer_gap_problems(pi, p));
                out.extend(self.sequence_problems(pi, p));
            }
        }
        out
    }

    /// A flyer may not hang over a gap: the whole span of the hop over the gap, 100 ms of
    /// press slack either way, has to be clear of it. A bat above a hole leaves a thumb that
    /// stays down a moment too long (a tap that turns into a hold) nowhere to go.
    fn flyer_gap_problems(&self, pi: usize, p: &Phrase) -> Vec<String> {
        let slack = 0.1 / self.beat_secs();
        let mut out = Vec::new();
        for (fi, f) in p.items.iter().enumerate() {
            let Some((Kind::Enemy, fhop)) = parse_item(f) else {
                continue;
            };
            let flies = self
                .enemy_index(&f.enemy)
                .and_then(|i| self.enemies.types.get(i))
                .is_some_and(|t| t.y_lo > 0.0);
            if !flies {
                continue;
            }
            let c = f.beat + self.arc(fhop).air_beats * 0.5;
            let (f0, f1) = (c - f.beats * 0.5, c + f.beats * 0.5);
            for (gi, g) in p.items.iter().enumerate() {
                if !matches!(parse_item(g), Some((Kind::Gap, _))) {
                    continue;
                }
                let air = parse_item(g).map_or(0.0, |(_, h)| self.arc(h).air_beats);
                if f0 < g.beat + air + slack && f1 > g.beat - slack {
                    out.push(format!(
                        "phrase {pi} item {fi}: a flyer over the gap of item {gi} (keep it clear of the hop across the gap)"
                    ));
                }
            }
        }
        out
    }

    /// Does this phrase hold a flyer (an enemy that hovers above the track and only
    /// punishes a hold hop)?
    pub fn has_flyer(&self, p: &Phrase) -> bool {
        p.items.iter().any(|it| {
            parse_item(it).is_some_and(|(k, _)| k == Kind::Enemy)
                && self
                    .enemy_index(&it.enemy)
                    .and_then(|i| self.enemies.types.get(i))
                    .is_some_and(|t| t.y_lo > 0.0)
        })
    }

    /// Phrase `a` followed straight away by phrase `b` (no rest bar: the worst case for
    /// hops that spill over the seam and for flyers that walk in early). True = a bot
    /// pressing every ideal `off` seconds early or late takes no damage.
    pub fn pair_survives(&self, a: &Phrase, b: &Phrase, off: f32) -> bool {
        let base = 4.0 * self.music.beats_per_bar as f32;
        let mut obst = self.phrase_obstacles(a, base);
        obst.extend(
            self.phrase_obstacles(b, base + a.bars as f32 * self.music.beats_per_bar as f32),
        );
        self.sequence_survives(&obst, off)
    }

    /// The widest press error (ms, in 10 ms steps, at most 100) a phrase gets through
    /// on its own. A seam is asked for no more than the narrower of its two phrases.
    pub fn phrase_half_ms(&self, p: &Phrase) -> f32 {
        let obst = self.phrase_obstacles(p, 4.0 * self.music.beats_per_bar as f32);
        let mut half = 0.0;
        while half < 100.0 {
            let next = half + 10.0;
            if !self.sequence_survives(&obst, -next * 0.001)
                || !self.sequence_survives(&obst, next * 0.001)
            {
                break;
            }
            half = next;
        }
        half
    }

    /// What a seam is held to: the narrower of the two phrases' own windows.
    pub fn pair_half_ms(&self, a: &Phrase, b: &Phrase) -> f32 {
        self.phrase_half_ms(a).min(self.phrase_half_ms(b))
    }

    /// The first press (ms, early = negative) that takes damage in phrase `a` followed
    /// straight away by phrase `b`, within `half_ms` either way in 10 ms steps.
    pub fn seam_fail(&self, a: &Phrase, b: &Phrase, half_ms: f32) -> Option<f32> {
        let n = (half_ms / 10.0).ceil() as i32;
        (-n..=n)
            .map(|k| (k as f32 * 10.0).clamp(-half_ms, half_ms))
            .find(|ms| !self.pair_survives(a, b, ms * 0.001))
    }

    /// The seams between phrases: every ordered pair of phrases with a flyer in it (a bat
    /// over a gap, a bat right after a hold) is played at every press from `half_ms` early
    /// to `half_ms` late. Empty = every such pair is winnable.
    pub fn seam_problems(&self, half_ms: f32) -> Vec<String> {
        let phrases = self.phrases();
        let mut out = Vec::new();
        for (ai, a) in phrases.iter().enumerate() {
            for (bi, b) in phrases.iter().enumerate() {
                if !(self.has_flyer(a) || self.has_flyer(b)) {
                    continue;
                }
                let half = half_ms.min(self.pair_half_ms(a, b));
                if let Some(ms) = self.seam_fail(a, b, half) {
                    out.push(format!(
                        "phrases {ai} then {bi}: a bot pressing {:.0} ms {} the ideal beat takes damage",
                        ms.abs(),
                        if ms < 0.0 { "before" } else { "after" }
                    ));
                }
            }
        }
        out
    }

    /// The obstacles of a phrase laid at `base` beats, as the course lays them.
    pub fn phrase_obstacles(&self, p: &Phrase, base: f32) -> Vec<Obst> {
        p.items
            .iter()
            .filter_map(|it| {
                let (kind, hop) = parse_item(it)?;
                let ety = if kind == Kind::Enemy {
                    Some(self.enemy_index(&it.enemy)?)
                } else {
                    None
                };
                Some(self.probe(kind, hop, it.beats, base + it.beat, ety))
            })
            .collect()
    }

    /// Plays the whole phrase with a bot that presses every ideal `off`
    /// seconds early or late (a negative `off` is early). True = no damage.
    /// A stomp counts as a clear; the bounce is left out because the phrase
    /// rules keep the next press clear of it.
    pub fn sequence_survives(&self, obst: &[Obst], off: f32) -> bool {
        let bs = self.beat_secs();
        let v = self.speed();
        let hw = self.hero.half_width;
        let mut presses: Vec<(f32, Hop)> = obst
            .iter()
            .filter(|o| self.needs_press(o))
            .map(|o| (o.take_beat * bs + off, o.hop))
            .collect();
        presses.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (Some(first), Some(last)) = (
            obst.iter().map(|o| o.take_beat).reduce(f32::min),
            obst.iter().map(|o| o.take_beat).reduce(f32::max),
        ) else {
            return true;
        };
        let mut gone = vec![false; obst.len()];
        // (start, land, start height) of a bounce in progress.
        let mut bounce: Option<(f32, f32, f32)> = None;
        let mut t = (first - 3.0) * bs;
        let t_end = (last + 5.0) * bs;
        while t <= t_end {
            let x = t * v;
            let mut y = 0.0;
            let mut airborne = false;
            let mut desc = false;
            for (pt, hop) in presses.iter().rev() {
                if *pt > t {
                    continue;
                }
                let arc = self.arc(*hop);
                let air = arc.air_beats * bs;
                if t - pt < air {
                    y = arc_y(arc.height, air, t - pt);
                    airborne = true;
                    desc = t - pt > air * 0.5;
                }
                break;
            }
            if let Some((t0, land, y0)) = bounce {
                if t >= land {
                    bounce = None;
                } else {
                    let u = (t - t0) / (land - t0);
                    y = bounce_y(y0, self.enemies.bounce_height, u);
                    airborne = true;
                    desc = bounce_dy(y0, self.enemies.bounce_height, u) < 0.0;
                }
            }
            for (i, o) in obst.iter().enumerate() {
                if gone[i] {
                    continue;
                }
                if o.kind == Kind::Gap {
                    if !airborne
                        && x > o.start + self.hero.edge_tol
                        && x < o.end - self.hero.edge_tol
                    {
                        return false;
                    }
                    continue;
                }
                match self.touch(o, t, x, y, desc, hw) {
                    Touch::Hit => return false,
                    Touch::Stomp => {
                        gone[i] = true;
                        let land = t + self.bounce_secs(t);
                        // No press can be taken while bouncing.
                        if presses.iter().any(|(pt, _)| *pt > t && *pt < land) {
                            return false;
                        }
                        bounce = Some((t, land, y));
                    }
                    Touch::None => {}
                }
            }
            t += 0.004;
        }
        true
    }

    /// Every press within the Perfect window of its ideal beat (`judge.perfect_ms`,
    /// at most 100 ms) must get through the whole phrase untouched: the per-item
    /// window alone can sit off centre, and a walker or a flyer next to a gap
    /// only shows up when the hops are played one after the other.
    fn sequence_problems(&self, pi: usize, p: &Phrase) -> Vec<String> {
        let base = 4.0 * self.music.beats_per_bar as f32;
        let obst = self.phrase_obstacles(p, base);
        // A phrase with a flyer in it has to hold at the full 100 ms: the bat is the one
        // thing a late or early tap can turn into a hold hop under.
        let half = if self.has_flyer(p) {
            0.1
        } else {
            (self.judge.perfect_ms * 0.001).min(0.1)
        };
        let mut out = Vec::new();
        let hook = PhraseRun { spec: self, obst };
        if let Some(off) = foes::sweep(&hook, half, 0.005) {
            out.push(format!(
                "phrase {pi}: a bot pressing {:.0} ms {} the ideal beat takes damage",
                off.abs() * 1000.0,
                if off < 0.0 { "before" } else { "after" }
            ));
        }
        out
    }

    /// The obstacle for an item at `take_beat` (the beat of the ideal press),
    /// centred under the arc of `hop`, exactly as the course lays it.
    pub fn probe(
        &self,
        kind: Kind,
        hop: Hop,
        width_beats: f32,
        take_beat: f32,
        ety: Option<usize>,
    ) -> Obst {
        let bl = self.hero.beat_len_m;
        let center = take_beat + self.arc(hop).air_beats * 0.5;
        let mut o = Obst::new(
            kind,
            (center - width_beats * 0.5) * bl,
            (center + width_beats * 0.5) * bl,
            take_beat,
            hop,
            self.course.block_height,
        );
        if let (Kind::Enemy, Some(i)) = (kind, ety) {
            if let Some(et) = self.enemies.types.get(i) {
                o.ety = i;
                o.y_lo = et.y_lo;
                o.height = et.y_hi;
            }
        }
        o
    }

    /// How far (metres) the enemy stands from its meeting spot at song time
    /// `t`, or None if it has not appeared yet. Positive = still ahead.
    pub fn enemy_dx(&self, o: &Obst, t: f32) -> Option<f32> {
        let Some(et) = self.enemies.types.get(o.ety) else {
            return Some(0.0);
        };
        let spb = et.steps_per_beat.max(1) as f32;
        let now = (t / self.beat_secs() * spb).floor() as i64;
        let meet = ((o.start + o.end) * 0.5 / self.hero.beat_len_m * spb).floor() as i64;
        self.enemies.walk_dx(o.ety, now, meet)
    }

    /// What the hero at (`x`, `y`) touches of `o` at song time `t`.
    pub fn touch(&self, o: &Obst, t: f32, x: f32, y: f32, descending: bool, hw: f32) -> Touch {
        match o.kind {
            Kind::Gap => Touch::None,
            Kind::Block => {
                if hits_block(o, x, y, hw) {
                    Touch::Hit
                } else {
                    Touch::None
                }
            }
            Kind::Enemy => {
                let Some(dx) = self.enemy_dx(o, t) else {
                    return Touch::None;
                };
                let hero = HeroBox {
                    x,
                    y,
                    half_width: hw,
                    descending,
                };
                self.enemies.touch(o.ety, o.start + dx, o.end + dx, &hero)
            }
        }
    }

    /// The unbroken stretch of press times (relative to the ideal, seconds)
    /// around the ideal one that clear an obstacle written for `hop`, found by
    /// playing the hop out. (0, 0) if even the ideal press fails.
    pub fn window_secs(&self, kind: Kind, hop: Hop, width_beats: f32) -> (f32, f32) {
        self.window_secs_obst(&self.probe(kind, hop, width_beats, 0.0, None))
    }

    /// Same, for a laid obstacle (an enemy's window depends on where on the
    /// beat grid it stands, so pass the real one).
    pub fn window_secs_obst(&self, o: &Obst) -> (f32, f32) {
        let bs = self.beat_secs();
        let step = 0.002;
        if !self.clears_obst(o, 0.0) {
            return (0.0, 0.0);
        }
        // The unbroken stretch of good presses around the ideal one.
        let (mut lo, mut hi) = (0.0_f32, 0.0_f32);
        while lo > -2.0 * bs && self.clears_obst(o, lo - step) {
            lo -= step;
        }
        while hi < 2.0 * bs && self.clears_obst(o, hi + step) {
            hi += step;
        }
        (lo, hi)
    }

    /// Does a hop of `hop` pressed `off` seconds from the ideal clear this
    /// obstacle? Uses the same support and hit tests as the sim.
    pub fn clears(&self, kind: Kind, hop: Hop, width_beats: f32, off: f32) -> bool {
        self.clears_obst(&self.probe(kind, hop, width_beats, 0.0, None), off)
    }

    pub fn clears_obst(&self, o: &Obst, off: f32) -> bool {
        let bs = self.beat_secs();
        let v = self.speed();
        let arc = self.arc(o.hop);
        let air = arc.air_beats * bs;
        let take = o.take_beat * bs + off;
        let hw = self.hero.half_width;
        let mut t = take - 3.0 * bs;
        while t <= take + air + 3.0 * bs {
            let x = t * v;
            let el = t - take;
            let airborne = el >= 0.0 && el < air;
            let y = if airborne {
                arc_y(arc.height, air, el)
            } else {
                0.0
            };
            if !airborne && !supported(std::slice::from_ref(o), x, self.hero.edge_tol) {
                return false;
            }
            match self.touch(o, t, x, y, airborne && el > air * 0.5, hw) {
                Touch::Hit => return false,
                Touch::Stomp => return true,
                Touch::None => {}
            }
            t += 0.004;
        }
        true
    }
}

/// A phrase played by the checker's bot: the kit's side of [`foes::Clearable`].
struct PhraseRun<'a> {
    spec: &'a HopSpec,
    obst: Vec<Obst>,
}

impl foes::Clearable for PhraseRun<'_> {
    fn survives(&self, offset: f32) -> bool {
        self.spec.sequence_survives(&self.obst, offset)
    }
}

/// `kind` and `hop` of an item, or None if either word is wrong.
pub fn parse_item(it: &Item) -> Option<(Kind, Hop)> {
    let kind = match it.kind.as_str() {
        "gap" => Kind::Gap,
        "block" => Kind::Block,
        "enemy" => Kind::Enemy,
        _ => return None,
    };
    let hop = match it.hop.as_str() {
        "tap" => Hop::Tap,
        "hold" => Hop::Hold,
        _ => return None,
    };
    Some((kind, hop))
}

fn item(beat: f32, kind: &str, beats: f32, hop: &str) -> Item {
    Item {
        beat,
        kind: kind.into(),
        beats,
        hop: hop.into(),
        enemy: String::new(),
    }
}

/// An enemy of type `id` whose ideal hop is pressed at `beat`.
fn foe(beat: f32, id: &str, beats: f32) -> Item {
    Item {
        enemy: id.into(),
        ..item(beat, "enemy", beats, "tap")
    }
}

/// The phrase set the game ships with: five levels, one to three bars each,
/// with enemies introduced one kind per level.
pub fn builtin_phrases() -> Vec<Phrase> {
    let p = |level, bars, items| Phrase {
        level,
        bars,
        items,
        weight: 1,
        debut: false,
    };
    let debut = |mut ph: Phrase| {
        ph.debut = true;
        ph
    };
    // Enemies arrive one kind per level: slime (0), beetle (1), bat (2), then mixes.
    let mut phrases = vec![
        p(
            0,
            2,
            vec![item(1.0, "gap", 0.5, "tap"), item(3.0, "gap", 0.5, "tap")],
        ),
        // The first slime comes on its own.
        debut(p(0, 2, vec![foe(1.0, "slime", 0.12)])),
        p(0, 2, vec![foe(1.0, "slime", 0.12), foe(3.0, "slime", 0.12)]),
        p(
            0,
            2,
            vec![item(1.0, "gap", 0.5, "tap"), foe(3.0, "slime", 0.12)],
        ),
        p(
            0,
            2,
            vec![
                item(1.0, "block", 0.2, "tap"),
                item(3.0, "block", 0.2, "tap"),
            ],
        ),
        p(0, 2, vec![item(0.0, "gap", 1.0, "hold")]),
        p(
            1,
            3,
            vec![
                item(0.0, "gap", 1.0, "hold"),
                item(2.0, "gap", 0.5, "tap"),
                item(3.0, "block", 0.2, "tap"),
            ],
        ),
        p(
            1,
            2,
            vec![
                item(0.0, "gap", 0.5, "tap"),
                item(1.0, "block", 0.2, "tap"),
                item(2.0, "gap", 0.5, "tap"),
            ],
        ),
        // The first beetle comes on its own.
        debut(p(1, 2, vec![foe(1.0, "beetle", 0.16)])),
        p(
            1,
            2,
            vec![foe(1.0, "beetle", 0.16), foe(3.0, "slime", 0.12)],
        ),
        p(
            1,
            2,
            vec![foe(1.0, "beetle", 0.16), item(3.0, "gap", 0.5, "tap")],
        ),
        p(
            2,
            2,
            vec![
                item(0.5, "gap", 0.5, "tap"),
                item(1.5, "block", 0.2, "tap"),
                item(2.5, "gap", 0.5, "tap"),
            ],
        ),
        // The first bat flies alone over a clear track: holding would hit it.
        debut(p(2, 2, vec![foe(1.0, "bat", 0.9)])),
        p(
            3,
            2,
            // The bat hangs over the beetle, never over the gap: a tap that stays down a
            // moment too long becomes a hold, and a hold flies into a bat.
            vec![
                item(1.0, "gap", 0.5, "tap"),
                foe(3.0, "beetle", 0.16),
                foe(3.0, "bat", 0.9),
            ],
        ),
        p(
            2,
            2,
            vec![item(0.0, "gap", 1.5, "hold"), item(2.0, "gap", 1.5, "hold")],
        ),
        p(
            3,
            1,
            vec![
                item(0.0, "gap", 0.5, "tap"),
                item(1.0, "block", 0.2, "tap"),
                item(2.0, "gap", 0.5, "tap"),
                item(3.0, "block", 0.2, "tap"),
            ],
        ),
        p(
            3,
            3,
            vec![
                foe(0.5, "slime", 0.12),
                foe(2.5, "slime", 0.12),
                foe(4.5, "beetle", 0.16),
            ],
        ),
        p(
            3,
            2,
            vec![
                foe(1.0, "beetle", 0.16),
                item(3.0, "gap", 0.5, "tap"),
                item(4.0, "gap", 0.5, "tap"),
            ],
        ),
        p(
            3,
            2,
            vec![
                foe(1.0, "slime", 0.12),
                foe(1.0, "bat", 0.9),
                foe(3.0, "slime", 0.12),
                foe(3.0, "bat", 0.9),
            ],
        ),
        p(
            3,
            2,
            vec![
                foe(0.5, "slime", 0.12),
                foe(2.5, "beetle", 0.16),
                foe(4.5, "slime", 0.12),
                item(6.5, "gap", 0.5, "tap"),
            ],
        ),
        p(
            3,
            3,
            vec![
                item(0.0, "gap", 1.5, "hold"),
                item(2.0, "gap", 0.5, "tap"),
                item(3.0, "gap", 0.5, "tap"),
                item(4.0, "gap", 1.0, "hold"),
            ],
        ),
        p(
            4,
            3,
            vec![
                item(0.0, "gap", 0.5, "tap"),
                item(1.0, "gap", 0.5, "tap"),
                item(2.0, "gap", 0.5, "tap"),
                item(3.0, "block", 0.2, "tap"),
                item(4.0, "gap", 1.5, "hold"),
            ],
        ),
        p(
            4,
            3,
            vec![
                foe(1.0, "beetle", 0.16),
                foe(3.0, "slime", 0.12),
                item(5.0, "gap", 0.5, "tap"),
                foe(7.0, "beetle", 0.16),
            ],
        ),
        p(
            4,
            3,
            vec![
                item(0.0, "gap", 1.5, "hold"),
                foe(3.0, "slime", 0.12),
                foe(3.0, "bat", 0.9),
                item(5.0, "gap", 1.5, "hold"),
            ],
        ),
        p(
            4,
            2,
            vec![
                foe(0.5, "beetle", 0.16),
                foe(2.5, "slime", 0.12),
                foe(4.5, "beetle", 0.16),
                foe(6.5, "slime", 0.12),
            ],
        ),
        p(
            4,
            2,
            vec![
                item(0.0, "gap", 1.5, "hold"),
                item(2.0, "gap", 1.5, "hold"),
                item(4.0, "gap", 0.5, "tap"),
                item(5.0, "block", 0.2, "tap"),
                item(6.0, "gap", 0.5, "tap"),
                item(7.0, "block", 0.2, "tap"),
            ],
        ),
    ];
    // Enemy phrases come up more often than the plain ones (a slime in the
    // first bars, then a heavier mix as the run goes on).
    for ph in phrases.iter_mut() {
        if ph.items.iter().any(|it| it.kind == "enemy") {
            ph.weight = 3;
        }
    }
    phrases
}

// -------------------------------------------------------------- course ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Gap,
    Block,
    /// A creature from `enemies.types`.
    Enemy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hop {
    Tap,
    Hold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fate {
    Pending,
    Cleared,
    Skipped,
}

#[derive(Clone, Debug)]
pub struct Obst {
    pub kind: Kind,
    pub start: f32,
    pub end: f32,
    /// Absolute beat of the ideal press.
    pub take_beat: f32,
    pub hop: Hop,
    /// Block and enemy: the top of the hurt band.
    pub height: f32,
    pub fate: Fate,
    /// Enemy: index into `enemies.types` (`usize::MAX` = none) and the bottom
    /// of its hurt band.
    pub ety: usize,
    pub y_lo: f32,
    /// Song time the hero stomped it.
    pub stomped_t: Option<f32>,
    /// Song time it hurt the hero.
    pub hit_t: Option<f32>,
}

impl Obst {
    pub fn new(kind: Kind, start: f32, end: f32, take_beat: f32, hop: Hop, height: f32) -> Self {
        Self {
            kind,
            start,
            end,
            take_beat,
            hop,
            height,
            fate: Fate::Pending,
            ety: usize::MAX,
            y_lo: 0.0,
            stomped_t: None,
            hit_t: None,
        }
    }
}

/// Height of a hop `el` seconds after takeoff.
/// Height `u` (0..1) of the way through a bounce that starts `y0` above the slab.
fn bounce_y(y0: f32, peak: f32, u: f32) -> f32 {
    (y0 * (1.0 - u) + 4.0 * peak * u * (1.0 - u)).max(0.0)
}

fn bounce_dy(y0: f32, peak: f32, u: f32) -> f32 {
    -y0 + 4.0 * peak * (1.0 - 2.0 * u)
}

fn arc_y(height: f32, air: f32, el: f32) -> f32 {
    let u = el / air;
    if (0.0..1.0).contains(&u) {
        4.0 * height * u * (1.0 - u)
    } else {
        0.0
    }
}

/// Is there slab under `x`?
fn supported(obst: &[Obst], x: f32, tol: f32) -> bool {
    !obst
        .iter()
        .any(|o| o.kind == Kind::Gap && x > o.start + tol && x < o.end - tol)
}

fn hits_block(o: &Obst, x: f32, y: f32, hw: f32) -> bool {
    o.kind == Kind::Block && y < o.height && x > o.start - hw && x < o.end + hw
}

fn level_at(bar: u32, ramp_bars: u32, max_level: u32) -> u32 {
    (bar.saturating_mul(max_level + 1) / ramp_bars.max(1)).min(max_level)
}

/// The drop that plays after a fatal fall (the sim has stopped; this is only drawn).
#[derive(Clone, Copy, Debug, PartialEq)]
struct DeadFall {
    /// Slowed seconds since the sim ended.
    age: f32,
    /// How far the camera already trailed the bunny.
    cam: f32,
    vy: f32,
    /// `fall_age` when it ended.
    t0: f32,
}

/// Lays the course phrase by phrase, from the seed. Pure: same spec and seed,
/// same obstacles.
#[derive(Clone, Debug)]
pub struct Course {
    phrases: Vec<Phrase>,
    max_level: u32,
    rng: u32,
    /// Next phrase starts here (beat).
    gen_beat: f32,
    bars_laid: u32,
    since_rest: u32,
    last: usize,
    /// What was laid, in order: a phrase index, or `usize::MAX` for a rest bar.
    laid: Vec<usize>,
    /// The level of the previous phrase (u32::MAX before the first).
    lvl_seen: u32,
    pub obst: Vec<Obst>,
}

impl Course {
    pub fn new(spec: &HopSpec) -> Self {
        let phrases = spec.phrases();
        let max_level = phrases.iter().map(|p| p.level).max().unwrap_or(0);
        Self {
            phrases,
            max_level,
            rng: spec
                .course
                .seed
                .wrapping_mul(2_654_435_761)
                .wrapping_add(12345),
            gen_beat: (spec.music.lead_in_bars * spec.music.beats_per_bar) as f32,
            bars_laid: 0,
            since_rest: 0,
            last: usize::MAX,
            laid: Vec::new(),
            lvl_seen: u32::MAX,
            obst: Vec::new(),
        }
    }

    fn rand(&mut self) -> u32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.rng >> 8
    }

    pub fn gen_beat(&self) -> f32 {
        self.gen_beat
    }

    /// The phrases laid so far, in order (`usize::MAX` = a rest bar).
    pub fn laid(&self) -> &[usize] {
        &self.laid
    }

    /// Lay phrases until the course reaches `until_beat`.
    pub fn ensure(&mut self, spec: &HopSpec, until_beat: f32) {
        let bpb = spec.music.beats_per_bar as f32;
        while self.gen_beat < until_beat {
            if self.since_rest >= spec.course.rest_every_bars {
                self.since_rest = 0;
                self.gen_beat += bpb;
                self.bars_laid += 1;
                self.laid.push(usize::MAX);
                continue;
            }
            let lvl = level_at(self.bars_laid, spec.course.ramp_bars, self.max_level);
            let mut pool: Vec<usize> = (0..self.phrases.len())
                .filter(|&i| {
                    self.phrases[i].level == lvl || (lvl > 0 && self.phrases[i].level == lvl - 1)
                })
                .collect();
            if pool.is_empty() {
                pool = (0..self.phrases.len()).collect();
            }
            if lvl != self.lvl_seen {
                self.lvl_seen = lvl;
                let debuts: Vec<usize> = pool
                    .iter()
                    .copied()
                    .filter(|&i| self.phrases[i].debut && self.phrases[i].level == lvl)
                    .collect();
                if !debuts.is_empty() {
                    pool = debuts;
                }
            }
            if pool.len() > 1 {
                pool.retain(|&i| i != self.last);
            }
            let weights: Vec<u32> = pool.iter().map(|&i| self.phrases[i].weight).collect();
            let roll = self.rand();
            let pick = pool[foes::weighted_pick(&weights, roll)];
            self.last = pick;
            self.laid.push(pick);
            let p = self.phrases[pick].clone();
            for it in &p.items {
                let Some((kind, hop)) = parse_item(it) else {
                    continue;
                };
                let ety = if kind == Kind::Enemy {
                    spec.enemy_index(&it.enemy)
                } else {
                    None
                };
                if kind == Kind::Enemy && ety.is_none() {
                    continue;
                }
                self.obst
                    .push(spec.probe(kind, hop, it.beats, self.gen_beat + it.beat, ety));
            }
            self.gen_beat += p.bars as f32 * bpb;
            self.bars_laid += p.bars;
            self.since_rest += p.bars;
        }
    }

    /// Drop obstacles whose ideal press is before `beat` (used by revive).
    pub fn clear_before(&mut self, beat: f32) {
        self.obst.retain(|o| o.take_beat >= beat);
    }

    fn prune(&mut self, behind_m: f32) {
        self.obst.retain(|o| o.end > behind_m);
    }
}

// ----------------------------------------------------------------- sim ----

#[derive(Clone, Copy, Debug, PartialEq)]
enum Air {
    Ground,
    Hop {
        t0: f32,
        hop: Hop,
        press_t: f32,
        id: u32,
    },
    Fall {
        vy: f32,
    },
    /// The hop after a stomp: a parabola from where the hero was to the slab,
    /// timed to come down on a half beat.
    Bounce {
        t0: f32,
        dur: f32,
        y0: f32,
        id: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grade {
    Perfect,
    Good,
    Ok,
    Stomp,
}

impl Grade {
    pub fn name(self) -> &'static str {
        match self {
            Grade::Perfect => "perfect",
            Grade::Good => "good",
            Grade::Ok => "ok",
            Grade::Stomp => "stomp",
        }
    }
}

#[derive(Debug)]
pub struct HopBeat {
    pub spec: HopSpec,
    pub flow: GameFlow,
    core: Core,
    course: Course,
    doc: WorldDoc,
    picture_quality: Option<String>,
    /// A/B switches from the page (`fx_post`, `fx_atmo`): the finishing pass and the sky extras.
    fx_post: bool,
    fx_atmo: bool,
    /// Song clock, seconds. x = song_t * speed.
    song_t: f32,
    /// Added to the page's audio clock (a revive restarts the music mid-song).
    clock_base: f32,
    clock: Option<f32>,
    latency_user_ms: f32,
    /// Direct mode: in the run, waiting for the first tap.
    armed: bool,
    air: Air,
    /// The latest hop: (takeoff song time, kind, press time, id). Kept after landing
    /// so an obstacle just passed is still credited to it.
    recent_hop: Option<(f32, Hop, f32, u32)>,
    /// Song time the hero last touched the slab; a buffered press takes off
    /// from there, so chained hops stay on the beat.
    ground_t: f32,
    /// Skip building the picture (the measure harness plays thousands of frames).
    headless: bool,
    /// Draw every model as primitives (a dev switch; also what a missing glb does).
    models_off: bool,
    touches: u32,
    /// Clears in a row without damage.
    streak: u32,
    y: f32,
    /// Smooths the hold→tap arc switch for the picture only.
    y_vis_off: f32,
    coyote: f32,
    buffer: f32,
    buffer_press_t: f32,
    down: bool,
    press_edge: bool,
    release_edge: bool,
    hop_id: u32,
    last_cleared_hop: i64,
    stray_check: Option<(u32, f32)>,
    lives: u32,
    invuln: f32,
    revives: u32,
    combo: u32,
    max_combo: u32,
    perfects: u32,
    stomps: u32,
    goods: u32,
    oks: u32,
    cleared: u32,
    beat_pts: f32,
    last_grade: Option<(Grade, f32, f32)>,
    grade_age: f32,
    beat_flash: f32,
    last_beat: i64,
    hit_flash: f32,
    land_squash: f32,
    /// The hero's squash and ear springs (`hop_anim`).
    anim: crate::hop_anim::HeroAnim,
    /// Visual-only hit-stop: the picture holds for this long while the sim and the music run on.
    vfreeze: f32,
    /// Where the hero was drawn when the freeze began (height, metres).
    freeze_y: f32,
    /// How far behind the true position the picture is while it catches up (metres).
    vlag: f32,
    /// Camera punch spring: FOV and dolly follow it.
    kick: crate::hop_anim::Wobble,
    /// Seconds since the last ring burst under the hero, and its colour.
    ring_age: f32,
    ring_col: [u32; 3],
    /// Combo glow, eased 0..1.
    combo_glow: f32,
    /// Enemy hints already shown this session, and the one on screen: (enemy type, seconds).
    hints_seen: Vec<bool>,
    hint: Option<(usize, f32)>,
    /// Page language for the HUD words (false = English).
    lang_ja: bool,
    /// The run beat the best score of the device (set at game over).
    new_best: bool,
    /// The running score passed the best: seconds of the "NEW BEST!" banner left, and whether it fired.
    best_banner: f32,
    best_passed: bool,
    /// Where the flyer we last warned about hovers (m), so the warning sounds once each.
    warned_x: f32,
    just_hop: bool,
    sfx: bool,
    time: f32,
    attract: f32,
    anim_t: f32,
    pending_finish: bool,
    /// Picture only: seconds since the bunny left the ledge, whether the ledge
    /// effects fired, how far the camera trails it, its fall speed, and the drop
    /// that plays on after the run has ended.
    fall_age: f32,
    fall_on: bool,
    fall_cam: f32,
    fall_vy: f32,
    dead_fall: Option<DeadFall>,
    /// The way back in after a fall that cost a life, or after a Continue.
    recover: Option<Recover>,
    last_loss: &'static str,
}

/// The colour a skin id paints the hero, or None for `classic` / an unknown id.
pub fn skin_tint(id: &str) -> Option<[u32; 3]> {
    SKINS.iter().find(|(n, _)| *n == id).and_then(|(_, c)| *c)
}

impl HopBeat {
    /// Switch the hero skin between runs (the shop / unlock hook). False for an unknown id.
    pub fn set_skin(&mut self, id: &str) -> bool {
        if !SKINS.iter().any(|(n, _)| *n == id) {
            return false;
        }
        self.spec.skin = id.into();
        self.rebuild();
        true
    }

    /// Switch the finishing look (a [`LOOK_PRESETS`] id) between runs. False for an unknown id.
    pub fn set_look(&mut self, id: &str) -> bool {
        if !LOOK_PRESETS.contains(&id) {
            return false;
        }
        self.spec.look.preset = id.into();
        self.rebuild();
        true
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("hop dump: {e}"))?;
        let spec = match value.get("game") {
            Some(g) => HopSpec::from_value(g)?,
            None => HopSpec::default(),
        };
        let mut run = Self::new(spec);
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            run.flow.best_score = best as u32;
        }
        run.load_progress();
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            run.flow.best_score = run.flow.best_score.max(best as u32);
        }
        if let Some(q) = value.get("quality").and_then(|v| v.as_str()) {
            let q = q.trim();
            if !q.is_empty() {
                run.picture_quality = Some(q.to_string());
            }
        }
        // Runtime picks from the page / save (`skin`, `look`); an unknown id keeps the spec's.
        if let Some(id) = value.get("skin").and_then(|v| v.as_str()) {
            run.set_skin(id);
        }
        if let Some(id) = value.get("look").and_then(|v| v.as_str()) {
            run.set_look(id);
        }
        if let Some(b) = value.get("fx_post").and_then(|v| v.as_bool()) {
            run.fx_post = b;
        }
        if let Some(b) = value.get("fx_atmo").and_then(|v| v.as_bool()) {
            run.fx_atmo = b;
        }
        if value.get("direct").and_then(|v| v.as_bool()) == Some(true) {
            run.spec.play.direct = true;
        }
        run.rebuild();
        Ok(run)
    }

    pub fn new(spec: HopSpec) -> Self {
        let mut flow = GameFlow::new(GAME_ID);
        flow.title = spec.title.to_uppercase();
        flow.tagline = "HOP ON THE BEAT".into();
        let course = Course::new(&spec);
        let mut core = Core::default();
        core.juice.seed = spec.course.seed ^ 0x9e37;
        let lives = spec.lives.count;
        Self {
            spec,
            flow,
            core,
            course,
            doc: WorldDoc::default(),
            picture_quality: None,
            fx_post: true,
            fx_atmo: true,
            song_t: 0.0,
            clock_base: 0.0,
            clock: None,
            latency_user_ms: 0.0,
            armed: false,
            air: Air::Ground,
            recent_hop: None,
            ground_t: f32::MIN,
            headless: false,
            models_off: false,
            touches: 0,
            streak: 0,
            y: 0.0,
            y_vis_off: 0.0,
            coyote: 0.0,
            buffer: 0.0,
            buffer_press_t: 0.0,
            down: false,
            press_edge: false,
            release_edge: false,
            hop_id: 0,
            last_cleared_hop: -1,
            stray_check: None,
            lives,
            invuln: 0.0,
            revives: 0,
            combo: 0,
            max_combo: 0,
            perfects: 0,
            stomps: 0,
            goods: 0,
            oks: 0,
            cleared: 0,
            beat_pts: 0.0,
            last_grade: None,
            grade_age: 9.0,
            beat_flash: 0.0,
            last_beat: -1,
            hit_flash: 0.0,
            land_squash: 0.0,
            anim: Default::default(),
            vfreeze: 0.0,
            freeze_y: 0.0,
            vlag: 0.0,
            kick: Default::default(),
            ring_age: 9.0,
            ring_col: [255, 255, 255],
            combo_glow: 0.0,
            hints_seen: Vec::new(),
            hint: None,
            lang_ja: false,
            new_best: false,
            best_banner: 0.0,
            best_passed: false,
            warned_x: f32::NEG_INFINITY,
            just_hop: false,
            sfx: true,
            time: 0.0,
            attract: 0.0,
            anim_t: 0.0,
            pending_finish: false,
            fall_age: 0.0,
            fall_on: false,
            fall_cam: 0.0,
            fall_vy: 0.0,
            dead_fall: None,
            recover: None,
            last_loss: "",
        }
    }

    // ---- small accessors ------------------------------------------------

    pub fn load_progress(&mut self) {
        let best = self.flow.best_score;
        self.flow.load_save();
        self.flow.best_score = self.flow.best_score.max(best);
    }
    pub fn phase_name(&self) -> &'static str {
        self.flow.phase.name()
    }
    pub fn playing(&self) -> bool {
        self.flow.phase == FlowPhase::Play && self.core.stats.alive
    }
    pub fn score(&self) -> u32 {
        self.core.stats.score
    }
    pub fn best_score(&self) -> u32 {
        self.flow.best_score
    }
    pub fn lives(&self) -> u32 {
        self.lives
    }
    /// Why the last life went: "fell" or "enemy" (for the measure bots).
    pub fn last_loss(&self) -> &'static str {
        self.last_loss
    }
    pub fn combo(&self) -> u32 {
        self.combo
    }
    pub fn max_combo(&self) -> u32 {
        self.max_combo
    }
    pub fn song_t(&self) -> f32 {
        self.song_t
    }
    pub fn hero_x(&self) -> f32 {
        self.song_t * self.spec.speed()
    }
    pub fn hero_y(&self) -> f32 {
        self.y
    }
    pub fn document(&self) -> &WorldDoc {
        &self.doc
    }
    pub fn course(&self) -> &Course {
        &self.course
    }
    pub fn death(&self) -> &str {
        &self.core.stats.death
    }
    pub fn set_sfx(&mut self, on: bool) {
        self.sfx = on;
    }
    pub fn stomps(&self) -> u32 {
        self.stomps
    }
    pub fn counts(&self) -> (u32, u32, u32, u32) {
        (self.cleared, self.perfects, self.goods, self.oks)
    }
    pub fn armed(&self) -> bool {
        self.armed
    }
    pub fn can_revive(&self) -> bool {
        self.flow.phase == FlowPhase::Result
            && self.spec.revive.enabled
            && self.revives < self.spec.revive.max_per_run
            && !self.core.stats.alive
    }

    fn latency_secs(&self) -> f32 {
        (self.spec.music.offset_ms + self.latency_user_ms) / 1000.0
    }

    /// `en` or `ja` by the page language.
    fn tr<'a>(&self, en: &'a str, ja: &'a str) -> &'a str {
        if self.lang_ja {
            ja
        } else {
            en
        }
    }

    /// `ja` / `en` (anything starting with "ja" is Japanese).
    pub fn set_lang(&mut self, lang: &str) {
        self.lang_ja = lang.to_ascii_lowercase().starts_with("ja");
    }

    /// Pause from the page or the button: let go of the hop and stop the music.
    pub fn pause(&mut self) {
        if self.flow.phase == FlowPhase::Play {
            self.touches = 0;
            self.set_hold(false);
            self.flow.pause();
        }
    }

    /// Back into the run from the page (the Escape key).
    pub fn resume_from_page(&mut self) {
        self.resume();
    }

    /// Back into the run with a second of grace.
    fn resume(&mut self) {
        if self.flow.phase == FlowPhase::Pause {
            self.flow.resume();
            self.grant_invuln(1.0);
        }
    }

    /// Hold the picture for `secs` (a hit-stop that leaves the sim and the music alone).
    fn freeze_picture(&mut self, secs: f32) {
        if self.vfreeze <= 0.0 && self.vlag < 0.02 {
            self.freeze_y = self.y + self.y_vis_off;
        }
        self.vfreeze = self.vfreeze.max(secs);
    }

    /// A burst ring under the hero (PERFECT, stomp, landing).
    fn ring(&mut self, color: [u32; 3]) {
        self.ring_age = 0.0;
        self.ring_col = color;
    }

    /// The nearest live enemy ahead of the hero within `max_m`, as (type, metres).
    /// `flyers` keeps only the ones that hover.
    fn enemy_ahead(&self, flyers: bool, max_m: f32) -> Option<(usize, f32)> {
        let hx = self.hero_x();
        let mut best: Option<(usize, f32)> = None;
        for o in self.course.obst.iter() {
            if o.kind != Kind::Enemy || o.fate != Fate::Pending {
                continue;
            }
            let Some(et) = self.spec.enemies.types.get(o.ety) else {
                continue;
            };
            if flyers && et.y_lo <= 0.0 {
                continue;
            }
            let d = (o.start + o.end) * 0.5 - hx;
            if d > 0.0 && d < max_m && best.is_none_or(|b| d < b.1) {
                best = Some((o.ety, d));
            }
        }
        best
    }

    /// Visual feel that does not touch the sim: the held picture, the camera
    /// spring, the ring, the combo glow and the first-sight hints.
    fn fx_step(&mut self, dt: f32) {
        if self.vfreeze > 0.0 {
            self.vlag += self.spec.speed() * dt;
            self.vfreeze -= dt;
            self.y_vis_off = self.freeze_y - self.y;
        } else {
            self.vlag *= (1.0 - dt * 14.0).max(0.0);
        }
        self.vlag = self.vlag.min(0.7);
        self.kick.step(0.0, 20.0, 0.5, dt);
        self.ring_age += dt;
        let want = if self.core.stats.alive {
            (self.combo as f32 / 12.0).min(1.0)
        } else {
            0.0
        };
        self.combo_glow += (want - self.combo_glow) * (1.0 - (-dt * 6.0).exp());
        if let Some((_, d)) = self.enemy_ahead(true, WARN_M) {
            let at = (self.hero_x() + d).round();
            if at != self.warned_x {
                self.warned_x = at;
                self.cue("warn");
            }
        }
        self.best_banner = (self.best_banner - dt).max(0.0);
        let best = self.flow.best_score;
        if !self.best_passed && best > 0 && self.core.stats.score > best {
            self.best_passed = true;
            self.best_banner = 2.4;
            self.ring([255, 236, 150]);
            self.kick.v += 3.0;
            self.cue("combo");
        }
        if let Some((e, age)) = self.hint {
            self.hint = (age + dt < 3.0).then_some((e, age + dt));
        } else if let Some((e, d)) = self.enemy_ahead(false, 34.0) {
            if self.hints_seen.len() <= e {
                self.hints_seen.resize(e + 1, false);
            }
            if !self.hints_seen[e] && d > 8.0 {
                self.hints_seen[e] = true;
                self.hint = Some((e, 0.0));
            }
        }
    }

    fn beat_pos(&self) -> f32 {
        self.song_t / self.spec.beat_secs()
    }

    fn cue(&self, name: &str) {
        if self.sfx {
            crate::mixer::cue_sfx(name);
        }
    }

    // ---- host API --------------------------------------------------------

    /// The audio clock: seconds of music heard since the BGM started. The
    /// page calls this once a frame while the music plays.
    pub fn set_music_clock(&mut self, secs: Option<f32>) {
        self.clock = secs.filter(|s| s.is_finite());
    }

    /// The player's own latency setting, ms (positive = picture later).
    pub fn set_latency_ms(&mut self, ms: f32) {
        self.latency_user_ms = if ms.is_finite() {
            ms.clamp(-300.0, 300.0)
        } else {
            0.0
        };
    }

    pub fn latency_ms(&self) -> f32 {
        self.latency_user_ms
    }

    /// The one button. `true` on touch down / key down, `false` on release.
    /// Direct-into-play: the next run waits for the first tap instead of
    /// starting at once.
    pub fn set_direct(&mut self, on: bool) {
        self.spec.play.direct = on;
    }

    pub fn set_headless(&mut self, on: bool) {
        self.headless = on;
    }

    /// Force the primitive picture (the fallback a missing glb gets).
    pub fn set_models_off(&mut self, off: bool) {
        self.models_off = off;
    }

    /// A finger went down / came up. Several fingers count as one held button.
    pub fn touch_begin(&mut self) {
        self.touches += 1;
        self.set_hold(true);
    }

    pub fn touch_end(&mut self) {
        self.touches = self.touches.saturating_sub(1);
        if self.touches == 0 {
            self.set_hold(false);
        }
    }

    /// Raw pointer for the session: menus and confirm outside play, the one
    /// button during it.
    pub fn pointer_event(
        &mut self,
        phase: crate::input::PointerPhase,
        x: f32,
        y: f32,
        w: u32,
        h: u32,
    ) {
        use crate::input::PointerPhase as P;
        match (self.flow.phase, phase) {
            (FlowPhase::Play, P::Begin) => self.press(x, y, w, h),
            (FlowPhase::Play, P::End | P::Cancel) => self.touch_end(),
            (FlowPhase::Title | FlowPhase::Result, P::Begin) => {
                if !self.pointer(x, y, w, h) {
                    self.confirm();
                }
            }
            (FlowPhase::Pause, P::Begin) => {
                self.pause_hit(x, y, w, h);
            }
            (_, P::End | P::Cancel) => self.touch_end(),
            _ => {}
        }
    }

    /// A finger down in play: the pause button pauses, anywhere else is the hop.
    fn press(&mut self, x: f32, y: f32, w: u32, h: u32) {
        if pause_button(w as f32, h as f32).contains(x, y) {
            self.pause();
        } else {
            self.touch_begin();
        }
    }

    /// A tap on the pause card.
    fn pause_hit(&mut self, x: f32, y: f32, w: u32, h: u32) -> bool {
        let btn = pause_layout(w as f32, h as f32)
            .into_iter()
            .find(|(_, r)| r.contains(x, y))
            .map(|(b, _)| b);
        match btn {
            Some(PauseBtn::Resume) => self.resume(),
            Some(PauseBtn::Retry) => self.start_run(),
            Some(PauseBtn::Title) => self.show_title(),
            Some(PauseBtn::SyncMinus) => self.set_latency_ms(self.latency_user_ms - 10.0),
            Some(PauseBtn::SyncPlus) => self.set_latency_ms(self.latency_user_ms + 10.0),
            None => return false,
        }
        true
    }

    pub fn hold_down(&self) -> bool {
        self.down
    }

    pub fn set_hold(&mut self, down: bool) {
        if down && !self.down {
            self.press_edge = true;
        }
        if !down && self.down {
            self.release_edge = true;
        }
        self.down = down;
    }

    pub fn confirm(&mut self) {
        match self.flow.phase {
            FlowPhase::Title | FlowPhase::Result => self.start_run(),
            FlowPhase::Pause | FlowPhase::Play => {}
        }
    }

    pub fn show_title(&mut self) {
        self.flow.show_title();
        self.core.stats.alive = true;
        self.core.stats.score = 0;
        self.armed = false;
    }

    pub fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) -> bool {
        if self.flow.phase == FlowPhase::Pause {
            return self.pause_hit(x, y, width, height);
        }
        let Some(action) = self.flow.hit(x, y, width, height) else {
            return false;
        };
        self.apply_action(action);
        true
    }

    pub fn on_key(&mut self, name: &str, down: bool) -> bool {
        let key = name.to_ascii_lowercase();
        if matches!(
            key.as_str(),
            "space" | " " | "arrowup" | "up" | "w" | "j" | "z"
        ) {
            if self.flow.phase == FlowPhase::Play {
                self.set_hold(down);
                return true;
            }
            if down {
                if let Some(a) = self.flow.on_key(if key == " " { "space" } else { &key }) {
                    self.apply_action(a);
                    return true;
                }
            }
            return false;
        }
        if !down {
            return false;
        }
        if let Some(action) = self.flow.on_key(&key) {
            self.apply_action(action);
            return true;
        }
        false
    }

    fn apply_action(&mut self, action: FlowAction) {
        match action {
            FlowAction::Start | FlowAction::Retry => self.start_run(),
            FlowAction::Resume => self.resume(),
            FlowAction::ToTitle => self.show_title(),
            FlowAction::Pause => self.pause(),
            FlowAction::VolumeUp => self.flow.bump_master(0.1),
            FlowAction::VolumeDown => self.flow.bump_master(-0.1),
            FlowAction::ToggleMute => self.flow.toggle_mute(),
        }
    }

    fn start_run(&mut self) {
        let keep = (
            self.flow.best_score,
            self.latency_user_ms,
            self.picture_quality.take(),
        );
        let spec = self.spec.clone();
        let flow = std::mem::replace(&mut self.flow, GameFlow::new(GAME_ID));
        let mut fresh = Self::new(spec);
        fresh.flow = flow;
        fresh.flow.best_score = keep.0;
        fresh.latency_user_ms = keep.1;
        fresh.picture_quality = keep.2;
        fresh.sfx = self.sfx;
        fresh.models_off = self.models_off;
        fresh.fx_post = self.fx_post;
        fresh.lang_ja = self.lang_ja;
        fresh.hints_seen = std::mem::take(&mut self.hints_seen);
        fresh.fx_atmo = self.fx_atmo;
        fresh.down = self.down;
        *self = fresh;
        self.flow.enter_play();
        crate::mixer::stop_bgm(0.0);
        if self.spec.play.direct {
            self.armed = true;
        } else {
            self.begin();
        }
        self.course
            .ensure(&self.spec, AHEAD_M / self.spec.hero.beat_len_m + 8.0);
        self.rebuild();
    }

    /// The music starts and the song clock runs.
    fn begin(&mut self) {
        self.armed = false;
        self.clock_base = self.song_t;
        crate::mixer::play_bgm("bgm", 0.05);
    }

    /// Continue after game over: lives back, the music restarts on the next
    /// bar line, and a few beats of empty slab let the player find the hop.
    pub fn revive(&mut self) -> bool {
        if !self.can_revive() {
            return false;
        }
        self.revives += 1;
        self.recover = None;
        let breather = self.spec.recover.enabled && self.spec.recover.ready_beats > 0.0;
        if !breather {
            // (With a READY count `begin_ready` puts the bunny on the bar line and clears the slab.)
            let bar = self.spec.beat_secs() * self.spec.music.beats_per_bar as f32;
            self.song_t = (self.song_t / bar).ceil() * bar;
            let beat = self.beat_pos();
            let clear = beat + self.spec.revive.clear_beats;
            self.course.clear_before(clear);
            self.course
                .ensure(&self.spec, beat + AHEAD_M / self.spec.hero.beat_len_m + 8.0);
        }
        self.core.stats.alive = true;
        self.core.stats.death.clear();
        self.pending_finish = false;
        self.new_best = false;
        self.core.juice.hitstop = 0.0;
        self.dead_fall = None;
        self.fall_age = 0.0;
        self.fall_on = false;
        self.fall_cam = 0.0;
        self.vfreeze = 0.0;
        self.vlag = 0.0;
        self.lives = self
            .spec
            .revive
            .lives
            .min(self.spec.lives.count.max(self.spec.revive.lives));
        self.invuln = self.spec.revive.invuln_secs;
        self.combo = 0;
        self.air = Air::Ground;
        self.y = 0.0;
        self.flow.phase = FlowPhase::Play;
        self.flow.subtitle.clear();
        crate::mixer::stop_bgm(0.0);
        self.cue("revive");
        let rc = self.spec.recover.clone();
        if breather {
            // A beat to breathe after the ad: READY, then the music from the bar line.
            let (clear, inv) = (self.spec.revive.clear_beats, self.spec.revive.invuln_secs);
            self.begin_ready(rc.ready_beats, clear, inv);
        } else {
            self.begin();
        }
        self.rebuild();
        true
    }

    pub fn tick(&mut self, dt: f32) {
        tick_frame(self, dt);
    }

    // ---- play step -------------------------------------------------------

    fn grant_invuln(&mut self, secs: f32) {
        self.invuln = self.invuln.max(secs);
    }

    fn lose_life(&mut self, reason: &str) {
        if self.invuln > 0.0 || !self.core.stats.alive {
            return;
        }
        self.combo = 0;
        self.streak = 0;
        let fell = reason == "fell";
        self.last_loss = if fell { "fell" } else { "enemy" };
        if !fell {
            // A fall has its own picture (tumble, trailing camera): no red flash or camera snap.
            self.hit_flash = 1.0;
            self.anim.on_hit();
            self.kick.v -= 6.0;
        }
        self.lives = self.lives.saturating_sub(1);
        if self.lives == 0 {
            self.core.stats.die(reason);
            if fell {
                // The sim is done; the drop plays on, then the card comes about
                // `card_delay_secs` after the ledge.
                let wait = (self.spec.fall.card_delay_secs - self.fall_age).max(0.15);
                self.core.juice.hitstop = wait;
                self.core.juice.shake = self.core.juice.shake.max(0.12);
                self.vfreeze = 0.0;
                self.dead_fall = Some(DeadFall {
                    age: 0.0,
                    cam: self.fall_cam,
                    vy: self.fall_vy,
                    t0: self.fall_age,
                });
            } else {
                self.core.juice.hit(0.7, 0.14);
                self.cue("hit");
            }
            self.pending_finish = true;
            // The stinger for a run that beat the device best, the other one otherwise.
            let best = self.flow.best_score;
            self.new_best = best > 0 && self.core.stats.score > best;
            self.cue(if self.new_best { "newbest" } else { "gameover" });
            return;
        }
        if !fell {
            self.cue("hit");
            self.core.juice.shake = self.core.juice.shake.max(0.35);
            self.freeze_picture(0.09);
        }
        if fell && self.spec.recover.enabled {
            self.begin_drop();
            return;
        }
        self.grant_invuln(self.spec.lives.invuln_secs);
        if fell {
            // Second wind: the hero floats until there is slab again.
            self.air = Air::Ground;
            self.y = 0.0;
        }
    }

    /// A fall that cost a life but not the run: the drop plays out (slowed, the same picture
    /// as a fatal fall), the music fades, and the way back in starts (`step_recover`).
    fn begin_drop(&mut self) {
        let rc = self.spec.recover.clone();
        self.dead_fall = Some(DeadFall {
            age: 0.0,
            cam: self.fall_cam,
            vy: self.fall_vy,
            t0: self.fall_age,
        });
        self.core.juice.shake = self.core.juice.shake.max(0.12);
        self.vfreeze = 0.0;
        self.recover = Some(Recover {
            stage: RecoverStage::Drop,
            left: rc.drop_secs,
            total: 0.0,
            ticked: 0,
            invuln: rc.invuln_secs,
        });
        crate::mixer::stop_bgm(0.25);
    }

    /// The drop is over: put the bunny on empty slab at the next bar line and hold the world
    /// for READY (`beats` of it). The music starts again when the count reaches 0.
    fn begin_ready(&mut self, beats: f32, clear_beats: f32, invuln: f32) {
        let bar = self.spec.beat_secs() * self.spec.music.beats_per_bar as f32;
        self.song_t = (self.song_t / bar).ceil() * bar;
        let beat = self.beat_pos();
        self.course.clear_before(beat + clear_beats);
        self.course
            .ensure(&self.spec, beat + AHEAD_M / self.spec.hero.beat_len_m + 8.0);
        self.dead_fall = None;
        self.fall_age = 0.0;
        self.fall_on = false;
        self.fall_cam = 0.0;
        self.fall_vy = 0.0;
        self.vfreeze = 0.0;
        self.vlag = 0.0;
        self.combo = 0;
        self.air = Air::Ground;
        self.y = 0.0;
        self.y_vis_off = 0.0;
        self.kick.v = 0.0;
        self.press_edge = false;
        self.release_edge = false;
        self.buffer = 0.0;
        self.coyote = 0.0;
        self.ground_t = self.song_t;
        // The blink starts with READY, so the bunny reads as "not yet".
        self.invuln = invuln.max(0.01);
        let total = beats * self.spec.beat_secs();
        self.recover = Some(Recover {
            stage: RecoverStage::Ready,
            left: total,
            total,
            ticked: 0,
            invuln,
        });
        crate::mixer::stop_bgm(0.0);
        self.rebuild();
    }

    /// READY is over: the music starts on the bar line and play resumes.
    fn finish_ready(&mut self, invuln: f32) {
        self.recover = None;
        self.invuln = invuln;
        self.begin();
        self.beat_flash = 1.0;
        self.cue("perfect");
    }

    /// One step of the way back in. The world is held: no scroll, no obstacles, no clock.
    fn step_recover(&mut self, dt: f32) {
        self.anim_t += dt;
        self.hit_flash = (self.hit_flash - dt).max(0.0);
        self.beat_flash = (self.beat_flash - dt * 4.0).max(0.0);
        let Some(mut r) = self.recover else { return };
        r.left -= dt;
        match r.stage {
            RecoverStage::Drop => {
                let slow = self.spec.fall.slow_scale;
                if let Some(d) = self.dead_fall.as_mut() {
                    d.age += dt * slow;
                }
                if r.left <= 0.0 {
                    self.begin_ready(
                        self.spec.recover.ready_beats,
                        self.spec.recover.clear_beats,
                        r.invuln,
                    );
                    if self.spec.recover.ready_beats <= 0.0 {
                        self.finish_ready(r.invuln);
                    }
                    return;
                }
            }
            RecoverStage::Ready => {
                let bs = self.spec.beat_secs();
                let k = ((r.total - r.left) / bs).floor() as i32;
                if k > r.ticked && r.left > 0.0 {
                    r.ticked = k;
                    self.beat_flash = 1.0;
                    self.cue("land");
                }
                if r.left <= 0.0 {
                    self.finish_ready(r.invuln);
                    return;
                }
            }
        }
        self.recover = Some(r);
    }

    fn start_hop(&mut self, press_t: f32) {
        self.hop_id += 1;
        let hop = if self.down { Hop::Hold } else { Hop::Tap };
        let t0 = press_t.max(self.ground_t).min(self.song_t);
        self.air = Air::Hop {
            t0,
            hop,
            press_t,
            id: self.hop_id,
        };
        self.recent_hop = Some((t0, hop, press_t, self.hop_id));
        self.coyote = 0.0;
        self.buffer = 0.0;
        self.just_hop = true;
        self.anim.on_takeoff();
        self.core.stats.just_jump = true;
        self.stray_check = None;
        self.cue("hop");
    }

    fn land(&mut self, id: u32) {
        self.land_squash = 1.0;
        self.anim.on_land(1.0);
        self.kick.v += 1.0;
        self.cue("land");
        let t = self.song_t + 0.2;
        self.stray_check = Some((id, t));
        let hx = self.hero_x();
        self.core
            .juice
            .burst(hx, 0.05, 0.0, 4, self.spec.look.glow, 1.4, 0.07);
    }

    fn judge(&mut self, o: &Obst, press_t: f32, used: Hop) {
        let bs = self.spec.beat_secs();
        let air = self.spec.arc(used).air_beats;
        let center = (o.start + o.end) * 0.5 / self.spec.hero.beat_len_m;
        let ideal = (center - air * 0.5) * bs;
        let err = press_t - ideal;
        let ms = err.abs() * 1000.0;
        let j = &self.spec.judge;
        let grade = if used != o.hop {
            Grade::Ok
        } else if ms <= j.perfect_ms {
            Grade::Perfect
        } else if ms <= j.good_ms {
            Grade::Good
        } else {
            Grade::Ok
        };
        let pts = match grade {
            Grade::Perfect => {
                self.combo += 1;
                self.perfects += 1;
                j.score_perfect + j.combo_bonus * self.combo.min(j.combo_cap)
            }
            Grade::Good => {
                self.goods += 1;
                j.score_good
            }
            Grade::Ok => {
                self.oks += 1;
                self.combo = 0;
                j.score_ok
            }
            Grade::Stomp => 0,
        };
        self.max_combo = self.max_combo.max(self.combo);
        self.cleared += 1;
        self.streak += 1;
        let every = self.spec.judge.regain_clears;
        if every > 0 && self.streak.is_multiple_of(every) && self.lives < self.spec.lives.count {
            self.lives += 1;
            self.cue("combo");
        }
        self.core.stats.pickup(pts);
        self.last_grade = Some((grade, err, 0.0));
        self.grade_age = 0.0;
        self.cue(match grade {
            Grade::Perfect if self.combo > 0 && self.combo.is_multiple_of(4) => "combo",
            Grade::Perfect => "perfect",
            Grade::Good => "good",
            Grade::Ok | Grade::Stomp => "land",
        });
        let hx = self.hero_x();
        match grade {
            Grade::Perfect => {
                let (c, n) = (self.spec.look.accent, 7);
                self.core.juice.burst(hx, 0.3, 0.0, n, c, 2.8, 0.09);
                self.kick.v += 3.5;
                self.ring([255, 236, 150]);
            }
            Grade::Good => {
                self.kick.v += 1.4;
                self.ring(self.spec.look.glow);
            }
            _ => {}
        }
    }

    fn step_play(&mut self, dt: f32) {
        let bs = self.spec.beat_secs();
        let v = self.spec.speed();
        let hero = self.spec.hero.clone();
        // ---- clock
        let mut dt_eff = dt;
        if let Some(c) = self.clock.take() {
            let target = c + self.clock_base - self.latency_secs();
            let err = target - self.song_t;
            if err.abs() > 0.25 {
                self.song_t = target.max(0.0);
                self.grant_invuln(1.2);
            } else {
                dt_eff = dt * (1.0 + (err * 4.0).clamp(-0.06, 0.06));
            }
        }
        let dt = dt_eff;
        self.song_t += dt;
        self.time += dt;
        self.course.ensure(
            &self.spec,
            self.beat_pos() + AHEAD_M / hero.beat_len_m + 4.0,
        );
        let x = self.song_t * v;
        self.course.prune(x - BEHIND_M - 10.0);
        // beat flash + score per beat
        let b = self.beat_pos().floor() as i64;
        if b != self.last_beat {
            self.last_beat = b;
            self.beat_flash = 1.0;
            self.beat_pts += self.spec.judge.per_beat;
            let whole = self.beat_pts.floor();
            if whole >= 1.0 {
                self.core.stats.score += whole as u32;
                self.beat_pts -= whole;
            }
        }
        self.beat_flash = (self.beat_flash - dt * 4.0).max(0.0);
        self.invuln = (self.invuln - dt).max(0.0);
        self.hit_flash = (self.hit_flash - dt * 3.0).max(0.0);
        self.grade_age += dt;
        self.land_squash = (self.land_squash - dt * 6.0).max(0.0);
        self.y_vis_off *= (1.0 - dt * 14.0).max(0.0);
        self.coyote = (self.coyote - dt).max(0.0);

        // ---- input
        let press_t = self.song_t;
        if self.press_edge {
            self.press_edge = false;
            self.buffer = hero.buffer_secs;
            self.buffer_press_t = press_t;
        }
        let can_hop = matches!(self.air, Air::Ground) || self.coyote > 0.0;
        if self.buffer > 0.0 {
            if can_hop {
                let pt = self.buffer_press_t;
                if !matches!(self.air, Air::Ground) {
                    self.y = 0.0;
                }
                self.start_hop(pt);
            } else {
                self.buffer -= dt;
            }
        }
        if self.release_edge {
            self.release_edge = false;
            if let Air::Hop {
                t0,
                hop: Hop::Hold,
                press_t,
                id,
            } = self.air
            {
                if self.song_t - t0 <= hero.tap_window_secs {
                    // Early release: the hop becomes the short one.
                    let before = self.hop_y(t0, Hop::Hold);
                    self.air = Air::Hop {
                        t0,
                        hop: Hop::Tap,
                        press_t,
                        id,
                    };
                    self.recent_hop = Some((t0, Hop::Tap, press_t, id));
                    let after = self.hop_y(t0, Hop::Tap);
                    self.y_vis_off += before - after;
                }
            }
        }

        // ---- vertical
        match self.air {
            Air::Ground => {
                self.y = 0.0;
                if !supported(&self.course.obst, x, hero.edge_tol) {
                    if self.invuln > 0.0 {
                        // floating over a gap on the second wind
                    } else {
                        self.air = Air::Fall { vy: 0.0 };
                        self.coyote = hero.coyote_secs;
                    }
                }
            }
            Air::Hop { t0, hop, id, .. } => {
                let arc = self.spec.arc(hop).clone();
                let air = arc.air_beats * bs;
                let el = self.song_t - t0;
                if el >= air {
                    self.y = 0.0;
                    if supported(&self.course.obst, x, hero.edge_tol) || self.invuln > 0.0 {
                        self.ground_t = t0 + air;
                        self.air = Air::Ground;
                        self.land(id);
                    } else {
                        self.air = Air::Fall { vy: 0.0 };
                    }
                } else {
                    self.y = arc_y(arc.height, air, el);
                }
            }
            Air::Bounce { t0, dur, y0, id } => {
                let u = (self.song_t - t0) / dur;
                if u >= 1.0 {
                    self.y = 0.0;
                    if supported(&self.course.obst, x, hero.edge_tol) || self.invuln > 0.0 {
                        self.ground_t = t0 + dur;
                        self.air = Air::Ground;
                        self.land(id);
                    } else {
                        self.air = Air::Fall { vy: 0.0 };
                    }
                } else {
                    self.y = bounce_y(y0, self.spec.enemies.bounce_height, u);
                }
            }
            Air::Fall { vy } => {
                let vy = vy - hero.fall_gravity * dt;
                self.y += vy * dt;
                self.air = Air::Fall { vy };
                self.fall_vy = vy;
                self.fall_age += dt;
                if !self.fall_on && self.y < -0.12 && self.invuln <= 0.0 {
                    self.fall_on = true;
                    self.ledge_fx();
                }
                if self.fall_on {
                    self.fall_cam += v * self.spec.fall.cam_lag * dt;
                }
                if self.y <= 0.0 && self.y > -0.4 && supported(&self.course.obst, x, hero.edge_tol)
                {
                    self.y = 0.0;
                    self.ground_t = self.song_t;
                    self.air = Air::Ground;
                    self.land_squash = 0.6;
                    self.anim.on_land(0.6);
                } else if self.y < -0.35 && self.invuln > 0.0 {
                    self.y = 0.0;
                    self.air = Air::Ground;
                } else if self.y < -3.5 {
                    self.y = -3.5;
                    self.lose_life("fell");
                    if self.core.stats.alive && self.recover.is_none() {
                        self.y = 0.0;
                        self.air = Air::Ground;
                    }
                }
            }
        }

        if !matches!(self.air, Air::Fall { .. }) {
            self.fall_age = 0.0;
            self.fall_on = false;
            self.fall_cam *= (1.0 - dt * 10.0).max(0.0);
        }
        let grounded = matches!(self.air, Air::Ground);
        let (bp, bf) = (self.beat_pos(), self.beat_flash);
        if self.vfreeze <= 0.0 {
            self.anim.update(dt, self.y, grounded, bp, bf);
        }
        self.fx_step(dt);

        // ---- obstacles
        let hw = hero.half_width;
        let y = self.y;
        let desc = self.descending();
        let t_now = self.song_t;
        let mut hit: Vec<usize> = Vec::new();
        let mut stomped: Vec<usize> = Vec::new();
        let mut resolved: Vec<usize> = Vec::new();
        for (i, o) in self.course.obst.iter().enumerate() {
            if o.fate != Fate::Pending {
                continue;
            }
            if o.start > x + 20.0 {
                break;
            }
            match self.spec.touch(o, t_now, x, y, desc, hw) {
                Touch::Hit => hit.push(i),
                Touch::Stomp => stomped.push(i),
                Touch::None => {}
            }
            if x > o.end + hw {
                resolved.push(i);
            }
        }
        if !hit.is_empty() {
            let enemy_hit = hit.iter().any(|&i| self.course.obst[i].kind == Kind::Enemy);
            // While ghosting (after a hit or a revive) they pass through.
            let hurt = self.invuln <= 0.0;
            if hurt {
                self.lose_life(if enemy_hit { "enemy" } else { "block" });
            }
            for &i in &hit {
                self.course.obst[i].fate = Fate::Skipped;
                if hurt {
                    self.course.obst[i].hit_t = Some(self.song_t);
                }
            }
            stomped.retain(|i| !hit.contains(i));
        }
        if self.invuln > 0.0 {
            for &i in &stomped {
                self.course.obst[i].fate = Fate::Skipped;
            }
            stomped.clear();
        }
        if !stomped.is_empty() && self.core.stats.alive {
            self.stomp(&stomped);
        }
        for i in resolved {
            if self.course.obst[i].fate != Fate::Pending {
                continue;
            }
            self.course.obst[i].fate = Fate::Cleared;
            let o = self.course.obst[i].clone();
            if !self.spec.needs_press(&o) {
                // A flyer passed underneath: nothing to judge.
                continue;
            }
            if self.invuln > 0.0 && !matches!(self.air, Air::Hop { .. }) {
                continue;
            }
            // The hop that carried the hero over it.
            let mid = (o.start + o.end) * 0.5 / v;
            let used = self.hop_over(mid);
            if let Some((hop, pt, id)) = used {
                self.last_cleared_hop = id as i64;
                self.judge(&o, pt, hop);
            }
        }
        if let Some((id, t)) = self.stray_check {
            if self.song_t > t {
                self.stray_check = None;
                if self.last_cleared_hop != id as i64 && self.combo > 0 {
                    self.combo = 0;
                }
            }
        }
        // juice sparks trail
        self.anim_t += dt;
        if matches!(self.air, Air::Ground) && self.core.stats.alive {
            let n = (self.anim_t * 30.0) as u32;
            if n.is_multiple_of(3) && self.core.juice.sparks.len() < 12 {
                let c = self.spec.look.glow;
                let hx = self.hero_x();
                self.core.juice.burst(hx - 0.2, 0.05, 0.0, 1, c, 0.6, 0.05);
            }
        }
    }

    /// Is the hero coming down (a stomp only counts from above)?
    fn descending(&self) -> bool {
        match self.air {
            Air::Hop { t0, hop, .. } => {
                let air = self.spec.arc(hop).air_beats * self.spec.beat_secs();
                self.song_t - t0 > air * 0.5
            }
            Air::Bounce { t0, dur, y0, .. } => {
                let u = (self.song_t - t0) / dur;
                bounce_dy(y0, self.spec.enemies.bounce_height, u) < 0.0
            }
            _ => false,
        }
    }

    /// Landed on top of enemies: squash them, score, and bounce to the next half beat.
    fn stomp(&mut self, which: &[usize]) {
        let bonus = self.spec.enemies.stomp_score;
        let j = self.spec.judge.clone();
        let (px, py) = (self.hero_x(), self.y);
        for &i in which {
            self.course.obst[i].fate = Fate::Cleared;
            self.course.obst[i].stomped_t = Some(self.song_t);
            self.combo += 1;
            self.stomps += 1;
            self.cleared += 1;
            self.streak += 1;
            let pts = bonus + j.combo_bonus * self.combo.min(j.combo_cap);
            self.core.stats.pickup(pts);
        }
        self.max_combo = self.max_combo.max(self.combo);
        let every = j.regain_clears;
        if every > 0 && self.streak.is_multiple_of(every) && self.lives < self.spec.lives.count {
            self.lives += 1;
            self.cue("combo");
        }
        self.last_grade = Some((Grade::Stomp, 0.0, 0.0));
        self.grade_age = 0.0;
        self.cue("stomp");
        self.anim.on_stomp();
        self.kick.v += 6.0;
        self.freeze_picture(0.07);
        self.ring(self.spec.look.accent);
        let c = self.spec.look.accent;
        self.core.juice.burst(px, py, 0.0, 9, c, 3.0, 0.1);
        self.core.juice.shake = self.core.juice.shake.max(0.12);
        let id = match self.air {
            Air::Hop { id, .. } | Air::Bounce { id, .. } => id,
            _ => self.hop_id,
        };
        self.last_cleared_hop = id as i64;
        self.stray_check = None;
        let dur = self.spec.bounce_secs(self.song_t);
        self.air = Air::Bounce {
            t0: self.song_t,
            dur,
            y0: self.y,
            id,
        };
        self.buffer = 0.0;
    }

    /// The hop (kind, press time, id) in the air around song time `t`.
    fn hop_over(&self, t: f32) -> Option<(Hop, f32, u32)> {
        let bs = self.spec.beat_secs();
        let (t0, hop, press_t, id) = self.recent_hop?;
        let air = self.spec.arc(hop).air_beats * bs;
        if t0 <= t + 0.2 && t0 + air + 0.2 >= t {
            Some((hop, press_t, id))
        } else {
            None
        }
    }

    fn hop_y(&self, t0: f32, hop: Hop) -> f32 {
        let arc = self.spec.arc(hop);
        arc_y(
            arc.height,
            arc.air_beats * self.spec.beat_secs(),
            self.song_t - t0,
        )
    }
}

// ------------------------------------------------------------- Kit impl ---

impl Kit for HopBeat {
    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }
    fn flow(&self) -> &GameFlow {
        &self.flow
    }
    fn flow_mut(&mut self) -> &mut GameFlow {
        &mut self.flow
    }
    fn substeps(&self, _dt: f32) -> u32 {
        1
    }
    fn step(&mut self, dt: f32) {
        if self.recover.is_some() {
            self.step_recover(dt);
            return;
        }
        if self.armed {
            self.anim_t += dt;
            if self.press_edge {
                // The first tap starts the music (it is the page's audio gesture).
                self.press_edge = false;
                self.release_edge = false;
                self.begin();
            }
            return;
        }
        self.step_play(dt);
    }
    fn attract(&mut self, dt: f32) {
        self.attract += dt;
        self.anim_t += dt;
        self.press_edge = false;
        self.release_edge = false;
    }
    fn hitstop_frame(&mut self, dt: f32) {
        self.anim_t += dt * 0.3;
        self.hit_flash = (self.hit_flash - dt).max(0.0);
        let slow = self.spec.fall.slow_scale;
        if let Some(d) = self.dead_fall.as_mut() {
            d.age += dt * slow;
        }
    }
    fn follow(&mut self, _dt: f32) {}
    fn blocks_spark(&self, _x: f32, _y: f32, _z: f32) -> bool {
        false
    }
    fn rebuild(&mut self) {
        if !self.headless {
            self.build_doc();
        }
    }
}

// ------------------------------------------------------------ JSON / HUD --

impl HopBeat {
    pub fn dump_json(&self) -> String {
        let body = self.doc.to_json().unwrap_or_else(|_| "{}".into());
        let sim = serde_json::to_string(&self.state_value()).unwrap_or_else(|_| "{}".into());
        match body.strip_prefix('{') {
            Some(rest) => format!("{{\n  \"sim\": {sim},{rest}"),
            None => body,
        }
    }

    /// The baked world file for this spec: `{"version", "genre", "game"}`.
    pub fn world_value(&self) -> serde_json::Value {
        // Via a string so f32 fields print as 0.3, not 0.30000001192092896.
        let game: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&self.spec).unwrap_or_default())
                .unwrap_or_default();
        json!({"version": 1, "genre": GAME_ID, "game": game})
    }

    pub fn state_json(&self) -> String {
        self.state_value().to_string()
    }

    fn result_value(&self) -> serde_json::Value {
        json!({
            "key": format!("{}:{}", self.spec.id, self.spec.course.seed),
            "title": self.spec.title,
            "enabled": true,
            "top_n": 5,
            "columns": ["score", "combo", "perfect"],
            "sort": "score",
            "run": {
                "score": self.core.stats.score,
                "combo": self.max_combo,
                "perfect": self.perfects,
                "stomps": self.stomps,
            },
            "can_revive": self.can_revive(),
            "new_best": self.new_best,
        })
    }

    fn state_value(&self) -> serde_json::Value {
        let beat = self.beat_pos();
        json!({
            "phase": self.flow.phase.name(),
            "score": self.core.stats.score,
            "best": self.flow.best_score,
            "lives": self.lives,
            "lives_max": self.spec.lives.count,
            "combo": self.combo,
            "max_combo": self.max_combo,
            "cleared": self.cleared,
            "perfect": self.perfects,
            "stomps": self.stomps,
            "good": self.goods,
            "ok": self.oks,
            "alive": self.core.stats.alive,
            "death": self.core.stats.death,
            "death_text": if self.core.stats.death == "enemy" { "You ran into an enemy (hop over it, or land on its head)" } else { "" },
            "armed": self.armed,
            "direct": self.spec.play.direct,
            "invulnerable": self.invuln > 0.0,
            "recover": match self.recover {
                Some(Recover { stage: RecoverStage::Drop, .. }) => "drop",
                Some(Recover { stage: RecoverStage::Ready, .. }) => "ready",
                None => "",
            },
            "ready_left": self.recover.map_or(0.0, |r| if r.stage == RecoverStage::Ready { r.left } else { 0.0 }),
            "song_t": self.song_t,
            // Where the music file should be right now (the inverse of the
            // clock sync), so a page that restarts the source can seek to it.
            "music_t": (self.song_t - self.clock_base + self.latency_secs()).max(0.0),
            "music_loop_secs": self.spec.music.loop_bars as f32 * self.spec.music.beats_per_bar as f32 * self.spec.beat_secs(),
            "beat": beat,
            "bpm": self.spec.music.bpm,
            "latency_ms": self.latency_user_ms + self.spec.music.offset_ms,
            // The player's own setting (what the pause screen steps and the page stores).
            "sync_ms": self.latency_user_ms,
            "new_best": self.new_best,
            "can_revive": self.can_revive(),
            "revives": self.revives,
            "x": self.hero_x(),
            "y": self.y,
            "grounded": matches!(self.air, Air::Ground),
            "falling": self.fall_on || self.dead_fall.is_some(),
            "result": self.result_value(),
        })
    }

    pub fn hud_json(&self, width: u32, height: u32) -> String {
        let value = serde_json::from_str::<serde_json::Value>(&crate::flow::draw_list_json(
            &self.hud(width, height, true),
        ))
        .unwrap_or_else(|_| json!({"quads": [], "texts": []}));
        value.to_string()
    }

    /// The pause screen: resume / retry / title and the audio sync stepper.
    fn hud_pause(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        let glow = self.spec.look.glow.map(|v| v as u8);
        let accent = self.spec.look.accent.map(|v| v as u8);
        list.quads.push(Quad::new(0.0, 0.0, w, h, [6, 6, 20, 205]));
        let mut text = |t: &str, x: f32, y: f32, size: f32, c: [u8; 4]| {
            list.texts
                .push(TextQuad::new(t, x, y, size * s, c).aligned(TextAlign::Center));
        };
        text(
            self.tr("PAUSED", "ポーズ"),
            w * 0.5,
            h * 0.12,
            30.0,
            [255, 255, 255, 255],
        );
        let sync = self.latency_user_ms.round() as i32;
        let layout = pause_layout(w, h);
        let mut sync_y = 0.0;
        let mut sync_x = (0.0, 0.0);
        for (b, r) in &layout {
            let (label, fill) = match b {
                PauseBtn::Resume => (
                    self.tr("RESUME", "つづける"),
                    [accent[0], accent[1], accent[2], 255],
                ),
                PauseBtn::Retry => (self.tr("RETRY", "リトライ"), [40, 34, 74, 245]),
                PauseBtn::Title => (self.tr("TITLE", "タイトル"), [40, 34, 74, 245]),
                PauseBtn::SyncMinus => ("−", [40, 34, 74, 245]),
                PauseBtn::SyncPlus => ("+", [40, 34, 74, 245]),
            };
            list.quads.push(Quad::new(
                r.x - 2.0 * s,
                r.y - 2.0 * s,
                r.w + 4.0 * s,
                r.h + 4.0 * s,
                [glow[0], glow[1], glow[2], 150],
            ));
            list.quads.push(Quad::new(r.x, r.y, r.w, r.h, fill));
            let ink = if *b == PauseBtn::Resume {
                [20, 12, 30, 255]
            } else {
                [255, 255, 255, 255]
            };
            let size = if matches!(b, PauseBtn::SyncMinus | PauseBtn::SyncPlus) {
                24.0
            } else {
                17.0
            };
            list.texts.push(
                TextQuad::new(
                    label,
                    r.x + r.w * 0.5,
                    r.y + r.h * 0.5 - size * s * 0.5,
                    size * s,
                    ink,
                )
                .aligned(TextAlign::Center),
            );
            if *b == PauseBtn::SyncMinus {
                sync_y = r.y;
                sync_x.0 = r.x + r.w;
            }
            if *b == PauseBtn::SyncPlus {
                sync_x.1 = r.x;
            }
        }
        let cx = (sync_x.0 + sync_x.1) * 0.5;
        let mut text = |t: &str, x: f32, y: f32, size: f32, c: [u8; 4]| {
            list.texts
                .push(TextQuad::new(t, x, y, size * s, c).aligned(TextAlign::Center));
        };
        text(
            self.tr("AUDIO SYNC", "音ズレ補正"),
            w * 0.5,
            sync_y - 22.0 * s,
            11.0,
            [glow[0], glow[1], glow[2], 230],
        );
        text(
            &format!("{}{} ms", if sync > 0 { "+" } else { "" }, sync),
            cx,
            sync_y + 12.0 * s,
            17.0,
            [255, 255, 255, 255],
        );
        text(
            self.tr(
                "Hopping late? Press +.  Early? Press −.",
                "遅れるなら＋、早すぎなら−",
            ),
            w * 0.5,
            sync_y + 54.0 * s,
            10.0,
            [255, 255, 255, 150],
        );
    }

    /// Grade popup near the hero, the enemy warning and hint, and the hurt vignette.
    fn hud_popups(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        let glow = self.spec.look.glow.map(|v| v as u8);
        let accent = self.spec.look.accent.map(|v| v as u8);
        // Text with a dark drop shadow so it reads on any sky.
        let shadowed = |list: &mut DrawList, t: &str, x: f32, y: f32, size: f32, c: [u8; 4]| {
            let sh = [6, 4, 20, (c[3] as f32 * 0.8) as u8];
            list.texts.push(
                TextQuad::new(t, x + 2.0 * s, y + 2.0 * s, size * s, sh).aligned(TextAlign::Center),
            );
            list.texts
                .push(TextQuad::new(t, x, y, size * s, c).aligned(TextAlign::Center));
        };
        if let Some((g, err, _)) = self.last_grade {
            let a = self.grade_age;
            if a < 0.8 {
                let fade = if a < 0.5 { 1.0 } else { 1.0 - (a - 0.5) / 0.3 };
                let al = (255.0 * fade).clamp(0.0, 255.0) as u8;
                // Pops in big, settles, floats up.
                let pop = 1.0 + 0.7 * (1.0 - a / 0.12).max(0.0);
                let y = h * 0.45 - a * 46.0 * s;
                let (word, c, size) = match g {
                    Grade::Perfect => ("PERFECT!", [255, 236, 150, al], 28.0),
                    Grade::Good => ("GOOD", [glow[0], glow[1], glow[2], al], 22.0),
                    Grade::Ok => ("OK", [200, 200, 220, al], 18.0),
                    Grade::Stomp => ("STOMP!", [accent[0], accent[1], accent[2], al], 28.0),
                };
                shadowed(list, word, w * 0.5, y, size * pop, c);
                if matches!(g, Grade::Good | Grade::Ok) {
                    let side = if err < 0.0 {
                        self.tr("EARLY", "はやい")
                    } else {
                        self.tr("LATE", "おそい")
                    };
                    shadowed(
                        list,
                        side,
                        w * 0.5,
                        y + 28.0 * s,
                        12.0,
                        [c[0], c[1], c[2], al / 2],
                    );
                }
            }
        }
        // A flyer is close: a flashing badge.
        if let Some((e, d)) = self.enemy_ahead(true, WARN_M) {
            let on = ((self.beat_pos() * 2.0).fract()) < 0.6;
            let col = self
                .spec
                .enemies
                .types
                .get(e)
                .map(|t| t.color.map(|v| v as u8))
                .unwrap_or([255, 200, 60]);
            let near = (1.0 - d / WARN_M).clamp(0.0, 1.0);
            let bw = (44.0 + 14.0 * near) * s;
            let (bx, by) = (w * 0.5 - bw * 0.5, h * 0.17);
            let a = if on { 235 } else { 120 };
            list.quads.push(Quad::new(
                bx - 3.0 * s,
                by - 3.0 * s,
                bw + 6.0 * s,
                bw + 6.0 * s,
                [255, 255, 255, a / 2],
            ));
            list.quads
                .push(Quad::new(bx, by, bw, bw, [col[0], col[1], col[2], a]));
            list.texts.push(
                TextQuad::new(
                    "!",
                    w * 0.5,
                    by + bw * 0.12,
                    bw / s * 0.62 * s,
                    [20, 8, 30, 255],
                )
                .aligned(TextAlign::Center),
            );
        }
        // First sight of a kind of enemy: one line on what to do.
        if let Some((e, age)) = self.hint {
            if let Some(et) = self.spec.enemies.types.get(e) {
                let (en, ja) = match (et.look.as_str(), et.stompable) {
                    ("bat", _) => (
                        "FLYING! TAP, DON'T HOLD",
                        "とんでる！ホールドしないでタップ",
                    ),
                    (_, true) if et.step_m > 0.0 => (
                        "IT WALKS AT YOU. LAND ON ITS HEAD!",
                        "あるいてくる！あたまにのろう！",
                    ),
                    (_, true) => (
                        "HOP OVER IT, OR LAND ON ITS HEAD!",
                        "とびこえるか、あたまにのろう！",
                    ),
                    _ => ("HOP OVER IT!", "とびこえよう！"),
                };
                let fade = (age.min(3.0 - age) * 4.0).clamp(0.0, 1.0);
                let al = (240.0 * fade) as u8;
                list.quads.push(Quad::new(
                    0.0,
                    h * 0.265,
                    w,
                    34.0 * s,
                    [8, 6, 24, (140.0 * fade) as u8],
                ));
                shadowed(
                    list,
                    self.tr(en, ja),
                    w * 0.5,
                    h * 0.265 + 8.0 * s,
                    13.0,
                    [255, 255, 255, al],
                );
            }
        }
        // Hurt: red closes in from the edges and fades.
        if self.hit_flash > 0.02 {
            let f = self.hit_flash.clamp(0.0, 1.0);
            let m = w.min(h);
            for k in 0..4 {
                let t = m * (0.05 + 0.035 * k as f32);
                let a = (150.0 * f / (1.0 + k as f32 * 0.9)) as u8;
                let c = [230, 20, 40, a];
                list.quads.push(Quad::new(0.0, 0.0, w, t, c));
                list.quads.push(Quad::new(0.0, h - t, w, t, c));
                list.quads.push(Quad::new(0.0, t, t, h - 2.0 * t, c));
                list.quads.push(Quad::new(w - t, t, t, h - 2.0 * t, c));
            }
        }
        // A fall: the screen edge closes in, dark and cool (no red: nothing hit it).
        let dark = self.fall_dark();
        if dark > 0.02 {
            let m = w.min(h);
            for k in 0..5 {
                let t = m * (0.06 + 0.05 * k as f32);
                let a = (170.0 * dark / (1.0 + k as f32 * 0.8)) as u8;
                let c = [6, 4, 22, a];
                list.quads.push(Quad::new(0.0, 0.0, w, t, c));
                list.quads.push(Quad::new(0.0, h - t, w, t, c));
                list.quads.push(Quad::new(0.0, t, t, h - 2.0 * t, c));
                list.quads.push(Quad::new(w - t, t, t, h - 2.0 * t, c));
            }
        }
    }

    /// `menu` draws the built-in title / pause / result cards; a page that
    /// brings its own passes false.
    pub fn hud(&self, width: u32, height: u32, menu: bool) -> DrawList {
        let mut list = DrawList {
            clear: [0, 0, 0, 0],
            quads: Vec::new(),
            texts: Vec::new(),
            images: Vec::new(),
        };
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let s = (w.min(h) / 400.0).clamp(0.9, 2.6);
        let txt =
            |list: &mut DrawList, t: &str, x: f32, y: f32, size: f32, c: [u8; 4], a: TextAlign| {
                list.texts
                    .push(TextQuad::new(t, x, y, size * s, c).aligned(a));
            };
        let glow = self.spec.look.glow.map(|v| v as u8);
        let accent = self.spec.look.accent.map(|v| v as u8);
        if self.flow.phase == FlowPhase::Play {
            let pad = 10.0 * s;
            let top = 8.0 * s;
            let ink = [glow[0], glow[1], glow[2], 255];
            // Score: a dark plate with an accent edge, label above the number.
            list.quads
                .push(Quad::new(pad, top, 148.0 * s, 46.0 * s, [8, 6, 24, 165]));
            list.quads.push(Quad::new(
                pad,
                top,
                4.0 * s,
                46.0 * s,
                [accent[0], accent[1], accent[2], 255],
            ));
            txt(
                &mut list,
                self.tr("SCORE", "スコア"),
                pad + 14.0 * s,
                top + 5.0 * s,
                9.0,
                [255, 255, 255, 170],
                TextAlign::Left,
            );
            txt(
                &mut list,
                &format!("{}", self.core.stats.score),
                pad + 14.0 * s,
                top + 17.0 * s,
                20.0,
                ink,
                TextAlign::Left,
            );
            if self.flow.best_score > 0 {
                txt(
                    &mut list,
                    &format!("{} {}", self.tr("BEST", "ベスト"), self.flow.best_score),
                    pad + 2.0 * s,
                    top + 50.0 * s,
                    9.0,
                    [255, 255, 255, 140],
                    TextAlign::Left,
                );
            }
            // Pause button, then the lives to its left.
            let pb = pause_button(w, h);
            list.quads
                .push(Quad::new(pb.x, pb.y, pb.w, pb.h, [8, 6, 24, 175]));
            for k in 0..2 {
                list.quads.push(Quad::new(
                    pb.x + pb.w * (0.30 + 0.24 * k as f32),
                    pb.y + pb.h * 0.26,
                    pb.w * 0.16,
                    pb.h * 0.48,
                    [255, 255, 255, 235],
                ));
            }
            let blink = self.invuln > 0.0 && ((self.anim_t * 8.0) as u32).is_multiple_of(2);
            let pip = |list: &mut DrawList, x: f32, on: bool| {
                let c = if on && !blink {
                    [255, 112, 150, 255]
                } else if on {
                    [255, 255, 255, 140]
                } else {
                    [40, 36, 64, 190]
                };
                list.quads.push(Quad::new(
                    x - 1.5 * s,
                    top + 10.0 * s - 1.5 * s,
                    17.0 * s,
                    17.0 * s,
                    [255, 255, 255, if on { 110 } else { 40 }],
                ));
                list.quads
                    .push(Quad::new(x, top + 10.0 * s, 14.0 * s, 14.0 * s, c));
            };
            let lives_max = self.spec.lives.count.max(self.lives);
            let mut x = pb.x - 10.0 * s - 14.0 * s;
            if lives_max <= 5 {
                for i in 0..lives_max {
                    pip(&mut list, x, i < self.lives);
                    x -= 20.0 * s;
                }
            } else {
                // Many lives: one pip and a count.
                pip(&mut list, x, self.lives > 0);
                txt(
                    &mut list,
                    &format!("×{}", self.lives),
                    x - 8.0 * s,
                    top + 8.0 * s,
                    16.0,
                    [255, 255, 255, 240],
                    TextAlign::Right,
                );
            }
            // Combo: big multiplier that pulses on the beat; the colour climbs with it.
            if self.combo >= 2 {
                let pulse = 1.0 + 0.22 * self.beat_flash;
                let c = match self.combo {
                    0..=3 => [255, 244, 190, 255],
                    4..=7 => [255, 214, 90, 255],
                    8..=15 => [255, 150, 90, 255],
                    _ => [255, 110, 200, 255],
                };
                let y = h * 0.095;
                let size = (26.0 + (self.combo.min(20) as f32) * 0.5) * pulse;
                txt(
                    &mut list,
                    &format!("×{}", self.combo),
                    w * 0.5 + 2.0 * s,
                    y + 2.0 * s,
                    size,
                    [6, 4, 20, 190],
                    TextAlign::Center,
                );
                txt(
                    &mut list,
                    &format!("×{}", self.combo),
                    w * 0.5,
                    y,
                    size,
                    c,
                    TextAlign::Center,
                );
                txt(
                    &mut list,
                    self.tr("COMBO", "コンボ"),
                    w * 0.5,
                    y + (size + 2.0) * s,
                    10.0,
                    [c[0], c[1], c[2], 200],
                    TextAlign::Center,
                );
            }
            self.hud_popups(&mut list, w, h, s);
            // beat dots
            let bpb = self.spec.music.beats_per_bar.max(1);
            let cur = (self.beat_pos().floor().max(0.0) as u32) % bpb;
            for i in 0..bpb {
                let lit = i == cur;
                let a = if lit {
                    (140.0 + 115.0 * (1.0 - self.beat_flash)) as u8
                } else {
                    70
                };
                let size = if lit { 13.0 } else { 9.0 } * s;
                let cx = w * 0.5 + (i as f32 - (bpb as f32 - 1.0) * 0.5) * 22.0 * s;
                list.quads.push(Quad::new(
                    cx - size * 0.5,
                    h - 26.0 * s - size * 0.5,
                    size,
                    size,
                    if i == 0 {
                        [accent[0], accent[1], accent[2], a]
                    } else {
                        [glow[0], glow[1], glow[2], a]
                    },
                ));
            }
            if let Some(r) = self.recover.filter(|r| r.stage == RecoverStage::Ready) {
                // READY and the beats left (3, 2, 1) while the world is held.
                let bs = self.spec.beat_secs();
                let n = (r.left / bs).ceil().max(1.0) as u32;
                list.quads
                    .push(Quad::new(0.0, h * 0.36, w, h * 0.20, [8, 6, 24, 150]));
                txt(
                    &mut list,
                    self.tr("READY", "よーい"),
                    w * 0.5,
                    h * 0.385,
                    16.0,
                    [glow[0], glow[1], glow[2], 235],
                    TextAlign::Center,
                );
                txt(
                    &mut list,
                    &n.to_string(),
                    w * 0.5,
                    h * 0.43,
                    34.0,
                    [255, 255, 255, 255],
                    TextAlign::Center,
                );
            }
            if self.armed {
                list.quads
                    .push(Quad::new(0.0, h * 0.40, w, h * 0.16, [8, 6, 24, 150]));
                txt(
                    &mut list,
                    self.tr("TAP TO START", "タップでスタート"),
                    w * 0.5,
                    h * 0.44,
                    22.0,
                    [255, 255, 255, 255],
                    TextAlign::Center,
                );
                txt(
                    &mut list,
                    self.tr(
                        "TAP = SHORT HOP   HOLD = LONG HOP",
                        "タップ＝小ジャンプ　長押し＝大ジャンプ",
                    ),
                    w * 0.5,
                    h * 0.50,
                    11.0,
                    [glow[0], glow[1], glow[2], 230],
                    TextAlign::Center,
                );
            }
        }
        if self.flow.phase == FlowPhase::Pause {
            self.hud_pause(&mut list, w, h, s);
        } else if menu {
            if self.flow.phase == FlowPhase::Result {
                list.quads.push(Quad::new(0.0, 0.0, w, h, [8, 0, 16, 150]));
            }
            self.flow.paint(&mut list, width, height);
        } else if self.flow.phase == FlowPhase::Result {
            list.quads.push(Quad::new(0.0, 0.0, w, h, [8, 0, 16, 120]));
        }
        list
    }
}

// ------------------------------------------------------------------- ui ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PauseBtn {
    Resume,
    Retry,
    Title,
    SyncMinus,
    SyncPlus,
}

fn ui_scale(w: f32, h: f32) -> f32 {
    (w.min(h) / 400.0).clamp(0.9, 2.6)
}

/// The touch pause button, top right (with a little slop around it).
fn pause_button(w: f32, h: f32) -> Quad {
    let s = ui_scale(w, h);
    Quad::new(
        w - 10.0 * s - 38.0 * s,
        8.0 * s,
        38.0 * s,
        38.0 * s,
        [0, 0, 0, 0],
    )
}

/// The pause screen's hit areas, pixels.
fn pause_layout(w: f32, h: f32) -> Vec<(PauseBtn, Quad)> {
    let s = ui_scale(w, h);
    let bw = 230.0 * s;
    let bh = 42.0 * s;
    let gap = 10.0 * s;
    let x = (w - bw) * 0.5;
    let mut y = h * 0.22;
    let mut v = Vec::new();
    for b in [PauseBtn::Resume, PauseBtn::Retry, PauseBtn::Title] {
        v.push((b, Quad::new(x, y, bw, bh, [0; 4])));
        y += bh + gap;
    }
    y += 40.0 * s;
    let sb = 46.0 * s;
    v.push((PauseBtn::SyncMinus, Quad::new(x, y, sb, sb, [0; 4])));
    v.push((
        PauseBtn::SyncPlus,
        Quad::new(x + bw - sb, y, sb, sb, [0; 4]),
    ));
    v
}

// -------------------------------------------------------------- picture ---

#[allow(clippy::too_many_arguments)]
fn prop(
    id: &str,
    model: &str,
    name: &str,
    pos: [f32; 3],
    scale: [f32; 3],
    color: [u32; 3],
    emissive: f32,
    rough: f32,
    metal: f32,
) -> WorldProp {
    WorldProp {
        id: format!("prop:{id}"),
        kind: "prop".into(),
        name: name.into(),
        position: pos,
        model: model.into(),
        scale,
        enabled: true,
        color: Some(color),
        emissive: (emissive > 0.01).then_some(emissive),
        roughness: rough,
        metallic: metal,
        is_static: true,
        collider: Some("none".into()),
        ..WorldProp::default()
    }
}

fn mix(a: [u32; 3], b: [u32; 3], t: f32) -> [u32; 3] {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u32)
}

#[allow(clippy::too_many_arguments)]
fn light(
    slot: u32,
    name: &str,
    pos: [f32; 3],
    dir: [f32; 3],
    color: [f32; 3],
    intensity: f32,
    radius: f32,
    spot: bool,
) -> WorldLight {
    WorldLight {
        id: format!("light:{name}"),
        kind_type: "light".into(),
        name: name.into(),
        position: pos,
        kind: if spot { "spot".into() } else { "point".into() },
        slot,
        intensity,
        radius,
        color: Some(color),
        direction: Some(dir),
    }
}

/// Track lateral half-width.
const TRACK_HALF: f32 = 1.7;

/// The models (`tools/gen_assets/hop_models.py`). Each one is optional: when a
/// file is missing or does not parse the picture falls back to primitives.
const BUNNY_GLB: &str = "assets/models/hop_beat/hop_bunny.glb";
const POST_GLB: &str = "assets/models/hop_beat/hop_post.glb";
const SLIME_GLB: &str = "assets/models/hop_beat/hop_slime.glb";
const BEETLE_GLB: &str = "assets/models/hop_beat/hop_beetle.glb";
/// The ears, a separate mesh whose origin is the pivot at the top of the head
/// (`EAR_PIVOT`, in the body's own units).
const EARS_GLB: &str = "assets/models/hop_beat/hop_ears.glb";
const EAR_PIVOT: [f32; 3] = [0.0, 0.80, 0.26];
/// A flyer this far ahead (metres, about 4 beats) starts to warn.
const WARN_M: f32 = 26.0;
const BAT_GLB: &str = "assets/models/hop_beat/hop_bat.glb";
/// Skyline variants: (tower, lit windows), unit footprint 3 x 3 m, base on y = 0.
const TOWER_GLBS: [(&str, &str); 3] = [
    (
        "assets/models/hop_beat/hop_tower_a.glb",
        "assets/models/hop_beat/hop_windows_a.glb",
    ),
    (
        "assets/models/hop_beat/hop_tower_b.glb",
        "assets/models/hop_beat/hop_windows_b.glb",
    ),
    (
        "assets/models/hop_beat/hop_tower_c.glb",
        "assets/models/hop_beat/hop_windows_c.glb",
    ),
];

/// True when `path` is a mounted or on-disk GLB that parses. A miss is not
/// cached, so a wasm page can mount the file after boot.
fn glb_ready(path: &str) -> bool {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static OK: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    static WARNED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let ok = OK.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(guard) = ok.lock() {
        if guard.contains(path) {
            return true;
        }
    }
    let parsed = crate::assets::resolve_asset(path)
        .and_then(|file| crate::assets::read_bytes(&file).ok())
        .map(|bytes| crate::gltf_load::mesh_from_glb(&bytes).is_ok())
        .unwrap_or(false);
    if parsed {
        if let Ok(mut guard) = ok.lock() {
            guard.insert(path.to_string());
        }
        return true;
    }
    // Say so once per file: a missing model is a quiet downgrade, not a crash.
    let warned = WARNED.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut guard) = warned.lock() {
        if guard.insert(path.to_string()) {
            let msg = format!("hop beat: model {path} is not available, using primitives");
            #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
            web_sys::console::warn_1(&msg.into());
            #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
            eprintln!("{msg}");
        }
    }
    false
}

/// A `prop` drawn from a GLB instead of a primitive.
#[allow(clippy::too_many_arguments)]
fn glb_prop(
    id: &str,
    path: &str,
    name: &str,
    pos: [f32; 3],
    scale: [f32; 3],
    color: [u32; 3],
    emissive: f32,
    emissive_color: Option<[f32; 3]>,
) -> WorldProp {
    let mut p = prop(id, "box", name, pos, scale, color, emissive, 0.6, 0.1);
    p.gltf = Some(path.to_string());
    p.emissive_color = emissive_color;
    p
}

impl HopBeat {
    /// Where the camera looks from. The course runs along +Z.
    fn view_x(&self) -> f32 {
        match self.flow.phase {
            FlowPhase::Title => 0.0,
            _ => self.hero_x() - self.vlag + self.fall_view().0,
        }
    }

    /// While the bunny falls the camera trails it: (shift of the camera along the
    /// track, how far the bunny is ahead of the camera). Both 0 outside a fall.
    fn fall_view(&self) -> (f32, f32) {
        if let Some(d) = self.dead_fall {
            let f = &self.spec.fall;
            let dx = self.spec.speed() * f.carry * (1.0 - (-3.0 * d.age).exp()) / 3.0;
            (-d.cam + (1.0 - f.cam_lag) * dx, d.cam + f.cam_lag * dx)
        } else {
            (-self.fall_cam, self.fall_cam)
        }
    }

    /// Seconds of the fall so far (the sim part, then the slowed drop).
    fn fall_clock(&self) -> f32 {
        match self.dead_fall {
            Some(d) => d.t0 + d.age,
            None if self.fall_on => self.fall_age,
            None => 0.0,
        }
    }

    /// Dark screen edge for a fall, 0..1.
    fn fall_dark(&self) -> f32 {
        if self.dead_fall.is_none() && !self.fall_on {
            return 0.0;
        }
        (self.fall_clock() / 0.7).clamp(0.0, 1.0) * self.spec.fall.vignette
    }

    /// The bunny leaves the ledge: whoosh, a short picture hold, dust and a glint.
    fn ledge_fx(&mut self) {
        self.cue("fall");
        self.freeze_picture(self.spec.fall.edge_stop_secs);
        let hx = self.hero_x();
        let n = match self.picture_quality.as_deref() {
            Some("low") => self.spec.fall.dust / 2,
            _ => self.spec.fall.dust,
        };
        self.core
            .juice
            .burst(hx, 0.05, 0.0, n, [196, 188, 220], 2.2, 0.11);
        self.core
            .juice
            .burst(hx, 0.2, 0.0, n / 3, self.spec.look.glow, 3.0, 0.07);
    }

    fn build_doc(&mut self) {
        let look = self.spec.look.clone();
        let hx = self.view_x();
        let bl = self.spec.hero.beat_len_m;
        let beat = hx / bl;
        let pulse = self.beat_flash;
        let fin = look.finish();
        // ?fx_post=0: the plain picture (no bloom, grade, vignette or tone curve).
        let post_on = self.fx_post;
        // ?fx_atmo=0: the old sky and haze, no stars, clouds or horizon glow.
        let atmo_on = self.fx_atmo;
        let sky = if atmo_on { fin.sky } else { [14, 10, 40] };
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: 90.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([sky[0] as u8, sky[1] as u8, sky[2] as u8, 255]),
            hemi_sky: Some(fin.hemi_sky),
            hemi_ground: Some(fin.hemi_ground),
            env_map: Some("preset:studio".into()),
            shadow_softness: Some(1.0),
            bloom: Some(if post_on { fin.bloom } else { 0.0 }),
            bloom_threshold: Some(fin.bloom_threshold),
            vignette: Some(if post_on { fin.vignette } else { 0.0 }),
            lut: Some(if post_on {
                fin.lut.clone()
            } else {
                "off".into()
            }),
            lut_strength: Some(if post_on { fin.lut_strength } else { 0.0 }),
            fxaa: Some(true),
            exposure: Some(if post_on { fin.exposure } else { 1.0 }),
            ibl: Some(fin.ibl),
            tonemap: Some(post_on),
            tonemap_curve: if post_on && !fin.tonemap_curve.is_empty() {
                Some(fin.tonemap_curve.clone())
            } else {
                None
            },
            cull: Some(true),
            prop_lod: Some(90.0),
            fog_start: Some(if atmo_on { fin.fog_start } else { 34.0 }),
            fog_end: Some(if atmo_on { fin.fog_end } else { 92.0 }),
            ..WorldDoc::default()
        };
        doc.quality = self.picture_quality.clone();
        let sh = self.core.juice.shake.min(1.0) * 0.07;
        let shake = [
            (self.anim_t * 61.0).sin() * sh,
            (self.anim_t * 47.0).sin() * sh,
        ];
        let lift = self.y.max(0.0) * 0.35;
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "chase".into(),
            position: [
                shake[0],
                3.1 + lift * 0.5 + shake[1],
                hx - 6.6 - 1.2 * self.kick.x,
            ],
            target: [0.0, 0.5 + lift, hx + 7.5],
            // The beat breathes the lens a little; a PERFECT or a stomp punches it.
            fov: 64.0 + 24.0 * self.kick.x.clamp(-0.18, 0.2) + 0.7 * pulse * pulse,
            roll: 0.0,
        });
        doc.lights.push(light(
            0,
            "key",
            [-3.0, 9.0, hx - 6.0],
            [0.18, -0.85, 0.5],
            [0.75, 0.82, 1.0],
            1.0,
            48.0,
            true,
        ));
        doc.lights.push(light(
            1,
            "hero",
            [0.0, self.y + 1.6, hx],
            [0.0; 3],
            [1.0, 0.82, 0.5],
            1.4,
            7.0,
            false,
        ));
        doc.lights.push(light(
            2,
            "rim",
            [0.0, 2.5, hx + 9.0],
            [0.0; 3],
            [1.0, 0.3, 0.6],
            1.0,
            14.0,
            false,
        ));

        // The renderer rebuilds its glTF slot table whenever the list of distinct
        // models changes. One tiny buried prop per model, first in the list, keeps
        // the table constant while towers and posts scroll in and out.
        let mut models: Vec<&str> = vec![
            BUNNY_GLB, EARS_GLB, POST_GLB, SLIME_GLB, BEETLE_GLB, BAT_GLB,
        ];
        for (tower, windows) in TOWER_GLBS {
            models.push(tower);
            models.push(windows);
        }
        let models: Vec<&str> = models
            .into_iter()
            .filter(|m| !self.models_off && glb_ready(m))
            .collect();
        for (i, m) in models.iter().enumerate() {
            doc.props.push(glb_prop(
                &format!("anchor-{i}"),
                m,
                "anchor",
                [0.0, -60.0, -60.0],
                [1e-4; 3],
                [0, 0, 0],
                0.0,
                None,
            ));
        }
        let have_post = models.contains(&POST_GLB);

        // ---- slabs between the gaps
        let lo = hx - BEHIND_M;
        let hi = hx + AHEAD_M;
        let mut gaps: Vec<(f32, f32)> = self
            .course
            .obst
            .iter()
            .filter(|o| o.kind == Kind::Gap && o.end > lo && o.start < hi)
            .map(|o| (o.start, o.end))
            .collect();
        gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut cursor = lo;
        let mut slabs: Vec<(f32, f32)> = Vec::new();
        for (s, e) in gaps.iter().copied() {
            if s > cursor {
                slabs.push((cursor, s));
            }
            cursor = cursor.max(e);
        }
        if cursor < hi {
            slabs.push((cursor, hi));
        }
        for (i, (a, b)) in slabs.iter().enumerate() {
            doc.props.push(prop(
                &format!("slab-{i}"),
                "box",
                "slab",
                [0.0, -0.5, (a + b) * 0.5],
                [TRACK_HALF * 2.0, 1.0, (b - a).max(0.05)],
                look.slab,
                0.0,
                0.45,
                0.1,
            ));
            // glowing edges so a gap reads from far away
            for (k, z) in [*a, *b].into_iter().enumerate() {
                let near_gap = (z - lo).abs() > 0.01 && (z - hi).abs() > 0.01;
                if near_gap {
                    doc.props.push(prop(
                        &format!("edge-{i}-{k}"),
                        "box",
                        "edge",
                        [0.0, 0.01, z],
                        [TRACK_HALF * 2.0, 0.04, 0.14],
                        look.accent,
                        1.3,
                        0.3,
                        0.0,
                    ));
                }
            }
        }
        for (i, (s, e)) in gaps.iter().enumerate() {
            doc.props.push(prop(
                &format!("void-{i}"),
                "box",
                "void",
                [0.0, -3.6, (s + e) * 0.5],
                [TRACK_HALF * 2.0, 0.1, e - s],
                look.accent,
                0.5 + 0.5 * pulse,
                0.5,
                0.0,
            ));
        }
        // ---- beat lines across the slab: the one that reaches the hero is the beat
        let first = beat.floor() as i64 - 4;
        for bi in first..first + (AHEAD_M / bl) as i64 + 8 {
            let z = bi as f32 * bl;
            if z < lo || z > hi || bi < 0 {
                continue;
            }
            if gaps.iter().any(|(s, e)| z > *s - 0.1 && z < *e + 0.1) {
                continue;
            }
            let bar = (bi as u32).is_multiple_of(self.spec.music.beats_per_bar.max(1));
            let near = (1.0 - ((z - hx).abs() / 3.0)).clamp(0.0, 1.0);
            doc.props.push(prop(
                &format!("beat-{bi}"),
                "box",
                "beatline",
                [0.0, 0.02, z],
                [TRACK_HALF * 2.0 - 0.1, 0.03, if bar { 0.16 } else { 0.08 }],
                if bar { look.accent } else { look.glow },
                0.5 + near * (1.0 + pulse),
                0.3,
                0.0,
            ));
        }
        // ---- blocks
        for (i, o) in self.course.obst.iter().enumerate() {
            if o.kind != Kind::Block
                || o.end < lo
                || o.start > hi
                || o.fate == Fate::Skipped && o.end < hx - 2.0
            {
                continue;
            }
            let hot = if o.fate == Fate::Pending { 1.2 } else { 0.4 };
            doc.props.push(prop(
                &format!("block-{i}"),
                "box",
                "block",
                [0.0, o.height * 0.5, (o.start + o.end) * 0.5],
                [TRACK_HALF * 2.0 - 0.3, o.height, (o.end - o.start).max(0.1)],
                look.accent,
                hot,
                0.35,
                0.2,
            ));
        }
        // ---- enemies
        self.push_enemies(&mut doc, &models, lo, hi);
        // ---- lanterns: speed cue, pulse with the beat
        let step = 2.0 * bl;
        let mut z = (lo / step).ceil() * step;
        let mut n = 0;
        while z < hi {
            let k = (z / step) as i64;
            let on = (1.0 - ((z - hx).abs() / 6.0)).clamp(0.0, 1.0);
            for side in [-1.0_f32, 1.0] {
                if have_post {
                    doc.props.push(glb_prop(
                        &format!("post-{n}"),
                        POST_GLB,
                        "post",
                        [side * (TRACK_HALF + 0.9), 0.0, z],
                        [1.0; 3],
                        [255, 255, 255],
                        0.0,
                        None,
                    ));
                } else {
                    doc.props.push(prop(
                        &format!("post-{n}"),
                        "box",
                        "post",
                        [side * (TRACK_HALF + 0.9), 0.9, z],
                        [0.22, 1.8, 0.22],
                        mix(look.slab, [255, 255, 255], 0.1),
                        0.0,
                        0.5,
                        0.2,
                    ));
                }
                doc.props.push(prop(
                    &format!("lamp-{n}"),
                    "sphere",
                    "lamp",
                    [side * (TRACK_HALF + 0.9), 1.95, z],
                    [0.32, 0.32, 0.32],
                    if k % 2 == 0 { look.glow } else { look.accent },
                    0.9 + 1.2 * (pulse * on).max(on * 0.3),
                    0.3,
                    0.0,
                ));
                n += 1;
            }
            z += step;
        }
        // ---- far scenery: a moon and a low skyline that move slower than the track
        doc.props.push(prop(
            "moon",
            "sphere",
            "moon",
            [-12.0, 15.0, hx + 60.0],
            [7.0, 7.0, 7.0],
            [255, 236, 200],
            1.1 + 0.4 * pulse,
            0.6,
            0.0,
        ));
        for i in 0..14 {
            let sx = if i % 2 == 0 { -1.0 } else { 1.0 };
            let off = 9.0 + ((i * 7) % 5) as f32 * 3.5;
            let hgt = 3.0 + ((i * 13) % 7) as f32 * 1.8;
            let zz = (hx * 0.9 / 12.0).floor() * 12.0 + i as f32 * 8.0 - 20.0 + hx * 0.1 % 12.0;
            let tw = if atmo_on {
                fin.twinkle * 0.12 * (self.anim_t * (1.1 + 0.23 * i as f32) + i as f32 * 1.9).sin()
            } else {
                0.0
            };
            let lit = (0.15 + 0.2 * (((i + (beat as i32)) % 3) as f32 / 3.0) + tw).max(0.02);
            let (tower, windows) = TOWER_GLBS[(i as usize) % TOWER_GLBS.len()];
            if models.contains(&tower) && models.contains(&windows) {
                // The model's own height, not a stretched box: the roof clutter keeps its shape.
                let k = 1.0 + ((i * 5) % 3) as f32 * 0.15;
                let tint = mix([255, 255, 255], look.accent, ((i * 3) % 5) as f32 * 0.05);
                doc.props.push(glb_prop(
                    &format!("tower-{i}"),
                    tower,
                    "tower",
                    [sx * off, -1.0, zz],
                    [1.0, k, 1.0],
                    tint,
                    0.0,
                    None,
                ));
                let glow = if i % 3 == 0 { look.glow } else { look.accent };
                let c = glow.map(|v| (v as f32 / 255.0).powf(2.2));
                doc.props.push(glb_prop(
                    &format!("windows-{i}"),
                    windows,
                    "windows",
                    [sx * off, -1.0, zz],
                    [1.0, k, 1.0],
                    mix([255, 255, 255], glow, 0.7),
                    0.7 + 1.6 * lit,
                    Some(c),
                ));
            } else {
                doc.props.push(prop(
                    &format!("tower-{i}"),
                    "box",
                    "tower",
                    [sx * off, hgt * 0.5 - 1.0, zz],
                    [3.0, hgt, 3.0],
                    mix([30, 26, 70], look.accent, ((i * 3) % 5) as f32 * 0.04),
                    lit,
                    0.7,
                    0.0,
                ));
            }
        }

        if atmo_on {
            self.push_sky(&mut doc, &fin, hx, pulse);
        }

        // ---- hero
        self.push_hero(
            &mut doc,
            hx,
            models.contains(&BUNNY_GLB) && models.contains(&EARS_GLB),
        );
        // ---- sparks (kit x = track, kit z = lateral)
        for (i, sp) in self.core.juice.sparks.iter().enumerate() {
            let fade = 1.0 - (sp.life / sp.max_life).clamp(0.0, 1.0);
            let size = (sp.size * (0.4 + 0.6 * fade)).max(0.04);
            doc.props.push(prop(
                &format!("spark-{i}"),
                "sphere",
                "spark",
                [sp.z, sp.y, sp.x],
                [size, size, size],
                [sp.color[0], sp.color[1], sp.color[2]],
                2.0 * fade + 0.5,
                0.3,
                0.0,
            ));
        }
        self.doc = doc;
    }

    /// The creatures on the slab. They are drawn exactly where the sim tests
    /// them (`enemy_dx`), with a little hop on every step so the walk reads as
    /// a rhythm; a stomped one is squashed flat for a moment, then gone.
    /// Stars, far clouds, a horizon glow and a halo round the moon. Cheap props,
    /// all far away: they ride along with the camera so they never pass.
    fn push_sky(&self, doc: &mut WorldDoc, fin: &Finish, hx: f32, pulse: f32) {
        let tier = match self.picture_quality.as_deref() {
            Some("low") => 0.34,
            Some("high") => 1.0,
            _ => 0.75,
        };
        let hash = |i: u32, k: u32| -> f32 {
            let mut h = i.wrapping_mul(0x9E37_79B1) ^ k.wrapping_mul(0x85EB_CA6B);
            h ^= h >> 15;
            h = h.wrapping_mul(0x2C1B_3C6D);
            h ^= h >> 12;
            (h & 0xFFFF) as f32 / 65535.0
        };
        let stars = (fin.stars as f32 * tier).round() as u32;
        for i in 0..stars {
            let x = (hash(i, 1) * 2.0 - 1.0) * 62.0;
            let y = 11.0 + hash(i, 2) * 26.0;
            let tw = 0.5 + 0.5 * (self.anim_t * (1.5 + hash(i, 3) * 2.5) + hash(i, 4) * 6.3).sin();
            let sz = 0.34 + 0.3 * hash(i, 5);
            doc.props.push(prop(
                &format!("star-{i}"),
                "sphere",
                "star",
                [x, y, hx + 54.0 + hash(i, 6) * 8.0],
                [sz, sz, sz],
                mix([200, 210, 255], [255, 236, 200], hash(i, 7)),
                1.2 + 1.4 * tw + 0.5 * pulse,
                0.5,
                0.0,
            ));
        }
        let clouds = if tier < 0.5 { 0 } else { fin.clouds };
        for i in 0..clouds {
            let drift =
                (hash(i, 8) * 140.0 + self.anim_t * (0.6 + hash(i, 9) * 0.5)) % 140.0 - 70.0;
            let wide = 12.0 + hash(i, 10) * 10.0;
            doc.props.push(prop(
                &format!("cloud-{i}"),
                "sphere",
                "cloud",
                [
                    drift,
                    17.0 + hash(i, 11) * 9.0,
                    hx + 66.0 + hash(i, 12) * 6.0,
                ],
                [wide, 1.1 + hash(i, 13) * 0.8, 2.6],
                mix(fin.horizon, [40, 24, 90], 0.55),
                0.22 + 0.1 * pulse,
                0.9,
                0.0,
            ));
        }
        if fin.horizon_glow > 0.0 {
            // Two stacked bands, bright at the bottom, so the haze fades upward.
            // Just inside the prop cull distance (90) and behind nearly every tower.
            let c = fin.horizon;
            for (k, (y, hh, g)) in [(0.2, 3.0, 1.0), (3.4, 4.0, 0.35)].into_iter().enumerate() {
                doc.props.push(prop(
                    &format!("horizon-{k}"),
                    "box",
                    "horizon",
                    [0.0, y, hx + 88.5],
                    [220.0, hh, 1.0],
                    mix(c, [14, 10, 40], 0.3 * k as f32),
                    fin.horizon_glow * g * (0.55 + 0.15 * pulse),
                    1.0,
                    0.0,
                ));
            }
            doc.props.push(prop(
                "moon-halo",
                "sphere",
                "moon",
                [-12.0, 15.0, hx + 66.0],
                [11.0, 11.0, 1.0],
                mix(c, [255, 220, 190], 0.6),
                0.16 + 0.1 * pulse,
                1.0,
                0.0,
            ));
        }
    }

    fn push_enemies(&self, doc: &mut WorldDoc, models: &[&str], lo: f32, hi: f32) {
        let bs = self.spec.beat_secs();
        let pulse = self.beat_flash;
        for o in self.course.obst.iter() {
            if o.kind != Kind::Enemy || o.end + MAX_WALK_M < lo || o.start - 1.0 > hi {
                continue;
            }
            let Some(et) = self.spec.enemies.types.get(o.ety) else {
                continue;
            };
            let squash_age = o.stomped_t.map(|t| self.song_t - t);
            if squash_age.is_some_and(|a| a > 0.35) {
                continue;
            }
            if o.fate == Fate::Skipped && o.end < self.hero_x() - 2.0 {
                continue;
            }
            let Some(dx) = self.spec.enemy_dx(o, self.song_t) else {
                continue;
            };
            let center = (o.start + o.end) * 0.5 + dx;
            let stepping = et.step_m > 0.0;
            // Where in its step: a hop of 0.14 m over the first half of each step.
            let phase = if stepping {
                (self.song_t / bs * et.steps_per_beat as f32).fract()
            } else {
                0.0
            };
            let hop = if stepping && o.stomped_t.is_none() {
                0.14 * (phase * 2.0)
                    .min(1.0)
                    .mul_add(std::f32::consts::PI, 0.0)
                    .sin()
                    .max(0.0)
            } else {
                0.0
            };
            // Appears over its first step instead of popping.
            let appear = if stepping {
                let first = self
                    .spec
                    .enemy_dx(o, self.song_t - bs / et.steps_per_beat as f32);
                if first.is_none() {
                    (phase * 2.0).min(1.0)
                } else {
                    1.0
                }
            } else {
                1.0
            };
            // Idle / walk / fly from the beat clock; a stomp or a hit overrides it.
            let mut pose = crate::hop_anim::enemy_pose(
                &et.look,
                self.beat_pos().rem_euclid(1.0),
                pulse,
                phase,
            );
            if let Some(a) = squash_age {
                pose.scale = crate::hop_anim::stomp_scale(a, 0.35);
                pose.flash = 1.0;
            } else if let Some(t) = o.hit_t {
                let h = crate::hop_anim::hit_puff(self.song_t - t);
                pose.scale = h.scale;
                pose.flash = h.flash;
            }
            let mut k = pose.scale;
            k = k.map(|v| v * appear.max(0.05));
            let live = o.fate == Fate::Pending;
            let em = if squash_age.is_some() {
                1.2
            } else if live {
                0.45 + 0.4 * pulse + 0.9 * pose.flash
            } else {
                0.2 + 0.9 * pose.flash
            };
            let linear = et.color.map(|v| (v as f32 / 255.0).powf(2.2));
            let id = format!("enemy-{}-{}", (o.take_beat * 100.0).round() as i64, o.ety);
            let bob = if et.y_lo > 0.0 {
                0.06 * (self.anim_t * 5.0 + o.take_beat).sin()
            } else {
                0.0
            };
            let y = o.y_lo + hop + bob + pose.dy;
            let glb = match et.look.as_str() {
                "slime" => SLIME_GLB,
                "beetle" => BEETLE_GLB,
                _ => BAT_GLB,
            };
            if et.y_lo > 0.0 {
                // A bright pulsing blob on the slab marks the flyer's lane: the track under it is safe to tap through.
                // Inside the warning distance it flashes white-hot on every half beat.
                let ahead = center - self.hero_x();
                let warn = if live && ahead > 0.0 && ahead < WARN_M {
                    let b = (self.beat_pos() * 2.0).fract();
                    (1.0 - b) * (1.0 - ahead / WARN_M * 0.5)
                } else {
                    0.0
                };
                doc.props.push(prop(
                    &format!("{id}-shadow"),
                    "sphere",
                    "shadow",
                    [0.0, 0.015, center],
                    [
                        (o.end - o.start).max(0.9) * (0.9 + 0.1 * pulse + 0.25 * warn),
                        0.012,
                        0.8 + 0.4 * warn,
                    ],
                    mix(et.color.map(|v| v / 2), [255, 255, 255], 0.6 * warn),
                    0.9 + 0.5 * pulse + 1.4 * warn,
                    1.0,
                    0.0,
                ));
            }
            if models.contains(&glb) {
                let mut p = glb_prop(
                    &id,
                    glb,
                    "enemy",
                    [0.0, y, center],
                    k,
                    [255, 255, 255],
                    em,
                    Some(linear),
                );
                p.roll = pose.roll;
                p.yaw = pose.yaw;
                doc.props.push(p);
                continue;
            }
            // Primitive fallback: the same silhouette from a few shapes.
            let c = et.color;
            let dark = c.map(|v| v / 3);
            let mut add =
                |name: &str, kind: &str, pos: [f32; 3], size: [f32; 3], col: [u32; 3], e: f32| {
                    doc.props.push(prop(
                        &format!("{id}-{name}"),
                        kind,
                        "enemy",
                        [pos[0] * k[0], y + pos[1] * k[1], center + pos[2] * k[2]],
                        [size[0] * k[0], size[1] * k[1], size[2] * k[2]],
                        col,
                        e,
                        0.4,
                        0.0,
                    ));
                };
            match et.look.as_str() {
                "slime" => {
                    add("body", "sphere", [0.0, 0.25, 0.0], [0.8, 0.5, 0.76], c, em);
                    add(
                        "eye-l",
                        "sphere",
                        [-0.15, 0.28, -0.3],
                        [0.16, 0.18, 0.1],
                        [255, 255, 255],
                        0.5,
                    );
                    add(
                        "eye-r",
                        "sphere",
                        [0.15, 0.28, -0.3],
                        [0.16, 0.18, 0.1],
                        [255, 255, 255],
                        0.5,
                    );
                }
                "beetle" => {
                    add("body", "sphere", [0.0, 0.3, 0.06], [0.72, 0.52, 0.8], c, em);
                    add(
                        "head",
                        "sphere",
                        [0.0, 0.22, -0.36],
                        [0.36, 0.32, 0.3],
                        dark,
                        0.2,
                    );
                    add(
                        "eye-l",
                        "sphere",
                        [-0.09, 0.28, -0.5],
                        [0.1, 0.12, 0.07],
                        [255, 255, 255],
                        0.5,
                    );
                    add(
                        "eye-r",
                        "sphere",
                        [0.09, 0.28, -0.5],
                        [0.1, 0.12, 0.07],
                        [255, 255, 255],
                        0.5,
                    );
                }
                _ => {
                    add("body", "sphere", [0.0, 0.34, 0.0], [0.44, 0.5, 0.4], c, em);
                    add(
                        "wing-l",
                        "box",
                        [-0.5, 0.34, 0.0],
                        [0.7, 0.06, 0.45],
                        dark,
                        em * 0.6,
                    );
                    add(
                        "wing-r",
                        "box",
                        [0.5, 0.34, 0.0],
                        [0.7, 0.06, 0.45],
                        dark,
                        em * 0.6,
                    );
                    add(
                        "eye-l",
                        "sphere",
                        [-0.08, 0.42, -0.16],
                        [0.09, 0.1, 0.06],
                        [255, 232, 96],
                        1.2,
                    );
                    add(
                        "eye-r",
                        "sphere",
                        [0.08, 0.42, -0.16],
                        [0.09, 0.1, 0.06],
                        [255, 232, 96],
                        1.2,
                    );
                }
            }
        }
    }

    fn push_hero(&self, doc: &mut WorldDoc, hx: f32, glb: bool) {
        let look = &self.spec.look;
        let title = self.flow.phase == FlowPhase::Title;
        let mut y = self.y + self.y_vis_off;
        let (mut squash, mut ear) = (self.anim.squash.x, self.anim.ear.x);
        if title {
            let b = (self.attract / self.spec.beat_secs()).fract();
            y = 4.0 * 0.5 * b * (1.0 - b);
            squash = if b < 0.1 { 0.5 } else { 0.0 };
            ear = crate::hop_anim::ear_target(4.0 * (1.0 - 2.0 * b), b, false);
        } else if matches!(self.air, Air::Ground) {
            // Standing: a small bob on the beat.
            y += crate::hop_anim::idle_bob(self.beat_pos().rem_euclid(1.0));
        }
        let blink = self.invuln > 0.0 && ((self.anim_t * 10.0) as u32).is_multiple_of(2);
        let [mut sx, mut sy, _] = crate::hop_anim::body_scale(squash);
        let r = 0.40;
        // A fall: the bunny tumbles about its middle, stretches along the drop, and runs
        // on ahead of the trailing camera. Dead or alive it is the same picture.
        let falling = !title && (self.fall_on || self.dead_fall.is_some());
        let (mut spin, mut z) = (0.0_f32, hx);
        if falling {
            let f = &self.spec.fall;
            let (_, off) = self.fall_view();
            z += off;
            spin =
                std::f32::consts::TAU * f.tumble_turns_per_sec * (self.fall_clock() - 0.1).max(0.0);
            let vy = match self.dead_fall {
                Some(d) => {
                    let g = self.spec.hero.fall_gravity;
                    y = -3.5 + d.vy * d.age - 0.5 * g * d.age * d.age;
                    d.vy - g * d.age
                }
                None => self.fall_vy,
            };
            let k = (vy.abs() / 14.0).clamp(0.0, 1.0) * f.stretch;
            sx *= 1.0 - 0.4 * k;
            sy *= 1.0 + k;
        }
        let gy = if falling { y } else { y.max(0.0) };
        let (sin_a, cos_a) = spin.sin_cos();
        let body_c = if self.hit_flash > 0.6 {
            [255, 255, 255]
        } else if self.hit_flash > 0.2 {
            [255, 70, 80]
        } else {
            skin_tint(&self.spec.skin).unwrap_or(look.hero)
        };
        let em = if blink {
            0.1
        } else {
            0.18 + 0.25 * self.hit_flash + 0.3 * self.combo_glow * (0.6 + 0.4 * self.beat_flash)
        };
        {
            // shadow blob, only while there is slab below
            let over = (supported(&self.course.obst, hx, 0.0) || title) && !falling;
            if over {
                let k = (1.0 - gy / 2.2).clamp(0.2, 1.0);
                doc.props.push(prop(
                    "hero-shadow",
                    "sphere",
                    "shadow",
                    [0.0, 0.015, z],
                    [0.9 * k, 0.012, 0.9 * k],
                    [6, 4, 14],
                    0.0,
                    1.0,
                    0.0,
                ));
                // Combo glow: a ring of light on the slab that grows with the streak.
                if self.combo_glow > 0.05 && !title {
                    let g = self.combo_glow;
                    let r = (0.9 + 0.7 * g) * (1.0 + 0.12 * self.beat_flash);
                    doc.props.push(prop(
                        "hero-aura",
                        "sphere",
                        "aura",
                        [0.0, 0.02, z],
                        [r, 0.012, r],
                        look.accent,
                        0.5 + 1.1 * g,
                        1.0,
                        0.0,
                    ));
                }
                // PERFECT / GOOD / stomp: a ring that runs out from the feet.
                if self.ring_age < 0.4 && !title {
                    let u = self.ring_age / 0.4;
                    let r = 0.7 + 2.6 * (1.0 - (1.0 - u) * (1.0 - u));
                    doc.props.push(prop(
                        "hero-ring",
                        "sphere",
                        "ring",
                        [0.0, 0.03, z],
                        [r, 0.01, r],
                        self.ring_col,
                        1.6 * (1.0 - u),
                        1.0,
                        0.0,
                    ));
                }
            }
        }
        if glb {
            // The model is 0.8 m tall with its feet on y = 0, like the ball below.
            let flash = self.hit_flash.clamp(0.0, 1.0);
            let beat = 0.12 * self.beat_flash;
            let (em, col) = if flash > 0.6 {
                (0.5 + 0.8 * flash, [1.0, 1.0, 1.0])
            } else if flash > 0.2 {
                (0.4 + 0.6 * flash, [1.0, 0.18, 0.22])
            } else if blink {
                (0.0, [1.0, 1.0, 1.0])
            } else {
                (
                    beat + 0.22 * self.combo_glow,
                    look.glow.map(|v| (v as f32 / 255.0).powf(2.2)),
                )
            };
            let scale = [sx * 1.2, sy * 1.2, sx * 1.2];
            // Turning about the middle of the body (0.4 m up the model), not the feet.
            let c = 0.4 * scale[1];
            let (fx, fy) = (c * sin_a, gy + c - c * cos_a);
            let mut body = glb_prop(
                "hero-body",
                BUNNY_GLB,
                "hero",
                [fx, fy, z],
                scale,
                [255, 255, 255],
                em,
                Some(col),
            );
            body.roll = spin;
            doc.props.push(body);
            // The ears swing from the top of the head: follow-through.
            let ey = EAR_PIVOT[1] * scale[1];
            let mut ears = glb_prop(
                "hero-ears",
                EARS_GLB,
                "hero",
                [
                    fx - ey * sin_a,
                    fy + ey * cos_a,
                    z + EAR_PIVOT[2] * scale[2],
                ],
                scale,
                [255, 255, 255],
                em,
                Some(col),
            );
            ears.pitch = ear;
            ears.roll = spin;
            doc.props.push(ears);
            return;
        }
        doc.props.push(prop(
            "hero-body",
            "sphere",
            "hero",
            [0.0, gy + r * sy, z],
            [r * 2.0 * sx, r * 2.0 * sy, r * 2.0 * sx],
            body_c,
            em,
            0.35,
            0.0,
        ));
        for (i, sx2) in [-1.0_f32, 1.0].into_iter().enumerate() {
            doc.props.push(prop(
                &format!("hero-ear-{i}"),
                "box",
                "hero",
                [
                    sx2 * 0.2 * cos_a - (r * sy + 0.06) * sin_a,
                    gy + r * sy + sx2 * 0.2 * sin_a + (r * sy + 0.06) * cos_a,
                    z - 0.02,
                ],
                [0.14, 0.24, 0.1],
                body_c,
                em,
                0.4,
                0.0,
            ));
            if let Some(p) = doc.props.last_mut() {
                p.pitch = ear;
                p.roll = spin;
            }
        }
        doc.props.push(prop(
            "hero-core",
            "sphere",
            "hero-core",
            [0.0, gy + r * sy + 0.02, z - r * 0.8],
            [0.22, 0.22, 0.12],
            look.glow,
            1.6 + 0.8 * self.beat_flash,
            0.2,
            0.0,
        ));
    }
}

impl crate::kit_host::KitHost for HopBeat {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn name(&self) -> &'static str {
        "hop beat"
    }
    fn phase_name(&self) -> &'static str {
        HopBeat::phase_name(self)
    }
    fn playing(&self) -> bool {
        HopBeat::playing(self)
    }
    fn document(&self) -> &WorldDoc {
        HopBeat::document(self)
    }
    fn dump_json(&self) -> String {
        HopBeat::dump_json(self)
    }
    fn state_json(&self) -> String {
        HopBeat::state_json(self)
    }
    fn confirm(&mut self) {
        HopBeat::confirm(self)
    }
    fn show_title(&mut self) {
        HopBeat::show_title(self)
    }
    fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) -> bool {
        HopBeat::pointer(self, x, y, width, height)
    }
    fn on_key(&mut self, name: &str, down: bool) -> bool {
        HopBeat::on_key(self, name, down)
    }
    fn tick(&mut self, dt: f32) {
        HopBeat::tick(self, dt)
    }
    fn hud(&self, width: u32, height: u32, menu: bool) -> DrawList {
        HopBeat::hud(self, width, height, menu)
    }
    /// One button: pointer and keys only, no stick.
    fn walk(&mut self, _lx: f32, _lz: f32, _jump: bool) {}
    /// A finger down in play is the hop button; any release lets go of it.
    fn touch(&mut self, ev: crate::input::PointerEvent, w: u32, h: u32, _t: f32) {
        use crate::input::PointerPhase as P;
        match ev.phase {
            P::Begin if self.flow.phase == FlowPhase::Play => self.press(ev.x, ev.y, w, h),
            P::End | P::Cancel => self.touch_end(),
            _ => {}
        }
    }
    fn uses_swipe(&self) -> bool {
        // No pads or action buttons from the host: the whole canvas is the button.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> HopBeat {
        let mut g = HopBeat::new(HopSpec::default());
        g.set_sfx(false);
        g
    }

    fn run(g: &mut HopBeat, secs: f32) {
        for _ in 0..(secs * 60.0) as u32 {
            g.tick(1.0 / 60.0);
        }
    }

    #[test]
    fn default_spec_is_valid_and_the_music_numbers_match_the_pack() {
        let s = HopSpec::default();
        s.validate().unwrap();
        assert_eq!(s.music.bpm, 126.0);
        assert_eq!(s.music.loop_bars, 32);
        // 32 bars of 4 at 126 BPM is exactly the 2,688,000-sample loop at 44.1 kHz.
        let samples =
            s.music.loop_bars as f32 * s.music.beats_per_bar as f32 * s.beat_secs() * 44_100.0;
        assert!((samples - 2_688_000.0).abs() < 1.0, "{samples}");
    }

    #[test]
    fn baked_fixture_is_the_default_world() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/hop_beat_world.json"
        );
        let text = std::fs::read_to_string(path)
            .expect("fixture (cargo run --example hop_measure -- --dump-world)");
        let want: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(want, HopBeat::new(HopSpec::default()).world_value());
        assert!(is_hop_dump(&text));
        HopBeat::from_json(&text).unwrap();
    }

    #[test]
    fn unknown_keys_and_bad_numbers_are_refused() {
        assert!(HopSpec::from_value(&json!({"music": {"bpm": 126, "bogus": 1}})).is_err());
        assert!(HopSpec::from_value(&json!({"music": {"bpm": 10}})).is_err());
        assert!(HopSpec::from_value(&json!({"music": {"bpm": 126}})).is_ok());
    }

    #[test]
    fn same_seed_same_course() {
        let spec = HopSpec::default();
        let (mut a, mut b) = (Course::new(&spec), Course::new(&spec));
        a.ensure(&spec, 400.0);
        b.ensure(&spec, 400.0);
        let key = |c: &Course| {
            c.obst
                .iter()
                .map(|o| (o.start.to_bits(), o.end.to_bits()))
                .collect::<Vec<_>>()
        };
        assert_eq!(key(&a), key(&b));
        let mut other = spec.clone();
        other.course.seed = 99;
        let mut c = Course::new(&other);
        c.ensure(&other, 400.0);
        assert_ne!(key(&a), key(&c));
    }

    #[test]
    fn obstacles_sit_on_the_beat_grid() {
        let spec = HopSpec::default();
        let mut c = Course::new(&spec);
        c.ensure(&spec, 200.0);
        for o in &c.obst {
            let twice = o.take_beat * 2.0;
            assert!(
                (twice - twice.round()).abs() < 1e-4,
                "off grid: {}",
                o.take_beat
            );
        }
        assert!(c.obst[0].take_beat >= (spec.music.lead_in_bars * spec.music.beats_per_bar) as f32);
    }

    #[test]
    fn title_waits_then_play_runs_and_idle_dies() {
        let mut g = fresh();
        run(&mut g, 2.0);
        assert_eq!(g.phase_name(), "title");
        assert_eq!(g.song_t(), 0.0);
        g.confirm();
        assert_eq!(g.phase_name(), "play");
        run(&mut g, 60.0);
        assert_eq!(g.phase_name(), "result");
        assert_eq!(g.lives(), 0);
        assert!(!g.can_revive() || g.spec.revive.enabled);
    }

    #[test]
    fn direct_mode_waits_for_the_first_tap() {
        let mut spec = HopSpec::default();
        spec.play.direct = true;
        let mut g = HopBeat::new(spec);
        g.set_sfx(false);
        g.confirm();
        assert!(g.armed());
        run(&mut g, 1.0);
        assert_eq!(g.song_t(), 0.0);
        g.set_hold(true);
        g.tick(1.0 / 60.0);
        g.set_hold(false);
        assert!(!g.armed());
        run(&mut g, 0.5);
        assert!(g.song_t() > 0.4);
    }

    #[test]
    fn music_clock_pulls_the_picture_and_latency_shifts_it() {
        let mut g = fresh();
        g.confirm();
        run(&mut g, 1.0);
        // The music says 1.10 s while the sim says ~1.0 s: it catches up gently.
        let t0 = g.song_t();
        g.set_music_clock(Some(t0 + 0.1));
        g.tick(1.0 / 60.0);
        let step = g.song_t() - t0;
        assert!(step > 1.0 / 60.0 && step < 1.0 / 60.0 * 1.1, "{step}");
        // A far-off clock (seek, tab switch) snaps and protects the hero.
        g.set_music_clock(Some(g.song_t() + 5.0));
        g.tick(1.0 / 60.0);
        assert!(g.song_t() > t0 + 4.0);
        assert!(g.state_value()["invulnerable"].as_bool().unwrap());
        // Latency: a positive setting makes the picture later than the sound.
        let mut a = fresh();
        a.confirm();
        a.set_latency_ms(100.0);
        a.set_music_clock(Some(2.0));
        a.tick(1.0 / 60.0);
        assert!((a.song_t() - 1.9).abs() < 0.02, "{}", a.song_t());
        a.set_latency_ms(1e9);
        assert_eq!(a.latency_ms(), 300.0);
    }

    #[test]
    fn revive_gives_lives_back_once() {
        let mut g = fresh();
        g.confirm();
        run(&mut g, 60.0);
        assert_eq!(g.phase_name(), "result");
        assert!(g.can_revive());
        let died_at = g.song_t();
        assert!(g.revive());
        assert_eq!(g.phase_name(), "play");
        assert_eq!(g.lives(), g.spec.revive.lives);
        assert!(g.song_t() >= died_at);
        let beat = g.beat_pos();
        let limit = beat + g.spec.revive.clear_beats - 0.01;
        assert!(g.course().obst.iter().all(|o| o.take_beat >= limit));
        // The bar line it restarts on is a whole number of bars.
        let bar = g.spec.beat_secs() * g.spec.music.beats_per_bar as f32;
        assert!(((g.song_t() / bar) - (g.song_t() / bar).round()).abs() < 1e-3);
        run(&mut g, 60.0);
        assert_eq!(g.phase_name(), "result");
        assert!(!g.can_revive());
        assert!(!g.revive());
    }

    #[test]
    fn revive_can_be_turned_off() {
        let mut spec = HopSpec::default();
        spec.revive.enabled = false;
        let mut g = HopBeat::new(spec);
        g.confirm();
        run(&mut g, 60.0);
        assert!(!g.can_revive());
        assert!(!g.revive());
    }

    #[test]
    fn state_result_dump_and_hud_are_well_formed() {
        let mut g = fresh();
        g.confirm();
        run(&mut g, 3.0);
        let st: serde_json::Value = serde_json::from_str(&g.state_json()).unwrap();
        assert_eq!(st["phase"], "play");
        assert_eq!(st["bpm"], 126.0);
        let res = &st["result"];
        assert_eq!(res["columns"], json!(["score", "combo", "perfect"]));
        assert!(res["key"].as_str().unwrap().starts_with("hop_beat:"));
        let dump = g.dump_json();
        assert!(is_hop_dump(&dump));
        let back = HopBeat::from_json(&dump).unwrap();
        assert_eq!(back.spec, g.spec);
        let hud: serde_json::Value = serde_json::from_str(&g.hud_json(800, 480)).unwrap();
        assert!(!hud["texts"].as_array().unwrap().is_empty());
        for (w, h) in [(1, 1), (360, 780), (1920, 1080)] {
            let _ = g.hud(w, h, true);
            let _ = g.hud(w, h, false);
        }
    }

    #[test]
    fn the_visual_hit_stop_holds_the_picture_but_never_the_sim() {
        let mut g = fresh();
        g.confirm();
        run(&mut g, 1.0);
        let (t0, x0) = (g.song_t(), g.view_x());
        g.freeze_picture(0.07);
        run(&mut g, 0.05);
        // The music clock and the hero ran on at full speed ...
        assert!((g.song_t() - t0 - 0.05).abs() < 0.02, "{}", g.song_t() - t0);
        // ... the picture did not move.
        assert!((g.view_x() - x0).abs() < 0.02, "{}", g.view_x() - x0);
        assert!(g.vlag > 0.2);
        // Then it catches up without a pop.
        run(&mut g, 0.4);
        assert!(g.vlag < 0.02, "{}", g.vlag);
        assert!((g.hero_x() - g.view_x()).abs() < 0.02);
        // A full freeze never lags more than 0.7 m.
        g.freeze_picture(9.0);
        run(&mut g, 2.0);
        assert!(g.vlag <= 0.7 + 1e-4);
    }

    #[test]
    fn the_first_sight_of_each_enemy_gets_one_hint_and_the_language_switches_it() {
        let mut g = fresh();
        g.confirm();
        let mut seen = false;
        for _ in 0..(14.0 * 60.0) as u32 {
            g.tick(1.0 / 60.0);
            if g.hint.is_some() {
                seen = true;
                let ja = |g: &HopBeat| {
                    g.hud(390, 844, false)
                        .texts
                        .iter()
                        .map(|t| t.text.clone())
                        .collect::<Vec<_>>()
                        .join("|")
                };
                assert!(ja(&g).contains("HOP OVER IT"), "{}", ja(&g));
                g.set_lang("ja-JP");
                assert!(ja(&g).contains("とびこえるか"), "{}", ja(&g));
                break;
            }
        }
        assert!(seen, "no hint before the first enemy");
        assert!(g.hints_seen.first().copied().unwrap_or(false));
    }

    #[test]
    fn the_pause_button_and_card_work_and_the_sync_steps() {
        use crate::input::PointerPhase as P;
        let (w, h) = (800u32, 400u32);
        let mut g = fresh();
        g.set_direct(false);
        g.confirm();
        run(&mut g, 1.0);
        assert_eq!(g.flow.phase, FlowPhase::Play);
        // A tap on the button pauses and does not hop.
        let pb = pause_button(w as f32, h as f32);
        g.pointer_event(P::Begin, pb.x + pb.w * 0.5, pb.y + pb.h * 0.5, w, h);
        assert_eq!(g.flow.phase, FlowPhase::Pause);
        assert_eq!(g.touches, 0);
        let hud = crate::flow::draw_list_json(&g.hud(w, h, false));
        assert!(
            hud.contains("PAUSED") && hud.contains("AUDIO SYNC") && hud.contains("0 ms"),
            "{hud}"
        );
        let hit = |g: &mut HopBeat, b: PauseBtn| {
            let r = pause_layout(w as f32, h as f32)
                .into_iter()
                .find(|(k, _)| *k == b)
                .unwrap()
                .1;
            g.pointer_event(P::Begin, r.x + r.w * 0.5, r.y + r.h * 0.5, w, h);
        };
        hit(&mut g, PauseBtn::SyncPlus);
        hit(&mut g, PauseBtn::SyncPlus);
        hit(&mut g, PauseBtn::SyncMinus);
        assert_eq!(g.latency_ms(), 10.0);
        assert!(crate::flow::draw_list_json(&g.hud(w, h, false)).contains("+10 ms"));
        // The pause card is on screen in Japanese too.
        g.set_lang("ja");
        assert!(crate::flow::draw_list_json(&g.hud(w, h, false)).contains("ポーズ"));
        // Every button of the card fits on a small phone in both orientations.
        for (pw, ph) in [(360.0f32, 640.0f32), (640.0, 360.0), (390.0, 844.0)] {
            for (_, r) in pause_layout(pw, ph) {
                assert!(
                    r.x >= 0.0 && r.x + r.w <= pw && r.y >= 0.0 && r.y + r.h + 40.0 <= ph,
                    "{pw}x{ph} {r:?}"
                );
            }
        }
        hit(&mut g, PauseBtn::Resume);
        assert_eq!(g.flow.phase, FlowPhase::Play);
        assert!(g.invuln > 0.5, "a second of grace after resume");
        // Retry starts a fresh run and keeps the player's sync.
        g.pause();
        hit(&mut g, PauseBtn::Retry);
        assert_eq!(g.flow.phase, FlowPhase::Play);
        assert_eq!(g.revives, 0, "a new run starts with its revives back");
        assert_eq!(g.latency_ms(), 10.0);
        g.pause();
        hit(&mut g, PauseBtn::Title);
        assert_eq!(g.flow.phase, FlowPhase::Title);
    }

    #[test]
    fn game_over_cues_the_new_best_stinger_only_for_a_new_best() {
        for (best, want) in [(1u32, true), (u32::MAX, false)] {
            let mut g = fresh();
            g.set_direct(false);
            g.flow.best_score = best;
            g.confirm();
            let mut t = 0.0;
            while !g.pending_finish && t < 60.0 {
                // No input: the bunny runs on until the first gap.
                g.tick(1.0 / 60.0);
                t += 1.0 / 60.0;
            }
            assert!(g.pending_finish, "the run never ended");
            if want {
                assert!(g.core.stats.score > best, "score {}", g.core.stats.score);
            }
            assert_eq!(g.new_best, want);
            assert_eq!(g.result_value()["new_best"], json!(want));
            let run = &g.result_value()["run"];
            assert!(
                run["score"].is_number() && run["combo"].is_number() && run["perfect"].is_number()
            );
            assert!(run["stomps"].is_number());
        }
    }

    #[test]
    fn every_cue_has_a_file_and_the_repeating_ones_sit_under_the_rewards() {
        let spec = HopSpec::default();
        for n in SFX_CUES {
            let cue = &spec.audio.sfx[n];
            let f = crate::assets::resolve_asset(&cue.file);
            assert!(f.is_some(), "{n}: {} is missing", cue.file);
        }
        let v = |n: &str| spec.audio.sfx[n].volume;
        assert!(v("hop") < v("perfect") && v("land") < v("stomp") && v("warn") < v("hit"));
        assert!(spec.audio.sfx.contains_key("warn"));
    }

    #[test]
    fn the_models_load_are_the_right_size_and_the_picture_uses_them() {
        let mut files = vec![
            BUNNY_GLB.to_string(),
            EARS_GLB.to_string(),
            POST_GLB.to_string(),
            SLIME_GLB.to_string(),
            BEETLE_GLB.to_string(),
            BAT_GLB.to_string(),
        ];
        for (t, w) in TOWER_GLBS {
            files.push(t.to_string());
            files.push(w.to_string());
        }
        for f in &files {
            assert!(glb_ready(f), "{f} did not load");
            let file = crate::assets::resolve_asset(f).unwrap();
            let bytes = crate::assets::read_bytes(&file).unwrap();
            assert!(bytes.len() < 200_000, "{f} is {} bytes", bytes.len());
            let b = crate::gltf_load::mesh_from_glb(&bytes).unwrap().bounds();
            assert!(b.min.y > -0.05, "{f} sinks below its base ({})", b.min.y);
            if f == BUNNY_GLB {
                // 1.0-1.3 m tall, feet on y = 0, much narrower than the track
                assert!(b.max.y > 0.8 && b.max.y < 1.4, "bunny {}", b.max.y);
                assert!(b.max.x - b.min.x < 1.0);
            }
        }
        let mut g = fresh();
        g.confirm();
        run(&mut g, 3.0);
        let d = g.document();
        let has = |id: &str| d.props.iter().any(|p| p.id == id && p.gltf.is_some());
        assert!(has("prop:hero-body"));
        assert!(!d.props.iter().any(|p| p.id == "prop:hero-ear-0"));
        assert!(d
            .props
            .iter()
            .any(|p| p.name == "tower" && p.gltf.is_some()));
        assert!(d
            .props
            .iter()
            .any(|p| p.name == "windows" && p.gltf.is_some()));
        assert!(d.props.iter().any(|p| p.name == "post" && p.gltf.is_some()));
        // the slot table sees the same models on every frame
        let specs = |d: &crate::world_doc::WorldDoc| {
            let mut v: Vec<String> = d.props.iter().filter_map(|p| p.gltf.clone()).collect();
            v.sort();
            v.dedup();
            v
        };
        let first = specs(d);
        for _ in 0..600 {
            g.tick(1.0 / 60.0);
            assert_eq!(specs(g.document()), first);
        }
    }

    fn item(beat: f32, kind: &str, beats: f32, hop: &str, enemy: &str) -> Item {
        Item {
            beat,
            kind: kind.into(),
            beats,
            hop: hop.into(),
            enemy: enemy.into(),
        }
    }

    /// One phrase, every enemy unlocked at level 0, no rest bars.
    fn enemy_spec(items: Vec<Item>) -> HopSpec {
        let mut s = HopSpec::default();
        s.course.phrases = vec![Phrase {
            level: 0,
            bars: 4,
            items,
            weight: 1,
            debut: false,
        }];
        s.course.rest_every_bars = 1000;
        for t in s.enemies.types.iter_mut() {
            t.unlock_level = 0;
        }
        s
    }

    /// Plays `spec` and presses `off` seconds from every ideal press (holding for
    /// a hold hop), until `secs` after the first obstacle's ideal press.
    fn drive(spec: HopSpec, off: f32, secs: f32) -> HopBeat {
        let mut c = Course::new(&spec);
        c.ensure(&spec, 200.0);
        let bs = spec.beat_secs();
        let presses: Vec<(f32, f32)> = c
            .obst
            .iter()
            .filter(|o| spec.needs_press(o))
            .map(|o| {
                let air = spec.arc(o.hop).air_beats * bs;
                let hold = if o.hop == Hop::Hold { air * 0.9 } else { 0.05 };
                (o.take_beat * bs + off, hold)
            })
            .collect();
        let end = c.obst.first().map(|o| o.take_beat * bs).unwrap_or(0.0) + secs;
        let mut g = HopBeat::new(spec);
        g.set_sfx(false);
        g.set_headless(true);
        g.confirm();
        while g.song_t() < end && g.phase_name() == "play" {
            let t = g.song_t();
            g.set_hold(presses.iter().any(|(p, h)| t >= *p && t < p + h));
            g.tick(1.0 / 60.0);
        }
        g
    }

    #[test]
    fn a_slime_is_stomped_by_an_early_press_and_hurts_a_late_one() {
        let spec = enemy_spec(vec![item(4.0, "enemy", 0.2, "tap", "slime")]);
        let ideal = drive(spec.clone(), 0.0, 2.0);
        assert_eq!(
            (ideal.lives(), ideal.stomps()),
            (3, 0),
            "ideal press clears it"
        );
        let early = drive(spec.clone(), -0.2, 2.0);
        assert_eq!(
            (early.lives(), early.stomps()),
            (3, 1),
            "early press lands on it"
        );
        assert!(
            early.score() >= spec.enemies.stomp_score,
            "{}",
            early.score()
        );
        assert_eq!(early.state_value()["stomps"], 1);
        let late = drive(spec, 0.25, 2.0);
        assert_eq!(late.lives(), 2, "a late press runs into its side");
    }

    #[test]
    fn the_stomp_bounce_lands_back_on_the_half_beat_grid() {
        let spec = HopSpec::default();
        let half = spec.beat_secs() * 0.5;
        for i in 0..40 {
            let t = 10.0 + i as f32 * 0.0137;
            let d = spec.bounce_secs(t);
            assert!(d >= 0.2 - 1e-5 && d < 0.2 + half + 1e-5, "{d}");
            let landing = (t + d) / half;
            assert!((landing - landing.round()).abs() < 1e-3, "{landing}");
        }
    }

    #[test]
    fn a_bat_punishes_a_hold_hop_and_leaves_a_tap_hop_alone() {
        let flyer = |hop: &str| {
            enemy_spec(vec![
                item(4.0, "enemy", 0.2, hop, "slime"),
                item(4.0, "enemy", 0.9, hop, "bat"),
            ])
        };
        // the slime is under the bat: a tap hop clears both, a hold hop climbs into the bat
        let tap = drive(flyer("tap"), 0.0, 2.0);
        assert_eq!(tap.lives(), 3);
        let hold = drive(flyer("hold"), 0.0, 3.0);
        assert!(hold.lives() < 3, "a hold hop must hit a bat");
    }

    #[test]
    fn a_bat_over_a_slime_is_clearable_with_room_to_spare() {
        // Emma's case: a slime under a bat reads as impossible. It is not, and the
        // tap hop's head stays well clear of the bat.
        let spec = HopSpec::default();
        let bat = &spec.enemies.types[spec.enemy_index("bat").unwrap()];
        let tap_peak = spec.arc(Hop::Tap).height + spec.enemies.body_h;
        assert!(
            bat.y_lo - tap_peak >= 0.3,
            "only {:.2} m between a tap hop and the bat",
            bat.y_lo - tap_peak
        );
        // every builtin phrase that stacks a bat over a ground enemy is cleared by a
        // tap at every press offset inside the perfect window
        let stacked: Vec<Phrase> = builtin_phrases()
            .into_iter()
            .filter(|p| {
                p.items.iter().any(|a| {
                    a.enemy == "bat"
                        && p.items
                            .iter()
                            .any(|b| b.enemy != "bat" && b.kind == "enemy" && b.beat == a.beat)
                })
            })
            .collect();
        assert!(!stacked.is_empty());
        for ph in &stacked {
            assert!(ph.level >= 3, "stacked bats wait until level 3");
            let items: Vec<Item> = ph.items.clone();
            for off in [-0.065_f32, -0.04, 0.0, 0.04, 0.065] {
                let mut s = enemy_spec(items.clone());
                s.course.phrases = vec![Phrase {
                    level: 0,
                    ..ph.clone()
                }];
                assert!(s.course_problems().is_empty(), "{:?}", s.course_problems());
                let g = drive(s, off, 5.0);
                assert_eq!(g.lives(), 3, "offset {off}");
            }
        }
    }

    #[test]
    fn enemies_arrive_in_the_stated_order_one_kind_at_a_time() {
        let base = HopSpec::default();
        let lead = (base.music.lead_in_bars) as f32;
        let bpb = base.music.beats_per_bar as f32;
        for seed in 0..40 {
            let mut spec = base.clone();
            spec.course.seed = seed;
            let mut c = Course::new(&spec);
            c.ensure(&spec, 200.0 * bpb);
            let first = |id: &str| {
                let i = spec.enemy_index(id).unwrap();
                c.obst
                    .iter()
                    .find(|o| o.kind == Kind::Enemy && o.ety == i)
                    .map(|o| o.take_beat / bpb - lead)
            };
            let slime = first("slime").unwrap();
            let beetle = first("beetle").unwrap();
            let bat = first("bat").unwrap();
            assert!(slime < 4.0, "seed {seed}: first slime at bar {slime}");
            assert!(
                (8.0..12.0).contains(&beetle),
                "seed {seed}: beetle at bar {beetle}"
            );
            assert!((16.0..20.0).contains(&bat), "seed {seed}: bat at bar {bat}");
            // the first bat has the track to itself
            let enemy_beats = |id: &str| -> Vec<f32> {
                let i = spec.enemy_index(id).unwrap();
                c.obst
                    .iter()
                    .filter(|o| o.kind == Kind::Enemy && o.ety == i)
                    .map(|o| o.take_beat)
                    .collect()
            };
            let first_bat = enemy_beats("bat")[0];
            let crowd = c
                .obst
                .iter()
                .filter(|o| (o.take_beat - first_bat).abs() < 1.5)
                .count();
            assert_eq!(crowd, 1, "seed {seed}: the first bat shares its stretch");
        }
    }

    #[test]
    fn a_flyer_alone_asks_for_no_press() {
        let spec = enemy_spec(vec![item(4.0, "enemy", 1.0, "tap", "bat")]);
        let mut c = Course::new(&spec);
        c.ensure(&spec, 100.0);
        assert!(!spec.needs_press(&c.obst[0]));
        let g = drive(spec, 0.0, 2.0);
        assert_eq!(g.lives(), 3);
        assert_eq!(g.state_value()["perfect"].as_u64().unwrap_or(0), 0);
    }

    #[test]
    fn a_walker_moves_on_the_half_beat_grid() {
        let spec = HopSpec::default();
        let bs = spec.beat_secs();
        let i = spec.enemy_index("beetle").unwrap();
        let o = spec.probe(Kind::Enemy, Hop::Tap, 0.25, 20.0, Some(i));
        let et = &spec.enemies.types[i];
        let meet_t = (o.start + o.end) * 0.5 / spec.speed();
        let step = bs / et.steps_per_beat as f32;
        // standing still inside a step, one step_m further on after it
        let a = spec.enemy_dx(&o, meet_t + 0.01).unwrap();
        let b = spec.enemy_dx(&o, meet_t + step * 0.4).unwrap();
        assert_eq!(a, b);
        let before = spec.enemy_dx(&o, meet_t - step * 1.01).unwrap();
        assert!((before - a - et.step_m).abs() < 1e-4 || a.abs() < 1e-4 + et.step_m);
        // not there yet before its walk starts
        assert!(spec
            .enemy_dx(&o, meet_t - step * (et.walk_steps as f32 + 2.0))
            .is_none());
        // a slime never moves
        let s = spec.probe(Kind::Enemy, Hop::Tap, 0.2, 20.0, spec.enemy_index("slime"));
        assert_eq!(spec.enemy_dx(&s, meet_t - 1.0), Some(0.0));
    }

    #[test]
    fn enemies_arrive_one_kind_at_a_time_in_the_built_in_course() {
        let spec = HopSpec::default();
        spec.validate().unwrap();
        for et in &spec.enemies.types {
            let first = builtin_phrases()
                .iter()
                .filter(|p| p.items.iter().any(|i| i.enemy == et.id))
                .map(|p| p.level)
                .min()
                .unwrap_or_else(|| panic!("{} is never used", et.id));
            assert_eq!(first, et.unlock_level, "{} appears at level {first}", et.id);
        }
        let mut levels: Vec<u32> = spec.enemies.types.iter().map(|t| t.unlock_level).collect();
        levels.dedup();
        assert_eq!(
            levels.len(),
            spec.enemies.types.len(),
            "one new kind per level"
        );
        // and the spec's own check lets all of it through
        assert_eq!(spec.course_problems(), Vec::<String>::new());
        // a long run gets all three, weighted towards the early kinds
        let mut c = Course::new(&spec);
        c.ensure(&spec, 3000.0);
        for i in 0..3 {
            assert!(c.obst.iter().any(|o| o.ety == i), "type {i} never laid");
        }
    }

    #[test]
    fn the_default_course_has_enemies_from_the_first_bars() {
        // Emma's "no enemies exist" report: a default run must show a
        // slime early, whatever the saved best / language / look say.
        let spec = HopSpec::default();
        let bpb = spec.music.beats_per_bar as f32;
        let mut c = Course::new(&spec);
        c.ensure(&spec, 24.0 * bpb + spec.music.lead_in_bars as f32 * bpb);
        let lead = spec.music.lead_in_bars as f32 * bpb;
        let first: Vec<(f32, usize)> = c
            .obst
            .iter()
            .filter(|o| o.kind == Kind::Enemy)
            .map(|o| {
                (
                    ((o.start + o.end) * 0.5 / spec.hero.beat_len_m - lead) / bpb,
                    o.ety,
                )
            })
            .collect();
        eprintln!("enemy bars: {first:?}");
        let early = first.iter().filter(|(bar, _)| *bar < 24.0).count();
        assert!(early >= 3, "only {early} enemies in the first 24 bars");
        assert!(
            first.iter().any(|(b, t)| *b < 12.0 && *t == 0),
            "no slime by bar 12"
        );
        // through the world JSON a real page sends, with a saved best
        let h2 =
            HopBeat::from_json(r#"{"best":5000,"quality":"low","skin":"x","look":"y"}"#).unwrap();
        assert_eq!(h2.spec.enemies.types.len(), 3);
        let mut c2 = h2.course().clone();
        c2.ensure(&h2.spec, 24.0 * bpb + lead);
        assert!(c2.obst.iter().filter(|o| o.kind == Kind::Enemy).count() >= 3);
    }

    #[test]
    fn the_checker_rejects_impossible_enemy_patterns() {
        let bad = |items: Vec<Item>| enemy_spec(items).course_problems();
        // fine: one slime
        assert_eq!(
            bad(vec![item(4.0, "enemy", 0.2, "tap", "slime")]),
            Vec::<String>::new()
        );
        // too tight: a second slime before the first bounce is over
        assert!(!bad(vec![
            item(4.0, "enemy", 0.2, "tap", "slime"),
            item(5.0, "enemy", 0.2, "tap", "slime")
        ])
        .is_empty());
        // a hold hop cannot be made under a bat
        assert!(!bad(vec![
            item(2.0, "gap", 1.0, "hold", ""),
            item(2.0, "enemy", 0.9, "hold", "bat")
        ])
        .is_empty());
        // too wide to clear with a tap hop at the perfect window
        assert!(!bad(vec![item(4.0, "enemy", 1.6, "tap", "slime")]).is_empty());
        // unknown enemy id, an enemy named on a gap, a locked enemy
        assert!(!bad(vec![item(4.0, "enemy", 0.2, "tap", "ghost")]).is_empty());
        assert!(!bad(vec![item(4.0, "gap", 1.0, "tap", "slime")]).is_empty());
        let mut locked = enemy_spec(vec![item(4.0, "enemy", 0.25, "tap", "beetle")]);
        locked.enemies.types[1].unlock_level = 3;
        assert!(!locked.course_problems().is_empty());
        // no rest after a walker
        assert!(!bad(vec![
            item(4.0, "enemy", 0.25, "tap", "beetle"),
            item(5.5, "gap", 1.0, "tap", "")
        ])
        .is_empty());
    }

    #[test]
    fn a_fourth_enemy_is_only_json() {
        // a quick red beetle: four small steps a beat, no new code or model
        let v = json!({"enemies": {"types": [
            {"id": "slime"},
            {"id": "dasher", "look": "beetle", "unlock_level": 0, "y_hi": 0.5, "step_m": 0.75,
             "steps_per_beat": 4, "walk_steps": 12, "color": [255, 70, 70]}
        ]}, "course": {"phrases": [
            {"level": 0, "bars": 2, "items": [
                {"beat": 1.0, "kind": "enemy", "beats": 0.2, "hop": "tap", "enemy": "dasher"},
                {"beat": 3.0, "kind": "enemy", "beats": 0.2, "hop": "tap", "enemy": "slime"}]}]}});
        let spec = HopSpec::from_value(&v).unwrap();
        assert_eq!(spec.enemy_index("dasher"), Some(1));
        assert_eq!(spec.course_problems(), Vec::<String>::new());
        let g = drive(spec, 0.0, 2.0);
        assert_eq!(g.lives(), 3, "the perfect press clears it");
    }

    #[test]
    fn enemy_specs_are_read_from_json_and_refused_when_wrong() {
        let ok = json!({"enemies": {"types": [
            {"id": "ghost", "look": "bat", "y_lo": 1.4, "y_hi": 2.0, "stompable": false}
        ], "stomp_score": 200}, "course": {"phrases": [
            {"level": 0, "bars": 2, "items": [
                {"beat": 1.0, "kind": "enemy", "beats": 0.9, "hop": "tap", "enemy": "ghost"}]}]}});
        let s = HopSpec::from_value(&ok).unwrap();
        assert_eq!(s.enemies.types.len(), 1);
        assert_eq!(s.enemies.stomp_score, 200);
        for bad in [
            json!({"enemies": {"types": [{"id": "x", "bogus": 1}]}}),
            json!({"enemies": {"types": [{"id": "x"}, {"id": "x"}]}}),
            json!({"enemies": {"types": [{"id": "X!"}]}}),
            json!({"enemies": {"types": [{"id": "x", "y_lo": 1.0, "y_hi": 2.0}]}}),
            json!({"enemies": {"types": [{"id": "x", "look": "dragon"}]}}),
            json!({"enemies": {"types": [{"id": "x", "step_m": 3.0, "walk_steps": 8}]}}),
            json!({"enemies": {"body_h": 0.0}}),
            json!({"enemies": {"types": []}, "course": {"phrases": [
                {"level": 0, "bars": 2, "items": [
                    {"beat": 2.0, "kind": "enemy", "beats": 0.2, "hop": "tap", "enemy": "slime"}]}]}}),
        ] {
            assert!(HopSpec::from_value(&bad).is_err(), "{bad}");
        }
    }

    /// One life and a gap on the first beat: standing still, the bunny walks off.
    fn fall_spec(lives: u32) -> HopSpec {
        let mut s = enemy_spec(vec![item(1.0, "gap", 1.5, "hold", "")]);
        s.lives.count = lives;
        s
    }

    fn falling(g: &HopBeat) -> bool {
        g.state_value()["falling"].as_bool().unwrap()
    }

    /// Plays without pressing until the bunny is over the ledge.
    fn until_falling(g: &mut HopBeat) {
        g.confirm();
        for _ in 0..(90.0 * 60.0) as u32 {
            g.tick(1.0 / 60.0);
            if falling(g) {
                return;
            }
        }
        panic!("never fell");
    }

    #[test]
    fn a_fatal_fall_plays_out_and_then_ends_in_game_over() {
        let mut g = HopBeat::new(fall_spec(1));
        g.set_sfx(false);
        until_falling(&mut g);
        assert_eq!(g.phase_name(), "play");
        let (mut frames, mut saw_drop) = (0, false);
        while g.phase_name() == "play" && frames < 600 {
            g.tick(1.0 / 60.0);
            frames += 1;
            saw_drop |= g.dead_fall.is_some();
            // No red flash and no camera snap: a fall is its own picture.
            assert_eq!(g.hit_flash, 0.0);
        }
        let secs = frames as f32 / 60.0;
        assert_eq!(g.phase_name(), "result");
        assert!(saw_drop, "the drop plays on after the run has ended");
        assert!(secs > 0.6 && secs < 1.1, "the card comes after {secs}s");
        assert_eq!(g.core.stats.death, "fell");
        assert_eq!(g.lives(), 0);
    }

    #[test]
    fn the_fall_timing_comes_from_the_spec_and_never_touches_the_sim() {
        let end = |delay: f32, slow: f32| {
            let mut spec = fall_spec(1);
            spec.fall.card_delay_secs = delay;
            spec.fall.slow_scale = slow;
            spec.fall.cam_lag = 0.0;
            spec.fall.dust = 0;
            let mut g = HopBeat::new(spec);
            g.set_sfx(false);
            until_falling(&mut g);
            let mut frames = 0;
            let mut at_death = None;
            while g.phase_name() == "play" && frames < 900 {
                g.tick(1.0 / 60.0);
                frames += 1;
                if at_death.is_none() && g.dead_fall.is_some() {
                    at_death = Some((g.song_t(), g.core.stats.score, g.combo, g.y));
                }
            }
            (
                frames as f32 / 60.0,
                at_death.unwrap(),
                g.state_value()["result"].clone(),
            )
        };
        let (short, a, ra) = end(0.5, 1.0);
        let (long, b, rb) = end(2.0, 0.2);
        assert!(long > short + 1.0, "{short} vs {long}");
        // The run itself ended in the same place, with the same result.
        assert_eq!(a, b);
        assert_eq!(ra, rb);
    }

    #[test]
    fn a_revive_after_a_fall_puts_the_bunny_on_a_safe_tile() {
        let mut g = HopBeat::new(fall_spec(1));
        g.set_sfx(false);
        until_falling(&mut g);
        run(&mut g, 3.0);
        assert_eq!(g.phase_name(), "result");
        assert!(g.dead_fall.is_some());
        assert!(g.revive());
        assert_eq!(g.phase_name(), "play");
        assert!(g.dead_fall.is_none() && !falling(&g));
        assert_eq!((g.y, g.fall_cam, g.fall_age), (0.0, 0.0, 0.0));
        assert!(supported(&g.course.obst, g.hero_x(), g.spec.hero.edge_tol));
        // The camera is back on the bunny, and it stays alive on the next frames.
        assert!((g.view_x() - g.hero_x()).abs() < 1e-3);
        run(&mut g, 0.5);
        assert_eq!(g.phase_name(), "play");
        assert!(g.lives() > 0);
    }

    fn recover_stage(g: &HopBeat) -> String {
        g.state_value()["recover"].as_str().unwrap().to_string()
    }

    /// A fall with lives left: the drop plays, the world holds for READY, the music restarts
    /// on a bar line, and the bunny blinks for a while. Nothing runs during the hold.
    #[test]
    fn a_fall_with_lives_left_gets_a_ready_beat_before_play_resumes() {
        let mut g = HopBeat::new(fall_spec(3));
        g.set_sfx(false);
        until_falling(&mut g);
        let (mut drop, mut ready) = (0, 0);
        let mut ready_song_t = None;
        let mut frames = 0;
        while frames < 60 * 12 {
            g.tick(1.0 / 60.0);
            frames += 1;
            match recover_stage(&g).as_str() {
                "drop" => drop += 1,
                "ready" => {
                    ready += 1;
                    assert!(g.invuln > 0.0, "blinks from the READY on");
                    let t = *ready_song_t.get_or_insert(g.song_t());
                    assert_eq!(g.song_t(), t, "the world is held during READY");
                    assert_eq!(g.lives(), 2);
                    assert_eq!(g.y, 0.0);
                    assert!(g.dead_fall.is_none());
                }
                _ => {
                    if ready > 0 {
                        break;
                    }
                }
            }
        }
        let rc = g.spec.recover.clone();
        let (drop_s, ready_s) = (drop as f32 / 60.0, ready as f32 / 60.0);
        assert!((drop_s - rc.drop_secs).abs() < 0.1, "drop {drop_s}s");
        let want = rc.ready_beats * g.spec.beat_secs();
        assert!(
            (ready_s - want).abs() < 0.1,
            "READY {ready_s}s, want {want}s"
        );
        assert!(
            ready_s > 1.2,
            "a real beat to breathe, not a blink: {ready_s}s"
        );
        // Play is back: on a bar line, on empty slab, blinking.
        let bar = g.spec.beat_secs() * g.spec.music.beats_per_bar as f32;
        let t = ready_song_t.unwrap();
        assert!(((t / bar) - (t / bar).round()).abs() < 1e-3);
        assert!(g.invuln > rc.invuln_secs - 0.1, "{}", g.invuln);
        assert!(g.state_value()["invulnerable"].as_bool().unwrap());
        assert!(g
            .course()
            .obst
            .iter()
            .all(|o| o.take_beat >= g.beat_pos() + rc.clear_beats - 0.1));
        run(&mut g, 0.5);
        assert_eq!(g.phase_name(), "play");
        assert_eq!(g.lives(), 2, "no second hit while it blinks");
    }

    #[test]
    fn the_recover_pause_can_be_turned_off_or_tuned() {
        let end = |f: fn(&mut HopSpec)| {
            let mut spec = fall_spec(3);
            f(&mut spec);
            let mut g = HopBeat::new(spec);
            g.set_sfx(false);
            until_falling(&mut g);
            let mut frames = 0;
            while g.lives() == 3 || !recover_stage(&g).is_empty() || g.dead_fall.is_some() {
                g.tick(1.0 / 60.0);
                frames += 1;
                assert!(frames < 60 * 20);
            }
            frames as f32 / 60.0
        };
        let off = end(|s| s.recover.enabled = false);
        let std = end(|_| {});
        let long = end(|s| s.recover.ready_beats = 6.0);
        assert!(off < 0.6, "{off}");
        assert!(std > off + 2.0, "{std} vs {off}");
        assert!(long > std + 1.0, "{long} vs {std}");
    }

    #[test]
    fn a_continue_also_gets_a_ready_count() {
        let mut g = HopBeat::new(fall_spec(1));
        g.set_sfx(false);
        until_falling(&mut g);
        run(&mut g, 3.0);
        assert!(g.revive());
        assert_eq!(recover_stage(&g), "ready");
        let t = g.song_t();
        run(&mut g, 0.5);
        assert_eq!(g.song_t(), t, "held while READY");
        run(&mut g, 3.0);
        assert_eq!(recover_stage(&g), "");
        assert!(g.song_t() > t);
        assert_eq!(g.phase_name(), "play");
    }

    #[test]
    fn recover_block_is_validated_and_round_trips() {
        let mut v = serde_json::to_value(HopSpec::default()).unwrap();
        v["recover"]["ready_beats"] = serde_json::json!(2.0);
        assert_eq!(HopSpec::from_value(&v).unwrap().recover.ready_beats, 2.0);
        for (k, bad) in [
            ("drop_secs", 0.0),
            ("ready_beats", 20.0),
            ("invuln_secs", -1.0),
            ("clear_beats", 99.0),
        ] {
            let mut v = serde_json::to_value(HopSpec::default()).unwrap();
            v["recover"][k] = serde_json::json!(bad);
            assert!(HopSpec::from_value(&v).is_err(), "{k}");
        }
        let mut v = serde_json::to_value(HopSpec::default()).unwrap();
        v["recover"]["nope"] = serde_json::json!(1);
        assert!(HopSpec::from_value(&v).is_err());
    }

    #[test]
    fn a_fall_with_lives_left_keeps_playing_and_recovers_the_camera() {
        let mut g = HopBeat::new(fall_spec(3));
        g.set_sfx(false);
        until_falling(&mut g);
        run(&mut g, 1.5);
        assert_eq!(g.phase_name(), "play");
        assert_eq!(g.lives(), 2);
        assert!(!falling(&g) && g.dead_fall.is_none());
        assert!(g.fall_cam < 0.05, "{}", g.fall_cam);
    }

    #[test]
    fn the_fall_is_drawn_on_every_tier_and_without_models() {
        for q in ["low", "mid", "high"] {
            for off in [false, true] {
                let mut g = HopBeat::from_json(&format!(r#"{{"quality":"{q}"}}"#)).unwrap();
                g.spec.lives.count = 1;
                g.spec.course = fall_spec(1).course;
                g.set_sfx(false);
                g.set_models_off(off);
                until_falling(&mut g);
                let mut y_prev = f32::MAX;
                for f in 0..70 {
                    g.tick(1.0 / 60.0);
                    let body = g
                        .document()
                        .props
                        .iter()
                        .find(|p| p.id == "prop:hero-body")
                        .unwrap_or_else(|| panic!("{q} {off}: no hero at frame {f}"));
                    assert!(body.position.iter().all(|v| v.is_finite()), "{q} {off}");
                    assert!(body.scale.iter().all(|v| v.is_finite() && *v > 0.0));
                    assert!(body.position[1] < y_prev + 0.08, "{q} {off}: rises at {f}");
                    y_prev = body.position[1];
                    if f > 20 {
                        // The model turns as a whole; the ball turns its ears.
                        let turned = g
                            .document()
                            .props
                            .iter()
                            .filter(|p| p.id.starts_with("prop:hero-ear"))
                            .any(|p| p.roll != 0.0);
                        assert!(turned, "{q} {off}: no tumble");
                    }
                }
            }
        }
    }

    #[test]
    fn fall_block_is_validated() {
        for bad in [
            json!({"fall": {"card_delay_secs": 0.0}}),
            json!({"fall": {"card_delay_secs": 9.0}}),
            json!({"fall": {"slow_scale": 0.0}}),
            json!({"fall": {"cam_lag": 2.0}}),
            json!({"fall": {"dust": 999}}),
            json!({"fall": {"bogus": 1}}),
        ] {
            assert!(HopSpec::from_value(&bad).is_err(), "{bad}");
        }
        let ok = HopSpec::from_value(&json!({"fall": {"card_delay_secs": 1.2}})).unwrap();
        assert_eq!(ok.fall.card_delay_secs, 1.2);
    }

    #[test]
    fn enemies_are_drawn_from_models_and_from_primitives_alike() {
        let enemy_props = |off: bool| {
            let mut g = HopBeat::new(enemy_spec(vec![item(3.0, "enemy", 0.2, "tap", "slime")]));
            g.set_sfx(false);
            g.set_models_off(off);
            g.confirm();
            run(&mut g, 2.0);
            g.document()
                .props
                .iter()
                .filter(|p| p.id.contains("enemy-"))
                .map(|p| (p.id.clone(), p.gltf.is_some()))
                .collect::<Vec<_>>()
        };
        let with = enemy_props(false);
        assert!(with.iter().any(|(_, glb)| *glb), "{with:?}");
        let without = enemy_props(true);
        assert!(
            !without.is_empty() && without.iter().all(|(_, glb)| !*glb),
            "{without:?}"
        );
    }

    #[test]
    fn default_runs_always_draw_enemies_with_a_real_transform() {
        // Regression for "the enemies are gone": every quality tier, a fresh run
        // shows enemies early, and none is ever drawn at zero size, non-finite
        // or disabled.
        for q in ["", "low", "mid", "high"] {
            let json = if q.is_empty() {
                "{}".to_string()
            } else {
                format!(r#"{{"quality":"{q}"}}"#)
            };
            let mut g = HopBeat::from_json(&json).unwrap();
            g.set_sfx(false);
            g.confirm();
            let (mut seen, mut first_t) = (0usize, None);
            for f in 0..(30.0 * 60.0) as u32 {
                g.tick(1.0 / 60.0);
                for p in g
                    .document()
                    .props
                    .iter()
                    .filter(|p| p.name == "enemy" || p.id.starts_with("enemy-"))
                {
                    if p.id.ends_with("-shadow") {
                        continue;
                    }
                    seen += 1;
                    first_t.get_or_insert(f as f32 / 60.0);
                    assert!(p.enabled, "{q}: {} is disabled", p.id);
                    assert!(
                        p.position.iter().all(|v| v.is_finite())
                            && p.yaw.is_finite()
                            && p.roll.is_finite(),
                        "{q}: {} has a bad transform {:?}",
                        p.id,
                        p.position
                    );
                    // the smallest scale is the 5 % "appearing" floor of a 0.3 m pose
                    assert!(
                        p.scale.iter().all(|v| v.is_finite() && *v > 0.01),
                        "{q}: {} scale {:?}",
                        p.id,
                        p.scale
                    );
                }
            }
            assert!(seen > 60, "{q}: only {seen} enemy prop frames in 30 s");
            assert!(first_t.unwrap() < 6.0, "{q}: first enemy at {first_t:?} s");
        }
    }

    #[test]
    fn a_missing_model_is_not_ready_and_never_panics() {
        assert!(!glb_ready("assets/models/hop_beat/does_not_exist.glb"));
        assert!(!glb_ready(""));
    }

    #[test]
    fn picture_stays_inside_the_prop_budget_and_round_trips() {
        let mut g = fresh();
        g.confirm();
        let mut most = 0;
        for _ in 0..(40.0 * 60.0) as u32 {
            g.tick(1.0 / 60.0);
            most = most.max(g.document().props.len());
        }
        assert!(most > 20 && most < 400, "{most} props");
        let json = g.document().to_json().unwrap();
        let back = crate::world_doc::WorldDoc::from_json(&json).unwrap();
        assert_eq!(back.props.len(), g.document().props.len());
        assert_eq!(back.cameras.len(), 1);
        let ids: std::collections::HashSet<_> =
            g.document().props.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids.len(), g.document().props.len(), "duplicate prop ids");
    }

    #[test]
    fn one_button_taps_and_holds_pick_different_hops() {
        let mut g = fresh();
        g.confirm();
        run(&mut g, 1.0);
        // Hold
        g.set_hold(true);
        g.tick(1.0 / 60.0);
        run(&mut g, 0.3);
        assert!(!matches!(g.state_value()["grounded"].as_bool(), Some(true)));
        let hold_peak = {
            let mut m = 0.0_f32;
            for _ in 0..90 {
                g.tick(1.0 / 60.0);
                m = m.max(g.hero_y());
            }
            m
        };
        g.set_hold(false);
        run(&mut g, 2.0);
        // Tap
        let mut t = fresh();
        t.confirm();
        run(&mut t, 1.0);
        t.set_hold(true);
        t.tick(1.0 / 60.0);
        t.set_hold(false);
        let mut tap_peak = 0.0_f32;
        for _ in 0..60 {
            t.tick(1.0 / 60.0);
            tap_peak = tap_peak.max(t.hero_y());
        }
        assert!(
            hold_peak > tap_peak + 0.3,
            "hold {hold_peak} tap {tap_peak}"
        );
    }

    #[test]
    fn keyboard_space_is_the_one_button() {
        let mut g = fresh();
        g.confirm();
        assert!(g.on_key("space", true));
        g.tick(1.0 / 60.0);
        assert!(g.hold_down());
        assert!(g.on_key("space", false));
        assert!(!g.hold_down());
    }

    #[test]
    fn the_session_loads_it_as_a_boxed_kit_and_one_finger_is_the_button() {
        use crate::input::{PointerEvent, PointerPhase};
        use crate::session::SharedSession;
        let json = include_str!("../tests/fixtures/hop_beat_world.json");
        let mut s = SharedSession::default();
        s.load_game_json(json).unwrap();
        s.create_surface(800, 450);
        assert!(s.hop().is_some() && s.cat().is_none());
        assert!(s.uses_swipe());
        s.set_direct_play(true);
        let at = |phase| PointerEvent {
            id: 1,
            x: 400.0,
            y: 300.0,
            phase,
            pressure: 1.0,
        };
        // A tap outside the buttons starts the run (title -> play).
        s.push_pointer(at(PointerPhase::Begin));
        s.push_pointer(at(PointerPhase::End));
        s.request_frame();
        assert_eq!(s.hop().unwrap().phase_name(), "play");
        // The next finger down is the hop button, and lifting lets go of it.
        s.push_pointer(at(PointerPhase::Begin));
        assert!(s.hop().unwrap().hold_down());
        s.push_pointer(at(PointerPhase::End));
        assert!(!s.hop().unwrap().hold_down());
        // The pad does not steer a one-button game.
        s.set_pad(1.0, 0.0);
        assert!(!s.hop().unwrap().hold_down());
        // Host hooks reach the kit.
        s.set_latency_ms(40.0);
        assert_eq!(s.hop().unwrap().latency_ms(), 40.0);
        assert!(!s.revive(), "nothing to continue from before a game over");
        assert!(s.hop().unwrap().state_json().contains("\"music_t\""));
    }
}
