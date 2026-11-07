//! The week loop's pieces: the ring of squares, the hand of step cards, events, the
//! calendar. No picture, no input.

use serde::{Deserialize, Serialize};

use super::team::{Drill, Mood, Team};
use crate::game_core::swarm::Rng;

/// What a square on the ring does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Square {
    Train(Drill),
    Rest,
    Event,
    Spirit,
}

impl Square {
    pub fn label(self, ja: bool) -> &'static str {
        match (self, ja) {
            (Square::Train(Drill::Batting), false) => "Batting",
            (Square::Train(Drill::Pitching), false) => "Pitching",
            (Square::Train(Drill::Fielding), false) => "Fielding",
            (Square::Train(Drill::Running), false) => "Running",
            (Square::Train(Drill::Strength), false) => "Strength",
            (Square::Rest, false) => "Rest",
            (Square::Event, false) => "Event",
            (Square::Spirit, false) => "Spirit",
            (Square::Train(Drill::Batting), true) => "打撃",
            (Square::Train(Drill::Pitching), true) => "投球",
            (Square::Train(Drill::Fielding), true) => "守備",
            (Square::Train(Drill::Running), true) => "走塁",
            (Square::Train(Drill::Strength), true) => "筋力",
            (Square::Rest, true) => "休養",
            (Square::Event, true) => "出来事",
            (Square::Spirit, true) => "気合",
        }
    }

    pub fn color(self) -> [u32; 3] {
        match self {
            Square::Train(Drill::Batting) => [235, 110, 90],
            Square::Train(Drill::Pitching) => [90, 150, 240],
            Square::Train(Drill::Fielding) => [110, 200, 120],
            Square::Train(Drill::Running) => [240, 200, 80],
            Square::Train(Drill::Strength) => [190, 120, 220],
            Square::Rest => [150, 220, 230],
            Square::Event => [245, 245, 245],
            Square::Spirit => [255, 150, 190],
        }
    }
}

/// The ring of twenty squares round the diamond.
pub fn default_ring() -> Vec<Square> {
    use Drill::*;
    use Square::*;
    vec![
        Rest,
        Train(Batting),
        Train(Fielding),
        Event,
        Train(Running),
        Train(Pitching),
        Train(Strength),
        Spirit,
        Train(Batting),
        Event,
        Rest,
        Train(Pitching),
        Train(Fielding),
        Event,
        Train(Strength),
        Train(Batting),
        Spirit,
        Train(Running),
        Event,
        Train(Pitching),
    ]
}

/// A card: steps to walk, and maybe a focus that doubles one kind of square this week.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub steps: u8,
    pub focus: Option<Drill>,
}

impl Card {
    pub fn draw(rng: &mut Rng) -> Self {
        let steps = 1 + (rng.next_u32() % 6) as u8;
        let focus = (rng.unit() < 0.25).then(|| Drill::ALL[(rng.next_u32() % 5) as usize]);
        Self { steps, focus }
    }
}

/// An event on an event square: what it says and what it does. All original text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Happening {
    pub id: String,
    pub text: String,
    pub text_ja: String,
    /// Stamina for everyone (+ or -).
    pub stamina: i32,
    /// Mood steps for everyone (+ or -).
    pub mood: i32,
    /// Practice points for everyone in one drill.
    pub drill: Option<Drill>,
    pub points: f32,
    /// One random player is hurt this many weeks.
    pub hurt: u8,
    /// Reputation (+ or -).
    pub reputation: f32,
    /// How often, relative to the others.
    pub weight: f32,
}

impl Default for Happening {
    fn default() -> Self {
        Self {
            id: String::new(),
            text: String::new(),
            text_ja: String::new(),
            stamina: 0,
            mood: 0,
            drill: None,
            points: 0.0,
            hurt: 0,
            reputation: 0.0,
            weight: 1.0,
        }
    }
}

pub fn default_happenings() -> Vec<Happening> {
    let h = |id: &str, en: &str, ja: &str| Happening {
        id: id.into(),
        text: en.into(),
        text_ja: ja.into(),
        ..Happening::default()
    };
    vec![
        Happening {
            stamina: 15,
            ..h(
                "drinks",
                "The bakery by the station sends a crate of drinks. Everyone gets their breath back.",
                "駅前のパン屋さんから飲み物の差し入れ。みんなひと息ついた。",
            )
        },
        Happening {
            drill: Some(Drill::Batting),
            points: 8.0,
            ..h(
                "alumnus",
                "A former captain drops by and pitches batting practice all afternoon.",
                "昔のキャプテンが顔を出し、午後いっぱいバッティング投手をしてくれた。",
            )
        },
        Happening {
            mood: 1,
            reputation: 2.0,
            ..h(
                "paper",
                "The town paper runs a small piece on the club. The players read it twice.",
                "町の新聞に野球部の小さな記事が載った。部員たちは二回読んだ。",
            )
        },
        Happening {
            hurt: 2,
            weight: 0.6,
            ..h(
                "ankle",
                "Someone twists an ankle on the wet grass. Two weeks off.",
                "濡れた芝で一人が足首をひねった。二週間の休み。",
            )
        },
        Happening {
            stamina: 5,
            ..h(
                "rain",
                "Rain all week. The team cleans the clubhouse instead.",
                "一週間ずっと雨。代わりに部室を大掃除した。",
            )
        },
        Happening {
            mood: -1,
            weight: 0.7,
            ..h(
                "exams",
                "Exam week. Nobody is thinking about baseball.",
                "テスト週間。誰も野球のことを考えていない。",
            )
        },
        Happening {
            drill: Some(Drill::Running),
            points: 10.0,
            stamina: -10,
            ..h(
                "hill",
                "The coach finds a long hill behind the shrine. Sprints until sunset.",
                "神社の裏に長い坂を見つけた。日が沈むまでダッシュ。",
            )
        },
        Happening {
            mood: 2,
            weight: 0.5,
            ..h(
                "festival",
                "The summer festival! The team helps at a stall and laughs all night.",
                "夏祭り！屋台を手伝って、みんな夜まで笑っていた。",
            )
        },
        Happening {
            drill: Some(Drill::Fielding),
            points: 8.0,
            ..h(
                "knock",
                "One thousand ground balls. Nobody can feel their hands.",
                "ノック千本。誰も手の感覚がない。",
            )
        },
    ]
}

/// Pick an event by weight.
pub fn pick_happening<'a>(rng: &mut Rng, list: &'a [Happening]) -> Option<&'a Happening> {
    let total: f32 = list.iter().map(|h| h.weight.max(0.0)).sum();
    let mut r = rng.unit() * total;
    for h in list {
        r -= h.weight.max(0.0);
        if r <= 0.0 {
            return Some(h);
        }
    }
    list.last()
}

/// Apply an event to the team. Returns the name of a player it hurt.
pub fn apply(rng: &mut Rng, h: &Happening, team: &mut Team) -> Option<String> {
    for p in &mut team.players {
        if h.stamina > 0 {
            p.rest(h.stamina as u8);
        } else {
            p.tire((-h.stamina) as u8);
        }
        for _ in 0..h.mood.max(0) {
            p.mood = p.mood.up();
        }
        for _ in 0..(-h.mood).max(0) {
            p.mood = p.mood.down();
        }
        if let Some(d) = h.drill {
            p.train(d, h.points);
        }
    }
    if h.hurt > 0 && !team.players.is_empty() {
        let i = (rng.next_u32() as usize) % team.players.len();
        team.players[i].hurt = team.players[i].hurt.max(h.hurt);
        return Some(team.players[i].family.clone());
    }
    None
}

/// What the calendar holds for a week (1 to 48).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fixture {
    Practice,
    /// The summer prefectural tournament, round 1 to 5.
    Summer(u8),
    /// The national tournament at the Summer Stadium, round 1 to 4 (if qualified).
    National(u8),
    /// The autumn tournament, round 1 to 3.
    Autumn(u8),
}

pub const WEEKS: u8 = 48;

/// The fixture of week `w`, if any.
pub fn fixture(w: u8) -> Option<Fixture> {
    match w {
        6 | 20 | 36 | 44 => Some(Fixture::Practice),
        13..=15 => Some(Fixture::Summer(w - 12)),
        16 => Some(Fixture::Summer(4)),
        17 => Some(Fixture::Summer(5)),
        18..=19 => Some(Fixture::National(w - 17)),
        25..=27 => Some(Fixture::Autumn(w - 24)),
        _ => None,
    }
}

/// The month of week `w` (April = 4 ... March = 3).
pub fn month(w: u8) -> u8 {
    let m = 4 + (w.saturating_sub(1) / 4);
    if m > 12 {
        m - 12
    } else {
        m
    }
}

/// How good a rival is in a fixture (the team's strength scale).
pub fn rival_level(f: Fixture, year: u8) -> f32 {
    let base = 32.0 + year as f32 * 1.5;
    match f {
        Fixture::Practice => base + 2.0,
        Fixture::Summer(r) => base + r as f32 * 4.0,
        Fixture::National(r) => base + 22.0 + r as f32 * 4.0,
        Fixture::Autumn(r) => base + r as f32 * 3.5,
    }
}

/// A rival school's name for a fixture (made up; picked by week and year).
pub fn rival_name(f: Fixture, year: u8, week: u8) -> (String, String) {
    const EN: [&str; 12] = [
        "Kitahama",
        "Minamino",
        "Sakuragaoka",
        "Higashiyama",
        "Nishikawa",
        "Asahidai",
        "Midorigaoka",
        "Kawabe",
        "Shiroishi",
        "Hoshimi",
        "Tsukiyama",
        "Aokita",
    ];
    const JA: [&str; 12] = [
        "北浜",
        "南野",
        "桜ヶ丘",
        "東山",
        "西川",
        "旭台",
        "緑ヶ丘",
        "川辺",
        "白石",
        "星見",
        "月山",
        "青北",
    ];
    let tier = match f {
        Fixture::National(_) => 6,
        _ => 0,
    };
    let i = ((week as usize) * 7 + (year as usize) * 5 + tier) % EN.len();
    let suffix_en = if matches!(f, Fixture::National(_)) {
        " Academy"
    } else {
        " High"
    };
    let suffix_ja = if matches!(f, Fixture::National(_)) {
        "学園"
    } else {
        "高校"
    };
    (
        format!("{}{suffix_en}", EN[i]),
        format!("{}{suffix_ja}", JA[i]),
    )
}

/// A week's training on a square for everyone, with a card's focus. Returns how many stats
/// went up.
pub fn train_week(team: &mut Team, d: Drill, focus: Option<Drill>, base: f32) -> usize {
    let k = if focus == Some(d) { 2.0 } else { 1.0 };
    let mut ups = 0;
    for p in &mut team.players {
        // Pitchers learn more from pitching, fielders less.
        let fit = match (d, p.pitcher()) {
            (Drill::Pitching, true) => 1.4,
            (Drill::Pitching, false) => 0.6,
            (Drill::Batting, true) => 0.6,
            _ => 1.0,
        };
        ups += p.train(d, base * k * fit).len();
        p.tire(12);
        if p.stamina < 20 && p.mood > Mood::Awful {
            p.mood = p.mood.down();
        }
    }
    ups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_calendar_has_the_tournaments_in_their_months() {
        assert_eq!(fixture(13), Some(Fixture::Summer(1)));
        assert_eq!(fixture(17), Some(Fixture::Summer(5)));
        assert_eq!(fixture(18), Some(Fixture::National(1)));
        assert_eq!(fixture(26), Some(Fixture::Autumn(2)));
        assert_eq!(fixture(7), None);
        assert_eq!(month(1), 4);
        assert_eq!(month(13), 7);
        assert_eq!(month(48), 3);
        let n = (1..=WEEKS).filter(|w| fixture(*w).is_some()).count();
        assert_eq!(n, 4 + 5 + 2 + 3);
        assert!(rival_level(Fixture::National(1), 1) > rival_level(Fixture::Summer(5), 1));
        let (en, ja) = rival_name(Fixture::Summer(1), 1, 13);
        assert!(en.ends_with("High") && ja.ends_with("高校"));
    }

    #[test]
    fn events_change_the_team_and_cards_have_steps() {
        let mut rng = Rng::new(4);
        let mut t = Team::founding(&mut rng);
        let list = default_happenings();
        let ankle = list.iter().find(|h| h.id == "ankle").unwrap();
        let who = apply(&mut rng, ankle, &mut t).unwrap();
        assert!(t.players.iter().any(|p| p.family == who && p.hurt == 2));
        let fest = list.iter().find(|h| h.id == "festival").unwrap();
        apply(&mut rng, fest, &mut t);
        assert!(t.players.iter().all(|p| p.mood >= Mood::Good));
        for _ in 0..50 {
            let c = Card::draw(&mut rng);
            assert!((1..=6).contains(&c.steps));
        }
        assert!(pick_happening(&mut rng, &list).is_some());
        assert_eq!(default_ring().len(), 20);
    }

    #[test]
    fn focus_doubles_a_week_of_training_and_it_tires() {
        let mut rng = Rng::new(2);
        let mut a = Team::founding(&mut rng);
        let mut b = a.clone();
        let plain = train_week(&mut a, Drill::Fielding, None, 6.0);
        let doubled = train_week(&mut b, Drill::Fielding, Some(Drill::Fielding), 6.0);
        assert!(doubled > plain, "{doubled} > {plain}");
        assert!(a.players.iter().all(|p| p.stamina == 88));
    }
}
