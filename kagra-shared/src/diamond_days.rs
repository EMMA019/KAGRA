//! 白球デイズ (Diamond Days): an original high-school baseball manager. Three years as the
//! coach of a small club: pick a step card each week, walk the ring of practice squares,
//! play practice games and tournaments by giving orders, watch players graduate and
//! first-years join. Design: `docs/design/diamond_days.md`.
//!
//! - `team.rs`: players, growth, the line-up, graduation and recruits.
//! - `board.rs`: the ring, the cards, events, the calendar.
//! - `sim.rs`: the game, plate appearance by plate appearance.
//! - `hud.rs`, `doc.rs`: the HUD and the 3D picture.

mod board;
mod doc;
mod hud;
mod sim;
pub mod team;
#[cfg(test)]
mod tests;

use std::cell::Cell;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::flow::{FlowPhase, GameFlow};
use crate::game_core::anim::{Animator, MoveSet};
use crate::game_core::swarm::Rng;
use crate::game_core::{tick_frame, Core, Kit};
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::DrawList;
use crate::world_doc::WorldDoc;
use board::{Card, Fixture, Happening, Square};
use sim::{Game, Order, Side};
use team::{Mood, Team};

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

const COACH_GLB: &str = "assets/models/kenney/mini-characters/character-male-e.glb";
const PLAYER_GLB: &str = "assets/models/kenney/mini-characters/character-male-a.glb";
const PITCHER_GLB: &str = "assets/models/kenney/mini-characters/character-male-c.glb";

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
    /// A game in progress: lines shown, seconds to the next, waiting for an order.
    Match { shown: usize, t: f32, asking: bool },
    /// The roster list.
    Roster,
}

/// A message on the notice card.
#[derive(Clone, Debug, Default, PartialEq)]
struct Note {
    title: String,
    title_ja: String,
    lines: Vec<(String, String)>,
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
    coach: Animator,
    roster_scroll: usize,
    /// A saved run to resume (from `world.save`).
    resume: Option<Save>,
    /// `?week=13` on the preview link: a fresh run starts at that week (to try a game).
    start_week: Option<u8>,
    clock: f32,
    lang_ja: bool,
    size: Cell<(f32, f32)>,
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
            coach: Animator::new(MoveSet::procedural()),
            roster_scroll: 0,
            resume: None,
            start_week: None,
            clock: 0.0,
            lang_ja: false,
            size: Cell::new((390.0, 844.0)),
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
                    g.resume = Some(s);
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
                self.flow.enter_play();
                crate::mixer::cue_sfx("go");
                crate::mixer::play_bgm("bgm", 0.5);
                if self.s.week == 1 && self.s.year == 1 && self.s.wins + self.s.losses == 0 {
                    self.note(
                        "A new coach",
                        "新しい監督",
                        vec![
                            (
                                format!("You are the new coach of {}. The club is small and has never won much.", self.spec.school),
                                format!("あなたは{}野球部の新しい監督。部員は少なく、勝った記憶もあまりない。", self.spec.school_ja),
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
        });
        self.mode = Mode::Notice;
    }

    /// The start of a week: a game if the calendar has one for us, else the cards.
    fn begin_week(&mut self) {
        if let Some(f) = board::fixture(self.s.week) {
            let ours = match f {
                Fixture::Practice => true,
                Fixture::Summer(_) => self.s.summer_in,
                Fixture::National(_) => self.s.national_in,
                Fixture::Autumn(_) => self.s.autumn_in,
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
        }
    }

    fn start_game(&mut self, f: Fixture) {
        let us = Side::from_team(&self.s.team, &self.spec.school, &self.spec.school_ja);
        let (en, ja) = board::rival_name(f, self.s.year, self.s.week);
        let them = Side::rival(&mut self.rng, &en, &ja, board::rival_level(f, self.s.year));
        let first = self.rng.unit() < 0.5;
        let seed = self.rng.next_u32();
        let mut g = Game::new(us, them, first, seed);
        let (title, title_ja) = Self::fixture_name(f);
        g.log.push(sim::Play {
            en: format!("{title}: {} vs {}.", g.sides[0].name, g.sides[1].name),
            ja: format!(
                "{title_ja}：{} 対 {}。",
                g.sides[0].name_ja, g.sides[1].name_ja
            ),
            runs: 0,
        });
        g.log.push(sim::Play {
            en: "Top of inning 1.".into(),
            ja: "1回表。".into(),
            runs: 0,
        });
        self.game = Some(g);
        self.mode = Mode::Match {
            shown: 0,
            t: 0.0,
            asking: false,
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
        for p in &mut self.s.team.players {
            p.tire(if p.pitcher() { 25 } else { 12 });
            p.mood = if won { p.mood.up() } else { p.mood.down() };
        }
        let mut lines = vec![(
            format!(
                "{} {us} - {them} {}",
                self.spec.school,
                g.sides[1 - g.us].name
            ),
            format!(
                "{} {us} - {them} {}",
                self.spec.school_ja,
                g.sides[1 - g.us].name_ja
            ),
        )];
        let rep = |r: &mut f32, d: f32| *r = (*r + d).clamp(0.0, 100.0);
        match f {
            Fixture::Practice => rep(&mut self.s.reputation, if won { 1.0 } else { 0.0 }),
            Fixture::Summer(r) => {
                if won {
                    rep(&mut self.s.reputation, 2.0 + r as f32);
                    if r == 5 {
                        self.s.national_in = true;
                        rep(&mut self.s.reputation, 10.0);
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
                    rep(&mut self.s.reputation, 6.0);
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
                    rep(&mut self.s.reputation, 1.5 + r as f32);
                } else {
                    self.s.autumn_in = false;
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
                4
            } else if t.contains("Stadium") {
                3
            } else if t.contains("Prefectural champions") {
                2
            } else if t.starts_with("Summer") {
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
        let card = self.s.hand.remove(i);
        self.s.hand.push(Card::draw(&mut self.rng));
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
        let (title, title_ja) = (sq.label(false).to_string(), sq.label(true).to_string());
        let mut lines = Vec::new();
        match sq {
            Square::Train(d) => {
                let ups =
                    board::train_week(&mut self.s.team, d, card.focus, self.spec.train_points);
                crate::mixer::cue_sfx("train");
                let doubled = card.focus == Some(d);
                lines.push((
                    format!(
                        "{} practice{}. {ups} stats went up.",
                        sq.label(false),
                        if doubled { " (focus: double)" } else { "" }
                    ),
                    format!(
                        "{}練習{}。能力が{ups}つ上がった。",
                        sq.label(true),
                        if doubled { "（重点：2倍）" } else { "" }
                    ),
                ));
                // A worn-out player can get hurt.
                let tired: Vec<usize> = (0..self.s.team.players.len())
                    .filter(|&k| {
                        self.s.team.players[k].stamina < 25 && self.s.team.players[k].hurt == 0
                    })
                    .collect();
                for k in tired {
                    if self.rng.unit() < 0.15 {
                        self.s.team.players[k].hurt = 2;
                        let n = self.s.team.players[k].family.clone();
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
            Square::Event => {
                let list = self.spec.happenings.clone();
                if let Some(h) = board::pick_happening(&mut self.rng, &list) {
                    let hurt = board::apply(&mut self.rng, h, &mut self.s.team);
                    self.s.reputation = (self.s.reputation + h.reputation).clamp(0.0, 100.0);
                    let mut en = h.text.clone();
                    let mut ja = h.text_ja.clone();
                    if let Some(n) = hurt {
                        en = format!("{en} ({n})");
                        ja = format!("{ja}（{n}）");
                    }
                    lines.push((en, ja));
                    crate::mixer::cue_sfx("event");
                }
            }
        }
        self.note(&title, &title_ja, lines);
        self.end_week();
    }

    /// The week is done: healing, rest, the next week, the year's turn.
    fn end_week(&mut self) {
        for p in &mut self.s.team.players {
            p.hurt = p.hurt.saturating_sub(1);
            p.rest(self.spec.weekly_rest);
        }
        self.s.week += 1;
        if self.s.week > board::WEEKS {
            self.end_year();
        }
    }

    fn end_year(&mut self) {
        let left = self.s.team.graduate();
        let y = self.s.year as usize;
        while self.s.results.len() < y {
            self.s.results.push(("-".into(), "-".into()));
        }
        if self.s.year >= self.spec.years {
            self.finish_run();
            return;
        }
        self.s.year += 1;
        self.s.week = 1;
        self.s.summer_in = true;
        self.s.autumn_in = true;
        self.s.national_in = false;
        let joined = self.s.team.recruit(&mut self.rng, self.s.reputation);
        self.note(
            "Spring",
            "春",
            vec![
                (
                    format!("{left} third-years graduated. Thank you, seniors."),
                    format!("3年生{left}人が卒業。先輩、ありがとう。"),
                ),
                (
                    format!(
                        "{joined} first-years joined the club. Year {} begins.",
                        self.s.year
                    ),
                    format!(
                        "新入部員が{joined}人入った。{}年目のはじまり。",
                        self.s.year
                    ),
                ),
            ],
        );
    }

    fn finish_run(&mut self) {
        let score = self.s.wins * 100 + self.s.reputation as u32 * 10;
        crate::mixer::stop_bgm(0.6);
        self.flow.finish(score, self.s.wins, "GRADUATION");
        // A new run next time.
        self.resume = None;
        self.s.year = self.spec.years + 1;
    }

    // ---- sim

    fn step_sim(&mut self, dt: f32) {
        self.clock += dt;
        let speed = self.coach_speed();
        self.coach.locomote(speed, true, 0.0);
        self.coach.tick(dt);
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
                if asking {
                    return;
                }
                let t = t + dt;
                if t < self.spec.line_secs {
                    self.mode = Mode::Match { shown, t, asking };
                    return;
                }
                self.advance_game(shown);
            }
            _ => {}
        }
    }

    fn coach_speed(&self) -> f32 {
        if matches!(self.mode, Mode::Walk { .. }) {
            2.0
        } else {
            0.0
        }
    }

    /// Show the next line of the game (playing it out as needed), or ask for an order.
    fn advance_game(&mut self, shown: usize) {
        let Some(g) = self.game.as_mut() else {
            return;
        };
        if shown < g.log.len() {
            let line = g.log[shown].clone();
            if line.ja.contains("ホームラン") {
                crate::mixer::cue_sfx("homer");
            } else if line.runs > 0 || line.ja.contains("ヒット") || line.ja.contains("ベース")
            {
                crate::mixer::cue_sfx("hit");
            }
            // A new half at bat for us: the coach gives an order first.
            let ours_starts = (line.ja.ends_with("回表。") || line.ja.ends_with("回裏。"))
                && g.we_bat()
                && !g.over;
            self.mode = Mode::Match {
                shown: shown + 1,
                t: 0.0,
                asking: ours_starts && !self.auto_orders,
            };
            return;
        }
        if g.over {
            self.end_game();
            return;
        }
        g.step();
        self.mode = Mode::Match {
            shown,
            t: self.spec.line_secs,
            asking: false,
        };
    }

    fn give_order(&mut self, o: Option<Order>) {
        if let (Some(g), Mode::Match { shown, .. }) = (self.game.as_mut(), self.mode.clone()) {
            match o {
                Some(o) => g.order = o,
                None => {
                    g.order = Order::Normal;
                    self.auto_orders = true;
                }
            }
            crate::mixer::cue_sfx("card");
            self.mode = Mode::Match {
                shown,
                t: 0.0,
                asking: false,
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
            self.mode = Mode::Plan;
            self.begin_week();
        }
    }

    // ---- input

    pub fn touch_ev(&mut self, ev: PointerEvent, w: u32, h: u32) {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        self.size.set((w, h));
        if ev.phase != PointerPhase::Begin || self.flow.phase != FlowPhase::Play {
            return;
        }
        if let Some(b) = crate::hud_kit::hit(&self.hud_buttons(w, h), ev.x, ev.y) {
            self.press(b);
            return;
        }
        match self.mode {
            Mode::Notice => self.tap_notice(),
            Mode::Roster => self.mode = Mode::Plan,
            Mode::Match {
                asking: false,
                shown,
                ..
            } => {
                // A tap hurries the next line.
                self.advance_game(shown);
            }
            _ => {}
        }
    }

    fn press(&mut self, b: hud::Btn) {
        match b {
            hud::Btn::Card(i) => self.play_card(i),
            hud::Btn::Roster => {
                self.roster_scroll = 0;
                self.mode = Mode::Roster;
            }
            hud::Btn::Order(o) => self.give_order(o),
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
            Mode::Match { .. } => "match",
            Mode::Roster => "roster",
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
    let hand = (0..5).map(|_| Card::draw(&mut rng)).collect();
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
    }
}

fn hud_scale(w: f32, h: f32) -> f32 {
    (w.min(h) / 400.0).clamp(0.9, 2.6)
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
            ("Enter" | " ", Mode::Match { asking: true, .. }) => {
                self.give_order(Some(Order::Normal))
            }
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
        if self.flow.phase == FlowPhase::Play && self.s.year <= self.spec.years {
            // Mid-game, keep the week before the game: it replays from its start.
            serde_json::to_string(&self.s).ok()
        } else {
            Some("{}".into())
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
