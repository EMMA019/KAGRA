//! Deterministic measurement harness for the runner kit.
//!
//! [`measure`] plays a world headless and returns one JSON report:
//! `feel` (input latency, jump arc, lane switch), `logic` (collision,
//! death, retry), `bot` (seeded autoplay), `course` (placement audit),
//! and `perf` (wall-clock, not deterministic). Compare two reports to
//! see what a preset or tuning change did. Everything except `perf` is
//! a pure function of the world and the options.

use super::{obstacle_shape, NeonCat};
use crate::flow::FlowPhase;
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
    let mut out = json!({
        "version": 1,
        "game": NeonCat::from_json(world)?.spec().id.clone(),
        "dt": r3(DT),
        "feel": feel,
        "touch": touch(world)?,
        "logic": logic(world)?,
        "bot": bot(world, opts)?,
        "course": course(world, opts, apex, switch_s)?,
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
    let (mut clears, mut score, mut rings) = (0u32, 0u64, 0u64);
    let max_f = (opts.clear_secs / DT).ceil() as u32;
    for seed in 0..opts.runs {
        let mut rng = Lcg(seed as u64 * 7919 + 17);
        let delay = (rng.next() * 5.0) as usize;
        let noise = 0.05 + rng.next() * 0.2;
        let miss = rng.next() * 0.04;
        let mut g = fresh(world)?;
        let mut q: VecDeque<(f32, bool)> = VecDeque::new();
        let mut f = 0u32;
        while f < max_f && g.alive() {
            q.push_back(g.script_steer());
            let (lx, j) = if q.len() > delay {
                q.pop_front().unwrap()
            } else {
                (0.0, false)
            };
            let lx = (lx + (rng.next() - 0.5) * 2.0 * noise).clamp(-1.0, 1.0);
            let j = j && rng.next() >= miss;
            g.set_steer(lx, 0.0, j);
            g.tick(DT);
            f += 1;
        }
        let secs = f as f32 * DT;
        lives.push(secs);
        score += g.score() as u64;
        rings += g.rings() as u64;
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

/// Free x intervals beside an obstacle inside the rails.
fn free_spans(cx: f32, hx: f32, limit: f32) -> Spans {
    let mut v = Vec::new();
    if cx - hx > -limit {
        v.push((-limit, cx - hx));
    }
    if cx + hx < limit {
        v.push((cx + hx, limit));
    }
    v
}

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

/// Audit placements over `course_secs`: an obstacle the cat can neither
/// jump nor pass beside, back-to-back walls with no time to change side,
/// and windows whose difficulty jumps well above the one before.
fn course(world: &str, opts: &MeasureOpts, apex: f32, switch_s: f32) -> Result<Value, String> {
    let g = NeonCat::from_json(world)?;
    let r = &g.spec.runner;
    let limit = r.lanes.limit;
    let steer = r.lanes.steer.max(0.01);
    let mut walls: Vec<(i32, f32, Spans)> = Vec::new();
    let mut unavoidable = Vec::new();
    let mut tight = Vec::new();
    let mut score_at: Vec<(f32, f32)> = Vec::new();
    let mut count = 0u32;
    let mut i = 0i32;
    let mut kinds: BTreeSet<String> = BTreeSet::new();
    loop {
        let o = obstacle_shape(&g.spec, i);
        let t = time_at(&g, o.hit_cz);
        if t > opts.course_secs || i > 100_000 {
            break;
        }
        count += 1;
        kinds.insert(o.kind.clone());
        let jumpable = o.hit_top < apex * 0.95;
        let spans = free_spans(o.hit_cx, o.hit_hx, limit);
        let mut d = 1.0;
        if !jumpable {
            d += 1.0;
            if spans.is_empty() {
                unavoidable.push(json!({"index": i, "kind": o.kind, "t": r3(t), "why": "blocks every lane and is too tall to jump"}));
            }
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
        if win[k] > win[k - 1] * 1.5 && win[k] - win[k - 1] > 0.3 {
            spikes.push(json!({"window": k, "from": r3(win[k - 1]), "to": r3(win[k])}));
        }
    }
    Ok(json!({
        "horizon_s": opts.course_secs,
        "obstacles": count,
        "kinds": kinds,
        "unavoidable": unavoidable,
        "tight": tight,
        "difficulty_per_window": win.iter().map(|v| r3(*v)).collect::<Vec<_>>(),
        "spikes": spikes,
    }))
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
        let (lx, j) = g.script_steer();
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

    #[test]
    fn walls_without_side_time_are_flagged() {
        assert!(span_gap(&[(-2.0, -1.0)], &[(1.0, 2.0)]) > 1.9);
        assert_eq!(span_gap(&[(-2.0, 0.5)], &[(0.0, 2.0)]), 0.0);
        assert!(free_spans(0.0, 3.0, 2.0).is_empty());
    }
}
