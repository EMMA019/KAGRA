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
    /// Visit a middle school: find a player who joins next April.
    Scout,
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
            (Square::Scout, false) => "Scout",
            (Square::Train(Drill::Batting), true) => "打撃",
            (Square::Train(Drill::Pitching), true) => "投球",
            (Square::Train(Drill::Fielding), true) => "守備",
            (Square::Train(Drill::Running), true) => "走塁",
            (Square::Train(Drill::Strength), true) => "筋力",
            (Square::Rest, true) => "休養",
            (Square::Event, true) => "出来事",
            (Square::Spirit, true) => "気合",
            (Square::Scout, true) => "スカウト",
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
            Square::Scout => [255, 190, 110],
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
        Scout,
        Train(Strength),
        Train(Batting),
        Spirit,
        Train(Running),
        Event,
        Train(Pitching),
    ]
}

/// A card: steps to walk, and maybe a focus that doubles one kind of square this week.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub steps: u8,
    pub focus: Option<Drill>,
    /// A player on the card (family name): that week he trains double.
    #[serde(default)]
    pub star: Option<String>,
}

impl Card {
    pub fn draw(rng: &mut Rng, team: &Team) -> Self {
        let steps = 1 + (rng.next_u32() % 6) as u8;
        let focus = (rng.unit() < 0.25).then(|| Drill::ALL[(rng.next_u32() % 5) as usize]);
        let star = (!team.players.is_empty() && rng.unit() < 0.6).then(|| {
            team.players[(rng.next_u32() as usize) % team.players.len()]
                .family
                .clone()
        });
        Self { steps, focus, star }
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
    /// A talk: one player (`who`) says `text` (`{name}` is his family name) and the coach
    /// answers with one of these. Empty: the event just happens to everyone.
    pub choices: Vec<Choice>,
    /// Who speaks in a talk.
    pub who: Who,
}

/// Who speaks in a talk event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Who {
    #[default]
    Any,
    Pitcher,
    /// A third-year.
    Senior,
    /// A first-year.
    Rookie,
}

/// One answer the coach can give in a talk, and what follows. All original text.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Choice {
    pub text: String,
    pub text_ja: String,
    /// What happens next (`{name}` is the player who spoke).
    pub reply: String,
    pub reply_ja: String,
    /// For the player who spoke.
    pub stamina: i32,
    pub mood: i32,
    pub drill: Option<Drill>,
    pub points: f32,
    /// For the whole club.
    pub team_mood: i32,
    pub team_stamina: i32,
    pub reputation: f32,
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
            choices: Vec::new(),
            who: Who::Any,
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
        talk(
            "night_swing",
            Who::Any,
            "Long after dark, {name} is still swinging in the yard. \"Coach, just a little longer.\"",
            "日が暮れても{name}が素振りを続けている。「監督、もう少しだけ振らせてください」",
            vec![
                Choice {
                    drill: Some(Drill::Batting),
                    points: 14.0,
                    stamina: -15,
                    ..answer(
                        "Stay with him until he is done",
                        "気の済むまで付き合おう",
                        "Two hundred more swings. {name} grins: \"I think I've got it.\"",
                        "さらに二百本。{name}「何かつかめた気がします！」",
                    )
                },
                Choice {
                    stamina: 15,
                    mood: 1,
                    ..answer(
                        "Send him home to rest",
                        "今日はもう休め",
                        "{name} bows and goes home. He looks lighter the next morning.",
                        "{name}は頭を下げて帰った。翌朝、表情が明るい。",
                    )
                },
            ],
        ),
        talk(
            "slump",
            Who::Any,
            "{name} sits alone on the bench. \"Nothing works lately, coach.\"",
            "{name}がベンチでひとり座っている。「最近、何をやってもダメで……」",
            vec![
                Choice {
                    drill: Some(Drill::Batting),
                    points: 8.0,
                    mood: 1,
                    ..answer(
                        "Look at his form together",
                        "一緒にフォームを見直そう",
                        "You find a hitch in his stride. {name} nods slowly.",
                        "踏み込みのクセが見つかった。{name}はゆっくりうなずいた。",
                    )
                },
                Choice {
                    mood: 2,
                    stamina: 5,
                    ..answer(
                        "Tell him everyone has weeks like this",
                        "誰にでもある、と声をかける",
                        "{name} laughs for the first time in days.",
                        "{name}が何日かぶりに笑った。",
                    )
                },
            ],
        ),
        talk(
            "sore_arm",
            Who::Pitcher,
            "{name} rubs his shoulder. \"It's a little tight, but I can throw.\"",
            "{name}が肩をさすっている。「少し張ってますけど、投げられます」",
            vec![
                Choice {
                    stamina: 25,
                    ..answer(
                        "No throwing this week",
                        "今週は投げ込み禁止",
                        "{name} sulks, then ices his arm every day. It feels fresh.",
                        "{name}は少しふてくされたが、毎日しっかり冷やした。肩は軽い。",
                    )
                },
                Choice {
                    drill: Some(Drill::Pitching),
                    points: 9.0,
                    stamina: -10,
                    ..answer(
                        "Light throwing only",
                        "軽めに投げておけ",
                        "Easy throws, good rhythm. {name}'s control is sharper.",
                        "力を抜いた投球でリズムが良くなった。{name}の制球が冴える。",
                    )
                },
            ],
        ),
        talk(
            "captain_talk",
            Who::Senior,
            "{name} stays after practice. \"Coach, the team feels loose. Can we talk?\"",
            "練習のあと{name}が残っていた。「監督、チームの雰囲気が緩んでいる気がします」",
            vec![
                Choice {
                    team_mood: 1,
                    team_stamina: -5,
                    ..answer(
                        "Call a team meeting",
                        "全員でミーティングをしよう",
                        "A long talk in the clubhouse. Everyone leaves with a goal.",
                        "部室で長い話し合い。みんな目標を口にして帰った。",
                    )
                },
                Choice {
                    mood: 2,
                    reputation: 0.5,
                    ..answer(
                        "Tell him he is the one to lead",
                        "お前に任せる",
                        "{name} straightens up. The next day he is first on the field.",
                        "{name}の背筋が伸びた。翌日、一番乗りでグラウンドに来ていた。",
                    )
                },
            ],
        ),
        talk(
            "rookie_nerves",
            Who::Rookie,
            "First-year {name} looks pale. \"The seniors are so good... can I really play here?\"",
            "1年生の{name}が青い顔をしている。「先輩たちすごすぎて……僕、やっていけますか」",
            vec![
                Choice {
                    drill: Some(Drill::Fielding),
                    points: 8.0,
                    mood: 1,
                    ..answer(
                        "Hit him grounders yourself",
                        "自分でノックを打ってやる",
                        "Grounder after grounder. {name} dives for the last one and comes up laughing.",
                        "ノックを何本も。最後の一球に{name}が飛びつき、泥だらけで笑った。",
                    )
                },
                Choice {
                    mood: 2,
                    team_mood: 1,
                    ..answer(
                        "Ask a senior to look after him",
                        "先輩に面倒を見てもらう",
                        "The seniors take him out for noodles. The club feels closer.",
                        "先輩たちが{name}をラーメンに連れていった。部の空気が温かい。",
                    )
                },
            ],
        ),
    ]
}

fn talk(id: &str, who: Who, en: &str, ja: &str, choices: Vec<Choice>) -> Happening {
    Happening {
        id: id.into(),
        text: en.into(),
        text_ja: ja.into(),
        weight: 0.9,
        choices,
        who,
        ..Happening::default()
    }
}

fn answer(en: &str, ja: &str, reply: &str, reply_ja: &str) -> Choice {
    Choice {
        text: en.into(),
        text_ja: ja.into(),
        reply: reply.into(),
        reply_ja: reply_ja.into(),
        ..Choice::default()
    }
}

/// Who speaks in a talk: a healthy player who fits `who` (anyone healthy if none fits).
pub fn speaker(rng: &mut Rng, who: Who, team: &Team) -> Option<usize> {
    let fits = |p: &super::team::Player| match who {
        Who::Any => true,
        Who::Pitcher => p.pitcher(),
        Who::Senior => p.grade == 3,
        Who::Rookie => p.grade == 1,
    };
    let mut pool: Vec<usize> = (0..team.players.len())
        .filter(|&i| team.players[i].hurt == 0 && fits(&team.players[i]))
        .collect();
    if pool.is_empty() {
        pool = (0..team.players.len())
            .filter(|&i| team.players[i].hurt == 0)
            .collect();
    }
    if pool.is_empty() {
        return None;
    }
    Some(pool[(rng.next_u32() as usize) % pool.len()])
}

/// The coach's answer takes effect: on the player who spoke, and on the club.
pub fn apply_choice(c: &Choice, team: &mut Team, who: usize) {
    let shift = |p: &mut super::team::Player, stamina: i32, mood: i32| {
        if stamina > 0 {
            p.rest(stamina as u8);
        } else if stamina < 0 {
            p.tire((-stamina) as u8);
        }
        for _ in 0..mood.max(0) {
            p.mood = p.mood.up();
        }
        for _ in 0..(-mood).max(0) {
            p.mood = p.mood.down();
        }
    };
    for p in &mut team.players {
        shift(p, c.team_stamina, c.team_mood);
    }
    if let Some(p) = team.players.get_mut(who) {
        shift(p, c.stamina, c.mood);
        if let Some(d) = c.drill {
            p.train(d, c.points);
        }
    }
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
    let base = 36.0 + year as f32 * 2.0;
    match f {
        Fixture::Practice => base + 2.0,
        Fixture::Summer(r) => base + r as f32 * 5.0,
        Fixture::National(r) => base + 28.0 + r as f32 * 5.0,
        Fixture::Autumn(r) => base + r as f32 * 4.0,
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

/// A week's training on a square: the whole team works on `d` (double with the card's
/// focus), each player also does his own menu, and the card's star trains double. Returns
/// the points the team gained.
pub fn train_week(team: &mut Team, d: Drill, card: &Card, base: f32) -> u32 {
    let k = if card.focus == Some(d) { 2.0 } else { 1.0 };
    let mut total = 0;
    for p in &mut team.players {
        // Pitchers learn more from pitching, fielders less.
        let fit = match (d, p.pitcher()) {
            (Drill::Pitching, true) => 1.4,
            (Drill::Pitching, false) => 0.6,
            (Drill::Batting, true) => 0.6,
            _ => 1.0,
        };
        let star = if card.star.as_deref() == Some(p.family.as_str()) {
            2.0
        } else {
            1.0
        };
        total += p.train(d, base * k * fit * star) as u32;
        let menu = p.menu;
        total += p.train(menu, base * 0.35 * star) as u32;
        p.tire(12);
        if p.stamina < 20 && p.mood > Mood::Awful {
            p.mood = p.mood.down();
        }
    }
    total
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
            let c = Card::draw(&mut rng, &t);
            assert!((1..=6).contains(&c.steps));
            if let Some(s) = &c.star {
                assert!(t.players.iter().any(|p| &p.family == s));
            }
        }
        assert!(pick_happening(&mut rng, &list).is_some());
        assert_eq!(default_ring().len(), 20);
    }

    #[test]
    fn focus_and_the_star_double_a_week_of_training_and_it_tires() {
        let mut rng = Rng::new(2);
        let a0 = Team::founding(&mut rng);
        let plain = Card {
            steps: 1,
            focus: None,
            star: None,
        };
        let focus = Card {
            focus: Some(Drill::Fielding),
            ..plain.clone()
        };
        let star = Card {
            star: Some(a0.players[3].family.clone()),
            ..plain.clone()
        };
        let mut a = a0.clone();
        let p_plain = train_week(&mut a, Drill::Fielding, &plain, 6.0);
        let mut b = a0.clone();
        let p_focus = train_week(&mut b, Drill::Fielding, &focus, 6.0);
        assert!(p_focus > p_plain, "{p_focus} > {p_plain}");
        let mut c = a0.clone();
        train_week(&mut c, Drill::Fielding, &star, 6.0);
        let gain = |t: &Team, i: usize| t.players[i].xp.iter().sum::<u16>();
        assert!(gain(&c, 3) > gain(&a, 3), "the star trains double");
        assert!(a.players.iter().all(|p| p.stamina == 88));
        // Everyone also did their own menu.
        assert!(a.players.iter().all(|p| p.xp[p.menu.index()] > 0));
    }
}
