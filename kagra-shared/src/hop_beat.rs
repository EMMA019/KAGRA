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

pub const SFX_CUES: [&str; 10] = [
    "hop", "land", "perfect", "good", "combo", "hit", "fall", "gameover", "revive", "newbest",
];

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
            tap_window_secs: 0.11,
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
    /// `gap` or `block`.
    pub kind: String,
    /// Width in beats.
    pub beats: f32,
    /// `tap` or `hold`: the hop this obstacle is written for.
    pub hop: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phrase {
    /// 0 = the first bars of a run, higher is harder.
    pub level: u32,
    pub bars: u32,
    pub items: Vec<Item>,
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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlaySpec {
    /// Land in the run itself: no title card, the first tap starts the music.
    pub direct: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LookSpec {
    pub hero: [u32; 3],
    pub slab: [u32; 3],
    pub glow: [u32; 3],
    pub accent: [u32; 3],
}

impl Default for LookSpec {
    fn default() -> Self {
        Self {
            hero: [255, 214, 120],
            slab: [58, 52, 92],
            glow: [96, 232, 255],
            accent: [255, 92, 170],
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
                file: format!("{AUDIO_DIR}/loop_120.wav"),
                volume: 0.8,
                fade_secs: 0.3,
            },
            sfx: SFX_CUES
                .iter()
                .map(|n| {
                    (
                        n.to_string(),
                        SfxSpec {
                            file: format!("{AUDIO_DIR}/{n}.wav"),
                            volume: if *n == "land" { 0.5 } else { 0.9 },
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
    pub play: PlaySpec,
    pub look: LookSpec,
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
            play: PlaySpec::default(),
            look: LookSpec::default(),
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
        let r = &self.revive;
        finite_in("revive.invuln_secs", r.invuln_secs, 0.0, 10.0)?;
        finite_in("revive.clear_beats", r.clear_beats, 0.0, 32.0)?;
        if !(1..=9).contains(&r.lives) || r.max_per_run > 9 {
            return Err("hop spec: revive.lives 1..=9, max_per_run 0..=9".into());
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
        let problems = self.course_problems();
        if let Some(first) = problems.first() {
            return Err(format!("hop spec: {first} ({} problem(s))", problems.len()));
        }
        Ok(())
    }

    /// The phrases in use: the spec's, or the built-in set.
    pub fn phrases(&self) -> Vec<Phrase> {
        if self.course.phrases.is_empty() {
            builtin_phrases()
        } else {
            self.course.phrases.clone()
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
            let mut free_from = 0.0_f32;
            for (ii, it) in p.items.iter().enumerate() {
                let where_ = format!("phrase {pi} item {ii}");
                let Some((kind, hop)) = parse_item(it) else {
                    out.push(format!("{where_}: kind gap|block and hop tap|hold"));
                    continue;
                };
                if !(0.1..=4.0).contains(&it.beats) || it.beat < 0.0 {
                    out.push(format!("{where_}: bad beat or width"));
                    continue;
                }
                let air = self.arc(hop).air_beats;
                if it.beat + 0.001 < free_from {
                    out.push(format!("{where_}: starts before the last hop lands"));
                }
                free_from = it.beat + air;
                if free_from > p.bars as f32 * bpb + 0.001 {
                    out.push(format!("{where_}: lands after the phrase ends"));
                }
                let (lo, hi) = self.window_secs(kind, hop, it.beats);
                let width_ms = (hi - lo).max(0.0) * 1000.0;
                if width_ms < self.course.min_window_ms {
                    out.push(format!(
                        "{where_}: only {width_ms:.0} ms of timing window (gap too wide for a {} hop?)",
                        if hop == Hop::Tap { "tap" } else { "hold" }
                    ));
                }
            }
        }
        out
    }

    /// The press times (relative to the ideal, seconds) that clear an
    /// obstacle written for `hop`, found by playing the hop out.
    pub fn window_secs(&self, kind: Kind, hop: Hop, width_beats: f32) -> (f32, f32) {
        let bs = self.beat_secs();
        let step = 0.002;
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        let mut off = -2.0 * bs;
        while off <= 2.0 * bs {
            if self.clears(kind, hop, width_beats, off) {
                lo = lo.min(off);
                hi = hi.max(off);
            }
            off += step;
        }
        if lo > hi {
            (0.0, 0.0)
        } else {
            (lo, hi)
        }
    }

    /// Does a hop of `hop` pressed `off` seconds from the ideal clear this
    /// obstacle? Uses the same support and hit tests as the sim.
    pub fn clears(&self, kind: Kind, hop: Hop, width_beats: f32, off: f32) -> bool {
        let bs = self.beat_secs();
        let v = self.speed();
        let arc = self.arc(hop);
        let air = arc.air_beats * bs;
        // The ideal press is at t = 0; the obstacle is centred on the arc.
        let center = air * 0.5 * v;
        let w = width_beats * self.hero.beat_len_m;
        let obst = Obst::new(
            kind,
            center - w / 2.0,
            center + w / 2.0,
            0.0,
            hop,
            self.course.block_height,
        );
        let take = off;
        let hw = self.hero.half_width;
        let mut t = take - 3.0 * bs;
        while t <= take + air + 3.0 * bs {
            let x = t * v;
            let y = if t < take {
                0.0
            } else if t < take + air {
                arc_y(arc.height, air, t - take)
            } else {
                0.0
            };
            let airborne = t >= take && t < take + air;
            if !airborne && !supported(std::slice::from_ref(&obst), x, self.hero.edge_tol) {
                return false;
            }
            if hits_block(&obst, x, y, hw) {
                return false;
            }
            t += 0.004;
        }
        true
    }
}

/// `kind` and `hop` of an item, or None if either word is wrong.
pub fn parse_item(it: &Item) -> Option<(Kind, Hop)> {
    let kind = match it.kind.as_str() {
        "gap" => Kind::Gap,
        "block" => Kind::Block,
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
    }
}

/// The phrase set the game ships with: five levels, one to three bars each.
pub fn builtin_phrases() -> Vec<Phrase> {
    let p = |level, bars, items| Phrase { level, bars, items };
    vec![
        p(
            0,
            2,
            vec![item(1.0, "gap", 0.5, "tap"), item(3.0, "gap", 0.5, "tap")],
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
        p(
            2,
            2,
            vec![
                item(0.5, "gap", 0.5, "tap"),
                item(1.5, "block", 0.2, "tap"),
                item(2.5, "gap", 0.5, "tap"),
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
    ]
}

// -------------------------------------------------------------- course ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Gap,
    Block,
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
    pub height: f32,
    pub fate: Fate,
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
        }
    }
}

/// Height of a hop `el` seconds after takeoff.
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

    /// Lay phrases until the course reaches `until_beat`.
    pub fn ensure(&mut self, spec: &HopSpec, until_beat: f32) {
        let bpb = spec.music.beats_per_bar as f32;
        while self.gen_beat < until_beat {
            if self.since_rest >= spec.course.rest_every_bars {
                self.since_rest = 0;
                self.gen_beat += bpb;
                self.bars_laid += 1;
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
            if pool.len() > 1 {
                pool.retain(|&i| i != self.last);
            }
            let pick = pool[(self.rand() as usize) % pool.len()];
            self.last = pick;
            let p = self.phrases[pick].clone();
            for it in &p.items {
                let Some((kind, hop)) = parse_item(it) else {
                    continue;
                };
                let air = spec.arc(hop).air_beats;
                let take = self.gen_beat + it.beat;
                let center = take + air * 0.5;
                let (s, e) = (center - it.beats * 0.5, center + it.beats * 0.5);
                self.obst.push(Obst::new(
                    kind,
                    s * spec.hero.beat_len_m,
                    e * spec.hero.beat_len_m,
                    take,
                    hop,
                    spec.course.block_height,
                ));
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grade {
    Perfect,
    Good,
    Ok,
}

impl Grade {
    pub fn name(self) -> &'static str {
        match self {
            Grade::Perfect => "perfect",
            Grade::Good => "good",
            Grade::Ok => "ok",
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
    just_hop: bool,
    sfx: bool,
    time: f32,
    attract: f32,
    anim_t: f32,
    pending_finish: bool,
}

impl HopBeat {
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
            song_t: 0.0,
            clock_base: 0.0,
            clock: None,
            latency_user_ms: 0.0,
            armed: false,
            air: Air::Ground,
            recent_hop: None,
            ground_t: f32::MIN,
            headless: false,
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
            just_hop: false,
            sfx: true,
            time: 0.0,
            attract: 0.0,
            anim_t: 0.0,
            pending_finish: false,
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
    pub fn set_headless(&mut self, on: bool) {
        self.headless = on;
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
            FlowAction::Resume => self.flow.resume(),
            FlowAction::ToTitle => self.show_title(),
            FlowAction::Pause => self.flow.pause(),
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
        let bar = self.spec.beat_secs() * self.spec.music.beats_per_bar as f32;
        self.song_t = (self.song_t / bar).ceil() * bar;
        let beat = self.beat_pos();
        let clear = beat + self.spec.revive.clear_beats;
        self.course.clear_before(clear);
        self.course
            .ensure(&self.spec, beat + AHEAD_M / self.spec.hero.beat_len_m + 8.0);
        self.core.stats.alive = true;
        self.core.stats.death.clear();
        self.pending_finish = false;
        self.core.juice.hitstop = 0.0;
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
        self.begin();
        self.cue("revive");
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
        self.hit_flash = 1.0;
        self.lives = self.lives.saturating_sub(1);
        if self.lives == 0 {
            self.core.stats.die(reason);
            self.core.juice.hit(0.7, 0.14);
            self.cue(if reason == "fell" { "fall" } else { "hit" });
            self.pending_finish = true;
            self.cue("gameover");
            return;
        }
        self.cue(if reason == "fell" { "fall" } else { "hit" });
        self.core.juice.shake = self.core.juice.shake.max(0.35);
        self.grant_invuln(self.spec.lives.invuln_secs);
        if reason == "fell" {
            // Second wind: the hero floats until there is slab again.
            self.air = Air::Ground;
            self.y = 0.0;
        }
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
        self.core.stats.just_jump = true;
        self.stray_check = None;
        self.cue("hop");
    }

    fn land(&mut self, id: u32) {
        self.land_squash = 1.0;
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
            Grade::Ok => "land",
        });
        let hx = self.hero_x();
        if grade == Grade::Perfect {
            let (c, n) = (self.spec.look.accent, 7);
            self.core.juice.burst(hx, 0.3, 0.0, n, c, 2.8, 0.09);
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
            Air::Fall { vy } => {
                let vy = vy - hero.fall_gravity * dt;
                self.y += vy * dt;
                self.air = Air::Fall { vy };
                if self.y <= 0.0 && self.y > -0.4 && supported(&self.course.obst, x, hero.edge_tol)
                {
                    self.y = 0.0;
                    self.ground_t = self.song_t;
                    self.air = Air::Ground;
                    self.land_squash = 0.6;
                } else if self.y < -0.35 && self.invuln > 0.0 {
                    self.y = 0.0;
                    self.air = Air::Ground;
                } else if self.y < -3.5 {
                    self.y = -3.5;
                    self.lose_life("fell");
                    if self.core.stats.alive {
                        self.y = 0.0;
                        self.air = Air::Ground;
                    }
                }
            }
        }

        // ---- obstacles
        let hw = hero.half_width;
        let y = self.y;
        let mut hit_block = false;
        let mut resolved: Vec<usize> = Vec::new();
        for (i, o) in self.course.obst.iter().enumerate() {
            if o.fate != Fate::Pending {
                continue;
            }
            if o.start > x + 20.0 {
                break;
            }
            if hits_block(o, x, y, hw) {
                hit_block = true;
            }
            if x > o.end + hw {
                resolved.push(i);
            }
        }
        if hit_block {
            if self.invuln > 0.0 {
                // ghosting through: those blocks do not count
                for o in self.course.obst.iter_mut() {
                    if o.fate == Fate::Pending && hits_block(o, x, y, hw) {
                        o.fate = Fate::Skipped;
                    }
                }
            } else {
                self.lose_life("block");
                for o in self.course.obst.iter_mut() {
                    if o.fate == Fate::Pending && hits_block(o, x, y, hw) {
                        o.fate = Fate::Skipped;
                    }
                }
            }
        }
        for i in resolved {
            if self.course.obst[i].fate != Fate::Pending {
                continue;
            }
            self.course.obst[i].fate = Fate::Cleared;
            let o = self.course.obst[i].clone();
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
            },
            "can_revive": self.can_revive(),
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
            "good": self.goods,
            "ok": self.oks,
            "alive": self.core.stats.alive,
            "death": self.core.stats.death,
            "death_text": "",
            "armed": self.armed,
            "direct": self.spec.play.direct,
            "invulnerable": self.invuln > 0.0,
            "song_t": self.song_t,
            "beat": beat,
            "bpm": self.spec.music.bpm,
            "latency_ms": self.latency_user_ms + self.spec.music.offset_ms,
            "can_revive": self.can_revive(),
            "revives": self.revives,
            "x": self.hero_x(),
            "y": self.y,
            "grounded": matches!(self.air, Air::Ground),
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
            list.quads.push(Quad::new(
                10.0 * s,
                8.0 * s,
                150.0 * s,
                32.0 * s,
                [8, 6, 24, 150],
            ));
            txt(
                &mut list,
                &format!("SCORE {}", self.core.stats.score),
                18.0 * s,
                14.0 * s,
                15.0,
                [glow[0], glow[1], glow[2], 255],
                TextAlign::Left,
            );
            // lives: one pip each
            for i in 0..self.spec.lives.count.max(self.lives) {
                let on = i < self.lives;
                let blink = self.invuln > 0.0 && ((self.anim_t * 8.0) as u32).is_multiple_of(2);
                let c = if on && !blink {
                    [accent[0], accent[1], accent[2], 255]
                } else if on {
                    [255, 255, 255, 120]
                } else {
                    [60, 56, 80, 150]
                };
                list.quads.push(Quad::new(
                    w - (22.0 + i as f32 * 22.0) * s,
                    14.0 * s,
                    16.0 * s,
                    16.0 * s,
                    c,
                ));
            }
            if self.combo >= 2 {
                let pulse = 1.0 + 0.25 * self.beat_flash;
                txt(
                    &mut list,
                    &format!("{} COMBO", self.combo),
                    w * 0.5,
                    h * 0.12,
                    22.0 * pulse,
                    [255, 236, 150, 255],
                    TextAlign::Center,
                );
            }
            if let Some((g, err, _)) = self.last_grade {
                if self.grade_age < 0.7 {
                    let a = (255.0 * (1.0 - self.grade_age / 0.7)).clamp(0.0, 255.0) as u8;
                    let (word, c) = match g {
                        Grade::Perfect => ("PERFECT", [255, 236, 150, a]),
                        Grade::Good => ("GOOD", [glow[0], glow[1], glow[2], a]),
                        Grade::Ok => ("OK", [200, 200, 220, a]),
                    };
                    txt(
                        &mut list,
                        word,
                        w * 0.5,
                        h * 0.60,
                        20.0,
                        c,
                        TextAlign::Center,
                    );
                    if g != Grade::Perfect {
                        let side = if err < 0.0 { "EARLY" } else { "LATE" };
                        txt(
                            &mut list,
                            side,
                            w * 0.5,
                            h * 0.60 + 26.0 * s,
                            11.0,
                            [c[0], c[1], c[2], a / 2],
                            TextAlign::Center,
                        );
                    }
                }
            }
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
            if self.armed {
                list.quads
                    .push(Quad::new(0.0, h * 0.40, w, h * 0.16, [8, 6, 24, 150]));
                txt(
                    &mut list,
                    "TAP TO START",
                    w * 0.5,
                    h * 0.44,
                    22.0,
                    [255, 255, 255, 255],
                    TextAlign::Center,
                );
                txt(
                    &mut list,
                    "TAP = SHORT HOP   HOLD = LONG HOP",
                    w * 0.5,
                    h * 0.50,
                    11.0,
                    [glow[0], glow[1], glow[2], 230],
                    TextAlign::Center,
                );
            }
        }
        if menu {
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

impl HopBeat {
    /// Where the camera looks from. The course runs along +Z.
    fn view_x(&self) -> f32 {
        match self.flow.phase {
            FlowPhase::Title => 0.0,
            _ => self.hero_x(),
        }
    }

    fn build_doc(&mut self) {
        let look = self.spec.look.clone();
        let hx = self.view_x();
        let bl = self.spec.hero.beat_len_m;
        let beat = hx / bl;
        let pulse = self.beat_flash;
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: 90.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([14, 10, 40, 255]),
            hemi_sky: Some([0.20, 0.16, 0.42]),
            hemi_ground: Some([0.10, 0.06, 0.14]),
            env_map: Some("preset:studio".into()),
            shadow_softness: Some(1.0),
            bloom: Some(0.16),
            bloom_threshold: Some(1.4),
            vignette: Some(0.6),
            lut: Some("cool".into()),
            lut_strength: Some(0.2),
            fxaa: Some(true),
            exposure: Some(0.9),
            ibl: Some(0.3),
            tonemap: Some(true),
            cull: Some(true),
            prop_lod: Some(90.0),
            fog_start: Some(34.0),
            fog_end: Some(92.0),
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
            position: [shake[0], 3.1 + lift * 0.5 + shake[1], hx - 6.6],
            target: [0.0, 0.5 + lift, hx + 7.5],
            fov: 64.0,
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
        // ---- lanterns: speed cue, pulse with the beat
        let step = 2.0 * bl;
        let mut z = (lo / step).ceil() * step;
        let mut n = 0;
        while z < hi {
            let k = (z / step) as i64;
            let on = (1.0 - ((z - hx).abs() / 6.0)).clamp(0.0, 1.0);
            for side in [-1.0_f32, 1.0] {
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
            [-16.0, 20.0, hx + 80.0],
            [9.0, 9.0, 9.0],
            [255, 236, 200],
            1.1 + 0.4 * pulse,
            0.6,
            0.0,
        ));
        for i in 0..14 {
            let sx = if i % 2 == 0 { -1.0 } else { 1.0 };
            let off = 9.0 + ((i * 7) % 5) as f32 * 3.5;
            let hgt = 3.0 + ((i * 13) % 7) as f32 * 1.8;
            let zz = (hx * 0.9 / 12.0).floor() * 12.0 + i as f32 * 8.0 - 20.0;
            doc.props.push(prop(
                &format!("tower-{i}"),
                "box",
                "tower",
                [sx * off, hgt * 0.5 - 1.0, zz + hx * 0.1 % 12.0],
                [3.0, hgt, 3.0],
                mix([30, 26, 70], look.accent, ((i * 3) % 5) as f32 * 0.04),
                0.15 + 0.2 * (((i + (beat as i32)) % 3) as f32 / 3.0),
                0.7,
                0.0,
            ));
        }

        // ---- hero
        self.push_hero(&mut doc, hx);
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

    fn push_hero(&self, doc: &mut WorldDoc, hx: f32) {
        let look = &self.spec.look;
        let title = self.flow.phase == FlowPhase::Title;
        let (mut y, mut squash, mut stretch) = (self.y + self.y_vis_off, self.land_squash, 0.0);
        if title {
            let b = (self.attract / self.spec.beat_secs()).fract();
            y = 4.0 * 0.5 * b * (1.0 - b);
            squash = if b < 0.1 { 0.5 } else { 0.0 };
        } else if !matches!(self.air, Air::Ground) {
            stretch = 0.18;
        }
        let blink = self.invuln > 0.0 && ((self.anim_t * 10.0) as u32).is_multiple_of(2);
        let (sx, sy) = (
            1.0 + 0.22 * squash - 0.06 * stretch,
            1.0 - 0.28 * squash + stretch,
        );
        let r = 0.40;
        let z = hx;
        let gy = y.max(0.0);
        let body_c = if self.hit_flash > 0.2 {
            [255, 255, 255]
        } else {
            look.hero
        };
        let em = if blink {
            0.1
        } else {
            0.18 + 0.25 * self.hit_flash
        };
        {
            // shadow blob, only while there is slab below
            let over = supported(&self.course.obst, hx, 0.0) || title;
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
            }
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
                [sx2 * 0.2, gy + r * 2.0 * sy + 0.06, z - 0.02],
                [0.14, 0.24, 0.1],
                body_c,
                em,
                0.4,
                0.0,
            ));
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
    fn picture_stays_inside_the_prop_budget_and_round_trips() {
        let mut g = fresh();
        g.confirm();
        let mut most = 0;
        for _ in 0..(40.0 * 60.0) as u32 {
            g.tick(1.0 / 60.0);
            most = most.max(g.document().props.len());
        }
        assert!(most > 20 && most < 260, "{most} props");
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
}
