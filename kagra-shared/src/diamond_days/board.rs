//! The week loop's pieces: the ring of squares, the hand of step cards, events, the
//! calendar. No picture, no input.

use serde::{Deserialize, Serialize};

use super::team::{Ability, Drill, Mood, Team};
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
        let open: Vec<usize> = (0..team.players.len())
            .filter(|&i| !team.players[i].has(Ability::Durable))
            .collect();
        let (i, weeks) = if open.is_empty() {
            ((rng.next_u32() as usize) % team.players.len(), 1)
        } else {
            (open[(rng.next_u32() as usize) % open.len()], h.hurt)
        };
        team.players[i].hurt = team.players[i].hurt.max(weeks);
        return Some(team.players[i].family.clone());
    }
    None
}

/// What the calendar holds for a week (1 to 48). A tournament (every variant but
/// `Practice`) is played as one stretch of game days from its first week: the board
/// waits, round follows round, and the calendar moves to [`cup_end`] when it is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fixture {
    Practice,
    /// The spring prefectural tournament (April), round 1 to 4. The last four are seeded
    /// in the summer.
    SpringPref(u8),
    /// The summer prefectural tournament, round 1 to 6 (a seed starts in round 2).
    Summer(u8),
    /// The national tournament at the Summer Stadium, round 1 to 5 (if qualified).
    National(u8),
    /// The autumn tournament of the new team, round 1 to 5.
    Autumn(u8),
    /// March invitation, chosen in January off the autumn. Round 1 to 4.
    Spring(u8),
}

pub const WEEKS: u8 = 48;
/// Late June: a training camp, not a square on the ring.
pub const CAMP_WEEK: u8 = 11;
/// The end of June: the numbers for the summer, and the draw.
pub const NUMBERS_WEEK: u8 = 12;
/// October: the pro draft (a retired third-year may be picked).
pub const DRAFT_WEEK: u8 = 27;
/// December to February: no games with other schools; winter training counts for more.
pub const WINTER: std::ops::RangeInclusive<u8> = 33..=44;
/// January: the day the March invitations are announced.
pub const SELECTION_WEEK: u8 = 37;
/// January: exam week. A red mark benches a player the way an injury does.
pub const EXAM_WEEK: u8 = 40;

/// The fixture that starts in week `w`, if any (a tournament's first round).
pub fn fixture(w: u8) -> Option<Fixture> {
    match w {
        4 => Some(Fixture::SpringPref(1)),
        6 | 10 | 22 | 30 => Some(Fixture::Practice),
        13 => Some(Fixture::Summer(1)),
        17 => Some(Fixture::National(1)),
        24 => Some(Fixture::Autumn(1)),
        45 => Some(Fixture::Spring(1)),
        _ => None,
    }
}

impl Fixture {
    /// The round (1 for a practice game).
    pub fn round(self) -> u8 {
        match self {
            Fixture::Practice => 1,
            Fixture::SpringPref(r)
            | Fixture::Summer(r)
            | Fixture::National(r)
            | Fixture::Autumn(r)
            | Fixture::Spring(r) => r,
        }
    }

    /// How many rounds the tournament has (1 for a practice game).
    pub fn rounds(self) -> u8 {
        match self {
            Fixture::Practice => 1,
            Fixture::SpringPref(_) => 4,
            Fixture::Summer(_) => 6,
            Fixture::National(_) => 5,
            Fixture::Autumn(_) => 5,
            Fixture::Spring(_) => 4,
        }
    }

    /// The same tournament at round `r`.
    pub fn at(self, r: u8) -> Fixture {
        match self {
            Fixture::Practice => Fixture::Practice,
            Fixture::SpringPref(_) => Fixture::SpringPref(r),
            Fixture::Summer(_) => Fixture::Summer(r),
            Fixture::National(_) => Fixture::National(r),
            Fixture::Autumn(_) => Fixture::Autumn(r),
            Fixture::Spring(_) => Fixture::Spring(r),
        }
    }

    /// The next round, if this is not the final (or a practice game).
    pub fn next(self) -> Option<Fixture> {
        (self.is_cup() && self.round() < self.rounds()).then(|| self.at(self.round() + 1))
    }

    pub fn is_cup(self) -> bool {
        self != Fixture::Practice
    }

    pub fn is_final(self) -> bool {
        self.is_cup() && self.round() == self.rounds()
    }

    /// The tournament's name, both languages.
    pub fn cup_name(self) -> (&'static str, &'static str) {
        match self {
            Fixture::Practice => ("Practice game", "練習試合"),
            Fixture::SpringPref(_) => ("Spring prefectural", "春季県大会"),
            Fixture::Summer(_) => ("Summer prefectural", "夏の県大会"),
            Fixture::National(_) => ("Summer Stadium", "夏の大舞台"),
            Fixture::Autumn(_) => ("Autumn tournament", "秋季大会"),
            Fixture::Spring(_) => ("Blossom Invitational", "春の招待舞台"),
        }
    }

    /// The round's name: the final, the semifinal, the quarterfinal, or "round 2".
    pub fn round_name(self) -> (String, String) {
        let (r, n) = (self.round(), self.rounds());
        match n - r {
            _ if !self.is_cup() => (String::new(), String::new()),
            0 => ("final".into(), "決勝".into()),
            1 => ("semifinal".into(), "準決勝".into()),
            2 if n >= 5 => ("quarterfinal".into(), "準々決勝".into()),
            _ => (format!("round {r}"), format!("{r}回戦")),
        }
    }
}

/// The week the calendar moves to when a tournament that started in its first week is
/// over (lost, or won outright). The national tournament follows the summer one.
pub fn cup_end(f: Fixture) -> u8 {
    match f {
        Fixture::Practice => 0,
        Fixture::SpringPref(_) => 5,
        Fixture::Summer(_) => 16,
        Fixture::National(_) => 19,
        Fixture::Autumn(_) => 27,
        Fixture::Spring(_) => 47,
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
        Fixture::SpringPref(r) => base + r as f32 * 4.0,
        Fixture::Summer(r) => base + r as f32 * 4.5,
        Fixture::National(r) => base + 26.0 + r as f32 * 4.5,
        Fixture::Autumn(r) => base + r as f32 * 4.0,
        Fixture::Spring(r) => base + 8.0 + r as f32 * 4.0,
    }
}

/// A school from another prefecture (the national stage, the March invitation): name,
/// both languages, and its colour. `n` picks one of 24; the names are made up.
pub fn away_school(n: usize, year: u8) -> (String, String, [u8; 3]) {
    const ROWS: [(&str, &str, [u8; 3]); 24] = [
        ("Aoba Gakuin", "青葉学院", [30, 120, 70]),
        ("Uminari Academy", "海鳴学園", [30, 80, 160]),
        ("Shirakaba High", "白樺高校", [90, 90, 110]),
        ("Koryo Academy", "紅陵学園", [170, 30, 50]),
        ("Hoohkan High", "鳳凰館高校", [200, 140, 30]),
        ("Tachibana Commercial", "立花商業", [40, 100, 60]),
        ("Hokurei High", "北嶺高校", [60, 70, 140]),
        ("Nishikigaoka High", "錦ヶ丘高校", [150, 60, 120]),
        ("Kaiyo Tech", "海洋工業", [20, 110, 150]),
        ("Seiran Academy", "青嵐学園", [40, 90, 170]),
        ("Asagiri High", "朝霧高校", [110, 120, 130]),
        ("Hyuga Daiichi", "日陽第一", [210, 90, 40]),
        ("Kurenai Gakuin", "紅学院", [180, 40, 70]),
        ("Mutsumi High", "睦美高校", [90, 140, 60]),
        ("Ishizuchi Tech", "石鎚工業", [80, 80, 90]),
        ("Sazanami Academy", "細波学園", [40, 140, 170]),
        ("Kogane High", "黄金高校", [200, 160, 40]),
        ("Tokiwa Gakuin", "常盤学院", [30, 110, 90]),
        ("Hayate Commercial", "疾風商業", [60, 60, 160]),
        ("Kirishima High", "霧島高校", [120, 90, 150]),
        ("Ryuo Academy", "竜王学園", [140, 30, 30]),
        ("Ginga High", "銀河高校", [50, 50, 110]),
        ("Matsukaze High", "松風高校", [60, 120, 80]),
        ("Kagaribi Gakuin", "篝火学院", [210, 80, 30]),
    ];
    let (en, ja, c) = ROWS[(n + year as usize * 5) % ROWS.len()];
    (en.into(), ja.into(), c)
}

/// A school on the circuit. The same ten stay for the whole run, and grow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct School {
    pub name: String,
    pub name_ja: String,
    pub color: [u8; 3],
    /// Seed for the captain's face.
    pub face: String,
    pub strength: f32,
    /// Our record against them.
    #[serde(default)]
    pub wins: u16,
    #[serde(default)]
    pub losses: u16,
    #[serde(default)]
    pub met: u16,
}

/// Which school is on the schedule for a week, so next year is a rematch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slate {
    pub chapter: u8,
    pub year: u8,
    pub week: u8,
    pub rival: u8,
}

/// Ten original schools. The last two are academies the nationals tend to send.
pub fn rival_schools() -> Vec<School> {
    let rows: [(&str, &str, [u8; 3], f32); 10] = [
        ("Kitahama High", "北浜高校", [176, 48, 52], 34.0),
        ("Minamino High", "南野高校", [24, 120, 100], 38.0),
        ("Sakuragaoka High", "桜ヶ丘高校", [210, 120, 30], 42.0),
        ("Higashiyama High", "東山高校", [110, 60, 150], 46.0),
        ("Nishikawa High", "西川高校", [30, 130, 170], 50.0),
        ("Asahidai High", "旭台高校", [60, 64, 80], 54.0),
        ("Midorigaoka High", "緑ヶ丘高校", [46, 130, 64], 58.0),
        ("Kawabe High", "川辺高校", [200, 70, 120], 63.0),
        ("Shiroishi Academy", "白石学園", [150, 110, 40], 68.0),
        ("Hoshimi Academy", "星見学園", [40, 70, 140], 74.0),
    ];
    rows.into_iter()
        .map(|(en, ja, color, strength)| School {
            name: en.into(),
            name_ja: ja.into(),
            color,
            face: ja.into(),
            strength,
            wins: 0,
            losses: 0,
            met: 0,
        })
        .collect()
}

/// Fame stars, 1 to 5 (the bar shows them).
pub fn fame_stars(reputation: f32) -> u8 {
    (reputation / 20.0).ceil().clamp(1.0, 5.0) as u8
}

/// What the region calls the school, by fame stars. Original names.
pub fn rank_title(reputation: f32) -> (&'static str, &'static str) {
    match fame_stars(reputation) {
        1 => ("Unknown", "無名校"),
        2 => ("Noticed", "注目校"),
        3 => ("Contender", "実力校"),
        4 => ("Regular", "常連校"),
        _ => ("Legend", "伝説校"),
    }
}

/// Grounds level from reputation. It only ever rises, and a move to a new school resets it.
pub fn grounds_of(reputation: f32) -> u8 {
    if reputation >= 75.0 {
        3
    } else if reputation >= 45.0 {
        2
    } else if reputation >= 20.0 {
        1
    } else {
        0
    }
}

/// A talk answer that can teach a trait. Not stored in the happening JSON.
pub fn choice_grant(id: &str, k: usize) -> Option<Ability> {
    match (id, k) {
        ("night_swing", 0) => Some(Ability::Chance),
        ("slump", 0) => Some(Ability::Clutch),
        ("sore_arm", 0) => Some(Ability::Durable),
        ("captain_talk", 1) => Some(Ability::Mood),
        ("rookie_nerves", 1) => Some(Ability::Eye),
        ("camp_night", 0) => Some(Ability::Steal),
        ("camp_night", 1) => Some(Ability::Eye),
        _ => None,
    }
}

/// The optional conversation at summer camp. Weight 0: the ring never draws it.
pub fn camp_talk() -> Happening {
    talk(
        "camp_night",
        Who::Any,
        "Lights out at camp, and {name} is still sitting on the steps. \"Coach, got a minute?\"",
        "合宿の消灯後、{name}が階段に残っていた。「監督、少しいいですか」",
        vec![
            Choice {
                drill: Some(Drill::Running),
                points: 12.0,
                stamina: -8,
                ..answer(
                    "Take a lap with him",
                    "一緒にグラウンドを一周しよう",
                    "You run in the dark. {name} times the last stretch and grins.",
                    "暗いグラウンドを走った。{name}は最後の直線を計って笑った。",
                )
            },
            Choice {
                mood: 2,
                team_mood: 1,
                ..answer(
                    "Sit and talk",
                    "座って話を聞こう",
                    "{name} talks about the summer. He sleeps better than he has in weeks.",
                    "{name}は夏のことを話した。何週間ぶりかにぐっすり眠れたようだ。",
                )
            },
        ],
    )
}

/// What a camp week did.
pub struct CampReport {
    pub points: u32,
    pub hurt: Vec<String>,
    pub shrugged: Vec<String>,
}

/// Extra work in every drill, one dose of fatigue, and a real chance of a knock.
pub fn camp_week(team: &mut Team, rng: &mut Rng, base: f32) -> CampReport {
    let mut points = 0;
    for p in &mut team.players {
        if p.hurt > 0 {
            continue;
        }
        for d in Drill::ALL {
            let fit = match (d, p.pitcher()) {
                (Drill::Pitching, true) | (Drill::Batting, false) => 1.15,
                (Drill::Pitching, false) | (Drill::Batting, true) => 0.55,
                _ => 0.9,
            };
            points += p.train(d, base * 0.55 * fit) as u32;
        }
        p.tire(18);
    }
    let mut hurt = Vec::new();
    let mut shrugged = Vec::new();
    let tough: Vec<bool> = team
        .players
        .iter()
        .map(|p| p.has(Ability::Durable))
        .collect();
    let names: Vec<String> = team.players.iter().map(|p| p.family.clone()).collect();
    for (i, p) in team.players.iter_mut().enumerate() {
        if p.hurt > 0 {
            continue;
        }
        let roll = rng.unit();
        if roll < 0.09 {
            if tough[i] {
                shrugged.push(names[i].clone());
            } else {
                // Two: the next week's heal leaves one, so he sits out the summer opener
                // (week 13), the game the camp risks him for.
                p.hurt = p.hurt.max(2);
                hurt.push(names[i].clone());
            }
        }
    }
    CampReport {
        points,
        hurt,
        shrugged,
    }
}

/// Exam week: the worst-conditioned players who are not durable pick up a red mark.
/// At least one, when anybody can.
pub fn red_marks(rng: &mut Rng, team: &mut Team) -> Vec<String> {
    let mut order: Vec<usize> = (0..team.players.len()).collect();
    order.sort_by_key(|&i| (team.players[i].mood, i));
    let mut out = Vec::new();
    for i in &order {
        if out.len() >= 2 {
            break;
        }
        let p = &team.players[*i];
        if p.has(Ability::Durable) || p.study > 0 || p.hurt > 0 {
            continue;
        }
        let chance = match p.mood {
            Mood::Awful => 0.9,
            Mood::Poor => 0.7,
            Mood::Normal => 0.45,
            Mood::Good => 0.22,
            Mood::Great => 0.08,
        };
        if rng.unit() < chance {
            team.players[*i].study = 2;
            out.push(team.players[*i].family.clone());
        }
    }
    if out.is_empty() {
        if let Some(i) = order.into_iter().find(|&i| {
            let p = &team.players[i];
            !p.has(Ability::Durable) && p.study == 0 && p.hurt == 0
        }) {
            team.players[i].study = 2;
            out.push(team.players[i].family.clone());
        }
    }
    out
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
        // Each tournament starts in its month and runs on as game days from there.
        assert_eq!(fixture(4), Some(Fixture::SpringPref(1)));
        assert_eq!(fixture(13), Some(Fixture::Summer(1)));
        assert_eq!(fixture(14), None);
        assert_eq!(fixture(17), Some(Fixture::National(1)));
        assert_eq!(fixture(24), Some(Fixture::Autumn(1)));
        assert_eq!(fixture(45), Some(Fixture::Spring(1)));
        assert_eq!(fixture(7), None);
        assert_eq!(fixture(CAMP_WEEK), None);
        assert_eq!(fixture(NUMBERS_WEEK), None);
        const { assert!(CAMP_WEEK < NUMBERS_WEEK && NUMBERS_WEEK < 13) };
        assert_eq!(month(1), 4);
        assert_eq!(month(13), 7);
        assert_eq!(month(45), 3);
        assert_eq!(month(48), 3);
        // Four practice games and five tournaments, none in the winter.
        let n = (1..=WEEKS).filter(|w| fixture(*w).is_some()).count();
        assert_eq!(n, 4 + 5);
        assert!(WINTER.clone().all(|w| fixture(w).is_none()));
        assert!(rival_level(Fixture::National(1), 1) > rival_level(Fixture::Summer(5), 1));
        let (en, ja, _) = away_school(3, 1);
        assert!(!en.is_empty() && !ja.is_empty());
        // Rounds and their names.
        assert_eq!(Fixture::Summer(6).round_name().1, "決勝");
        assert_eq!(Fixture::Summer(5).round_name().1, "準決勝");
        assert_eq!(Fixture::Summer(4).round_name().1, "準々決勝");
        assert_eq!(Fixture::Summer(2).round_name().1, "2回戦");
        assert_eq!(Fixture::SpringPref(3).round_name().1, "準決勝");
        assert_eq!(Fixture::Summer(5).next(), Some(Fixture::Summer(6)));
        assert_eq!(Fixture::Summer(6).next(), None);
        assert!(Fixture::National(5).is_final() && !Fixture::Practice.is_cup());
        // The national tournament starts after the summer one is over.
        assert!(cup_end(Fixture::Summer(1)) < 17 && fixture(17) == Some(Fixture::National(1)));
        let schools = rival_schools();
        assert_eq!(schools.len(), 10);
        assert!(schools.iter().any(|s| s.name_ja.ends_with("学園")));
    }

    #[test]
    fn camp_trains_harder_and_a_red_mark_benches_someone() {
        let mut rng = Rng::new(8);
        let mut t = Team::founding(&mut rng);
        for p in &mut t.players {
            p.abilities.retain(|a| *a != Ability::Durable);
        }
        let before: u16 = t.players.iter().map(|p| p.xp.iter().sum::<u16>()).sum();
        let report = camp_week(&mut t, &mut rng, 7.0);
        let after: u16 = t.players.iter().map(|p| p.xp.iter().sum::<u16>()).sum();
        assert!(after > before);
        assert!(report.points > 0);
        // A camp knock outlasts the next week's heal: he misses the summer opener.
        let mut knocked = None;
        for seed in 0..200 {
            let mut rng = Rng::new(seed);
            let mut t = Team::founding(&mut rng);
            for p in &mut t.players {
                p.abilities.retain(|a| *a != Ability::Durable);
            }
            let r = camp_week(&mut t, &mut rng, 7.0);
            if let Some(n) = r.hurt.first() {
                knocked = t.players.iter().find(|p| &p.family == n).map(|p| p.hurt);
                break;
            }
        }
        let hurt = knocked.expect("some seed knocks someone");
        assert!(
            hurt.saturating_sub(1) > 0,
            "still out after one heal: {hurt}"
        );
        let mut t = Team::founding(&mut rng);
        for p in &mut t.players {
            p.abilities.retain(|a| *a != Ability::Durable);
            p.mood = Mood::Awful;
        }
        let marked = red_marks(&mut rng, &mut t);
        assert!(!marked.is_empty());
        assert!(t.players.iter().any(|p| p.study == 2 && !p.can_play()));
        let mut tough = Team::founding(&mut rng);
        for p in &mut tough.players {
            p.abilities.clear();
            p.grant_force(Ability::Durable);
        }
        assert!(red_marks(&mut rng, &mut tough).is_empty());
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
