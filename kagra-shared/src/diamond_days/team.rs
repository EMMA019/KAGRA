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

    fn index(self) -> usize {
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
    /// Practice points per drill (10 raise a stat).
    pub xp: [u16; 5],
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
        self.hurt == 0
    }

    /// Practice points for a week's drill. Returns the stats that went up (names).
    pub fn train(&mut self, d: Drill, points: f32) -> Vec<&'static str> {
        if self.hurt > 0 {
            return Vec::new();
        }
        let tired = if self.stamina < 30 { 0.5 } else { 1.0 };
        let gain = (points * self.mood.factor() * tired).round() as u16;
        let i = d.index();
        self.xp[i] += gain;
        let mut ups = Vec::new();
        while self.xp[i] >= 10 {
            self.xp[i] -= 10;
            let up = |v: &mut u8| {
                let before = *v;
                *v = (*v + 1).min(99);
                *v > before
            };
            match d {
                Drill::Batting => {
                    if self.contact <= self.power && up(&mut self.contact) {
                        ups.push("contact");
                    } else if up(&mut self.power) {
                        ups.push("power");
                    }
                }
                Drill::Pitching if self.pitcher() => {
                    // Velocity, control and the breaking ball take turns.
                    let k = (self.velo as u16 + self.control as u16 + self.breaking as u16) % 3;
                    if k == 0 && self.velo < 160 {
                        self.velo += 1;
                        ups.push("velo");
                    } else if k == 1 && up(&mut self.control) {
                        ups.push("control");
                    } else if up(&mut self.breaking) {
                        ups.push("breaking");
                    }
                }
                Drill::Pitching => {
                    if up(&mut self.arm) {
                        ups.push("arm");
                    }
                }
                Drill::Fielding => {
                    if up(&mut self.field) {
                        ups.push("field");
                    }
                }
                Drill::Running => {
                    if up(&mut self.speed) {
                        ups.push("speed");
                    }
                }
                Drill::Strength => {
                    if self.pitcher() && self.stamina_p <= self.power && up(&mut self.stamina_p) {
                        ups.push("stamina");
                    } else if up(&mut self.power) {
                        ups.push("power");
                    }
                }
            }
        }
        ups
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
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Team {
    pub players: Vec<Player>,
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
        let mut t = Self { players };
        t.unique_names();
        t
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

    /// How strong the team is (about 30 for a weak club, 70 for a powerhouse).
    pub fn strength(&self) -> f32 {
        let Some(p) = self.ace() else {
            return 0.0;
        };
        let nine = self.lineup(p);
        let bats: f32 = nine
            .iter()
            .map(|&i| self.players[i].bat_rating())
            .sum::<f32>()
            / nine.len().max(1) as f32;
        bats * 0.6 + self.players[p].pitch_rating() * 0.4
    }

    /// March: third-years leave. Returns how many.
    pub fn graduate(&mut self) -> usize {
        let before = self.players.len();
        self.players.retain(|p| p.grade < 3);
        for p in &mut self.players {
            p.grade += 1;
        }
        before - self.players.len()
    }

    /// April: first-years join, more and better with reputation (0 to 100). At least one
    /// pitcher and a catcher if the club needs them.
    pub fn recruit(&mut self, rng: &mut Rng, reputation: f32) -> usize {
        let n = 4 + (reputation / 25.0) as usize;
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
        n
    }
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
    fn training_raises_stats_and_tired_or_hurt_players_learn_less() {
        let mut rng = Rng::new(1);
        let mut a = rookie(&mut rng, 1, Pos::SS, 40.0);
        let speed = a.speed;
        let ups = a.train(Drill::Running, 25.0);
        assert_eq!(ups.len(), 2);
        assert_eq!(a.speed, speed + 2);
        let mut b = a.clone();
        b.stamina = 10;
        b.xp = [0; 5];
        b.train(Drill::Running, 10.0);
        assert_eq!(b.xp[Drill::Running.index()], 5, "half when tired");
        b.hurt = 2;
        assert!(b.train(Drill::Running, 50.0).is_empty());
        b.mood = Mood::Great;
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
        let n = t.recruit(&mut rng, 60.0);
        assert_eq!(n, 6);
        assert!(t.players.iter().filter(|p| p.pitcher()).count() >= 2);
    }
}
