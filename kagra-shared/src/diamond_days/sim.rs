//! A game, plate appearance by plate appearance, from the players' numbers. Each line of
//! play-by-play is kept in both languages. No picture, no input.

use serde::{Deserialize, Serialize};

use super::team::{Player, Team};
use crate::game_core::swarm::Rng;

/// The coach's order for an inning at bat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Order {
    /// Swing hard: more power, more strikeouts.
    Swing,
    Normal,
    /// Bunts and steals move runners.
    Small,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Batter {
    pub name: String,
    pub contact: f32,
    pub power: f32,
    pub speed: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arm {
    pub name: String,
    pub velo: f32,
    pub control: f32,
    pub breaking: f32,
    /// Pitches before he tires.
    pub endurance: f32,
}

impl Arm {
    fn stuff(&self, thrown: u32) -> f32 {
        let tired = (thrown as f32 - self.endurance).max(0.0) * 0.5;
        (self.velo - 115.0) * 1.3 + self.control * 0.45 + self.breaking * 0.45 - tired
    }
}

/// One team in a game: nine batters in order, pitchers in the order they come in, fielding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Side {
    pub name: String,
    pub name_ja: String,
    pub bats: Vec<Batter>,
    pub arms: Vec<Arm>,
    pub field: f32,
}

impl Side {
    /// Our club as it stands this week (condition counts).
    pub fn from_team(t: &Team, name: &str, name_ja: &str) -> Self {
        let ace = t.ace().unwrap_or(0);
        let order = t.lineup(ace);
        let m = |p: &Player| p.mood.factor().sqrt();
        let bats = order
            .iter()
            .map(|&i| {
                let p = &t.players[i];
                Batter {
                    name: p.family.clone(),
                    contact: p.contact as f32 * m(p),
                    power: p.power as f32 * m(p),
                    speed: p.speed as f32,
                }
            })
            .collect();
        let mut arms: Vec<(usize, &Player)> = t
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.pitcher() && p.can_play())
            .collect();
        arms.sort_by(|a, b| {
            let first = |i: usize| if i == ace { 1.0 } else { 0.0 };
            (first(b.0) * 1000.0 + b.1.pitch_rating())
                .total_cmp(&(first(a.0) * 1000.0 + a.1.pitch_rating()))
        });
        let arms = arms
            .into_iter()
            .map(|(_, p)| Arm {
                name: p.family.clone(),
                velo: p.velo as f32,
                control: p.control as f32 * m(p),
                breaking: p.breaking as f32 * m(p),
                endurance: 40.0 + p.stamina_p as f32 * 0.9 * (p.stamina as f32 / 100.0).max(0.4),
            })
            .collect();
        let field = order
            .iter()
            .map(|&i| t.players[i].field as f32)
            .sum::<f32>()
            / order.len().max(1) as f32;
        Self {
            name: name.into(),
            name_ja: name_ja.into(),
            bats,
            arms,
            field,
        }
    }

    /// A rival school of strength `level` (30 weak, 70 strong).
    pub fn rival(rng: &mut Rng, name: &str, name_ja: &str, level: f32) -> Self {
        let mut t = Team::default();
        for pos in super::team::Pos::FIELD.iter() {
            t.players.push(super::team::rookie(rng, 2, *pos, level));
        }
        t.players.push(super::team::rookie(
            rng,
            2,
            super::team::Pos::P,
            level - 6.0,
        ));
        t.unique_names();
        Self::from_team(&t, name, name_ja)
    }
}

/// A line of play-by-play.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Play {
    pub en: String,
    pub ja: String,
    /// Runs it scored.
    pub runs: u8,
}

#[derive(Clone, Debug)]
pub struct Game {
    /// [visitors, home].
    pub sides: [Side; 2],
    /// Which side is ours (0: we bat first).
    pub us: usize,
    /// Innings so far (1-based), and whether it is the top.
    pub inning: u8,
    pub top: bool,
    pub outs: u8,
    /// Runners: the batter index on first, second, third.
    pub bases: [Option<usize>; 3],
    /// Runs per inning per side.
    pub line: [Vec<u8>; 2],
    pub hits: [u16; 2],
    up: [usize; 2],
    arm: [usize; 2],
    thrown: [u32; 2],
    pub order: Order,
    pub over: bool,
    pub log: Vec<Play>,
    rng: Rng,
}

impl Game {
    pub fn new(us_side: Side, them: Side, we_bat_first: bool, seed: u32) -> Self {
        let (us, sides) = if we_bat_first {
            (0, [us_side, them])
        } else {
            (1, [them, us_side])
        };
        Self {
            sides,
            us,
            inning: 1,
            top: true,
            outs: 0,
            bases: [None; 3],
            line: [vec![0], Vec::new()],
            hits: [0; 2],
            up: [0; 2],
            arm: [0; 2],
            thrown: [0; 2],
            order: Order::Normal,
            over: false,
            log: Vec::new(),
            rng: Rng::new(seed),
        }
    }

    /// The side at bat now (0 visitors, 1 home).
    pub fn batting(&self) -> usize {
        if self.top {
            0
        } else {
            1
        }
    }

    pub fn runs(&self, side: usize) -> u32 {
        self.line[side].iter().map(|&r| r as u32).sum()
    }

    /// Is it our turn at bat (the coach picks an order)?
    pub fn we_bat(&self) -> bool {
        self.batting() == self.us
    }

    pub fn won(&self) -> Option<bool> {
        self.over
            .then(|| self.runs(self.us) > self.runs(1 - self.us))
    }

    pub fn batter_now(&self) -> &Batter {
        let s = self.batting();
        &self.sides[s].bats[self.up[s] % self.sides[s].bats.len()]
    }

    pub fn pitcher_now(&self) -> &Arm {
        let f = 1 - self.batting();
        let arms = &self.sides[f].arms;
        &arms[self.arm[f].min(arms.len().saturating_sub(1))]
    }

    fn say(&mut self, en: String, ja: String, runs: u8) {
        self.log.push(Play { en, ja, runs });
    }

    /// Play one plate appearance (or the change of sides). Returns false when the game is
    /// over.
    pub fn step(&mut self) -> bool {
        if self.over {
            return false;
        }
        let s = self.batting();
        let f = 1 - s;
        // A tired pitcher makes way.
        let arm_now = self.pitcher_now().clone();
        if self.thrown[f] as f32 > arm_now.endurance + 25.0
            && self.arm[f] + 1 < self.sides[f].arms.len()
        {
            self.arm[f] += 1;
            self.thrown[f] = 0;
            let n = self.pitcher_now().name.clone();
            self.say(
                format!("Pitching change: {n} comes in."),
                format!("ピッチャー交代、{n}がマウンドへ。"),
                0,
            );
            return true;
        }
        let bi = self.up[s] % self.sides[s].bats.len();
        let b = self.sides[s].bats[bi].clone();
        let arm = self.pitcher_now().clone();
        let stuff = arm.stuff(self.thrown[f]);
        let field = self.sides[f].field;
        let order = if s == self.us {
            self.order
        } else {
            Order::Normal
        };
        self.thrown[f] += 3 + (self.rng.unit() * 4.0) as u32;
        self.up[s] += 1;
        let name = b.name.clone();
        let r = self.rng.unit();
        // Small ball: a bunt with a runner on and fewer than two out.
        if order == Order::Small
            && self.outs < 2
            && self.bases[0].is_some()
            && self.bases[2].is_none()
        {
            if r < 0.78 {
                self.outs += 1;
                let runs = 0;
                if let Some(x) = self.bases[1].take() {
                    self.bases[2] = Some(x);
                }
                if let Some(x) = self.bases[0].take() {
                    self.bases[1] = Some(x);
                }
                self.say(
                    format!("{name} bunts. The runner moves up."),
                    format!("{name}、送りバント成功。ランナー進塁。"),
                    runs,
                );
            } else {
                self.outs += 1;
                self.say(
                    format!("{name} bunts it up. Caught."),
                    format!("{name}、バント失敗。小フライでアウト。"),
                    0,
                );
            }
            return self.after_out(s);
        }
        let (k_adj, power_adj) = match order {
            Order::Swing => (0.06, 1.25),
            Order::Normal => (0.0, 1.0),
            Order::Small => (-0.04, 0.7),
        };
        let k = (0.13 + (stuff - b.contact) * 0.0035 + k_adj).clamp(0.04, 0.42);
        let bb = (0.10 - arm.control * 0.0007).clamp(0.02, 0.11);
        let hit =
            (0.28 + (b.contact - stuff * 0.8) * 0.0035 - (field - 50.0) * 0.0012).clamp(0.12, 0.5);
        if r < k {
            self.outs += 1;
            self.say(
                format!("{name} strikes out."),
                format!("{name}、空振り三振！"),
                0,
            );
            return self.after_out(s);
        }
        if r < k + bb {
            let mut runs = 0;
            self.walk(s, bi, &mut runs);
            self.say(
                format!("{name} walks."),
                format!("{name}、フォアボール。"),
                runs,
            );
            return self.after_play(s);
        }
        let in_play = (r - k - bb) / (1.0 - k - bb);
        if in_play < hit {
            self.hits[s] += 1;
            let p = self.rng.unit();
            let hr = ((b.power * power_adj - 35.0) * 0.004).clamp(0.005, 0.22);
            let triple = (b.speed - 50.0).max(0.0) * 0.0015 + 0.01;
            let double = 0.12 + (b.power * power_adj - 40.0).max(0.0) * 0.003;
            let mut runs = 0;
            let (bases, en, ja) = if p < hr {
                (
                    4,
                    format!("{name} homers!"),
                    format!("{name}、ホームラン！"),
                )
            } else if p < hr + triple {
                (
                    3,
                    format!("{name} triples."),
                    format!("{name}、スリーベース！"),
                )
            } else if p < hr + triple + double {
                (
                    2,
                    format!("{name} doubles."),
                    format!("{name}、ツーベースヒット。"),
                )
            } else {
                (1, format!("{name} singles."), format!("{name}、ヒット。"))
            };
            self.advance(s, bi, bases, &mut runs);
            let (en, ja) = if runs > 0 && bases < 4 {
                (
                    format!("{en} {runs} run{} in.", if runs > 1 { "s" } else { "" }),
                    format!("{ja}{runs}点が入る！"),
                )
            } else if runs > 1 {
                (format!("{en} {runs} runs."), format!("{ja}{runs}点！"))
            } else {
                (en, ja)
            };
            self.say(en, ja, runs);
            // Small ball: a fast runner on first tries to steal.
            if order == Order::Small {
                self.try_steal(s);
            }
            return self.after_play(s);
        }
        // An out in play.
        let ground = self.rng.unit() < 0.5;
        if ground && self.bases[0].is_some() && self.outs < 2 && self.rng.unit() < 0.3 {
            self.outs += 2;
            self.bases[0] = None;
            self.say(
                format!("{name} grounds into a double play."),
                format!("{name}、ゲッツー。"),
                0,
            );
            return self.after_out(s);
        }
        self.outs += 1;
        // A fly with a runner on third and fewer than two out scores him.
        if !ground && self.outs < 3 && self.bases[2].is_some() && self.rng.unit() < 0.6 {
            self.bases[2] = None;
            self.add_runs(s, 1);
            self.say(
                format!("{name} flies out; the runner tags up and scores."),
                format!("{name}、犠牲フライ！1点。"),
                1,
            );
        } else {
            let (en, ja) = if ground {
                (
                    format!("{name} grounds out."),
                    format!("{name}、内野ゴロ。"),
                )
            } else {
                (
                    format!("{name} flies out."),
                    format!("{name}、外野フライ。"),
                )
            };
            self.say(en, ja, 0);
        }
        self.after_out(s)
    }

    fn add_runs(&mut self, s: usize, n: u8) {
        if let Some(last) = self.line[s].last_mut() {
            *last += n;
        }
    }

    fn walk(&mut self, s: usize, bi: usize, runs: &mut u8) {
        if self.bases[0].is_some() {
            if self.bases[1].is_some() {
                if self.bases[2].is_some() {
                    *runs += 1;
                }
                self.bases[2] = self.bases[1];
            }
            self.bases[1] = self.bases[0];
        }
        self.bases[0] = Some(bi);
        self.add_runs(s, *runs);
    }

    fn advance(&mut self, s: usize, bi: usize, by: usize, runs: &mut u8) {
        let speed = |g: &Game, i: usize| g.sides[s].bats[i].speed;
        let mut next: [Option<usize>; 3] = [None; 3];
        for base in (0..3).rev() {
            let Some(r) = self.bases[base] else {
                continue;
            };
            // A single scores a runner from second if he is quick; extra bases otherwise.
            let mut to = base + by;
            if by == 1 && base == 1 && self.rng.unit() < 0.35 + speed(self, r) * 0.006 {
                to = 3;
            }
            if by == 2 && base == 0 && self.rng.unit() < 0.25 + speed(self, r) * 0.006 {
                to = 3;
            }
            if to >= 3 {
                *runs += 1;
            } else {
                let mut t = to;
                while next[t].is_some() && t < 2 {
                    t += 1;
                }
                next[t] = Some(r);
            }
        }
        if by >= 4 {
            *runs += 1;
        } else {
            next[by - 1] = Some(bi);
        }
        self.bases = next;
        self.add_runs(s, *runs);
    }

    fn try_steal(&mut self, s: usize) {
        if let (Some(r), None) = (self.bases[0], self.bases[1]) {
            let sp = self.sides[s].bats[r].speed;
            if sp > 55.0 {
                let name = self.sides[s].bats[r].name.clone();
                if self.rng.unit() < 0.45 + (sp - 55.0) * 0.012 {
                    self.bases[1] = self.bases[0].take();
                    self.say(
                        format!("{name} steals second!"),
                        format!("{name}、盗塁成功！"),
                        0,
                    );
                } else {
                    self.bases[0] = None;
                    self.outs += 1;
                    self.say(
                        format!("{name} is caught stealing."),
                        format!("{name}、盗塁失敗。"),
                        0,
                    );
                }
            }
        }
    }

    /// After a hit or a walk: a walk-off ends it.
    fn after_play(&mut self, s: usize) -> bool {
        if s == 1 && self.inning >= 9 && self.runs(1) > self.runs(0) {
            self.over = true;
            self.say(
                "Walk-off! The game is over.".into(),
                "サヨナラ！試合終了。".into(),
                0,
            );
            return false;
        }
        if self.outs >= 3 {
            return self.after_out(s);
        }
        true
    }

    fn after_out(&mut self, s: usize) -> bool {
        if s == 1 && self.inning >= 9 && self.runs(1) > self.runs(0) {
            self.over = true;
            self.say(
                "Walk-off! The game is over.".into(),
                "サヨナラ！試合終了。".into(),
                0,
            );
            return false;
        }
        if self.outs < 3 {
            return true;
        }
        self.outs = 0;
        self.bases = [None; 3];
        if self.top {
            // Bottom of the ninth (or later) is not needed when the home side leads.
            if self.inning >= 9 && self.runs(1) > self.runs(0) {
                self.over = true;
                self.line[1].push(0);
                self.say("Game over.".into(), "試合終了。".into(), 0);
                return false;
            }
            self.top = false;
            self.line[1].push(0);
        } else {
            if self.inning >= 9 && self.runs(0) != self.runs(1) || self.inning >= 15 {
                self.over = true;
                // A tie after 15 goes to the visitors by a run of luck: no ties in a knockout.
                if self.runs(0) == self.runs(1) {
                    let w = if self.rng.unit() < 0.5 { 0 } else { 1 };
                    self.add_runs_last(w);
                    self.say(
                        "Tie-break: decided by a final run.".into(),
                        "タイブレークの末、決着。".into(),
                        1,
                    );
                }
                self.say("Game over.".into(), "試合終了。".into(), 0);
                return false;
            }
            self.inning += 1;
            self.top = true;
            self.line[0].push(0);
        }
        let (n, en_side) = (self.inning, if self.top { "Top" } else { "Bottom" });
        let ja_side = if self.top { "表" } else { "裏" };
        self.say(
            format!("{en_side} of inning {n}."),
            format!("{n}回{ja_side}。"),
            0,
        );
        true
    }

    fn add_runs_last(&mut self, side: usize) {
        if let Some(last) = self.line[side].last_mut() {
            *last += 1;
        }
    }

    /// Play to the end (tests, and the "skip" button).
    pub fn finish(&mut self) {
        let mut guard = 0;
        while self.step() && guard < 2000 {
            guard += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(level: f32, seed: u32) -> Side {
        Side::rival(&mut Rng::new(seed), "X", "X", level)
    }

    #[test]
    fn a_game_ends_with_a_winner_and_sensible_scores() {
        for seed in 0..40 {
            let mut g = Game::new(
                side(45.0, seed),
                side(45.0, seed + 100),
                seed % 2 == 0,
                seed,
            );
            g.finish();
            assert!(g.over);
            let (a, b) = (g.runs(0), g.runs(1));
            assert_ne!(a, b, "seed {seed}: no ties");
            assert!(a + b < 40, "seed {seed}: {a}-{b}");
            assert!(g.inning >= 9);
            assert_eq!(g.line[0].len(), g.inning as usize);
            assert!(!g.log.is_empty());
        }
    }

    #[test]
    fn the_stronger_side_wins_more_often() {
        let mut strong_wins = 0;
        for seed in 0..60 {
            let mut g = Game::new(side(70.0, seed), side(35.0, seed + 500), true, seed);
            g.finish();
            if g.won() == Some(true) {
                strong_wins += 1;
            }
        }
        assert!(strong_wins > 45, "{strong_wins} / 60");
    }

    #[test]
    fn swinging_away_hits_more_homers_and_small_ball_bunts() {
        let count = |order: Order, ja: &str| {
            let mut n = 0;
            for seed in 0..40 {
                let mut g = Game::new(side(55.0, seed), side(55.0, seed + 9), true, seed);
                g.order = order;
                g.finish();
                n += g.log.iter().filter(|p| p.ja.contains(ja)).count();
            }
            n
        };
        assert!(count(Order::Swing, "ホームラン") > count(Order::Small, "ホームラン"));
        assert!(count(Order::Small, "バント") > 20);
        assert_eq!(count(Order::Normal, "バント"), 0);
    }
}
