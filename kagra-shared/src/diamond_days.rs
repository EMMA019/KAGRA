//! 白球デイズ (Diamond Days): an original high-school baseball manager. Three years as the
//! coach of a small club: pick a step card each week, walk the ring of practice squares,
//! play practice games and tournaments by giving orders, watch players graduate and
//! first-years join. Design: `docs/design/diamond_days.md`.
//!
//! - `team.rs`: players, growth, the line-up, graduation and recruits.
//! - `board.rs`: the ring, the cards, events, the calendar.
//! - `sim.rs`: the game, plate appearance by plate appearance.
//! - `hud.rs`, `doc.rs`: the HUD and the 3D picture.

mod art;
mod board;
mod doc;
mod hud;
mod sim;
mod sprites;
pub mod team;
#[cfg(test)]
mod tests;

use std::cell::Cell;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::flow::{FlowPhase, GameFlow};
use crate::game_core::swarm::Rng;
use crate::game_core::{tick_frame, Core, Kit};
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::DrawList;
use crate::world_doc::WorldDoc;
use board::{Card, Fixture, Happening, School, Slate, Square};
use sim::{Game, Side, Spot, Tactic};
use team::{Ability, Drill, Mood, Role, Team};

/// The `genre` of this kit's world file (and the `game.id` default).
pub const GAME_ID: &str = "diamond_days";

/// True for a world file whose `genre` is this kit's.
pub fn is_diamond_days_dump(json: &str) -> bool {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|v| {
            v.get("genre")
                .and_then(|g| g.as_str())
                .map(|s| s == GAME_ID)
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------- spec ----

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DiamondDaysSpec {
    pub id: String,
    pub title: String,
    pub school: String,
    pub school_ja: String,
    /// Years in a run.
    pub years: u8,
    pub ring: Vec<Square>,
    pub happenings: Vec<Happening>,
    /// Practice points a training square gives (before focus and condition).
    pub train_points: f32,
    /// Stamina a rest square gives back, and every week on its own.
    pub rest: u8,
    pub weekly_rest: u8,
    /// Seconds per play-by-play line in a game, and per square while walking.
    pub line_secs: f32,
    pub step_secs: f32,
    pub seed: u32,
    pub audio: Option<Value>,
}

impl Default for DiamondDaysSpec {
    fn default() -> Self {
        Self {
            id: GAME_ID.into(),
            title: "Diamond Days".into(),
            school: "Hinata High".into(),
            school_ja: "日向高校".into(),
            years: 3,
            ring: board::default_ring(),
            happenings: board::default_happenings(),
            train_points: 7.0,
            rest: 40,
            weekly_rest: 8,
            line_secs: 0.7,
            step_secs: 0.22,
            seed: 11,
            audio: Some(default_audio()),
        }
    }
}

impl DiamondDaysSpec {
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let spec: Self =
            serde_json::from_value(v.clone()).map_err(|e| format!("diamond_days spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(1..=6).contains(&self.years) {
            return Err("diamond_days: years must be 1..=6".into());
        }
        if !(8..=40).contains(&self.ring.len()) {
            return Err("diamond_days: the ring needs 8..=40 squares".into());
        }
        if self.happenings.is_empty() && self.ring.contains(&Square::Event) {
            return Err("diamond_days: event squares need happenings".into());
        }
        if !(1.0..=30.0).contains(&self.train_points) {
            return Err("diamond_days: train_points must be 1..=30".into());
        }
        if !(0.05..=3.0).contains(&self.line_secs) || !(0.05..=1.0).contains(&self.step_secs) {
            return Err("diamond_days: line_secs / step_secs out of range".into());
        }
        Ok(())
    }
}

fn default_audio() -> Value {
    let sfx = "assets/audio/oga-retro-synth-sfx/";
    let cue =
        |f: &str, v: f64, r: f64| json!({"file": format!("{sfx}{f}"), "volume": v, "rate": r});
    let ui = |v: f64, r: f64| json!({"file": "assets/audio/kenney-interface-sounds/confirmation_001.wav", "volume": v, "rate": r});
    let bat = |v: f64, r: f64| json!({"file": "assets/audio/kenney-impact-sounds/impactGeneric_light_001.wav", "volume": v, "rate": r});
    json!({
        "enabled": true,
        "master": 0.8,
        "sfx_volume": 1.0,
        "bgm_volume": 0.5,
        "bgm": {"file": "assets/audio/oga-flowerbed-fields/flowerbed_fields.mp3", "volume": 0.45, "fade_secs": 0.5},
        "sfx": {
            "go": ui(0.6, 1.0),
            "card": ui(0.4, 1.3),
            "step": ui(0.2, 1.8),
            "train": cue("power_up_02.wav", 0.45, 1.2),
            "event": ui(0.5, 0.8),
            "hit": bat(0.6, 1.1),
            "homer": cue("power_up_04.wav", 0.6, 1.1),
            "out": bat(0.3, 0.7),
            "win": cue("power_up_04.wav", 0.8, 1.0),
            "lose": cue("retro_die_03.wav", 0.5, 1.0)
        }
    })
}

// ----------------------------------------------------------------- run ----

/// Where the week is.
#[derive(Clone, Debug, PartialEq)]
enum Mode {
    /// Pick a card.
    Plan,
    /// The coach walks: squares left to walk, seconds to the next hop, the card.
    Walk { left: u8, t: f32, card: Card },
    /// A card of text; a tap goes on.
    Notice,
    /// A player talks to the coach (event `h`, player `who`); the coach picks an answer.
    Ask { h: usize, who: usize },
    /// A game in progress: lines shown, seconds to the next, waiting for an order.
    Match {
        shown: usize,
        t: f32,
        asking: Option<Spot>,
    },
    /// One player's page: points, stats to raise, the practice menu.
    Player(usize),
    /// The roster list.
    Roster,
    /// Batting order and positions. `pick` is the slot waiting for a player.
    Lineup { pick: Option<usize> },
    /// Summer camp, one player still wants a word.
    CampTalk { who: usize },
    /// Three years are over. `schools` lists places that would take the coach.
    Epilogue { schools: bool },
    /// Game day: the tournament, both schools, our order. The coach starts the game.
    MatchIntro,
    /// After the whistle: the score, the player of the game, what the club learned.
    MatchEnd,
    /// A tournament: our path through the draw, and how to spend the day before the
    /// next game (rest, tune up, study them).
    Bracket,
    /// The coach decides something for the club (the ace's number, the captain).
    Decide(Decide),
}

/// What the coach is asked to decide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decide {
    /// Who wears number 1 this summer.
    Ace,
    /// Who captains the new team.
    Captain,
}

/// What the result screen shows after a game.
#[derive(Clone, Debug, PartialEq)]
struct Report {
    won: bool,
    title: (String, String),
    /// [visitors, home] names, both languages, and which one is us.
    names: [(String, String); 2],
    us: usize,
    line: [Vec<u8>; 2],
    hits: [u16; 2],
    /// The player of the game (ours): family name and the line under him.
    star: Option<(String, String, String)>,
    /// Practice points the game gave the club.
    exp: u32,
    /// What it means for the tournament.
    lines: Vec<(String, String)>,
}

/// How long one pitch takes on screen (seconds at normal speed).
pub(crate) const PITCH_SECS: f32 = 0.7;

/// A message on the notice card.
#[derive(Clone, Debug, Default, PartialEq)]
struct Note {
    title: String,
    title_ja: String,
    lines: Vec<(String, String)>,
    /// Players shown with their faces (family, given): who graduates, who joins.
    faces: Vec<(String, String)>,
    /// A line under each face (a destination, both languages).
    captions: Vec<(String, String)>,
    /// Hand-tuned portraits, parallel to `faces`.
    roles: Vec<Role>,
}

/// What the device keeps between visits (`page.save`), and what a new run starts from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Save {
    team: Team,
    year: u8,
    week: u8,
    pos: usize,
    hand: Vec<Card>,
    reputation: f32,
    wins: u32,
    losses: u32,
    /// Still in the summer / autumn tournament this year; qualified for the nationals.
    summer_in: bool,
    autumn_in: bool,
    national_in: bool,
    /// The best result each year ("Summer Stadium round 2", ...), both languages.
    results: Vec<(String, String)>,
    seed: u32,
    /// Middle-school players found by scouting; they join next April.
    #[serde(default)]
    scouted: Vec<team::Player>,
    /// The (year, week) whose healing already happened (so a reload does not heal twice).
    #[serde(default)]
    healed: (u8, u8),
    /// Named schools that stay for the whole run and grow.
    #[serde(default)]
    rivals: Vec<School>,
    /// Who we play in a given week, so the next year is a rematch.
    #[serde(default)]
    slate: Vec<Slate>,
    /// Invited to the March tournament off the autumn.
    #[serde(default)]
    spring_in: bool,
    /// Autumn wins this year (the seed).
    #[serde(default)]
    autumn_wins: u8,
    /// The year camp and exams already happened (0: not yet).
    #[serde(default)]
    camp_year: u8,
    #[serde(default)]
    exam_year: u8,
    /// Which three-year chapter this is. 0 on an old save means the first.
    #[serde(default)]
    chapter: u8,
    /// Grounds, 0 to 3. Rises with reputation and carries into the next chapter.
    #[serde(default)]
    grounds: u8,
    /// Empty: the spec's school. Set when the coach moves.
    #[serde(default)]
    school: String,
    #[serde(default)]
    school_ja: String,
    #[serde(default)]
    career_wins: u32,
    #[serde(default)]
    career_losses: u32,
    /// The three years are over and the coach has not chosen yet.
    #[serde(default)]
    epilogue: bool,
    /// Every finished year of the whole career, oldest first (the run never has to end).
    #[serde(default)]
    history: Vec<YearLog>,
    /// Wins and losses when this year began (the year's own record is the difference).
    #[serde(default)]
    year_from: (u32, u32),
    /// Graduates who went pro: full name and the career year they left.
    #[serde(default)]
    pros: Vec<(String, u32)>,
    /// The tournament being played: its next game (the round). The board waits.
    #[serde(default)]
    cup: Option<Fixture>,
    /// Who we meet in each round of the tournament, drawn when it starts: a circuit
    /// school's index, or `AWAY + n` for the n-th school from elsewhere.
    #[serde(default)]
    cup_path: Vec<u8>,
    /// Our score in each round played so far (us, them).
    #[serde(default)]
    cup_scores: Vec<(u8, u8)>,
    /// Seeded in the summer off the spring: the summer starts in round 2.
    #[serde(default)]
    seeded: bool,
    /// The next game's opponent was studied the day before: it plays a little weaker.
    #[serde(default)]
    studied: bool,
    /// The years the summer numbers, the draft, the selection and winter happened.
    #[serde(default)]
    numbers_year: u8,
    #[serde(default)]
    draft_year: u8,
    #[serde(default)]
    selection_year: u8,
    #[serde(default)]
    winter_year: u8,
    /// Third-years whose last summer is over: out of the team until they graduate.
    #[serde(default)]
    seniors: Vec<team::Player>,
    /// Retired third-years the pros picked in October (family names).
    #[serde(default)]
    drafted: Vec<String>,
    /// The captain (family name), named when the new team forms.
    #[serde(default)]
    captain: String,
    /// The new team still waits for the coach to name a captain.
    #[serde(default)]
    need_captain: bool,
    /// Summer games this coach has won here (the first ends a long drought).
    #[serde(default)]
    summer_won: u32,
}

/// `cup_path` entries from here on are schools from other prefectures.
const AWAY: u8 = 100;

/// One finished year in the career record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct YearLog {
    /// The career year (1, 2, ... across chapters and schools).
    year: u32,
    school: String,
    school_ja: String,
    best: String,
    best_ja: String,
    wins: u32,
    losses: u32,
}

impl YearLog {
    /// 0 a national title, 1 the Summer Stadium, 2 a prefectural title, 3 anything else.
    fn tier(&self) -> u8 {
        if self.best_ja.contains("優勝") && self.best.contains("Stadium") {
            0
        } else if self.best.contains("Stadium") || self.best.contains("Blossom") {
            1
        } else if self.best.contains("champions") {
            2
        } else {
            3
        }
    }
}

/// The career year a save is in (chapters run on; year counts within one chapter).
fn career_year(s: &Save, years: u8) -> u32 {
    (s.chapter.max(1) as u32 - 1) * years.max(1) as u32 + s.year as u32
}

#[derive(Debug)]
pub struct DiamondDays {
    spec: DiamondDaysSpec,
    core: Core,
    flow: GameFlow,
    doc: WorldDoc,
    rng: Rng,
    s: Save,
    mode: Mode,
    notes: Vec<Note>,
    game: Option<Game>,
    /// The order the coach gave this half, and whether the coach leaves orders to the team.
    auto_orders: bool,
    /// The hop between squares (0..1) and where it started.
    hop: f32,
    hop_from: usize,
    /// The roster (and the player picker) as a list that follows a finger.
    roster: crate::hud_art::Scroll,
    /// A saved run to resume (from `world.save`).
    resume: Option<Save>,
    /// The week as it was before the card was played: what a save while walking keeps, so
    /// a reload replays the week instead of half of it.
    pre_walk: Option<Save>,
    /// `?week=13` on the preview link: a fresh run starts at that week (to try a game).
    start_week: Option<u8>,
    clock: f32,
    lang_ja: bool,
    size: Cell<(f32, f32)>,
    /// The host asked the game to draw its own title, pause and result cards (a native
    /// shell; the web page brings HTML cards).
    own_menu: Cell<bool>,
    /// The rival of the game in progress (not saved: a reload replays the week).
    rival_idx: Option<usize>,
    /// Camp results, told after the optional conversation.
    camp_lines: Vec<(String, String)>,
    /// Schools offered when the coach decides to move.
    move_picks: Vec<usize>,
    /// Today's fixture while the pre-game screen is up.
    fixture: Option<Fixture>,
    /// The last game's result screen.
    report: Option<Report>,
    /// Double speed in a game.
    fast: bool,
    /// The order was opened from the pre-game screen (closing it goes back there).
    lineup_from_intro: bool,
    /// The coach asked to call the next plate appearance (any batter, not only chances).
    want_call: bool,
    /// Today's opponent, drawn up on game day so the pre-game screen can show their ace.
    pending_rival: Option<Side>,
    /// Words popping up over the screen ("UP!", a new grade).
    pops: crate::hud_art::Pops,
    /// A scene change opening over the screen (into a game, out of it).
    transition: Option<crate::hud_art::Transition>,
}

impl DiamondDays {
    pub fn new(spec: DiamondDaysSpec) -> Self {
        let mut flow = GameFlow::new(spec.id.clone());
        flow.title = spec.title.clone();
        flow.tagline = String::new();
        let mut g = Self {
            rng: Rng::new(spec.seed),
            s: fresh_save(spec.seed),
            spec,
            core: Core::default(),
            flow,
            doc: WorldDoc::default(),
            mode: Mode::Plan,
            notes: Vec::new(),
            game: None,
            auto_orders: false,
            hop: 1.0,
            hop_from: 0,
            roster: crate::hud_art::Scroll::default(),
            resume: None,
            pre_walk: None,
            start_week: None,
            clock: 0.0,
            lang_ja: false,
            size: Cell::new((390.0, 844.0)),
            own_menu: Cell::new(false),
            rival_idx: None,
            camp_lines: Vec::new(),
            move_picks: Vec::new(),
            fixture: None,
            report: None,
            fast: false,
            lineup_from_intro: false,
            want_call: false,
            pending_rival: None,
            pops: crate::hud_art::Pops::default(),
            transition: None,
        };
        g.build_doc();
        g
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| format!("diamond_days dump: {e}"))?;
        let spec = match value.get("game") {
            Some(g) => DiamondDaysSpec::from_value(g)?,
            None => DiamondDaysSpec::default(),
        };
        let mut g = Self::new(spec);
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            g.flow.best_score = best as u32;
        }
        // `?week=13` on the preview link (the manifest's `page.url_params`).
        let week = value.get("week").and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        });
        if let Some(w) = week {
            g.start_week = Some((w as u8).clamp(1, board::WEEKS));
        }
        // A run in progress, kept on the device. A save from an older version is ignored.
        if let Some(sv) = value.get("save") {
            if let Ok(s) = serde_json::from_value::<Save>(sv.clone()) {
                if s.year >= 1 && s.year <= g.spec.years && !s.team.players.is_empty() {
                    g.resume = Some(prepare_save(s));
                }
            }
        }
        Ok(g)
    }

    /// The world file this spec ships as (the baked fixture is exactly this).
    pub fn world_value(&self) -> Value {
        let game: Value =
            serde_json::from_str(&serde_json::to_string(&self.spec).unwrap_or_default())
                .unwrap_or_default();
        json!({"version": 1, "genre": GAME_ID, "game": game})
    }

    fn tr<'a>(&self, en: &'a str, ja: &'a str) -> &'a str {
        if self.lang_ja {
            ja
        } else {
            en
        }
    }

    fn school_en(&self) -> &str {
        if self.s.school.is_empty() {
            &self.spec.school
        } else {
            &self.s.school
        }
    }

    fn school_ja(&self) -> &str {
        if self.s.school_ja.is_empty() {
            &self.spec.school_ja
        } else {
            &self.s.school_ja
        }
    }

    fn chapter(&self) -> u8 {
        self.s.chapter.max(1)
    }

    /// The career year: 1, 2, 3, ... for as long as the coach keeps going.
    fn career_year(&self) -> u32 {
        career_year(&self.s, self.spec.years)
    }

    /// Log the finished year in the career record.
    fn log_year(&mut self) {
        let y = self.s.year as usize;
        let (best, best_ja) = self
            .s
            .results
            .get(y.saturating_sub(1))
            .cloned()
            .unwrap_or_else(|| ("-".into(), "-".into()));
        let (w0, l0) = self.s.year_from;
        let log = YearLog {
            year: self.career_year(),
            school: self.school_en().to_string(),
            school_ja: self.school_ja().to_string(),
            best,
            best_ja,
            wins: self.s.wins.saturating_sub(w0),
            losses: self.s.losses.saturating_sub(l0),
        };
        self.s.history.push(log);
        self.s.year_from = (self.s.wins, self.s.losses);
    }

    /// The next chapter: the record carries on, old fixtures go.
    fn next_chapter(&mut self) {
        self.s.chapter = self.chapter().saturating_add(1);
        let chapter = self.s.chapter;
        self.s.slate.retain(|e| e.chapter >= chapter);
        self.s.year_from = (0, 0);
    }

    fn bump_rep(&mut self, d: f32) {
        self.s.reputation = (self.s.reputation + d).clamp(0.0, 100.0);
        let g = board::grounds_of(self.s.reputation);
        if g > self.s.grounds {
            self.s.grounds = g;
        }
    }

    // ---- flow

    pub fn confirm(&mut self) {
        match self.flow.phase {
            FlowPhase::Title | FlowPhase::Result => {
                self.core = Core::default();
                let kept = self
                    .resume
                    .take()
                    .filter(|_| self.flow.phase == FlowPhase::Title);
                let resumed = kept.is_some();
                self.s = kept.unwrap_or_else(|| {
                    fresh_save(self.spec.seed.wrapping_add(self.clock.to_bits()))
                });
                self.rng = Rng::new(self.s.seed);
                self.want_call = false;
                self.transition = None;
                // The `?week=` test switch starts a new run there; it never moves a kept one.
                if let Some(w) = self.start_week.take() {
                    if !resumed {
                        self.s.week = w;
                    }
                }
                self.notes.clear();
                self.game = None;
                self.pre_walk = None;
                // Calls are asked again in a new run.
                self.auto_orders = false;
                self.flow.enter_play();
                crate::mixer::cue_sfx("go");
                crate::mixer::play_bgm("bgm", 0.5);
                // Only the very first week of a career (a later chapter also starts at
                // year 1, week 1, 0-0).
                if resumed {
                    // Opening the game again: say where the run stands before anything
                    // happens (even on a game day), and start the week on the tap.
                    self.welcome_back();
                    return;
                }
                if self.s.week == 1
                    && self.career_year() == 1
                    && self.s.history.is_empty()
                    && self.s.wins + self.s.losses == 0
                {
                    let (school, school_ja) =
                        (self.school_en().to_string(), self.school_ja().to_string());
                    let cap = self.captain_name().unwrap_or_default();
                    self.note(
                        "A new coach",
                        "新しい監督",
                        vec![
                            (
                                format!("April. You are the new coach of {school}: a dozen players, and not one summer win in ten years."),
                                format!("4月。あなたは{school_ja}野球部の新しい監督。部員は12人、夏はもう10年も勝てていない。"),
                            ),
                            (
                                "A first-year girl runs up with a scorebook. \"I'm Kotori, the manager! I'll be with you for three years!\"".into(),
                                "スコアブックを抱えた1年生が駆けてきた。「マネージャーの七瀬ことりです！三年間、よろしくお願いします！」".into(),
                            ),
                            (
                                format!("Captain {cap}, a third-year: \"This is our last summer. Just once, I want to win a summer game.\""),
                                format!("3年生の主将、{cap}が頭を下げる。「俺たち、これが最後の夏なんです。一度でいいから、夏に勝ちたい」"),
                            ),
                            (
                                "This year: win a summer game. Pick a card each week, train them, and get there.".into(),
                                "今年の目標は「夏の1勝」。毎週カードを選んで鍛え、夏へ向かおう。".into(),
                            ),
                        ],
                    );
                }
                self.begin_week();
            }
            FlowPhase::Pause => self.flow.resume(),
            FlowPhase::Play => {}
        }
    }

    pub fn show_title(&mut self) {
        self.flow.show_title();
    }

    pub fn pause(&mut self) {
        if self.flow.phase == FlowPhase::Play {
            self.flow.pause();
        }
    }

    fn note(&mut self, title: &str, title_ja: &str, lines: Vec<(String, String)>) {
        self.notes.push(Note {
            title: title.into(),
            title_ja: title_ja.into(),
            lines,
            faces: Vec::new(),
            captions: Vec::new(),
            roles: Vec::new(),
        });
        self.mode = Mode::Notice;
    }

    /// The start of a week: a game if the calendar has one for us, else the cards.
    fn begin_week(&mut self) {
        if self.s.epilogue {
            self.mode = Mode::Epilogue { schools: false };
            return;
        }
        // Injuries and red marks heal a week at the start of each week (once).
        let now = (self.s.year, self.s.week);
        if self.s.healed != now {
            self.s.healed = now;
            for p in &mut self.s.team.players {
                p.hurt = p.hurt.saturating_sub(1);
                p.study = p.study.saturating_sub(1);
            }
        }
        // A tournament under way: the draw, then the next game.
        if self.s.cup.is_some() {
            if self.game.is_none() && self.notes.is_empty() {
                self.mode = Mode::Bracket;
            }
            return;
        }
        // The new team still needs its captain.
        if self.s.need_captain {
            if self.notes.is_empty() {
                self.mode = Mode::Decide(Decide::Captain);
            }
            return;
        }
        let year = self.s.year;
        if self.s.week == board::CAMP_WEEK && self.s.camp_year != year {
            self.run_camp();
            return;
        }
        if self.s.week == board::NUMBERS_WEEK && self.s.numbers_year != year {
            self.mode = Mode::Decide(Decide::Ace);
            return;
        }
        if self.s.week == board::EXAM_WEEK && self.s.exam_year != year {
            self.run_exams();
            return;
        }
        if self.s.week == board::DRAFT_WEEK && self.s.draft_year != year {
            self.s.draft_year = year;
            self.run_draft();
        }
        if self.s.week == board::SELECTION_WEEK && self.s.selection_year != year {
            self.s.selection_year = year;
            self.run_selection();
        }
        if self.s.week == *board::WINTER.start() && self.s.winter_year != year {
            self.s.winter_year = year;
            self.note(
                "Winter",
                "冬",
                vec![
                    (
                        "December: no more games with other schools until March. Winter training counts for more.".into(),
                        "12月。3月まで対外試合はできない。冬の練習は身につきやすい。".into(),
                    ),
                    (
                        "Kotori: \"Run, lift, swing. Spring will tell.\"".into(),
                        "ことり「走って、鍛えて、振り込んで。春に答えが出ますよ」".into(),
                    ),
                ],
            );
        }
        if let Some(f) = self.game_this_week() {
            if self.game.is_none() {
                if f.is_cup() {
                    self.start_cup(f);
                } else {
                    self.prepare_game(f);
                }
                return;
            }
        }
        if self.notes.is_empty() {
            self.mode = Mode::Plan;
        } else {
            self.mode = Mode::Notice;
        }
    }

    /// Whether the club plays in a fixture this year.
    fn plays_in(&self, f: Fixture) -> bool {
        match f {
            Fixture::Practice | Fixture::SpringPref(_) => true,
            Fixture::Summer(_) => self.s.summer_in,
            Fixture::National(_) => self.s.national_in,
            Fixture::Autumn(_) => self.s.autumn_in,
            Fixture::Spring(_) => self.s.spring_in,
        }
    }

    /// This week's game for us: the tournament under way, or one the calendar starts now.
    fn game_this_week(&self) -> Option<Fixture> {
        self.s
            .cup
            .or_else(|| board::fixture(self.s.week).filter(|f| self.plays_in(*f)))
    }

    /// A kept run picked up again: where it stands, and what this week holds.
    fn welcome_back(&mut self) {
        let m = board::month(self.s.week);
        let wk = (self.s.week - 1) % 4 + 1;
        let y = self.career_year();
        let (w, l) = (self.s.wins, self.s.losses);
        let mut lines = vec![(
            format!(
                "Year {y}, {} week {wk}, {w}-{l}: picking up where you left off.",
                hud::month_en(m)
            ),
            format!("{y}年目 {m}月 第{wk}週、{w}勝{l}敗。前回の続きからです。"),
        )];
        // What the week opens on, in the order `begin_week` takes them.
        let camp = self.s.week == board::CAMP_WEEK && self.s.camp_year != self.s.year;
        let exams = self.s.week == board::EXAM_WEEK && self.s.exam_year != self.s.year;
        lines.push(match self.game_this_week() {
            _ if self.s.epilogue => (
                "A milestone: time to decide what comes next.".into(),
                "ひと区切りの時です。これからを決めましょう。".into(),
            ),
            _ if camp => (
                "This week is summer camp.".into(),
                "今週は夏合宿です。".into(),
            ),
            _ if exams => (
                "This week is exam week.".into(),
                "今週はテスト週間です。".into(),
            ),
            _ if self.s.need_captain => (
                "The new team needs a captain.".into(),
                "新チームの主将を決めるところからです。".into(),
            ),
            None if self.s.week == board::NUMBERS_WEEK && self.s.numbers_year != self.s.year => (
                "This week: the summer numbers and the draw.".into(),
                "今週は背番号発表と、夏の組み合わせ抽選会です。".into(),
            ),
            Some(f) => {
                let (en, ja) = Self::fixture_name(f);
                (
                    format!("This week is game day: {en}."),
                    format!("今週は{ja}の日です。準備はいいですか？"),
                )
            }
            None => (
                "Another week of practice: pick a card.".into(),
                "今週も練習から。カードを選びましょう。".into(),
            ),
        });
        self.note("Welcome back", "おかえりなさい、監督", lines);
    }

    fn fixture_name(f: Fixture) -> (String, String) {
        let (cup, cup_ja) = f.cup_name();
        if !f.is_cup() {
            return (cup.into(), cup_ja.into());
        }
        let (r, r_ja) = f.round_name();
        (format!("{cup}, {r}"), format!("{cup_ja} {r_ja}"))
    }

    /// Game day: pick today's opponent and show the pre-game screen.
    fn prepare_game(&mut self, f: Fixture) {
        let idx = self.opponent(f);
        self.rival_idx = Some(idx);
        self.pending_rival = Some(self.rival_side(f, idx));
        self.fixture = Some(f);
        self.mode = Mode::MatchIntro;
        self.transition = Some(crate::hud_art::Transition::open(
            crate::hud_art::Wipe::Curtain,
            1.2,
            [176, 40, 52, 255],
            self.tr("GAME DAY!", "試合の日！"),
        ));
        crate::mixer::cue_sfx("event");
    }

    /// The opponent's nine for a fixture against school `idx`.
    fn rival_side(&mut self, f: Fixture, idx: usize) -> Side {
        let target = board::rival_level(f, self.s.year);
        let (en, ja, level) = match self.s.rivals.get(idx) {
            Some(s) => (
                s.name.clone(),
                s.name_ja.clone(),
                s.strength * 0.72 + target * 0.28,
            ),
            None => {
                let (en, ja, _) =
                    board::away_school(idx.saturating_sub(AWAY as usize), self.s.year);
                (en, ja, target)
            }
        };
        let ease = if matches!(f, Fixture::Spring(_)) {
            self.s.autumn_wins as f32 * 2.0
        } else {
            0.0
        } + if self.s.studied { 3.0 } else { 0.0 };
        Side::rival(&mut self.rng, &en, &ja, (level - ease).clamp(28.0, 92.0))
    }

    /// Who we meet in a fixture: the tournament's draw, or this week's practice school.
    fn opponent(&mut self, f: Fixture) -> usize {
        if f.is_cup() {
            if let Some(&i) = self.s.cup_path.get(f.round() as usize - 1) {
                return i as usize;
            }
        }
        self.pick_rival(f)
    }

    /// A tournament starts: the draw (our path to the final), the opening words, then the
    /// bracket. A summer seed starts in round 2.
    fn start_cup(&mut self, f: Fixture) {
        if self.s.rivals.is_empty() {
            self.s.rivals = board::rival_schools();
        }
        let f = if matches!(f, Fixture::Summer(_)) && self.s.seeded {
            f.at(2)
        } else {
            f.at(1)
        };
        self.s.cup_path = self.draw_path(f);
        self.s.cup_scores.clear();
        self.s.cup = Some(f);
        self.s.studied = false;
        let (cup, cup_ja) = f.cup_name();
        let mut lines = Vec::new();
        match f {
            Fixture::SpringPref(_) => lines.push((
                "The spring prefectural. Reach the last four and the summer draw seeds us.".into(),
                "春季県大会。ベスト4まで行けば、夏はシード校になれる。".into(),
            )),
            Fixture::Summer(_) => {
                lines.push((
                    "Opening day. The schools march in behind their banners; one captain takes the oath for all.".into(),
                    "開会式。各校がプラカードを先頭に行進し、ひとりの主将が選手宣誓をした。".into(),
                ));
                lines.push((
                    "Lose once and the third-years' summer is over.".into(),
                    "一度負けたら、3年生の夏は終わる。".into(),
                ));
                if self.s.seeded {
                    lines.push((
                        "Seeded off the spring: we start in round 2.".into(),
                        "春の結果でシード。2回戦から登場。".into(),
                    ));
                }
            }
            Fixture::National(_) => lines.push((
                "The Summer Stadium. Every school here won its prefecture. Each win plays the school song.".into(),
                "夏の大舞台。ここにいるのは、どこも県を勝ち抜いた学校だ。勝てば校歌が流れる。".into(),
            )),
            Fixture::Autumn(_) => lines.push((
                "The new team's first tournament. Go deep and March may call.".into(),
                "新チーム最初の大会。勝ち進めば、春の招待舞台が見えてくる。".into(),
            )),
            Fixture::Spring(_) => lines.push((
                "The Blossom Invitational. The first big stage of the year.".into(),
                "春の招待舞台。一年で最初の大舞台だ。".into(),
            )),
            Fixture::Practice => {}
        }
        if let Some(c) = self.captain_line() {
            lines.push(c);
        }
        crate::mixer::cue_sfx("event");
        self.note(cup, cup_ja, lines);
    }

    /// Our path through a tournament: one school per round, stronger as it goes. The
    /// strongest circuit school waits near the end of a prefectural draw.
    fn draw_path(&mut self, f: Fixture) -> Vec<u8> {
        let rounds = f.rounds();
        if matches!(f, Fixture::National(_) | Fixture::Spring(_)) {
            let base = self.rng.next_u32() as u8 % 24;
            return (0..rounds).map(|r| AWAY + (base + r * 7) % 24).collect();
        }
        let n = self.s.rivals.len();
        let mut used: Vec<usize> = Vec::new();
        let mut path = Vec::new();
        for r in 1..=rounds {
            let target = board::rival_level(f.at(r), self.s.year);
            let jitter = self.rng.unit() * 6.0 - 3.0;
            let pick = (0..n).filter(|i| !used.contains(i)).min_by(|a, b| {
                let d = |i: usize| (self.s.rivals[i].strength - target - jitter).abs();
                d(*a).total_cmp(&d(*b))
            });
            match pick {
                Some(i) => {
                    used.push(i);
                    path.push(i as u8);
                }
                None => path.push(AWAY + r),
            }
        }
        // The best school on the circuit is in our half: we meet it late.
        if let Some(top) = (0..n).max_by(|a, b| {
            self.s.rivals[*a]
                .strength
                .total_cmp(&self.s.rivals[*b].strength)
        }) {
            let late = (rounds as usize).saturating_sub(2);
            if let Some(at) = path.iter().position(|&i| i as usize == top) {
                if at < late {
                    path.swap(at, late);
                }
            } else if late < path.len() {
                path[late] = top as u8;
            }
        }
        path
    }

    /// The captain's word before a tournament, if the club has one.
    fn captain_line(&self) -> Option<(String, String)> {
        let cap = self.captain_name()?;
        Some((
            format!("Captain {cap} in the huddle: \"One game at a time. Let's go!\""),
            format!("円陣で主将の{cap}が叫ぶ。「一戦必勝！いくぞ！」"),
        ))
    }

    /// The captain's family name: the one the coach named, else the founding captain.
    pub(crate) fn captain_name(&self) -> Option<String> {
        if !self.s.captain.is_empty()
            && self
                .s
                .team
                .players
                .iter()
                .any(|p| p.family == self.s.captain)
        {
            return Some(self.s.captain.clone());
        }
        self.s
            .team
            .players
            .iter()
            .find(|p| p.role == Role::Captain)
            .map(|p| p.family.clone())
    }

    /// Play ball: build both nines (the order as it stands now) and start.
    fn play_ball(&mut self) {
        if self.mode != Mode::MatchIntro {
            return;
        }
        if let Some(f) = self.fixture.take() {
            crate::mixer::cue_sfx("go");
            self.start_game(f);
            self.transition = Some(crate::hud_art::Transition::open(
                crate::hud_art::Wipe::Iris,
                0.9,
                [12, 16, 32, 255],
                self.tr("PLAY BALL", "プレイボール"),
            ));
        }
    }

    fn start_game(&mut self, f: Fixture) {
        // A call asked for in an earlier game does not carry over.
        self.want_call = false;
        let (school, school_ja) = (self.school_en().to_string(), self.school_ja().to_string());
        let us = Side::from_team(&self.s.team, &school, &school_ja);
        let idx = match self.rival_idx {
            Some(i) => i,
            None => self.opponent(f),
        };
        self.rival_idx = Some(idx);
        let them = match self.pending_rival.take() {
            Some(s) => s,
            None => self.rival_side(f, idx),
        };
        let first = self.rng.unit() < 0.5;
        let seed = self.rng.next_u32();
        let mut g = Game::new(us, them, first, seed);
        let (title, title_ja) = Self::fixture_name(f);
        let spark = g.sides[g.us].spark.clone();
        g.log.push(sim::Play::note(
            format!("{title}: {} vs {}.", g.sides[0].name, g.sides[1].name),
            format!(
                "{title_ja}：{} 対 {}。",
                g.sides[0].name_ja, g.sides[1].name_ja
            ),
        ));
        if let Some(name) = spark {
            let mut shout = sim::Play::note(
                format!("Spark! {name} lifts the bench."),
                format!("元気印！{name}がベンチを明るくする。"),
            );
            shout.banner = Some(Ability::Mood);
            g.log.push(shout);
        }
        g.log.push(sim::Play::note("Top of inning 1.", "1回表。"));
        self.game = Some(g);
        self.mode = Mode::Match {
            shown: 0,
            t: 0.0,
            asking: None,
        };
    }

    /// The game is over: results, the tournament, the players' wear.
    fn end_game(&mut self) {
        let Some(g) = self.game.take() else {
            return;
        };
        self.want_call = false;
        let won = g.won() == Some(true);
        let f = self.game_this_week().unwrap_or(Fixture::Practice);
        let (title, title_ja) = Self::fixture_name(f);
        let (us, them) = (g.runs(g.us), g.runs(1 - g.us));
        if won {
            self.s.wins += 1;
            crate::mixer::cue_sfx("win");
        } else {
            self.s.losses += 1;
            crate::mixer::cue_sfx("lose");
        }
        if let Some(i) = self.rival_idx.take() {
            if let Some(school) = self.s.rivals.get_mut(i) {
                school.met = school.met.saturating_add(1);
                if won {
                    school.wins = school.wins.saturating_add(1);
                    school.strength = (school.strength + 0.35).min(90.0);
                } else {
                    school.losses = school.losses.saturating_add(1);
                    school.strength = (school.strength + 1.15).min(90.0);
                }
            }
        }
        for p in &mut self.s.team.players {
            p.tire(if p.pitcher() { 25 } else { 12 });
            p.mood = if won { p.mood.up() } else { p.mood.down() };
        }
        let (exp, star) = self.game_experience(&g, won);
        let mut lines = vec![(
            format!(
                "{} {us} - {them} {}",
                self.school_en(),
                g.sides[1 - g.us].name
            ),
            format!(
                "{} {us} - {them} {}",
                self.school_ja(),
                g.sides[1 - g.us].name_ja
            ),
        )];
        if f.is_cup() {
            self.s
                .cup_scores
                .push((us.min(255) as u8, them.min(255) as u8));
            self.s.studied = false;
        }
        let (r, last) = (f.round(), f.is_final());
        match f {
            Fixture::Practice => self.bump_rep(if won { 0.5 } else { 0.0 }),
            Fixture::SpringPref(_) => {
                if won {
                    self.bump_rep(0.8 + r as f32 * 0.4);
                }
                // The last four (a semifinal) are seeded in the summer.
                if !won || last {
                    let reached = if won { r + 1 } else { r };
                    self.s.seeded = reached >= f.rounds() - 1;
                    if last && won {
                        self.record("Spring prefectural champions", "春季県大会 優勝");
                    }
                    lines.push(if self.s.seeded {
                        (
                            "The last four in spring: we are seeded in the summer draw.".into(),
                            "春はベスト4以上。夏はシード校だ。".into(),
                        )
                    } else {
                        (
                            "No seed for the summer: we start from round 1.".into(),
                            "夏はノーシード。1回戦からの戦いになる。".into(),
                        )
                    });
                }
            }
            Fixture::Summer(_) => {
                if won {
                    self.s.summer_won += 1;
                    if self.s.summer_won == 1 {
                        lines.push((
                            "The first summer win in years! The stands are in tears.".into(),
                            "何年ぶりかの夏の1勝！スタンドが泣いている。".into(),
                        ));
                    }
                    self.bump_rep(1.0 + r as f32 * 0.5);
                    if last {
                        self.s.national_in = true;
                        self.bump_rep(10.0);
                        lines.push((
                            "Prefectural champions! On to the Summer Stadium.".into(),
                            "県大会優勝！夏の大舞台へ！".into(),
                        ));
                        self.record("Prefectural champions", "県大会優勝");
                    }
                } else {
                    self.s.summer_in = false;
                    let (_, rn) = f.round_name();
                    self.record(&format!("Summer round {r}"), &format!("夏 {rn}敗退"));
                }
            }
            Fixture::National(_) => {
                if won {
                    self.bump_rep(6.0);
                    if last {
                        self.record("Summer Stadium champions", "夏の大舞台 優勝");
                        lines.push((
                            "National champions! The whole town is out in the street.".into(),
                            "全国制覇！町じゅうが大騒ぎ。".into(),
                        ));
                    }
                } else {
                    self.s.national_in = false;
                    let (_, rn) = f.round_name();
                    self.record(
                        &format!("Summer Stadium round {r}"),
                        &format!("夏の大舞台 {rn}"),
                    );
                }
            }
            Fixture::Autumn(_) => {
                if won {
                    self.s.autumn_wins = self.s.autumn_wins.saturating_add(1);
                    self.bump_rep(1.0 + r as f32 * 0.5);
                    if last {
                        self.record("Autumn champions", "秋季大会優勝");
                    }
                } else {
                    self.s.autumn_in = false;
                }
                if !won || last {
                    lines.push(if self.s.autumn_wins >= 3 {
                        (
                            "That run should put us on the list for March. The answer comes in January.".into(),
                            "この成績なら、春の招待舞台の候補に入るはず。発表は1月だ。".into(),
                        )
                    } else {
                        (
                            "The autumn is over. Winter is for getting stronger.".into(),
                            "秋が終わった。冬は力をためる季節だ。".into(),
                        )
                    });
                }
            }
            Fixture::Spring(_) => {
                if won {
                    self.bump_rep(2.0 + r as f32);
                    if last {
                        self.record("Blossom Invitational champions", "春の招待舞台 優勝");
                        lines.push((
                            "The Blossom Invitational is yours.".into(),
                            "春の招待舞台を制した！".into(),
                        ));
                    }
                } else {
                    self.s.spring_in = false;
                    let (_, rn) = f.round_name();
                    self.record(
                        &format!("Blossom Invitational round {r}"),
                        &format!("春の招待舞台 {rn}"),
                    );
                }
            }
        }
        // Still in it: who is next.
        if let Some(next) = f.next().filter(|_| won) {
            let (rn, rn_ja) = next.round_name();
            if let Some(&i) = self.s.cup_path.get(next.round() as usize - 1) {
                let (en, ja, _) = self.path_school(i);
                lines.push((
                    format!("Next, the {rn}: {en}."),
                    format!("次は{rn_ja}。相手は{ja}だ。"),
                ));
            }
        }
        self.report = Some(Report {
            won,
            title: (title, title_ja),
            names: [
                (g.sides[0].name.clone(), g.sides[0].name_ja.clone()),
                (g.sides[1].name.clone(), g.sides[1].name_ja.clone()),
            ],
            us: g.us,
            line: g.line.clone(),
            hits: g.hits,
            star,
            exp,
            lines: lines.into_iter().skip(1).collect(),
        });
        // The calendar moves on now (a save here is past the game), then the result shows.
        // A tournament still going stays on its week: the next round is the next day.
        match f.next().filter(|_| won) {
            Some(next) => self.s.cup = Some(next),
            None if f.is_cup() => self.finish_cup(f, won),
            None => self.end_week(),
        }
        self.mode = Mode::MatchEnd;
    }

    /// What the game taught the club: every one of our nine gets batting (more for each
    /// hit) and a little running; whoever pitched gets pitching (more for strikeouts); a win
    /// adds fielding. Returns the total and the player of the game.
    fn game_experience(&mut self, g: &Game, won: bool) -> (u32, Option<(String, String, String)>) {
        #[derive(Default, Clone)]
        struct Line {
            ab: u32,
            hits: u32,
            rbi: u32,
            homers: u32,
            k: u32,
            allowed: u32,
            faced: u32,
        }
        let us = g.us as u8;
        let mut by: Vec<(String, Line)> = Vec::new();
        fn at(by: &mut Vec<(String, Line)>, name: &str) -> usize {
            if let Some(i) = by.iter().position(|(n, _)| n == name) {
                return i;
            }
            by.push((name.to_string(), Line::default()));
            by.len() - 1
        }
        for play in &g.log {
            if !play.is_pa() {
                continue;
            }
            if play.side == us {
                let i = at(&mut by, &play.batter);
                let l = &mut by[i].1;
                if play.is_at_bat() {
                    l.ab += 1;
                }
                if let sim::PlayKind::Hit(n) = play.kind {
                    l.hits += 1;
                    l.homers += (n == 4) as u32;
                }
                l.rbi += play.runs as u32;
            } else {
                let i = at(&mut by, &play.pitcher);
                let l = &mut by[i].1;
                l.faced += play.faced_batter() as u32;
                l.k += (play.kind == sim::PlayKind::Strikeout) as u32;
                l.allowed += play.runs as u32;
            }
        }
        let mut total = 0u32;
        // `by` holds only our players, whoever was subbed in or out on the way.
        for p in &mut self.s.team.players {
            let Some(l) = by
                .iter()
                .find(|(n, _)| *n == p.family)
                .map(|(_, l)| l.clone())
            else {
                continue;
            };
            let line = team::Stats {
                games: 1,
                ab: l.ab as u16,
                hits: l.hits as u16,
                hr: l.homers as u16,
                rbi: l.rbi as u16,
                faced: l.faced as u16,
                k: l.k as u16,
                allowed: l.allowed as u16,
            };
            p.season.add(&line);
            p.career.add(&line);
            let mut add = |d: Drill, n: u32| {
                p.xp[d.index()] = p.xp[d.index()].saturating_add(n as u16);
                total += n;
            };
            if l.ab > 0 {
                add(Drill::Batting, 2 + l.hits);
                add(Drill::Running, 1);
            }
            if l.faced > 0 {
                add(Drill::Pitching, 2 + l.k / 2);
            }
            if won {
                add(Drill::Fielding, 1);
            }
        }
        // The player of the game: the best bat, or a pitcher who shut them down.
        let star = by
            .iter()
            .map(|(n, l)| {
                let bat = l.hits as f32 * 2.0 + l.rbi as f32 * 2.5 + l.homers as f32 * 2.0;
                let arm = if l.faced >= 12 {
                    l.k as f32 * 0.9 + (6.0 - l.allowed as f32).max(0.0) * 1.2
                } else {
                    0.0
                };
                (n, l, bat.max(arm), arm > bat)
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .filter(|s| s.2 > 0.0)
            .map(|(n, l, _, pitched)| {
                if pitched {
                    (
                        n.clone(),
                        format!("{} strikeouts, {} runs", l.k, l.allowed),
                        format!("{}奪三振 {}失点", l.k, l.allowed),
                    )
                } else {
                    (
                        n.clone(),
                        format!("{} for {}, {} RBI", l.hits, l.ab, l.rbi),
                        format!("{}打数{}安打 {}打点", l.ab, l.hits, l.rbi),
                    )
                }
            });
        (total, star)
    }

    /// Words over the player's page for every stat that rose since `before`: "+1" by the
    /// row, and the new grade when the letter changes.
    fn pop_growth(&mut self, i: usize, before: &team::Player) {
        let (w, h) = self.size.get();
        let ja = self.lang_ja;
        let rows = self.raise_rows(w, h);
        let after = self.s.team.players[i].clone();
        let mut k = 0.0;
        for st in team::Stat::ALL {
            let (a, b) = (before.stat(st), after.stat(st));
            if b <= a {
                continue;
            }
            let Some(r) = rows.iter().find(|(s, _)| *s == st).map(|(_, r)| *r) else {
                continue;
            };
            let at = [r[0] - 40.0, r[1] + r[3] * 0.5];
            let unit = if st == team::Stat::Velo { "km/h" } else { "" };
            self.pops.push(
                at,
                format!("+{}{unit}", b - a),
                [255, 214, 80, 255],
                18.0,
                k,
            );
            let (ga, gb) = (team::grade(a), team::grade(b));
            if st != team::Stat::Velo && ga != gb {
                self.pops.push(
                    [200.0, r[1] - 6.0],
                    format!("{} {ga}→{gb}!", st.label(ja)),
                    [255, 150, 60, 255],
                    26.0,
                    k + 0.2,
                );
            }
            k += 0.12;
        }
    }

    /// The best bat and the best arm of the season, on a notice card.
    fn season_awards(&mut self) {
        // The third-years retired in the summer count too: their numbers are this season's.
        let team: Vec<&team::Player> = self
            .s
            .team
            .players
            .iter()
            .chain(self.s.seniors.iter())
            .collect();
        let bat = team
            .iter()
            .filter(|p| p.season.ab >= 8)
            .max_by(|a, b| {
                let k = |p: &team::Player| p.season.hits as f32 / p.season.ab as f32;
                k(a).total_cmp(&k(b))
                    .then(a.season.hits.cmp(&b.season.hits))
            })
            .map(|p| (p.family.clone(), p.season));
        let arm = team
            .iter()
            .filter(|p| p.season.faced >= 15)
            .max_by_key(|p| p.season.k)
            .map(|p| (p.family.clone(), p.season));
        let mut lines = Vec::new();
        if let Some((n, s)) = bat {
            lines.push((
                format!(
                    "Best bat: {n}, {} ({} for {}), {} HR, {} RBI.",
                    s.avg(),
                    s.hits,
                    s.ab,
                    s.hr,
                    s.rbi
                ),
                format!(
                    "首位打者：{n}　打率{}（{}打数{}安打）本塁打{} 打点{}",
                    s.avg(),
                    s.ab,
                    s.hits,
                    s.hr,
                    s.rbi
                ),
            ));
        }
        if let Some((n, s)) = arm {
            lines.push((
                format!("Best arm: {n}, {} strikeouts.", s.k),
                format!("奪三振王：{n}　{}奪三振", s.k),
            ));
        }
        if !lines.is_empty() {
            self.note("Season awards", "今季の表彰", lines);
        }
    }

    /// The coach leaves the result screen: the notes the week left, or the next week.
    /// A tournament is over (lost, or won outright): the calendar moves past it. The end
    /// of our summer (a summer loss, or the national tournament over) retires the
    /// third-years and starts the new team.
    fn finish_cup(&mut self, f: Fixture, won: bool) {
        self.s.cup = None;
        self.s.cup_path.clear();
        self.s.cup_scores.clear();
        self.s.studied = false;
        let end = board::cup_end(f);
        if self.s.week < end {
            self.s.week = end - 1;
        }
        let summer_over = match f {
            Fixture::Summer(_) => !won,
            Fixture::National(_) => true,
            _ => false,
        };
        self.end_week();
        if summer_over {
            self.retire_seniors();
        }
    }

    /// The third-years' summer is over: a word from each, and they leave the team (they
    /// graduate in March). A club left short takes in players from other clubs. The new
    /// team then needs a captain.
    fn retire_seniors(&mut self) {
        let (leaving, staying): (Vec<team::Player>, Vec<team::Player>) =
            std::mem::take(&mut self.s.team.players)
                .into_iter()
                .partition(|p| p.grade >= 3);
        self.s.team.players = staying;
        self.s.team.clear_lineup();
        self.s.captain.clear();
        self.s.team.number_one.clear();
        self.s.need_captain = true;
        if !leaving.is_empty() {
            let n = leaving.len();
            let mut lines = vec![(
                format!("Our summer is over. {n} third-years hang up their uniforms."),
                format!("{}の夏が終わった。3年生{n}人が引退する。", self.school_ja()),
            )];
            for p in leaving.iter().take(6) {
                let (en, ja) = farewell(p);
                lines.push((
                    format!("{}: \"{en}\"", p.family),
                    format!("{}「{ja}」", p.family),
                ));
            }
            lines.push((
                "Kotori can't stop crying. \"Thank you, seniors.\"".into(),
                "ことりは涙が止まらない。「先輩たち、本当にお疲れさまでした…！」".into(),
            ));
            lines.push((
                "You: \"Hold your heads up. You left this summer to the ones behind you.\"".into(),
                "監督「胸を張れ。お前たちの夏は、ちゃんと後輩に残った」".into(),
            ));
            crate::mixer::cue_sfx("event");
            self.note("Farewell, seniors", "3年生 引退", lines);
            if let Some(note) = self.notes.last_mut() {
                note.faces = leaving
                    .iter()
                    .map(|p| (p.family.clone(), p.given.clone()))
                    .collect();
                note.captions = leaving
                    .iter()
                    .map(|p| {
                        let (en, ja) = senior_title(p);
                        (en.to_string(), ja.to_string())
                    })
                    .collect();
                note.roles = leaving.iter().map(|p| p.role).collect();
            }
            self.s.seniors.extend(leaving);
        }
        // Nine and a spare, whatever it takes.
        let short = 10usize.saturating_sub(self.s.team.players.len());
        if short > 0 {
            let names = self
                .s
                .team
                .late_joiners(&mut self.rng, short, self.s.reputation);
            self.note(
                "New members",
                "助っ人入部",
                vec![(
                    format!(
                        "Too few for a nine: {} join from other clubs.",
                        names.join(", ")
                    ),
                    format!(
                        "このままでは9人そろわない。{}が他の部から入部してくれた！",
                        names.join("、")
                    ),
                )],
            );
        }
        self.note(
            "The new team",
            "新チーム発足",
            vec![(
                "The new team starts today. First, a captain.".into(),
                "今日から新チームだ。まずは主将を決めよう。".into(),
            )],
        );
    }

    /// Who could wear number 1: the best three pitchers.
    pub(crate) fn ace_candidates(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.s.team.players.len())
            .filter(|&i| self.s.team.players[i].pitcher())
            .collect();
        v.sort_by(|a, b| {
            let r = |i: usize| self.s.team.players[i].pitch_rating();
            r(*b).total_cmp(&r(*a))
        });
        v.truncate(3);
        v
    }

    /// Who could captain the new team, each with his way of leading: the best player
    /// (leads by example), the loudest (leads with his voice), the steadiest glove (holds
    /// the field together). Drawn from the oldest class on the team.
    pub(crate) fn captain_candidates(&self) -> Vec<(usize, Ability)> {
        let ps = &self.s.team.players;
        let top = ps.iter().map(|p| p.grade).max().unwrap_or(1);
        let class: Vec<usize> = (0..ps.len()).filter(|&i| ps[i].grade == top).collect();
        let pool = if class.len() >= 3 {
            class
        } else {
            (0..ps.len()).collect()
        };
        let best = |key: &dyn Fn(usize) -> f32, taken: &[usize]| {
            pool.iter()
                .copied()
                .filter(|i| !taken.contains(i))
                .max_by(|a, b| key(*a).total_cmp(&key(*b)))
        };
        let mut out: Vec<(usize, Ability)> = Vec::new();
        let rating = |i: usize| ps[i].bat_rating().max(ps[i].pitch_rating());
        let voice =
            |i: usize| (ps[i].has(Ability::Mood) as u8 as f32) * 100.0 + ps[i].mood as u8 as f32;
        let glove = |i: usize| ps[i].field as f32;
        for (key, a) in [
            (&rating as &dyn Fn(usize) -> f32, Ability::Chance),
            (&voice, Ability::Mood),
            (&glove, Ability::Clutch),
        ] {
            let taken: Vec<usize> = out.iter().map(|(i, _)| *i).collect();
            if let Some(i) = best(key, &taken) {
                out.push((i, a));
            }
        }
        out
    }

    /// The coach decides: the ace's number, or the new captain.
    fn decide(&mut self, d: Decide, k: usize) {
        match d {
            Decide::Ace => {
                let Some(&i) = self.ace_candidates().get(k) else {
                    return;
                };
                self.s.numbers_year = self.s.year;
                let fam = self.s.team.players[i].family.clone();
                let given = self.s.team.players[i].given.clone();
                self.s.team.number_one = fam.clone();
                for (j, p) in self.s.team.players.iter_mut().enumerate() {
                    if j == i {
                        p.mood = p.mood.up().up();
                    } else if p.pitcher() && p.grade == 3 {
                        p.mood = p.mood.down();
                    }
                }
                let mut lines = vec![
                    (
                        "The night before the summer, the coach hands out the numbers, one by one.".into(),
                        "夏の大会を前に、監督が背番号をひとりずつ手渡していく。".into(),
                    ),
                    (
                        format!("Number 1 goes to {fam}. He holds it in both hands and doesn't say a word."),
                        format!("背番号1は{fam}。両手で受け取ったまま、何も言えずにいる。"),
                    ),
                ];
                let seeded = self.s.seeded;
                lines.push(if seeded {
                    (
                        "Then the draw: seeded, we start in round 2.".into(),
                        "続いて組み合わせ抽選会。シード校として2回戦から登場する。".into(),
                    )
                } else {
                    (
                        "Then the draw: the captain pulls our slot. Round 1, and a long way up."
                            .into(),
                        "続いて組み合わせ抽選会。主将がくじを引いた。1回戦から、長い道のりだ。"
                            .into(),
                    )
                });
                crate::mixer::cue_sfx("event");
                self.note("Summer numbers", "背番号発表・抽選会", lines);
                if let Some(n) = self.notes.last_mut() {
                    n.faces = vec![(fam, given)];
                    n.captions = vec![("No. 1".into(), "背番号1".into())];
                }
                self.end_week();
            }
            Decide::Captain => {
                let Some(&(i, a)) = self.captain_candidates().get(k) else {
                    return;
                };
                self.s.need_captain = false;
                let p = &mut self.s.team.players[i];
                p.mood = p.mood.up().up();
                let fam = p.family.clone();
                let given = p.given.clone();
                self.s.captain = fam.clone();
                let learned = self.s.team.players[i].grant(a);
                for p in &mut self.s.team.players {
                    p.mood = p.mood.up();
                }
                let mut lines = vec![(
                    format!("{fam} is the new captain. \"I'll take us further than last year.\""),
                    format!("新主将は{fam}。「去年より、ひとつでも上へ連れていきます」"),
                )];
                if learned {
                    lines.push((
                        format!("The armband suits him: {}.", a.label(false)),
                        format!("主将の自覚で、特技「{}」がついた。", a.label(true)),
                    ));
                }
                crate::mixer::cue_sfx("train");
                self.note("New captain", "新主将", lines);
                if let Some(n) = self.notes.last_mut() {
                    n.faces = vec![(fam, given)];
                    n.captions = vec![("Captain".into(), "主将".into())];
                }
            }
        }
    }

    /// The day before a tournament game: rest, tune up, or study the next school. Then
    /// the game.
    fn day_before(&mut self, k: usize) {
        let Some(f) = self.s.cup else {
            return;
        };
        match k {
            0 => {
                for p in &mut self.s.team.players {
                    p.rest(30);
                }
            }
            1 => {
                for p in &mut self.s.team.players {
                    p.rest(10);
                    p.mood = p.mood.up();
                }
            }
            _ => {
                for p in &mut self.s.team.players {
                    p.rest(10);
                }
                self.s.studied = true;
            }
        }
        crate::mixer::cue_sfx("card");
        self.prepare_game(f);
    }

    /// April: every tournament is open again; last year's seed and draw are gone.
    fn new_season(&mut self) {
        self.s.summer_in = true;
        self.s.autumn_in = true;
        self.s.national_in = false;
        self.s.spring_in = false;
        self.s.autumn_wins = 0;
        self.s.seeded = false;
        self.s.cup = None;
        self.s.cup_path.clear();
        self.s.cup_scores.clear();
        self.s.studied = false;
    }

    /// October: the pro draft. A retired third-year good enough may hear his name.
    fn run_draft(&mut self) {
        let picks: Vec<String> = self
            .s
            .seniors
            .iter()
            .filter(|p| team::destination(p, self.s.reputation) == team::After::Pro)
            .map(|p| p.family.clone())
            .collect();
        if picks.is_empty() {
            return;
        }
        let names = picks.join(", ");
        let names_ja = picks.join("、");
        self.s.drafted = picks;
        crate::mixer::cue_sfx("win");
        self.note(
            "Draft day",
            "ドラフト会議",
            vec![
                (
                    format!("Draft day. The phone rings in the staff room: {names} has been picked by a pro club!"),
                    format!("ドラフト会議の日。職員室の電話が鳴った。{names_ja}がプロ球団から指名された！"),
                ),
                (
                    "The club lines up for the press photo, caps in the air.".into(),
                    "部員みんなで記者会見の写真に並び、帽子を空へ放り投げた。".into(),
                ),
            ],
        );
    }

    /// January: the March invitations are announced. A deep autumn run waits by the
    /// phone in the principal's office.
    fn run_selection(&mut self) {
        let wins = self.s.autumn_wins;
        if wins < 2 {
            return;
        }
        let chosen = wins >= 3 || self.rng.unit() < 0.45;
        self.s.spring_in = chosen;
        let mut lines = vec![(
            "Selection day. The club waits in the principal's office, staring at the phone.".into(),
            "選考発表の日。部員たちは校長室で、じっと電話を見つめている。".into(),
        )];
        if chosen {
            crate::mixer::cue_sfx("win");
            lines.push((
                "It rings. \"Congratulations: you are invited to the Blossom Invitational.\" The room erupts.".into(),
                "電話が鳴った。「春の招待舞台への出場が決まりました」部屋が歓声に包まれた。".into(),
            ));
        } else {
            lines.push((
                "The phone never rings. Someone else's spring. \"Then we take the summer,\" says the captain.".into(),
                "電話は最後まで鳴らなかった。「なら、夏で取り返すぞ」と主将が言った。".into(),
            ));
        }
        self.note("Selection day", "選考発表", lines);
    }

    fn after_match(&mut self) {
        if self.mode != Mode::MatchEnd {
            return;
        }
        self.report = None;
        crate::mixer::cue_sfx("card");
        let (title, color) = if self.s.cup.is_some() {
            (
                self.tr("ON TO THE NEXT ROUND", "次の試合へ"),
                [176, 40, 52, 255],
            )
        } else {
            (
                self.tr("BACK TO PRACTICE", "練習に戻ろう"),
                [70, 140, 84, 255],
            )
        };
        self.transition = Some(crate::hud_art::Transition::open(
            crate::hud_art::Wipe::Curtain,
            1.0,
            color,
            title,
        ));
        if !self.notes.is_empty() {
            self.mode = Mode::Notice;
        } else if self.s.epilogue {
            self.mode = Mode::Epilogue { schools: false };
        } else {
            self.mode = Mode::Plan;
            self.begin_week();
        }
    }

    /// Keep the year's best result (the first of a deeper run wins).
    fn record(&mut self, en: &str, ja: &str) {
        let y = self.s.year as usize;
        while self.s.results.len() < y {
            self.s.results.push(("-".into(), "-".into()));
        }
        let rank = |t: &str| {
            if t.contains("Stadium champions") {
                5
            } else if t.contains("Blossom") && t.contains("champions") {
                4
            } else if t.contains("Stadium") {
                3
            } else if t.contains("Prefectural champions") || t.contains("Autumn champions") {
                2
            } else if t.starts_with("Summer") || t.starts_with("Autumn") || t.starts_with("Blossom")
            {
                1
            } else {
                0
            }
        };
        if rank(en) >= rank(&self.s.results[y - 1].0) {
            self.s.results[y - 1] = (en.into(), ja.into());
        }
    }

    /// Play a card: walk its steps.
    fn play_card(&mut self, i: usize) {
        if self.mode != Mode::Plan || i >= self.s.hand.len() {
            return;
        }
        self.pre_walk = Some(self.s.clone());
        let card = self.s.hand.remove(i);
        self.s.hand.push(Card::draw(&mut self.rng, &self.s.team));
        crate::mixer::cue_sfx("card");
        self.mode = Mode::Walk {
            left: card.steps,
            t: 0.0,
            card,
        };
    }

    /// The coach stops on a square: it happens.
    fn land(&mut self, card: Card) {
        let sq = self.spec.ring[self.s.pos];
        // A talk waits for the coach's answer (a save meanwhile replays the week).
        if sq == Square::Event {
            let list = self.spec.happenings.clone();
            let pick = board::pick_happening(&mut self.rng, &list)
                .map(|h| (h.id.clone(), h.who, !h.choices.is_empty()));
            if let Some((id, who, true)) = pick.clone() {
                if let (Some(h), Some(w)) = (
                    list.iter().position(|x| x.id == id),
                    board::speaker(&mut self.rng, who, &self.s.team),
                ) {
                    crate::mixer::cue_sfx("event");
                    self.mode = Mode::Ask { h, who: w };
                    return;
                }
            }
            self.pre_walk = None;
            self.happen(pick.map(|p| p.0));
            return;
        }
        self.pre_walk = None;
        let (title, title_ja) = (sq.label(false).to_string(), sq.label(true).to_string());
        let mut lines = Vec::new();
        match sq {
            Square::Train(d) => {
                let pts = board::train_week(
                    &mut self.s.team,
                    d,
                    &card,
                    self.spec.train_points
                        * (1.0 + self.s.grounds as f32 * 0.08)
                        * if board::WINTER.contains(&self.s.week) {
                            1.25
                        } else {
                            1.0
                        },
                );
                crate::mixer::cue_sfx("train");
                let doubled = card.focus == Some(d);
                let star = card.star.clone().unwrap_or_default();
                lines.push((
                    format!(
                        "{} practice{}. The team earned {pts} points.",
                        sq.label(false),
                        if doubled { " (focus: double)" } else { "" }
                    ),
                    format!(
                        "{}練習{}。チームで{pts}ポイント獲得。",
                        sq.label(true),
                        if doubled { "（重点：2倍）" } else { "" }
                    ),
                ));
                if !star.is_empty() {
                    lines.push((
                        format!("{star} trained twice as hard."),
                        format!("{star}が人一倍がんばった。"),
                    ));
                }
                // A worn-out player can get hurt.
                let tired: Vec<usize> = (0..self.s.team.players.len())
                    .filter(|&k| {
                        self.s.team.players[k].stamina < 25 && self.s.team.players[k].hurt == 0
                    })
                    .collect();
                for k in tired {
                    let roll = self.rng.unit();
                    if roll >= 0.15 {
                        continue;
                    }
                    let n = self.s.team.players[k].family.clone();
                    if self.s.team.players[k].has(Ability::Durable) {
                        lines.push((
                            format!("Durable! {n} shakes off a twist."),
                            format!("鉄の体！{n}は捻挫をこらえた。"),
                        ));
                    } else {
                        self.s.team.players[k].hurt = 2;
                        lines.push((
                            format!("{n} pushed too hard and is hurt (2 weeks)."),
                            format!("{n}が無理をしてケガ（2週間）。"),
                        ));
                    }
                }
            }
            Square::Rest => {
                for p in &mut self.s.team.players {
                    p.rest(self.spec.rest);
                    if p.mood < Mood::Normal {
                        p.mood = p.mood.up();
                    }
                }
                lines.push((
                    "A day off. Everyone sleeps late.".into(),
                    "休養日。みんなぐっすり眠った。".into(),
                ));
            }
            Square::Spirit => {
                for p in &mut self.s.team.players {
                    p.mood = p.mood.up();
                }
                lines.push((
                    "The captain gives a speech. Spirits are high.".into(),
                    "キャプテンの一言で、チームの士気が上がった。".into(),
                ));
            }
            Square::Scout => {
                if self.s.scouted.len() >= 4 {
                    lines.push((
                        "Four middle-schoolers already promised to come. Enough for one year."
                            .into(),
                        "中学生4人がもう入部を約束してくれた。今年はこれで十分。".into(),
                    ));
                } else {
                    let p = Team::prospect(&mut self.rng, self.s.reputation);
                    let rating = if p.pitcher() {
                        p.pitch_rating()
                    } else {
                        p.bat_rating()
                    };
                    let g = team::grade(rating.round().clamp(1.0, 99.0) as u8);
                    lines.push((
                        format!(
                            "A middle-schooler, {} ({}, rated {g}), will join next April.",
                            p.full_name(),
                            p.pos.short(false)
                        ),
                        format!(
                            "中学生の{}（{}・評価{g}）が来年4月に入部してくれることに。",
                            p.full_name(),
                            p.pos.short(true)
                        ),
                    ));
                    self.s.scouted.push(p);
                }
            }
            Square::Event => {}
        }
        self.note(&title, &title_ja, lines);
        self.end_week();
    }

    /// An event without a talk happens to everyone (`id`: the one already drawn).
    fn happen(&mut self, id: Option<String>) {
        let (title, title_ja) = (
            Square::Event.label(false).to_string(),
            Square::Event.label(true).to_string(),
        );
        let mut lines = Vec::new();
        let list = self.spec.happenings.clone();
        if let Some(h) = id.and_then(|id| list.iter().find(|x| x.id == id)) {
            let hurt = board::apply(&mut self.rng, h, &mut self.s.team);
            self.s.reputation = (self.s.reputation + h.reputation).clamp(0.0, 100.0);
            let mut en = h.text.clone();
            let mut ja = h.text_ja.clone();
            if let Some(n) = hurt {
                en = format!("{en} ({n})");
                ja = format!("{ja}（{n}）");
            }
            lines.push((en, ja));
            if h.id == "exams" {
                self.push_red_marks(&mut lines);
            }
            crate::mixer::cue_sfx("event");
        }
        self.note(&title, &title_ja, lines);
        self.end_week();
    }

    /// The coach answers a talk: it takes effect, the reply is told, the week ends.
    fn answer(&mut self, k: usize) {
        let Mode::Ask { h, who } = self.mode else {
            return;
        };
        let Some(c) = self
            .spec
            .happenings
            .get(h)
            .and_then(|x| x.choices.get(k))
            .cloned()
        else {
            return;
        };
        self.pre_walk = None;
        board::apply_choice(&c, &mut self.s.team, who);
        self.s.reputation = (self.s.reputation + c.reputation).clamp(0.0, 100.0);
        let (fam, given) = self
            .s
            .team
            .players
            .get(who)
            .map(|p| (p.family.clone(), p.given.clone()))
            .unwrap_or_default();
        let id = self
            .spec
            .happenings
            .get(h)
            .map(|x| x.id.clone())
            .unwrap_or_default();
        let mut lines = vec![(
            c.reply.replace("{name}", &fam),
            c.reply_ja.replace("{name}", &fam),
        )];
        if let Some(a) = board::choice_grant(&id, k) {
            if self.s.team.players.get_mut(who).is_some_and(|p| p.grant(a)) {
                lines.push((
                    format!("New ability: {}.", a.label(false)),
                    format!("新しい特技：{}。", a.label(true)),
                ));
            }
        }
        crate::mixer::cue_sfx("train");
        self.note(Square::Event.label(false), Square::Event.label(true), lines);
        if let Some(n) = self.notes.last_mut() {
            n.faces = vec![(fam, given)];
        }
        self.end_week();
    }

    /// The week is done: healing, rest, the next week, the year's turn.
    fn end_week(&mut self) {
        for p in &mut self.s.team.players {
            p.rest(self.spec.weekly_rest);
        }
        self.s.week += 1;
        if self.s.week > board::WEEKS {
            self.end_year();
        }
    }

    fn end_year(&mut self) {
        // The third-years retired in the summer graduate now (and any still on the team).
        let drafted = std::mem::take(&mut self.s.drafted);
        let rep = self.s.reputation;
        let retired_after = |p: &team::Player| {
            if drafted.contains(&p.family) {
                team::After::Pro
            } else {
                // A retired senior goes pro only through the October draft.
                match team::destination(p, rep) {
                    team::After::Pro => team::After::College,
                    a => a,
                }
            }
        };
        let leaving: Vec<(String, String, Role, team::After)> = self
            .s
            .seniors
            .iter()
            .map(|p| (p, retired_after(p)))
            .chain(
                self.s
                    .team
                    .players
                    .iter()
                    .filter(|p| p.grade >= 3)
                    .map(|p| (p, team::destination(p, rep))),
            )
            .map(|(p, after)| (p.family.clone(), p.given.clone(), p.role, after))
            .collect();
        let retired = self.s.seniors.len();
        let names = |t: &Team, grade: u8| -> Vec<(String, String)> {
            t.players
                .iter()
                .filter(|p| p.grade == grade)
                .map(|p| (p.family.clone(), p.given.clone()))
                .collect()
        };
        // The season's prizes, then the pros, then the numbers start again.
        self.season_awards();
        let year_now = self.career_year();
        for (fam, given, _, after) in &leaving {
            if *after == team::After::Pro {
                self.s.pros.push((format!("{fam} {given}"), year_now));
            }
        }
        let left = self.s.team.graduate() + retired;
        self.s.seniors.clear();
        for p in &mut self.s.team.players {
            p.season = team::Stats::default();
        }
        let y = self.s.year as usize;
        while self.s.results.len() < y {
            self.s.results.push(("-".into(), "-".into()));
        }
        self.log_year();
        for school in &mut self.s.rivals {
            school.strength = (school.strength + 0.8).min(90.0);
        }
        if self.s.year >= self.spec.years {
            if left > 0 {
                self.push_graduation(left, &leaving);
            }
            self.s.epilogue = true;
            return;
        }
        self.s.year += 1;
        self.s.week = 1;
        self.new_season();
        let scouted = std::mem::take(&mut self.s.scouted);
        let joined = self
            .s
            .team
            .recruit(&mut self.rng, self.s.reputation, scouted);
        if left > 0 {
            self.push_graduation(left, &leaving);
        }
        self.note(
            "New faces",
            "新入部員",
            vec![(
                format!(
                    "{joined} first-years joined the club. Year {} begins.",
                    self.career_year()
                ),
                format!(
                    "新入部員が{joined}人入った。{}年目のはじまり。",
                    self.career_year()
                ),
            )],
        );
        let fresh = names(&self.s.team, 1);
        if let Some(n) = self.notes.last_mut() {
            n.faces = fresh;
        }
    }

    fn push_graduation(&mut self, left: usize, leaving: &[(String, String, Role, team::After)]) {
        let mut lines = vec![(
            format!("{left} third-years graduated. Thank you, seniors."),
            format!("3年生{left}人が卒業。先輩、三年間ありがとう。"),
        )];
        for (fam, _, _, after) in leaving.iter().take(8) {
            lines.push((
                format!("{fam} — {}.", after.label(false)),
                format!("{fam} — {}。", after.label(true)),
            ));
        }
        self.note("Graduation", "卒業式", lines);
        if let Some(n) = self.notes.last_mut() {
            n.faces = leaving
                .iter()
                .map(|(a, b, _, _)| (a.clone(), b.clone()))
                .collect();
            n.captions = leaving
                .iter()
                .map(|(_, _, _, after)| (after.label(false).into(), after.label(true).into()))
                .collect();
            n.roles = leaving.iter().map(|(_, _, role, _)| *role).collect();
        }
    }

    fn push_red_marks(&mut self, lines: &mut Vec<(String, String)>) {
        let marked = board::red_marks(&mut self.rng, &mut self.s.team);
        if marked.is_empty() {
            lines.push((
                "Everyone scraped through. Nobody is benched.".into(),
                "全員、なんとか合格。出場停止はなし。".into(),
            ));
        } else {
            let names = marked.join(", ");
            lines.push((
                format!("Red marks: {names} cannot play for 2 weeks."),
                format!("赤点：{names}は2週間、試合に出られない。"),
            ));
        }
    }

    fn run_camp(&mut self) {
        self.pre_walk = Some(self.s.clone());
        let report = board::camp_week(&mut self.s.team, &mut self.rng, self.spec.train_points);
        self.s.camp_year = self.s.year;
        let mut lines = vec![(
            format!("Summer camp. The club earned {} points.", report.points),
            format!("夏合宿。チームで{}ポイント。", report.points),
        )];
        for n in &report.hurt {
            lines.push((
                format!("{n} tweaked something and misses the summer opener."),
                format!("{n}が少し痛めた。夏の初戦は欠場。"),
            ));
        }
        for n in &report.shrugged {
            lines.push((
                format!("Durable! {n} finishes the camp healthy."),
                format!("鉄の体！{n}は合宿を無事に終えた。"),
            ));
        }
        if self.rng.unit() < 0.55 {
            if let Some(who) = board::speaker(&mut self.rng, board::Who::Any, &self.s.team) {
                self.camp_lines = lines;
                self.mode = Mode::CampTalk { who };
                return;
            }
        }
        self.pre_walk = None;
        self.note("Summer camp", "夏合宿", lines);
        self.end_week();
    }

    fn run_exams(&mut self) {
        self.s.exam_year = self.s.year;
        for p in &mut self.s.team.players {
            p.mood = p.mood.down();
        }
        let mut lines = vec![(
            "Exam week. The clubhouse is quiet.".into(),
            "テスト週間。部室は静かだ。".into(),
        )];
        self.push_red_marks(&mut lines);
        crate::mixer::cue_sfx("event");
        self.note("Exams", "テスト", lines);
        self.end_week();
    }

    fn answer_camp(&mut self, k: usize) {
        let Mode::CampTalk { who } = self.mode else {
            return;
        };
        let h = board::camp_talk();
        let Some(c) = h.choices.get(k).cloned() else {
            return;
        };
        self.pre_walk = None;
        board::apply_choice(&c, &mut self.s.team, who);
        let fam = self
            .s
            .team
            .players
            .get(who)
            .map(|p| p.family.clone())
            .unwrap_or_default();
        let mut lines = self.camp_lines.clone();
        self.camp_lines.clear();
        lines.push((
            c.reply.replace("{name}", &fam),
            c.reply_ja.replace("{name}", &fam),
        ));
        if let Some(a) = board::choice_grant("camp_night", k) {
            if self.s.team.players.get_mut(who).is_some_and(|p| p.grant(a)) {
                lines.push((
                    format!("New ability: {}.", a.label(false)),
                    format!("新しい特技：{}。", a.label(true)),
                ));
            }
        }
        crate::mixer::cue_sfx("train");
        self.note("Summer camp", "夏合宿", lines);
        self.end_week();
    }

    /// The school for this week. The same week next year is a rematch.
    fn pick_rival(&mut self, f: Fixture) -> usize {
        if self.s.rivals.is_empty() {
            self.s.rivals = board::rival_schools();
        }
        let chapter = self.chapter();
        if let Some(e) = self
            .s
            .slate
            .iter()
            .find(|e| e.chapter == chapter && e.year == self.s.year && e.week == self.s.week)
        {
            return (e.rival as usize).min(self.s.rivals.len().saturating_sub(1));
        }
        let rematch =
            self.s.slate.iter().find(|e| {
                e.chapter == chapter && e.year + 1 == self.s.year && e.week == self.s.week
            });
        let i = if let Some(prev) = rematch {
            prev.rival as usize
        } else {
            let target = board::rival_level(f, self.s.year);
            let used: Vec<u8> = self
                .s
                .slate
                .iter()
                .filter(|e| e.chapter == chapter && e.year == self.s.year)
                .map(|e| e.rival)
                .collect();
            let mut best = 0usize;
            let mut best_score = f32::MAX;
            for (i, school) in self.s.rivals.iter().enumerate() {
                let mut score = (school.strength - target).abs();
                if matches!(f, Fixture::National(_)) && school.strength < 60.0 {
                    score += 20.0;
                }
                if matches!(f, Fixture::Practice) && school.strength > 60.0 {
                    score += 12.0;
                }
                if used.contains(&(i as u8)) {
                    score += 8.0;
                }
                if score < best_score {
                    best_score = score;
                    best = i;
                }
            }
            best
        };
        let i = i.min(self.s.rivals.len().saturating_sub(1));
        self.s.slate.push(Slate {
            chapter,
            year: self.s.year,
            week: self.s.week,
            rival: i as u8,
        });
        i
    }

    fn continue_here(&mut self) {
        self.s.epilogue = false;
        self.s.career_wins += self.s.wins;
        self.s.career_losses += self.s.losses;
        self.s.wins = 0;
        self.s.losses = 0;
        self.s.results.clear();
        self.next_chapter();
        self.s.year = 1;
        self.s.week = 1;
        self.new_season();
        self.s.grounds = self.s.grounds.max(board::grounds_of(self.s.reputation));
        let scouted = std::mem::take(&mut self.s.scouted);
        let joined = self
            .s
            .team
            .recruit(&mut self.rng, self.s.reputation, scouted);
        self.note(
            "Next generation",
            "次の世代",
            vec![(
                format!(
                    "Year {}, chapter {}. {joined} first-years join the club you built.",
                    self.career_year(),
                    self.chapter()
                ),
                format!(
                    "{}年目、第{}期。育てた部に新入部員が{joined}人入った。",
                    self.career_year(),
                    self.chapter()
                ),
            )],
        );
        let fresh: Vec<(String, String)> = self
            .s
            .team
            .players
            .iter()
            .filter(|p| p.grade == 1)
            .map(|p| (p.family.clone(), p.given.clone()))
            .collect();
        if let Some(n) = self.notes.last_mut() {
            n.faces = fresh;
        }
    }

    fn continue_move(&mut self, shown: usize) {
        let Some(&pick) = self.move_picks.get(shown) else {
            return;
        };
        if pick >= self.s.rivals.len() {
            return;
        }
        let old_en = self.school_en().to_string();
        let old_ja = self.school_ja().to_string();
        let new_en = self.s.rivals[pick].name.clone();
        let new_ja = self.s.rivals[pick].name_ja.clone();
        let left = self.s.team.strength().clamp(30.0, 80.0);
        // The old club joins the circuit in its own colours and with its own captain.
        let c = art::team_color(&old_en);
        self.s.rivals[pick].color = [c[0], c[1], c[2]];
        self.s.rivals[pick].face = old_en.clone();
        self.s.rivals[pick].name = old_en;
        self.s.rivals[pick].name_ja = old_ja;
        self.s.rivals[pick].strength = left;
        self.s.rivals[pick].wins = 0;
        self.s.rivals[pick].losses = 0;
        self.s.school = new_en.clone();
        self.s.school_ja = new_ja.clone();
        self.s.epilogue = false;
        self.s.career_wins += self.s.wins;
        self.s.career_losses += self.s.losses;
        self.s.wins = 0;
        self.s.losses = 0;
        self.s.results.clear();
        self.next_chapter();
        self.s.year = 1;
        self.s.week = 1;
        self.new_season();
        self.s.reputation = (self.s.reputation * 0.6).clamp(0.0, 100.0);
        self.s.grounds = 0;
        self.s.scouted.clear();
        let level = 32.0 + self.s.reputation * 0.12;
        self.s.team = Team::takeover(&mut self.rng, level);
        self.note(
            "A new school",
            "新しい学校",
            vec![(
                format!("You take over {new_en}. The club is small, and they know your name."),
                format!("{new_ja}の監督になった。小さな部だが、名前は知られている。"),
            )],
        );
    }

    fn finish_run(&mut self) {
        let score = (self.s.career_wins + self.s.wins) * 100 + self.s.reputation as u32 * 10;
        crate::mixer::stop_bgm(0.6);
        self.s.epilogue = false;
        self.flow.finish(score, self.s.wins, "GRADUATION");
        // A new run next time.
        self.resume = None;
        self.s.year = self.spec.years + 1;
    }

    // ---- sim

    fn step_sim(&mut self, dt: f32) {
        self.step_play(dt);
        // The 3D world under the match HUD follows the game on screen.
        if matches!(self.mode, Mode::Match { .. }) {
            self.doc = self.match_world();
        } else if !self.doc.props.is_empty() {
            self.doc = WorldDoc::default();
        }
    }

    fn step_play(&mut self, dt: f32) {
        self.clock += dt;
        self.pops.tick(dt);
        if let Some(t) = self.transition.as_mut() {
            if !t.tick(dt) {
                self.transition = None;
            }
        }
        let (w, h) = self.size.get();
        let view = hud::list_view(self.stage(w, h).h);
        self.roster.view = view[3];
        self.roster.content = hud::list_content(self.s.team.players.len());
        self.roster.step(dt);
        match self.mode.clone() {
            Mode::Walk { left, t, card } => {
                let t = t + dt;
                self.hop = (t / self.spec.step_secs).min(1.0);
                if t >= self.spec.step_secs {
                    self.hop_from = self.s.pos;
                    self.s.pos = (self.s.pos + 1) % self.spec.ring.len();
                    crate::mixer::cue_sfx("step");
                    if left <= 1 {
                        self.hop = 1.0;
                        self.land(card);
                    } else {
                        self.hop = 0.0;
                        self.mode = Mode::Walk {
                            left: left - 1,
                            t: 0.0,
                            card,
                        };
                    }
                } else {
                    self.mode = Mode::Walk { left, t, card };
                }
            }
            Mode::Match { shown, t, asking } => {
                // The first pitch waits for the iris to open.
                if asking.is_some() || self.transition.as_ref().is_some_and(|t| t.openness() < 0.8)
                {
                    return;
                }
                let t = t + dt * if self.fast { 2.0 } else { 1.0 };
                if t < self.line_time(shown) {
                    self.mode = Mode::Match { shown, t, asking };
                    return;
                }
                self.advance_game(shown);
            }
            _ => {}
        }
    }

    /// The pitches of the latest line (none for words).
    pub(crate) fn line_pitches(&self, shown: usize) -> Vec<sim::Pitch> {
        shown
            .checked_sub(1)
            .and_then(|i| self.game.as_ref().and_then(|g| g.log.get(i)))
            .map(|p| p.pitches())
            .unwrap_or_default()
    }

    /// Seconds of pitches before the latest line's result shows.
    pub(crate) fn pitch_time(&self, shown: usize) -> f32 {
        self.line_pitches(shown).len() as f32 * PITCH_SECS
    }

    /// The latest line on screen: its pitches, then the result held a while.
    pub(crate) fn line_time(&self, shown: usize) -> f32 {
        self.pitch_time(shown) + self.spec.line_secs * self.line_pace(shown)
    }

    /// Clutch plays and ability shouts stay on screen longer.
    pub(crate) fn line_pace(&self, shown: usize) -> f32 {
        let Some(play) = shown
            .checked_sub(1)
            .and_then(|i| self.game.as_ref().and_then(|g| g.log.get(i)))
        else {
            return 1.0;
        };
        let mut pace = 1.0_f32;
        if play.clutch {
            pace = 1.85;
        }
        if play.banner.is_some() {
            pace = pace.max(1.65);
        }
        if matches!(play.kind, sim::PlayKind::Hit(n) if n >= 2) {
            pace = pace.max(1.7);
        }
        // A home run gets its moment; the end of the game (a walk-off, the bow) longer.
        if play.kind == sim::PlayKind::Hit(4) {
            pace = pace.max(3.0);
        }
        if self
            .game
            .as_ref()
            .is_some_and(|g| self.finale(g, shown).is_some())
        {
            pace = pace.max(6.0);
        }
        pace
    }

    /// Show the next line of the game (playing it out as needed), or ask for an order.
    fn advance_game(&mut self, shown: usize) {
        let Some(g) = self.game.as_mut() else {
            return;
        };
        if shown < g.log.len() {
            let line = g.log[shown].clone();
            if line.banner.is_some() {
                crate::mixer::cue_sfx("event");
            }
            if line.ja.contains("ホームラン") {
                crate::mixer::cue_sfx("homer");
            } else if line.runs > 0 || line.ja.contains("ヒット") || line.ja.contains("ベース")
            {
                crate::mixer::cue_sfx("hit");
            }
            self.mode = Mode::Match {
                shown: shown + 1,
                t: 0.0,
                asking: None,
            };
            return;
        }
        if g.over {
            self.end_game();
            return;
        }
        // A chance or a pinch, or any batter the coach asked for: the coach calls it
        // (unless the team plays on its own).
        let spot = g.spot().or_else(|| {
            (self.want_call && !g.over && !g.asked_now()).then(|| {
                if g.batting() == g.us {
                    Spot::Chance
                } else {
                    Spot::Pinch
                }
            })
        });
        if let Some(spot) = spot {
            if self.auto_orders {
                g.call(None);
            } else {
                self.want_call = false;
                self.mode = Mode::Match {
                    shown,
                    t: 0.0,
                    asking: Some(spot),
                };
                return;
            }
        }
        g.step();
        // The line already on screen is done: the next tick shows the new one.
        self.mode = Mode::Match {
            shown,
            t: f32::MAX,
            asking: None,
        };
    }

    /// The coach's call at a chance or a pinch (None: let them play).
    fn give_call(&mut self, t: Option<Tactic>) {
        if let Mode::Match {
            shown,
            asking: Some(Spot::Bench(_)),
            ..
        } = self.mode
        {
            if t.is_none() {
                let spot = self.game.as_ref().and_then(|g| g.spot());
                self.mode = Mode::Match {
                    shown,
                    t: 0.0,
                    asking: spot,
                };
                return;
            }
        }
        if let (Some(g), Mode::Match { shown, .. }) = (self.game.as_mut(), self.mode.clone()) {
            g.call(t);
            crate::mixer::cue_sfx("card");
            self.mode = Mode::Match {
                shown,
                t: 0.0,
                asking: None,
            };
        }
    }

    /// The team plays the rest of the game on its own (no more calls). From any prompt,
    /// the bench picker too (where a plain `Call(None)` only goes back).
    fn leave_to_team(&mut self) {
        self.auto_orders = true;
        if let (Some(g), Mode::Match { shown, .. }) = (self.game.as_mut(), self.mode.clone()) {
            g.call(None);
            crate::mixer::cue_sfx("card");
            self.mode = Mode::Match {
                shown,
                t: 0.0,
                asking: None,
            };
        }
    }

    /// Play the rest of the game at once.
    fn skip_game(&mut self) {
        if let Some(g) = self.game.as_mut() {
            g.finish();
        }
        self.end_game();
    }

    fn tap_notice(&mut self) {
        if !self.notes.is_empty() {
            self.notes.remove(0);
        }
        if self.notes.is_empty() {
            if self.flow.phase != FlowPhase::Play {
                return;
            }
            if self.s.epilogue {
                self.mode = Mode::Epilogue { schools: false };
                return;
            }
            self.mode = Mode::Plan;
            self.begin_week();
        }
    }

    // ---- input

    pub fn touch_ev(&mut self, ev: PointerEvent, w: u32, h: u32) {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        self.size.set((w, h));
        // The roster and the player picker follow a finger; a tap that hardly moved picks.
        if self.flow.phase == FlowPhase::Play
            && matches!(self.mode, Mode::Roster | Mode::Lineup { pick: Some(_) })
        {
            let st = self.stage(w, h);
            let (_, y) = st.to_design(ev.x, ev.y);
            let view = hud::list_view(st.h);
            self.roster.view = view[3];
            self.roster.content = hud::list_content(self.s.team.players.len());
            match ev.phase {
                PointerPhase::Begin if st.hit(ev.x, ev.y, view) => {
                    self.roster.press(y);
                    return;
                }
                PointerPhase::Move if self.roster.dragging() => {
                    self.roster.drag(y);
                    return;
                }
                PointerPhase::End | PointerPhase::Cancel if self.roster.dragging() => {
                    if self.roster.release() && ev.phase == PointerPhase::End {
                        let rows: Vec<_> = self
                            .hud_buttons(w, h)
                            .into_iter()
                            .filter(|(b, _)| matches!(b, hud::Btn::Row(_) | hud::Btn::Assign(_)))
                            .collect();
                        if let Some(b) = crate::hud_kit::hit(&rows, ev.x, ev.y) {
                            self.press(b);
                        }
                    }
                    return;
                }
                _ => {}
            }
        }
        if ev.phase != PointerPhase::Begin {
            return;
        }
        // Our own pause card: a tap goes back to the game.
        if self.flow.phase == FlowPhase::Pause && self.own_menu.get() {
            self.flow.resume();
            return;
        }
        if self.flow.phase != FlowPhase::Play {
            return;
        }
        // A scene change still mostly covering the screen takes no taps.
        if self.transition.as_ref().is_some_and(|t| t.openness() < 0.6) {
            return;
        }
        if let Some(b) = crate::hud_kit::hit(&self.hud_buttons(w, h), ev.x, ev.y) {
            self.press(b);
            return;
        }
        match self.mode {
            Mode::Notice => self.tap_notice(),
            // A tap off the list closes it.
            Mode::Roster => self.mode = Mode::Plan,
            Mode::Lineup { .. } => {
                self.mode = if std::mem::take(&mut self.lineup_from_intro) {
                    Mode::MatchIntro
                } else {
                    Mode::Roster
                }
            }
            Mode::Player(_) => self.mode = Mode::Roster,
            Mode::Match {
                asking: None,
                shown,
                t,
            } => {
                // A tap skips the pitches to the result, then hurries the next line.
                let pre = self.pitch_time(shown);
                if t < pre {
                    self.mode = Mode::Match {
                        shown,
                        t: pre,
                        asking: None,
                    };
                } else {
                    self.advance_game(shown);
                }
            }
            _ => {}
        }
    }

    fn open_lineup(&mut self) {
        self.mode = Mode::Lineup { pick: None };
    }

    fn nine_now(&self) -> (Vec<usize>, Vec<team::Pos>) {
        let nine = self.s.team.resolved();
        (
            nine.iter().map(|(i, _)| *i).collect(),
            nine.iter().map(|(_, p)| *p).collect(),
        )
    }

    fn write_nine(&mut self, order: Vec<usize>, mut spots: Vec<team::Pos>) -> bool {
        if self.s.team.set_lineup(&order, &spots) {
            return true;
        }
        if let Some(p) = order
            .iter()
            .position(|&i| self.s.team.players.get(i).is_some_and(|pl| pl.pitcher()))
        {
            if !spots.contains(&team::Pos::P) {
                spots[p] = team::Pos::P;
            } else if spots.get(p).copied() != Some(team::Pos::P) {
                // The mound must be a pitcher. Move P onto one.
                if let Some(old) = spots.iter().position(|s| *s == team::Pos::P) {
                    spots[old] = self.s.team.players[order[p]].pos;
                    if spots[old] == team::Pos::P {
                        spots[old] = team::Pos::LF;
                    }
                }
                spots[p] = team::Pos::P;
            }
            return self.s.team.set_lineup(&order, &spots);
        }
        false
    }

    fn nudge_line(&mut self, i: usize, dir: i8) {
        let (mut order, mut spots) = self.nine_now();
        let j = i as i32 + dir as i32;
        if order.len() != 9 || j < 0 || j >= order.len() as i32 {
            return;
        }
        let j = j as usize;
        order.swap(i, j);
        spots.swap(i, j);
        let _ = self.write_nine(order, spots);
    }

    fn cycle_spot(&mut self, i: usize) {
        let (order, mut spots) = self.nine_now();
        if order.len() != 9 || i >= spots.len() {
            return;
        }
        let start = team::Pos::FIELD
            .iter()
            .position(|p| *p == spots[i])
            .unwrap_or(0);
        for step in 1..=9 {
            spots[i] = team::Pos::FIELD[(start + step) % 9];
            if self.s.team.set_lineup(&order, &spots) {
                return;
            }
        }
    }

    fn assign_slot(&mut self, player: usize) {
        let Mode::Lineup { pick: Some(slot) } = self.mode else {
            return;
        };
        let (mut order, mut spots) = self.nine_now();
        if order.len() != 9
            || slot >= order.len()
            || player >= self.s.team.players.len()
            || !self.s.team.players[player].can_play()
        {
            return;
        }
        if let Some(other) = order.iter().position(|&i| i == player) {
            order.swap(slot, other);
            spots.swap(slot, other);
        } else {
            order[slot] = player;
        }
        let _ = self.write_nine(order, spots);
        self.mode = Mode::Lineup { pick: None };
    }

    fn open_bench(&mut self, kind: u8) {
        if let Mode::Match { shown, .. } = self.mode {
            self.mode = Mode::Match {
                shown,
                t: 0.0,
                asking: Some(Spot::Bench(kind)),
            };
        }
    }

    fn use_bench(&mut self, i: u8) {
        let Mode::Match {
            shown,
            asking: Some(Spot::Bench(kind)),
            ..
        } = self.mode
        else {
            return;
        };
        let ok = self.game.as_mut().is_some_and(|g| match kind {
            0 => g.pinch_hit(i as usize),
            1 => g.pinch_run(i as usize),
            _ => g.defense_sub(i as usize),
        });
        if ok {
            self.mode = Mode::Match {
                shown,
                t: self.spec.line_secs,
                asking: None,
            };
        }
    }

    fn press(&mut self, b: hud::Btn) {
        match b {
            hud::Btn::Card(i) => self.play_card(i),
            hud::Btn::Roster => {
                self.roster = crate::hud_art::Scroll::default();
                self.mode = Mode::Roster;
            }
            hud::Btn::Call(t) => self.give_call(t),
            hud::Btn::Auto => self.leave_to_team(),
            hud::Btn::Raise(st) => {
                if let Mode::Player(i) = self.mode {
                    let before = self.s.team.players[i].clone();
                    if self.s.team.players[i].raise(st) {
                        crate::mixer::cue_sfx("train");
                        self.pop_growth(i, &before);
                    }
                }
            }
            hud::Btn::AutoRaise => {
                if let Mode::Player(i) = self.mode {
                    let before = self.s.team.players[i].clone();
                    self.s.team.players[i].auto_raise();
                    self.pop_growth(i, &before);
                } else {
                    for p in &mut self.s.team.players {
                        p.auto_raise();
                    }
                }
                crate::mixer::cue_sfx("train");
            }
            hud::Btn::Menu(d) => {
                if let Mode::Player(i) = self.mode {
                    self.s.team.players[i].menu = d;
                    crate::mixer::cue_sfx("card");
                }
            }
            hud::Btn::Back => match self.mode {
                Mode::Epilogue { schools: true } => {
                    self.mode = Mode::Epilogue { schools: false };
                }
                Mode::Lineup { pick: Some(_) } => {
                    self.mode = Mode::Lineup { pick: None };
                }
                _ => self.mode = Mode::Roster,
            },
            hud::Btn::Row(i) => {
                if i < self.s.team.players.len() {
                    self.mode = Mode::Player(i);
                    crate::mixer::cue_sfx("card");
                }
            }
            hud::Btn::Close => {
                self.mode = if std::mem::take(&mut self.lineup_from_intro) {
                    Mode::MatchIntro
                } else {
                    Mode::Plan
                }
            }
            hud::Btn::PlayBall => self.play_ball(),
            hud::Btn::Continue => self.after_match(),
            hud::Btn::Speed => self.fast = !self.fast,
            hud::Btn::Orders => {
                self.want_call = !self.want_call;
                if self.want_call {
                    self.auto_orders = false;
                }
                crate::mixer::cue_sfx("card");
            }
            hud::Btn::Choose(k) => match self.mode {
                Mode::CampTalk { .. } => self.answer_camp(k),
                Mode::Bracket => self.day_before(k),
                Mode::Decide(d) => self.decide(d, k),
                _ => self.answer(k),
            },
            hud::Btn::Lineup => {
                self.lineup_from_intro = self.mode == Mode::MatchIntro;
                self.open_lineup();
            }
            hud::Btn::AutoFill => {
                self.s.team.clear_lineup();
                self.open_lineup();
            }
            hud::Btn::Slot(i) => {
                if matches!(self.mode, Mode::Lineup { .. }) {
                    self.mode = Mode::Lineup { pick: Some(i) };
                }
            }
            hud::Btn::Nudge(i, dir) => self.nudge_line(i, dir),
            hud::Btn::Cycle(i) => self.cycle_spot(i),
            hud::Btn::Assign(i) => self.assign_slot(i),
            hud::Btn::OpenBench(k) => self.open_bench(k),
            hud::Btn::UseBench(i) => self.use_bench(i),
            hud::Btn::Stay => self.continue_here(),
            hud::Btn::MoveSchool => {
                let mut picks: Vec<usize> = (0..self.s.rivals.len()).collect();
                picks.sort_by(|&a, &b| {
                    self.s.rivals[a]
                        .strength
                        .total_cmp(&self.s.rivals[b].strength)
                });
                picks.truncate(3);
                self.move_picks = picks;
                self.mode = Mode::Epilogue { schools: true };
            }
            hud::Btn::School(i) => self.continue_move(i as usize),
            hud::Btn::EndStory => self.finish_run(),
            hud::Btn::Skip => self.skip_game(),
            hud::Btn::RosterUp => self.roster.nudge(-(hud::ROW_H + 4.0) * 3.0),
            hud::Btn::RosterDown => self.roster.nudge((hud::ROW_H + 4.0) * 3.0),
        }
    }

    // ---- what the page reads

    pub fn state_value(&self) -> Value {
        let mode = match self.mode {
            Mode::Plan => "plan",
            Mode::Walk { .. } => "walk",
            Mode::Notice => "notice",
            Mode::Ask { .. } => "ask",
            Mode::Match { .. } => "match",
            Mode::MatchIntro => "match_intro",
            Mode::MatchEnd => "match_end",
            Mode::Roster => "roster",
            Mode::Player(_) => "player",
            Mode::Lineup { .. } => "lineup",
            Mode::CampTalk { .. } => "camp",
            Mode::Epilogue { .. } => "epilogue",
            Mode::Bracket => "bracket",
            Mode::Decide(Decide::Ace) => "numbers",
            Mode::Decide(Decide::Captain) => "captain",
        };
        json!({
            "phase": self.flow.phase.name(),
            "mode": mode,
            "year": self.s.year,
            "week": self.s.week,
            "pos": self.s.pos,
            "players": self.s.team.players.len(),
            "strength": self.s.team.strength(),
            "reputation": self.s.reputation,
            "wins": self.s.wins,
            "losses": self.s.losses,
            "score": self.core.stats.score,
            "best": self.flow.best_score,
            "win": self.s.wins > 0,
            "death": "",
            "death_text": "",
        })
    }
}

/// What a third-year says when his summer ends (by his part in the club; the rest pick
/// one of a few lines by name).
fn farewell(p: &team::Player) -> (&'static str, &'static str) {
    const LINES: [(&str, &str); 6] = [
        (
            "Three years of baseball. I was lucky.",
            "この三年間、野球ができて幸せでした",
        ),
        (
            "You lot had better get to the Summer Stadium.",
            "後輩たち、絶対に夏の大舞台へ行けよ",
        ),
        ("I swore I wouldn't cry...", "泣かないって決めてたのに…"),
        (
            "Coach, thank you for everything!",
            "監督、ありがとうございました！",
        ),
        (
            "I'm keeping this uniform forever.",
            "このユニフォーム、ずっと宝物にします",
        ),
        (
            "Next summer I'll be shouting from the stands.",
            "次は応援席から声を出すからな",
        ),
    ];
    match p.role {
        Role::Captain => (
            "Thanks for following me. It's yours now.",
            "みんな、ついてきてくれてありがとう。あとは頼んだぞ",
        ),
        Role::Ace => (
            "I wanted to throw one more inning...",
            "もう1イニング、投げたかったな…",
        ),
        _ if p.pitcher() => (
            "I'm glad I got to stand on that mound.",
            "あのマウンドに立てて、よかった",
        ),
        _ => LINES[art::hash(&p.family) as usize % LINES.len()],
    }
}

/// The line under a retiring third-year's face.
fn senior_title(p: &team::Player) -> (&'static str, &'static str) {
    match p.role {
        Role::Captain => ("Captain", "主将"),
        Role::Ace => ("Ace", "エース"),
        _ => (p.pos.short(false), p.pos.short(true)),
    }
}

fn fresh_save(seed: u32) -> Save {
    let mut rng = Rng::new(seed);
    let team = Team::founding(&mut rng);
    let hand = (0..5).map(|_| Card::draw(&mut rng, &team)).collect();
    Save {
        team,
        year: 1,
        week: 1,
        pos: 0,
        hand,
        reputation: 5.0,
        wins: 0,
        losses: 0,
        summer_in: true,
        autumn_in: true,
        national_in: false,
        results: Vec::new(),
        seed: rng.next_u32(),
        scouted: Vec::new(),
        healed: (0, 0),
        rivals: board::rival_schools(),
        slate: Vec::new(),
        spring_in: false,
        autumn_wins: 0,
        camp_year: 0,
        exam_year: 0,
        chapter: 1,
        grounds: 0,
        school: String::new(),
        school_ja: String::new(),
        career_wins: 0,
        career_losses: 0,
        epilogue: false,
        history: Vec::new(),
        year_from: (0, 0),
        pros: Vec::new(),
        cup: None,
        cup_path: Vec::new(),
        cup_scores: Vec::new(),
        seeded: false,
        studied: false,
        numbers_year: 0,
        draft_year: 0,
        selection_year: 0,
        winter_year: 0,
        seniors: Vec::new(),
        drafted: Vec::new(),
        captain: String::new(),
        need_captain: false,
        summer_won: 0,
    }
}

/// Old saves omit the new fields. Fill the circuit and the chapter so a resumed run
/// still has schools to play.
fn prepare_save(mut s: Save) -> Save {
    if s.rivals.is_empty() {
        s.rivals = board::rival_schools();
    }
    if s.chapter == 0 {
        s.chapter = 1;
    }
    s
}

// ------------------------------------------------------------ the host ----

impl Kit for DiamondDays {
    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }
    fn flow(&self) -> &GameFlow {
        &self.flow
    }
    fn flow_mut(&mut self) -> &mut GameFlow {
        &mut self.flow
    }
    fn step(&mut self, dt: f32) {
        self.step_sim(dt);
    }
    fn attract(&mut self, dt: f32) {
        self.clock += dt;
    }
    fn rebuild(&mut self) {
        self.update_doc();
    }
}

impl crate::kit_host::KitHost for DiamondDays {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn name(&self) -> &'static str {
        GAME_ID
    }
    fn phase_name(&self) -> &'static str {
        self.flow.phase.name()
    }
    fn playing(&self) -> bool {
        self.flow.phase == FlowPhase::Play
    }
    fn document(&self) -> &WorldDoc {
        &self.doc
    }
    fn dump_json(&self) -> String {
        crate::kit_host::dump_with_sim(&self.doc, &self.state_value())
    }
    fn state_json(&self) -> String {
        self.state_value().to_string()
    }
    fn confirm(&mut self) {
        DiamondDays::confirm(self)
    }
    fn show_title(&mut self) {
        DiamondDays::show_title(self)
    }
    fn pointer(&mut self, _x: f32, _y: f32, _width: u32, _height: u32) -> bool {
        false
    }
    fn on_key(&mut self, name: &str, down: bool) -> bool {
        if !down || self.flow.phase != FlowPhase::Play {
            return false;
        }
        match (name, &self.mode) {
            ("1" | "2" | "3" | "4" | "5", Mode::Plan) => {
                let i = name.parse::<usize>().unwrap_or(1) - 1;
                self.play_card(i);
            }
            ("Enter" | " ", Mode::Notice) => self.tap_notice(),
            ("1" | "2", Mode::Ask { .. }) => {
                let k = name.parse::<usize>().unwrap_or(1) - 1;
                self.answer(k);
            }
            ("1" | "2", Mode::CampTalk { .. }) => {
                let k = name.parse::<usize>().unwrap_or(1) - 1;
                self.answer_camp(k);
            }
            (
                "Enter" | " ",
                Mode::Match {
                    asking: Some(_), ..
                },
            ) => self.give_call(None),
            _ => return false,
        }
        true
    }
    fn tick(&mut self, dt: f32) {
        tick_frame(self, dt);
    }
    fn hud(&self, width: u32, height: u32, menu: bool) -> DrawList {
        DiamondDays::hud(self, width, height, menu)
    }
    fn walk(&mut self, _lx: f32, _lz: f32, _jump: bool) {}
    fn save_json(&self) -> Option<String> {
        match self.flow.phase {
            // In a run (paused too). Mid-game the week has not moved yet, so a reload
            // replays the game; mid-walk, the week as it was before the card.
            FlowPhase::Play | FlowPhase::Pause if self.s.year <= self.spec.years => {
                serde_json::to_string(self.pre_walk.as_ref().unwrap_or(&self.s)).ok()
            }
            // On the title screen a kept run waits to be resumed: keep it.
            FlowPhase::Title => self
                .resume
                .as_ref()
                .and_then(|r| serde_json::to_string(r).ok()),
            // Three years done: the next visit starts a new run.
            _ => Some("{}".into()),
        }
    }
    fn touch(&mut self, ev: PointerEvent, w: u32, h: u32, _t: f32) {
        self.touch_ev(ev, w, h);
    }
    fn uses_swipe(&self) -> bool {
        true
    }
    fn set_lang(&mut self, lang: &str) {
        self.lang_ja = lang.eq_ignore_ascii_case("ja");
    }
    fn pause_run(&mut self) {
        self.pause();
    }
    fn resume_run(&mut self) {
        self.flow.resume();
    }
}
