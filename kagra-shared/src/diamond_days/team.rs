//! The club: players, their stats and growth, the line-up, graduation and first-years.
//! No picture, no input.

use serde::{Deserialize, Serialize};

use crate::game_core::swarm::Rng;

/// A fielding position. `P` pitches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Pos {
    P,
    C,
    B1,
    B2,
    B3,
    SS,
    LF,
    CF,
    RF,
}

impl Pos {
    pub const FIELD: [Pos; 9] = [
        Pos::P,
        Pos::C,
        Pos::B1,
        Pos::B2,
        Pos::B3,
        Pos::SS,
        Pos::LF,
        Pos::CF,
        Pos::RF,
    ];

    pub fn short(self, ja: bool) -> &'static str {
        match (self, ja) {
            (Pos::P, false) => "P",
            (Pos::C, false) => "C",
            (Pos::B1, false) => "1B",
            (Pos::B2, false) => "2B",
            (Pos::B3, false) => "3B",
            (Pos::SS, false) => "SS",
            (Pos::LF, false) => "LF",
            (Pos::CF, false) => "CF",
            (Pos::RF, false) => "RF",
            (Pos::P, true) => "投",
            (Pos::C, true) => "捕",
            (Pos::B1, true) => "一",
            (Pos::B2, true) => "二",
            (Pos::B3, true) => "三",
            (Pos::SS, true) => "遊",
            (Pos::LF, true) => "左",
            (Pos::CF, true) => "中",
            (Pos::RF, true) => "右",
        }
    }
}

/// What a week of training works on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Drill {
    Batting,
    Pitching,
    Fielding,
    Running,
    Strength,
}

impl Drill {
    pub const ALL: [Drill; 5] = [
        Drill::Batting,
        Drill::Pitching,
        Drill::Fielding,
        Drill::Running,
        Drill::Strength,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

/// How a player feels this week. Better condition trains and plays better.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Mood {
    Awful,
    Poor,
    Normal,
    Good,
    Great,
}

impl Mood {
    pub fn up(self) -> Self {
        match self {
            Mood::Awful => Mood::Poor,
            Mood::Poor => Mood::Normal,
            Mood::Normal => Mood::Good,
            _ => Mood::Great,
        }
    }

    pub fn down(self) -> Self {
        match self {
            Mood::Great => Mood::Good,
            Mood::Good => Mood::Normal,
            Mood::Normal => Mood::Poor,
            _ => Mood::Awful,
        }
    }

    /// Training and playing multiplier.
    pub fn factor(self) -> f32 {
        match self {
            Mood::Awful => 0.6,
            Mood::Poor => 0.8,
            Mood::Normal => 1.0,
            Mood::Good => 1.15,
            Mood::Great => 1.3,
        }
    }

    pub fn label(self, ja: bool) -> &'static str {
        match (self, ja) {
            (Mood::Awful, false) => "Awful",
            (Mood::Poor, false) => "Poor",
            (Mood::Normal, false) => "Normal",
            (Mood::Good, false) => "Good",
            (Mood::Great, false) => "Great",
            (Mood::Awful, true) => "絶不調",
            (Mood::Poor, true) => "不調",
            (Mood::Normal, true) => "普通",
            (Mood::Good, true) => "好調",
            (Mood::Great, true) => "絶好調",
        }
    }
}

/// A special trait. A player carries at most two. They change the plate-appearance
/// sim, and the match names them when they fire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Ability {
    /// Batting up with a runner on second or third.
    Chance = 0,
    /// Pitching when the opponent has a runner in scoring position.
    Pinch = 1,
    /// Late and close.
    Clutch = 2,
    /// Base stealing.
    Steal = 3,
    /// Less likely to get hurt, and shrugs off a red-mark week.
    Durable = 4,
    /// Lifts the rest of the lineup.
    Mood = 5,
    /// More walks.
    Eye = 6,
    /// A heavy fastball: pitching stuff up.
    Heavy = 7,
}

pub const BIT_CHANCE: u8 = 1 << 0;
pub const BIT_PINCH: u8 = 1 << 1;
pub const BIT_CLUTCH: u8 = 1 << 2;
pub const BIT_STEAL: u8 = 1 << 3;
pub const BIT_DURABLE: u8 = 1 << 4;
pub const BIT_MOOD: u8 = 1 << 5;
pub const BIT_EYE: u8 = 1 << 6;
pub const BIT_HEAVY: u8 = 1 << 7;

impl Ability {
    pub const ALL: [Ability; 8] = [
        Ability::Chance,
        Ability::Pinch,
        Ability::Clutch,
        Ability::Steal,
        Ability::Durable,
        Ability::Mood,
        Ability::Eye,
        Ability::Heavy,
    ];

    pub fn bit(self) -> u8 {
        1 << (self as u8)
    }

    /// What the screen shouts when it fires.
    pub fn label(self, ja: bool) -> &'static str {
        match (self, ja) {
            (Ability::Chance, false) => "Big Moment",
            (Ability::Pinch, false) => "Fireman",
            (Ability::Clutch, false) => "Late Grit",
            (Ability::Steal, false) => "Burner",
            (Ability::Durable, false) => "Iron Body",
            (Ability::Mood, false) => "Spark",
            (Ability::Eye, false) => "Good Eye",
            (Ability::Heavy, false) => "Heater",
            (Ability::Chance, true) => "ここ一番",
            (Ability::Pinch, true) => "火消し",
            (Ability::Clutch, true) => "終盤の粘り",
            (Ability::Steal, true) => "韋駄天",
            (Ability::Durable, true) => "鉄の体",
            (Ability::Mood, true) => "元気印",
            (Ability::Eye, true) => "見極め",
            (Ability::Heavy, true) => "剛球",
        }
    }
}

/// A hand-tuned face: the founding ace and the founding captain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    #[default]
    Regular,
    Ace,
    Captain,
}

/// The coach's batting order and defensive spots, by family name (unique in the club).
/// Empty means the automatic order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineCard {
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub spots: Vec<Pos>,
}

/// Where a third-year goes in March. Derived, not a minigame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    Pro,
    College,
    Job,
}

impl After {
    pub fn label(self, ja: bool) -> &'static str {
        match (self, ja) {
            (After::Pro, false) => "Pro",
            (After::College, false) => "College",
            (After::Job, false) => "Work",
            (After::Pro, true) => "プロ",
            (After::College, true) => "大学",
            (After::Job, true) => "就職",
        }
    }
}

/// A stat as a letter: S (90+), A (80+), B, C, D, E, F, G (below 20).
pub fn grade(v: u8) -> &'static str {
    match v {
        90.. => "S",
        80..=89 => "A",
        70..=79 => "B",
        60..=69 => "C",
        50..=59 => "D",
        40..=49 => "E",
        20..=39 => "F",
        _ => "G",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub family: String,
    pub given: String,
    /// 1 to 3.
    pub grade: u8,
    pub pos: Pos,
    pub contact: u8,
    pub power: u8,
    pub speed: u8,
    pub field: u8,
    pub arm: u8,
    /// Pitchers: top speed in km/h.
    pub velo: u8,
    pub control: u8,
    pub stamina_p: u8,
    pub breaking: u8,
    pub mood: Mood,
    /// 0 to 100: tiredness taken away; low stamina trains badly and risks injury.
    pub stamina: u8,
    /// Weeks still hurt (cannot train or play).
    pub hurt: u8,
    /// Practice points per drill, waiting to be spent on stats.
    pub xp: [u16; 5],
    /// The drill this player works on in individual practice every week.
    #[serde(default = "default_menu")]
    pub menu: Drill,
    /// Zero, one or two special traits.
    #[serde(default)]
    pub abilities: Vec<Ability>,
    /// Weeks he cannot play for a red mark (the same gate as an injury).
    #[serde(default)]
    pub study: u8,
    /// Hand-tuned portrait for the founding ace and captain.
    #[serde(default)]
    pub role: Role,
}

fn default_menu() -> Drill {
    Drill::Batting
}

/// A stat the coach can raise with points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stat {
    Contact,
    Power,
    Speed,
    Field,
    Arm,
    Velo,
    Control,
    Stamina,
    Breaking,
}

impl Stat {
    pub const ALL: [Stat; 9] = [
        Stat::Contact,
        Stat::Power,
        Stat::Speed,
        Stat::Field,
        Stat::Arm,
        Stat::Velo,
        Stat::Control,
        Stat::Stamina,
        Stat::Breaking,
    ];

    /// The drill whose points pay for it.
    pub fn pool(self) -> Drill {
        match self {
            Stat::Contact => Drill::Batting,
            Stat::Power | Stat::Stamina => Drill::Strength,
            Stat::Speed => Drill::Running,
            Stat::Field | Stat::Arm => Drill::Fielding,
            Stat::Velo | Stat::Control | Stat::Breaking => Drill::Pitching,
        }
    }

    /// Only pitchers raise these.
    pub fn pitching(self) -> bool {
        matches!(
            self,
            Stat::Velo | Stat::Control | Stat::Stamina | Stat::Breaking
        )
    }

    pub fn label(self, ja: bool) -> &'static str {
        match (self, ja) {
            (Stat::Contact, false) => "Contact",
            (Stat::Power, false) => "Power",
            (Stat::Speed, false) => "Speed",
            (Stat::Field, false) => "Fielding",
            (Stat::Arm, false) => "Arm",
            (Stat::Velo, false) => "Velocity",
            (Stat::Control, false) => "Control",
            (Stat::Stamina, false) => "Stamina",
            (Stat::Breaking, false) => "Breaking",
            (Stat::Contact, true) => "ミート",
            (Stat::Power, true) => "パワー",
            (Stat::Speed, true) => "走力",
            (Stat::Field, true) => "守備",
            (Stat::Arm, true) => "肩",
            (Stat::Velo, true) => "球速",
            (Stat::Control, true) => "制球",
            (Stat::Stamina, true) => "スタミナ",
            (Stat::Breaking, true) => "変化球",
        }
    }
}

impl Player {
    pub fn name(&self) -> String {
        self.family.clone()
    }

    pub fn full_name(&self) -> String {
        format!("{} {}", self.family, self.given)
    }

    pub fn pitcher(&self) -> bool {
        self.pos == Pos::P
    }

    /// One number for how good a fielder-hitter is.
    pub fn bat_rating(&self) -> f32 {
        self.contact as f32 * 0.35
            + self.power as f32 * 0.3
            + self.speed as f32 * 0.15
            + self.field as f32 * 0.15
            + self.arm as f32 * 0.05
    }

    pub fn pitch_rating(&self) -> f32 {
        (self.velo as f32 - 110.0) * 1.4 * 0.35
            + self.control as f32 * 0.3
            + self.breaking as f32 * 0.25
            + self.stamina_p as f32 * 0.1
    }

    pub fn can_play(&self) -> bool {
        self.hurt == 0 && self.study == 0
    }

    pub fn has(&self, a: Ability) -> bool {
        self.abilities.contains(&a)
    }

    pub fn bits(&self) -> u8 {
        self.abilities.iter().fold(0, |n, a| n | a.bit())
    }

    /// Learn `a` if there is room and he does not already have it.
    pub fn grant(&mut self, a: Ability) -> bool {
        if self.has(a) || self.abilities.len() >= 2 {
            return false;
        }
        self.abilities.push(a);
        true
    }

    /// Make room for a trait the story insists on (the ace's heavy ball, the captain's spark).
    pub fn grant_force(&mut self, a: Ability) {
        if self.has(a) {
            return;
        }
        if self.abilities.len() >= 2 {
            self.abilities.pop();
        }
        self.abilities.push(a);
    }

    /// Practice points into the drill's pool (spent later on stats, see [`Player::raise`]).
    /// Condition and tiredness scale them; the hurt learn nothing. Returns the points.
    pub fn train(&mut self, d: Drill, points: f32) -> u16 {
        if self.hurt > 0 {
            return 0;
        }
        let tired = if self.stamina < 30 { 0.5 } else { 1.0 };
        let gain = (points * self.mood.factor() * tired).round() as u16;
        self.xp[d.index()] += gain;
        gain
    }

    pub fn stat(&self, st: Stat) -> u8 {
        match st {
            Stat::Contact => self.contact,
            Stat::Power => self.power,
            Stat::Speed => self.speed,
            Stat::Field => self.field,
            Stat::Arm => self.arm,
            Stat::Velo => self.velo,
            Stat::Control => self.control,
            Stat::Stamina => self.stamina_p,
            Stat::Breaking => self.breaking,
        }
    }

    fn stat_mut(&mut self, st: Stat) -> &mut u8 {
        match st {
            Stat::Contact => &mut self.contact,
            Stat::Power => &mut self.power,
            Stat::Speed => &mut self.speed,
            Stat::Field => &mut self.field,
            Stat::Arm => &mut self.arm,
            Stat::Velo => &mut self.velo,
            Stat::Control => &mut self.control,
            Stat::Stamina => &mut self.stamina_p,
            Stat::Breaking => &mut self.breaking,
        }
    }

    /// Points one step of a stat costs now: more the higher it is.
    pub fn cost(&self, st: Stat) -> u16 {
        let v = self.stat(st) as u16;
        match st {
            // Velocity counts in km/h (110..160): about the same climb.
            Stat::Velo => 6 + (v.saturating_sub(110)) / 3,
            _ => 4 + v / 8,
        }
    }

    /// Can `st` go up a step now (enough points, below the cap, the right player)?
    pub fn can_raise(&self, st: Stat) -> bool {
        let cap = if st == Stat::Velo { 160 } else { 99 };
        (self.pitcher() || !st.pitching())
            && self.stat(st) < cap
            && self.xp[st.pool().index()] >= self.cost(st)
    }

    /// Spend points: `st` goes up one. False when it cannot.
    pub fn raise(&mut self, st: Stat) -> bool {
        if !self.can_raise(st) {
            return false;
        }
        let c = self.cost(st);
        self.xp[st.pool().index()] -= c;
        *self.stat_mut(st) += 1;
        self.maybe_learn(st);
        true
    }

    /// Crossing a threshold can teach a trait. One step, once.
    fn maybe_learn(&mut self, st: Stat) {
        let v = self.stat(st);
        let learned = match (st, v) {
            (Stat::Speed, 70) => Some(Ability::Steal),
            (Stat::Contact, 72) => Some(Ability::Eye),
            (Stat::Power, 74) => Some(Ability::Chance),
            (Stat::Power, 82) => Some(Ability::Clutch),
            (Stat::Velo, 140) if self.pitcher() => Some(Ability::Heavy),
            (Stat::Control, 66) if self.pitcher() => Some(Ability::Pinch),
            (Stat::Field, 74) => Some(Ability::Durable),
            _ => None,
        };
        if let Some(a) = learned {
            self.grant(a);
        }
    }

    /// Spend every point that can be spent, on the lowest stat each pool feeds first (the
    /// "leave it to the coach" button, and the test bots). Returns the steps taken.
    pub fn auto_raise(&mut self) -> usize {
        let mut n = 0;
        loop {
            let pick = Stat::ALL
                .iter()
                .copied()
                .filter(|&st| self.can_raise(st))
                .min_by_key(|&st| {
                    // Velocity on the same 0-99 scale as the rest.
                    if st == Stat::Velo {
                        (self.velo as u16).saturating_sub(110) * 2
                    } else {
                        self.stat(st) as u16
                    }
                });
            match pick {
                Some(st) => {
                    self.raise(st);
                    n += 1;
                }
                None => return n,
            }
        }
    }

    /// The week's wear: training costs stamina, rest gives it back.
    pub fn tire(&mut self, cost: u8) {
        self.stamina = self.stamina.saturating_sub(cost);
    }

    pub fn rest(&mut self, gain: u8) {
        self.stamina = (self.stamina + gain).min(100);
    }
}

// Names: common Japanese family names and given names (no real players).
const FAMILY: [&str; 40] = [
    "佐藤",
    "鈴木",
    "高橋",
    "田中",
    "伊藤",
    "渡辺",
    "山本",
    "中村",
    "小林",
    "加藤",
    "吉田",
    "山田",
    "佐々木",
    "山口",
    "松本",
    "井上",
    "木村",
    "林",
    "清水",
    "山崎",
    "森",
    "池田",
    "橋本",
    "阿部",
    "石川",
    "前田",
    "藤田",
    "小川",
    "岡田",
    "後藤",
    "長谷川",
    "村上",
    "近藤",
    "坂本",
    "遠藤",
    "青木",
    "西村",
    "福田",
    "太田",
    "三浦",
];
const GIVEN: [&str; 32] = [
    "翔太",
    "大輝",
    "拓海",
    "蓮",
    "陽斗",
    "颯太",
    "悠真",
    "湊",
    "大和",
    "陸",
    "海斗",
    "健太",
    "直樹",
    "亮",
    "誠",
    "優斗",
    "智也",
    "隼人",
    "啓介",
    "光",
    "航",
    "樹",
    "駿",
    "楓",
    "瑛太",
    "蒼",
    "隆之介",
    "一真",
    "晴",
    "空",
    "勇気",
    "仁",
];

/// A new player of `grade` with talent around `level` (40 is an ordinary first-year).
pub fn rookie(rng: &mut Rng, grade: u8, pos: Pos, level: f32) -> Player {
    let mut stat =
        |spread: f32| -> u8 { (level + rng.range(-spread, spread)).clamp(5.0, 95.0) as u8 };
    let pitcher = pos == Pos::P;
    let (contact, power, speed, field, arm) =
        (stat(12.0), stat(14.0), stat(14.0), stat(12.0), stat(12.0));
    let control = if pitcher { stat(12.0) } else { 20 };
    let stamina_p = if pitcher { stat(12.0) } else { 20 };
    let breaking = if pitcher { stat(12.0) } else { 10 };
    let velo = if pitcher {
        (118.0 + (level - 40.0) * 0.5 + rng.range(-5.0, 7.0)).clamp(110.0, 150.0) as u8
    } else {
        110
    };
    let family = FAMILY[(rng.next_u32() as usize) % FAMILY.len()].to_string();
    let given = GIVEN[(rng.next_u32() as usize) % GIVEN.len()].to_string();
    Player {
        family,
        given,
        grade,
        pos,
        contact,
        power,
        speed,
        field,
        arm,
        velo,
        control,
        stamina_p,
        breaking,
        mood: Mood::Normal,
        stamina: 100,
        hurt: 0,
        xp: [0; 5],
        menu: if pitcher {
            Drill::Pitching
        } else {
            Drill::Batting
        },
        abilities: roll_abilities(rng, pitcher),
        study: 0,
        role: Role::Regular,
    }
}

/// Most players have none; a few have one; a handful have two. Pitchers and fielders
/// draw from different lists so a shortstop is not born with a heavy fastball.
fn roll_abilities(rng: &mut Rng, pitcher: bool) -> Vec<Ability> {
    let roll = rng.unit();
    let n = if roll < 0.46 {
        0
    } else if roll < 0.86 {
        1
    } else {
        2
    };
    let pool: [Ability; 6] = if pitcher {
        [
            Ability::Heavy,
            Ability::Pinch,
            Ability::Clutch,
            Ability::Durable,
            Ability::Mood,
            Ability::Eye,
        ]
    } else {
        [
            Ability::Chance,
            Ability::Steal,
            Ability::Eye,
            Ability::Clutch,
            Ability::Mood,
            Ability::Durable,
        ]
    };
    let mut out = Vec::new();
    for _ in 0..n {
        let a = pool[(rng.next_u32() as usize) % pool.len()];
        if !out.contains(&a) {
            out.push(a);
        }
    }
    out
}

/// Pro / college / work, from how good he is, the school's name, and his traits.
pub fn destination(p: &Player, reputation: f32) -> After {
    let rating = if p.pitcher() {
        p.pitch_rating()
    } else {
        p.bat_rating()
    };
    let score = rating + reputation * 0.12 + p.abilities.len() as f32 * 3.5;
    if score >= 64.0 {
        After::Pro
    } else if score >= 46.0 {
        After::College
    } else {
        After::Job
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Team {
    pub players: Vec<Player>,
    /// The coach's order. Empty: [`Team::lineup`] fills it every game.
    #[serde(default)]
    pub card: LineCard,
}

impl Team {
    /// The club on the coach's first day: a dozen players, three grades, weak.
    pub fn founding(rng: &mut Rng) -> Self {
        let mut players = Vec::new();
        let plan = [
            (Pos::P, 3),
            (Pos::P, 2),
            (Pos::C, 2),
            (Pos::B1, 3),
            (Pos::B2, 2),
            (Pos::B3, 1),
            (Pos::SS, 3),
            (Pos::LF, 1),
            (Pos::CF, 2),
            (Pos::RF, 1),
            (Pos::P, 1),
            (Pos::B2, 1),
        ];
        for (pos, grade) in plan {
            let level = 30.0 + grade as f32 * 5.0;
            players.push(rookie(rng, grade, pos, level));
        }
        let mut t = Self {
            players,
            card: LineCard::default(),
        };
        t.unique_names();
        t.crown();
        t
    }

    /// A club you inherit at another school: the same shape as opening day, around `level`.
    pub fn takeover(rng: &mut Rng, level: f32) -> Self {
        let mut players = Vec::new();
        let plan = [
            (Pos::P, 3),
            (Pos::P, 2),
            (Pos::C, 2),
            (Pos::B1, 3),
            (Pos::B2, 2),
            (Pos::B3, 1),
            (Pos::SS, 3),
            (Pos::LF, 1),
            (Pos::CF, 2),
            (Pos::RF, 1),
            (Pos::P, 1),
            (Pos::B2, 1),
        ];
        for (pos, grade) in plan {
            players.push(rookie(rng, grade, pos, level + grade as f32 * 2.0));
        }
        let mut t = Self {
            players,
            card: LineCard::default(),
        };
        t.unique_names();
        t.crown();
        t
    }

    /// The founding ace throws a heavy ball; the best older fielder is the captain.
    fn crown(&mut self) {
        if let Some(i) = self.ace() {
            self.players[i].role = Role::Ace;
            self.players[i].grant_force(Ability::Heavy);
        }
        let cap = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.pitcher())
            .max_by(|a, b| {
                (a.1.grade, a.1.bat_rating().to_bits())
                    .cmp(&(b.1.grade, b.1.bat_rating().to_bits()))
            })
            .map(|(i, _)| i);
        if let Some(i) = cap {
            self.players[i].role = Role::Captain;
            self.players[i].grant_force(Ability::Mood);
        }
    }

    /// No two players share a family name (the play-by-play uses it).
    pub fn unique_names(&mut self) {
        for i in 0..self.players.len() {
            let mut k = 0;
            while self.players[..i]
                .iter()
                .any(|p| p.family == self.players[i].family)
            {
                k += 1;
                self.players[i].family = FAMILY[(i * 7 + k * 13) % FAMILY.len()].to_string();
                if k > FAMILY.len() {
                    break;
                }
            }
        }
    }

    /// The best pitcher who can play and is fresh enough, else anyone.
    pub fn ace(&self) -> Option<usize> {
        let ok = |p: &&Player| p.pitcher() && p.can_play();
        self.players
            .iter()
            .enumerate()
            .filter(|(_, p)| ok(p))
            .max_by(|a, b| {
                let score = |p: &Player| p.pitch_rating() * (0.5 + p.stamina as f32 / 200.0);
                score(a.1).total_cmp(&score(b.1))
            })
            .map(|(i, _)| i)
    }

    /// Nine batters (indices), the best one per position, then the best left, in a batting
    /// order: speed first, the strongest third and fourth. The pitcher bats ninth.
    pub fn lineup(&self, pitcher: usize) -> Vec<usize> {
        let mut used = vec![pitcher];
        let mut nine = Vec::new();
        for pos in Pos::FIELD.iter().skip(1) {
            let best = self
                .players
                .iter()
                .enumerate()
                .filter(|(i, p)| !used.contains(i) && p.can_play() && p.pos == *pos)
                .max_by(|a, b| a.1.bat_rating().total_cmp(&b.1.bat_rating()))
                .map(|(i, _)| i);
            if let Some(i) = best {
                used.push(i);
                nine.push(i);
            }
        }
        // Fill gaps with the best hitters left (pitchers last).
        while nine.len() < 8 {
            let best = self
                .players
                .iter()
                .enumerate()
                .filter(|(i, p)| !used.contains(i) && p.can_play())
                .max_by(|a, b| {
                    let k = |p: &Player| p.bat_rating() - if p.pitcher() { 20.0 } else { 0.0 };
                    k(a.1).total_cmp(&k(b.1))
                })
                .map(|(i, _)| i);
            match best {
                Some(i) => {
                    used.push(i);
                    nine.push(i);
                }
                None => break,
            }
        }
        // Order: by rating, then lead-off the fastest of the top half.
        nine.sort_by(|&a, &b| {
            self.players[b]
                .bat_rating()
                .total_cmp(&self.players[a].bat_rating())
        });
        // 1: speed, 2: contact, 3/4/5: the best; the rest in order.
        let mut order: Vec<usize> = Vec::new();
        if nine.len() >= 5 {
            let top: Vec<usize> = nine[..5].to_vec();
            let lead = *top.iter().max_by_key(|&&i| self.players[i].speed).unwrap();
            let rest: Vec<usize> = top.iter().copied().filter(|&i| i != lead).collect();
            order.push(lead);
            order.push(rest[3]);
            order.extend_from_slice(&rest[..3]);
            order.extend_from_slice(&nine[5..]);
        } else {
            order = nine;
        }
        order.push(pitcher);
        order
    }

    /// Nine who can play, with a position each. The saved card if it is still legal,
    /// otherwise [`Team::lineup`].
    pub fn resolved(&self) -> Vec<(usize, Pos)> {
        if let Some(nine) = self.try_card() {
            return nine;
        }
        let Some(p) = self.ace() else {
            return Vec::new();
        };
        let order = self.lineup(p);
        let spots = assign_spots(&self.players, &order);
        order.into_iter().zip(spots).collect()
    }

    fn try_card(&self) -> Option<Vec<(usize, Pos)>> {
        if self.card.order.len() != 9 || self.card.spots.len() != 9 {
            return None;
        }
        let mut nine = Vec::with_capacity(9);
        for (name, pos) in self.card.order.iter().zip(self.card.spots.iter()) {
            let i = self
                .players
                .iter()
                .position(|p| &p.family == name && p.can_play())?;
            if nine.iter().any(|(j, _)| *j == i) {
                return None;
            }
            nine.push((i, *pos));
        }
        let mound = nine
            .iter()
            .any(|(i, pos)| *pos == Pos::P && self.players[*i].pitcher());
        mound.then_some(nine)
    }

    /// Store a coach's order. False when it repeats a player, uses someone who cannot
    /// play, or leaves the mound without a pitcher.
    pub fn set_lineup(&mut self, order: &[usize], spots: &[Pos]) -> bool {
        if order.len() != 9 || spots.len() != 9 {
            return false;
        }
        let mut seen = Vec::new();
        for &i in order {
            if i >= self.players.len() || seen.contains(&i) || !self.players[i].can_play() {
                return false;
            }
            seen.push(i);
        }
        if !order
            .iter()
            .zip(spots)
            .any(|(&i, pos)| *pos == Pos::P && self.players[i].pitcher())
        {
            return false;
        }
        self.card.order = order
            .iter()
            .map(|&i| self.players[i].family.clone())
            .collect();
        self.card.spots = spots.to_vec();
        true
    }

    /// Back to the automatic order.
    pub fn clear_lineup(&mut self) {
        self.card = LineCard::default();
    }

    /// How strong the team is (about 30 for a weak club, 70 for a powerhouse).
    pub fn strength(&self) -> f32 {
        let nine = self.resolved();
        if nine.is_empty() {
            return 0.0;
        }
        let bats: f32 = nine
            .iter()
            .map(|(i, _)| self.players[*i].bat_rating())
            .sum::<f32>()
            / nine.len() as f32;
        let p = nine
            .iter()
            .find(|(_, pos)| *pos == Pos::P)
            .map(|(i, _)| *i)
            .unwrap_or(nine[0].0);
        bats * 0.6 + self.players[p].pitch_rating() * 0.4
    }

    /// March: third-years leave. Returns how many. The saved order is cleared; April
    /// starts on the automatic one.
    pub fn graduate(&mut self) -> usize {
        let before = self.players.len();
        self.players.retain(|p| p.grade < 3);
        for p in &mut self.players {
            p.grade += 1;
        }
        self.card = LineCard::default();
        before - self.players.len()
    }

    /// April: the scouted players join, then walk-ins (more and better with reputation, 0 to
    /// 100). At least one pitcher and a catcher if the club needs them.
    pub fn recruit(&mut self, rng: &mut Rng, reputation: f32, scouted: Vec<Player>) -> usize {
        let signed = scouted.len();
        for mut p in scouted {
            p.grade = 1;
            self.players.push(p);
        }
        let n = (3 + (reputation / 25.0) as usize).saturating_sub(signed / 2);
        let level = 30.0 + reputation * 0.25;
        let mut need: Vec<Pos> = Vec::new();
        if self.players.iter().filter(|p| p.pitcher()).count() < 3 {
            need.push(Pos::P);
        }
        if !self.players.iter().any(|p| p.pos == Pos::C) {
            need.push(Pos::C);
        }
        for k in 0..n {
            let pos = need
                .get(k)
                .copied()
                .unwrap_or_else(|| Pos::FIELD[1 + (rng.next_u32() as usize) % 8]);
            // Now and then a gem.
            let gem = if rng.unit() < 0.1 + reputation / 500.0 {
                12.0
            } else {
                0.0
            };
            self.players.push(rookie(rng, 1, pos, level + gem));
        }
        self.unique_names();
        n + signed
    }

    /// A middle-school player found by scouting: better with reputation, sometimes a gem.
    pub fn prospect(rng: &mut Rng, reputation: f32) -> Player {
        let pos = Pos::FIELD[(rng.next_u32() as usize) % 9];
        let gem = if rng.unit() < 0.25 { 10.0 } else { 0.0 };
        rookie(rng, 1, pos, 38.0 + reputation * 0.25 + gem)
    }
}

/// Defensive spots for an automatic batting order: the pitcher on the mound, everyone
/// else at his own position when it is free, and the leftovers in the gaps.
fn assign_spots(players: &[Player], order: &[usize]) -> Vec<Pos> {
    let mut spots: Vec<Option<Pos>> = vec![None; order.len()];
    let mut used = [false; 9];
    let index = |pos: Pos| Pos::FIELD.iter().position(|p| *p == pos).unwrap_or(0);
    if let Some(k) = order
        .iter()
        .enumerate()
        .rev()
        .find_map(|(k, &i)| players.get(i).and_then(|p| p.pitcher().then_some(k)))
    {
        spots[k] = Some(Pos::P);
        used[index(Pos::P)] = true;
    }
    for (k, &i) in order.iter().enumerate() {
        if spots[k].is_some() {
            continue;
        }
        let Some(p) = players.get(i) else { continue };
        let idx = index(p.pos);
        if p.pos != Pos::P && !used[idx] {
            spots[k] = Some(p.pos);
            used[idx] = true;
        }
    }
    for spot in spots.iter_mut() {
        if spot.is_some() {
            continue;
        }
        let pos = Pos::FIELD
            .into_iter()
            .find(|p| !used[index(*p)])
            .unwrap_or(Pos::LF);
        *spot = Some(pos);
        used[index(pos)] = true;
    }
    spots.into_iter().map(|s| s.unwrap_or(Pos::LF)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_founding_club_fields_nine_with_a_pitcher_batting_ninth() {
        let mut rng = Rng::new(3);
        let t = Team::founding(&mut rng);
        assert_eq!(t.players.len(), 12);
        let p = t.ace().unwrap();
        assert!(t.players[p].pitcher());
        let nine = t.lineup(p);
        assert_eq!(nine.len(), 9);
        assert_eq!(*nine.last().unwrap(), p);
        let mut uniq = nine.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(uniq.len(), 9);
        let names: std::collections::BTreeSet<&str> =
            t.players.iter().map(|p| p.family.as_str()).collect();
        assert_eq!(names.len(), 12, "family names are unique");
        assert!((20.0..60.0).contains(&t.strength()), "{}", t.strength());
    }

    #[test]
    fn points_pile_up_and_the_coach_spends_them_on_stats() {
        let mut rng = Rng::new(1);
        let mut a = rookie(&mut rng, 1, Pos::SS, 40.0);
        assert_eq!(a.train(Drill::Running, 25.0), 25);
        let speed = a.speed;
        let cost = a.cost(Stat::Speed);
        assert!(a.raise(Stat::Speed));
        assert_eq!(a.speed, speed + 1);
        assert_eq!(a.xp[Drill::Running.index()], 25 - cost);
        assert!(!a.raise(Stat::Velo), "a shortstop does not pitch");
        assert!(!a.raise(Stat::Contact), "no batting points yet");
        let steps = a.auto_raise();
        assert!(steps >= 1 && !a.can_raise(Stat::Speed));
        // Tired players learn half, the hurt nothing.
        let mut b = rookie(&mut rng, 1, Pos::P, 40.0);
        b.stamina = 10;
        assert_eq!(b.train(Drill::Pitching, 10.0), 5);
        b.hurt = 2;
        assert_eq!(b.train(Drill::Pitching, 50.0), 0);
        // Higher stats cost more.
        b.control = 90;
        let dear = b.cost(Stat::Control);
        b.control = 30;
        assert!(dear > b.cost(Stat::Control));
        assert!(Mood::Great.factor() > Mood::Awful.factor());
        assert_eq!(grade(91), "S");
        assert_eq!(grade(15), "G");
    }

    #[test]
    fn march_and_april_turn_the_club_over() {
        let mut rng = Rng::new(9);
        let mut t = Team::founding(&mut rng);
        let thirds = t.players.iter().filter(|p| p.grade == 3).count();
        assert_eq!(t.graduate(), thirds);
        assert!(t.players.iter().all(|p| p.grade >= 2));
        let kid = Team::prospect(&mut rng, 60.0);
        let n = t.recruit(&mut rng, 60.0, vec![kid]);
        assert_eq!(n, 6);
        assert!(t.players.iter().filter(|p| p.pitcher()).count() >= 2);
    }

    #[test]
    fn players_carry_at_most_two_abilities_and_the_ace_throws_heavy() {
        let mut rng = Rng::new(3);
        let t = Team::founding(&mut rng);
        assert!(t.players.iter().all(|p| p.abilities.len() <= 2));
        let ace = t.ace().unwrap();
        assert_eq!(t.players[ace].role, Role::Ace);
        assert!(t.players[ace].has(Ability::Heavy));
        assert!(t
            .players
            .iter()
            .any(|p| p.role == Role::Captain && p.has(Ability::Mood)));
        let mut kid = rookie(&mut rng, 1, Pos::SS, 40.0);
        kid.abilities.clear();
        kid.speed = 69;
        kid.xp[Drill::Running.index()] = 500;
        assert!(kid.raise(Stat::Speed));
        assert!(kid.has(Ability::Steal));
        assert!(!kid.grant(Ability::Steal));
        kid.grant_force(Ability::Chance);
        kid.grant_force(Ability::Eye);
        assert_eq!(kid.abilities.len(), 2);
    }

    #[test]
    fn a_saved_order_is_used_until_it_is_illegal_or_cleared() {
        let mut rng = Rng::new(3);
        let mut t = Team::founding(&mut rng);
        let auto = t.resolved();
        assert_eq!(auto.len(), 9);
        assert!(auto.iter().any(|(_, p)| *p == Pos::P));
        let order: Vec<usize> = auto.iter().map(|(i, _)| *i).collect();
        let mut spots: Vec<Pos> = auto.iter().map(|(_, p)| *p).collect();
        // Swap the first two batters and keep their positions with them.
        spots.swap(0, 1);
        let mut flipped = order.clone();
        flipped.swap(0, 1);
        assert!(t.set_lineup(&flipped, &spots));
        let again = t.resolved();
        assert_eq!(again[0].0, flipped[0]);
        assert_eq!(again[0].1, spots[0]);
        // A red mark knocks the card out: the automatic order comes back.
        t.players[flipped[0]].study = 2;
        assert!(!t.players[flipped[0]].can_play());
        assert_ne!(t.resolved()[0].0, flipped[0]);
        t.players[flipped[0]].study = 0;
        assert_eq!(t.resolved()[0].0, flipped[0]);
        t.clear_lineup();
        assert!(t.card.order.is_empty());
        // No pitcher on the mound is rejected.
        let mut bad = spots.clone();
        if let Some(p) = bad.iter_mut().find(|p| **p == Pos::P) {
            *p = Pos::LF;
        }
        assert!(!t.set_lineup(&order, &bad));
        let mut star = rookie(&mut rng, 3, Pos::P, 70.0);
        star.velo = 155;
        star.control = 90;
        star.breaking = 85;
        star.abilities = vec![Ability::Heavy, Ability::Pinch];
        assert_eq!(destination(&star, 80.0), After::Pro);
        let mut weak = rookie(&mut rng, 3, Pos::LF, 18.0);
        weak.abilities.clear();
        weak.contact = 22;
        weak.power = 20;
        weak.speed = 22;
        weak.field = 20;
        weak.arm = 20;
        assert_eq!(destination(&weak, 0.0), After::Job);
    }
}
