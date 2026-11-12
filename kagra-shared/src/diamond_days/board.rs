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
    /// Watch game film together: fielding and batting points, no tired legs.
    Film,
    /// Keep the ground tidy: a little rest, and word travels.
    Grounds,
    /// A practice game against a nearby school this week (in winter, a game among
    /// ourselves: no games with other schools from December to February).
    Game,
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
            (Square::Film, false) => "Film",
            (Square::Grounds, false) => "Grounds",
            (Square::Game, false) => "Game",
            (Square::Train(Drill::Batting), true) => "打撃",
            (Square::Train(Drill::Pitching), true) => "投球",
            (Square::Train(Drill::Fielding), true) => "守備",
            (Square::Train(Drill::Running), true) => "走塁",
            (Square::Train(Drill::Strength), true) => "筋力",
            (Square::Rest, true) => "休養",
            (Square::Event, true) => "出来事",
            (Square::Spirit, true) => "気合",
            (Square::Scout, true) => "スカウト",
            (Square::Film, true) => "映像",
            (Square::Grounds, true) => "整備",
            (Square::Game, true) => "練習試合",
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
            Square::Film => [110, 120, 185],
            Square::Grounds => [180, 150, 105],
            Square::Game => [236, 84, 72],
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
        Game,
        Train(Strength),
        Spirit,
        Train(Batting),
        Event,
        Rest,
        Train(Pitching),
        Train(Fielding),
        Scout,
        Train(Strength),
        Film,
        Game,
        Spirit,
        Train(Running),
        Event,
        Grounds,
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
    /// Only in these months (4 = April .. 3 = March). Empty: any month.
    pub months: Vec<u8>,
    /// Teaches this trait to one player who lacks it (the best at what it needs).
    pub grant: Option<Ability>,
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
            months: Vec::new(),
            grant: None,
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
        Happening {
            drill: Some(Drill::Pitching),
            points: 8.0,
            ..h(
                "bullpen",
                "A college pitcher from the next town throws in the bullpen for the club to watch. Nobody blinks.",
                "近くの町の大学生投手がブルペンで投げて見せてくれた。みんな真剣に見ていた。",
            )
        },
        Happening {
            drill: Some(Drill::Strength),
            points: 8.0,
            stamina: -5,
            ..h(
                "weights",
                "The judo club lends the team its weight room for the week. Everyone feels a size bigger.",
                "一週間、トレーニング室を使わせてもらった。みんな体がひと回り大きくなった気がする。",
            )
        },
        Happening {
            stamina: -12,
            weight: 0.7,
            ..h(
                "heat",
                "The hottest day of the year. Even the cicadas give up by noon.",
                "真夏日。セミの声も聞こえない。",
            )
        },
        Happening {
            stamina: 5,
            mood: 1,
            ..h(
                "stray_dog",
                "A stray dog wanders onto the ground and joins the warm-up run. It beats half the team.",
                "どこかのイヌがグラウンドに入ってきて、準備運動のランニングに混ざった。半分以上の部員が負けた。",
            )
        },
        Happening {
            stamina: 5,
            mood: 1,
            ..h(
                "cherry",
                "The cherry tree behind the ground is in full bloom. Practice keeps stopping to look.",
                "グラウンド裏の桜が満開。練習の手が、つい止まってしまう。",
            )
        },
        Happening {
            mood: 2,
            reputation: 1.0,
            weight: 0.6,
            ..h(
                "uniform",
                "New uniforms arrive, paid for by the supporters' association. The team stands a little taller.",
                "後援会の寄付で新しいユニフォームが届いた。みんな少し背筋が伸びた。",
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
        talk(
            "ace_pressure",
            Who::Pitcher,
            "{name} keeps looking at the number 1 on his back. \"Everyone thinks I have to win it on my own.\"",
            "{name}が背番号1を何度も見ている。「僕ひとりで勝たなきゃいけない気がして……」",
            vec![
                Choice {
                    drill: Some(Drill::Pitching),
                    points: 10.0,
                    stamina: -10,
                    ..answer(
                        "Throw until the doubt is gone",
                        "とことん投げ込め",
                        "Fifty pitches, then fifty more. {name}'s last ball is the best of the day.",
                        "50球、さらに50球。最後の一球が{name}の今日一番だった。",
                    )
                },
                Choice {
                    mood: 2,
                    ..answer(
                        "Tell him nobody wins alone",
                        "野球は一人で勝てない",
                        "{name} laughs, a little embarrassed. \"Right. I knew that.\"",
                        "{name}は照れくさそうに笑った。「……ですよね」",
                    )
                },
            ],
        ),
        talk(
            "bench_chance",
            Who::Any,
            "{name} is always first to practice and last in the game. \"Coach... will my turn come?\"",
            "{name}は誰より早く来て、試合にはなかなか出られない。「監督……僕にも出番、来ますか」",
            vec![
                Choice {
                    drill: Some(Drill::Fielding),
                    points: 8.0,
                    mood: 1,
                    ..answer(
                        "Show me in the field, today",
                        "今日の守備で見せてみろ",
                        "Grounders until dark. {name} does not miss one.",
                        "日が暮れるまでノック。{name}は一球もこぼさなかった。",
                    )
                },
                Choice {
                    mood: 2,
                    team_mood: 1,
                    ..answer(
                        "Promise him the next practice game",
                        "次の練習試合で使うと約束する",
                        "{name}'s face lights up. The whole bench hears, and trains harder.",
                        "{name}の顔がぱっと明るくなった。聞いていたベンチ全員が、練習にも力が入った。",
                    )
                },
            ],
        ),
        talk(
            "family_shop",
            Who::Senior,
            "Third-year {name} bows his head. \"My family's shop is short-handed. I may miss some practices.\"",
            "3年の{name}が頭を下げた。「うちの店が人手不足で……練習、休む日があるかもしれません」",
            vec![
                Choice {
                    stamina: 5,
                    mood: 2,
                    ..answer(
                        "Family comes first, go",
                        "うちのことが先だ、行ってこい",
                        "{name} bows deeper. On the days he comes, he gives everything.",
                        "{name}は深く頭を下げた。来られる日の練習は、誰より集中していた。",
                    )
                },
                Choice {
                    drill: Some(Drill::Strength),
                    points: 8.0,
                    mood: 1,
                    ..answer(
                        "The shop work is training too",
                        "店の荷物運びも筋トレだ",
                        "{name} laughs. Carrying crates all summer does build an arm.",
                        "{name}が笑った。夏の間ずっと荷物を運べば、たしかに腕は太くなる。",
                    )
                },
            ],
        ),
        talk(
            "after_graduation",
            Who::Senior,
            "Third-year {name} asks quietly, \"Coach, can someone like me keep playing baseball after I graduate?\"",
            "3年の{name}が静かに聞いた。「監督、僕みたいな選手でも、卒業してから野球を続けられますか」",
            vec![
                Choice {
                    mood: 2,
                    reputation: 0.5,
                    ..answer(
                        "Talk about the college clubs",
                        "大学の野球部の話をする",
                        "You tell him about the alumni who still play. {name} nods, eyes bright.",
                        "今も大学で投げている先輩の話をした。{name}は目を輝かせてうなずいた。",
                    )
                },
                Choice {
                    mood: 1,
                    team_mood: 1,
                    ..answer(
                        "Baseball does not end at graduation",
                        "野球は卒業で終わりじゃない",
                        "The other third-years are listening. The clubhouse is quiet, in a good way.",
                        "ほかの3年生も聞いていた。部室が静かになった。いい意味で。",
                    )
                },
            ],
        ),
        // ---- round 7: more kinds of days (seasons, school life, the town, the game) ----
        Happening {
            months: vec![6],
            stamina: 10,
            drill: Some(Drill::Strength),
            points: 6.0,
            ..h(
                "rainy_season",
                "The rainy season sets in. A week of stairs and push-ups in the gym corridor.",
                "梅雨入り。一週間、体育館の廊下で階段ダッシュと腕立て伏せ。",
            )
        },
        Happening {
            months: vec![5, 9],
            drill: Some(Drill::Running),
            points: 7.0,
            mood: 1,
            ..h(
                "sports_day",
                "Sports day. The baseball club sweeps the relay, and the whole school cheers.",
                "体育祭。野球部がリレーを総なめにして、学校中が盛り上がった。",
            )
        },
        Happening {
            months: vec![10, 11],
            mood: 1,
            stamina: -5,
            reputation: 1.0,
            ..h(
                "school_festival",
                "The school festival. The club runs a pitching-speed booth; the line goes round the gym.",
                "文化祭。野球部はスピードガン当ての出し物。体育館を一周する行列ができた。",
            )
        },
        Happening {
            months: vec![11],
            stamina: 20,
            mood: 1,
            ..h(
                "school_trip",
                "The second-years go on the school trip. The first-years hold the fort, and everyone comes back rested.",
                "2年生は修学旅行。1年生が留守を守り、みんな元気になって戻ってきた。",
            )
        },
        Happening {
            months: vec![8],
            mood: 1,
            stamina: 10,
            ..h(
                "fireworks",
                "The town fireworks. The club watches from the riverbank, still in their practice shirts.",
                "町の花火大会。部員たちは練習着のまま、河原から見上げた。",
            )
        },
        Happening {
            months: vec![12],
            mood: 1,
            stamina: 10,
            ..h(
                "year_end",
                "The year's last practice ends with rice cakes pounded on the field. Kotori keeps count.",
                "今年最後の練習は、グラウンドでの餅つきで締めた。数えているのはことりだ。",
            )
        },
        Happening {
            months: vec![1, 2],
            drill: Some(Drill::Strength),
            points: 7.0,
            stamina: -8,
            ..h(
                "snow",
                "Snow overnight. The club shovels the whole ground before a single throw. Legs of iron.",
                "夜のうちに雪。投げる前にグラウンド全部を雪かき。足腰が鍛えられた。",
            )
        },
        Happening {
            months: vec![1],
            mood: 1,
            ..h(
                "shrine",
                "New Year at the shrine. Every fortune drawn says \"great luck\", which seems unlikely.",
                "初詣。全員が引いたおみくじが「大吉」。さすがに出来すぎだ。",
            )
        },
        Happening {
            months: vec![7, 8],
            stamina: 12,
            ..h(
                "heat_rest",
                "A heat warning. The coach moves practice to the evening and makes everyone drink.",
                "高温注意報。練習を夕方に移し、全員にしっかり水分をとらせた。",
            )
        },
        Happening {
            drill: Some(Drill::Fielding),
            points: 7.0,
            grant: Some(Ability::Steal),
            weight: 0.6,
            ..h(
                "sprint_clinic",
                "A track coach from the university gives a running clinic: first steps, leans, turns.",
                "大学の陸上部コーチが走塁教室。一歩目、前傾、ベースの回り方。",
            )
        },
        Happening {
            drill: Some(Drill::Batting),
            points: 9.0,
            ..h(
                "batting_center",
                "The batting center by the highway gives the club an evening for free. Thousands of swings.",
                "国道沿いのバッティングセンターが、夜をまるごと貸してくれた。何千本も打ち込んだ。",
            )
        },
        Happening {
            drill: Some(Drill::Fielding),
            points: 8.0,
            ..h(
                "umpire_clinic",
                "A local umpire teaches the rules that win games: tag-ups, infield flies, the obstruction call.",
                "地元の審判さんがルール講習。タッチアップ、インフィールドフライ、走塁妨害。",
            )
        },
        Happening {
            reputation: 2.0,
            mood: 1,
            ..h(
                "kids_clinic",
                "The club runs a clinic for the town's little league. The kids want autographs.",
                "町の少年野球に教えに行った。子どもたちにサインをねだられた。",
            )
        },
        Happening {
            reputation: 3.0,
            mood: 1,
            stamina: -5,
            weight: 0.6,
            ..h(
                "tv_crew",
                "A local TV crew films practice for the evening news. Everyone runs a little faster.",
                "地元テレビの取材。夕方のニュースで流れるらしい。みんな少し足が速くなった。",
            )
        },
        Happening {
            reputation: 1.0,
            stamina: 10,
            weight: 0.7,
            ..h(
                "ob_gift",
                "The alumni association sends new balls and a pitching machine. The old one retires with honours.",
                "OB会から新しいボールとピッチングマシンが届いた。古いマシンは名誉の引退。",
            )
        },
        Happening {
            mood: 1,
            weight: 0.6,
            ..h(
                "pro_scout",
                "A man with a radar gun sits behind the backstop all afternoon. Nobody says the word \"scout\".",
                "スピードガンを持った男性が、午後ずっとバックネット裏にいた。誰も「スカウト」とは口にしない。",
            )
        },
        Happening {
            mood: -1,
            weight: 0.5,
            ..h(
                "broken_bats",
                "Three bats crack in one day. The budget meeting is not a happy one.",
                "一日でバットが3本折れた。部費の会議は暗い雰囲気だった。",
            )
        },
        Happening {
            drill: Some(Drill::Pitching),
            points: 7.0,
            grant: Some(Ability::Pinch),
            weight: 0.5,
            ..h(
                "bullpen_catcher",
                "A retired catcher from the town team catches every pitcher's bullpen and talks them through the jams.",
                "町のチームの元捕手が、投手全員のブルペンを受けて、ピンチの考え方を教えてくれた。",
            )
        },
        Happening {
            drill: Some(Drill::Batting),
            points: 6.0,
            grant: Some(Ability::Eye),
            weight: 0.5,
            ..h(
                "eye_drill",
                "A week of tracking drills: numbers on the balls, called out before they reach the plate.",
                "ボールに書いた数字を、ホームに届く前に読み上げる練習を一週間。",
            )
        },
        Happening {
            stamina: -10,
            drill: Some(Drill::Running),
            points: 9.0,
            ..h(
                "beach_run",
                "Sand running at the beach in the morning mist. Some cry. All finish.",
                "朝もやの砂浜ダッシュ。泣く者もいたが、全員が走りきった。",
            )
        },
        Happening {
            mood: 1,
            reputation: 1.0,
            ..h(
                "rival_visit",
                "A rival school's captain comes by to borrow the ground. He stays to watch, and leaves impressed.",
                "ライバル校の主将がグラウンドを借りに来た。練習を見て、感心して帰っていった。",
            )
        },
        Happening {
            months: vec![2],
            mood: 1,
            ..h(
                "chocolate",
                "Mid-February. A box of chocolates appears in the clubhouse, \"for everyone\". Nobody admits a thing.",
                "2月半ば。部室に「みんなへ」とチョコの箱。誰も何も言わない。",
            )
        },
        Happening {
            months: vec![4],
            mood: 1,
            ..h(
                "new_gloves",
                "The first-years break in their new gloves, oiling and folding them every night.",
                "1年生が新しいグラブの型付けに夢中。毎晩オイルを塗って畳んでいる。",
            )
        },
        talk(
            "quit_thinking",
            Who::Rookie,
            "First-year {name} stays behind with his bag packed. \"Coach... I don't think I'm good enough for this club.\"",
            "1年の{name}が荷物をまとめたまま残っていた。「監督……僕、この部にいてもいいんでしょうか」",
            vec![
                Choice {
                    mood: 2,
                    drill: Some(Drill::Batting),
                    points: 6.0,
                    ..answer(
                        "Stay. Swing with me tomorrow",
                        "残れ。明日、一緒に振ろう",
                        "The next morning {name} is the first one on the field.",
                        "次の朝、グラウンドに一番乗りしたのは{name}だった。",
                    )
                },
                Choice {
                    mood: 1,
                    team_mood: 1,
                    ..answer(
                        "Ask the third-years what they thought at your age",
                        "3年生に同じ頃の話を聞いてこい",
                        "The third-years laugh: every one of them thought the same. {name} stays.",
                        "3年生は笑った。全員が同じことを考えていたらしい。{name}は残ることにした。",
                    )
                },
            ],
        ),
        talk(
            "position_change",
            Who::Any,
            "{name} brings a request: \"Coach, let me try the outfield. My arm wants room.\"",
            "{name}が相談に来た。「監督、外野をやらせてください。肩を思いきり使いたいんです」",
            vec![
                Choice {
                    drill: Some(Drill::Fielding),
                    points: 10.0,
                    mood: 1,
                    ..answer(
                        "Try it this week",
                        "今週やってみろ",
                        "{name} chases fly balls till dark, grinning the whole time.",
                        "{name}は暗くなるまでフライを追いかけた。ずっと笑顔だった。",
                    )
                },
                Choice {
                    drill: Some(Drill::Batting),
                    points: 8.0,
                    ..answer(
                        "First, earn it with your bat",
                        "まずはバットで認めさせろ",
                        "{name} takes it as a challenge and doubles his swings.",
                        "{name}は挑戦と受け取り、素振りを倍にした。",
                    )
                },
            ],
        ),
        talk(
            "exam_or_ball",
            Who::Senior,
            "Third-year {name} looks worried. \"My parents want me at cram school. But the summer...\"",
            "3年の{name}が浮かない顔。「親に塾へ行けって言われて。でも、夏が……」",
            vec![
                Choice {
                    mood: 1,
                    stamina: 15,
                    ..answer(
                        "Do both: mornings here, evenings there",
                        "両方やろう。朝は練習、夜は塾",
                        "{name} keeps both, and sleeps on the bus between them.",
                        "{name}は両方を続け、移動のバスで眠ることにした。",
                    )
                },
                Choice {
                    mood: 2,
                    team_mood: 1,
                    ..answer(
                        "Talk to his parents yourself",
                        "監督から親御さんに話す",
                        "His parents come to the next practice game, and cheer the loudest.",
                        "次の練習試合に親御さんが来て、誰よりも大きな声で応援してくれた。",
                    )
                },
            ],
        ),
        talk(
            "kotori_idea",
            Who::Any,
            "Kotori waves a notebook. \"I've charted every pitch they threw at us. Want to see where we keep missing?\"",
            "ことりがノートを振っている。「打たれた球、全部記録しました。どこを打ち損じてるか見ます？」",
            vec![
                Choice {
                    team_mood: 1,
                    reputation: 0.5,
                    ..answer(
                        "Show the whole club",
                        "部員全員に見せよう",
                        "The club crowds round the notebook. {name} spots his own habit first.",
                        "部員がノートを囲んだ。最初に自分のくせに気づいたのは{name}だった。",
                    )
                },
                Choice {
                    drill: Some(Drill::Batting),
                    points: 10.0,
                    ..answer(
                        "Make a drill out of it",
                        "それで練習メニューを作ろう",
                        "A week of swings at exactly those pitches. {name} goes first.",
                        "その球だけを打つ練習を一週間。最初に打席に入ったのは{name}だった。",
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
#[cfg(test)]
pub fn pick_happening<'a>(rng: &mut Rng, list: &'a [Happening]) -> Option<&'a Happening> {
    pick_happening_in(rng, list, 0)
}

/// [`pick_happening`] among the events that can happen in `month` (0: any event).
pub fn pick_happening_in<'a>(
    rng: &mut Rng,
    list: &'a [Happening],
    month: u8,
) -> Option<&'a Happening> {
    let fits = |h: &&Happening| month == 0 || h.months.is_empty() || h.months.contains(&month);
    let total: f32 = list.iter().filter(fits).map(|h| h.weight.max(0.0)).sum();
    let mut r = rng.unit() * total;
    for h in list.iter().filter(fits) {
        r -= h.weight.max(0.0);
        if r <= 0.0 {
            return Some(h);
        }
    }
    list.iter().rfind(fits)
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
    /// The spring prefectural tournament (April), round 1 to 3. The last four are seeded
    /// in the summer.
    SpringPref(u8),
    /// The summer prefectural tournament, round 1 to 6 (a seed starts in round 2).
    Summer(u8),
    /// The national tournament at the Summer Stadium, round 1 to 5 (if qualified).
    National(u8),
    /// The autumn tournament of the new team, round 1 to 4.
    Autumn(u8),
    /// March invitation, chosen in January off the autumn. Round 1 to 3.
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
    // Two tournaments a year (the summer and the new team's autumn), and the stages they
    // lead to. Practice games are squares on the road (`Square::Game`), not dates.
    match w {
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
            Fixture::SpringPref(_) => 3,
            Fixture::Summer(_) => 6,
            Fixture::National(_) => 5,
            Fixture::Autumn(_) => 4,
            Fixture::Spring(_) => 3,
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

/// How good a rival is in a fixture (the team's strength scale). A famous school
/// draws stronger opposition: the circuit keeps up with the name it has made.
pub fn rival_level(f: Fixture, year: u8, reputation: f32) -> f32 {
    let base = 36.0 + year as f32 * 2.0 + reputation * 0.20;
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
        ("ace_pressure", 0) => Some(Ability::Heavy),
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
        if roll < 0.09 * p.injury_risk() {
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
        // Each tournament starts in its month and runs on as game days from there. No
        // spring prefectural, and no practice games on dates: those are road squares.
        assert_eq!(fixture(4), None);
        assert_eq!(fixture(6), None);
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
        // The summer and the autumn, and the stages they lead to; none in the winter.
        let n = (1..=WEEKS).filter(|w| fixture(*w).is_some()).count();
        assert_eq!(n, 4);
        assert!((1..=WEEKS).all(|w| fixture(w) != Some(Fixture::Practice)));
        assert!(WINTER.clone().all(|w| fixture(w).is_none()));
        assert!(
            rival_level(Fixture::National(1), 1, 0.0) > rival_level(Fixture::Summer(5), 1, 0.0)
        );
        let (en, ja, _) = away_school(3, 1);
        assert!(!en.is_empty() && !ja.is_empty());
        // Rounds and their names.
        assert_eq!(Fixture::Summer(6).round_name().1, "決勝");
        assert_eq!(Fixture::Summer(5).round_name().1, "準決勝");
        assert_eq!(Fixture::Summer(4).round_name().1, "準々決勝");
        assert_eq!(Fixture::Summer(2).round_name().1, "2回戦");
        assert_eq!(Fixture::SpringPref(2).round_name().1, "準決勝");
        assert_eq!(Fixture::SpringPref(3).round_name().1, "決勝");
        // One round fewer in the spring, autumn and March cups (the summer and the
        // national stage keep theirs): a year holds at most 25 games.
        assert_eq!(Fixture::SpringPref(1).rounds(), 3);
        assert_eq!(Fixture::Summer(1).rounds(), 6);
        assert_eq!(Fixture::National(1).rounds(), 5);
        assert_eq!(Fixture::Autumn(1).rounds(), 4);
        assert_eq!(Fixture::Spring(1).rounds(), 3);
        // Tournament games a year at most: the summer (6), the national stage (5), the
        // autumn (4) and March (3). A usual year is far fewer, plus the practice games the
        // road brings.
        let most: u8 = (1..=WEEKS).filter_map(fixture).map(|f| f.rounds()).sum();
        assert_eq!(most, 18);
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
        assert_eq!(default_ring().len(), 22);
    }

    #[test]
    fn the_event_deck_is_fifty_one_and_every_event_is_complete() {
        let list = default_happenings();
        assert_eq!(list.len(), 51);
        let mut ids: Vec<&str> = list.iter().map(|h| h.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 51, "event ids are unique");
        for h in &list {
            assert!(!h.text.is_empty() && !h.text_ja.is_empty(), "{}", h.id);
            assert!(h.weight > 0.0, "{}", h.id);
            for c in &h.choices {
                assert!(!c.text.is_empty() && !c.text_ja.is_empty(), "{}", h.id);
                assert!(!c.reply.is_empty() && !c.reply_ja.is_empty(), "{}", h.id);
            }
        }
        let talks = list.iter().filter(|h| !h.choices.is_empty()).count();
        assert_eq!(talks, 13, "thirteen talks among the singles");
        // Every month has its own events besides the ones that come any time, and an
        // event that teaches does so with a trait some player can learn.
        for m in (1..=12u8).filter(|m| !matches!(m, 3)) {
            assert!(
                list.iter().any(|h| h.months.contains(&m)) || m == 3,
                "month {m}"
            );
        }
        for h in list.iter().filter(|h| h.grant.is_some()) {
            let a = h.grant.unwrap();
            assert!(a.fits(true) || a.fits(false), "{}", h.id);
        }
        // A month draw never picks an event of another month.
        let mut rng2 = Rng::new(9);
        for _ in 0..400 {
            let h = pick_happening_in(&mut rng2, &list, 6).unwrap();
            assert!(
                h.months.is_empty() || h.months.contains(&6),
                "{} in June",
                h.id
            );
        }
        // The bullpen visit feeds the pitching pool, for fielders too.
        let mut rng = Rng::new(3);
        let mut t = Team::founding(&mut rng);
        let bullpen = list.iter().find(|h| h.id == "bullpen").unwrap();
        apply(&mut rng, bullpen, &mut t);
        assert!(t.players.iter().all(|p| p.xp[Drill::Pitching.index()] > 0));
        // The new talk's first answer can teach a heavy ball.
        assert_eq!(choice_grant("ace_pressure", 0), Some(Ability::Heavy));
        assert_eq!(choice_grant("ace_pressure", 1), None);
    }

    #[test]
    fn the_ring_has_two_of_each_drill_and_the_two_new_squares() {
        let ring = default_ring();
        assert_eq!(ring.len(), 22);
        // Two practice-game squares, apart from each other.
        let games: Vec<usize> = (0..ring.len())
            .filter(|&i| ring[i] == Square::Game)
            .collect();
        assert_eq!(games.len(), 2);
        assert!(games[1] - games[0] >= 6, "{games:?}");
        for d in Drill::ALL {
            let n = ring.iter().filter(|s| **s == Square::Train(d)).count();
            assert_eq!(n, 2, "two {d:?} squares");
        }
        assert_eq!(ring.iter().filter(|s| **s == Square::Film).count(), 1);
        assert_eq!(ring.iter().filter(|s| **s == Square::Grounds).count(), 1);
        for sq in [Square::Film, Square::Grounds, Square::Game] {
            assert!(!sq.label(false).is_empty() && !sq.label(true).is_empty());
        }
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
