//! A game, plate appearance by plate appearance, from the players' numbers. Each line of
//! play-by-play is kept in both languages. No picture, no input.

use serde::{Deserialize, Serialize};

use super::team::{
    Ability, Player, Pos, Team, BIT_CHANCE, BIT_CLUTCH, BIT_EYE, BIT_HEAVY, BIT_PINCH, BIT_STEAL,
};
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
    pub field: f32,
    /// [`Ability`] bits.
    #[serde(default)]
    pub abil: u8,
    #[serde(default)]
    pub pitcher: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arm {
    pub name: String,
    pub velo: f32,
    pub control: f32,
    pub breaking: f32,
    /// Pitches before he tires.
    pub endurance: f32,
    #[serde(default)]
    pub abil: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Glove {
    pub name: String,
    pub field: f32,
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
    /// Players who can come off the bench this game.
    #[serde(default)]
    pub bench: Vec<Batter>,
    /// One rating per fielder, so a defensive sub can change the average.
    #[serde(default)]
    pub gloves: Vec<Glove>,
    /// A mood-maker in the order, named once at the first pitch.
    #[serde(default)]
    pub spark: Option<String>,
}

impl Side {
    /// Our club as it stands this week (condition counts). The coach's card is used when
    /// it is still a legal nine; otherwise the automatic order.
    pub fn from_team(t: &Team, name: &str, name_ja: &str) -> Self {
        let nine = t.resolved();
        let mound = nine.iter().find(|(_, pos)| *pos == Pos::P).map(|(i, _)| *i);
        let m = |p: &Player| p.mood.factor().sqrt();
        let lifted = nine.iter().any(|(i, _)| t.players[*i].has(Ability::Mood));
        let to_batter = |p: &Player, pos: Option<Pos>| {
            let boost = if lifted && !p.has(Ability::Mood) {
                3.0
            } else if lifted {
                1.0
            } else {
                0.0
            };
            let field = match pos {
                Some(spot) if spot != p.pos && !(spot == Pos::P && p.pitcher()) => {
                    p.field as f32 * 0.9
                }
                _ => p.field as f32,
            };
            Batter {
                name: p.family.clone(),
                contact: p.contact as f32 * m(p) + boost,
                power: p.power as f32 * m(p),
                speed: p.speed as f32,
                field,
                abil: p.bits(),
                pitcher: p.pitcher(),
            }
        };
        let bats: Vec<Batter> = nine
            .iter()
            .map(|(i, pos)| to_batter(&t.players[*i], Some(*pos)))
            .collect();
        let gloves: Vec<Glove> = bats
            .iter()
            .map(|b| Glove {
                name: b.name.clone(),
                field: b.field,
            })
            .collect();
        let field = if gloves.is_empty() {
            40.0
        } else {
            gloves.iter().map(|g| g.field).sum::<f32>() / gloves.len() as f32
        };
        let mut arms: Vec<(usize, &Player)> = t
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.pitcher() && p.can_play())
            .collect();
        arms.sort_by(|a, b| {
            let first = |i: usize| if Some(i) == mound { 1.0 } else { 0.0 };
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
                abil: p.bits(),
            })
            .collect();
        let playing: Vec<&str> = bats.iter().map(|b| b.name.as_str()).collect();
        let bench = t
            .players
            .iter()
            .filter(|p| p.can_play() && !playing.contains(&p.family.as_str()))
            .map(|p| to_batter(p, None))
            .collect();
        let spark = nine.iter().find_map(|(i, _)| {
            let p = &t.players[*i];
            p.has(Ability::Mood).then(|| p.family.clone())
        });
        Self {
            name: name.into(),
            name_ja: name_ja.into(),
            bats,
            arms,
            field,
            bench,
            gloves,
            spark,
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

/// A moment the coach is asked about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    /// We bat with a runner in scoring position.
    Chance,
    /// They bat with a runner in scoring position.
    Pinch,
    /// Pick someone off our bench: 0 pinch-hit, 1 pinch-run, 2 defensive sub.
    Bench(u8),
}

/// The coach's call for one plate appearance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tactic {
    /// Chance: swing away.
    Hit,
    /// Chance: make him throw strikes (walks up, hits down).
    Wait,
    /// Chance: sacrifice bunt.
    Bunt,
    /// Chance: the runner on third breaks for home on the bunt.
    Squeeze,
    /// Chance: the lead runner steals.
    Steal,
    /// Pinch: go right at him (the pitcher bears down).
    Challenge,
    /// Pinch: walk him on purpose.
    Walk,
    /// Pinch: bring in the next pitcher.
    Relief,
}

impl Tactic {
    pub const CHANCE: [Tactic; 5] = [
        Tactic::Hit,
        Tactic::Wait,
        Tactic::Bunt,
        Tactic::Squeeze,
        Tactic::Steal,
    ];
    pub const PINCH: [Tactic; 3] = [Tactic::Challenge, Tactic::Walk, Tactic::Relief];

    pub fn label(self, ja: bool) -> &'static str {
        match (self, ja) {
            (Tactic::Hit, false) => "SWING",
            (Tactic::Wait, false) => "TAKE",
            (Tactic::Bunt, false) => "BUNT",
            (Tactic::Squeeze, false) => "SQUEEZE",
            (Tactic::Steal, false) => "STEAL",
            (Tactic::Challenge, false) => "ATTACK",
            (Tactic::Walk, false) => "WALK HIM",
            (Tactic::Relief, false) => "RELIEF",
            (Tactic::Hit, true) => "打て",
            (Tactic::Wait, true) => "待て",
            (Tactic::Bunt, true) => "バント",
            (Tactic::Squeeze, true) => "スクイズ",
            (Tactic::Steal, true) => "盗塁",
            (Tactic::Challenge, true) => "勝負",
            (Tactic::Walk, true) => "敬遠",
            (Tactic::Relief, true) => "継投",
        }
    }

    /// Can it be called now?
    pub fn possible(self, g: &Game) -> bool {
        let f = 1 - g.batting();
        match self {
            // A sacrifice moves runners up from first and second; with a runner on third,
            // the squeeze is the bunt.
            Tactic::Bunt => {
                g.outs < 2 && (g.bases[0].is_some() || g.bases[1].is_some()) && g.bases[2].is_none()
            }
            Tactic::Squeeze => g.outs < 2 && g.bases[2].is_some(),
            Tactic::Steal => {
                (g.bases[0].is_some() && g.bases[1].is_none())
                    || (g.bases[1].is_some() && g.bases[2].is_none())
            }
            Tactic::Relief => g.arm[f] + 1 < g.sides[f].arms.len(),
            _ => true,
        }
    }
}

/// A line of play-by-play.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Play {
    pub en: String,
    pub ja: String,
    /// Runs it scored.
    pub runs: u8,
    /// What happened on the field (for the picture).
    #[serde(default)]
    pub kind: PlayKind,
    /// Where the ball went: degrees from centre field, negative toward left field.
    #[serde(default)]
    pub dir: f32,
    /// An ability that fired on this play, drawn big on the field.
    #[serde(default)]
    pub banner: Option<Ability>,
    /// A chance, an extra-base hit, or a walk-off: the picture holds longer.
    #[serde(default)]
    pub clutch: bool,
    /// The plate appearance before its result, for the pitch-by-pitch picture: who bats
    /// (0 visitors, 1 home), outs and runners (bit 0 first base), batter, pitcher, top speed.
    #[serde(default)]
    pub side: u8,
    #[serde(default)]
    pub outs0: u8,
    #[serde(default)]
    pub bases0: u8,
    #[serde(default)]
    pub batter: String,
    #[serde(default)]
    pub pitcher: String,
    #[serde(default)]
    pub velo: f32,
    /// The inning of the plate appearance (the game may already be in the next half).
    #[serde(default)]
    pub inning: u8,
}

/// One pitch of a plate appearance, as shown (the result comes from the plate appearance).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pitch {
    Ball,
    /// A called strike.
    Looking,
    /// A swing and a miss.
    Swinging,
    Foul,
    /// Swung and hit into play: the play happens.
    InPlay,
}

impl Play {
    /// A line of words only (the game's title, an inning, a shout).
    pub fn note(en: impl Into<String>, ja: impl Into<String>) -> Self {
        Play {
            en: en.into(),
            ja: ja.into(),
            runs: 0,
            kind: PlayKind::Note,
            dir: 0.0,
            banner: None,
            clutch: false,
            side: 0,
            outs0: 0,
            bases0: 0,
            batter: String::new(),
            pitcher: String::new(),
            velo: 0.0,
            inning: 0,
        }
    }

    /// A plate appearance (or a steal during one), not a line of words or a pitching change.
    /// Words spoken mid-game still carry the batter of the moment, so check the kind.
    pub fn is_pa(&self) -> bool {
        !self.batter.is_empty() && !matches!(self.kind, PlayKind::Note | PlayKind::Change)
    }

    /// The pitches leading to this result, in order, with a count that never goes past
    /// three balls or two strikes before the last pitch. Drawn from a hash of the play, so
    /// the same game shows the same pitches and no result changes.
    pub fn pitches(&self) -> Vec<Pitch> {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in self
            .en
            .bytes()
            .chain(self.batter.bytes())
            .chain((self.dir.to_bits()).to_le_bytes())
        {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
        let mut next = move |n: u64| -> u64 {
            h ^= h >> 29;
            h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
            h ^= h >> 32;
            h % n.max(1)
        };
        let walked_on_purpose = self.kind == PlayKind::Walk && self.en.contains("on purpose");
        let (balls, strikes, last) = match self.kind {
            PlayKind::Note | PlayKind::Change => return Vec::new(),
            PlayKind::Steal | PlayKind::CaughtStealing => return vec![Pitch::Ball],
            PlayKind::Walk if walked_on_purpose => return vec![Pitch::Ball; 4],
            PlayKind::Walk => (3, next(3) as usize, Pitch::Ball),
            PlayKind::Strikeout => (next(4) as usize, 2, Pitch::Swinging),
            PlayKind::Bunt => (next(2) as usize, next(2) as usize, Pitch::InPlay),
            _ => (next(4) as usize, next(3) as usize, Pitch::InPlay),
        };
        // Strikes before the last pitch: called, missed or fouled off.
        let mut seq: Vec<Pitch> = (0..strikes)
            .map(|_| match next(3) {
                0 => Pitch::Looking,
                1 => Pitch::Swinging,
                _ => Pitch::Foul,
            })
            .chain(std::iter::repeat_n(Pitch::Ball, balls))
            .collect();
        // Shuffle (any order keeps the count legal: at most 3 balls and 2 strikes).
        for i in (1..seq.len()).rev() {
            let j = next(i as u64 + 1) as usize;
            seq.swap(i, j);
        }
        // With two strikes a batter may foul a few off.
        if strikes == 2 && last != Pitch::Ball {
            for _ in 0..next(3) {
                seq.push(Pitch::Foul);
            }
        }
        seq.push(last);
        seq
    }
}

/// What a play looks like on the field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayKind {
    /// Words only: an inning starts, the game ends.
    #[default]
    Note,
    /// A hit for this many bases (4 = a home run).
    Hit(u8),
    Strikeout,
    Walk,
    GroundOut,
    FlyOut,
    DoublePlay,
    SacFly,
    Bunt,
    Steal,
    CaughtStealing,
    /// A new pitcher.
    Change,
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
    /// The call for the next plate appearance, and the plate appearance last asked about.
    pub tactic: Option<Tactic>,
    asked: Option<(usize, usize)>,
    pub over: bool,
    pub log: Vec<Play>,
    rng: Rng,
    /// Set just before a line is spoken.
    mark_banner: Option<Ability>,
    mark_clutch: bool,
    /// The plate appearance as it starts (see [`Play::side`]).
    snap: (u8, u8, u8, String, String, f32, u8),
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
            tactic: None,
            asked: None,
            over: false,
            log: Vec::new(),
            rng: Rng::new(seed),
            mark_banner: None,
            mark_clutch: false,
            snap: (0, 0, 0, String::new(), String::new(), 0.0, 1),
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
        self.say_as(PlayKind::Note, en, ja, runs);
    }

    /// A play with its picture. The direction comes from a hash of the play, not the dice,
    /// so adding pictures changes no result.
    fn say_as(&mut self, kind: PlayKind, en: String, ja: String, runs: u8) {
        let banner = self.mark_banner.take();
        let clutch = self.mark_clutch || matches!(kind, PlayKind::Hit(n) if n >= 2);
        self.mark_clutch = false;
        let (en, ja) = match banner {
            Some(a) => (
                format!("{}! {en}", a.label(false)),
                format!("{}！{ja}", a.label(true)),
            ),
            None => (en, ja),
        };
        let mut h: u32 = 2_166_136_261;
        for b in en.bytes().chain([self.log.len() as u8, self.inning]) {
            h = (h ^ b as u32).wrapping_mul(16_777_619);
        }
        let dir = (h % 900) as f32 / 10.0 - 45.0;
        let snap = self.snap.clone();
        self.log.push(Play {
            en,
            ja,
            runs,
            kind,
            dir,
            banner,
            clutch,
            side: snap.0,
            outs0: snap.1,
            bases0: snap.2,
            batter: snap.3,
            pitcher: snap.4,
            velo: snap.5,
            inning: snap.6,
        });
    }

    fn risp(&self) -> bool {
        self.bases[1].is_some() || self.bases[2].is_some()
    }

    fn late_close(&self) -> bool {
        self.inning >= 7 && (self.runs(0) as i32 - self.runs(1) as i32).abs() <= 2
    }

    /// The trait to shout for our side. Rivals' traits still change the numbers.
    fn our_banner(&self, batting: usize, bat_abil: u8, arm_abil: u8) -> Option<Ability> {
        let our_bat = batting == self.us;
        if our_bat && self.risp() && bat_abil & BIT_CHANCE != 0 {
            return Some(Ability::Chance);
        }
        if our_bat && self.late_close() && bat_abil & BIT_CLUTCH != 0 {
            return Some(Ability::Clutch);
        }
        if !our_bat && self.risp() && arm_abil & BIT_PINCH != 0 {
            return Some(Ability::Pinch);
        }
        if !our_bat && self.late_close() && arm_abil & BIT_CLUTCH != 0 {
            return Some(Ability::Clutch);
        }
        None
    }

    /// A moment to ask the coach about, once per plate appearance: a runner on second or
    /// third, ours at bat (a chance) or theirs (a pinch).
    pub fn spot(&self) -> Option<Spot> {
        if self.over || self.outs >= 3 || (self.bases[1].is_none() && self.bases[2].is_none()) {
            return None;
        }
        let s = self.batting();
        if self.asked == Some((s, self.up[s])) {
            return None;
        }
        Some(if s == self.us {
            Spot::Chance
        } else {
            Spot::Pinch
        })
    }

    /// The coach already made a call for the plate appearance now.
    pub fn asked_now(&self) -> bool {
        let s = self.batting();
        self.asked == Some((s, self.up[s]))
    }

    /// The coach's call for the plate appearance now (None: let them play).
    pub fn call(&mut self, t: Option<Tactic>) {
        let s = self.batting();
        self.asked = Some((s, self.up[s]));
        self.tactic = t.filter(|t| t.possible(self));
    }

    /// Play one plate appearance (or the change of sides). Returns false when the game is
    /// over.
    pub fn step(&mut self) -> bool {
        if self.over {
            return false;
        }
        let s = self.batting();
        let bases = self
            .bases
            .iter()
            .enumerate()
            .fold(0u8, |m, (i, b)| m | ((b.is_some() as u8) << i));
        self.snap = (
            s as u8,
            self.outs,
            bases,
            self.batter_now().name.clone(),
            self.pitcher_now().name.clone(),
            self.pitcher_now().velo,
            self.inning,
        );
        let f = 1 - s;
        // A tired pitcher makes way.
        let arm_now = self.pitcher_now().clone();
        if self.thrown[f] as f32 > arm_now.endurance + 25.0
            && self.arm[f] + 1 < self.sides[f].arms.len()
        {
            self.arm[f] += 1;
            self.thrown[f] = 0;
            // The change the coach called for has happened: do not bring in another.
            if self.tactic == Some(Tactic::Relief) {
                self.tactic = None;
            }
            let n = self.pitcher_now().name.clone();
            self.say_as(
                PlayKind::Change,
                format!("Pitching change: {n} comes in."),
                format!("ピッチャー交代、{n}がマウンドへ。"),
                0,
            );
            return true;
        }
        let tactic = self.tactic.take();
        match tactic {
            Some(Tactic::Relief) if self.arm[f] + 1 < self.sides[f].arms.len() => {
                self.arm[f] += 1;
                self.thrown[f] = 0;
                let n = self.pitcher_now().name.clone();
                self.say_as(
                    PlayKind::Change,
                    format!("The coach goes to the bullpen: {n}."),
                    format!("継投、{n}がマウンドへ。"),
                    0,
                );
                // The batter still has to face him.
                let s2 = self.batting();
                self.asked = Some((s2, self.up[s2]));
                return true;
            }
            Some(Tactic::Steal) => {
                self.steal_lead(s);
                return self.after_play(s);
            }
            _ => {}
        }
        let bi = self.up[s] % self.sides[s].bats.len();
        let b = self.sides[s].bats[bi].clone();
        let arm = self.pitcher_now().clone();
        let mut stuff = arm.stuff(self.thrown[f])
            + if tactic == Some(Tactic::Challenge) {
                5.0
            } else {
                0.0
            };
        let mut contact = b.contact;
        let mut power = b.power;
        if self.risp() && b.abil & BIT_CHANCE != 0 {
            contact += 8.0;
            power += 6.0;
        }
        if self.late_close() && b.abil & BIT_CLUTCH != 0 {
            contact += 6.0;
            power += 4.0;
        }
        if self.risp() && arm.abil & BIT_PINCH != 0 {
            stuff += 8.0;
        }
        if self.late_close() && arm.abil & BIT_CLUTCH != 0 {
            stuff += 6.0;
        }
        if arm.abil & BIT_HEAVY != 0 {
            stuff += 7.0;
        }
        let situation = self.our_banner(s, b.abil, arm.abil);
        let our_bat = s == self.us;
        let field = self.sides[f].field;
        let order = match tactic {
            Some(Tactic::Hit) => Order::Swing,
            Some(Tactic::Bunt) => Order::Small,
            _ if s == self.us => self.order,
            _ => Order::Normal,
        };
        self.thrown[f] += 3 + (self.rng.unit() * 4.0) as u32;
        self.up[s] += 1;
        let name = b.name.clone();
        let r = self.rng.unit();
        // An intentional walk.
        if tactic == Some(Tactic::Walk) {
            let mut runs = 0;
            self.walk(s, bi, &mut runs);
            self.say_as(
                PlayKind::Walk,
                format!("{name} is walked on purpose."),
                format!("{name}を敬遠。"),
                runs,
            );
            return self.after_play(s);
        }
        // The squeeze: the runner on third breaks as the batter bunts.
        if tactic == Some(Tactic::Squeeze) && self.bases[2].is_some() {
            self.outs += 1;
            if r < 0.68 {
                self.mark_clutch = true;
                self.mark_banner = situation;
                self.bases[2] = None;
                if let Some(x) = self.bases[1].take() {
                    self.bases[2] = Some(x);
                }
                if let Some(x) = self.bases[0].take() {
                    self.bases[1] = Some(x);
                }
                self.add_runs(s, 1);
                self.say_as(
                    PlayKind::Bunt,
                    format!("Squeeze! {name} lays it down and the run scores."),
                    format!("スクイズ成功！{name}のバントで1点。"),
                    1,
                );
            } else {
                self.bases[2] = None;
                self.mark_banner = situation;
                self.say_as(
                    PlayKind::Bunt,
                    format!("Squeeze foiled: {name} misses and the runner is out at home."),
                    "スクイズ失敗、三塁ランナーが本塁でタッチアウト。".to_string(),
                    0,
                );
            }
            return self.after_out(s);
        }
        // Small ball: a bunt with a runner on and fewer than two out.
        if order == Order::Small
            && self.outs < 2
            && (self.bases[0].is_some() || self.bases[1].is_some())
            && self.bases[2].is_none()
        {
            if r < 0.78 {
                self.outs += 1;
                self.mark_banner = situation;
                let runs = 0;
                if let Some(x) = self.bases[1].take() {
                    self.bases[2] = Some(x);
                }
                if let Some(x) = self.bases[0].take() {
                    self.bases[1] = Some(x);
                }
                self.say_as(
                    PlayKind::Bunt,
                    format!("{name} bunts. The runner moves up."),
                    format!("{name}、送りバント成功。ランナー進塁。"),
                    runs,
                );
            } else {
                self.outs += 1;
                self.mark_banner = situation;
                self.say_as(
                    PlayKind::FlyOut,
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
        let wait = tactic == Some(Tactic::Wait);
        let eye = if b.abil & BIT_EYE != 0 { 1.8 } else { 1.0 };
        let k = (0.13 + (stuff - contact) * 0.0035 + k_adj + if wait { 0.03 } else { 0.0 })
            .clamp(0.04, 0.42);
        let bb =
            (0.10 - arm.control * 0.0007).clamp(0.02, 0.11) * if wait { 2.2 } else { 1.0 } * eye;
        let hit =
            (0.28 + (contact - stuff * 0.8) * 0.0035 - (field - 50.0) * 0.0012).clamp(0.12, 0.5);
        if r < k {
            self.outs += 1;
            self.mark_banner = situation.or(if !our_bat && arm.abil & BIT_HEAVY != 0 {
                Some(Ability::Heavy)
            } else {
                None
            });
            self.say_as(
                PlayKind::Strikeout,
                format!("{name} strikes out."),
                format!("{name}、空振り三振！"),
                0,
            );
            return self.after_out(s);
        }
        if r < k + bb {
            let mut runs = 0;
            self.walk(s, bi, &mut runs);
            self.mark_banner = situation.or(if our_bat && b.abil & BIT_EYE != 0 {
                Some(Ability::Eye)
            } else {
                None
            });
            self.say_as(
                PlayKind::Walk,
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
            let hr = ((power * power_adj - 35.0) * 0.004).clamp(0.005, 0.22);
            let triple = (b.speed - 50.0).max(0.0) * 0.0015 + 0.01;
            let double = 0.12 + (power * power_adj - 40.0).max(0.0) * 0.003;
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
            if self.risp() {
                self.mark_clutch = true;
            }
            self.advance(s, bi, bases, &mut runs);
            self.mark_banner = situation;
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
            self.say_as(PlayKind::Hit(bases as u8), en, ja, runs);
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
            self.mark_banner = situation;
            self.say_as(
                PlayKind::DoublePlay,
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
            self.mark_clutch = true;
            self.mark_banner = situation;
            self.say_as(
                PlayKind::SacFly,
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
            let kind = if ground {
                PlayKind::GroundOut
            } else {
                PlayKind::FlyOut
            };
            self.mark_banner = situation;
            self.say_as(kind, en, ja, 0);
        }
        self.after_out(s)
    }

    fn add_runs(&mut self, s: usize, n: u8) {
        if let Some(last) = self.line[s].last_mut() {
            *last = last.saturating_add(n);
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

    /// The lead runner with an open base ahead steals (a called steal).
    fn steal_lead(&mut self, s: usize) {
        let base = if self.bases[1].is_some() && self.bases[2].is_none() {
            1
        } else if self.bases[0].is_some() && self.bases[1].is_none() {
            0
        } else {
            return;
        };
        let r = self.bases[base].unwrap_or(0);
        let runner = &self.sides[s].bats[r];
        let sp = runner.speed;
        let steal_o = runner.abil & BIT_STEAL != 0;
        let name = runner.name.clone();
        let (to_en, to_ja) = if base == 1 {
            ("third", "三盗")
        } else {
            ("second", "盗塁")
        };
        if s == self.us && steal_o {
            self.mark_banner = Some(Ability::Steal);
        }
        if self.rng.unit() < 0.35 + sp * 0.006 + if steal_o { 0.16 } else { 0.0 } {
            self.bases[base + 1] = self.bases[base].take();
            self.say_as(
                PlayKind::Steal,
                format!("{name} steals {to_en}!"),
                format!("{name}、{to_ja}成功！"),
                0,
            );
        } else {
            self.bases[base] = None;
            self.outs += 1;
            self.say_as(
                PlayKind::CaughtStealing,
                format!("{name} is thrown out stealing."),
                format!("{name}、{to_ja}失敗。"),
                0,
            );
        }
    }

    fn try_steal(&mut self, s: usize) {
        if let (Some(r), None) = (self.bases[0], self.bases[1]) {
            let runner = &self.sides[s].bats[r];
            let sp = runner.speed;
            let steal_o = runner.abil & BIT_STEAL != 0;
            if sp > 55.0 || steal_o {
                let name = runner.name.clone();
                if s == self.us && steal_o {
                    self.mark_banner = Some(Ability::Steal);
                }
                if self.rng.unit()
                    < 0.45 + (sp - 55.0).max(0.0) * 0.012 + if steal_o { 0.16 } else { 0.0 }
                {
                    self.bases[1] = self.bases[0].take();
                    self.say_as(
                        PlayKind::Steal,
                        format!("{name} steals second!"),
                        format!("{name}、盗塁成功！"),
                        0,
                    );
                } else {
                    self.bases[0] = None;
                    self.outs += 1;
                    self.say_as(
                        PlayKind::CaughtStealing,
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
            self.mark_clutch = true;
            self.say(
                "WALK-OFF! The bench empties.".into(),
                "サヨナラ！ベンチが一斉に飛び出した！".into(),
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
            self.mark_clutch = true;
            self.say(
                "WALK-OFF! The bench empties.".into(),
                "サヨナラ！ベンチが一斉に飛び出した！".into(),
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

    /// Swap the batter at the plate for someone on our bench. The plate appearance
    /// is still to come.
    pub fn pinch_hit(&mut self, bench_idx: usize) -> bool {
        if self.batting() != self.us {
            return false;
        }
        self.swap_batter(
            self.up[self.us] % self.sides[self.us].bats.len(),
            bench_idx,
            true,
        )
    }

    /// The lead runner leaves for a faster pair of legs. The slot in the order changes
    /// for the rest of the game.
    pub fn pinch_run(&mut self, bench_idx: usize) -> bool {
        if self.batting() != self.us {
            return false;
        }
        let base = [2usize, 1, 0]
            .into_iter()
            .find(|&b| self.bases[b].is_some());
        let Some(base) = base else { return false };
        let slot = self.bases[base].unwrap_or(0);
        self.swap_batter(slot, bench_idx, false)
    }

    /// Replace the weakest glove on our side. Fielding is one number; this changes it.
    pub fn defense_sub(&mut self, bench_idx: usize) -> bool {
        let f = 1 - self.batting();
        if f != self.us {
            return false;
        }
        let Some(new) = self.sides[self.us].bench.get(bench_idx).cloned() else {
            return false;
        };
        if self.sides[self.us].gloves.is_empty() {
            return false;
        }
        let pitcher = self.pitcher_now().name.clone();
        let slot = self.sides[self.us]
            .gloves
            .iter()
            .enumerate()
            .filter(|(_, g)| g.name != pitcher)
            .min_by(|a, b| a.1.field.total_cmp(&b.1.field))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let old_name = self.sides[self.us].gloves[slot].name.clone();
        self.sides[self.us].gloves[slot] = Glove {
            name: new.name.clone(),
            field: new.field,
        };
        let n = self.sides[self.us].gloves.len().max(1) as f32;
        self.sides[self.us].field = self.sides[self.us]
            .gloves
            .iter()
            .map(|g| g.field)
            .sum::<f32>()
            / n;
        // The replaced fielder takes the bench spot if he is still in the lineup as a hitter.
        if let Some(i) = self.sides[self.us]
            .bats
            .iter()
            .position(|b| b.name == old_name)
        {
            let old = self.sides[self.us].bats[i].clone();
            self.sides[self.us].bats[i] = new.clone();
            self.sides[self.us].bench.remove(bench_idx);
            self.sides[self.us].bench.push(old);
        } else {
            self.sides[self.us].bench.remove(bench_idx);
            self.sides[self.us].bench.push(new.clone());
        }
        let name = new.name;
        self.say_as(
            PlayKind::Change,
            format!("Defensive change: {name}."),
            format!("守備交代、{name}。"),
            0,
        );
        true
    }

    fn swap_batter(&mut self, slot: usize, bench_idx: usize, hit: bool) -> bool {
        let s = self.us;
        if slot >= self.sides[s].bats.len() {
            return false;
        }
        let Some(new) = self.sides[s].bench.get(bench_idx).cloned() else {
            return false;
        };
        let old = self.sides[s].bats[slot].clone();
        self.sides[s].bats[slot] = new.clone();
        self.sides[s].bench.remove(bench_idx);
        self.sides[s].bench.push(old);
        let name = new.name;
        if hit {
            self.say_as(
                PlayKind::Change,
                format!("Pinch hitter: {name}."),
                format!("代打、{name}。"),
                0,
            );
        } else {
            self.say_as(
                PlayKind::Change,
                format!("Pinch runner: {name}."),
                format!("代走、{name}。"),
                0,
            );
        }
        true
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

    #[test]
    fn chances_and_pinches_ask_once_and_calls_play_out() {
        let mut asked = 0;
        let mut squeezes = 0;
        let mut walks = 0;
        for seed in 0..30 {
            let mut g = Game::new(side(50.0, seed), side(50.0, seed + 3), seed % 2 == 0, seed);
            let mut guard = 0;
            while !g.over && guard < 3000 {
                guard += 1;
                if let Some(spot) = g.spot() {
                    asked += 1;
                    let t = match spot {
                        Spot::Chance if Tactic::Squeeze.possible(&g) => Tactic::Squeeze,
                        Spot::Chance => Tactic::Steal,
                        // Walk him only with first base open (no forced run).
                        Spot::Pinch if g.bases[0].is_none() => Tactic::Walk,
                        Spot::Pinch => Tactic::Challenge,
                        Spot::Bench(_) => {
                            g.call(None);
                            g.step();
                            continue;
                        }
                    };
                    g.call(Some(t));
                    assert!(g.spot().is_none(), "asked once per plate appearance");
                }
                g.step();
            }
            assert!(g.over);
            squeezes += g.log.iter().filter(|p| p.ja.contains("スクイズ")).count();
            walks += g.log.iter().filter(|p| p.ja.contains("敬遠")).count();
        }
        assert!(asked > 60, "{asked}");
        assert!(squeezes > 5 && walks > 5, "{squeezes} {walks}");
        let g = Game::new(side(50.0, 1), side(50.0, 2), true, 1);
        assert!(!Tactic::Squeeze.possible(&g), "nobody on third");
    }

    #[test]
    fn a_bunt_is_offered_only_with_third_open_and_relief_never_doubles() {
        let mut g = Game::new(side(50.0, 1), side(50.0, 2), true, 1);
        g.bases = [Some(0), None, Some(1)];
        assert!(
            !Tactic::Bunt.possible(&g),
            "first and third: squeeze, not a sacrifice"
        );
        assert!(Tactic::Squeeze.possible(&g));
        g.bases = [Some(0), Some(1), None];
        assert!(Tactic::Bunt.possible(&g));
        // A tired pitcher is changed with Relief pending: only one new pitcher comes in.
        let mut g = Game::new(side(50.0, 3), side(50.0, 4), true, 3);
        g.top = false; // we field (we bat first = visitors)
        let f = 1 - g.batting();
        g.thrown[f] = 10_000;
        g.tactic = Some(Tactic::Relief);
        g.step();
        assert_eq!(g.arm[f], 1);
        assert_eq!(g.tactic, None);
        g.step();
        assert_eq!(g.arm[f], 1, "no second change");
    }

    #[test]
    fn abilities_are_named_and_a_pinch_hitter_takes_the_at_bat() {
        let mut g = Game::new(side(60.0, 4), side(40.0, 5), true, 4);
        assert!(!g.sides[0].bench.is_empty(), "the extra pitcher sits");
        let came = g.sides[0].bench[0].name.clone();
        let was = g.sides[0].bats[0].name.clone();
        assert!(g.pinch_hit(0));
        assert_eq!(g.sides[0].bats[0].name, came);
        assert_ne!(came, was);
        assert!(g.log.iter().any(|p| p.ja.contains("代打")));
        g.sides[0].bats[0].abil |= BIT_CHANCE;
        g.sides[0].bats[0].contact = 99.0;
        g.sides[0].bats[0].power = 90.0;
        g.bases = [None, Some(2), None];
        g.step();
        assert!(
            g.log.iter().any(|p| p.banner == Some(Ability::Chance)),
            "ここ一番 is on the play"
        );
        let mut fast = Game::new(side(50.0, 8), side(50.0, 9), true, 8);
        fast.sides[0].bats[1].abil |= BIT_STEAL;
        fast.sides[0].bats[1].speed = 80.0;
        fast.bases = [Some(1), None, None];
        fast.steal_lead(0);
        assert!(fast.log.iter().any(|p| p.banner == Some(Ability::Steal)));
    }
}
