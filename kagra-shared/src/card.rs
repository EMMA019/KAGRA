//! 1v1 card duel (`genre: "card"`).
//!
//! Deck, seeded shuffle, draw, hand, mana, play, attack, and a small
//! opponent AI live here. `CardSession` is the host surface (Python, and
//! later a net client): `dump` / `from_json` round-trip the match, including
//! the hidden deck order and the opponent hand. The picture is a `WorldDoc`
//! table. HUD copy is Japanese and covered by the glyph test. This module
//! does not plug into `WorldPlay`.

use serde::{Deserialize, Serialize};

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::world_doc::{WorldDoc, WorldEvent, WorldTimer};

pub const GAME_ID: &str = "card";

const LIFE: i32 = 20;
const MANA_CAP: u8 = 10;
const HAND_MAX: usize = 7;
const BOARD_MAX: usize = 5;
const OPENING: usize = 3;
const LOG_MAX: usize = 64;

const TITLE: &str = "カード対戦";
const HINT_START: &str = "タップではじめる";
const HINT_PLAY: &str = "ドラッグして出す";
const HINT_FOE: &str = "相手の番";
const YOU_TURN: &str = "あなたの番";
const YOU_WIN: &str = "勝ち";
const YOU_LOSE: &str = "負け";
const LABEL_END: &str = "エンド";
const LABEL_REMATCH: &str = "再戦";
const LABEL_PILE: &str = "山札";
const LABEL_HAND: &str = "手札";
const WORD_MANA: &str = "マナ";
const WORD_LIFE: &str = "体力";
const WORD_TURN: &str = "ターン";
const WORD_VS: &str = "対";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Unit,
    Spell,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Unit => "unit",
            Kind::Spell => "spell",
        }
    }
}

struct Def {
    id: u16,
    name: &'static str,
    cost: u8,
    kind: Kind,
    atk: u8,
    hp: u8,
    art: u8,
    line: &'static str,
}

const CATALOG: [Def; 15] = [
    Def {
        id: 1,
        name: "狼",
        cost: 1,
        kind: Kind::Unit,
        atk: 1,
        hp: 2,
        art: 0,
        line: "攻撃",
    },
    Def {
        id: 2,
        name: "見習",
        cost: 1,
        kind: Kind::Unit,
        atk: 2,
        hp: 1,
        art: 1,
        line: "攻撃",
    },
    Def {
        id: 3,
        name: "守衛",
        cost: 2,
        kind: Kind::Unit,
        atk: 2,
        hp: 3,
        art: 2,
        line: "攻撃",
    },
    Def {
        id: 4,
        name: "弓兵",
        cost: 2,
        kind: Kind::Unit,
        atk: 3,
        hp: 1,
        art: 3,
        line: "攻撃",
    },
    Def {
        id: 5,
        name: "騎士",
        cost: 3,
        kind: Kind::Unit,
        atk: 3,
        hp: 3,
        art: 4,
        line: "攻撃",
    },
    Def {
        id: 6,
        name: "僧侶",
        cost: 3,
        kind: Kind::Unit,
        atk: 2,
        hp: 4,
        art: 5,
        line: "攻撃",
    },
    Def {
        id: 7,
        name: "魔道",
        cost: 4,
        kind: Kind::Unit,
        atk: 4,
        hp: 3,
        art: 1,
        line: "攻撃",
    },
    Def {
        id: 8,
        name: "巨人",
        cost: 5,
        kind: Kind::Unit,
        atk: 5,
        hp: 5,
        art: 2,
        line: "攻撃",
    },
    Def {
        id: 9,
        name: "竜",
        cost: 6,
        kind: Kind::Unit,
        atk: 6,
        hp: 6,
        art: 0,
        line: "攻撃",
    },
    Def {
        id: 10,
        name: "王",
        cost: 7,
        kind: Kind::Unit,
        atk: 6,
        hp: 7,
        art: 4,
        line: "攻撃",
    },
    Def {
        id: 11,
        name: "火花",
        cost: 1,
        kind: Kind::Spell,
        atk: 2,
        hp: 0,
        art: 3,
        line: "敵へ",
    },
    Def {
        id: 12,
        name: "矢",
        cost: 2,
        kind: Kind::Spell,
        atk: 3,
        hp: 0,
        art: 3,
        line: "敵へ",
    },
    Def {
        id: 13,
        name: "炎",
        cost: 3,
        kind: Kind::Spell,
        atk: 4,
        hp: 0,
        art: 3,
        line: "敵へ",
    },
    Def {
        id: 14,
        name: "雷",
        cost: 4,
        kind: Kind::Spell,
        atk: 5,
        hp: 0,
        art: 3,
        line: "敵へ",
    },
    Def {
        id: 15,
        name: "治癒",
        cost: 2,
        kind: Kind::Spell,
        atk: 0,
        hp: 4,
        art: 5,
        line: "回復",
    },
];

/// Twenty cards. Cheap units and spells are duplicated. Every catalog id appears.
const DECK: [u16; 20] = [
    1, 2, 2, 3, 3, 4, 4, 5, 6, 7, 8, 9, 10, 11, 11, 12, 12, 13, 14, 15,
];

fn catalog(id: u16) -> Option<&'static Def> {
    CATALOG.iter().find(|d| d.id == id)
}

#[cfg(test)]
fn static_copy() -> Vec<&'static str> {
    let mut lines = vec![
        TITLE,
        HINT_START,
        HINT_PLAY,
        HINT_FOE,
        YOU_TURN,
        YOU_WIN,
        YOU_LOSE,
        LABEL_END,
        LABEL_REMATCH,
        LABEL_PILE,
        LABEL_HAND,
        WORD_MANA,
        WORD_LIFE,
        WORD_TURN,
        WORD_VS,
    ];
    for def in &CATALOG {
        lines.push(def.name);
        lines.push(def.line);
    }
    lines
}

struct Rng {
    state: u64,
}

impl Rng {
    fn next_u64(&mut self) -> u64 {
        // SplitMix64. Seed 0 is fine: the increment runs before the mix.
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

fn shuffle(deck: &mut [u16], rng: &mut Rng) {
    if deck.len() < 2 {
        return;
    }
    for i in (1..deck.len()).rev() {
        let j = (rng.next_u64() as usize) % (i + 1);
        deck.swap(i, j);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct CardInst {
    uid: u32,
    def: u16,
    atk: u8,
    hp: u8,
    #[serde(default)]
    sick: bool,
    #[serde(default)]
    attacked: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Side {
    life: i32,
    mana: u8,
    mana_max: u8,
    #[serde(default)]
    fatigue: u32,
    #[serde(default)]
    deck: Vec<u16>,
    #[serde(default)]
    hand: Vec<CardInst>,
    #[serde(default)]
    board: Vec<CardInst>,
    #[serde(default)]
    grave: Vec<u16>,
}

impl Default for Side {
    fn default() -> Self {
        Self {
            life: LIFE,
            mana: 0,
            mana_max: 0,
            fatigue: 0,
            deck: Vec::new(),
            hand: Vec::new(),
            board: Vec::new(),
            grave: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Act {
    t: u32,
    side: u8,
    op: String,
    uid: u32,
    def: u16,
    target: i32,
    n: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct CardState {
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default)]
    rng: u64,
    #[serde(default)]
    turn: u32,
    #[serde(default)]
    active: u8,
    #[serde(default)]
    winner: Option<u8>,
    #[serde(default)]
    phase: GamePhase,
    #[serde(default)]
    next_uid: u32,
    #[serde(default)]
    cursor: u32,
    #[serde(default)]
    deals: u32,
    #[serde(default)]
    you: Side,
    #[serde(default)]
    foe: Side,
    #[serde(default)]
    log: Vec<Act>,
}

fn default_seed() -> u64 {
    1
}

impl Default for CardState {
    fn default() -> Self {
        Self::title(1)
    }
}

impl CardState {
    fn title(seed: u64) -> Self {
        Self {
            seed,
            rng: seed,
            turn: 0,
            active: 0,
            winner: None,
            phase: GamePhase::Title,
            next_uid: 1,
            cursor: 0,
            deals: 0,
            you: Side::default(),
            foe: Side::default(),
            log: Vec::new(),
        }
    }
}

/// Duel session. The `card` object on the dump is the match. The rest is the table picture.
#[derive(Clone, Debug)]
pub struct CardSession {
    doc: WorldDoc,
    input: WalkInput,
    state: CardState,
    event_seq: u32,
}

impl CardSession {
    /// Load a world dump. `genre` must be [`GAME_ID`].
    /// A `card` object restores the match. Without one, the duel waits on the title.
    pub fn open(json: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let doc = WorldDoc::from_json(json)?;
        if !doc.genre_is(GAME_ID) {
            return Err(format!(
                "WorldDoc.genre must be {:?}, got {:?}",
                GAME_ID, doc.genre
            ));
        }
        let state = match value.get("card") {
            None | Some(serde_json::Value::Null) => CardState::title(1),
            Some(other) => serde_json::from_value(other.clone()).map_err(|e| e.to_string())?,
        };
        let mut session = Self {
            event_seq: event_seq_from(&doc),
            doc,
            input: WalkInput::default(),
            state,
        };
        session.validate()?;
        session.bump_uid();
        session.sync_flag();
        Ok(session)
    }

    pub fn apply_input(&mut self, input: WalkInput) {
        let input = input.clamped();
        let prev = self.input;
        self.input = input;
        if self.state.phase != GamePhase::Playing || self.state.active != 0 {
            return;
        }
        let n = self.state.you.hand.len();
        if input.lx > 0.5 && prev.lx <= 0.5 && n > 0 {
            self.state.cursor = (self.state.cursor + 1).min((n as u32) - 1);
        } else if input.lx < -0.5 && prev.lx >= -0.5 {
            self.state.cursor = self.state.cursor.saturating_sub(1);
        }
        if input.jump && !prev.jump {
            let index = self.state.cursor as usize;
            let _ = self.play(index);
        }
        if input.attack && !prev.attack {
            let _ = self.end_turn();
        }
    }

    pub fn step(&mut self, dt: f32) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.tick_timers(dt);
    }

    pub fn dump_json(&self) -> Result<String, String> {
        let mut value = serde_json::to_value(&self.doc).map_err(|e| e.to_string())?;
        value["card"] = serde_json::to_value(&self.state).map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
    }

    pub fn drain_events(&mut self, name: &str) -> Vec<WorldEvent> {
        let mut out = Vec::new();
        self.doc.events.retain(|event| {
            if event.name == name {
                out.push(event.clone());
                false
            } else {
                true
            }
        });
        out
    }

    pub fn confirm_action(&mut self) {
        match self.state.phase {
            GamePhase::Title | GamePhase::Complete => self.deal(),
            GamePhase::Playing => {}
        }
    }

    pub fn record_event(&mut self, name: &str, data: Option<serde_json::Value>) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(event) = self.doc.events.iter_mut().find(|event| event.name == name) {
            event.count += 1;
            if data.is_some() {
                event.data = data;
            }
            return;
        }
        self.event_seq += 1;
        self.doc.events.push(WorldEvent {
            id: format!("event:{}", self.event_seq),
            kind: "event".into(),
            name: name.into(),
            count: 1,
            data,
        });
    }

    pub fn begin_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        let id = format!("timer:{name}");
        if let Some(timer) = self.doc.timers.iter_mut().find(|timer| timer.id == id) {
            timer.name = name.into();
            timer.seconds = seconds;
            timer.remaining = seconds;
            timer.on_done = on_done.map(str::to_string);
            timer.active = true;
            return id;
        }
        self.doc.timers.push(WorldTimer {
            id: id.clone(),
            kind: "timer".into(),
            name: name.into(),
            seconds,
            remaining: seconds,
            on_done: on_done.map(str::to_string),
            active: true,
        });
        id
    }

    /// Play the card under the cursor. Stick hosts use this as the interact verb.
    pub fn use_nearby(&mut self) {
        if self.state.phase != GamePhase::Playing || self.state.active != 0 {
            return;
        }
        let index = self.state.cursor as usize;
        let _ = self.play(index);
    }

    pub fn current_phase(&self) -> GamePhase {
        self.state.phase
    }

    pub fn you_life(&self) -> i32 {
        self.state.you.life
    }

    pub fn foe_life(&self) -> i32 {
        self.state.foe.life
    }

    pub fn mana(&self) -> u8 {
        self.state.you.mana
    }

    pub fn mana_max(&self) -> u8 {
        self.state.you.mana_max
    }

    pub fn turn(&self) -> u32 {
        self.state.turn
    }

    pub fn active(&self) -> u8 {
        self.state.active
    }

    pub fn winner(&self) -> Option<u8> {
        self.state.winner
    }

    pub fn you_hand(&self) -> usize {
        self.state.you.hand.len()
    }

    pub fn foe_hand(&self) -> usize {
        self.state.foe.hand.len()
    }

    pub fn you_board(&self) -> usize {
        self.state.you.board.len()
    }

    pub fn foe_board(&self) -> usize {
        self.state.foe.board.len()
    }

    pub fn status(&self) -> String {
        match self.state.phase {
            GamePhase::Title => format!("{TITLE}  {HINT_START}"),
            GamePhase::Complete => match self.state.winner {
                Some(0) => format!("{YOU_WIN}  {LABEL_REMATCH}"),
                Some(1) => format!("{YOU_LOSE}  {LABEL_REMATCH}"),
                _ => LABEL_REMATCH.to_string(),
            },
            GamePhase::Playing if self.state.active == 0 => format!(
                "{YOU_TURN}  {WORD_TURN} {}  {WORD_MANA} {}/{}  {WORD_LIFE} {} {WORD_VS} {}",
                self.state.turn,
                self.state.you.mana,
                self.state.you.mana_max,
                self.state.you.life.max(0),
                self.state.foe.life.max(0)
            ),
            GamePhase::Playing => format!(
                "{HINT_FOE}  {WORD_TURN} {}  {WORD_LIFE} {} {WORD_VS} {}",
                self.state.turn,
                self.state.you.life.max(0),
                self.state.foe.life.max(0)
            ),
        }
    }

    pub fn hint(&self) -> &'static str {
        match self.state.phase {
            GamePhase::Title => HINT_START,
            GamePhase::Complete => LABEL_REMATCH,
            GamePhase::Playing if self.state.active != 0 => HINT_FOE,
            GamePhase::Playing => HINT_PLAY,
        }
    }

    /// Player-facing snapshot. The opponent hand and both decks stay as counts.
    pub fn view_json(&self) -> String {
        let view = serde_json::json!({
            "phase": phase_name(self.state.phase),
            "seed": self.state.seed,
            "turn": self.state.turn,
            "active": self.state.active,
            "winner": self.state.winner,
            "deals": self.state.deals,
            "cursor": self.state.cursor,
            "status": self.status(),
            "hint": self.hint(),
            "labels": {
                "end": LABEL_END,
                "rematch": LABEL_REMATCH,
                "pile": LABEL_PILE,
                "hand": LABEL_HAND,
                "life": WORD_LIFE,
                "mana": WORD_MANA,
                "win": YOU_WIN,
                "lose": YOU_LOSE,
            },
            "you": side_public(&self.state.you, true),
            "foe": side_public(&self.state.foe, false),
            "catalog": catalog_json(),
        });
        view.to_string()
    }

    /// Play a card from your hand. Index is into the current hand.
    pub fn play(&mut self, index: usize) -> bool {
        self.play_side(0, index)
    }

    /// Attack with your board unit. `target` -1 hits the opposing hero. Otherwise an opposing unit.
    pub fn attack(&mut self, index: usize, target: i32) -> bool {
        self.attack_side(0, index, target)
    }

    /// End your turn. The opponent plays immediately, then it is your turn again.
    pub fn end_turn(&mut self) -> bool {
        if self.state.phase != GamePhase::Playing || self.state.active != 0 {
            return false;
        }
        self.push_log(0, "end", 0, 0, -1, 0);
        self.begin_turn(1);
        if self.state.phase == GamePhase::Playing {
            self.run_ai();
        }
        if self.state.phase == GamePhase::Playing {
            self.begin_turn(0);
        }
        true
    }

    fn play_side(&mut self, side: u8, index: usize) -> bool {
        if self.state.phase != GamePhase::Playing || self.state.active != side {
            return false;
        }
        let (uid, def_id, atk, hp) = {
            let Some(card) = self.side(side).hand.get(index) else {
                return false;
            };
            (card.uid, card.def, card.atk, card.hp)
        };
        let Some(def) = catalog(def_id) else {
            return false;
        };
        if self.side(side).mana < def.cost {
            return false;
        }
        if def.kind == Kind::Unit && self.side(side).board.len() >= BOARD_MAX {
            return false;
        }
        let cost = def.cost;
        let kind = def.kind;
        let spell_atk = def.atk;
        let spell_heal = def.hp;
        {
            let player = self.side_mut(side);
            player.hand.remove(index);
            player.mana -= cost;
            if kind == Kind::Unit {
                player.board.push(CardInst {
                    uid,
                    def: def_id,
                    atk,
                    hp,
                    sick: true,
                    attacked: false,
                });
            } else {
                player.grave.push(def_id);
            }
        }
        if kind == Kind::Spell {
            self.resolve_spell(side, spell_atk, spell_heal);
        }
        self.clamp_cursor();
        self.push_log(side, "play", uid, def_id, -1, i32::from(cost));
        self.check_end();
        true
    }

    fn attack_side(&mut self, side: u8, index: usize, target: i32) -> bool {
        if self.state.phase != GamePhase::Playing || self.state.active != side {
            return false;
        }
        let enemy = side ^ 1;
        let (uid, def_id, atk, can) = {
            let Some(card) = self.side(side).board.get(index) else {
                return false;
            };
            (card.uid, card.def, card.atk, self.ready(card))
        };
        if !can {
            return false;
        }
        if target >= 0 {
            let Ok(ti) = usize::try_from(target) else {
                return false;
            };
            let Some(their) = self.side(enemy).board.get(ti).map(|card| card.atk) else {
                return false;
            };
            if let Some(card) = self.side_mut(enemy).board.get_mut(ti) {
                card.hp = card.hp.saturating_sub(atk);
            }
            if let Some(card) = self.side_mut(side).board.get_mut(index) {
                card.hp = card.hp.saturating_sub(their);
                card.attacked = true;
            }
            self.reap(side);
            self.reap(enemy);
        } else {
            self.side_mut(enemy).life -= i32::from(atk);
            if let Some(card) = self.side_mut(side).board.get_mut(index) {
                card.attacked = true;
            }
        }
        self.push_log(side, "attack", uid, def_id, target, i32::from(atk));
        self.check_end();
        true
    }

    fn resolve_spell(&mut self, side: u8, atk: u8, heal: u8) {
        let enemy = side ^ 1;
        if atk > 0 {
            self.side_mut(enemy).life -= i32::from(atk);
            self.push_log(side, "spell", 0, 0, -1, i32::from(atk));
        }
        if atk == 0 && heal > 0 {
            {
                let player = self.side_mut(side);
                player.life = (player.life + i32::from(heal)).min(LIFE);
            }
            self.push_log(side, "heal", 0, 0, -1, i32::from(heal));
        }
    }

    fn deal(&mut self) {
        let seed = self.state.seed;
        self.state.you = Side::default();
        self.state.foe = Side::default();
        self.state.you.deck = DECK.to_vec();
        self.state.foe.deck = DECK.to_vec();
        self.state.winner = None;
        self.state.turn = 0;
        self.state.active = 0;
        self.state.cursor = 0;
        self.state.log.clear();
        self.state.deals = self.state.deals.saturating_add(1);
        self.state.next_uid = 1;
        self.state.phase = GamePhase::Playing;
        let mut rng = Rng { state: seed };
        shuffle(&mut self.state.you.deck, &mut rng);
        shuffle(&mut self.state.foe.deck, &mut rng);
        self.state.rng = rng.state;
        for _ in 0..OPENING {
            self.draw_one(0);
            self.draw_one(1);
        }
        self.begin_turn(0);
        self.sync_flag();
    }

    fn begin_turn(&mut self, side: u8) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        self.state.active = side;
        if side == 0 {
            self.state.turn = self.state.turn.saturating_add(1);
        }
        {
            let player = self.side_mut(side);
            player.mana_max = player.mana_max.saturating_add(1).min(MANA_CAP);
            player.mana = player.mana_max;
            for card in &mut player.board {
                card.sick = false;
                card.attacked = false;
            }
        }
        self.draw_one(side);
    }

    fn draw_one(&mut self, side: u8) {
        let drawn = {
            let player = self.side_mut(side);
            if player.deck.is_empty() {
                player.fatigue = player.fatigue.saturating_add(1);
                let n = player.fatigue;
                player.life -= i32::try_from(n).unwrap_or(i32::MAX);
                Err(n)
            } else {
                Ok(player.deck.pop().unwrap_or(0))
            }
        };
        match drawn {
            Err(n) => {
                self.push_log(side, "fatigue", 0, 0, -1, i32::try_from(n).unwrap_or(0));
                self.check_end();
            }
            Ok(def_id) => {
                if self.side(side).hand.len() >= HAND_MAX {
                    self.side_mut(side).grave.push(def_id);
                    self.push_log(side, "burn", 0, def_id, -1, 0);
                } else {
                    let inst = self.make_inst(def_id);
                    let uid = inst.uid;
                    self.side_mut(side).hand.push(inst);
                    self.push_log(side, "draw", uid, def_id, -1, 0);
                }
            }
        }
    }

    fn run_ai(&mut self) {
        if self.state.phase != GamePhase::Playing || self.state.active != 1 {
            return;
        }
        for _ in 0..12 {
            if self.state.phase != GamePhase::Playing {
                return;
            }
            let Some(index) = self.best_index(1) else {
                break;
            };
            if !self.play_side(1, index) {
                break;
            }
        }
        if self.state.phase != GamePhase::Playing {
            return;
        }
        let uids: Vec<u32> = self
            .state
            .foe
            .board
            .iter()
            .filter(|card| self.ready(card))
            .map(|card| card.uid)
            .collect();
        for uid in uids {
            if self.state.phase != GamePhase::Playing {
                return;
            }
            let Some(index) = self.state.foe.board.iter().position(|card| card.uid == uid) else {
                continue;
            };
            let target = self.ai_target(index);
            let _ = self.attack_side(1, index, target);
        }
    }

    fn best_index(&self, side: u8) -> Option<usize> {
        let player = self.side(side);
        let mut best: Option<(usize, i32)> = None;
        for (index, card) in player.hand.iter().enumerate() {
            let Some(def) = catalog(card.def) else {
                continue;
            };
            if player.mana < def.cost {
                continue;
            }
            if def.kind == Kind::Unit && player.board.len() >= BOARD_MAX {
                continue;
            }
            let score = match def.kind {
                Kind::Unit => i32::from(def.cost) * 10 + i32::from(def.atk) + i32::from(def.hp),
                Kind::Spell if def.atk > 0 => i32::from(def.atk) * 12 + i32::from(def.cost),
                Kind::Spell => {
                    if player.life <= LIFE - i32::from(def.hp) {
                        i32::from(def.hp) * 4
                    } else {
                        -1
                    }
                }
            };
            if score < 0 {
                continue;
            }
            if best.is_none_or(|(_, prev)| score > prev) {
                best = Some((index, score));
            }
        }
        best.map(|(index, _)| index)
    }

    fn ai_target(&self, attacker: usize) -> i32 {
        let atk = self
            .state
            .foe
            .board
            .get(attacker)
            .map(|card| card.atk)
            .unwrap_or(0);
        let mut best: Option<(usize, i32)> = None;
        for (index, card) in self.state.you.board.iter().enumerate() {
            if card.hp == 0 || card.hp > atk {
                continue;
            }
            let score = i32::from(card.atk) * 10 + i32::from(card.hp);
            if best.is_none_or(|(_, prev)| score > prev) {
                best = Some((index, score));
            }
        }
        best.map(|(index, _)| i32::try_from(index).unwrap_or(-1))
            .unwrap_or(-1)
    }

    fn ready(&self, card: &CardInst) -> bool {
        catalog(card.def).is_some_and(|def| def.kind == Kind::Unit)
            && !card.sick
            && !card.attacked
            && card.atk > 0
    }

    fn make_inst(&mut self, def_id: u16) -> CardInst {
        let (atk, hp) = catalog(def_id)
            .map(|def| (def.atk, def.hp))
            .unwrap_or((0, 0));
        let uid = self.state.next_uid.max(1);
        self.state.next_uid = uid.saturating_add(1);
        CardInst {
            uid,
            def: def_id,
            atk,
            hp,
            sick: false,
            attacked: false,
        }
    }

    fn reap(&mut self, side: u8) {
        let dead: Vec<u16> = self
            .side(side)
            .board
            .iter()
            .filter(|card| card.hp == 0)
            .map(|card| card.def)
            .collect();
        if dead.is_empty() {
            return;
        }
        self.side_mut(side).board.retain(|card| card.hp > 0);
        self.side_mut(side).grave.extend(dead);
    }

    fn check_end(&mut self) -> bool {
        if self.state.phase != GamePhase::Playing {
            return self.state.phase == GamePhase::Complete;
        }
        let you_dead = self.state.you.life <= 0;
        let foe_dead = self.state.foe.life <= 0;
        if !you_dead && !foe_dead {
            return false;
        }
        let winner = if foe_dead && !you_dead {
            0
        } else if you_dead && !foe_dead {
            1
        } else {
            self.state.active
        };
        self.state.winner = Some(winner);
        self.state.phase = GamePhase::Complete;
        let op = if winner == 0 { "win" } else { "lose" };
        self.push_log(winner, op, 0, 0, -1, 0);
        self.sync_flag();
        true
    }

    fn clamp_cursor(&mut self) {
        let n = self.state.you.hand.len();
        if n == 0 {
            self.state.cursor = 0;
        } else if self.state.cursor as usize >= n {
            self.state.cursor = (n as u32) - 1;
        }
    }

    fn push_log(&mut self, side: u8, op: &str, uid: u32, def: u16, target: i32, n: i32) {
        self.state.log.push(Act {
            t: self.state.turn,
            side,
            op: op.to_string(),
            uid,
            def,
            target,
            n,
        });
        if self.state.log.len() > LOG_MAX {
            let drop_n = self.state.log.len() - LOG_MAX;
            self.state.log.drain(0..drop_n);
        }
        self.record_event(
            op,
            Some(serde_json::json!({
                "side": side,
                "uid": uid,
                "def": def,
                "target": target,
                "n": n,
            })),
        );
    }

    fn side(&self, side: u8) -> &Side {
        if side == 0 {
            &self.state.you
        } else {
            &self.state.foe
        }
    }

    fn side_mut(&mut self, side: u8) -> &mut Side {
        if side == 0 {
            &mut self.state.you
        } else {
            &mut self.state.foe
        }
    }

    fn validate(&self) -> Result<(), String> {
        for (label, side) in [("you", &self.state.you), ("foe", &self.state.foe)] {
            for id in side.deck.iter().chain(side.grave.iter()) {
                if catalog(*id).is_none() {
                    return Err(format!("{label} pile has unknown card {id}"));
                }
            }
            for card in side.hand.iter().chain(side.board.iter()) {
                if catalog(card.def).is_none() {
                    return Err(format!("{label} has unknown card {}", card.def));
                }
            }
        }
        Ok(())
    }

    fn bump_uid(&mut self) {
        let mut next = self.state.next_uid.max(1);
        let cards = self
            .state
            .you
            .hand
            .iter()
            .chain(self.state.you.board.iter())
            .chain(self.state.foe.hand.iter())
            .chain(self.state.foe.board.iter());
        for card in cards {
            next = next.max(card.uid.saturating_add(1));
        }
        self.state.next_uid = next;
    }

    fn sync_flag(&mut self) {
        let (enabled, color) = match (self.state.phase, self.state.winner) {
            (GamePhase::Complete, Some(0)) => (true, Some([240, 196, 72])),
            (GamePhase::Complete, Some(1)) => (true, Some([176, 64, 58])),
            _ => (false, None),
        };
        if let Some(flag) = self.doc.props.iter_mut().find(|prop| prop.name == "flag") {
            flag.enabled = enabled;
            if let Some(color) = color {
                flag.color = Some(color);
            }
        }
    }

    fn tick_timers(&mut self, dt: f32) {
        let mut done = Vec::new();
        for timer in &mut self.doc.timers {
            if !timer.active {
                continue;
            }
            timer.remaining = (timer.remaining - dt).max(0.0);
            if timer.remaining <= 1e-4 {
                timer.active = false;
                if let Some(event) = timer.on_done.clone() {
                    done.push(event);
                }
            }
        }
        for event in done {
            self.record_event(&event, None);
        }
    }
}

fn event_seq_from(doc: &WorldDoc) -> u32 {
    doc.events
        .iter()
        .filter_map(|event| {
            event
                .id
                .strip_prefix("event:")
                .and_then(|n| n.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0)
}

fn phase_name(phase: GamePhase) -> &'static str {
    match phase {
        GamePhase::Title => "title",
        GamePhase::Playing => "playing",
        GamePhase::Complete => "complete",
    }
}

fn card_json(card: &CardInst) -> serde_json::Value {
    let def = catalog(card.def);
    serde_json::json!({
        "uid": card.uid,
        "def": card.def,
        "name": def.map(|d| d.name).unwrap_or(""),
        "cost": def.map(|d| d.cost).unwrap_or(0),
        "kind": def.map(|d| d.kind.as_str()).unwrap_or("unit"),
        "atk": card.atk,
        "hp": card.hp,
        "art": def.map(|d| d.art).unwrap_or(0),
        "line": def.map(|d| d.line).unwrap_or(""),
        "sick": card.sick,
        "attacked": card.attacked,
        "ready": catalog(card.def).is_some_and(|d| d.kind == Kind::Unit)
            && !card.sick
            && !card.attacked
            && card.atk > 0,
    })
}

fn side_public(side: &Side, reveal_hand: bool) -> serde_json::Value {
    let mut value = serde_json::json!({
        "life": side.life.max(0),
        "mana": side.mana,
        "mana_max": side.mana_max,
        "deck_n": side.deck.len(),
        "grave_n": side.grave.len(),
        "hand_n": side.hand.len(),
        "board": side.board.iter().map(card_json).collect::<Vec<_>>(),
    });
    if reveal_hand {
        value["hand"] = serde_json::Value::Array(side.hand.iter().map(card_json).collect());
    }
    value
}

fn catalog_json() -> serde_json::Value {
    serde_json::Value::Array(
        CATALOG
            .iter()
            .map(|def| {
                serde_json::json!({
                    "def": def.id,
                    "name": def.name,
                    "cost": def.cost,
                    "kind": def.kind.as_str(),
                    "atk": def.atk,
                    "hp": def.hp,
                    "art": def.art,
                    "line": def.line,
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: &str = r#"{"version":1,"genre":"card"}"#;

    fn open() -> CardSession {
        CardSession::open(MIN).expect("open")
    }

    #[test]
    fn deck_is_twenty_and_covers_the_catalog() {
        assert_eq!(DECK.len(), 20);
        let mut ids: Vec<u16> = CATALOG.iter().map(|def| def.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
        for id in DECK {
            assert!(catalog(id).is_some(), "deck id {id}");
        }
        for def in &CATALOG {
            assert!(DECK.contains(&def.id), "{}", def.name);
        }
    }

    #[test]
    fn shuffle_follows_the_seed() {
        let mut a = open();
        let mut b = open();
        a.confirm_action();
        b.confirm_action();
        assert_eq!(a.state.you.deck, b.state.you.deck);
        assert_eq!(a.state.you.hand, b.state.you.hand);
        assert_eq!(a.state.foe.deck, b.state.foe.deck);
        assert_eq!(a.you_hand(), 4, "opening 3 plus the turn draw");
        assert_eq!(a.foe_hand(), 3);
        assert_eq!(a.state.you.deck.len() + a.you_hand(), 20);
        assert_eq!(a.mana(), 1);
        assert_eq!(a.turn(), 1);
        a.state.seed = 2;
        a.deal();
        assert_ne!(a.state.you.deck, b.state.you.deck);
    }

    #[test]
    fn play_spends_mana_and_a_fresh_unit_cannot_attack() {
        let mut s = open();
        s.confirm_action();
        s.state.you.hand.clear();
        s.state.you.mana = 1;
        s.state.you.mana_max = 1;
        let wolf = s.make_inst(1);
        let king = s.make_inst(10);
        s.state.you.hand.push(wolf);
        s.state.you.hand.push(king);
        assert!(!s.play(1), "the king costs 7");
        assert!(s.play(0));
        assert_eq!(s.mana(), 0);
        assert_eq!(s.you_board(), 1);
        assert!(s.state.you.board[0].sick);
        assert!(!s.attack(0, -1));
        assert!(!s.play(99));
    }

    #[test]
    fn spell_can_win_and_dump_round_trips() {
        let mut s = CardSession::open(include_str!("../tests/fixtures/card_world.json")).unwrap();
        s.confirm_action();
        s.state.foe.life = 2;
        s.state.you.hand.clear();
        s.state.you.mana = 1;
        let spark = s.make_inst(11);
        s.state.you.hand.push(spark);
        let hidden = s.state.foe.hand.clone();
        assert!(s.play(0));
        assert_eq!(s.current_phase(), GamePhase::Complete);
        assert_eq!(s.winner(), Some(0));
        assert!(s.status().contains(YOU_WIN));
        let dump = s.dump_json().expect("dump");
        let restored = CardSession::open(&dump).expect("restore");
        assert_eq!(restored.state, s.state);
        assert_eq!(restored.state.foe.hand, hidden);
        let view: serde_json::Value = serde_json::from_str(&s.view_json()).unwrap();
        assert!(view["foe"].get("hand").is_none());
        assert!(view["foe"]["hand_n"].as_u64().unwrap() >= 1);
        assert!(view["you"].get("hand").is_some());
        let flag = s.doc.props.iter().find(|prop| prop.name == "flag");
        if let Some(flag) = flag {
            assert!(flag.enabled);
        }
    }

    #[test]
    fn foe_spell_loses_and_rematch_deals_again() {
        let mut s = open();
        s.confirm_action();
        s.state.you.life = 2;
        s.state.active = 1;
        s.state.foe.mana = 1;
        let spark = s.make_inst(11);
        s.state.foe.hand = vec![spark];
        assert!(s.play_side(1, 0));
        assert_eq!(s.current_phase(), GamePhase::Complete);
        assert_eq!(s.winner(), Some(1));
        assert!(s.status().contains(YOU_LOSE));
        s.confirm_action();
        assert_eq!(s.current_phase(), GamePhase::Playing);
        assert_eq!(s.you_life(), LIFE);
        assert_eq!(s.foe_life(), LIFE);
        assert!(s.winner().is_none());
        assert_eq!(s.state.deals, 2);
    }

    #[test]
    fn stick_selects_and_attack_ends_the_turn() {
        let mut s = open();
        s.confirm_action();
        s.state.you.hand.clear();
        s.state.you.mana = 5;
        let first = s.make_inst(1);
        let second = s.make_inst(1);
        s.state.you.hand.push(first);
        s.state.you.hand.push(second);
        s.apply_input(WalkInput {
            lx: 1.0,
            lz: 0.0,
            jump: false,
            attack: false,
            dodge: false,
        });
        assert_eq!(s.state.cursor, 1);
        s.apply_input(WalkInput {
            lx: 0.0,
            lz: 0.0,
            jump: true,
            attack: false,
            dodge: false,
        });
        assert_eq!(s.you_board(), 1);
        let turn = s.turn();
        s.apply_input(WalkInput {
            lx: 0.0,
            lz: 0.0,
            jump: false,
            attack: true,
            dodge: false,
        });
        assert_eq!(s.active(), 0);
        assert!(s.turn() > turn);
    }

    #[test]
    fn a_duel_reaches_a_winner() {
        let mut s = open();
        s.confirm_action();
        for _ in 0..80 {
            if s.current_phase() == GamePhase::Complete {
                break;
            }
            for _ in 0..8 {
                let mut played = false;
                let n = s.you_hand();
                for index in (0..n).rev() {
                    if s.play(index) {
                        played = true;
                        break;
                    }
                }
                if !played {
                    break;
                }
            }
            if s.current_phase() == GamePhase::Complete {
                break;
            }
            let n = s.you_board();
            for index in 0..n {
                let _ = s.attack(index, -1);
            }
            if s.current_phase() == GamePhase::Complete {
                break;
            }
            assert!(
                s.end_turn(),
                "turn {} life {} {}",
                s.turn(),
                s.you_life(),
                s.foe_life()
            );
        }
        assert_eq!(
            s.current_phase(),
            GamePhase::Complete,
            "turn {} you {} foe {}",
            s.turn(),
            s.you_life(),
            s.foe_life()
        );
        assert!(s.winner().is_some());
        let dump = s.dump_json().unwrap();
        let restored = CardSession::open(&dump).unwrap();
        assert_eq!(restored.state.winner, s.state.winner);
        assert_eq!(restored.state.you.life, s.state.you.life);
        assert_eq!(restored.state.foe.deck, s.state.foe.deck);
    }

    #[test]
    fn wrong_genre_is_rejected() {
        let err = CardSession::open(r#"{"version":1,"genre":"puzzle"}"#).unwrap_err();
        assert!(err.contains("card"));
    }

    #[test]
    fn fixture_picture_uses_bundled_meshes() {
        let session = CardSession::open(include_str!("../tests/fixtures/card_world.json")).unwrap();
        assert_eq!(session.current_phase(), GamePhase::Title);
        let dump = session.dump_json().unwrap();
        assert!(dump.contains("crate.glb"));
        assert!(dump.contains("knight"));
        assert!(dump.contains("chest"));
        assert!(dump.contains("\"genre\": \"card\""));
    }

    #[test]
    fn japanese_copy_rasterizes_without_missing_glyphs() {
        use crate::font::TextRaster;
        use crate::scene::TextQuad;

        let mut session = open();
        session.confirm_action();
        let mut blob = String::new();
        for line in static_copy() {
            blob.push_str(line);
        }
        blob.push_str(&session.status());
        blob.push_str(session.hint());
        let mut raster = TextRaster::new();
        for ch in blob.chars() {
            if ch.is_whitespace() {
                continue;
            }
            let quads = raster.text_quads(&TextQuad::new(
                &ch.to_string(),
                0.0,
                0.0,
                16.0,
                [255, 255, 255, 255],
            ));
            assert!(
                !quads.is_empty(),
                "glyph {ch} (U+{:04X}) did not rasterize",
                ch as u32
            );
        }
    }
}
