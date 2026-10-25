//! Measure harness for HOP-BEAT: playtest bots, the impossible-placement
//! check and the difficulty curve. Pure Rust, no window: the bots drive the
//! same `HopBeat` the page runs, one 60 Hz tick at a time.
//!
//! `cargo run -p kagra-shared --example hop_measure` prints the report;
//! `docs/measure/hop_beat_baseline.json` is the committed copy and a test
//! keeps the two in step.

use crate::hop_beat::{Course, Fate, Hop, HopBeat, HopSpec, Kind, Obst};
use serde_json::{json, Value};
use std::collections::HashSet;

/// How a bot plays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bot {
    /// Presses exactly on the ideal beat.
    Perfect,
    /// Presses `sigma_ms` off on average and sometimes forgets (`miss_pct`).
    Human { sigma_ms: f32, miss_pct: f32 },
    /// Presses every ideal beat `ms` early (negative) or late, always the right hop.
    Offset { ms: f32 },
    /// Presses on the beat like `Perfect`, but keeps the finger down `ms` on a tap (a
    /// thumb on glass, not a mouse click).
    SlowTap { ms: f32 },
    /// Never touches the screen.
    Idle,
    /// Taps on every beat, whatever is coming.
    Spam,
    /// Holds the button down for the whole run.
    HoldAll,
}

impl Bot {
    pub fn name(&self) -> String {
        match self {
            Bot::Perfect => "perfect".into(),
            Bot::Human { sigma_ms, miss_pct } => format!("human_{sigma_ms:.0}ms_{miss_pct:.0}pct"),
            Bot::Offset { ms } => format!("offset_{ms:+.0}ms"),
            Bot::SlowTap { ms } => format!("slow_tap_{ms:.0}ms"),
            Bot::Idle => "idle".into(),
            Bot::Spam => "spam".into(),
            Bot::HoldAll => "hold_all".into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BotRun {
    pub score: u32,
    pub cleared: u32,
    pub perfects: u32,
    pub goods: u32,
    pub oks: u32,
    pub stomps: u32,
    pub max_combo: u32,
    /// Lives lost before the run ended (or the time cap).
    pub lives_lost: u32,
    pub seconds: f32,
    pub bars: f32,
    pub died: bool,
    /// The beat (song time / beat length) of each life lost, with the reason.
    pub losses: Vec<(f32, String)>,
}

struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
    /// Roughly normal, unit sigma.
    fn gauss(&mut self) -> f32 {
        (self.next() + self.next() + self.next() + self.next() - 2.0) * 1.732
    }
}

struct Plan {
    press: f32,
    release: f32,
    pressed: bool,
}

/// Play one run on `spec` with `bot`, at most `max_secs` of song time.
pub fn play(spec: &HopSpec, bot: Bot, seed: u32, max_secs: f32) -> BotRun {
    let mut spec = spec.clone();
    spec.play.direct = false;
    // The bots measure the course, not the breather after a fall.
    spec.recover.enabled = false;
    spec.course.seed = seed;
    let bs = spec.beat_secs();
    let mut g = HopBeat::new(spec.clone());
    g.set_sfx(false);
    g.set_headless(true);
    g.confirm();
    let mut rng = Rng(seed.wrapping_mul(2_246_822_519).wrapping_add(3_266_489_917));
    let mut planned: HashSet<u32> = HashSet::new();
    let mut plans: Vec<Plan> = Vec::new();
    let mut last_release = -1.0_f32;
    let dt = 1.0 / 60.0;
    let mut lives_lost = 0;
    let mut losses: Vec<(f32, String)> = Vec::new();
    let mut lives = g.lives();
    let mut steps = 0u32;
    let max_steps = (max_secs / dt) as u32 + 600;
    while g.playing() && g.song_t() < max_secs && steps < max_steps {
        steps += 1;
        let now = g.song_t();
        match bot {
            Bot::Idle => {}
            Bot::HoldAll => g.set_hold(true),
            Bot::Spam => {
                let phase = (now / bs).fract();
                g.set_hold(phase < 0.1);
            }
            Bot::Perfect | Bot::Human { .. } | Bot::Offset { .. } | Bot::SlowTap { .. } => {
                let (sigma, miss) = match bot {
                    Bot::Human { sigma_ms, miss_pct } => (sigma_ms / 1000.0, miss_pct / 100.0),
                    _ => (0.0, 0.0),
                };
                let tap_hold = match bot {
                    Bot::SlowTap { ms } => ms / 1000.0,
                    _ => 0.05,
                };
                let shift = match bot {
                    Bot::Offset { ms } => ms / 1000.0,
                    _ => 0.0,
                };
                for o in g.course().obst.iter() {
                    if o.fate != Fate::Pending || !spec.needs_press(o) {
                        // A flyer is only something to stay clear of.
                        continue;
                    }
                    let ideal = o.take_beat * bs;
                    if ideal > now + 2.5 {
                        break;
                    }
                    let key = (o.take_beat * 1000.0) as u32 ^ ((o.kind == Kind::Gap) as u32) << 31;
                    if !planned.insert(key) {
                        continue;
                    }
                    if rng.next() < miss {
                        continue;
                    }
                    let press = (ideal + shift + rng.gauss() * sigma)
                        .max(now)
                        .max(last_release + 0.03);
                    let release = match o.hop {
                        Hop::Tap => press + tap_hold,
                        Hop::Hold => press + spec.arc(Hop::Hold).air_beats * bs * 0.8,
                    };
                    last_release = release;
                    plans.push(Plan {
                        press,
                        release,
                        pressed: false,
                    });
                }
                for p in plans.iter_mut() {
                    if !p.pressed && now >= p.press {
                        p.pressed = true;
                        g.set_hold(true);
                    } else if p.pressed && now >= p.release && g.hold_down() {
                        g.set_hold(false);
                    }
                }
                plans.retain(|p| !(p.pressed && now >= p.release + 0.05));
            }
        }
        g.tick(dt);
        if g.lives() < lives {
            lives_lost += lives - g.lives();
            lives = g.lives();
            losses.push((g.song_t() / bs, g.last_loss().to_string()));
        }
    }
    let (cleared, perfects, goods, oks) = g.counts();
    let seconds = g.song_t();
    BotRun {
        score: g.score(),
        cleared,
        perfects,
        goods,
        oks,
        stomps: g.stomps(),
        max_combo: g.max_combo(),
        lives_lost,
        seconds,
        bars: seconds / (bs * spec.music.beats_per_bar as f32),
        died: !g.playing(),
        losses,
    }
}

/// Timing window of a laid obstacle in ms. Walkers depend on where on the beat
/// grid they stand, so the cache key holds the fraction of the beat.
type WindowCache = Vec<((u8, u8, usize, u32, u32), f32)>;

fn window_ms(spec: &HopSpec, cache: &mut WindowCache, o: &Obst) -> f32 {
    let width_beats = (o.end - o.start) / spec.hero.beat_len_m;
    let key = (
        o.kind as u8,
        o.hop as u8,
        o.ety,
        (width_beats * 1000.0) as u32,
        ((o.take_beat.fract() * 1000.0).round()) as u32,
    );
    if let Some((_, w)) = cache.iter().find(|(k, _)| *k == key) {
        return *w;
    }
    let (lo, hi) = spec.window_secs_obst(o);
    let w = (hi - lo) * 1000.0;
    cache.push((key, w));
    w
}

/// What the placement check found.
#[derive(Clone, Debug, Default)]
pub struct PlacementReport {
    pub obstacles: usize,
    /// Problems in the phrase set itself (see `HopSpec::course_problems`).
    pub phrase_problems: Vec<String>,
    /// Real-geometry problems in the laid course (windows too tight, hops
    /// that overlap the next press).
    pub course_problems: Vec<String>,
    pub min_window_ms: f32,
    pub min_gap_ms: f32,
}

/// Lay `bars` bars of course and check every obstacle against the real hop
/// geometry by brute force: each must have a timing window of at least
/// `course.min_window_ms`, and the ideal hops must not overlap.
pub fn check_placement(spec: &HopSpec, bars: u32) -> PlacementReport {
    let bpb = spec.music.beats_per_bar as f32;
    let mut c = Course::new(spec);
    c.ensure(spec, bars as f32 * bpb);
    let bs = spec.beat_secs();
    let mut rep = PlacementReport {
        obstacles: c.obst.len(),
        phrase_problems: spec.course_problems(),
        min_window_ms: f32::MAX,
        min_gap_ms: f32::MAX,
        ..PlacementReport::default()
    };
    let mut cache: WindowCache = Vec::new();
    let mut prev_free = f32::MIN;
    for (i, o) in c.obst.iter().enumerate() {
        if !spec.needs_press(o) {
            continue;
        }
        let w = window_ms(spec, &mut cache, o);
        rep.min_window_ms = rep.min_window_ms.min(w);
        if w < spec.course.min_window_ms {
            rep.course_problems.push(format!(
                "obstacle {i} at beat {:.2}: window {w:.0} ms",
                o.take_beat
            ));
        }
        let gap_ms = (o.take_beat - prev_free) * bs * 1000.0;
        if prev_free > f32::MIN {
            rep.min_gap_ms = rep.min_gap_ms.min(gap_ms);
            if gap_ms < -1.0 {
                rep.course_problems.push(format!(
                    "obstacle {i} at beat {:.2}: starts {:.0} ms before the last hop lands",
                    o.take_beat, -gap_ms
                ));
            }
        }
        prev_free = o.take_beat + spec.arc(o.hop).air_beats;
        if o.kind == Kind::Enemy && spec.enemies.types.get(o.ety).is_some_and(|t| t.stompable) {
            prev_free += spec.enemies.bounce_beats;
        }
    }
    if rep.obstacles == 0 {
        rep.min_window_ms = 0.0;
        rep.min_gap_ms = 0.0;
    }
    rep
}

/// One bucket of the difficulty curve.
#[derive(Clone, Debug)]
pub struct CurvePoint {
    pub from_bar: u32,
    pub to_bar: u32,
    pub per_bar: f32,
    pub hold_share: f32,
    pub gap_share: f32,
    pub mean_window_ms: f32,
    pub min_window_ms: f32,
    /// Share of each beat taken up by hops in the air (0..1).
    pub busy: f32,
    /// Share of the bucket's obstacles that are enemies, and which kinds.
    pub enemy_share: f32,
    pub enemy_kinds: Vec<String>,
}

pub fn difficulty_curve(spec: &HopSpec, bars: u32, bucket: u32) -> Vec<CurvePoint> {
    let bpb = spec.music.beats_per_bar as f32;
    let mut c = Course::new(spec);
    c.ensure(spec, bars as f32 * bpb + 8.0);
    let mut out = Vec::new();
    let mut cache: WindowCache = Vec::new();
    let mut from = 0;
    while from < bars {
        let to = (from + bucket).min(bars);
        let (lo, hi) = (from as f32 * bpb, to as f32 * bpb);
        let items: Vec<_> = c
            .obst
            .iter()
            .filter(|o| o.take_beat >= lo && o.take_beat < hi)
            .collect();
        let n = items.len() as f32;
        let mut wins = Vec::new();
        let mut air = 0.0;
        for o in &items {
            if !spec.needs_press(o) {
                continue;
            }
            wins.push(window_ms(spec, &mut cache, o));
            air += spec.arc(o.hop).air_beats;
        }
        let mut kinds: Vec<String> = items
            .iter()
            .filter_map(|o| spec.enemies.types.get(o.ety).map(|t| t.id.clone()))
            .collect();
        kinds.sort();
        kinds.dedup();
        let mean = if wins.is_empty() {
            0.0
        } else {
            wins.iter().sum::<f32>() / wins.len() as f32
        };
        let minw = wins.iter().cloned().fold(f32::MAX, f32::min);
        out.push(CurvePoint {
            from_bar: from,
            to_bar: to,
            per_bar: n / (to - from) as f32,
            hold_share: if n > 0.0 {
                items.iter().filter(|o| o.hop == Hop::Hold).count() as f32 / n
            } else {
                0.0
            },
            gap_share: if n > 0.0 {
                items.iter().filter(|o| o.kind == Kind::Gap).count() as f32 / n
            } else {
                0.0
            },
            mean_window_ms: mean,
            min_window_ms: if wins.is_empty() { 0.0 } else { minw },
            busy: air / (hi - lo).max(1.0),
            enemy_share: if n > 0.0 {
                items.iter().filter(|o| o.kind == Kind::Enemy).count() as f32 / n
            } else {
                0.0
            },
            enemy_kinds: kinds,
        });
        from = to;
    }
    out
}

fn r2(v: f32) -> f64 {
    (v as f64 * 100.0).round() / 100.0
}

fn stat(runs: &[BotRun]) -> Value {
    let n = runs.len().max(1) as f32;
    let mean = |f: &dyn Fn(&BotRun) -> f32| runs.iter().map(f).sum::<f32>() / n;
    let mut secs: Vec<f32> = runs.iter().map(|r| r.seconds).collect();
    secs.sort_by(|a, b| a.total_cmp(b));
    json!({
        "runs": runs.len(),
        "mean_score": r2(mean(&|r| r.score as f32)),
        "mean_cleared": r2(mean(&|r| r.cleared as f32)),
        "mean_perfects": r2(mean(&|r| r.perfects as f32)),
        "mean_stomps": r2(mean(&|r| r.stomps as f32)),
        "mean_max_combo": r2(mean(&|r| r.max_combo as f32)),
        "mean_lives_lost": r2(mean(&|r| r.lives_lost as f32)),
        "median_seconds": r2(secs[secs.len() / 2]),
        "mean_bars": r2(mean(&|r| r.bars)),
        "died": runs.iter().filter(|r| r.died).count(),
    })
}

/// The whole report as JSON (what `docs/measure/hop_beat_baseline.json` holds).
pub fn report(spec: &HopSpec) -> Value {
    let seeds: Vec<u32> = (1..=8).collect();
    let cap = 150.0;
    let bots = [
        Bot::Perfect,
        Bot::Human {
            sigma_ms: 25.0,
            miss_pct: 2.0,
        },
        Bot::Human {
            sigma_ms: 55.0,
            miss_pct: 6.0,
        },
        Bot::Human {
            sigma_ms: 90.0,
            miss_pct: 12.0,
        },
        Bot::Idle,
        Bot::Spam,
        Bot::HoldAll,
    ];
    let mut bot_json = serde_json::Map::new();
    for b in bots {
        let runs: Vec<BotRun> = seeds.iter().map(|s| play(spec, b, *s, cap)).collect();
        bot_json.insert(b.name(), stat(&runs));
    }
    let place = check_placement(spec, 200);
    let curve: Vec<Value> = difficulty_curve(spec, 80, 8)
        .iter()
        .map(|p| {
            json!({
                "bars": [p.from_bar, p.to_bar],
                "obstacles_per_bar": r2(p.per_bar),
                "hold_share": r2(p.hold_share),
                "gap_share": r2(p.gap_share),
                "mean_window_ms": r2(p.mean_window_ms),
                "min_window_ms": r2(p.min_window_ms),
                "busy": r2(p.busy),
                "enemy_share": r2(p.enemy_share),
                "enemy_kinds": p.enemy_kinds,
            })
        })
        .collect();
    json!({
        "game": spec.id,
        "bpm": spec.music.bpm,
        "seeds": seeds,
        "time_cap_secs": cap,
        "bots": Value::Object(bot_json),
        "placement": {
            "bars_checked": 200,
            "obstacles": place.obstacles,
            "phrase_problems": place.phrase_problems,
            "course_problems": place.course_problems,
            "min_window_ms": r2(place.min_window_ms),
            "min_gap_ms": r2(place.min_gap_ms),
        },
        "difficulty_curve": curve,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> HopSpec {
        HopSpec::default()
    }

    #[test]
    fn perfect_bot_clears_the_course_untouched() {
        let r = play(&spec(), Bot::Perfect, 3, 90.0);
        assert!(!r.died, "perfect bot died: {r:?}");
        assert_eq!(r.lives_lost, 0, "{r:?}");
        assert!(r.cleared > 20, "{r:?}");
        assert!(r.perfects as f32 >= r.cleared as f32 * 0.95, "{r:?}");
    }

    #[test]
    fn idle_bot_dies_early() {
        let r = play(&spec(), Bot::Idle, 3, 120.0);
        assert!(r.died, "{r:?}");
        assert!(r.seconds < 40.0, "{r:?}");
    }

    #[test]
    fn spam_and_hold_all_do_not_win() {
        for b in [Bot::Spam, Bot::HoldAll] {
            let r = play(&spec(), b, 5, 120.0);
            assert!(r.died, "{} survived: {r:?}", b.name());
        }
    }

    #[test]
    fn skilled_beats_sloppy() {
        let good = play(
            &spec(),
            Bot::Human {
                sigma_ms: 25.0,
                miss_pct: 2.0,
            },
            2,
            150.0,
        );
        let bad = play(
            &spec(),
            Bot::Human {
                sigma_ms: 90.0,
                miss_pct: 12.0,
            },
            2,
            150.0,
        );
        assert!(good.score > bad.score, "{good:?} vs {bad:?}");
    }

    #[test]
    fn placement_check_is_clean_for_the_stock_phrases() {
        let r = check_placement(&spec(), 200);
        assert!(r.phrase_problems.is_empty(), "{:?}", r.phrase_problems);
        assert!(r.course_problems.is_empty(), "{:?}", r.course_problems);
        assert!(r.obstacles > 50);
        assert!(
            r.min_window_ms >= spec().course.min_window_ms,
            "{}",
            r.min_window_ms
        );
    }

    #[test]
    fn placement_check_catches_an_impossible_spec() {
        let mut s = spec();
        s.course.phrases = vec![crate::hop_beat::Phrase {
            level: 0,
            bars: 2,
            // A gap far wider than the longest hop can span.
            items: vec![crate::hop_beat::Item {
                kind: "gap".into(),
                hop: "tap".into(),
                beat: 1.0,
                beats: 3.5,
                enemy: String::new(),
            }],
            weight: 1,
            debut: false,
        }];
        assert!(!s.course_problems().is_empty());
        assert!(s.validate().is_err());
    }

    #[test]
    fn difficulty_rises_then_levels_off() {
        let c = difficulty_curve(&spec(), 80, 8);
        assert_eq!(c.len(), 10);
        let early: f32 = c[..2].iter().map(|p| p.per_bar + p.busy).sum();
        let late: f32 = c[6..].iter().map(|p| p.per_bar + p.busy).sum();
        assert!(late > early * 1.2, "early {early} late {late}");
        assert!(
            c[9].per_bar > c[0].per_bar * 2.0,
            "{} -> {}",
            c[0].per_bar,
            c[9].per_bar
        );
    }

    #[test]
    fn baseline_file_matches_the_code() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../docs/measure/hop_beat_baseline.json"
        );
        let Ok(text) = std::fs::read_to_string(path) else {
            panic!("docs/measure/hop_beat_baseline.json missing: run `cargo run -p kagra-shared --example hop_measure -- --write`");
        };
        let want: Value = serde_json::from_str(&text).unwrap();
        let got = report(&spec());
        assert!(
            close(&want, &got, 0.02),
            "baseline is stale; regenerate with --write\nwant {want}\ngot  {got}"
        );
    }

    fn close(a: &Value, b: &Value, rel: f64) -> bool {
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => {
                let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
                (x - y).abs() <= rel * x.abs().max(y.abs()).max(1.0)
            }
            (Value::Array(x), Value::Array(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(p, q)| close(p, q, rel))
            }
            (Value::Object(x), Value::Object(y)) => {
                x.len() == y.len()
                    && x.iter()
                        .all(|(k, v)| y.get(k).is_some_and(|w| close(v, w, rel)))
            }
            _ => a == b,
        }
    }

    fn item(beat: f32, kind: &str, beats: f32, hop: &str, enemy: &str) -> crate::hop_beat::Item {
        crate::hop_beat::Item {
            beat,
            kind: kind.into(),
            beats,
            hop: hop.into(),
            enemy: enemy.into(),
        }
    }

    fn phrase(items: Vec<crate::hop_beat::Item>) -> crate::hop_beat::Phrase {
        crate::hop_beat::Phrase {
            level: 0,
            bars: 2,
            items,
            weight: 1,
            debut: false,
        }
    }

    /// Every phrase with a bat, next to every other phrase, can be played at any press
    /// within 100 ms of the beat: a bat over a gap, or after a hold, is never a trap.
    #[test]
    fn every_bat_phrase_pair_is_winnable_at_100ms() {
        let s = spec();
        let problems = s.seam_problems(100.0);
        assert!(problems.is_empty(), "{problems:#?}");
        // The checker does see the bats of the stock set.
        let ph = s.phrases();
        let bats: Vec<_> = ph.iter().filter(|p| s.has_flyer(p)).collect();
        assert!(bats.len() >= 4, "{}", bats.len());
        for p in bats {
            assert!(
                s.phrase_half_ms(p) >= 100.0,
                "a bat phrase holds at +-100 ms alone"
            );
        }
    }

    /// The same check flags a bat over a gap that only a hold hop clears: a tap
    /// would fall in, a hold flies into the bat.
    #[test]
    fn a_bat_over_a_hold_gap_is_caught() {
        let s = spec();
        let trap = phrase(vec![
            item(1.0, "gap", 1.5, "hold", ""),
            item(1.0, "enemy", 0.9, "tap", "bat"),
        ]);
        assert!(s.has_flyer(&trap));
        let calm = phrase(vec![item(1.0, "gap", 0.5, "tap", "")]);
        assert!(
            s.seam_fail(&trap, &calm, 0.0).is_some(),
            "a bat over a hold gap must fail even on the beat"
        );
        let mut custom = s.clone();
        custom.course.phrases = vec![trap];
        assert!(
            !custom.course_problems().is_empty(),
            "course_problems must flag a bat over a hold gap"
        );
        // And the stock bat-over-a-tap-gap phrase is fine.
        let fair = phrase(vec![
            item(1.0, "gap", 0.5, "tap", ""),
            item(1.0, "enemy", 0.9, "tap", "bat"),
        ]);
        assert!(s.seam_fail(&fair, &calm, 100.0).is_none());
    }

    /// 2000 daily-style seeds, every modifier of the shipped set (sprint, no_breaks,
    /// midnight, stomp_party) and the plain course: every phrase seam the generator lays
    /// is one that was played at +-100 ms (flyers) or +-the Perfect window (the rest).
    #[test]
    fn laid_courses_only_use_winnable_seams() {
        use std::collections::HashMap;
        let base = spec();
        let phrases = base.phrases();
        let mut memo: HashMap<(usize, usize), bool> = HashMap::new();
        let mut seams_seen = 0usize;
        type Tweak = fn(&mut HopSpec);
        let mods: [(&str, Tweak); 5] = [
            ("plain", |_| {}),
            ("sprint", |s| s.course.ramp_bars = 24),
            ("no_breaks", |s| s.course.rest_every_bars = 16),
            ("midnight", |s| {
                s.look.accent = [190, 140, 255];
                s.look.glow = [120, 160, 255];
                s.look.slab = [30, 28, 60];
            }),
            ("stomp_party", |s| s.enemies.stomp_score = 300),
        ];
        for (name, apply) in mods {
            let mut sp = base.clone();
            apply(&mut sp);
            sp.validate().unwrap();
            for seed in 0..2000u32 {
                sp.course.seed = seed.wrapping_mul(2_654_435_761).wrapping_add(seed);
                let mut c = Course::new(&sp);
                c.ensure(&sp, 160.0 * sp.music.beats_per_bar as f32);
                let laid = c.laid().to_vec();
                for w in laid.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    if a == usize::MAX || b == usize::MAX {
                        continue;
                    }
                    seams_seen += 1;
                    let ok = *memo.entry((a, b)).or_insert_with(|| {
                        let cap = if sp.has_flyer(&phrases[a]) || sp.has_flyer(&phrases[b]) {
                            100.0
                        } else {
                            sp.judge.perfect_ms.min(100.0)
                        };
                        let half = cap.min(sp.pair_half_ms(&phrases[a], &phrases[b]));
                        sp.seam_fail(&phrases[a], &phrases[b], half).is_none()
                    });
                    assert!(
                        ok,
                        "{name} seed {seed}: phrase {a} then {b} is not winnable"
                    );
                }
            }
        }
        assert!(seams_seen > 100_000, "{seams_seen}");
    }

    /// A tap that stays down a little long (a thumb, not a click) is still a tap: the
    /// bat phrases have no trap for it. Found by this test: with the old 110 ms tap window
    /// a 140 ms touch turned into a hold hop under every bat.
    #[test]
    fn a_thumb_tap_clears_every_bat_phrase() {
        let s = spec();
        for seed in 0..40u32 {
            let r = play(&s, Bot::SlowTap { ms: 140.0 }, seed, 260.0);
            let mut sp = s.clone();
            sp.course.seed = seed;
            let mut c = Course::new(&sp);
            c.ensure(&sp, 700.0);
            let near: Vec<String> = r
                .losses
                .iter()
                .map(|(b, why)| {
                    let o: Vec<String> = c
                        .obst
                        .iter()
                        .filter(|o| o.take_beat <= b + 0.3 && o.take_beat > b - 3.0)
                        .map(|o| {
                            format!(
                                "{:?}{:?}@{:.1}ety{}",
                                o.kind, o.hop, o.take_beat, o.ety as i32
                            )
                        })
                        .collect();
                    format!("{why} at beat {b:.1}: {o:?}")
                })
                .collect();
            assert!(r.losses.is_empty(), "seed {seed}: {near:#?}");
        }
    }
}
