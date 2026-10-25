//! Enemies: the genre-neutral half. JSON-defined creature types, how they move,
//! when they unlock, how often they are picked, what touching one does, and the
//! fairness checks every kit can run on its own placements.
//!
//! What this module does **not** know: what a stage, a lane, a beat or a pixel
//! is. A kit keeps its own clock and its own coordinates and supplies three
//! things (see `docs/enemy_kit.md`):
//!
//! 1. **Placement**: where an enemy of type `i` stands (or when it spawns). The
//!    kit turns that into [`Slot`]s (a start time and a length in *its* time unit).
//! 2. **Response**: what a [`Touch::Hit`] and a [`Touch::Stomp`] do (a life, a
//!    score, a bounce). This module only classifies the contact.
//! 3. **Clearability**: how to play a pattern with the allowed inputs ([`Clearable`]).
//!    [`sweep`] then runs it at every offset inside the judge window.
//!
//! [`EnemySpec`] is the `enemies` section of a GameSpec: add a creature by
//! adding an entry to `types`. Movement, hover height, stomp rule, unlock stage
//! and colour all come from that JSON. The `look` word is the only kit-specific
//! part: the kit lists the looks it can draw ([`EnemySpec::validate`]) and falls
//! back to primitives for the rest.

use serde::{Deserialize, Serialize};

/// The farthest an enemy may stand from its meeting spot (steps x metres).
pub const MAX_WALK_M: f32 = 12.0;

/// One kind of enemy.
///
/// It stands where the kit places it, or takes `step_m` toward the hero every
/// `steps_per_beat`-th of the kit's time unit, so the move that clears it stays
/// on the kit's own grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnemyType {
    /// Name used by the kit's placements.
    pub id: String,
    /// Placements must be at least this stage/level.
    pub unlock_level: u32,
    /// The hurt band above the ground (metres). `y_lo` 0 = it stands on the
    /// ground; above 0 it hovers, and reaching the band hurts.
    pub y_lo: f32,
    pub y_hi: f32,
    /// Coming down on it (within `stomp_depth` of its top) squashes it for a
    /// bonus. Only for enemies on the ground.
    pub stompable: bool,
    /// Metres it moves toward the hero at each step; 0 = stays put.
    pub step_m: f32,
    /// Steps per time unit (a beat, a second, a tick: the kit decides).
    pub steps_per_beat: u32,
    /// Steps it has walked when the hero reaches it: it appears that many steps out.
    pub walk_steps: u32,
    /// Peak height (metres) of the leap this enemy makes on every step. 0 = it
    /// does not jump (a slime sits, a beetle walks). A jumper rests on the
    /// ground, then covers its `step_m` through the air in one arc, so it lands
    /// on the step grid: `steps_per_beat` is its jump period and
    /// `step_m * steps_per_beat` its approach speed (metres per time unit).
    pub jump_height: f32,
    /// Share of a step the leap takes (the rest is spent on the ground). 0.2..=1.
    pub jump_air: f32,
    /// Share of a step, right before the leap, spent crouching: the kit's
    /// "it is about to jump" tell. 0..=1 - `jump_air`.
    pub telegraph: f32,
    /// Picture the kit draws it with; see [`EnemySpec::validate`].
    pub look: String,
    pub color: [u32; 3],
}

impl Default for EnemyType {
    fn default() -> Self {
        Self {
            id: "slime".into(),
            unlock_level: 0,
            y_lo: 0.0,
            y_hi: 0.38,
            stompable: true,
            step_m: 0.0,
            steps_per_beat: 1,
            walk_steps: 0,
            jump_height: 0.0,
            jump_air: 0.5,
            telegraph: 0.25,
            look: "slime".into(),
            color: [120, 240, 150],
        }
    }
}

impl EnemyType {
    /// A hovering enemy is something to stay clear of, not something to answer.
    pub fn is_flyer(&self) -> bool {
        self.y_lo > 0.0
    }

    /// A jumper leaves the ground on every step (see `jump_height`).
    pub fn is_jumper(&self) -> bool {
        self.jump_height > 0.0
    }

    /// Approach speed in metres per time unit (a beat).
    pub fn speed_m(&self) -> f32 {
        self.step_m * self.steps_per_beat as f32
    }
}

/// The `enemies` section of a GameSpec.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnemySpec {
    pub types: Vec<EnemyType>,
    /// How tall the hero is where a hovering enemy is concerned (metres).
    pub body_h: f32,
    /// A stomp counts when the hero's feet are this close under the enemy's top.
    pub stomp_depth: f32,
    pub stomp_score: u32,
    /// How high the bounce after a stomp goes.
    pub bounce_height: f32,
    /// Time units a bounce can run past the hop's landing; [`spacing_problems`]
    /// keeps the next action at least this far from a stompable enemy.
    pub bounce_beats: f32,
    /// Free time units kept between an enemy's action and the ones before and after it.
    pub rest_beats: f32,
}

impl Default for EnemySpec {
    /// The starter roster: slime (stage 0), beetle (1), bat (2), hopper (3).
    fn default() -> Self {
        Self {
            types: vec![
                EnemyType::default(),
                EnemyType {
                    id: "beetle".into(),
                    unlock_level: 1,
                    y_hi: 0.42,
                    step_m: 1.0,
                    steps_per_beat: 2,
                    walk_steps: 8,
                    look: "beetle".into(),
                    color: [255, 170, 80],
                    ..EnemyType::default()
                },
                EnemyType {
                    id: "bat".into(),
                    unlock_level: 2,
                    y_lo: 1.75,
                    y_hi: 2.4,
                    stompable: false,
                    look: "bat".into(),
                    color: [190, 130, 255],
                    ..EnemyType::default()
                },
                // The jumper: a frog that leaps 3 m at the hero on every beat, half a beat
                // in the air and a crouch before it goes. 3 m a beat against the beetle's 2.
                EnemyType {
                    id: "hopper".into(),
                    unlock_level: 3,
                    y_hi: 0.42,
                    step_m: 3.0,
                    steps_per_beat: 1,
                    walk_steps: 4,
                    jump_height: 0.4,
                    jump_air: 0.5,
                    telegraph: 0.25,
                    look: "frog".into(),
                    color: [170, 235, 80],
                    ..EnemyType::default()
                },
            ],
            body_h: 0.45,
            stomp_depth: 0.3,
            stomp_score: 150,
            bounce_height: 0.7,
            bounce_beats: 0.5,
            rest_beats: 0.5,
        }
    }
}

/// What the hero touches of an enemy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Touch {
    None,
    /// Side or from below, or a flyer: costs a life in most kits.
    Hit,
    /// Came down on top of a stompable enemy.
    Stomp,
}

/// The hero's box for one contact test.
#[derive(Clone, Copy, Debug)]
pub struct HeroBox {
    pub x: f32,
    /// Feet height above the ground.
    pub y: f32,
    pub half_width: f32,
    pub descending: bool,
}

fn finite_in(who: &str, name: &str, v: f32, lo: f32, hi: f32) -> Result<(), String> {
    if v.is_finite() && (lo..=hi).contains(&v) {
        Ok(())
    } else {
        Err(format!("{who}: {name} must be {lo}..={hi}"))
    }
}

impl EnemySpec {
    /// Index of the type called `id`.
    pub fn index(&self, id: &str) -> Option<usize> {
        self.types.iter().position(|t| t.id == id)
    }

    /// Does the player have to answer this one? (A flyer needs no answer of its own.)
    pub fn needs_answer(&self, ety: usize) -> bool {
        self.types.get(ety).is_some_and(|t| !t.is_flyer())
    }

    /// Time units kept free after an action over this enemy (the stomp bounce).
    pub fn reserve(&self, ety: usize) -> f32 {
        if self.types.get(ety).is_some_and(|t| t.stompable) {
            self.bounce_beats
        } else {
            0.0
        }
    }

    /// Types a stage may use, as indices.
    pub fn unlocked(&self, level: u32) -> impl Iterator<Item = usize> + '_ {
        self.types
            .iter()
            .enumerate()
            .filter(move |(_, t)| t.unlock_level <= level)
            .map(|(i, _)| i)
    }

    /// Movement pattern: how far (metres) the enemy stands from its meeting
    /// spot at step `step_now`, when the hero meets it on step `meet_step`.
    /// Positive = still ahead; None = it has not appeared yet. The kit maps its
    /// clock to a step number (for a beat grid: `floor(t / beat * steps_per_beat)`).
    pub fn walk_dx(&self, ety: usize, step_now: i64, meet_step: i64) -> Option<f32> {
        let Some(et) = self.types.get(ety) else {
            return Some(0.0);
        };
        if et.step_m <= 0.0 {
            return Some(0.0);
        }
        if step_now < meet_step - et.walk_steps as i64 {
            None
        } else {
            Some(et.step_m * (meet_step - step_now) as f32)
        }
    }

    /// Movement with jumps: `(dx, lift)` at the fractional step position
    /// `step_pos` (the kit's clock in steps, not floored), or None if it has not
    /// appeared yet. `dx` is as in [`walk_dx`](Self::walk_dx) but a jumper
    /// covers each step in one arc: it waits on the ground for `1 - jump_air`
    /// of the step, then flies to the next step's spot and lands on the grid.
    /// `lift` is its height above the ground (0 for everything that does not jump).
    pub fn motion(&self, ety: usize, step_pos: f32, meet_step: i64) -> Option<(f32, f32)> {
        let n = step_pos.floor();
        let dx = self.walk_dx(ety, n as i64, meet_step)?;
        let Some(et) = self.types.get(ety) else {
            return Some((dx, 0.0));
        };
        if !et.is_jumper() {
            return Some((dx, 0.0));
        }
        let f = step_pos - n;
        let air = et.jump_air.clamp(0.05, 1.0);
        let u = ((f - (1.0 - air)) / air).clamp(0.0, 1.0);
        // Ease the flight (quick off the ground, slower over the top is the arc's job): linear in x.
        let dx = dx - et.step_m * u;
        Some((dx, et.jump_height * 4.0 * u * (1.0 - u)))
    }

    /// The step fraction (0..1) at which an enemy of this type starts to crouch,
    /// and the one at which it takes off: the kit's tell for "it jumps now".
    pub fn crouch_window(&self, ety: usize) -> Option<(f32, f32)> {
        let et = self.types.get(ety).filter(|t| t.is_jumper())?;
        let takeoff = 1.0 - et.jump_air.clamp(0.05, 1.0);
        Some(((takeoff - et.telegraph).max(0.0), takeoff))
    }

    /// Hit/stomp rule: what the hero touches of an enemy of type `ety`
    /// occupying `x0..x1` along the track (already shifted by its walk).
    pub fn touch(&self, ety: usize, x0: f32, x1: f32, hero: &HeroBox) -> Touch {
        self.touch_lifted(ety, x0, x1, 0.0, hero)
    }

    /// Same, for an enemy whose whole hurt band is `lift` metres above where its
    /// type puts it (a jumper in the air).
    pub fn touch_lifted(&self, ety: usize, x0: f32, x1: f32, lift: f32, hero: &HeroBox) -> Touch {
        let Some(et) = self.types.get(ety) else {
            return Touch::None;
        };
        let (y_lo, y_hi) = (et.y_lo + lift, et.y_hi + lift);
        if !(hero.x > x0 - hero.half_width && hero.x < x1 + hero.half_width) {
            return Touch::None;
        }
        if !(hero.y + self.body_h > y_lo && hero.y < y_hi) {
            return Touch::None;
        }
        if et.stompable && hero.descending && hero.y >= y_hi - self.stomp_depth {
            Touch::Stomp
        } else {
            Touch::Hit
        }
    }

    /// Ranges and charset. `who` prefixes the messages (`"hop spec"`); `looks`
    /// lists the pictures the kit can draw.
    pub fn validate(&self, who: &str, looks: &[&str]) -> Result<(), String> {
        finite_in(who, "enemies.body_h", self.body_h, 0.1, 2.0)?;
        finite_in(who, "enemies.stomp_depth", self.stomp_depth, 0.05, 1.0)?;
        finite_in(who, "enemies.bounce_height", self.bounce_height, 0.0, 3.0)?;
        finite_in(who, "enemies.bounce_beats", self.bounce_beats, 0.0, 4.0)?;
        finite_in(who, "enemies.rest_beats", self.rest_beats, 0.0, 4.0)?;
        if self.types.len() > 8 {
            return Err(format!("{who}: enemies.types holds at most 8 kinds"));
        }
        for (i, t) in self.types.iter().enumerate() {
            let n = format!("enemies.types[{i}]");
            if t.id.is_empty()
                || t.id.len() > 16
                || !t
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err(format!("{who}: {n}.id is 1..=16 of a-z 0-9 _"));
            }
            if self.types[..i].iter().any(|o| o.id == t.id) {
                return Err(format!("{who}: {n}.id {:?} is used twice", t.id));
            }
            if t.unlock_level > 20 {
                return Err(format!("{who}: {n}.unlock_level must be 0..=20"));
            }
            finite_in(who, &format!("{n}.y_lo"), t.y_lo, 0.0, 4.0)?;
            finite_in(who, &format!("{n}.y_hi"), t.y_hi, 0.05, 4.0)?;
            if t.y_hi <= t.y_lo {
                return Err(format!("{who}: {n}.y_hi must be above y_lo"));
            }
            if t.stompable && t.y_lo > 0.0 {
                return Err(format!(
                    "{who}: {n}: only an enemy on the ground can be stomped"
                ));
            }
            finite_in(who, &format!("{n}.step_m"), t.step_m, 0.0, 4.0)?;
            if !(1..=4).contains(&t.steps_per_beat) || t.walk_steps > 16 {
                return Err(format!(
                    "{who}: {n}.steps_per_beat 1..=4, walk_steps 0..=16"
                ));
            }
            if t.step_m * t.walk_steps as f32 > MAX_WALK_M {
                return Err(format!(
                    "{who}: {n}: step_m * walk_steps must be at most {MAX_WALK_M} m"
                ));
            }
            finite_in(who, &format!("{n}.jump_height"), t.jump_height, 0.0, 2.0)?;
            finite_in(who, &format!("{n}.jump_air"), t.jump_air, 0.2, 1.0)?;
            finite_in(who, &format!("{n}.telegraph"), t.telegraph, 0.0, 1.0)?;
            if t.is_jumper() {
                if t.y_lo > 0.0 {
                    return Err(format!(
                        "{who}: {n}: a jumper starts on the ground (y_lo 0)"
                    ));
                }
                if t.step_m <= 0.0 {
                    return Err(format!("{who}: {n}: a jumper needs step_m above 0"));
                }
                if t.telegraph + t.jump_air > 1.0 + 1e-4 {
                    return Err(format!(
                        "{who}: {n}: telegraph + jump_air must be at most 1"
                    ));
                }
            }
            if !looks.contains(&t.look.as_str()) {
                return Err(format!("{who}: {n}.look is {}", looks.join("|")));
            }
            if t.color.iter().any(|c| *c > 255) {
                return Err(format!("{who}: {n}.color is 0..=255 per channel"));
            }
        }
        Ok(())
    }
}

/// Weighted pick: `weights[i]` is the chance share of entry `i` (0 counts as 1),
/// `roll` any random number. Returns an index into `weights`.
pub fn weighted_pick(weights: &[u32], roll: u32) -> usize {
    let total: u32 = weights.iter().map(|w| (*w).max(1)).sum();
    let mut roll = roll % total.max(1);
    for (i, w) in weights.iter().enumerate() {
        let w = (*w).max(1);
        if roll < w {
            return i;
        }
        roll -= w;
    }
    0
}

/// One placed obstacle of a pattern, in the kit's time unit.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    /// Index in the kit's item list (for messages).
    pub item: usize,
    /// Enemy type, or None for the kit's other obstacles (they get no rest).
    pub enemy: Option<usize>,
    /// When the player's action starts.
    pub at: f32,
    /// How long that action takes (the air time of a hop).
    pub len: f32,
}

/// Rules that need no simulation, for one pattern (`slots` in time order):
/// unknown enemy, a type used before its unlock level, `rest_beats` kept around
/// an enemy, the stomp bounce reserve, and nothing running past `end`.
pub fn spacing_problems(
    spec: &EnemySpec,
    label: &str,
    level: u32,
    end: f32,
    slots: &[Slot],
) -> Vec<String> {
    let mut out = Vec::new();
    let mut free_from = 0.0_f32;
    let mut prev_enemy = false;
    for s in slots {
        let w = format!("{label} item {}", s.item);
        if let Some(i) = s.enemy {
            let Some(t) = spec.types.get(i) else {
                out.push(format!("{w}: unknown enemy"));
                continue;
            };
            if level < t.unlock_level {
                out.push(format!(
                    "{w}: {} is unlocked at level {} but the pattern is level {level}",
                    t.id, t.unlock_level
                ));
            }
            if t.is_flyer() {
                // Answered by staying clear: only the end applies.
                if s.at + s.len > end + 0.001 {
                    out.push(format!("{w}: runs past the pattern end"));
                }
                continue;
            }
        }
        let rest = if s.enemy.is_some() || prev_enemy {
            spec.rest_beats
        } else {
            0.0
        };
        if s.at + 0.001 < free_from + rest {
            out.push(format!(
                "{w}: starts before the last action ends{}",
                if rest > 0.0 { " (plus rest_beats)" } else { "" }
            ));
        }
        free_from = s.at + s.len + s.enemy.map_or(0.0, |i| spec.reserve(i));
        prev_enemy = s.enemy.is_some();
        if free_from > end + 0.001 {
            out.push(format!("{w}: ends after the pattern ends"));
        }
    }
    out
}

/// A kit plays its pattern with the inputs it allows.
pub trait Clearable {
    /// Does the whole pattern get through with every action taken `offset`
    /// seconds early (negative) or late of its ideal moment?
    fn survives(&self, offset: f32) -> bool;
}

/// The fairness hook: sweeps `survives` over `-half..=half` in `step` seconds
/// and returns the first offset that takes damage, or None if all clear.
pub fn sweep(pattern: &dyn Clearable, half: f32, step: f32) -> Option<f32> {
    let n = (half / step).ceil() as i32;
    for k in -n..=n {
        let off = (k as f32 * step).clamp(-half, half);
        if !pattern.survives(off) {
            return Some(off);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hero(x: f32, y: f32, descending: bool) -> HeroBox {
        HeroBox {
            x,
            y,
            half_width: 0.3,
            descending,
        }
    }

    #[test]
    fn default_roster_is_valid_and_staged() {
        let s = EnemySpec::default();
        s.validate("t", &["slime", "beetle", "bat", "frog"])
            .unwrap();
        let levels: Vec<u32> = s.types.iter().map(|t| t.unlock_level).collect();
        assert_eq!(levels, [0, 1, 2, 3]);
        assert_eq!(s.unlocked(0).collect::<Vec<_>>(), [0]);
        assert_eq!(s.unlocked(5).count(), 4);
        assert!(s.needs_answer(0) && !s.needs_answer(2));
        assert_eq!(s.reserve(0), s.bounce_beats);
        assert_eq!(s.reserve(2), 0.0);
    }

    #[test]
    fn touch_tells_stomp_from_hit_from_miss() {
        let s = EnemySpec::default();
        assert_eq!(s.touch(0, 5.0, 5.6, &hero(2.0, 0.0, false)), Touch::None);
        assert_eq!(s.touch(0, 5.0, 5.6, &hero(5.2, 0.0, false)), Touch::Hit);
        assert_eq!(s.touch(0, 5.0, 5.6, &hero(5.2, 0.3, true)), Touch::Stomp);
        assert_eq!(s.touch(0, 5.0, 5.6, &hero(5.2, 0.3, false)), Touch::Hit);
        assert_eq!(s.touch(0, 5.0, 5.6, &hero(5.2, 0.9, true)), Touch::None);
        // a bat: under it is safe, in its band is a hit, never a stomp
        assert_eq!(s.touch(2, 5.0, 5.6, &hero(5.2, 0.5, true)), Touch::None);
        assert_eq!(s.touch(2, 5.0, 5.6, &hero(5.2, 1.5, true)), Touch::Hit);
    }

    #[test]
    fn a_jumper_rests_then_leaps_and_lands_on_the_step_grid() {
        let s = EnemySpec::default();
        let h = 3; // the hopper
        let et = &s.types[h];
        assert!(et.is_jumper() && !s.types[1].is_jumper());
        // not on screen before its walk starts
        assert_eq!(s.motion(h, 3.0, 10), None);
        // on a step boundary it is on the ground, a whole step_m closer each step
        let (d0, l0) = s.motion(h, 7.0, 10).unwrap();
        let (d1, l1) = s.motion(h, 8.0, 10).unwrap();
        assert_eq!((l0, l1), (0.0, 0.0));
        assert!((d0 - d1 - et.step_m).abs() < 1e-4, "{d0} {d1}");
        // it sits through the first half of the step, in the air mid-way, apex at the middle
        let rest = 1.0 - et.jump_air;
        assert_eq!(s.motion(h, 7.0 + rest * 0.5, 10).unwrap(), (d0, 0.0));
        let mid = s.motion(h, 7.0 + rest + et.jump_air * 0.5, 10).unwrap();
        assert!((mid.1 - et.jump_height).abs() < 1e-4);
        assert!((mid.0 - (d0 - et.step_m * 0.5)).abs() < 1e-4);
        // a beetle and a slime never leave the ground, and the beetle still steps in place
        for ety in [0, 1] {
            let (dx, lift) = s.motion(ety, 9.4, 10).unwrap();
            assert_eq!(lift, 0.0);
            assert_eq!(Some(dx), s.walk_dx(ety, 9, 10));
        }
        // the speed contrast is data: metres per beat differ by half
        assert_eq!(et.speed_m(), 3.0);
        assert_eq!(s.types[1].speed_m(), 2.0);
        // the tell comes before takeoff and inside the resting part of the step
        let (from, off) = s.crouch_window(h).unwrap();
        assert!(from < off && (off - (1.0 - et.jump_air)).abs() < 1e-5);
        assert!(s.crouch_window(1).is_none());
    }

    #[test]
    fn a_jumper_in_the_air_is_safe_to_pass_under_and_hurts_where_it_is() {
        let s = EnemySpec::default();
        let h = 3;
        // on the ground: same as any walker
        assert_eq!(
            s.touch_lifted(h, 5.0, 5.2, 0.0, &hero(5.1, 0.0, false)),
            Touch::Hit
        );
        // its band moved up: a hero on the ground passes under, one in the band is hit
        assert_eq!(
            s.touch_lifted(h, 5.0, 5.2, 0.8, &hero(5.1, 0.0, false)),
            Touch::None
        );
        assert_eq!(
            s.touch_lifted(h, 5.0, 5.2, 0.8, &hero(5.1, 0.9, false)),
            Touch::Hit
        );
        // stomping it from above works at its lifted height too
        assert_eq!(
            s.touch_lifted(h, 5.0, 5.2, 0.8, &hero(5.1, 1.1, true)),
            Touch::Stomp
        );
        // no lift is exactly `touch`
        let b = hero(5.1, 0.3, true);
        assert_eq!(
            s.touch(h, 5.0, 5.2, &b),
            s.touch_lifted(h, 5.0, 5.2, 0.0, &b)
        );
    }

    #[test]
    fn bad_jumpers_are_refused() {
        let looks = ["slime", "frog"];
        let ok = |f: &dyn Fn(&mut EnemyType)| {
            let mut s = EnemySpec::default();
            s.types = vec![s.types[0].clone(), s.types[3].clone()];
            f(&mut s.types[1]);
            s.validate("t", &looks)
        };
        assert!(ok(&|_| {}).is_ok());
        assert!(ok(&|t| t.step_m = 0.0).is_err(), "a jumper must advance");
        assert!(
            ok(&|t| t.y_lo = 0.5).is_err(),
            "a jumper starts on the ground"
        );
        assert!(
            ok(&|t| t.telegraph = 0.8).is_err(),
            "the tell has to fit before takeoff"
        );
        assert!(ok(&|t| t.jump_height = 3.0).is_err());
        assert!(ok(&|t| t.jump_air = 0.05).is_err());
    }

    #[test]
    fn walkers_step_on_their_own_clock() {
        let s = EnemySpec::default();
        assert_eq!(s.walk_dx(0, -50, 10), Some(0.0));
        assert_eq!(s.walk_dx(1, 10, 10), Some(0.0));
        assert_eq!(s.walk_dx(1, 8, 10), Some(2.0));
        assert_eq!(s.walk_dx(1, 2, 10), Some(8.0));
        assert_eq!(s.walk_dx(1, 1, 10), None);
    }

    #[test]
    fn weighted_pick_follows_the_weights() {
        let w = [1, 3, 0];
        let mut n = [0; 3];
        for r in 0..500 {
            n[weighted_pick(&w, r)] += 1;
        }
        assert!(n[1] > n[0] * 2, "{n:?}");
        assert!(n[2] > 0, "weight 0 counts as 1");
        assert_eq!(weighted_pick(&[], 7), 0);
    }

    #[test]
    fn spacing_rules_are_kit_neutral() {
        let s = EnemySpec::default();
        let slot = |item, enemy, at, len| Slot {
            item,
            enemy,
            at,
            len,
        };
        let ok = [slot(0, Some(0), 1.0, 1.0), slot(1, Some(0), 3.0, 1.0)];
        assert!(spacing_problems(&s, "p", 0, 8.0, &ok).is_empty());
        let tight = [slot(0, Some(0), 1.0, 1.0), slot(1, Some(0), 2.0, 1.0)];
        assert_eq!(spacing_problems(&s, "p", 0, 8.0, &tight).len(), 1);
        let locked = [slot(0, Some(1), 1.0, 1.0)];
        assert_eq!(spacing_problems(&s, "p", 0, 8.0, &locked).len(), 1);
        assert_eq!(spacing_problems(&s, "p", 1, 8.0, &locked).len(), 0);
        let unknown = [slot(0, Some(9), 1.0, 1.0)];
        assert_eq!(spacing_problems(&s, "p", 0, 8.0, &unknown).len(), 1);
        let past_end = [slot(0, Some(2), 7.5, 1.0)];
        assert_eq!(spacing_problems(&s, "p", 2, 8.0, &past_end).len(), 1);
        // a plain obstacle after a stompable enemy still waits for the bounce
        let after = [slot(0, Some(0), 1.0, 1.0), slot(1, None, 2.8, 1.0)];
        assert_eq!(spacing_problems(&s, "p", 0, 8.0, &after).len(), 1);
        let later = [slot(0, Some(0), 1.0, 1.0), slot(1, None, 3.5, 1.0)];
        assert!(spacing_problems(&s, "p", 0, 8.0, &later).is_empty());
    }

    struct Window(f32);
    impl Clearable for Window {
        fn survives(&self, offset: f32) -> bool {
            offset.abs() <= self.0
        }
    }

    #[test]
    fn sweep_finds_the_first_failing_offset() {
        assert_eq!(sweep(&Window(0.1), 0.07, 0.005), None);
        let bad = sweep(&Window(0.04), 0.07, 0.005).unwrap();
        assert!(bad < 0.0 && bad.abs() > 0.04);
    }

    #[test]
    fn bad_specs_are_refused() {
        let looks = ["slime", "beetle", "bat", "frog"];
        let mut s = EnemySpec::default();
        s.types[0].look = "dragon".into();
        assert!(s.validate("t", &looks).is_err());
        let mut s = EnemySpec::default();
        s.types[1].id = "bat".into();
        assert!(s.validate("t", &looks).is_err());
        let mut s = EnemySpec::default();
        s.types[2].stompable = true;
        assert!(s.validate("t", &looks).is_err());
    }
}
