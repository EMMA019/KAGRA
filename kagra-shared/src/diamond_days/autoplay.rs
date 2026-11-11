//! Autoplay statistics: the game played headless through its own screens, for years on end,
//! so that balance is read off numbers instead of off a feeling.
//!
//! The harness is a coach bot. It taps the same buttons a player does (the first card every
//! week, the game skipped to its result, every question answered with the first answer) and
//! writes down what the game does: how many games of each kind, the win rate, the errors, which
//! squares the coach lands on, how the players grow, how the school's name moves, who goes pro.
//! It reads state the game already keeps and changes none of it, so a run is exactly what a
//! player who does nothing but the defaults would get, and it repeats exactly: the seed fixes
//! every dice roll.
//!
//! * `cargo test -p kagra-shared --lib autoplay_smoke` is the short run CI does (one seed, two
//!   years). It checks that the harness works and repeats, never the balance.
//! * `cargo test -p kagra-shared --lib autoplay_baseline -- --ignored --nocapture` is the full
//!   run (5 seeds, 10 years), written to `target/autoplay/autoplay.{md,json}`.
//!   `KAGRA_AUTOPLAY_SEEDS`, `_YEARS`, `_OUT`, `_SPEND=1` (spend the points by hand every week)
//!   and `_GACHA=1` (pull every free scout) change it. See `docs/design/diamond_days_autoplay.md`.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::tests::{press, run, tap};
use super::*;
use team::{Player, Stat};

/// The phone screen the bot plays on (the same one the other tests use).
const W: u32 = 390;
const H: u32 = 844;

/// Safety stop for one run: the bot is stuck if it has not finished the years by then.
const STEP_LIMIT: u64 = 6_000_000;

/// What the bot does besides the defaults (the game's own monthly automatic growth and menus,
/// the first card, no scouting).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Policy {
    /// Spend every practice point by hand each week (the AUTO SPEND button) instead of
    /// leaving it to the monthly automatic growth.
    pub spend_weekly: bool,
    /// Pull every free April scout (gacha).
    pub gacha: bool,
}

impl Policy {
    fn label(self) -> String {
        let mut v = vec!["first card", "games skipped", "orders left to the team"];
        v.push(if self.spend_weekly {
            "points spent by hand weekly"
        } else {
            "points left to the monthly automatic growth"
        });
        v.push(if self.gacha {
            "every free scout pulled"
        } else {
            "no scouting"
        });
        v.join(", ")
    }
}

/// The kinds of game, in the order the tables list them.
const KINDS: [&str; 6] = [
    "Practice",
    "Summer",
    "National",
    "Autumn",
    "Spring",
    "SpringPref",
];

fn kind_of(f: Fixture) -> &'static str {
    match f {
        Fixture::Practice => "Practice",
        Fixture::SpringPref(_) => "SpringPref",
        Fixture::Summer(_) => "Summer",
        Fixture::National(_) => "National",
        Fixture::Autumn(_) => "Autumn",
        Fixture::Spring(_) => "Spring",
    }
}

fn kind_ja(k: &str) -> &'static str {
    match k {
        "Practice" => "練習試合",
        "Summer" => "夏の県大会",
        "National" => "全国大会",
        "Autumn" => "秋季大会",
        "Spring" => "春の招待",
        _ => "春季県大会",
    }
}

/// One game, as the result screen sees it.
#[derive(Clone, Debug)]
struct GameRec {
    year: u32,
    kind: &'static str,
    won: bool,
    runs_us: u32,
    runs_them: u32,
    /// Errors our fielders made / the other school's made.
    err_us: u32,
    err_them: u32,
    /// Plate appearances (both sides).
    pa: u32,
}

/// A graduate who joined after the run began: his stats the first week he was seen and the
/// last week.
#[derive(Clone, Debug)]
struct Grad {
    pitcher: bool,
    join: Vec<f64>,
    exit: Vec<f64>,
}

/// Everything one seed's run wrote down.
#[derive(Clone, Debug, Default)]
pub(super) struct SeedRun {
    seed: u32,
    games: Vec<GameRec>,
    /// (career year, square) for every card the coach played.
    landings: Vec<(u32, String)>,
    /// The school's name at the start and at the end of each year.
    rep: Vec<f64>,
    /// Graduates who went pro, per year.
    pros: Vec<u32>,
    /// The year's best result, 0 (a national title) to 3 (nothing special), per year.
    best: Vec<u8>,
    /// Wins and losses per year as the game itself counts them (a check on `games`).
    history: Vec<(u32, u32)>,
    grads: Vec<Grad>,
    /// Third-years who left the team at the end of each year (the founding ones too).
    left_by_year: Vec<u32>,
    /// Players who were seen and then left before the third year (should be none).
    left_early: u32,
    steps: u64,
}

/// The stats a role is measured on: the six a fielder has and the four a pitcher has. A
/// pitcher's velocity is in km/h in the record and scaled to 0 to 100 (as the gauge on his
/// page is) when it is averaged with the others.
fn role_stats(pitcher: bool) -> &'static [Stat] {
    if pitcher {
        &[Stat::Velo, Stat::Control, Stat::Stamina, Stat::Breaking]
    } else {
        &[
            Stat::Contact,
            Stat::Power,
            Stat::Speed,
            Stat::Field,
            Stat::Catching,
            Stat::Arm,
        ]
    }
}

fn stats_of(p: &Player) -> Vec<f64> {
    role_stats(p.pitcher())
        .iter()
        .map(|st| p.stat(*st) as f64)
        .collect()
}

/// A player's overall level from his role's stats.
fn level(pitcher: bool, v: &[f64]) -> f64 {
    if pitcher {
        ((v[0] - 100.0) / 60.0 * 100.0 + v[1] + v[2] + v[3]) / 4.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

struct Tracked {
    join: Vec<f64>,
    last: Vec<f64>,
    grade: u8,
    pitcher: bool,
    /// On the team when the run began: his join stats are not known.
    founding: bool,
}

/// Follows every player by name from the week he is first seen until he leaves.
#[derive(Default)]
struct Tracker {
    live: BTreeMap<String, Tracked>,
    started: bool,
}

impl Tracker {
    /// `ended`: how many years are over (whoever left since the last look left at the end
    /// of that year).
    fn observe(&mut self, g: &DiamondDays, out: &mut SeedRun, ended: usize) {
        let mut present = Vec::new();
        for p in g.s.team.players.iter().chain(g.s.seniors.iter()) {
            let key = format!("{} {}", p.family, p.given);
            present.push(key.clone());
            let now = stats_of(p);
            match self.live.get_mut(&key) {
                Some(t) if p.grade >= t.grade => {
                    t.last = now;
                    t.grade = p.grade;
                }
                found => {
                    // A boy with a graduate's name: the graduate is gone.
                    if found.is_some() {
                        if let Some(old) = self.live.remove(&key) {
                            Self::close(old, out, ended);
                        }
                    }
                    self.live.insert(
                        key,
                        Tracked {
                            join: now.clone(),
                            last: now,
                            grade: p.grade,
                            pitcher: p.pitcher(),
                            founding: !self.started,
                        },
                    );
                }
            }
        }
        self.started = true;
        let gone: Vec<String> = self
            .live
            .keys()
            .filter(|k| !present.contains(k))
            .cloned()
            .collect();
        for k in gone {
            if let Some(t) = self.live.remove(&k) {
                Self::close(t, out, ended);
            }
        }
    }

    fn close(t: Tracked, out: &mut SeedRun, ended: usize) {
        if t.grade >= 3 && ended >= 1 {
            if out.left_by_year.len() < ended {
                out.left_by_year.resize(ended, 0);
            }
            out.left_by_year[ended - 1] += 1;
        }
        if t.founding {
            return;
        }
        if t.grade >= 3 {
            out.grads.push(Grad {
                pitcher: t.pitcher,
                join: t.join,
                exit: t.last,
            });
        } else {
            out.left_early += 1;
        }
    }
}

/// Where a card landed: the square's name, with the winter game (an intrasquad match for
/// points, no other school) told apart from a game in season.
fn landing_label(g: &DiamondDays) -> String {
    let sq = g.spec.ring[g.s.pos % g.spec.ring.len()];
    if sq == Square::Game && board::WINTER.contains(&g.s.week) {
        "Game (winter)".into()
    } else {
        sq.label(false).into()
    }
}

/// Plays `years` years from `seed` through the screens and writes down what happened.
pub(super) fn run_seed(seed: u32, years: u32, policy: Policy) -> SeedRun {
    let spec = DiamondDaysSpec {
        seed,
        ..DiamondDaysSpec::default()
    };
    let mut g = DiamondDays::new(spec);
    let _ = g.hud(W, H, false);
    g.confirm();
    let mut out = SeedRun {
        seed,
        rep: vec![g.s.reputation as f64],
        ..SeedRun::default()
    };
    let mut tracker = Tracker::default();
    let mut years_seen = 0usize;
    let mut counted = false;
    let mut last_week = (0u8, 0u8, 0u8);
    while out.steps < STEP_LIMIT {
        out.steps += 1;
        // A year has just ended: write down the school's name, and stop after the last one.
        while years_seen < g.s.history.len() {
            years_seen += 1;
            out.rep.push(g.s.reputation as f64);
        }
        if g.s.history.len() as u32 >= years {
            break;
        }
        if !matches!(g.mode, Mode::Match { .. }) {
            counted = false;
        }
        match g.mode {
            Mode::Notice => tap(&mut g, 195.0, 400.0),
            Mode::Ask { .. } => press(&mut g, hud::Btn::Choose(0)),
            Mode::Plan => {
                let now = (g.s.chapter, g.s.year, g.s.week);
                if now != last_week {
                    last_week = now;
                    tracker.observe(&g, &mut out, years_seen);
                    if policy.spend_weekly {
                        for p in &mut g.s.team.players {
                            p.auto_raise();
                        }
                    }
                }
                press(&mut g, hud::Btn::Card(0));
            }
            Mode::Walk { .. } => {
                run(&mut g, 0.3);
                if !matches!(g.mode, Mode::Walk { .. }) {
                    out.landings.push((g.career_year(), landing_label(&g)));
                }
            }
            Mode::Match {
                asking: Some(_), ..
            } => press(&mut g, hud::Btn::Call(None)),
            Mode::Match { .. } => {
                // The first skip plays the game to its last moment; the game is over there
                // and still in hand, so count it then (the second skip files it away).
                if !counted {
                    if let Some(gm) = g.game.as_ref().filter(|gm| gm.over) {
                        counted = true;
                        out.games.push(record_game(&g, gm));
                    }
                }
                press(&mut g, hud::Btn::Skip);
            }
            Mode::Player(_) | Mode::Menus => press(&mut g, hud::Btn::Back),
            Mode::Roster => tap(&mut g, 10.0, 10.0),
            Mode::Lineup { pick: Some(_) } => press(&mut g, hud::Btn::Back),
            Mode::Lineup { .. } => press(&mut g, hud::Btn::Close),
            Mode::CampTalk { .. } => press(&mut g, hud::Btn::Choose(0)),
            Mode::Epilogue { schools: true } => press(&mut g, hud::Btn::Back),
            // The coach keeps going at the same school, chapter after chapter.
            Mode::Epilogue { .. } => press(&mut g, hud::Btn::Stay),
            Mode::MatchIntro => press(&mut g, hud::Btn::PlayBall),
            Mode::MatchEnd => press(&mut g, hud::Btn::Continue),
            Mode::Bracket | Mode::Decide(_) => press(&mut g, hud::Btn::Choose(0)),
            Mode::Gacha => {
                if policy.gacha && g.gacha_left() > 0 {
                    press(&mut g, hud::Btn::GachaDraw);
                } else {
                    press(&mut g, hud::Btn::GachaDone);
                }
            }
        }
    }
    assert!(
        g.s.history.len() as u32 >= years,
        "seed {seed}: stuck in {:?} (year {}, week {}) after {} steps",
        g.mode,
        g.s.year,
        g.s.week,
        out.steps
    );
    // The last year's graduates left the roster as it ended: one more look closes them.
    tracker.observe(&g, &mut out, years_seen);
    out.left_by_year.resize(years as usize, 0);
    // The year-by-year facts the game itself keeps.
    for y in 1..=years {
        out.pros
            .push(g.s.pros.iter().filter(|(_, at)| *at == y).count() as u32);
    }
    for h in g.s.history.iter().take(years as usize) {
        out.best.push(h.tier());
        out.history.push((h.wins, h.losses));
    }
    out
}

fn record_game(g: &DiamondDays, gm: &sim::Game) -> GameRec {
    let us = gm.us;
    let errors = |ours: bool| {
        gm.log
            .iter()
            .filter(|p| p.kind == sim::PlayKind::Error && ((p.side as usize != us) == ours))
            .count() as u32
    };
    GameRec {
        year: g.career_year(),
        kind: kind_of(g.game_this_week().unwrap_or(Fixture::Practice)),
        won: gm.won() == Some(true),
        runs_us: gm.runs(us),
        runs_them: gm.runs(1 - us),
        err_us: errors(true),
        err_them: errors(false),
        pa: gm.log.iter().filter(|p| p.faced_batter()).count() as u32,
    }
}

// ---- the numbers

/// A number across the seeds: the mean, the spread between seeds and the extremes.
#[derive(Clone, Copy, Debug, Default)]
struct Spread {
    mean: f64,
    sd: f64,
    min: f64,
    max: f64,
    n: usize,
}

fn spread(v: &[f64]) -> Spread {
    if v.is_empty() {
        return Spread::default();
    }
    let n = v.len();
    let mean = v.iter().sum::<f64>() / n as f64;
    let sd = if n > 1 {
        (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt()
    } else {
        0.0
    };
    Spread {
        mean,
        sd,
        min: v.iter().cloned().fold(f64::INFINITY, f64::min),
        max: v.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        n,
    }
}

fn r4(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

impl Spread {
    fn json(self) -> Value {
        json!({"mean": r4(self.mean), "sd": r4(self.sd), "min": r4(self.min), "max": r4(self.max), "n": self.n})
    }

    /// `mean ± sd` for a table cell.
    fn cell(self, digits: usize) -> String {
        format!("{:.*} ± {:.*}", digits, self.mean, digits, self.sd)
    }

    fn range(self, digits: usize) -> String {
        format!("{:.*}–{:.*}", digits, self.min, digits, self.max)
    }
}

/// Collects the summary numbers for the JSON file as the tables are built.
#[derive(Default)]
struct Summary {
    keys: BTreeMap<String, Value>,
}

impl Summary {
    fn put(&mut self, key: impl Into<String>, s: Spread) -> Spread {
        self.keys.insert(key.into(), s.json());
        s
    }
}

pub(super) struct Report {
    runs: Vec<SeedRun>,
    years: u32,
    policy: Policy,
}

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        a as f64 * 100.0 / b as f64
    }
}

impl Report {
    pub(super) fn new(runs: Vec<SeedRun>, years: u32, policy: Policy) -> Self {
        Self {
            runs,
            years,
            policy,
        }
    }

    /// Per seed, per year, and `f` of the seed's games that pass `pick`, averaged over the
    /// years.
    fn per_year(&self, f: impl Fn(&SeedRun) -> f64) -> Vec<f64> {
        self.runs.iter().map(|r| f(r) / self.years as f64).collect()
    }

    fn games_of<'a>(r: &'a SeedRun, kind: Option<&str>) -> Vec<&'a GameRec> {
        r.games
            .iter()
            .filter(|g| kind.is_none_or(|k| g.kind == k))
            .collect()
    }

    /// The seed's win rate over the games `pick` selects (None: no such game).
    fn win_rate(r: &SeedRun, pick: impl Fn(&GameRec) -> bool) -> Option<f64> {
        let v: Vec<&GameRec> = r.games.iter().filter(|g| pick(g)).collect();
        (!v.is_empty()).then(|| pct(v.iter().filter(|g| g.won).count(), v.len()))
    }

    /// The checks that say the harness itself works (not the game's balance).
    pub(super) fn problems(&self) -> Vec<String> {
        let mut bad = Vec::new();
        for r in &self.runs {
            let mine: u32 = r.games.len() as u32;
            let theirs: u32 = r.history.iter().map(|(w, l)| w + l).sum();
            if mine != theirs {
                bad.push(format!(
                    "seed {}: {mine} games seen, the game's own record says {theirs}",
                    r.seed
                ));
            }
            for (y, (w, l)) in r.history.iter().enumerate() {
                let year = y as u32 + 1;
                let won = r.games.iter().filter(|g| g.year == year && g.won).count() as u32;
                let all = r.games.iter().filter(|g| g.year == year).count() as u32;
                if won != *w || all != w + l {
                    bad.push(format!(
                        "seed {} year {year}: seen {won}/{all}, the game says {w}/{}",
                        r.seed,
                        w + l
                    ));
                }
            }
            if r.rep.len() as u32 != self.years + 1 {
                bad.push(format!(
                    "seed {}: {} reputation readings",
                    r.seed,
                    r.rep.len()
                ));
            }
        }
        bad
    }

    /// Everything as one JSON value: the run, the summary and each seed's own numbers.
    pub(super) fn json(&self) -> Value {
        let mut sum = Summary::default();
        let _ = self.tables(&mut sum);
        let per_seed: Vec<Value> = self.runs.iter().map(|r| self.seed_json(r)).collect();
        json!({
            "schema": 1,
            "meta": {
                "years": self.years,
                "seeds": self.runs.iter().map(|r| r.seed).collect::<Vec<_>>(),
                "policy": self.policy.label(),
                "ring": DiamondDaysSpec::default().ring.iter().map(|s| s.label(false)).collect::<Vec<_>>(),
            },
            "summary": sum.keys,
            "per_seed": per_seed,
        })
    }

    fn seed_json(&self, r: &SeedRun) -> Value {
        let years: Vec<Value> = (1..=self.years)
            .map(|y| {
                let games: Vec<&GameRec> = r.games.iter().filter(|g| g.year == y).collect();
                let by = |won_only: bool| {
                    KINDS
                        .iter()
                        .map(|k| {
                            let n = games
                                .iter()
                                .filter(|g| g.kind == *k && (!won_only || g.won))
                                .count();
                            (k.to_string(), n)
                        })
                        .filter(|(_, n)| *n > 0)
                        .collect::<BTreeMap<_, _>>()
                };
                let mut land: BTreeMap<String, u32> = BTreeMap::new();
                for (_, l) in r.landings.iter().filter(|(yy, _)| *yy == y) {
                    *land.entry(l.clone()).or_default() += 1;
                }
                json!({
                    "year": y,
                    "games": by(false),
                    "wins": by(true),
                    "errors_us": games.iter().map(|g| g.err_us).sum::<u32>(),
                    "errors_them": games.iter().map(|g| g.err_them).sum::<u32>(),
                    "plate_appearances": games.iter().map(|g| g.pa).sum::<u32>(),
                    "runs_us": games.iter().map(|g| g.runs_us).sum::<u32>(),
                    "runs_them": games.iter().map(|g| g.runs_them).sum::<u32>(),
                    "landings": land,
                    "reputation_end": r4(r.rep[y as usize]),
                    "pros": r.pros[y as usize - 1],
                    "best_tier": r.best[y as usize - 1],
                })
            })
            .collect();
        json!({
            "seed": r.seed,
            "reputation_start": r4(r.rep[0]),
            "graduates": r.grads.len(),
            "left_early": r.left_early,
            "years": years,
        })
    }

    /// The report as Markdown tables.
    pub(super) fn markdown(&self) -> String {
        let mut sum = Summary::default();
        let mut md = String::new();
        md.push_str("# 白球デイズ 自動プレイの集計\n\n");
        md.push_str(&format!(
            "- シード: {}（{} 本）／ {} 年ずつ\n- コーチの動き: {}\n- 数字は「シード間の平均 ± SD（シード間のばらつき）」と最小–最大。年ごとの数字は 1 年あたり。\n\n",
            self.runs
                .iter()
                .map(|r| r.seed.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            self.runs.len(),
            self.years,
            self.policy.label(),
        ));
        md.push_str(&self.tables(&mut sum));
        let bad = self.problems();
        md.push_str("\n## ハーネスの自己チェック\n\n");
        if bad.is_empty() {
            md.push_str("- 数えた試合数が、ゲーム自身の勝敗記録と全シード・全年で同じ。\n");
        } else {
            for b in &bad {
                md.push_str(&format!("- ⚠ {b}\n"));
            }
        }
        let early: u32 = self.runs.iter().map(|r| r.left_early).sum();
        md.push_str(&format!(
            "- 3 年を待たずにチームから消えた選手: {early} 人。\n"
        ));
        md
    }

    fn tables(&self, sum: &mut Summary) -> String {
        let mut md = String::new();
        let years = self.years as f64;

        // ---- games
        md.push_str("## 年間試合数\n\n| 種別 | 1年あたり | シード間 |\n|---|---|---|\n");
        let all = sum.put(
            "games_per_year.total",
            spread(&self.per_year(|r| r.games.len() as f64)),
        );
        md.push_str(&format!(
            "| **合計** | {} | {} |\n",
            all.cell(1),
            all.range(1)
        ));
        for k in KINDS {
            let s = sum.put(
                format!("games_per_year.{k}"),
                spread(&self.per_year(|r| Self::games_of(r, Some(k)).len() as f64)),
            );
            if s.max > 0.0 {
                md.push_str(&format!(
                    "| {} ({k}) | {} | {} |\n",
                    kind_ja(k),
                    s.cell(2),
                    s.range(2)
                ));
            }
        }

        // ---- win rate
        md.push_str(
            "\n## 勝率（%）\n\n| 範囲 | 全試合まとめ | シード間 | 試合数 |\n|---|---|---|---|\n",
        );
        type Pick = Box<dyn Fn(&GameRec) -> bool>;
        let mut rows: Vec<(String, String, Pick)> = vec![
            ("all".into(), "**全体**".into(), Box::new(|_| true)),
            (
                "cups".into(),
                "大会のみ".into(),
                Box::new(|g| g.kind != "Practice"),
            ),
        ];
        for k in KINDS {
            rows.push((
                k.to_string(),
                format!("{} ({k})", kind_ja(k)),
                Box::new(move |g| g.kind == k),
            ));
        }
        for (key, label, pick) in &rows {
            let n: usize = self
                .runs
                .iter()
                .map(|r| r.games.iter().filter(|g| pick(g)).count())
                .sum();
            if n == 0 {
                continue;
            }
            let won: usize = self
                .runs
                .iter()
                .map(|r| r.games.iter().filter(|g| pick(g) && g.won).count())
                .sum();
            let per_seed: Vec<f64> = self
                .runs
                .iter()
                .filter_map(|r| Self::win_rate(r, |g| pick(g)))
                .collect();
            let s = sum.put(format!("win_rate.{key}"), spread(&per_seed));
            md.push_str(&format!(
                "| {label} | {:.1} | {} ({}) | {n} |\n",
                pct(won, n),
                s.cell(1),
                s.range(1)
            ));
        }

        // ---- errors and runs
        md.push_str("\n## エラーと得点\n\n| 指標 | 平均 | シード間 |\n|---|---|---|\n");
        let per_game = |f: fn(&GameRec) -> u32| {
            self.runs
                .iter()
                .filter(|r| !r.games.is_empty())
                .map(|r| r.games.iter().map(f).sum::<u32>() as f64 / r.games.len() as f64)
                .collect::<Vec<f64>>()
        };
        for (key, label, v) in [
            (
                "errors_per_game.us",
                "自校のエラー / 試合",
                per_game(|g| g.err_us),
            ),
            (
                "errors_per_game.them",
                "相手のエラー / 試合",
                per_game(|g| g.err_them),
            ),
            (
                "errors_per_game.both",
                "両校のエラー / 試合",
                per_game(|g| g.err_us + g.err_them),
            ),
            (
                "runs_per_game.us",
                "自校の得点 / 試合",
                per_game(|g| g.runs_us),
            ),
            (
                "runs_per_game.them",
                "相手の得点 / 試合",
                per_game(|g| g.runs_them),
            ),
        ] {
            let s = sum.put(key, spread(&v));
            md.push_str(&format!("| {label} | {} | {} |\n", s.cell(2), s.range(2)));
        }
        let per100: Vec<f64> = self
            .runs
            .iter()
            .filter(|r| r.games.iter().map(|g| g.pa).sum::<u32>() > 0)
            .map(|r| {
                let e: u32 = r.games.iter().map(|g| g.err_us + g.err_them).sum();
                let pa: u32 = r.games.iter().map(|g| g.pa).sum();
                e as f64 * 100.0 / pa as f64
            })
            .collect();
        let s = sum.put("errors_per_100_pa", spread(&per100));
        md.push_str(&format!(
            "| エラー / 100打席 | {} | {} |\n",
            s.cell(2),
            s.range(2)
        ));

        // ---- landings
        let ring = DiamondDaysSpec::default().ring;
        let mut labels: Vec<String> = Vec::new();
        for sq in &ring {
            let l = sq.label(false).to_string();
            if !labels.contains(&l) {
                labels.push(l);
            }
        }
        let landed = |r: &SeedRun, l: &str| r.landings.iter().filter(|(_, x)| x == l).count();
        let total_landings: usize = self.runs.iter().map(|r| r.landings.len()).sum();
        md.push_str(&format!(
            "\n## すごろくマスの着地\n\n着地は 1 年あたり平均 {:.1} 回。期待割合は盤面（{} マス）の構成から（カードの出目は 1〜6 の一様で、長く回せば盤面全体に一様に着地する）。\n\n| マス | 1年あたり | 割合 % | 期待 % | 実際/期待 |\n|---|---|---|---|---|\n",
            total_landings as f64 / self.runs.len().max(1) as f64 / years,
            ring.len(),
        ));
        for l in &labels {
            let in_ring = ring.iter().filter(|s| s.label(false) == l).count();
            let expect = pct(in_ring, ring.len());
            // The game square is told apart by season below its total.
            let count = |r: &SeedRun| {
                if l == "Game" {
                    landed(r, "Game") + landed(r, "Game (winter)")
                } else {
                    landed(r, l)
                }
            };
            let per = spread(&self.per_year(|r| count(r) as f64));
            sum.put(format!("landings_per_year.{l}"), per);
            let n: usize = self.runs.iter().map(&count).sum();
            let share = pct(n, total_landings);
            sum.put(
                format!("landing_share.{l}"),
                spread(
                    &self
                        .runs
                        .iter()
                        .map(|r| pct(count(r), r.landings.len()))
                        .collect::<Vec<_>>(),
                ),
            );
            md.push_str(&format!(
                "| {l} | {} | {share:.1} | {expect:.1} | {:.2} |\n",
                per.cell(2),
                if expect > 0.0 { share / expect } else { 0.0 }
            ));
            if l == "Game" {
                for part in ["Game", "Game (winter)"] {
                    let per = spread(&self.per_year(|r| landed(r, part) as f64));
                    sum.put(format!("landings_per_year.{part}"), per);
                    let n: usize = self.runs.iter().map(|r| landed(r, part)).sum();
                    let name = if part == "Game" {
                        "　うち 試合のある時期（練習試合）"
                    } else {
                        "　うち 冬（紅白戦・試合なし）"
                    };
                    md.push_str(&format!(
                        "| {name} | {} | {:.1} | | |\n",
                        per.cell(2),
                        pct(n, total_landings)
                    ));
                }
            }
        }

        // ---- growth
        md.push_str(
            "\n## 選手の成長（入部時 → 卒業時）\n\n入部後に初めてチームで見た週の能力と、最後に見た週の能力。最初からいた選手は入部時が分からないので除く。伸びは「選手ごとの伸びの平均」のシード間の平均。投手の能力平均は球速を 0〜100 に直したもの（選手ページのゲージと同じ）と他の 3 つの能力の平均。\n\n| 役割 | 卒業生 | 入部時の平均 | 卒業時の平均 | 伸び（シード間） |\n|---|---|---|---|---|\n",
        );
        for pitcher in [false, true] {
            let role = if pitcher { "pitcher" } else { "fielder" };
            let grads = |r: &SeedRun| -> Vec<Grad> {
                r.grads
                    .iter()
                    .filter(|g| g.pitcher == pitcher)
                    .cloned()
                    .collect()
            };
            let n: usize = self.runs.iter().map(|r| grads(r).len()).sum();
            let mean_of = |r: &SeedRun, f: &dyn Fn(&Grad) -> f64| -> Option<f64> {
                let v = grads(r);
                (!v.is_empty()).then(|| v.iter().map(f).sum::<f64>() / v.len() as f64)
            };
            let joined: Vec<f64> = self
                .runs
                .iter()
                .filter_map(|r| mean_of(r, &|g| level(pitcher, &g.join)))
                .collect();
            let exited: Vec<f64> = self
                .runs
                .iter()
                .filter_map(|r| mean_of(r, &|g| level(pitcher, &g.exit)))
                .collect();
            let gain: Vec<f64> = self
                .runs
                .iter()
                .filter_map(|r| mean_of(r, &|g| level(pitcher, &g.exit) - level(pitcher, &g.join)))
                .collect();
            let (a, b, c) = (
                sum.put(format!("growth.{role}.join_level"), spread(&joined)),
                sum.put(format!("growth.{role}.exit_level"), spread(&exited)),
                sum.put(format!("growth.{role}.gain_level"), spread(&gain)),
            );
            sum.put(
                format!("growth.{role}.graduates"),
                spread(
                    &self
                        .runs
                        .iter()
                        .map(|r| grads(r).len() as f64)
                        .collect::<Vec<_>>(),
                ),
            );
            md.push_str(&format!(
                "| {} | {n} | {:.1} | {:.1} | {:+.1} ± {:.1} |\n",
                if pitcher { "投手" } else { "野手" },
                a.mean,
                b.mean,
                c.mean,
                c.sd
            ));
        }
        md.push_str("\n能力ごとの伸び（選手ごとの伸びの平均、全シードまとめ）:\n\n| 役割 | 能力 | 入部時 | 卒業時 | 伸び |\n|---|---|---|---|---|\n");
        for pitcher in [false, true] {
            let role = if pitcher { "pitcher" } else { "fielder" };
            let all: Vec<&Grad> = self
                .runs
                .iter()
                .flat_map(|r| r.grads.iter())
                .filter(|g| g.pitcher == pitcher)
                .collect();
            if all.is_empty() {
                continue;
            }
            for (i, st) in role_stats(pitcher).iter().enumerate() {
                let m = |f: &dyn Fn(&Grad) -> f64| {
                    all.iter().map(|g| f(g)).sum::<f64>() / all.len() as f64
                };
                let (j, e) = (m(&|g| g.join[i]), m(&|g| g.exit[i]));
                let unit = if *st == Stat::Velo { " km/h" } else { "" };
                sum.put(
                    format!("growth.{role}.{}", st.label(false)),
                    spread(&[e - j]),
                );
                md.push_str(&format!(
                    "| {} | {}{unit} | {j:.1} | {e:.1} | {:+.1} |\n",
                    if pitcher { "投手" } else { "野手" },
                    st.label(true),
                    e - j
                ));
            }
        }

        // ---- reputation, and the year-by-year trend
        md.push_str("\n## 学校の名声（0〜100）と年ごとの変化\n\n| 時点 | 名声 平均 ± SD | 名声 最小–最大 | 試合数 | 勝率 % | 卒業生 | プロ入り |\n|---|---|---|---|---|---|---|\n");
        for y in 0..=self.years as usize {
            let rep = sum.put(
                format!("reputation.year_{y:02}"),
                spread(&self.runs.iter().map(|r| r.rep[y]).collect::<Vec<_>>()),
            );
            let label = if y == 0 {
                "開始時".to_string()
            } else {
                format!("{y}年目の終わり")
            };
            if y == 0 {
                md.push_str(&format!(
                    "| {label} | {} | {} | | | | |\n",
                    rep.cell(1),
                    rep.range(1)
                ));
                continue;
            }
            let year = y as u32;
            let n = sum.put(
                format!("games_by_year.year_{y:02}"),
                spread(
                    &self
                        .runs
                        .iter()
                        .map(|r| r.games.iter().filter(|g| g.year == year).count() as f64)
                        .collect::<Vec<_>>(),
                ),
            );
            let win = sum.put(
                format!("win_rate_by_year.year_{y:02}"),
                spread(
                    &self
                        .runs
                        .iter()
                        .filter_map(|r| Self::win_rate(r, |g| g.year == year))
                        .collect::<Vec<_>>(),
                ),
            );
            let pros = sum.put(
                format!("pros_by_year.year_{y:02}"),
                spread(
                    &self
                        .runs
                        .iter()
                        .map(|r| r.pros[y - 1] as f64)
                        .collect::<Vec<_>>(),
                ),
            );
            let left = sum.put(
                format!("graduates_by_year.year_{y:02}"),
                spread(
                    &self
                        .runs
                        .iter()
                        .map(|r| r.left_by_year[y - 1] as f64)
                        .collect::<Vec<_>>(),
                ),
            );
            md.push_str(&format!(
                "| {label} | {} | {} | {} | {} | {} | {} |\n",
                rep.cell(1),
                rep.range(1),
                n.cell(1),
                win.cell(1),
                left.cell(1),
                pros.cell(1),
            ));
        }

        // ---- pros
        md.push_str("\n## 卒業生のプロ入り\n\n| 範囲 | 人数 | シード間 |\n|---|---|---|\n");
        let total = sum.put(
            "pros.total",
            spread(
                &self
                    .runs
                    .iter()
                    .map(|r| r.pros.iter().sum::<u32>() as f64)
                    .collect::<Vec<_>>(),
            ),
        );
        md.push_str(&format!(
            "| {}年間の合計 | {} | {} |\n",
            self.years,
            total.cell(1),
            total.range(0)
        ));
        let by_year = sum.put(
            "pros.per_year",
            spread(&self.per_year(|r| r.pros.iter().sum::<u32>() as f64)),
        );
        md.push_str(&format!(
            "| 1年あたり | {} | {} |\n",
            by_year.cell(2),
            by_year.range(2)
        ));

        // ---- best results
        md.push_str("\n## 年ごとの最高成績\n\n| 成績 | 年数 | 割合 % |\n|---|---|---|\n");
        let years_all: usize = self.runs.iter().map(|r| r.best.len()).sum();
        for (tier, name) in [
            (0u8, "全国優勝"),
            (1, "全国大会・春の招待の出場"),
            (2, "県優勝（夏・秋）"),
            (3, "それ以外"),
        ] {
            let n: usize = self
                .runs
                .iter()
                .map(|r| r.best.iter().filter(|b| **b == tier).count())
                .sum();
            sum.put(
                format!("best.tier_{tier}"),
                spread(
                    &self
                        .runs
                        .iter()
                        .map(|r| pct(r.best.iter().filter(|b| **b == tier).count(), r.best.len()))
                        .collect::<Vec<_>>(),
                ),
            );
            md.push_str(&format!("| {name} | {n} | {:.1} |\n", pct(n, years_all)));
        }
        md
    }
}

// ---- tests

fn env_num(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_on(key: &str) -> bool {
    std::env::var(key).is_ok_and(|v| v == "1")
}

/// The short run CI does: the harness works and repeats. Never judges the balance.
#[test]
fn autoplay_smoke_works_and_repeats() {
    let once = || {
        Report::new(
            vec![run_seed(1, 2, Policy::default())],
            2,
            Policy::default(),
        )
    };
    let (a, b) = (once(), once());
    assert_eq!(
        a.json().to_string(),
        b.json().to_string(),
        "the same seed plays out the same way"
    );
    assert!(a.problems().is_empty(), "{:?}", a.problems());
    let r = &a.runs[0];
    assert!(!r.games.is_empty(), "two years hold games");
    assert!(!r.landings.is_empty(), "the coach walked");
    assert_eq!(r.rep.len(), 3);
    assert!(a.markdown().contains("年間試合数"));
    eprintln!("{}", a.markdown());
}

/// The full run: `cargo test -p kagra-shared --lib autoplay_baseline -- --ignored --nocapture`.
#[test]
#[ignore = "the full run takes minutes; start it by hand"]
fn autoplay_baseline() {
    let seeds = env_num("KAGRA_AUTOPLAY_SEEDS", 5);
    let years = env_num("KAGRA_AUTOPLAY_YEARS", 10);
    let policy = Policy {
        spend_weekly: env_on("KAGRA_AUTOPLAY_SPEND"),
        gacha: env_on("KAGRA_AUTOPLAY_GACHA"),
    };
    let started = std::time::Instant::now();
    let runs: Vec<SeedRun> = (1..=seeds)
        .map(|seed| {
            let t = std::time::Instant::now();
            let r = run_seed(seed, years, policy);
            eprintln!(
                "seed {seed}: {} games, {} steps, {:.1}s",
                r.games.len(),
                r.steps,
                t.elapsed().as_secs_f32()
            );
            r
        })
        .collect();
    let report = Report::new(runs, years, policy);
    let dir = std::env::var("KAGRA_AUTOPLAY_OUT")
        .unwrap_or_else(|_| format!("{}/../target/autoplay", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).unwrap();
    let md = report.markdown();
    std::fs::write(format!("{dir}/autoplay.md"), &md).unwrap();
    std::fs::write(
        format!("{dir}/autoplay.json"),
        serde_json::to_string_pretty(&report.json()).unwrap(),
    )
    .unwrap();
    eprintln!("{md}");
    eprintln!(
        "{seeds} seeds x {years} years in {:.1}s, written to {dir}",
        started.elapsed().as_secs_f32()
    );
}
