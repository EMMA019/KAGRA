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
use team::{Ability, Mood, Role, Team};

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
}

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
}

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
    roster_scroll: usize,
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
            roster_scroll: 0,
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
                self.s = match self.resume.take() {
                    Some(s) if self.flow.phase == FlowPhase::Title => s,
                    _ => fresh_save(self.spec.seed.wrapping_add(self.clock.to_bits())),
                };
                self.rng = Rng::new(self.s.seed);
                if let Some(w) = self.start_week.take() {
                    self.s.week = w;
                }
                self.notes.clear();
                self.game = None;
                self.pre_walk = None;
                // Calls are asked again in a new run.
                self.auto_orders = false;
                self.flow.enter_play();
                crate::mixer::cue_sfx("go");
                crate::mixer::play_bgm("bgm", 0.5);
                if self.s.week == 1 && self.s.year == 1 && self.s.wins + self.s.losses == 0 {
                    let (school, school_ja) =
                        (self.school_en().to_string(), self.school_ja().to_string());
                    self.note(
                        "A new coach",
                        "新しい監督",
                        vec![
                            (
                                format!("You are the new coach of {school}. The club is small and has never won much."),
                                format!("あなたは{school_ja}野球部の新しい監督。部員は少なく、勝った記憶もあまりない。"),
                            ),
                            (
                                "Pick a card each week and walk the ring. Train, rest, and take them to the Summer Stadium.".into(),
                                "毎週カードを選んで盤を進もう。鍛えて、休ませて、夏の大舞台へ。".into(),
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
        if self.s.week == board::CAMP_WEEK && self.s.camp_year != self.s.year {
            self.run_camp();
            return;
        }
        if self.s.week == board::EXAM_WEEK && self.s.exam_year != self.s.year {
            self.run_exams();
            return;
        }
        if let Some(f) = board::fixture(self.s.week) {
            let ours = match f {
                Fixture::Practice => true,
                Fixture::Summer(_) => self.s.summer_in,
                Fixture::National(_) => self.s.national_in,
                Fixture::Autumn(_) => self.s.autumn_in,
                Fixture::Spring(_) => self.s.spring_in,
            };
            if ours && self.game.is_none() {
                self.start_game(f);
                return;
            }
        }
        if self.notes.is_empty() {
            self.mode = Mode::Plan;
        }
    }

    fn fixture_name(f: Fixture) -> (String, String) {
        match f {
            Fixture::Practice => ("Practice game".into(), "練習試合".into()),
            Fixture::Summer(5) => ("Summer prefectural final".into(), "夏の県大会 決勝".into()),
            Fixture::Summer(r) => (
                format!("Summer prefectural, round {r}"),
                format!("夏の県大会 {r}回戦"),
            ),
            Fixture::National(r) => (
                format!("Summer Stadium, round {r}"),
                format!("夏の大舞台 {r}回戦"),
            ),
            Fixture::Autumn(3) => ("Autumn tournament final".into(), "秋季大会 決勝".into()),
            Fixture::Autumn(r) => (
                format!("Autumn tournament, round {r}"),
                format!("秋季大会 {r}回戦"),
            ),
            Fixture::Spring(3) => (
                "Blossom Invitational final".into(),
                "春の招待舞台 決勝".into(),
            ),
            Fixture::Spring(r) => (
                format!("Blossom Invitational, round {r}"),
                format!("春の招待舞台 {r}回戦"),
            ),
        }
    }

    fn start_game(&mut self, f: Fixture) {
        let (school, school_ja) = (self.school_en().to_string(), self.school_ja().to_string());
        let us = Side::from_team(&self.s.team, &school, &school_ja);
        let idx = self.pick_rival(f);
        self.rival_idx = Some(idx);
        let (en, ja) = self
            .s
            .rivals
            .get(idx)
            .map(|s| (s.name.clone(), s.name_ja.clone()))
            .unwrap_or_else(|| board::rival_name(f, self.s.year, self.s.week));
        let rival = &self.s.rivals[idx];
        let ease = if matches!(f, Fixture::Spring(_)) {
            self.s.autumn_wins as f32 * 2.5
        } else {
            0.0
        };
        let level = (rival.strength * 0.72 + board::rival_level(f, self.s.year) * 0.28 - ease)
            .clamp(28.0, 92.0);
        let them = Side::rival(&mut self.rng, &en, &ja, level);
        let first = self.rng.unit() < 0.5;
        let seed = self.rng.next_u32();
        let mut g = Game::new(us, them, first, seed);
        let (title, title_ja) = Self::fixture_name(f);
        let spark = g.sides[g.us].spark.clone();
        g.log.push(sim::Play {
            en: format!("{title}: {} vs {}.", g.sides[0].name, g.sides[1].name),
            ja: format!(
                "{title_ja}：{} 対 {}。",
                g.sides[0].name_ja, g.sides[1].name_ja
            ),
            runs: 0,
            kind: sim::PlayKind::Note,
            dir: 0.0,
            banner: None,
            clutch: false,
        });
        if let Some(name) = spark {
            g.log.push(sim::Play {
                en: format!("Spark! {name} lifts the bench."),
                ja: format!("元気印！{name}がベンチを明るくする。"),
                runs: 0,
                kind: sim::PlayKind::Note,
                dir: 0.0,
                banner: Some(Ability::Mood),
                clutch: false,
            });
        }
        g.log.push(sim::Play {
            en: "Top of inning 1.".into(),
            ja: "1回表。".into(),
            runs: 0,
            kind: sim::PlayKind::Note,
            dir: 0.0,
            banner: None,
            clutch: false,
        });
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
        let won = g.won() == Some(true);
        let f = board::fixture(self.s.week).unwrap_or(Fixture::Practice);
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
        match f {
            Fixture::Practice => self.bump_rep(if won { 0.5 } else { 0.0 }),
            Fixture::Summer(r) => {
                if won {
                    self.bump_rep(1.0 + r as f32 * 0.5);
                    if r == 5 {
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
                    self.record(&format!("Summer round {r}"), &format!("夏 {r}回戦敗退"));
                }
            }
            Fixture::National(r) => {
                if won {
                    self.bump_rep(6.0);
                    if r == 2 {
                        self.record("Summer Stadium champions", "夏の大舞台 優勝");
                        lines.push((
                            "National champions! The whole town is out in the street.".into(),
                            "全国制覇！町じゅうが大騒ぎ。".into(),
                        ));
                    }
                } else {
                    self.s.national_in = false;
                    self.record(
                        &format!("Summer Stadium round {r}"),
                        &format!("夏の大舞台 {r}回戦"),
                    );
                }
            }
            Fixture::Autumn(r) => {
                if won {
                    self.s.autumn_wins = self.s.autumn_wins.saturating_add(1);
                    self.bump_rep(1.0 + r as f32 * 0.5);
                    if r == 3 {
                        self.record("Autumn champions", "秋季大会優勝");
                    }
                } else {
                    self.s.autumn_in = false;
                }
                if !self.s.autumn_in || r == 3 {
                    self.s.spring_in = self.s.autumn_wins > 0;
                    if self.s.spring_in {
                        let seeded = self.s.autumn_wins >= 3;
                        lines.push((
                            if seeded {
                                "Seeded for the Blossom Invitational in March.".into()
                            } else {
                                "Invited to the Blossom Invitational in March.".into()
                            },
                            if seeded {
                                "3月の春の招待舞台、シードで出場。".into()
                            } else {
                                "3月の春の招待舞台に招待された。".into()
                            },
                        ));
                    }
                }
            }
            Fixture::Spring(r) => {
                if won {
                    self.bump_rep(2.0 + r as f32);
                    if r == 3 {
                        self.record("Blossom Invitational champions", "春の招待舞台 優勝");
                        lines.push((
                            "The Blossom Invitational is yours.".into(),
                            "春の招待舞台を制した！".into(),
                        ));
                    }
                } else {
                    self.s.spring_in = false;
                    self.record(
                        &format!("Blossom Invitational round {r}"),
                        &format!("春の招待舞台 {r}回戦"),
                    );
                }
            }
        }
        self.note(
            if won { "Win!" } else { "Lost" },
            if won { "勝利！" } else { "敗戦" },
            std::iter::once((title, title_ja)).chain(lines).collect(),
        );
        self.end_week();
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
                    self.spec.train_points * (1.0 + self.s.grounds as f32 * 0.08),
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
        let leaving: Vec<(String, String, Role, team::After)> = self
            .s
            .team
            .players
            .iter()
            .filter(|p| p.grade == 3)
            .map(|p| {
                (
                    p.family.clone(),
                    p.given.clone(),
                    p.role,
                    team::destination(p, self.s.reputation),
                )
            })
            .collect();
        let names = |t: &Team, grade: u8| -> Vec<(String, String)> {
            t.players
                .iter()
                .filter(|p| p.grade == grade)
                .map(|p| (p.family.clone(), p.given.clone()))
                .collect()
        };
        let left = self.s.team.graduate();
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
        self.s.summer_in = true;
        self.s.autumn_in = true;
        self.s.national_in = false;
        self.s.spring_in = false;
        self.s.autumn_wins = 0;
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
                format!("{n} tweaked something (1 week)."),
                format!("{n}が少し痛めた（1週間）。"),
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
        self.s.summer_in = true;
        self.s.autumn_in = true;
        self.s.national_in = false;
        self.s.spring_in = false;
        self.s.autumn_wins = 0;
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
        self.s.summer_in = true;
        self.s.autumn_in = true;
        self.s.national_in = false;
        self.s.spring_in = false;
        self.s.autumn_wins = 0;
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
        self.clock += dt;
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
                if asking.is_some() {
                    return;
                }
                let t = t + dt;
                let pace = self.line_pace(shown);
                if t < self.spec.line_secs * pace {
                    self.mode = Mode::Match { shown, t, asking };
                    return;
                }
                self.advance_game(shown);
            }
            _ => {}
        }
    }

    /// Clutch plays and ability shouts stay on screen longer. Not a pitch count.
    fn line_pace(&self, shown: usize) -> f32 {
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
        // A chance or a pinch: the coach calls it (unless the team plays on its own).
        if let Some(spot) = g.spot() {
            if self.auto_orders {
                g.call(None);
            } else {
                self.mode = Mode::Match {
                    shown,
                    t: 0.0,
                    asking: Some(spot),
                };
                return;
            }
        }
        g.step();
        self.mode = Mode::Match {
            shown,
            t: self.spec.line_secs,
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

    /// The team plays the rest of the game on its own (no more calls).
    fn leave_to_team(&mut self) {
        self.auto_orders = true;
        self.give_call(None);
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
        if let Some(b) = crate::hud_kit::hit(&self.hud_buttons(w, h), ev.x, ev.y) {
            self.press(b);
            return;
        }
        match self.mode {
            Mode::Notice => self.tap_notice(),
            // A tap off the list closes it.
            Mode::Roster => self.mode = Mode::Plan,
            Mode::Lineup { .. } => self.mode = Mode::Roster,
            Mode::Player(_) => self.mode = Mode::Roster,
            Mode::Match {
                asking: None,
                shown,
                ..
            } => {
                // A tap hurries the next line.
                self.advance_game(shown);
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
                self.roster_scroll = 0;
                self.mode = Mode::Roster;
            }
            hud::Btn::Call(t) => self.give_call(t),
            hud::Btn::Auto => self.leave_to_team(),
            hud::Btn::Raise(st) => {
                if let Mode::Player(i) = self.mode {
                    if self.s.team.players[i].raise(st) {
                        crate::mixer::cue_sfx("train");
                    }
                }
            }
            hud::Btn::AutoRaise => {
                if let Mode::Player(i) = self.mode {
                    self.s.team.players[i].auto_raise();
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
            hud::Btn::Close => self.mode = Mode::Plan,
            hud::Btn::Choose(k) => {
                if matches!(self.mode, Mode::CampTalk { .. }) {
                    self.answer_camp(k);
                } else {
                    self.answer(k);
                }
            }
            hud::Btn::Lineup => self.open_lineup(),
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
            hud::Btn::RosterUp => self.roster_scroll = self.roster_scroll.saturating_sub(4),
            hud::Btn::RosterDown => {
                self.roster_scroll =
                    (self.roster_scroll + 4).min(self.s.team.players.len().saturating_sub(1))
            }
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
            Mode::Roster => "roster",
            Mode::Player(_) => "player",
            Mode::Lineup { .. } => "lineup",
            Mode::CampTalk { .. } => "camp",
            Mode::Epilogue { .. } => "epilogue",
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
