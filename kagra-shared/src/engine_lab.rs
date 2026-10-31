//! Engine Lab: a playground for the engine's shared parts, three tabs on one screen.
//!
//! - **Physics** (`game_core::physics2d`): a tower of crates and planks; tap the sky to drop
//!   a ball, tap a crate to blast it, RESET to build it again.
//! - **Moves** (`game_core::anim`): a KayKit knight with real clips (walk, run, attack,
//!   cheer, die) next to a Kenney mini character doing the same moves as a procedural pose.
//!   The stick walks them; the attack's blow lands on its cue.
//! - **Talk & shop** (`game_core::{dialogue, items}`, `hud_kit::Menu`): talk to the
//!   merchant, buy from the shop, equip from the bag, drink a potion; the stats add up.
//!
//! The items, the shop and the conversation are JSON in the world (`game.items`,
//! `game.shop`, `game.talk`). It never ends: the page's pause and title still work.

use std::cell::Cell;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::flow::{FlowPhase, GameFlow};
use crate::game_core::anim::{Animator, MoveSet};
use crate::game_core::dialogue::{Beat as TalkBeat, Script, Talk};
use crate::game_core::floats::Floats;
use crate::game_core::items::{Bag, Gear, ItemBook, Kind, Refusal, Shop};
use crate::game_core::physics2d::{Body, BodyId, Shape, World};
use crate::game_core::view::View;
use crate::game_core::{declare_models, stick, tick_frame, Core, Kit};
use crate::hop_beat::{glb_prop, glb_ready, light, prop};
use crate::hud_kit::{hit, pill_button, round_panel, Menu, Row};
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, TextAlign, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc, WorldWalker};

/// The `genre` of this kit's world file (and the `game.id` default).
pub const GAME_ID: &str = "engine_lab";

/// True for a world file whose `genre` is this kit's.
pub fn is_engine_lab_dump(json: &str) -> bool {
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
pub struct EngineLabSpec {
    pub id: String,
    pub title: String,
    /// The item kinds (`game_core::items::ItemDef` list).
    pub items: Value,
    /// The merchant's shop.
    pub shop: Shop,
    /// The merchant's conversation (`game_core::dialogue::Script`).
    pub talk: Value,
    /// Coins to start with.
    pub coins: u32,
    /// Bag slots.
    pub bag_slots: usize,
    /// The hero's hit points before gear.
    pub hp: i32,
    /// Walk speed in the moves tab (m/s).
    pub speed: f32,
    pub stick_px: f32,
    pub stick_dead: f32,
    pub audio: Option<Value>,
}

impl Default for EngineLabSpec {
    fn default() -> Self {
        Self {
            id: GAME_ID.into(),
            title: "Engine Lab".into(),
            items: default_items(),
            shop: serde_json::from_value(json!({
                "name": "Travelling Merchant",
                "name_ja": "旅の商人",
                "wares": [
                    {"id": "potion"},
                    {"id": "ether"},
                    {"id": "short_sword"},
                    {"id": "war_axe", "left": 1},
                    {"id": "leather"},
                    {"id": "chain_mail", "left": 1},
                    {"id": "moon_charm", "left": 1}
                ]
            }))
            .unwrap_or_default(),
            talk: default_talk(),
            coins: 150,
            bag_slots: 12,
            hp: 30,
            speed: 3.0,
            stick_px: 70.0,
            stick_dead: 0.12,
            audio: Some(default_audio()),
        }
    }
}

fn default_items() -> Value {
    json!([
        {"id": "potion", "name": "Potion", "name_ja": "回復薬", "icon": "薬", "color": [255, 120, 140],
         "price": 12, "stack": 9, "heal": 15, "desc": "Heals 15 HP.", "desc_ja": "HPを15回復する。"},
        {"id": "ether", "name": "Moon Water", "name_ja": "月の水", "icon": "水", "color": [140, 200, 255],
         "price": 30, "stack": 9, "heal": 40, "desc": "Heals 40 HP.", "desc_ja": "HPを40回復する。"},
        {"id": "short_sword", "name": "Short Sword", "name_ja": "ショートソード", "icon": "剣", "color": [220, 230, 255],
         "kind": "equipment", "slot": "hand", "price": 40, "stats": {"atk": 3}},
        {"id": "war_axe", "name": "War Axe", "name_ja": "バトルアックス", "icon": "斧", "color": [255, 200, 140],
         "kind": "equipment", "slot": "hand", "price": 90, "stats": {"atk": 7, "def": -1}},
        {"id": "leather", "name": "Leather Coat", "name_ja": "革のコート", "icon": "服", "color": [210, 160, 110],
         "kind": "equipment", "slot": "body", "price": 35, "stats": {"def": 2}},
        {"id": "chain_mail", "name": "Chain Mail", "name_ja": "くさりかたびら", "icon": "鎧", "color": [200, 210, 225],
         "kind": "equipment", "slot": "body", "price": 80, "stats": {"def": 5}},
        {"id": "moon_charm", "name": "Moon Charm", "name_ja": "月のお守り", "icon": "月", "color": [255, 230, 140],
         "kind": "equipment", "slot": "charm", "price": 60, "stats": {"hp": 10, "def": 1}},
        {"id": "old_key", "name": "Old Key", "name_ja": "古い鍵", "icon": "鍵", "color": [230, 200, 120],
         "kind": "key", "desc": "From the merchant.", "desc_ja": "商人にもらった。"}
    ])
}

fn default_talk() -> Value {
    json!({
        "start": "hello",
        "cast": {
            "merchant": {"name": "Mira", "name_ja": "ミラ", "skin": [245, 210, 180], "hair": [120, 60, 150],
                          "eyes": [60, 40, 90], "color": [190, 120, 255], "side": "right"},
            "hero": {"name": "You", "name_ja": "あなた", "skin": [240, 200, 165], "hair": [60, 45, 35],
                     "color": [120, 180, 255]}
        },
        "nodes": {
            "hello": {
                "lines": [
                    {"who": "merchant", "text": "Welcome, traveller! Moonlight prices, tonight only.", "text_ja": "いらっしゃい、旅の人！今夜だけの月明かり価格だよ。", "face": "smile"},
                    {"who": "merchant", "text": "Back again? I kept the good stuff for you.", "text_ja": "また来たね？いい品をとっておいたよ。", "if": "visits>=2"}
                ],
                "set": {"visits": 1},
                "choices": [
                    {"text": "Show me your wares", "text_ja": "品物を見せて", "goto": "shop"},
                    {"text": "Any news?", "text_ja": "なにか噂は？", "goto": "news"},
                    {"text": "Goodbye", "text_ja": "さようなら", "goto": "bye"}
                ]
            },
            "shop": {"lines": [{"who": "merchant", "text": "Take your time.", "text_ja": "ゆっくり見ていってね。"}],
                     "events": ["open_shop"]},
            "news": {
                "lines": [
                    {"who": "merchant", "text": "The old crypt opened at midnight. They say the dead walk there.", "text_ja": "真夜中に古い地下墓地が開いたの。亡者が歩いているって。", "face": "sad"},
                    {"who": "hero", "text": "I'll take a look.", "text_ja": "見に行ってみるよ。"},
                    {"who": "merchant", "text": "Then take this key. Don't die on me!", "text_ja": "じゃあこの鍵を持っていって。死なないでよ！", "face": "surprised", "if": "!has_key"}
                ],
                "next": "key"
            },
            "key": {"lines": [], "events": ["give_key"], "set": {"has_key": 1}, "next": "hello"},
            "bye": {"lines": [{"who": "merchant", "text": "Come back soon!", "text_ja": "また来てね！", "face": "smile"}]}
        }
    })
}

impl EngineLabSpec {
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let spec: Self =
            serde_json::from_value(v.clone()).map_err(|e| format!("engine_lab spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), String> {
        let book = ItemBook::from_value(&self.items)?;
        for w in &self.shop.wares {
            if book.get(&w.id).is_none() {
                return Err(format!(
                    "engine_lab: the shop sells {:?}, not an item",
                    w.id
                ));
            }
        }
        Script::from_value(&self.talk)?;
        if !(1..=40).contains(&self.bag_slots) {
            return Err("engine_lab: bag_slots must be 1..=40".into());
        }
        if !(0.5..=10.0).contains(&self.speed) {
            return Err("engine_lab: speed must be 0.5..=10".into());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- look ----

const KNIGHT_GLB: &str = "assets/library/kaykit/adventurers/characters/Knight.glb";
const MERCHANT_GLB: &str = "assets/library/kaykit/adventurers/characters/Mage.glb";
const MINI_GLB: &str = "assets/models/kenney/mini-characters/character-male-a.glb";
const CRATE_GLB: &str = "assets/models/kenney/platformer-kit/crate.glb";
/// KayKit adventurers stand about 2.4 m tall; this makes them about 1.7.
const KAYKIT_SCALE: f32 = 0.7;

fn default_audio() -> Value {
    let sfx = "assets/audio/oga-retro-synth-sfx/";
    let cue =
        |f: &str, v: f64, r: f64| json!({"file": format!("{sfx}{f}"), "volume": v, "rate": r});
    let ui = |v: f64, r: f64| json!({"file": "assets/audio/kenney-interface-sounds/confirmation_001.wav", "volume": v, "rate": r});
    let thud = |v: f64, r: f64| json!({"file": "assets/audio/kenney-impact-sounds/impactGeneric_light_001.wav", "volume": v, "rate": r});
    json!({
        "enabled": true,
        "master": 0.8,
        "sfx_volume": 1.0,
        "bgm_volume": 0.5,
        "sfx": {
            "go": ui(0.6, 1.0),
            "tab": ui(0.4, 1.3),
            "thud": thud(0.5, 0.9),
            "knock": thud(0.6, 1.3),
            "swing": cue("shot_01.wav", 0.3, 0.7),
            "hit": thud(0.7, 0.8),
            "buy": cue("retro_coin_01.wav", 0.5, 1.2),
            "no": cue("retro_die_01.wav", 0.4, 1.6),
            "equip": ui(0.5, 0.8),
            "heal": cue("power_up_02.wav", 0.5, 1.4),
            "cheer": cue("power_up_04.wav", 0.5, 1.0),
            "down": cue("retro_die_03.wav", 0.5, 1.0)
        }
    })
}

// ----------------------------------------------------------------- run ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Physics,
    Moves,
    Talk,
}

/// What the talk tab shows over the merchant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Panel {
    None,
    Shop,
    Bag,
}

/// A button on the HUD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Btn {
    Tab(Tab),
    Reset,
    Attack,
    Cheer,
    Die,
    Speak,
    OpenBag,
    Close,
}

/// How the physics tab's tower is built.
const FLOOR_Y: f32 = 0.0;

#[derive(Debug)]
pub struct EngineLab {
    spec: EngineLabSpec,
    core: Core,
    flow: GameFlow,
    doc: WorldDoc,
    tab: Tab,
    t: f32,
    lang_ja: bool,
    size: Cell<(f32, f32)>,
    floats: Floats,
    // Physics.
    world: World,
    /// Body ids and how each is drawn: 0 crate, 1 plank, 2 ball, 3 fixed.
    look: Vec<(BodyId, u8)>,
    thud_cool: f32,
    // Moves.
    knight: Animator,
    mini: Animator,
    hero: (f32, f32),
    yaw: f32,
    speed_now: f32,
    keys: (f32, f32),
    finger_id: Option<u32>,
    finger: Option<(f32, f32)>,
    base: (f32, f32),
    // Talk and shop.
    book: ItemBook,
    talk: Talk,
    bag: Bag,
    gear: Gear,
    shop: Shop,
    panel: Panel,
    pick: Option<usize>,
    scroll: usize,
    hp: i32,
    note: String,
    note_t: f32,
}

impl EngineLab {
    pub fn new(spec: EngineLabSpec) -> Self {
        let mut flow = GameFlow::new(spec.id.clone());
        flow.title = spec.title.clone();
        flow.tagline = String::new();
        let book = ItemBook::from_value(&spec.items).unwrap_or_default();
        let script = Script::from_value(&spec.talk).unwrap_or_default();
        let mut bag = Bag::new(spec.bag_slots);
        bag.coins = spec.coins;
        let shop = spec.shop.clone();
        let hp = spec.hp;
        let mut g = Self {
            spec,
            core: Core::default(),
            flow,
            doc: WorldDoc::default(),
            tab: Tab::Physics,
            t: 0.0,
            lang_ja: false,
            size: Cell::new((390.0, 844.0)),
            floats: Floats::default(),
            world: World::new(),
            look: Vec::new(),
            thud_cool: 0.0,
            knight: Animator::new(MoveSet::kaykit()),
            mini: Animator::new(MoveSet::procedural()),
            hero: (0.0, 0.0),
            yaw: 0.0,
            speed_now: 0.0,
            keys: (0.0, 0.0),
            finger_id: None,
            finger: None,
            base: (0.0, 0.0),
            book,
            talk: Talk::new(script),
            bag,
            gear: Gear::default(),
            shop,
            panel: Panel::None,
            pick: None,
            scroll: 0,
            hp,
            note: String::new(),
            note_t: 0.0,
        };
        g.build_tower();
        g.build_doc();
        g
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| format!("engine_lab dump: {e}"))?;
        let spec = match value.get("game") {
            Some(g) => EngineLabSpec::from_value(g)?,
            None => EngineLabSpec::default(),
        };
        Ok(Self::new(spec))
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

    pub fn confirm(&mut self) {
        match self.flow.phase {
            FlowPhase::Title | FlowPhase::Result => {
                self.core = Core::default();
                self.flow.enter_play();
                crate::mixer::cue_sfx("go");
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
            self.finger = None;
            self.finger_id = None;
        }
    }

    fn say(&mut self, text: String) {
        self.note = text;
        self.note_t = 2.0;
    }

    fn set_tab(&mut self, tab: Tab) {
        if tab != self.tab {
            self.tab = tab;
            self.floats.clear();
            self.panel = Panel::None;
            crate::mixer::cue_sfx("tab");
        }
    }

    // ---- physics

    /// The floor, two walls, and a tower: three rows of crates with planks between.
    fn build_tower(&mut self) {
        let mut w = World::new();
        let mut look = Vec::new();
        let mut add = |w: &mut World, b: Body, kind: u8| look.push((w.add(b), kind));
        add(
            &mut w,
            Body::fixed(Shape::rect(12.0, 0.5), Vec2::new(0.0, FLOOR_Y - 0.5)),
            3,
        );
        for x in [-5.0, 5.0] {
            add(
                &mut w,
                // Tall enough that nothing is thrown over (a crate on a wall top never falls).
                Body::fixed(Shape::rect(0.3, 15.0), Vec2::new(x, FLOOR_Y + 15.0)),
                3,
            );
        }
        let crate_ = |x: f32, y: f32| Body::dynamic(Shape::rect(0.45, 0.45), Vec2::new(x, y), 1.0);
        let mut y = FLOOR_Y + 0.45;
        for row in 0..3 {
            let n = 3 - row;
            for i in 0..n {
                let x = (i as f32 - (n - 1) as f32 * 0.5) * 1.15;
                add(&mut w, crate_(x, y), 0);
                add(&mut w, crate_(x, y + 0.9), 0);
            }
            y += 1.8;
            let plank_w = n as f32 * 0.62 + 0.3;
            add(
                &mut w,
                Body::dynamic(Shape::rect(plank_w, 0.12), Vec2::new(0.0, y - 0.33), 0.8),
                1,
            );
            y += 0.12;
        }
        self.world = w;
        self.look = look;
    }

    /// The physics tab's camera, side on.
    fn side_view(&self) -> View {
        let (w, h) = self.size.get();
        View::frame(
            [0.0, 3.4, 0.0],
            [0.0, 0.18, 1.0],
            45.0,
            &[
                [-5.3, -0.4, 0.0],
                [5.3, -0.4, 0.0],
                [-5.3, 7.5, 0.0],
                [5.3, 7.5, 0.0],
            ],
            w / h.max(1.0),
        )
    }

    /// A tap in the physics tab: blast a body, or drop a ball from above.
    fn physics_tap(&mut self, x: f32, y: f32) {
        let (w, h) = self.size.get();
        let view = self.side_view();
        // The point on the z = 0 plane under the finger: walk the ray to z = 0.
        let Some(p) = plane_z0(&view, x, y, w, h) else {
            return;
        };
        if let Some(id) = self.world.body_at(p) {
            if self.world.get(id).is_some_and(|b| !b.is_fixed()) {
                self.world.blast(p - Vec2::new(0.0, 0.3), 2.2, 7.0);
                crate::mixer::cue_sfx("knock");
                self.floats
                    .push([p.x, p.y + 0.6, 0.0], "BOOM", [255, 200, 120, 255], 0.0);
                return;
            }
        }
        let ball =
            Body::dynamic(Shape::ball(0.5), Vec2::new(p.x, p.y.max(7.0)), 8.0).with_bounce(0.3);
        let id = self.world.add(ball);
        if let Some(b) = self.world.get_mut(id) {
            b.vel = Vec2::new(0.0, -12.0);
        }
        self.look.push((id, 2));
    }

    // ---- moves

    fn press_move(&mut self, b: Btn) {
        match b {
            Btn::Attack => {
                let pick = if self.t.fract() < 0.5 {
                    "attack"
                } else {
                    "attack2"
                };
                if self.knight.act(pick) {
                    self.mini.act("attack");
                    crate::mixer::cue_sfx("swing");
                }
            }
            Btn::Cheer => {
                if self.knight.act("cheer") {
                    self.mini.act("cheer");
                    crate::mixer::cue_sfx("cheer");
                }
            }
            Btn::Die => {
                if self.knight.holding() {
                    self.knight.revive();
                    self.mini.revive();
                } else {
                    self.knight.act("die");
                    self.mini.act("die");
                    crate::mixer::cue_sfx("down");
                }
            }
            _ => {}
        }
    }

    fn knight_pos(&self) -> [f32; 3] {
        [self.hero.0 - 0.9, 0.0, self.hero.1]
    }

    fn mini_pos(&self) -> [f32; 3] {
        [self.hero.0 + 0.9, 0.0, self.hero.1]
    }

    // ---- talk and shop

    fn speak(&mut self) {
        if !self.talk.active() {
            self.panel = Panel::None;
            self.talk.begin();
            crate::mixer::cue_sfx("tab");
        }
    }

    fn run_talk_events(&mut self) {
        for ev in self.talk.drain() {
            match ev.as_str() {
                "open_shop" => {
                    self.panel = Panel::Shop;
                    self.pick = None;
                    self.scroll = 0;
                }
                "give_key" if self.bag.add(&self.book, "old_key", 1).is_ok() => {
                    let name = self
                        .book
                        .get("old_key")
                        .map(|d| d.name(self.lang_ja).to_string());
                    self.say(format!("+ {}", name.unwrap_or_default()));
                }
                _ => {}
            }
        }
    }

    fn max_hp(&self) -> i32 {
        self.spec.hp + self.gear.stat(&self.book, "hp")
    }

    fn refusal(&self, r: Refusal) -> String {
        match r {
            Refusal::NotEnoughCoins => self.tr("Not enough coins", "お金が足りない"),
            Refusal::BagFull => self.tr("The bag is full", "持ち物がいっぱい"),
            Refusal::SoldOut => self.tr("Sold out", "売り切れ"),
            Refusal::CannotSell => self.tr("Can't sell that", "売れない"),
            _ => self.tr("Can't", "できない"),
        }
        .to_string()
    }

    fn shop_rows(&self) -> Vec<Row> {
        let ja = self.lang_ja;
        (0..self.shop.wares.len())
            .filter_map(|i| {
                let w = &self.shop.wares[i];
                let d = self.book.get(&w.id)?;
                let price = self.shop.price(&self.book, i)?;
                let sold_out = w.left == Some(0);
                let mut label = d.name(ja).to_string();
                if d.kind == Kind::Equipment {
                    let mut bits = Vec::new();
                    for st in ["atk", "def", "hp"] {
                        let dv = self.gear.delta(&self.book, &d.id, st);
                        if dv != 0 {
                            bits.push(format!("{}{dv:+}", st.to_uppercase()));
                        }
                    }
                    if !bits.is_empty() {
                        label = format!("{label}  {}", bits.join(" "));
                    }
                }
                Some(Row {
                    icon: d.icon.clone(),
                    icon_color: [d.color[0], d.color[1], d.color[2], 255],
                    label,
                    right: if sold_out {
                        self.tr("SOLD", "売切").into()
                    } else {
                        format!("{price} G")
                    },
                    dim: sold_out || price > self.bag.coins,
                })
            })
            .collect()
    }

    fn bag_rows(&self) -> Vec<Row> {
        let ja = self.lang_ja;
        let mut rows: Vec<Row> = self
            .gear
            .worn
            .values()
            .filter_map(|id| self.book.get(id))
            .map(|d| Row {
                icon: d.icon.clone(),
                icon_color: [d.color[0], d.color[1], d.color[2], 255],
                label: d.name(ja).to_string(),
                right: self.tr("WORN", "装備中").into(),
                dim: false,
            })
            .collect();
        rows.extend(self.bag.stacks.iter().filter_map(|s| {
            let d = self.book.get(&s.id)?;
            Some(Row {
                icon: d.icon.clone(),
                icon_color: [d.color[0], d.color[1], d.color[2], 255],
                label: d.name(ja).to_string(),
                right: format!("x{}", s.count),
                dim: d.kind == Kind::Key,
            })
        }));
        rows
    }

    /// A tap on row `i` of the open panel.
    fn use_row(&mut self, i: usize) {
        match self.panel {
            Panel::Shop => match self.shop.buy(&self.book, i, &mut self.bag) {
                Ok(()) => {
                    crate::mixer::cue_sfx("buy");
                    let name = self
                        .book
                        .get(&self.shop.wares[i].id)
                        .map(|d| d.name(self.lang_ja).to_string());
                    self.say(format!("+ {}", name.unwrap_or_default()));
                }
                Err(r) => {
                    crate::mixer::cue_sfx("no");
                    let msg = self.refusal(r);
                    self.say(msg);
                }
            },
            Panel::Bag => {
                let worn: Vec<(String, String)> = self
                    .gear
                    .worn
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if let Some((slot, _)) = worn.get(i) {
                    if self.gear.unequip(&self.book, &mut self.bag, slot).is_ok() {
                        crate::mixer::cue_sfx("equip");
                    }
                } else if let Some(st) = self.bag.stacks.get(i - worn.len()).cloned() {
                    let kind = self.book.get(&st.id).map(|d| d.kind);
                    match kind {
                        Some(Kind::Equipment) => {
                            if self.gear.equip(&self.book, &mut self.bag, &st.id).is_ok() {
                                crate::mixer::cue_sfx("equip");
                            }
                        }
                        Some(Kind::Consumable) => {
                            let heal = self.bag.use_one(&self.book, &st.id).map(|d| d.heal);
                            if let Ok(heal) = heal {
                                self.hp = (self.hp + heal).min(self.max_hp());
                                crate::mixer::cue_sfx("heal");
                                self.say(format!("HP +{heal}"));
                            }
                        }
                        _ => crate::mixer::cue_sfx("no"),
                    }
                }
                self.hp = self.hp.min(self.max_hp());
            }
            Panel::None => {}
        }
    }

    fn menu(&self, w: f32, h: f32) -> Menu {
        let s = hud_scale(w, h);
        Menu::layout(w, 108.0 * s, h * 0.72, 440.0, s)
    }

    // ---- sim

    fn step_sim(&mut self, dt: f32) {
        self.t += dt;
        self.floats.tick(dt);
        self.note_t = (self.note_t - dt).max(0.0);
        match self.tab {
            Tab::Physics => {
                self.world.step(dt);
                self.thud_cool = (self.thud_cool - dt).max(0.0);
                if let Some(i) = self.world.impacts.first() {
                    if i.impulse > 4.0 && self.thud_cool == 0.0 {
                        crate::mixer::cue_sfx("thud");
                        self.thud_cool = 0.12;
                    }
                }
                // Balls that roll out of the picture go.
                let gone: Vec<BodyId> = self
                    .look
                    .iter()
                    .filter(|(id, _)| self.world.get(*id).is_some_and(|b| b.pos.y < -5.0))
                    .map(|(id, _)| *id)
                    .collect();
                for id in gone {
                    self.world.remove(id);
                    self.look.retain(|(i, _)| *i != id);
                }
            }
            Tab::Moves => {
                let (w, h) = self.size.get();
                let dir = match self.finger {
                    Some(f) => stick::analog(
                        self.base,
                        f,
                        self.spec.stick_px * hud_scale(w, h),
                        self.spec.stick_dead,
                    ),
                    None => self.keys,
                };
                let frozen = self.knight.holding() || self.knight.busy();
                let (dx, dz) = if frozen { (0.0, 0.0) } else { dir };
                let push = dx.hypot(dz).min(1.0);
                // Push past 0.75 runs.
                let speed = if push > 0.75 {
                    self.spec.speed * 1.8
                } else {
                    self.spec.speed * push
                };
                if push > 0.05 {
                    self.yaw = dx.atan2(dz);
                    let l = dx.hypot(dz).max(1e-4);
                    self.hero.0 = (self.hero.0 + dx / l * speed * dt).clamp(-4.0, 4.0);
                    self.hero.1 = (self.hero.1 + dz / l * speed * dt).clamp(-2.5, 2.5);
                }
                self.speed_now = if push > 0.05 { speed } else { 0.0 };
                self.knight.locomote(self.speed_now, true, 0.0);
                self.mini.locomote(self.speed_now, true, 0.0);
                if self.knight.tick(dt).is_some() {
                    let p = self.knight_pos();
                    let (fx, fz) = (self.yaw.sin(), self.yaw.cos());
                    let at = [p[0] + fx * 1.1, 1.2, p[2] + fz * 1.1];
                    self.floats.push(at, "HIT!", [255, 220, 120, 255], 0.0);
                    self.core
                        .juice
                        .burst(at[0], at[1], at[2], 14, [255, 220, 150], 3.0, 0.08);
                    crate::mixer::cue_sfx("hit");
                }
                self.mini.tick(dt);
            }
            Tab::Talk => {
                self.talk.tick(dt);
                self.run_talk_events();
                // The merchant idles; the hero shows the gear.
                self.knight.locomote(0.0, true, 0.0);
                self.knight.tick(dt);
            }
        }
    }

    // ---- input

    fn hud_buttons(&self, w: f32, h: f32) -> Vec<(Btn, [f32; 4])> {
        let s = hud_scale(w, h);
        let mut out = Vec::new();
        let tw = ((w - 32.0 * s) / 3.0).min(140.0 * s);
        let x0 = (w - 3.0 * tw - 16.0 * s) * 0.5;
        for (i, t) in [Tab::Physics, Tab::Moves, Tab::Talk]
            .into_iter()
            .enumerate()
        {
            out.push((
                Btn::Tab(t),
                [x0 + i as f32 * (tw + 8.0 * s), 12.0 * s, tw, 38.0 * s],
            ));
        }
        let (bw, bh) = (110.0 * s, 46.0 * s);
        let right = w - bw - 14.0 * s;
        let bottom = h - bh - 18.0 * s;
        match self.tab {
            Tab::Physics => out.push((Btn::Reset, [right, bottom, bw, bh])),
            Tab::Moves => {
                out.push((Btn::Attack, [right, bottom, bw, bh]));
                out.push((Btn::Cheer, [right, bottom - bh - 10.0 * s, bw, bh]));
                out.push((Btn::Die, [right, bottom - 2.0 * (bh + 10.0 * s), bw, bh]));
            }
            Tab::Talk => {
                if self.talk.active() {
                    return out;
                }
                if self.panel == Panel::None {
                    out.push((Btn::Speak, [right, bottom, bw, bh]));
                    out.push((Btn::OpenBag, [14.0 * s, bottom, bw, bh]));
                } else {
                    let m = self.menu(w, h);
                    out.push((Btn::Close, [m.x + m.w - bw, m.y + m.h + 10.0 * s, bw, bh]));
                }
            }
        }
        out
    }

    fn press(&mut self, b: Btn) {
        match b {
            Btn::Tab(t) => self.set_tab(t),
            Btn::Reset => {
                self.build_tower();
                crate::mixer::cue_sfx("tab");
            }
            Btn::Attack | Btn::Cheer | Btn::Die => self.press_move(b),
            Btn::Speak => self.speak(),
            Btn::OpenBag => {
                self.panel = Panel::Bag;
                self.pick = None;
                self.scroll = 0;
                crate::mixer::cue_sfx("tab");
            }
            Btn::Close => {
                self.panel = Panel::None;
                crate::mixer::cue_sfx("tab");
            }
        }
    }

    pub fn touch_ev(&mut self, ev: PointerEvent, w: u32, h: u32) {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        self.size.set((w, h));
        if self.flow.phase != FlowPhase::Play {
            return;
        }
        let s = hud_scale(w, h);
        match ev.phase {
            PointerPhase::Begin => {
                // A talk takes every tap.
                if self.tab == Tab::Talk && self.talk.active() {
                    if self.talk.beat == TalkBeat::Choosing {
                        if let Some(i) = self.talk.choice_at(ev.x, ev.y, w, h, s) {
                            self.talk.choose(i);
                        }
                    } else {
                        self.talk.tap();
                    }
                    self.run_talk_events();
                    return;
                }
                if let Some(b) = hit(&self.hud_buttons(w, h), ev.x, ev.y) {
                    self.press(b);
                    return;
                }
                match self.tab {
                    Tab::Physics => self.physics_tap(ev.x, ev.y),
                    Tab::Moves if self.finger_id.is_none() => {
                        let r = self.spec.stick_px * s;
                        self.finger_id = Some(ev.id);
                        self.finger = Some((ev.x, ev.y));
                        self.base = stick::land(ev.x, ev.y, r, 4.0 * s, w, h);
                    }
                    Tab::Talk if self.panel != Panel::None => {
                        let rows = match self.panel {
                            Panel::Shop => self.shop_rows().len(),
                            _ => self.bag_rows().len(),
                        };
                        if let Some(i) = self.menu(w, h).row_at(ev.x, ev.y, rows, self.scroll) {
                            self.pick = Some(i);
                            self.use_row(i);
                        }
                    }
                    _ => {}
                }
            }
            PointerPhase::Move if self.finger_id == Some(ev.id) => {
                let r = self.spec.stick_px * s;
                self.finger = Some((ev.x, ev.y));
                self.base = stick::follow(self.base, (ev.x, ev.y), r);
            }
            PointerPhase::End | PointerPhase::Cancel if self.finger_id == Some(ev.id) => {
                self.finger_id = None;
                self.finger = None;
            }
            _ => {}
        }
    }

    // ---- picture

    fn build_doc(&mut self) {
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: 14.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([6, 8, 20, 255]),
            hemi_sky: Some([0.22, 0.26, 0.46]),
            hemi_ground: Some([0.1, 0.09, 0.15]),
            env_map: Some("preset:studio".into()),
            ibl: Some(0.25),
            exposure: Some(0.9),
            bloom: Some(0.45),
            bloom_threshold: Some(0.9),
            vignette: Some(0.5),
            fxaa: Some(true),
            tonemap: Some(true),
            ..WorldDoc::default()
        };
        let view = match self.tab {
            Tab::Physics => {
                self.physics_doc(&mut doc);
                self.side_view()
            }
            Tab::Moves | Tab::Talk => {
                self.stage_doc(&mut doc);
                self.stage_view()
            }
        };
        self.walkers_doc(&mut doc);
        declare_models(&mut doc, &[KNIGHT_GLB, MERCHANT_GLB, MINI_GLB, CRATE_GLB]);
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "lab".into(),
            position: view.eye,
            target: view.target,
            fov: view.fov,
            roll: 0.0,
        });
        doc.lights.push(light(
            0,
            "key",
            [2.0, 9.0, 6.0],
            [0.0; 3],
            [1.0, 0.95, 0.88],
            1.3,
            40.0,
            false,
        ));
        doc.lights.push(light(
            1,
            "rim",
            [-5.0, 4.0, -4.0],
            [0.0; 3],
            [0.55, 0.65, 1.0],
            0.7,
            30.0,
            false,
        ));
        self.doc = doc;
    }

    fn physics_doc(&self, doc: &mut WorldDoc) {
        // The engine's sky shades toward a dusk gradient; the lab hides it behind a wall
        // and a ground that fill the view.
        doc.props.push(prop(
            "backdrop",
            "box",
            "backdrop",
            [0.0, 5.0, -1.2],
            [80.0, 60.0, 0.2],
            [34, 40, 72],
            0.0,
            0.95,
            0.0,
        ));
        doc.props.push(prop(
            "ground",
            "box",
            "ground",
            [0.0, -2.0, 0.0],
            [80.0, 2.0, 30.0],
            [26, 30, 52],
            0.0,
            0.95,
            0.0,
        ));
        for (n, (id, kind)) in self.look.iter().enumerate() {
            let Some(b) = self.world.get(*id) else {
                continue;
            };
            let pos = [b.pos.x, b.pos.y, 0.0];
            let mut p = match (b.shape, kind) {
                (Shape::Rect { hw, hh }, 0) if glb_ready(CRATE_GLB) => {
                    // Kenney's crate is a 0.5 m cube standing on its origin: scale it to the
                    // body and drop the origin to the body's bottom (rolled with it).
                    let mut p = glb_prop(
                        &format!("body-{n}"),
                        CRATE_GLB,
                        "crate",
                        pos,
                        [4.0 * hw, 4.0 * hh, 4.0 * hw],
                        [255, 255, 255],
                        0.0,
                        None,
                    );
                    let (c, s) = (b.angle.cos(), b.angle.sin());
                    p.position = [pos[0] + s * hh, pos[1] - c * hh, 0.0];
                    p
                }
                (Shape::Rect { hw, hh }, k) => prop(
                    &format!("body-{n}"),
                    "box",
                    "body",
                    pos,
                    [2.0 * hw, 2.0 * hh, if *k == 3 { 2.0 } else { 0.9 }],
                    match k {
                        1 => [190, 140, 90],
                        3 => [70, 78, 120],
                        _ => [200, 160, 100],
                    },
                    0.0,
                    0.8,
                    0.0,
                ),
                (Shape::Ball { r }, _) => prop(
                    &format!("body-{n}"),
                    "sphere",
                    "ball",
                    pos,
                    [2.0 * r; 3],
                    [235, 90, 90],
                    0.15,
                    0.4,
                    0.2,
                ),
            };
            p.roll = b.angle;
            doc.props.push(p);
        }
    }

    fn stage_doc(&self, doc: &mut WorldDoc) {
        doc.props.push(prop(
            "floor",
            "box",
            "floor",
            [0.0, -0.1, 0.0],
            [11.0, 0.2, 7.0],
            [58, 66, 100],
            0.0,
            0.9,
            0.0,
        ));
        doc.props.push(prop(
            "ground",
            "box",
            "ground",
            [0.0, -0.3, -10.0],
            [80.0, 0.2, 60.0],
            [40, 46, 82],
            0.0,
            0.95,
            0.0,
        ));
        doc.props.push(prop(
            "backdrop",
            "box",
            "backdrop",
            [0.0, 10.0, -7.0],
            [80.0, 30.0, 0.4],
            [30, 36, 70],
            0.0,
            0.95,
            0.0,
        ));
        match self.tab {
            Tab::Moves => {
                let mut mini = glb_prop(
                    "mini",
                    MINI_GLB,
                    "mini",
                    [0.0; 3],
                    [1.6; 3],
                    [255, 255, 255],
                    0.0,
                    None,
                );
                self.mini
                    .apply_prop(&mut mini, self.mini_pos(), self.yaw, 1.6);
                doc.props.push(mini);
            }
            Tab::Talk => {
                // The stall.
                doc.props.push(prop(
                    "stall",
                    "box",
                    "stall",
                    [1.6, 0.45, 0.6],
                    [2.2, 0.9, 0.7],
                    [150, 100, 70],
                    0.0,
                    0.8,
                    0.0,
                ));
            }
            Tab::Physics => {}
        }
        for (i, s) in self.core.juice.sparks.iter().enumerate() {
            doc.props.push(prop(
                &format!("spark-{i}"),
                "sphere",
                "spark",
                [s.x, s.y, s.z],
                [0.08; 3],
                [255, 220, 150],
                2.0,
                0.5,
                0.0,
            ));
        }
    }

    /// The two rigged characters, in every tab: off-stage ones are named `wait` (not drawn),
    /// so the set of skinned models the GPU holds never changes with the tab.
    fn walkers_doc(&self, doc: &mut WorldDoc) {
        let mut knight = WorldWalker {
            id: "walker:knight".into(),
            name: "knight".into(),
            position: self.knight_pos(),
            yaw: self.yaw,
            gltf: Some(KNIGHT_GLB.into()),
            scale: Some([KAYKIT_SCALE; 3]),
            ..WorldWalker::default()
        };
        match self.tab {
            Tab::Physics => knight.name = "wait".into(),
            Tab::Talk => {
                knight.position = [-1.6, 0.0, 0.6];
                knight.yaw = 0.9;
            }
            Tab::Moves => {}
        }
        self.knight.apply_walker(&mut knight);
        let merchant = WorldWalker {
            id: "walker:merchant".into(),
            name: if self.tab == Tab::Talk {
                "merchant".into()
            } else {
                "wait".into()
            },
            position: [1.4, 0.0, -0.4],
            yaw: -0.6,
            gltf: Some(MERCHANT_GLB.into()),
            scale: Some([KAYKIT_SCALE; 3]),
            clip_name: Some("Idle".into()),
            clip: self.t.max(1e-3),
            anim_blend: 1.0,
            ..WorldWalker::default()
        };
        for (w, path) in [(knight, KNIGHT_GLB), (merchant, MERCHANT_GLB)] {
            if glb_ready(path) {
                doc.walkers.push(w);
            }
        }
    }

    fn stage_view(&self) -> View {
        let (w, h) = self.size.get();
        View::frame(
            [0.0, 0.9, 0.0],
            [0.0, 0.55, 1.0],
            42.0,
            &View::ground_box(-4.2, 4.2, -2.6, 2.6, 0.0),
            w / h.max(1.0),
        )
    }

    pub fn hud(&self, w: u32, h: u32, _menu: bool) -> DrawList {
        let (w, h) = (w as f32, h as f32);
        self.size.set((w.max(1.0), h.max(1.0)));
        let s = hud_scale(w, h);
        let mut list = DrawList::default();
        if !matches!(self.flow.phase, FlowPhase::Play | FlowPhase::Pause) {
            return list;
        }
        let view = match self.tab {
            Tab::Physics => self.side_view(),
            _ => self.stage_view(),
        };
        self.floats.draw(&mut list, &view, w, h, s);
        let help = match self.tab {
            Tab::Physics => self.tr(
                "Tap the sky: a ball. Tap a crate: boom.",
                "空をタップでボール、箱をタップで爆発",
            ),
            Tab::Moves => self.tr(
                "Drag to walk, push far to run.",
                "ドラッグで歩く、大きく倒すと走る",
            ),
            Tab::Talk => self.tr("Talk to Mira, shop, equip.", "ミラと話して、買い物と装備"),
        };
        list.texts.push(
            TextQuad::new(help, w * 0.5, 60.0 * s, 15.0 * s, [210, 220, 255, 230])
                .aligned(TextAlign::Center),
        );
        if self.tab == Tab::Talk {
            // Stats.
            let atk = 4 + self.gear.stat(&self.book, "atk");
            let def = 2 + self.gear.stat(&self.book, "def");
            let line = format!(
                "HP {}/{}   ATK {atk}   DEF {def}   {} G",
                self.hp,
                self.max_hp(),
                self.bag.coins
            );
            list.texts.push(
                TextQuad::new(&line, w * 0.5, 84.0 * s, 16.0 * s, [255, 236, 200, 255])
                    .aligned(TextAlign::Center),
            );
            match self.panel {
                Panel::Shop => {
                    let m = self.menu(w, h);
                    let title = if self.lang_ja {
                        &self.shop.name_ja
                    } else {
                        &self.shop.name
                    };
                    m.draw(
                        &mut list,
                        title,
                        &format!("{} G", self.bag.coins),
                        &self.shop_rows(),
                        self.scroll,
                        self.pick,
                    );
                }
                Panel::Bag => {
                    let m = self.menu(w, h);
                    let rows = self.bag_rows();
                    m.draw(
                        &mut list,
                        self.tr("Bag (tap: use / equip)", "持ち物（タップで使う・装備）"),
                        &format!("{} G", self.bag.coins),
                        &rows,
                        self.scroll,
                        self.pick,
                    );
                }
                Panel::None => {}
            }
            self.talk.draw(&mut list, w, h, s, self.lang_ja);
        }
        if self.tab == Tab::Moves {
            if let (Some(f), Some(_)) = (self.finger, self.finger_id) {
                let r = self.spec.stick_px * s;
                let mut ring = crate::scene::Quad::new(
                    self.base.0 - r,
                    self.base.1 - r,
                    2.0 * r,
                    2.0 * r,
                    [255, 255, 255, 70],
                );
                ring.radius = r;
                ring.stroke = 3.0 * s;
                list.quads.push(ring);
                let k = 26.0 * s;
                let mut knob = crate::scene::Quad::new(
                    f.0 - k,
                    f.1 - k,
                    2.0 * k,
                    2.0 * k,
                    [255, 255, 255, 150],
                );
                knob.radius = k;
                list.quads.push(knob);
            }
            list.texts.push(TextQuad::new(
                &format!("{}: {}", self.tr("Knight", "騎士"), self.knight.current()),
                14.0 * s,
                h - 60.0 * s,
                15.0 * s,
                [200, 220, 255, 230],
            ));
        }
        for (b, r) in self.hud_buttons(w, h) {
            let (label, fill) = match b {
                Btn::Tab(t) => {
                    let on = t == self.tab;
                    let label = match t {
                        Tab::Physics => self.tr("Physics", "物理"),
                        Tab::Moves => self.tr("Moves", "動き"),
                        Tab::Talk => self.tr("Talk & Shop", "会話・お店"),
                    };
                    (
                        label,
                        if on {
                            [80, 120, 220, 240]
                        } else {
                            [30, 38, 70, 220]
                        },
                    )
                }
                Btn::Reset => (self.tr("RESET", "リセット"), [60, 70, 110, 230]),
                Btn::Attack => (self.tr("ATTACK", "攻撃"), [150, 60, 60, 235]),
                Btn::Cheer => (self.tr("CHEER", "喜ぶ"), [60, 120, 80, 235]),
                Btn::Die => {
                    if self.knight.holding() {
                        (self.tr("GET UP", "起きる"), [70, 70, 110, 235])
                    } else {
                        (self.tr("FALL", "やられる"), [70, 70, 110, 235])
                    }
                }
                Btn::Speak => (self.tr("TALK", "話す"), [120, 80, 170, 235]),
                Btn::OpenBag => (self.tr("BAG", "持ち物"), [60, 90, 130, 235]),
                Btn::Close => (self.tr("CLOSE", "閉じる"), [60, 70, 110, 235]),
            };
            pill_button(
                &mut list,
                r[0],
                r[1],
                r[2],
                r[3],
                label,
                15.0 * s,
                fill,
                [255, 255, 255, 255],
            );
        }
        if self.note_t > 0.0 {
            let a = (self.note_t.min(0.5) / 0.5 * 255.0) as u8;
            let nw = 220.0 * s;
            round_panel(
                &mut list,
                (w - nw) * 0.5,
                h * 0.76,
                nw,
                34.0 * s,
                17.0 * s,
                [20, 24, 50, a.min(220)],
                [255, 255, 255, a / 4],
                1.0 * s,
                (0.0, 0.0, 0),
            );
            list.texts.push(
                TextQuad::new(
                    &self.note,
                    w * 0.5,
                    h * 0.76 + 8.0 * s,
                    16.0 * s,
                    [255, 240, 210, a],
                )
                .aligned(TextAlign::Center),
            );
        }
        list
    }

    pub fn state_value(&self) -> Value {
        json!({
            "phase": self.flow.phase.name(),
            "tab": format!("{:?}", self.tab).to_lowercase(),
            "bodies": self.world.bodies().count(),
            "awake": self.world.awake(),
            "knight": self.knight.current(),
            "talk": self.talk.node(),
            "coins": self.bag.coins,
            "hp": self.hp,
            "score": 0,
            "best": 0,
            "win": false,
            "death": "",
            "death_text": "",
        })
    }
}

/// The point on the z = 0 plane under a canvas pixel (the physics tab's play plane).
fn plane_z0(view: &View, px: f32, py: f32, w: f32, h: f32) -> Option<Vec2> {
    // The ray from the eye through the pixel (as `View::ground_at` builds it), to z = 0.
    let (eye, target) = (glam::Vec3::from(view.eye), glam::Vec3::from(view.target));
    let aspect = w / h.max(1.0);
    let f = (target - eye).normalize();
    let r = f.cross(glam::Vec3::Y).normalize();
    let u = r.cross(f);
    let ty = (view.fov.to_radians() * 0.5).tan();
    let nx = (2.0 * px / w.max(1.0) - 1.0) * ty * aspect;
    let ny = (1.0 - 2.0 * py / h.max(1.0)) * ty;
    let d = f + r * nx + u * ny;
    if d.z.abs() < 1e-5 {
        return None;
    }
    let t = -eye.z / d.z;
    (t > 0.0).then(|| {
        let p = eye + d * t;
        Vec2::new(p.x, p.y)
    })
}

fn hud_scale(w: f32, h: f32) -> f32 {
    (w.min(h) / 400.0).clamp(0.9, 2.6)
}

// ------------------------------------------------------------ the host ----

impl Kit for EngineLab {
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
        self.t += dt;
        self.world.step(dt);
    }
    fn rebuild(&mut self) {
        self.build_doc();
    }
}

impl crate::kit_host::KitHost for EngineLab {
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
        EngineLab::confirm(self)
    }
    fn show_title(&mut self) {
        EngineLab::show_title(self)
    }
    fn pointer(&mut self, _x: f32, _y: f32, _width: u32, _height: u32) -> bool {
        false
    }
    fn on_key(&mut self, name: &str, down: bool) -> bool {
        if !down || self.flow.phase != FlowPhase::Play {
            return false;
        }
        match name {
            "1" => self.set_tab(Tab::Physics),
            "2" => self.set_tab(Tab::Moves),
            "3" => self.set_tab(Tab::Talk),
            "j" | "J" if self.tab == Tab::Moves => self.press_move(Btn::Attack),
            "Enter" | " " if self.tab == Tab::Talk => {
                if self.talk.active() {
                    self.talk.tap();
                    self.run_talk_events();
                } else {
                    self.speak();
                }
            }
            _ => return false,
        }
        true
    }
    fn tick(&mut self, dt: f32) {
        tick_frame(self, dt);
    }
    fn hud(&self, width: u32, height: u32, menu: bool) -> DrawList {
        EngineLab::hud(self, width, height, menu)
    }
    fn walk(&mut self, lx: f32, lz: f32, _jump: bool) {
        // Keys and the gamepad: +lz is up, which is world -z here.
        self.keys = (lx.clamp(-1.0, 1.0), (-lz).clamp(-1.0, 1.0));
        if self.tab == Tab::Talk && self.talk.beat == TalkBeat::Choosing && lz.abs() > 0.5 {
            self.talk.point(if lz > 0.0 { -1 } else { 1 });
        }
    }
    fn buttons(&mut self, attack: bool, _dodge: bool) {
        if attack && self.tab == Tab::Moves && !self.knight.busy() {
            self.press_move(Btn::Attack);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit_host::KitHost;

    const DT: f32 = 1.0 / 60.0;
    const W: u32 = 390;
    const H: u32 = 844;

    fn started() -> EngineLab {
        let mut g = EngineLab::new(EngineLabSpec::default());
        let _ = g.hud(W, H, false);
        g.confirm();
        g
    }

    fn tap(g: &mut EngineLab, x: f32, y: f32) {
        for phase in [PointerPhase::Begin, PointerPhase::End] {
            g.touch(
                PointerEvent {
                    id: 1,
                    x,
                    y,
                    phase,
                    pressure: 1.0,
                },
                W,
                H,
                0.0,
            );
        }
    }

    fn press(g: &mut EngineLab, b: Btn) {
        let r = g
            .hud_buttons(W as f32, H as f32)
            .into_iter()
            .find(|(x, _)| *x == b)
            .map(|(_, r)| r)
            .unwrap_or_else(|| panic!("no {b:?} button"));
        tap(g, r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
    }

    fn run(g: &mut EngineLab, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            g.tick(DT);
        }
    }

    #[test]
    fn the_default_spec_is_valid_and_strict() {
        let spec = EngineLabSpec::default();
        assert!(spec.validate().is_ok());
        let mut bad = spec.world_value_for_test();
        bad["shop"]["wares"][0]["id"] = json!("dragon");
        assert!(EngineLabSpec::from_value(&bad).is_err());
        let mut bad = spec.world_value_for_test();
        bad["oops"] = json!(1);
        assert!(EngineLabSpec::from_value(&bad).is_err());
    }

    impl EngineLabSpec {
        fn world_value_for_test(&self) -> Value {
            serde_json::to_value(self).unwrap()
        }
    }

    #[test]
    fn the_tower_stands_until_a_ball_falls_on_it() {
        let mut g = started();
        run(&mut g, 3.0);
        assert_eq!(g.world.awake(), 0, "the tower settles and sleeps");
        let tops: f32 = g
            .look
            .iter()
            .filter_map(|(id, _)| g.world.get(*id))
            .filter(|b| !b.is_fixed())
            .map(|b| b.pos.y)
            .fold(0.0, f32::max);
        assert!(tops > 5.0, "still tall: {tops}");
        // Drop a ball onto the top through the screen: it lands with a thud.
        let view = g.side_view();
        let (sx, sy) = view.project([0.0, 7.5, 0.0], W as f32, H as f32).unwrap();
        tap(&mut g, sx, sy);
        assert!(g.look.iter().any(|(_, k)| *k == 2), "a ball appeared");
        let mut thud = 0.0_f32;
        for _ in 0..60 {
            g.tick(DT);
            thud = thud.max(g.world.impacts.first().map_or(0.0, |i| i.impulse));
        }
        assert!(thud > 4.0, "it hit the tower: {thud}");
        // Tap crates low in the tower: each blows, and a few bring it down.
        for _ in 0..3 {
            let low = g
                .look
                .iter()
                .filter(|(_, k)| *k == 0)
                .filter_map(|(id, _)| g.world.get(*id))
                .filter(|b| b.pos.x.abs() < 2.0)
                .min_by(|a, b| (a.pos.y + a.pos.x.abs()).total_cmp(&(b.pos.y + b.pos.x.abs())))
                .map(|b| b.pos)
                .unwrap();
            let (sx, sy) = view
                .project([low.x, low.y, 0.0], W as f32, H as f32)
                .unwrap();
            tap(&mut g, sx, sy);
            run(&mut g, 0.4);
        }
        run(&mut g, 4.0);
        let top_now: f32 = g
            .look
            .iter()
            .filter(|(_, k)| *k != 2)
            .filter_map(|(id, _)| g.world.get(*id))
            // Inside the walls (a blast can throw a crate over one).
            .filter(|b| !b.is_fixed() && b.pos.x.abs() < 4.7)
            .map(|b| b.pos.y)
            .fold(0.0, f32::max);
        assert!(
            top_now < tops - 0.5,
            "the tower came down: {top_now} < {tops}"
        );
        press(&mut g, Btn::Reset);
        assert!(
            !g.look.iter().any(|(_, k)| *k == 2),
            "reset clears the balls"
        );
    }

    #[test]
    fn the_knight_walks_swings_on_cue_and_falls() {
        let mut g = started();
        press(&mut g, Btn::Tab(Tab::Moves));
        assert_eq!(g.tab, Tab::Moves);
        g.walk(1.0, 0.0, false);
        run(&mut g, 0.5);
        assert!(g.hero.0 > 0.5);
        assert_eq!(g.knight.current(), "run");
        g.walk(0.0, 0.0, false);
        run(&mut g, 0.1);
        assert_eq!(g.knight.current(), "idle");
        press(&mut g, Btn::Attack);
        assert!(g.knight.current().starts_with("attack"));
        run(&mut g, 0.6);
        assert!(
            g.floats.items.iter().any(|f| f.text == "HIT!"),
            "the blow landed on its cue"
        );
        if glb_ready(KNIGHT_GLB) {
            let w = &g.doc.walkers[0];
            assert!(w.clip_name.as_deref().unwrap().contains("Attack"));
        }
        run(&mut g, 1.0);
        press(&mut g, Btn::Die);
        run(&mut g, 1.5);
        assert!(g.knight.holding());
        g.walk(1.0, 0.0, false);
        let x = g.hero.0;
        run(&mut g, 0.3);
        assert_eq!(g.hero.0, x, "the fallen do not walk");
        press(&mut g, Btn::Die);
        run(&mut g, 0.1);
        assert_eq!(g.knight.current(), "run", "up and running again");
    }

    #[test]
    fn talk_shop_buy_equip_and_drink() {
        let mut g = started();
        press(&mut g, Btn::Tab(Tab::Talk));
        press(&mut g, Btn::Speak);
        assert!(g.talk.active());
        while g.talk.beat != TalkBeat::Choosing {
            tap(&mut g, 195.0, 300.0);
        }
        // "Any news?" gives the key and comes back.
        let r = g
            .talk
            .choice_rects(W as f32, H as f32, hud_scale(W as f32, H as f32))[1];
        tap(&mut g, r[0] + 10.0, r[1] + 10.0);
        while g.talk.beat != TalkBeat::Choosing {
            tap(&mut g, 195.0, 300.0);
        }
        assert!(g.bag.has("old_key"));
        assert_eq!(g.talk.node(), "hello");
        // "Show me your wares" opens the shop.
        let r = g
            .talk
            .choice_rects(W as f32, H as f32, hud_scale(W as f32, H as f32))[0];
        tap(&mut g, r[0] + 10.0, r[1] + 10.0);
        while g.talk.active() {
            tap(&mut g, 195.0, 300.0);
        }
        assert_eq!(g.panel, Panel::Shop);
        let m = g.menu(W as f32, H as f32);
        let row_y = |i: usize| m.y + 44.0 * m.s + (i as f32 + 0.5) * m.row_h;
        tap(&mut g, m.x + 40.0, row_y(2)); // short sword, 40 G
        tap(&mut g, m.x + 40.0, row_y(0)); // potion, 12 G
        assert_eq!(g.bag.coins, 150 - 40 - 12);
        assert!(g.bag.has("short_sword") && g.bag.has("potion"));
        g.bag.coins = 20;
        tap(&mut g, m.x + 40.0, row_y(3)); // axe 90 G: too dear
        assert!(!g.bag.has("war_axe"));
        assert_eq!(g.bag.coins, 20);
        assert_eq!(g.note, "Not enough coins");
        press(&mut g, Btn::Close);
        press(&mut g, Btn::OpenBag);
        // The bag: key, sword, potion (in the order they came).
        let rows = g.bag_rows();
        let sword = rows.iter().position(|r| r.label == "Short Sword").unwrap();
        tap(&mut g, m.x + 40.0, row_y(sword));
        assert_eq!(g.gear.stat(&g.book, "atk"), 3);
        assert_eq!(g.bag_rows()[0].right, "WORN");
        g.hp = 10;
        let potion = g
            .bag_rows()
            .iter()
            .position(|r| r.label == "Potion")
            .unwrap();
        tap(&mut g, m.x + 40.0, row_y(potion));
        assert_eq!(g.hp, 25);
        assert!(!g.bag.has("potion"));
        let hud = g.hud(W, H, false);
        assert!(hud.texts.iter().any(|t| t.text.contains("ATK 7")));
    }

    #[test]
    fn every_tab_draws_and_the_model_set_stays_fixed() {
        let mut g = started();
        let key = |g: &EngineLab| g.doc.gpu_mesh_key();
        let first = key(&g);
        for t in [Tab::Moves, Tab::Talk, Tab::Physics] {
            press(&mut g, Btn::Tab(t));
            run(&mut g, 0.3);
            assert_eq!(key(&g), first, "{t:?}");
            let hud = g.hud(W, H, false);
            assert!(!hud.texts.is_empty());
            let land = g.hud(H, W, false);
            assert!(!land.texts.is_empty());
        }
    }

    #[test]
    fn the_baked_fixture_is_the_default_world() {
        let want = EngineLab::new(EngineLabSpec::default()).world_value();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/engine_lab_world.json"
        );
        if std::env::var("KAGRA_BAKE").is_ok() {
            std::fs::write(path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
        }
        let got: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default())
            .unwrap_or(Value::Null);
        assert_eq!(
            got, want,
            "run `KAGRA_BAKE=1 cargo test -p kagra-shared --lib engine_lab::` to refresh it"
        );
        assert!(is_engine_lab_dump(&got.to_string()));
        let loaded = crate::kit_host::load(&got.to_string()).unwrap().unwrap();
        assert_eq!(loaded.name(), GAME_ID, "the session finds this kit");
    }
}
