//! Deterministic measurement harness for the runner kit.
//!
//! [`measure`] plays a world headless and returns one JSON report:
//! `feel` (input latency, jump arc, lane switch), `logic` (collision,
//! death, retry), `bot` (seeded autoplay), `course` (placement audit),
//! and `perf` (wall-clock, not deterministic). Compare two reports to
//! see what a preset or tuning change did. Everything except `perf` is
//! a pure function of the world and the options.

use super::{obstacle_anchor, obstacle_parts, obstacle_shape, NeonCat, CAT_HALF_W};
use crate::flow::FlowPhase;
use crate::game_core::{collide, Hitbox};
use crate::game_spec::{Behavior, PartRole, SectionKind};
use crate::gesture::Swipe;
use crate::input::{PointerEvent, PointerPhase};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Instant;

/// Fixed sim frame for every measurement.
pub const DT: f32 = 1.0 / 60.0;

#[derive(Clone, Debug)]
pub struct MeasureOpts {
    /// Seeded bot runs.
    pub runs: u32,
    /// A bot run that lives this long counts as a clear.
    pub clear_secs: f32,
    /// Course audit horizon in seconds of play.
    pub course_secs: f32,
    /// Difficulty window for spike detection.
    pub window_secs: f32,
    /// An empty stretch at least this long counts as a rest.
    pub rest_gap_secs: f32,
    /// Longest play without a rest before `missing_rest` is raised.
    pub max_no_rest_secs: f32,
    /// Slack a back-to-back jump/roll/ride pair should leave a human, in
    /// seconds. Under it the pair is flagged `combo_tight`; under zero it is
    /// `combo_infeasible`.
    pub combo_margin_secs: f32,
    /// Shortest time an obstacle is readable before the cat reaches it.
    pub min_reaction_secs: f32,
    /// Skip the wall-clock section (tests).
    pub perf: bool,
}

impl Default for MeasureOpts {
    fn default() -> Self {
        Self {
            runs: 20,
            clear_secs: 90.0,
            course_secs: 120.0,
            window_secs: 10.0,
            rest_gap_secs: 3.0,
            max_no_rest_secs: 45.0,
            combo_margin_secs: 0.25,
            min_reaction_secs: 0.8,
            perf: true,
        }
    }
}

fn fresh(world: &str) -> Result<NeonCat, String> {
    let mut g = NeonCat::from_json(world)?;
    g.tick(DT);
    g.confirm();
    g.tick(DT);
    Ok(g)
}

fn r3(v: f32) -> f64 {
    ((v as f64) * 1000.0).round() / 1000.0
}

pub fn measure(world: &str, opts: &MeasureOpts) -> Result<Value, String> {
    let feel = feel(world)?;
    let apex = feel["jump"]["apex_m"].as_f64().unwrap_or(0.0) as f32;
    let switch_s = feel["lane_switch"]["secs"].as_f64().unwrap_or(1.0) as f32;
    let reach = reachability(world)?;
    let mut out = json!({
        "version": 1,
        "game": NeonCat::from_json(world)?.spec().id.clone(),
        "dt": r3(DT),
        "feel": feel,
        "touch": touch(world)?,
        "logic": logic(world)?,
        "bot": bot(world, opts)?,
        "course": course(world, opts, apex, switch_s, &reach)?,
        "reachability": reach,
        "view": view(world, opts)?,
    });
    if opts.perf {
        out["perf"] = perf(world)?;
    }
    Ok(out)
}

/// Frames are sim ticks at [`DT`]. A latency of 1 means the press moves
/// the cat on the same tick it lands.
fn feel(world: &str) -> Result<Value, String> {
    // Jump.
    let mut g = fresh(world)?;
    for _ in 0..5 {
        g.tick(DT);
    }
    g.set_steer(0.0, 0.0, true);
    let (mut latency, mut apex, mut apex_f, mut land_f) = (None, 0.0f32, 0u32, None);
    for f in 1..=600u32 {
        g.tick(DT);
        if f == 2 {
            g.set_steer(0.0, 0.0, false);
        }
        if latency.is_none() && g.y > 0.0 {
            latency = Some(f);
        }
        if g.y > apex {
            apex = g.y;
            apex_f = f;
        }
        if latency.is_some() && g.y <= 0.0 {
            land_f = Some(f);
            break;
        }
    }
    let lat = latency.unwrap_or(0);
    let land = land_f.unwrap_or(0);
    let s = g.spec().runner.jump.clone();
    // Steer.
    let mut g = fresh(world)?;
    let x0 = g.x;
    let width = g.spec().runner.lanes.width;
    g.set_steer(1.0, 0.0, false);
    let (mut s_lat, mut sw) = (None, None);
    for f in 1..=600u32 {
        g.tick(DT);
        let d = (g.x - x0).abs();
        if s_lat.is_none() && d > 0.01 {
            s_lat = Some(f);
        }
        if d >= width {
            sw = Some(f);
            break;
        }
    }
    let sw = sw.unwrap_or(0);
    // Swipes, as the session delivers them after recognition.
    let mut g = fresh(world)?;
    for _ in 0..5 {
        g.tick(DT);
    }
    g.swipe(Swipe::Up);
    let mut sw_jump = 0u32;
    for f in 1..=60u32 {
        g.tick(DT);
        if g.y > 0.0 {
            sw_jump = f;
            break;
        }
    }
    let mut g = fresh(world)?;
    let x1 = g.x;
    g.swipe(Swipe::Right);
    let (mut sw_lat, mut sw_lane) = (0u32, 0u32);
    for f in 1..=600u32 {
        g.tick(DT);
        let d = (g.x - x1).abs();
        if sw_lat == 0 && d > 0.01 {
            sw_lat = f;
        }
        if d >= width - 0.02 {
            sw_lane = f;
            break;
        }
    }
    // Swipe up while airborne, landing inside the buffer: does it rejump?
    let mut g = fresh(world)?;
    g.set_steer(0.0, 0.0, true);
    g.tick(DT);
    g.set_steer(0.0, 0.0, false);
    while g.y > 0.0 && g.vy >= 0.0 {
        g.tick(DT);
    }
    // About 5 frames (0.08 s) before touchdown.
    while g.y > -g.vy * DT * 5.0 {
        g.tick(DT);
    }
    g.swipe(Swipe::Up);
    let (mut touched, mut buffered) = (false, false);
    for _ in 0..30 {
        g.tick(DT);
        // Landing and the buffered rejump can share one frame's substeps.
        if g.vy < 0.0 || g.y <= 0.02 {
            touched = true;
        }
        if touched && g.vy > 0.0 {
            buffered = true;
            break;
        }
    }
    Ok(json!({
        "swipe": {
            "buffer_s": super::SWIPE_BUFFER_SECS,
            "jump_latency_frames": sw_jump,
            "lane_latency_frames": sw_lat,
            "lane_frames": sw_lane,
            "lane_secs": r3(sw_lane as f32 * DT),
            "jump_buffered_on_landing": buffered,
        },
        "jump": {
            "latency_frames": lat,
            "apex_m": r3(apex),
            "apex_analytic_m": r3(s.velocity * s.velocity / (-2.0 * s.gravity)),
            "time_to_apex_s": r3((apex_f.saturating_sub(lat) + 1) as f32 * DT),
            "airtime_s": r3((land.saturating_sub(lat)) as f32 * DT),
            "apex_to_land_s": r3(land.saturating_sub(apex_f) as f32 * DT),
        },
        "lane_switch": {
            "latency_frames": s_lat.unwrap_or(0),
            "width_m": r3(width),
            "frames": sw,
            "secs": r3(sw as f32 * DT),
        },
    }))
}

/// A 1080 x 2340 portrait phone; the short side is 1080 px.
const PHONE: (u32, u32) = (1080, 2340);

struct Finger<'a> {
    g: &'a mut NeonCat,
    f: u32,
}

impl Finger<'_> {
    fn ev(&mut self, phase: PointerPhase, x: f32, y: f32) {
        let ev = PointerEvent {
            id: 1,
            x,
            y,
            phase,
            pressure: 0.5,
        };
        self.g.touch(ev, PHONE.0, PHONE.1, self.f as f32 * DT);
    }
    fn tick(&mut self) {
        self.g.tick(DT);
        self.f += 1;
    }
}

/// The touch schemes end to end: pointer events in, cat motion out.
fn touch(world: &str) -> Result<Value, String> {
    let side = PHONE.0.min(PHONE.1) as f32;
    let (cx, cy) = (PHONE.0 as f32 * 0.5, PHONE.1 as f32 * 0.6);
    // Slow drag of exactly one lane of finger travel, held, then released.
    let mut g = fresh(world)?;
    let w = g.spec.runner.lanes.width;
    let lane_px = g.spec.input.drag_lane_frac * side;
    let x0 = g.x;
    let mut f = Finger { g: &mut g, f: 0 };
    f.ev(PointerPhase::Begin, cx, cy);
    for i in 1..=45 {
        f.ev(PointerPhase::Move, cx + lane_px * i as f32 / 45.0, cy);
        f.tick();
    }
    for _ in 0..30 {
        f.tick();
    }
    let held_lanes = (f.g.x - x0).abs() / w;
    let mut jumped = f.g.y > 0.0;
    f.ev(PointerPhase::End, cx + lane_px, cy);
    for _ in 0..60 {
        f.tick();
    }
    let drag_lanes = (f.g.x - x0).abs() / w;
    // Jittery resting finger: +-2 % of the short side every frame.
    let mut g = fresh(world)?;
    let x0 = g.x;
    let mut rng = Lcg(99);
    let mut f = Finger { g: &mut g, f: 0 };
    f.ev(PointerPhase::Begin, cx, cy);
    let mut wobble = 0.0f32;
    for _ in 0..120 {
        let jx = (rng.next() - 0.5) * 2.0 * 0.02 * side;
        let jy = (rng.next() - 0.5) * 2.0 * 0.02 * side;
        f.ev(PointerPhase::Move, cx + jx, cy + jy);
        f.tick();
        wobble = wobble.max((f.g.x - x0).abs() / w);
        jumped |= f.g.y > 0.0;
    }
    f.ev(PointerPhase::End, cx, cy);
    for _ in 0..60 {
        f.tick();
    }
    let jitter_lanes = (f.g.x - x0).abs() / w;
    let jitter_jumped = jumped;
    // Quick up-flick: 3 frames, 30 px each.
    let mut g = fresh(world)?;
    let mut f = Finger { g: &mut g, f: 0 };
    f.ev(PointerPhase::Begin, cx, cy);
    let mut flick_lat = 0;
    for i in 1..=30u32 {
        if i <= 3 {
            f.ev(PointerPhase::Move, cx, cy - 30.0 * i as f32);
        }
        f.tick();
        if f.g.y > 0.0 {
            flick_lat = i;
            break;
        }
    }
    f.ev(PointerPhase::End, cx, cy - 90.0);
    // A 45 degree stroke is not a flick.
    let mut g = fresh(world)?;
    let mut f = Finger { g: &mut g, f: 0 };
    f.ev(PointerPhase::Begin, cx, cy);
    let mut diag_jump = false;
    for i in 1..=8 {
        f.ev(
            PointerPhase::Move,
            cx + 25.0 * i as f32,
            cy - 25.0 * i as f32,
        );
        f.tick();
        diag_jump |= f.g.y > 0.0;
    }
    f.ev(PointerPhase::End, cx + 200.0, cy - 200.0);
    // Swipe scheme: one long stroke (3 lanes of travel) moves one lane.
    let mut doc: Value = serde_json::from_str(world).map_err(|e| e.to_string())?;
    doc["game"]["input"]["scheme"] = json!("swipe");
    let mut g = fresh(&doc.to_string())?;
    let x0 = g.x;
    let px = g.spec.input.swipe_dist * side;
    let mut f = Finger { g: &mut g, f: 0 };
    f.ev(PointerPhase::Begin, cx, cy);
    for i in 1..=12 {
        f.ev(PointerPhase::Move, cx + px * 3.0 * i as f32 / 12.0, cy);
        f.tick();
    }
    f.ev(PointerPhase::End, cx + px * 3.0, cy);
    for _ in 0..60 {
        f.tick();
    }
    let swipe_lanes = (f.g.x - x0).abs() / w;
    Ok(json!({
        "phone_px": [PHONE.0, PHONE.1],
        "drag_one_lane_held_lanes": r3(held_lanes),
        "drag_one_lane_released_lanes": r3(drag_lanes),
        "jitter_max_wobble_lanes": r3(wobble),
        "jitter_final_lanes": r3(jitter_lanes),
        "jitter_jumped": jitter_jumped,
        "flick_up_latency_frames": flick_lat,
        "diagonal_45_jumped": diag_jump,
        "swipe_scheme_long_stroke_lanes": r3(swipe_lanes),
    }))
}

fn logic(world: &str) -> Result<Value, String> {
    let mut g = fresh(world)?;
    let grace = g.spec().runner.jump.grace_secs;
    while g.time < grace + 0.1 {
        g.tick(DT);
        if !g.alive() {
            break;
        }
    }
    let survived_grace = g.alive();
    // Drop the cat just in front of the nearest drawn obstacle.
    let target = g
        .spawned
        .iter()
        .copied()
        .filter(|&i| obstacle_shape(&g.spec, i).hit_cz > g.z + 0.5)
        .filter(|&i| {
            // A plain floor-seated solid: the one kind that must kill head-on.
            matches!(
                g.spec.obstacle_kind(i).behavior,
                Behavior::JumpOver | Behavior::Solid
            )
        })
        .min();
    let (mut hit, mut kind) = (false, String::new());
    if let Some(i) = target {
        let o = obstacle_shape(&g.spec, i);
        g.x = o.hit_cx;
        g.y = 0.0;
        g.vy = 0.0;
        g.z = o.hit_cz - o.hit_hz - 0.01;
        g.set_steer(0.0, 0.0, false);
        for _ in 0..30 {
            g.tick(DT);
            if !g.alive() {
                hit = true;
                kind = g.death().to_string();
                break;
            }
        }
        if hit && kind != o.kind {
            kind = format!("{kind} (expected {})", o.kind);
        }
    }
    let mut to_result = None;
    for f in 1..=600u32 {
        g.tick(DT);
        if matches!(g.flow.phase, FlowPhase::Result) {
            to_result = Some(f);
            break;
        }
    }
    g.confirm();
    g.tick(DT);
    let retry_ok = matches!(g.flow.phase, FlowPhase::Play)
        && g.alive()
        && g.score() == 0
        && g.rings() == 0
        && g.z < 1.0;
    Ok(json!({
        "survives_grace": survived_grace,
        "collision_kills": hit,
        "death_kind": kind,
        "death_to_result_frames": to_result,
        "retry_resets": retry_ok,
        "pass": survived_grace && hit && to_result.is_some() && retry_ok,
    }))
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32) / ((1u64 << 24) as f32)
    }
}

/// `script_steer` with a seeded reaction delay, stick noise, and the
/// odd missed jump, so N seeds sample a spread of human-ish players.
fn bot(world: &str, opts: &MeasureOpts) -> Result<Value, String> {
    let mut lives = Vec::new();
    let mut deaths: BTreeMap<String, u32> = BTreeMap::new();
    let mut death_z = Vec::new();
    let (mut clears, mut score, mut rings, mut coins) = (0u32, 0u64, 0u64, 0u64);
    let max_f = (opts.clear_secs / DT).ceil() as u32;
    for seed in 0..opts.runs {
        let mut rng = Lcg(seed as u64 * 7919 + 17);
        let delay = (rng.next() * 5.0) as usize;
        let noise = 0.05 + rng.next() * 0.2;
        let miss = rng.next() * 0.04;
        let mut g = fresh(world)?;
        let mut q: VecDeque<(f32, bool, bool)> = VecDeque::new();
        let mut f = 0u32;
        while f < max_f && g.alive() {
            q.push_back(g.script_plan());
            let (lx, j, roll) = if q.len() > delay {
                q.pop_front().unwrap()
            } else {
                (0.0, false, false)
            };
            let lx = (lx + (rng.next() - 0.5) * 2.0 * noise).clamp(-1.0, 1.0);
            let miss_it = rng.next() < miss;
            let j = j && !miss_it;
            if roll && !miss_it {
                g.swipe(Swipe::Down);
            }
            g.set_steer(lx, 0.0, j);
            g.tick(DT);
            f += 1;
        }
        let secs = f as f32 * DT;
        lives.push(secs);
        score += g.score() as u64;
        rings += g.rings() as u64;
        coins += g.coins as u64;
        if g.alive() {
            clears += 1;
        } else {
            *deaths.entry(g.death().to_string()).or_default() += 1;
            death_z.push(r3(g.z));
        }
    }
    let n = opts.runs.max(1) as f32;
    let mut sorted = lives.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
    Ok(json!({
        "runs": opts.runs,
        "clear_secs": opts.clear_secs,
        "clear_rate": r3(clears as f32 / n),
        "survival_mean_s": r3(lives.iter().sum::<f32>() / n),
        "survival_median_s": r3(median),
        "collisions": opts.runs - clears,
        "death_causes": deaths,
        "death_z": death_z,
        "score_mean": r3(score as f32 / n),
        "rings_mean": r3(rings as f32 / n),
        "coins_mean": r3(coins as f32 / n),
    }))
}

/// Play time at which the cat reaches `z` on the speed curve.
fn time_at(g: &NeonCat, z: f32) -> f32 {
    let s = &g.spec.runner.speed;
    let t_cap = if s.ramp > 0.0 {
        (s.cap - s.start) / s.ramp
    } else {
        f32::INFINITY
    };
    let z_cap = s.start * t_cap + 0.5 * s.ramp * t_cap * t_cap;
    if z >= z_cap || s.ramp <= 0.0 {
        if s.ramp <= 0.0 {
            return z / s.start.max(0.01);
        }
        return t_cap + (z - z_cap) / s.cap.max(0.01);
    }
    (-s.start + (s.start * s.start + 2.0 * s.ramp * z).sqrt()) / s.ramp
}

/// Free x intervals, `(min, max)`.
type Spans = Vec<(f32, f32)>;

fn span_gap(a: &[(f32, f32)], b: &[(f32, f32)]) -> f32 {
    let mut best = f32::INFINITY;
    for &(a0, a1) in a {
        for &(b0, b1) in b {
            let d = if a1 < b0 {
                b0 - a1
            } else if b1 < a0 {
                a0 - b1
            } else {
                0.0
            };
            best = best.min(d);
        }
    }
    best
}

fn blocked_spans(parts: &[Hitbox], y: f32, h: f32) -> Spans {
    collide::blocked_spans(parts, y, h, CAT_HALF_W)
}

fn free_between(blocked: &[(f32, f32)], limit: f32) -> Spans {
    collide::free_between(blocked, limit)
}

/// What the cat can do about one obstacle, from its geometry alone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Options {
    walk: bool,
    roll: bool,
    jump: bool,
    ride: bool,
}

fn speed_at(g: &NeonCat, t: f32) -> f32 {
    let s = &g.spec.runner.speed;
    (s.start + s.ramp * t).min(s.cap)
}

/// Passability of obstacle `i` at play time `t`, with the lane spans that
/// walking uses.
fn options_for(g: &NeonCat, index: i32, parts: &[Hitbox], t: f32, apex: f32) -> (Options, Spans) {
    let r = &g.spec.runner;
    let limit = r.lanes.limit;
    let walk_spans = free_between(&blocked_spans(parts, 0.0, r.body.stand_height), limit);
    let roll_spans = free_between(&blocked_spans(parts, 0.0, r.body.roll_height), limit);
    let high = blocked_spans(parts, apex * 0.95, r.body.stand_height);
    // Rolling or jumping is judged where the obstacle puts its opening or
    // its ramp (the anchor): a free strip somewhere else is just walking.
    let (ax, _) = obstacle_anchor(&g.spec, index);
    let at = |spans: &[(f32, f32)]| spans.iter().any(|&(a, b)| a < ax && ax < b);
    let front = parts
        .iter()
        .map(|p| p.skin_min[2])
        .fold(f32::INFINITY, f32::min);
    let back = parts.iter().map(|p| p.skin_max[2]).fold(0.0, f32::max);
    // Airtime times speed, with margin, against the depth of the thing.
    let air = 2.0 * r.jump.velocity / r.jump.gravity.abs();
    let jump_len = air * speed_at(g, t) * 0.8;
    let ramp = parts.iter().find(|p| p.role == PartRole::Ramp);
    let top = parts.iter().find(|p| p.role == PartRole::Ride);
    let ride = match (ramp, top) {
        (Some(rp), Some(tp)) => {
            let ramp_end = rp.skin_min[2] + rp.ramp_run;
            rp.skin_max[1] >= tp.skin_max[1] - r.body.step_tol
                && (ramp_end - tp.skin_min[2]).abs() < 0.3
                && (rp.hit_cx - tp.hit_cx).abs() < 0.3
        }
        _ => false,
    };
    (
        Options {
            walk: !walk_spans.is_empty(),
            roll: at(&roll_spans),
            jump: !at(&high) && back - front <= jump_len,
            ride,
        },
        walk_spans,
    )
}

/// Audit placements over `course_secs`: an obstacle the cat has no way past,
/// kinds whose geometry does not match their declared behavior, back-to-back
/// walls with no time to change side, and windows whose difficulty jumps
/// well above the one before.
fn course(
    world: &str,
    opts: &MeasureOpts,
    apex: f32,
    switch_s: f32,
    reach: &Value,
) -> Result<Value, String> {
    let g = NeonCat::from_json(world)?;
    let r = &g.spec.runner;
    let steer = r.lanes.steer.max(0.01);
    let mut walls: Vec<(i32, f32, Spans)> = Vec::new();
    let mut unavoidable = Vec::new();
    let mut tight = Vec::new();
    let mut geometry = Vec::new();
    let mut score_at: Vec<(f32, f32)> = Vec::new();
    let mut count = 0u32;
    let mut i = 0i32;
    let mut kinds: BTreeSet<String> = BTreeSet::new();
    let mut behaviors: BTreeMap<String, u32> = BTreeMap::new();
    let mut seen_kind: BTreeSet<String> = BTreeSet::new();
    let mut times: Vec<f32> = vec![0.0];
    let mut combos = Combos::default();
    let mut prev_move: Option<Move> = None;
    let mut sec_diff: Vec<f32> = vec![0.0; g.spec.runner.sections.len()];
    loop {
        if !g.spec.obstacle_active(i) {
            if time_at(&g, g.spec.runner.obstacle_track.z(i)) > opts.course_secs || i > 100_000 {
                break;
            }
            i += 1;
            continue;
        }
        let parts = obstacle_parts(&g.spec, i);
        let o = &parts[0];
        let kind = g.spec.obstacle_kind(i);
        let t = time_at(&g, o.hit_cz);
        if t > opts.course_secs || i > 100_000 {
            break;
        }
        count += 1;
        times.push(t);
        kinds.insert(kind.name.clone());
        *behaviors
            .entry(kind.behavior.name().to_string())
            .or_default() += 1;
        let (opt, spans) = options_for(&g, i, &parts, t, apex);
        let depth = parts.iter().map(|p| p.skin_max[2]).fold(0.0, f32::max)
            - parts
                .iter()
                .map(|p| p.skin_min[2])
                .fold(f32::INFINITY, f32::min);
        // Declared behavior against what the geometry allows. Once per kind.
        if seen_kind.insert(kind.name.clone()) {
            let name = &kind.name;
            let bad = |why: &str| {
                json!({"kind": name, "behavior": kind.behavior.name(), "why": why,
                    "walk": opt.walk, "roll": opt.roll, "jump": opt.jump, "ride": opt.ride})
            };
            match kind.behavior {
                Behavior::RollUnder | Behavior::CatGap => {
                    if !opt.roll {
                        geometry.push(json!({"severity": "error", "code": "behavior_unmet",
                            "at": bad("a rolling cat does not fit through it")}));
                    } else {
                        if opt.walk || opt.jump {
                            geometry.push(json!({"severity": "warn", "code": "behavior_leaks",
                                "at": bad("it can be walked round or jumped, so rolling is optional")}));
                        }
                        let dist = r.body.roll_secs * speed_at(&g, 0.0);
                        if dist < depth + 0.3 {
                            geometry.push(json!({"severity": "error", "code": "roll_too_short",
                                "at": bad("a roll ends before the cat is clear"),
                                "roll_m": r3(dist), "depth_m": r3(depth)}));
                        }
                    }
                }
                Behavior::Rideable => {
                    if !opt.ride {
                        geometry.push(json!({"severity": "error", "code": "behavior_unmet",
                            "at": bad("no ramp reaches the top, so it cannot be ridden")}));
                    }
                }
                Behavior::Solid => {
                    if opt.jump || opt.roll {
                        geometry.push(json!({"severity": "warn", "code": "behavior_leaks",
                            "at": bad("a solid can be jumped or rolled through")}));
                    }
                }
                Behavior::JumpOver => {}
            }
        }
        let any = opt.walk || opt.roll || opt.jump || opt.ride;
        let wall = !(opt.roll || opt.jump || opt.ride);
        let mut d = match kind.behavior {
            Behavior::JumpOver => 1.0,
            Behavior::Solid => 2.0,
            Behavior::RollUnder => 1.2,
            Behavior::CatGap => 1.4,
            Behavior::Rideable => 0.8,
        };
        if wall && kind.behavior == Behavior::JumpOver {
            d += 1.0;
        }
        if !any {
            unavoidable.push(json!({"index": i, "kind": kind.name, "t": r3(t), "why": "blocks every lane and cannot be jumped or rolled"}));
        }
        if wall {
            if let Some((pi, pt, prev)) = walls.last() {
                let need = span_gap(prev, &spans) / steer;
                let have = t - pt;
                if need > have {
                    unavoidable.push(json!({"index": i, "after": pi, "t": r3(t), "why": "no time to change side", "need_s": r3(need), "have_s": r3(have)}));
                } else if need + switch_s * 0.5 > have {
                    tight.push(json!({"index": i, "after": pi, "t": r3(t), "need_s": r3(need), "have_s": r3(have)}));
                }
                d += need / have.max(0.05);
            }
            walls.push((i, t, spans));
        }
        let mv = intended_move(&g, kind.behavior, &opt, &parts);
        if let (Some(a), Some(b)) = (prev_move, mv) {
            combos.add(&a, &b, i);
        }
        prev_move = mv;
        if let Some((k, _)) = g.spec.section_at(o.hit_cz) {
            sec_diff[k] += d;
        }
        score_at.push((t, d));
        i += 1;
    }
    let nwin = (opts.course_secs / opts.window_secs).ceil() as usize;
    let mut win = vec![0.0f32; nwin.max(1)];
    for (t, d) in &score_at {
        let k = ((t / opts.window_secs) as usize).min(win.len() - 1);
        win[k] += d / opts.window_secs;
    }
    let mut spikes = Vec::new();
    for k in 1..win.len() {
        // A rise out of a calm window is the pacing working, not a spike.
        if win[k - 1] >= 0.35 && win[k] > win[k - 1] * 1.5 && win[k] - win[k - 1] > 0.3 {
            spikes.push(json!({"window": k, "from": r3(win[k - 1]), "to": r3(win[k])}));
        }
    }
    let mut issues = placement_issues(&g, opts, &times, &sec_diff, &unavoidable, &tight, &spikes);
    issues.extend(geometry);
    issues.extend(combos.issues(opts.combo_margin_secs));
    if let Some(list) = reach["issues"].as_array() {
        issues.extend(list.iter().cloned());
    }
    let errors = issues.iter().filter(|v| v["severity"] == "error").count();
    Ok(json!({
        "horizon_s": opts.course_secs,
        "placement": {"ok": errors == 0, "errors": errors, "issues": issues},
        "obstacles": count,
        "kinds": kinds,
        "behaviors": behaviors,
        "unavoidable": unavoidable,
        "tight": tight,
        "difficulty_per_window": win.iter().map(|v| r3(*v)).collect::<Vec<_>>(),
        "spikes": spikes,
        "combos": combos.summary(),
    }))
}

/// Where the camera puts the cat and how far ahead a 1 m obstacle reads,
/// for a portrait phone and a landscape screen, at start speed and with the
/// FOV opened for top speed.
fn view(world: &str, opts: &MeasureOpts) -> Result<Value, String> {
    let g = fresh(world)?;
    let r = &g.spec.runner;
    // The three lanes: their outer edges, not the rail beyond them.
    let (limit, v0, vcap) = (r.lanes.width * 1.5, r.speed.start, r.speed.cap);
    let cat_z = g.z();
    let first = (0..64)
        .find(|i| g.spec.obstacle_active(*i))
        .map(|i| obstacle_parts(&g.spec, i)[0].hit_cz - cat_z);
    let mut rows = Vec::new();
    let mut issues = Vec::new();
    for (screen, aspect) in [("portrait", 412.0 / 915.0), ("landscape", 16.0 / 9.0)] {
        for (state, fov, speed) in [
            ("start", g.spec.camera.fov, v0),
            ("fast", g.spec.camera.fov_fast, vcap),
        ] {
            let mut scene = g.document().compile_scene(aspect);
            scene.camera.fov_y = fov.to_radians();
            let vp = scene.camera.view_projection(aspect);
            let ndc = |p: [f32; 3]| {
                let c = vp * glam::Vec3::from_array(p).extend(1.0);
                (c.w > 0.05).then(|| c.truncate() / c.w)
            };
            let cat = ndc([0.0, 0.31, cat_z]).map(|n| (n.y + 1.0) / 2.0);
            let lane = |d: f32| {
                let a = ndc([limit, 0.0, cat_z + d]);
                let b = ndc([-limit, 0.0, cat_z + d]);
                match (a, b) {
                    (Some(a), Some(b)) => a.x.abs().max(b.x.abs()),
                    _ => f32::INFINITY,
                }
            };
            // The farthest a 1 m obstacle is still tall enough to read:
            // 4% of the frame height, and clear of the top HUD band.
            let mut readable = 0.0f32;
            let mut d = 2.0f32;
            while d < 120.0 {
                if let (Some(base), Some(top)) =
                    (ndc([0.0, 0.0, cat_z + d]), ndc([0.0, 1.0, cat_z + d]))
                {
                    if (top.y - base.y) / 2.0 >= 0.04 && top.y < 0.7 {
                        readable = d;
                    }
                }
                d += 0.25;
            }
            let reaction = readable / speed.max(0.01);
            let (lane_near, lane_far) = (lane(4.0), lane(readable.min(30.0)));
            let cat_frac = cat.unwrap_or(f32::NAN);
            rows.push(json!({
                "screen": screen, "state": state, "fov": r3(fov),
                "cat_from_bottom": r3(cat_frac),
                "lane_edge_ndc_x_near": r3(lane_near),
                "lane_edge_ndc_x_far": r3(lane_far),
                "readable_m": r3(readable),
                "reaction_s": r3(reaction),
            }));
            let at = json!({"screen": screen, "state": state});
            if screen == "portrait" && cat_frac > 0.38 {
                issues.push(json!({"severity": "warn", "code": "cat_too_high", "at": at, "cat_from_bottom": r3(cat_frac)}));
            }
            if lane_near > 1.0 || lane_far > 1.0 {
                issues.push(json!({"severity": "warn", "code": "lanes_cropped", "at": at}));
            }
            if reaction < opts.min_reaction_secs {
                issues.push(json!({"severity": "warn", "code": "short_reaction", "at": at, "reaction_s": r3(reaction)}));
            }
        }
    }
    Ok(json!({"first_obstacle_m": first.map(r3), "rows": rows, "issues": issues}))
}

/// What the cat does about an obstacle when it plays it as intended, with
/// the moments that action pins down. Walking round something is a lane
/// change, which the wall checks already cover.
#[derive(Clone, Copy, Debug)]
struct Move {
    kind: MoveKind,
    /// The latest moment the action must have started.
    need_by: f32,
    /// The earliest moment the cat is grounded and free again.
    free_at: f32,
    /// The cat could also walk round, so a bad combo is not forced.
    walkable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveKind {
    Jump,
    Roll,
    Ride,
}

impl MoveKind {
    fn name(self) -> &'static str {
        match self {
            MoveKind::Jump => "jump",
            MoveKind::Roll => "roll",
            MoveKind::Ride => "ride",
        }
    }
}

/// Times come from the real jump arc (not a fast fall) and the real roll,
/// with the distance to each obstacle turned into seconds by the speed ramp.
fn intended_move(g: &NeonCat, behavior: Behavior, opt: &Options, parts: &[Hitbox]) -> Option<Move> {
    let r = &g.spec.runner;
    let front = parts
        .iter()
        .map(|p| p.skin_min[2])
        .fold(f32::INFINITY, f32::min);
    let back = parts.iter().map(|p| p.skin_max[2]).fold(0.0, f32::max);
    let (t_front, t_back) = (time_at(g, front), time_at(g, back));
    let walkable = opt.walk;
    let v = r.jump.velocity;
    let grav = r.jump.gravity.abs().max(0.01);
    match behavior {
        Behavior::JumpOver if opt.jump => {
            let h = parts
                .iter()
                .map(|p| p.hit_top)
                .fold(0.0, f32::max)
                .min(v * v / (2.0 * grav) - 0.01);
            // Rising to the obstacle's top takes this long after takeoff.
            let rise = (v - (v * v - 2.0 * grav * h).max(0.0).sqrt()) / grav;
            let air = 2.0 * v / grav;
            let take_off = t_front - rise;
            Some(Move {
                kind: MoveKind::Jump,
                need_by: take_off,
                free_at: take_off + air,
                walkable,
            })
        }
        Behavior::RollUnder | Behavior::CatGap if opt.roll => Some(Move {
            kind: MoveKind::Roll,

            need_by: t_front,
            // A jump cancels a roll, and a roll can be restarted, so the
            // cat is free as soon as it is past the back.
            free_at: t_back,
            walkable,
        }),
        Behavior::Rideable if opt.ride => {
            let top = parts.iter().map(|p| p.skin_max[1]).fold(0.0, f32::max);
            let fall = (2.0 * top / grav).sqrt();
            Some(Move {
                kind: MoveKind::Ride,
                need_by: t_front,
                free_at: t_back + fall,
                walkable,
            })
        }
        _ => None,
    }
}

/// Back-to-back actions: the second one has to start before the first has
/// let the cat go.
#[derive(Default)]
struct Combos {
    rows: BTreeMap<String, (u32, f32, i32)>,
    pairs: u32,
}

impl Combos {
    fn add(&mut self, a: &Move, b: &Move, at: i32) {
        let margin = b.need_by - a.free_at;
        let key = format!(
            "{}>{}{}",
            a.kind.name(),
            b.kind.name(),
            if a.walkable { "~" } else { "" }
        );
        let e = self.rows.entry(key).or_insert((0, f32::INFINITY, at));
        e.0 += 1;
        if margin < e.1 {
            e.1 = margin;
            e.2 = at;
        }
        self.pairs += 1;
    }

    fn issues(&self, tight: f32) -> Vec<Value> {
        let mut out = Vec::new();
        for (k, (n, m, at)) in &self.rows {
            let walkable = k.ends_with('~');
            let row = json!({"pair": k.trim_end_matches('~'), "count": n, "min_margin_s": r3(*m), "at_obstacle": at});
            if *m < 0.0 && !walkable {
                out.push(json!({"severity": "error", "code": "combo_infeasible", "at": row}));
            } else if *m < tight {
                out.push(json!({"severity": "warn", "code": "combo_tight", "at": row}));
            }
        }
        out
    }

    fn summary(&self) -> Value {
        let rows: Vec<Value> = self
            .rows
            .iter()
            .map(|(k, (n, m, at))| json!({"pair": k.trim_end_matches('~'), "walkable_first": k.ends_with('~'), "count": n, "min_margin_s": r3(*m), "at_obstacle": at}))
            .collect();
        json!({"pairs": self.pairs, "by_pair": rows})
    }
}

/// How a probe tries to get past an obstacle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    /// Hold the lane at the anchor and do nothing.
    Stand,
    /// Steer into the widest free span, no jump.
    Walk,
    Jump,
    Roll,
}

/// Teleport a live run to just before obstacle `i`, act, and report whether
/// the cat is still alive once past it. The same sim the player gets.
fn probe(world: &str, i: i32, act: Act) -> Result<bool, String> {
    let mut g = fresh(world)?;
    let grace = g.spec().runner.jump.grace_secs;
    g.time = grace + 0.2;
    let parts = obstacle_parts(&g.spec, i);
    let front = parts
        .iter()
        .map(|p| p.skin_min[2])
        .fold(f32::INFINITY, f32::min);
    let back = parts.iter().map(|p| p.skin_max[2]).fold(0.0, f32::max);
    let (ax, _) = obstacle_anchor(&g.spec, i);
    let limit = g.spec.runner.lanes.limit;
    let free = free_between(
        &blocked_spans(&parts, 0.0, g.spec.runner.body.stand_height),
        limit,
    );
    let aim = match act {
        Act::Walk => free
            .iter()
            .max_by(|a, b| (a.1 - a.0).partial_cmp(&(b.1 - b.0)).unwrap())
            .map(|(a, b)| (a + b) * 0.5)
            .unwrap_or(ax),
        // A roll or a jump goes through the anchor (the gap, the ramp).
        _ => ax,
    };
    g.x = aim;
    g.y = 0.0;
    g.vy = 0.0;
    g.z = front - 7.0;
    g.rebuild();
    for _ in 0..(10.0 / DT) as u32 {
        if !g.alive() {
            return Ok(false);
        }
        if g.z > back + 1.5 {
            return Ok(true);
        }
        let ahead = front - g.z;
        let mut jump = false;
        if act == Act::Jump && (0.0..1.0).contains(&ahead) {
            jump = true;
        }
        if act == Act::Roll && (0.0..1.0).contains(&ahead) && g.rolling <= 0.0 {
            g.swipe(Swipe::Down);
        }
        g.set_steer(0.0, 0.0, jump);
        g.tick(DT);
    }
    Ok(g.alive())
}

/// Route reachability, on the live sim: for each obstacle kind, which of
/// stand / walk / jump / roll gets a cat past, against what its behavior
/// promises. A kind that its own behavior cannot pass is an error.
fn reachability(world: &str) -> Result<Value, String> {
    let g = NeonCat::from_json(world)?;
    let mut rows = Vec::new();
    let mut issues = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for i in 0..4000 {
        if !g.spec.obstacle_active(i) {
            continue;
        }
        let kind = g.spec.obstacle_kind(i);
        if !seen.insert(kind.name.clone()) {
            continue;
        }
        let stand = probe(world, i, Act::Stand)?;
        let walk = probe(world, i, Act::Walk)?;
        let jump = probe(world, i, Act::Jump)?;
        let roll = probe(world, i, Act::Roll)?;
        let (want, ok) = match kind.behavior {
            Behavior::JumpOver => ("jump", jump && !stand),
            Behavior::Solid => ("walk", walk && !stand && !jump),
            Behavior::RollUnder | Behavior::CatGap => ("roll", roll && !stand && !jump),
            // Standing in the lane climbs the ramp and rides the roof.
            Behavior::Rideable => ("ride", stand),
        };
        if !ok {
            issues.push(json!({"severity": "error", "code": "unreachable", "at": {
                "kind": kind.name, "behavior": kind.behavior.name(), "index": i,
                "expected": want, "stand": stand, "walk": walk, "jump": jump, "roll": roll}}));
        }
        rows.push(json!({
            "kind": kind.name, "behavior": kind.behavior.name(), "index": i,
            "expected": want, "stand": stand, "walk": walk, "jump": jump, "roll": roll,
            "ok": ok,
        }));
        if seen.len() >= g.spec.obstacles.len() {
            break;
        }
    }
    Ok(json!({"kinds": rows, "issues": issues}))
}

/// The validator proper. Severity `error` means a designer must fix it.
fn placement_issues(
    g: &NeonCat,
    opts: &MeasureOpts,
    times: &[f32],
    sec_diff: &[f32],
    unavoidable: &[Value],
    tight: &[Value],
    spikes: &[Value],
) -> Vec<Value> {
    let mut out = Vec::new();
    for u in unavoidable {
        out.push(json!({"severity": "error", "code": "unavoidable", "at": u}));
    }
    for t in tight {
        out.push(json!({"severity": "warn", "code": "tight", "at": t}));
    }
    for s in spikes {
        out.push(json!({"severity": "warn", "code": "difficulty_spike", "at": s}));
    }
    // Longest stretch with no empty gap of `rest_gap_secs`.
    let mut last_rest = 0.0f32;
    let mut longest = 0.0f32;
    let mut best_gap = 0.0f32;
    for w in times.windows(2) {
        best_gap = best_gap.max(w[1] - w[0]);
        if w[1] - w[0] >= opts.rest_gap_secs {
            longest = longest.max(w[0] - last_rest);
            last_rest = w[1];
        }
    }
    longest = longest.max(opts.course_secs - last_rest);
    if longest > opts.max_no_rest_secs {
        out.push(json!({"severity": "error", "code": "missing_rest",
            "longest_without_rest_s": r3(longest), "limit_s": opts.max_no_rest_secs,
            "longest_empty_gap_s": r3(best_gap), "needs_gap_s": opts.rest_gap_secs,
            "why": "no empty stretch long enough to breathe; add a rest section"}));
    }
    let secs = &g.spec.runner.sections;
    for (k, s) in secs.iter().enumerate() {
        let next = &secs[(k + 1) % secs.len()];
        let allowed = if matches!(s.kind, SectionKind::Rest | SectionKind::Reward) {
            3
        } else {
            1
        };
        if secs.len() > 1 && next.level as i32 - s.level as i32 > allowed {
            out.push(json!({"severity": "warn", "code": "level_jump", "from": k,
                "to": (k + 1) % secs.len(), "levels": [s.level, next.level], "allowed_rise": allowed}));
        }
        // Declared-calm sections must measure calm.
        let sum = sec_diff[k];
        if matches!(s.kind, SectionKind::Rest | SectionKind::Reward) && sum > 0.0 {
            out.push(
                json!({"severity": "error", "code": "rest_not_calm", "section": k,
                "why": "a rest/reward section has obstacles (density > 0)"}),
            );
        }
    }
    if !secs.is_empty()
        && !secs
            .iter()
            .any(|s| matches!(s.kind, SectionKind::Rest | SectionKind::Reward))
    {
        out.push(json!({"severity": "error", "code": "missing_rest_section",
            "why": "sections are set but none is a rest or reward"}));
    }
    out
}

/// Wall-clock tick cost and dump size over one scripted run. Not
/// deterministic. Draw batches estimate one instanced draw per distinct
/// mesh, which is how the renderer groups props.
fn perf(world: &str) -> Result<Value, String> {
    let mut g = fresh(world)?;
    let mut ms = Vec::new();
    let mut props = Vec::new();
    let mut batches = Vec::new();
    for _ in 0..(60.0 / DT) as u32 {
        if !g.alive() {
            break;
        }
        let (lx, j, roll) = g.script_plan();
        if roll {
            g.swipe(Swipe::Down);
        }
        g.set_steer(lx, 0.0, j);
        let t0 = Instant::now();
        g.tick(DT);
        ms.push(t0.elapsed().as_secs_f32() * 1000.0);
        let d = g.document();
        let on: Vec<_> = d.props.iter().filter(|p| p.enabled).collect();
        props.push(on.len());
        let meshes: BTreeSet<&str> = on
            .iter()
            .map(|p| p.gltf.as_deref().unwrap_or(p.model.as_str()))
            .collect();
        batches.push(meshes.len());
    }
    let pct = |v: &mut Vec<f32>, q: f32| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.get(((v.len() as f32 - 1.0) * q).round() as usize)
            .copied()
            .unwrap_or(0.0)
    };
    let mut p: Vec<f32> = props.iter().map(|&v| v as f32).collect();
    let mut b: Vec<f32> = batches.iter().map(|&v| v as f32).collect();
    Ok(json!({
        "note": "wall clock; sim tick + dump rebuild only, no GPU",
        "frames": ms.len(),
        "tick_ms_p50": r3(pct(&mut ms.clone(), 0.5)),
        "tick_ms_p95": r3(pct(&mut ms, 0.95)),
        "props_p95": pct(&mut p.clone(), 0.95),
        "props_max": pct(&mut p, 1.0),
        "draw_batches_est_p95": pct(&mut b.clone(), 0.95),
        "draw_batches_est_max": pct(&mut b, 1.0),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORLD: &str = include_str!("../tests/fixtures/neon_cat_world.json");

    fn opts() -> MeasureOpts {
        MeasureOpts {
            runs: 4,
            clear_secs: 30.0,
            perf: false,
            ..Default::default()
        }
    }

    #[test]
    fn report_is_deterministic() {
        let a = measure(WORLD, &opts()).unwrap();
        let b = measure(WORLD, &opts()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn neon_cat_feel_and_logic() {
        let r = measure(WORLD, &opts()).unwrap();
        assert_eq!(r["feel"]["jump"]["latency_frames"], 1);
        let apex = r["feel"]["jump"]["apex_m"].as_f64().unwrap();
        let want = r["feel"]["jump"]["apex_analytic_m"].as_f64().unwrap();
        assert!((apex - want).abs() < 0.05, "{apex} vs {want}");
        assert!(r["feel"]["lane_switch"]["frames"].as_u64().unwrap() > 0);
        let sw = &r["feel"]["swipe"];
        assert_eq!(sw["jump_latency_frames"], 1, "{sw}");
        assert!(sw["lane_latency_frames"].as_u64().unwrap() <= 2, "{sw}");
        assert!(sw["lane_frames"].as_u64().unwrap() > 0, "{sw}");
        assert_eq!(sw["jump_buffered_on_landing"], true, "{sw}");
        let t = &r["touch"];
        let lanes = |k: &str| t[k].as_f64().unwrap();
        assert!((lanes("drag_one_lane_held_lanes") - 1.0).abs() < 0.1, "{t}");
        assert!(
            (lanes("drag_one_lane_released_lanes") - 1.0).abs() < 0.05,
            "{t}"
        );
        assert!(lanes("jitter_max_wobble_lanes") < 0.25, "{t}");
        assert!(lanes("jitter_final_lanes") < 0.05, "{t}");
        assert_eq!(t["jitter_jumped"], false, "{t}");
        assert_eq!(t["diagonal_45_jumped"], false, "{t}");
        assert!(t["flick_up_latency_frames"].as_u64().unwrap() <= 4, "{t}");
        assert!(
            (lanes("swipe_scheme_long_stroke_lanes") - 1.0).abs() < 0.05,
            "{t}"
        );
        assert_eq!(r["logic"]["pass"], true, "{}", r["logic"]);
        assert_eq!(
            r["course"]["unavoidable"].as_array().unwrap().len(),
            0,
            "{}",
            r["course"]
        );
    }

    // The default course is sectioned; the classic one is three crates, no pacing.
    const CLASSIC: &str = include_str!("../tests/fixtures/neon_cat_classic_world.json");
    const SECTIONED: &str = WORLD;

    fn issues(world: &str) -> Vec<Value> {
        let o = MeasureOpts {
            perf: false,
            runs: 2,
            ..Default::default()
        };
        let r = measure(world, &o).unwrap();
        r["course"]["placement"]["issues"]
            .as_array()
            .unwrap()
            .clone()
    }

    fn codes(v: &[Value], sev: &str) -> Vec<String> {
        v.iter()
            .filter(|i| i["severity"] == sev)
            .map(|i| i["code"].as_str().unwrap().to_string())
            .collect()
    }

    fn tight_world(gap: f64, keep: &[&str]) -> String {
        let mut w: Value = serde_json::from_str(WORLD).unwrap();
        w["game"]["runner"]["sections"] = json!([]);
        w["game"]["runner"]["obstacle_track"]["gap"] = json!(gap);
        let list: Vec<Value> = w["game"]["obstacles"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|k| keep.contains(&k["name"].as_str().unwrap()))
            .cloned()
            .collect();
        w["game"]["obstacles"] = Value::Array(list);
        w.to_string()
    }

    #[test]
    fn back_to_back_actions_are_checked_against_the_real_airtime_and_roll() {
        // A roll-under and a train a few metres apart: the cat cannot be
        // through one and set for the other.
        let errs = codes(&issues(&tight_world(5.0, &["OVERPASS", "TRAIN"])), "error");
        assert!(errs.contains(&"combo_infeasible".to_string()), "{errs:?}");
        // The shipped course leaves room for every pair it contains.
        let errs = codes(&issues(WORLD), "error");
        assert!(!errs.contains(&"combo_infeasible".to_string()), "{errs:?}");
        // And a wide gap clears the same pair.
        let wide = codes(&issues(&tight_world(30.0, &["OVERPASS", "TRAIN"])), "error");
        assert!(!wide.contains(&"combo_infeasible".to_string()), "{wide:?}");
    }

    #[test]
    fn combo_margins_follow_the_speed_ramp() {
        let a = measure(WORLD, &opts()).unwrap();
        let rows = a["course"]["combos"]["by_pair"].as_array().unwrap();
        assert!(rows
            .iter()
            .any(|r| r["pair"] == "jump>roll" || r["pair"] == "roll>jump"));
        // Jump then jump at the speed cap leaves the least slack.
        let jj = rows.iter().find(|r| r["pair"] == "jump>jump").unwrap();
        assert!(jj["min_margin_s"].as_f64().unwrap() > 0.0, "{jj}");
    }

    #[test]
    fn camera_view_keeps_the_cat_low_and_the_lanes_in_frame() {
        let r = measure(WORLD, &opts()).unwrap();
        let rows = r["view"]["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        for row in rows {
            let low = row["cat_from_bottom"].as_f64().unwrap();
            assert!(low > 0.1 && low < 0.3, "cat in the lower quarter: {row}");
            assert!(
                row["lane_edge_ndc_x_near"].as_f64().unwrap() <= 1.0,
                "{row}"
            );
            assert!(row["reaction_s"].as_f64().unwrap() >= 0.8, "{row}");
        }
        assert!(
            r["view"]["issues"].as_array().unwrap().is_empty(),
            "{}",
            r["view"]
        );
    }

    fn with_sections(sections: Value) -> String {
        let mut w: Value = serde_json::from_str(SECTIONED).unwrap();
        w["game"]["runner"]["sections"] = sections;
        w.to_string()
    }

    #[test]
    fn uniform_course_is_flagged_for_no_rest_and_sections_fix_it() {
        assert_eq!(codes(&issues(CLASSIC), "error"), ["missing_rest"]);
        let errs = codes(&issues(SECTIONED), "error");
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn validator_flags_sections_without_rest_and_sudden_jumps() {
        let no_rest = with_sections(json!([
            {"kind":"learn","length":200,"level":2},
            {"kind":"challenge","length":300,"level":3}]));
        let errs = codes(&issues(&no_rest), "error");
        assert!(
            errs.contains(&"missing_rest_section".to_string()),
            "{errs:?}"
        );
        let jump = with_sections(json!([
            {"kind":"intro","length":150,"level":1},
            {"kind":"challenge","length":300,"level":5},
            {"kind":"rest","length":200,"level":0}]));
        let warns = codes(&issues(&jump), "warn");
        assert!(warns.contains(&"level_jump".to_string()), "{warns:?}");
        // A rest that still has obstacles is not a rest.
        let fake = with_sections(json!([
            {"kind":"rest","length":400,"level":0,"density":0.5}]));
        let errs = codes(&issues(&fake), "error");
        assert!(errs.contains(&"rest_not_calm".to_string()), "{errs:?}");
    }

    #[test]
    fn validator_flags_unavoidable_placements() {
        // A crate wider than the rails blocks every lane and cannot be jumped.
        let mut w: Value = serde_json::from_str(WORLD).unwrap();
        w["game"]["obstacles"][0]["scale"] = json!([9.0, 3.0, 1.0]);
        let errs = codes(&issues(&w.to_string()), "error");
        assert!(errs.contains(&"unavoidable".to_string()), "{errs:?}");
    }

    #[test]
    fn rest_sections_spawn_nothing_and_the_bot_does_better() {
        let g = NeonCat::from_json(SECTIONED).unwrap();
        let spec = g.spec();
        let (mut kept, mut empty_rest) = (0, 0);
        for i in 0..200 {
            let z = spec.runner.obstacle_track.z(i);
            let (_, s) = spec.section_at(z).unwrap();
            if spec.obstacle_active(i) {
                kept += 1;
                assert!(s.density() > 0.0, "obstacle {i} inside a {:?}", s.kind);
            } else if s.density() == 0.0 {
                empty_rest += 1;
            }
        }
        assert!(kept > 40 && empty_rest > 10, "{kept} {empty_rest}");
        // Deterministic.
        let again = NeonCat::from_json(SECTIONED).unwrap();
        assert!((0..200).all(|i| g.spec().obstacle_active(i) == again.spec().obstacle_active(i)));
        let o = MeasureOpts {
            perf: false,
            runs: 8,
            ..Default::default()
        };
        let uni = bot(CLASSIC, &o).unwrap();
        let sec = bot(SECTIONED, &o).unwrap();
        assert!(
            sec["survival_mean_s"].as_f64() >= uni["survival_mean_s"].as_f64(),
            "{uni} vs {sec}"
        );
    }

    #[test]
    fn walls_without_side_time_are_flagged() {
        assert!(span_gap(&[(-2.0, -1.0)], &[(1.0, 2.0)]) > 1.9);
        assert_eq!(span_gap(&[(-2.0, 0.5)], &[(0.0, 2.0)]), 0.0);
        assert!(free_between(&[(-3.0, 3.0)], 2.0).is_empty());
    }
}
