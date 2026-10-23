//! Fish, cook, and sell as one day. `fish_cast` / `cook_stove` / `shop_buy`
//! stay one-shot slices on `WorldPlay`. This session does not join that
//! dispatcher.
//!
//! Cast waits with [`fish::tick_cast`]. The bite is a tension reel
//! ([`fish::tick_reel`]): hold while the marker sits in the band. A raw
//! fish then has to reach the stove. Heat rises on its own ([`cook::tick_heat`]);
//! tap inside the band. The dish sells at the stall for [`shop::SALE_PRICE`].
//! Repeat until the coin goal or the day clock. HUD copy is ASCII.

use crate::collectathon::WalkInput;
use crate::cook;
use crate::fish;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};
use crate::shop;
use crate::world_doc::{WorldDoc, WorldEvent, WorldTimer, WorldWalker};

pub const GAME_ID: &str = "fishcook";

const WALK: f32 = 4.8;
const GOAL_DEFAULT: u32 = 8;
const DAY_DEFAULT: f32 = 90.0;

const PAN_IDLE: [u32; 3] = [72, 76, 82];
const PAN_COOK: [u32; 3] = [240, 140, 48];
const PAN_ZONE: [u32; 3] = [255, 196, 72];
const PAN_DONE: [u32; 3] = [70, 180, 110];
const BOBBER_CAST: [u32; 3] = [240, 196, 72];
const BOBBER_BITE: [u32; 3] = [255, 140, 64];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Beat {
    Idle,
    Cast,
    Reel,
    Carry,
    Cook,
    Haul,
}

/// Dock, stove, and stall for one day. Picture is a `WorldDoc`
/// (`genre = fishcook`). Rules stay here; Python sends sticks and taps.
#[derive(Clone, Debug)]
pub struct FishCookSession {
    doc: WorldDoc,
    input: WalkInput,
    prev_attack: bool,
    tap_latch: bool,
    phase: GamePhase,
    beat: Beat,
    cast: fish::Cast,
    reel: fish::Reel,
    heat: cook::Heat,
    coins: u32,
    goal: u32,
    sold: u32,
    day_left: f32,
    day_total: f32,
    won: bool,
    face: f32,
    spawn: [f32; 3],
    spawn_yaw: f32,
    event_seq: u32,
}

impl FishCookSession {
    pub fn open(json: &str) -> Result<Self, String> {
        let doc = WorldDoc::from_json(json)?;
        if doc.genre.as_deref() != Some(GAME_ID) {
            return Err(format!(
                "WorldDoc.genre must be {GAME_ID:?}, got {:?}",
                doc.genre
            ));
        }
        let (spawn, spawn_yaw) = doc
            .player
            .as_ref()
            .map(|p| (p.position, p.yaw))
            .unwrap_or(([0.0, fish::BODY_H, 4.0], std::f32::consts::PI));
        let mut session = Self {
            doc,
            input: WalkInput::default(),
            prev_attack: false,
            tap_latch: false,
            phase: GamePhase::Title,
            beat: Beat::Idle,
            cast: fish::Cast::default(),
            reel: fish::Reel::default(),
            heat: cook::Heat::default(),
            coins: 0,
            goal: GOAL_DEFAULT,
            sold: 0,
            day_left: DAY_DEFAULT,
            day_total: DAY_DEFAULT,
            won: false,
            face: spawn_yaw,
            spawn,
            spawn_yaw,
            event_seq: 0,
        };
        session.goal = goal_from(&session.doc);
        session.day_total = day_from(&session.doc);
        session.day_left = session.day_total;
        session.decorate();
        Ok(session)
    }

    pub fn apply_input(&mut self, input: WalkInput) {
        let input = input.clamped();
        self.input = WalkInput {
            lx: axis(input.lx),
            lz: axis(input.lz),
            jump: input.jump,
            attack: input.attack,
            dodge: input.dodge,
        };
    }

    pub fn step(&mut self, dt: f32) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.tick_timers(dt);
        if self.phase != GamePhase::Playing {
            self.prev_attack = self.input.attack;
            self.tap_latch = false;
            self.decorate();
            return;
        }
        let edge = (self.input.attack && !self.prev_attack) || self.tap_latch;
        self.tap_latch = false;
        if !self.locked() {
            self.walk(dt);
        }
        self.advance(edge, dt);
        if self.phase == GamePhase::Playing {
            self.day_left = (self.day_left - dt).max(0.0);
            if self.day_left <= 1.0e-3 {
                self.finish();
            }
        }
        self.prev_attack = self.input.attack;
        self.decorate();
    }

    pub fn world_json(&self) -> Result<String, String> {
        self.doc.to_json()
    }

    pub fn drain_events(&mut self, name: &str) -> Vec<WorldEvent> {
        let mut out = Vec::new();
        self.doc.events.retain(|e| {
            if e.name == name {
                out.push(e.clone());
                false
            } else {
                true
            }
        });
        out
    }

    pub fn confirm_action(&mut self) {
        if self.phase != GamePhase::Playing {
            self.phase = GamePhase::Playing;
            self.reset_round();
            self.decorate();
        }
    }

    pub fn record_event(&mut self, name: &str, data: Option<serde_json::Value>) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(ev) = self.doc.events.iter_mut().find(|e| e.name == name) {
            ev.count += 1;
            if data.is_some() {
                ev.data = data;
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
        if let Some(t) = self.doc.timers.iter_mut().find(|t| t.id == id) {
            t.name = name.into();
            t.seconds = seconds;
            t.remaining = seconds;
            t.on_done = on_done.map(str::to_string);
            t.active = true;
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

    /// Stations are beats, not `interact` props. The host still has the hook.
    pub fn use_nearby(&mut self) {}

    pub fn current_phase(&self) -> GamePhase {
        self.phase
    }

    pub fn beat(&self) -> &'static str {
        self.label()
    }

    pub fn coins(&self) -> u32 {
        self.coins
    }

    pub fn goal(&self) -> u32 {
        self.goal
    }

    pub fn sold(&self) -> u32 {
        self.sold
    }

    pub fn day_left(&self) -> f32 {
        self.day_left
    }

    pub fn reel_marker(&self) -> f32 {
        fish::reel_marker(self.reel.t)
    }

    pub fn reel_fill(&self) -> f32 {
        self.reel.fill
    }

    pub fn reel_lo(&self) -> f32 {
        fish::REEL_ZONE_LO
    }

    pub fn reel_hi(&self) -> f32 {
        fish::REEL_ZONE_HI
    }

    pub fn heat(&self) -> f32 {
        self.heat.value
    }

    pub fn heat_lo(&self) -> f32 {
        cook::HEAT_LO
    }

    pub fn heat_hi(&self) -> f32 {
        cook::HEAT_HI
    }

    pub fn won(&self) -> bool {
        self.won
    }

    pub fn done(&self) -> bool {
        self.phase == GamePhase::Complete
    }

    pub fn on_dock(&self) -> bool {
        fish::player_on_dock(&self.doc)
    }

    pub fn at_stove(&self) -> bool {
        cook::player_at_stove(&self.doc)
    }

    pub fn at_stall(&self) -> bool {
        shop::player_at_stall(&self.doc)
    }

    /// Bottom-right HOLD pad. `x, y, w, h` in screen pixels, origin top-left.
    pub fn hold_pad(width: u32, height: u32) -> (f32, f32, f32, f32) {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        (w - 108.0, h - 64.0, 96.0, 52.0)
    }

    /// Title and result: any tap confirms. Play: the HOLD pad is one attack edge.
    /// A sustained reel hold is `set_input(..., attack: true)` while the finger is down.
    pub fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) {
        match self.phase {
            GamePhase::Title | GamePhase::Complete => self.confirm_action(),
            GamePhase::Playing => {
                let (px, py, pw, ph) = Self::hold_pad(width, height);
                if x >= px && x <= px + pw && y >= py && y <= py + ph {
                    self.tap_latch = true;
                }
            }
        }
    }

    pub fn hud_json(&self, width: u32, height: u32) -> String {
        let hud = self.build_hud(width, height);
        let quads: Vec<_> = hud
            .quads
            .iter()
            .map(|q| {
                serde_json::json!({
                    "x": q.x,
                    "y": q.y,
                    "w": q.w,
                    "h": q.h,
                    "color": q.color,
                })
            })
            .collect();
        let texts: Vec<_> = hud
            .texts
            .iter()
            .map(|t| {
                let align = match t.align {
                    TextAlign::Left => "left",
                    TextAlign::Center => "center",
                    TextAlign::Right => "right",
                };
                serde_json::json!({
                    "text": t.text,
                    "x": t.x,
                    "y": t.y,
                    "size": t.size,
                    "color": t.color,
                    "align": align,
                })
            })
            .collect();
        serde_json::to_string(&serde_json::json!({ "quads": quads, "texts": texts }))
            .unwrap_or_else(|_| "{}".into())
    }

    fn build_hud(&self, width: u32, height: u32) -> DrawList {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
        let mut quads = Vec::new();
        let mut texts = Vec::new();
        let ink = [236, 242, 236, 255];
        match self.phase {
            GamePhase::Title => {
                quads.push(Quad::new(0.0, 0.0, w, h, [12, 28, 36, 150]));
                quads.push(Quad::new(
                    w * 0.18,
                    h * 0.30,
                    w * 0.64,
                    h * 0.22,
                    [18, 36, 44, 230],
                ));
                texts.push(line(
                    "FISH AND COOK",
                    w * 0.5,
                    h * 0.34,
                    28.0 * scale,
                    [240, 220, 140, 255],
                    TextAlign::Center,
                ));
                texts.push(line(
                    "space or tap",
                    w * 0.5,
                    h * 0.44,
                    16.0 * scale,
                    ink,
                    TextAlign::Center,
                ));
            }
            GamePhase::Playing => {
                quads.push(Quad::new(
                    12.0,
                    10.0,
                    280.0 * scale,
                    44.0 * scale,
                    [16, 24, 28, 200],
                ));
                texts.push(line(
                    &format!(
                        "coins {}/{}  day {:.0}s",
                        self.coins,
                        self.goal,
                        self.day_left.max(0.0)
                    ),
                    22.0,
                    14.0,
                    15.0 * scale,
                    ink,
                    TextAlign::Left,
                ));
                texts.push(line(
                    self.hint(),
                    22.0,
                    32.0,
                    13.0 * scale,
                    [210, 220, 210, 255],
                    TextAlign::Left,
                ));
                if self.beat == Beat::Reel || self.beat == Beat::Cook {
                    let bar = if self.beat == Beat::Reel {
                        MeterBar {
                            frac: self.reel.fill,
                            marker: self.reel_marker(),
                            lo: fish::REEL_ZONE_LO,
                            hi: fish::REEL_ZONE_HI,
                            fill: [240, 196, 72, 255],
                        }
                    } else {
                        MeterBar {
                            frac: self.heat.value,
                            marker: self.heat.value,
                            lo: cook::HEAT_LO,
                            hi: cook::HEAT_HI,
                            fill: [240, 140, 48, 255],
                        }
                    };
                    meter(&mut quads, w, h, scale, bar);
                }
            }
            GamePhase::Complete => {
                quads.push(Quad::new(0.0, h * 0.22, w, h * 0.34, [12, 28, 22, 210]));
                let verdict = if self.won { "goal met" } else { "short" };
                texts.push(line(
                    "DAY END",
                    w * 0.5,
                    h * 0.28,
                    26.0 * scale,
                    [240, 220, 140, 255],
                    TextAlign::Center,
                ));
                texts.push(line(
                    verdict,
                    w * 0.5,
                    h * 0.38,
                    18.0 * scale,
                    ink,
                    TextAlign::Center,
                ));
                texts.push(line(
                    &format!("coins {}/{}", self.coins, self.goal),
                    w * 0.5,
                    h * 0.46,
                    16.0 * scale,
                    ink,
                    TextAlign::Center,
                ));
            }
        }
        DrawList {
            clear: [18, 32, 42, 255],
            quads,
            texts,
            ..Default::default()
        }
    }

    fn hint(&self) -> &'static str {
        match self.beat {
            Beat::Idle => "DOCK  tap to cast",
            Beat::Cast => "CAST  wait for a bite",
            Beat::Reel => "REEL  hold in the bar",
            Beat::Carry => "FISH  walk to the stove",
            Beat::Cook => "COOK  tap in the bar",
            Beat::Haul => "MEAL  walk to the stall and tap",
        }
    }

    fn label(&self) -> &'static str {
        if self.phase == GamePhase::Complete {
            return "result";
        }
        match self.beat {
            Beat::Idle => "player",
            Beat::Cast => "cast",
            Beat::Reel => "reel",
            Beat::Carry => "catch",
            Beat::Cook => "cook",
            Beat::Haul => "cooked",
        }
    }

    fn locked(&self) -> bool {
        matches!(self.beat, Beat::Cast | Beat::Reel | Beat::Cook)
    }

    fn walk(&mut self, dt: f32) {
        let Some(p) = self.doc.player.as_ref() else {
            return;
        };
        let lx = self.input.lx;
        let lz = self.input.lz;
        let half = if self.doc.half > 1.0 {
            self.doc.half
        } else {
            16.0
        };
        let x = (p.position[0] + lx * WALK * dt).clamp(-half, half);
        let z = (p.position[2] + lz * WALK * dt).clamp(-half, half);
        if lx.abs() + lz.abs() > 0.08 {
            self.face = lx.atan2(lz);
        }
        let y = self.doc.height_at(x, z) + fish::BODY_H;
        self.write_walker(self.label(), [x, y, z]);
    }

    fn advance(&mut self, edge: bool, dt: f32) {
        match self.beat {
            Beat::Idle => {
                if edge && fish::player_on_dock(&self.doc) {
                    self.beat = Beat::Cast;
                    self.cast.phase = fish::CastPhase::Waiting;
                    self.cast.wait = 0.0;
                }
            }
            Beat::Cast => {
                fish::tick_cast(&mut self.cast, false, true, dt);
                if self.cast.phase == fish::CastPhase::Bite {
                    self.beat = Beat::Reel;
                    self.reel = fish::Reel::default();
                }
            }
            Beat::Reel => {
                fish::tick_reel(&mut self.reel, self.input.attack, dt);
                if self.reel.landed {
                    self.beat = Beat::Carry;
                    self.record_event("catch", None);
                } else if self.reel.snapped {
                    self.beat = Beat::Idle;
                    self.reel = fish::Reel::default();
                    self.cast = fish::Cast::default();
                    self.record_event("snap", None);
                }
            }
            Beat::Carry => {
                if cook::player_at_stove(&self.doc) {
                    self.beat = Beat::Cook;
                    self.heat = cook::Heat::default();
                }
            }
            Beat::Cook => {
                cook::tick_heat(&mut self.heat, edge, dt);
                if self.heat.cooked {
                    self.beat = Beat::Haul;
                    self.record_event("cooked", None);
                } else if self.heat.burnt {
                    self.beat = Beat::Idle;
                    self.heat = cook::Heat::default();
                    self.record_event("burnt", None);
                } else if self.heat.early {
                    self.heat = cook::Heat::default();
                }
            }
            Beat::Haul => {
                if edge && shop::player_at_stall(&self.doc) {
                    self.coins = shop::credit_sale(self.coins);
                    self.sold += 1;
                    let coins = self.coins;
                    self.record_event("sold", Some(serde_json::json!({ "coins": coins })));
                    self.beat = Beat::Idle;
                    self.heat = cook::Heat::default();
                    if self.coins >= self.goal {
                        self.finish();
                    }
                }
            }
        }
    }

    fn finish(&mut self) {
        if self.phase == GamePhase::Complete {
            return;
        }
        self.won = self.coins >= self.goal;
        self.phase = GamePhase::Complete;
        let coins = self.coins;
        let goal = self.goal;
        let won = self.won;
        self.record_event(
            "day_end",
            Some(serde_json::json!({ "coins": coins, "goal": goal, "won": won })),
        );
    }

    fn reset_round(&mut self) {
        self.beat = Beat::Idle;
        self.cast = fish::Cast::default();
        self.reel = fish::Reel::default();
        self.heat = cook::Heat::default();
        self.coins = 0;
        self.sold = 0;
        self.day_left = self.day_total;
        self.won = false;
        self.face = self.spawn_yaw;
        self.prev_attack = false;
        self.tap_latch = false;
        self.doc.events.clear();
        let y = self.doc.height_at(self.spawn[0], self.spawn[2]) + fish::BODY_H;
        self.write_walker("player", [self.spawn[0], y, self.spawn[2]]);
    }

    fn decorate(&mut self) {
        let name = self.label();
        let xz = self
            .doc
            .player
            .as_ref()
            .map(|p| [p.position[0], p.position[2]])
            .unwrap_or([self.spawn[0], self.spawn[2]]);
        let y = self.doc.height_at(xz[0], xz[1]) + fish::BODY_H;
        self.write_walker(name, [xz[0], y, xz[1]]);

        match self.beat {
            Beat::Cast => fish::show_bobber(&mut self.doc, BOBBER_CAST),
            Beat::Reel => fish::paint_bobber(&mut self.doc, BOBBER_BITE),
            _ => set_enabled(&mut self.doc, "bobber", false),
        }
        let carrying = matches!(self.beat, Beat::Carry | Beat::Cook);
        set_enabled(&mut self.doc, "fish", carrying);
        if self.beat == Beat::Haul {
            cook::show_meal(&mut self.doc);
        } else {
            set_enabled(&mut self.doc, "meal", false);
        }
        let pan = match self.beat {
            Beat::Cook if cook::heat_in_zone(self.heat.value) => PAN_ZONE,
            Beat::Cook => PAN_COOK,
            Beat::Haul => PAN_DONE,
            _ => PAN_IDLE,
        };
        cook::paint_pan(&mut self.doc, pan);
        if self.phase == GamePhase::Complete && self.won {
            set_enabled(&mut self.doc, "flag", true);
            set_color(&mut self.doc, "flag", PAN_DONE);
        } else {
            set_enabled(&mut self.doc, "flag", false);
        }
        self.doc.coins = self.coins;
        if let Some(t) = self.doc.timers.iter_mut().find(|t| t.name == "day") {
            t.remaining = self.day_left.max(0.0);
            t.active = false;
        }
        match (self.phase, self.beat) {
            (GamePhase::Playing, Beat::Carry | Beat::Cook) => {
                cook::place_stove_camera(&mut self.doc)
            }
            (GamePhase::Playing, Beat::Haul) => shop::place_stall_camera(&mut self.doc),
            _ => frame_shore(&mut self.doc),
        }
    }

    fn write_walker(&mut self, name: &str, position: [f32; 3]) {
        let id = self
            .doc
            .player
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| "walker:player".into());
        let model = self
            .doc
            .player
            .as_ref()
            .map(|p| p.model.clone())
            .unwrap_or_default();
        let gltf = self.doc.player.as_ref().and_then(|p| p.gltf.clone());
        let walker = WorldWalker {
            id,
            kind: "walker".into(),
            name: name.into(),
            position,
            yaw: self.face,
            face: self.face,
            on_ground: true,
            model,
            gltf,
            ..Default::default()
        };
        if let Some(existing) = self.doc.player.as_mut() {
            *existing = walker.clone();
        } else {
            self.doc.player = Some(walker.clone());
        }
        let mut found = false;
        for w in &mut self.doc.walkers {
            if w.id == walker.id {
                *w = walker.clone();
                found = true;
            }
        }
        if !found {
            if let Some(first) = self.doc.walkers.first_mut() {
                *first = walker;
            } else {
                self.doc.walkers.push(walker);
            }
        }
    }

    fn tick_timers(&mut self, dt: f32) {
        let mut done = Vec::new();
        for t in &mut self.doc.timers {
            if !t.active || t.name == "day" || t.name == "goal" {
                continue;
            }
            t.remaining = (t.remaining - dt).max(0.0);
            if t.remaining <= 1.0e-4 {
                t.active = false;
                if let Some(ev) = t.on_done.clone() {
                    done.push(ev);
                }
            }
        }
        for ev in done {
            self.record_event(&ev, None);
        }
    }
}

fn axis(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

fn goal_from(doc: &WorldDoc) -> u32 {
    doc.timers
        .iter()
        .find(|t| t.name == "goal")
        .map(|t| {
            let n = t.seconds.round();
            if n.is_finite() && n >= 1.0 {
                n as u32
            } else {
                GOAL_DEFAULT
            }
        })
        .unwrap_or(GOAL_DEFAULT)
}

fn day_from(doc: &WorldDoc) -> f32 {
    doc.timers
        .iter()
        .find(|t| t.name == "day")
        .map(|t| {
            if t.seconds.is_finite() && t.seconds >= 0.25 {
                t.seconds
            } else {
                DAY_DEFAULT
            }
        })
        .unwrap_or(DAY_DEFAULT)
}

fn set_enabled(doc: &mut WorldDoc, name: &str, enabled: bool) {
    if let Some(p) = doc.props.iter_mut().find(|p| p.name == name) {
        p.enabled = enabled;
    }
}

fn set_color(doc: &mut WorldDoc, name: &str, color: [u32; 3]) {
    if let Some(p) = doc.props.iter_mut().find(|p| p.name == name) {
        p.color = Some(color);
    }
}

fn frame_shore(doc: &mut WorldDoc) {
    let eye = [0.4, 9.2, 14.5];
    let target = [0.2, 0.5, 1.2];
    let fov = doc.cameras.first().map(|cam| cam.fov).unwrap_or(46.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye;
        cam.target = target;
        cam.fov = fov;
        cam.name = "shore".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "shore".into(),
            position: eye,
            target,
            fov,
            roll: 0.0,
        });
    }
}

fn line(text: &str, x: f32, y: f32, size: f32, color: [u8; 4], align: TextAlign) -> TextQuad {
    TextQuad::new(text, x, y, size, color).aligned(align)
}

struct MeterBar {
    frac: f32,
    marker: f32,
    lo: f32,
    hi: f32,
    fill: [u8; 4],
}

fn meter(quads: &mut Vec<Quad>, w: f32, h: f32, scale: f32, bar: MeterBar) {
    let x = w * 0.28;
    let y = h * 0.72;
    let bw = w * 0.44;
    let bh = 16.0 * scale;
    quads.push(Quad::new(x, y, bw, bh, [20, 24, 28, 210]));
    let zx = x + bw * bar.lo.clamp(0.0, 1.0);
    let zw = bw * (bar.hi - bar.lo).clamp(0.0, 1.0);
    quads.push(Quad::new(zx, y, zw.max(2.0), bh, [70, 140, 90, 230]));
    quads.push(Quad::new(
        x,
        y,
        bw * bar.frac.clamp(0.0, 1.0),
        bh * 0.45,
        bar.fill,
    ));
    let mx = x + bw * bar.marker.clamp(0.0, 1.0) - 3.0;
    quads.push(Quad::new(mx, y - 3.0, 6.0, bh + 6.0, [255, 244, 220, 255]));
}

pub fn is_fishcook(doc: &WorldDoc) -> bool {
    doc.genre_is(GAME_ID)
}

/// Scripted day: hold the reel in the band, tap the hob in the band, walk, sell.
#[cfg(test)]
pub(crate) fn drive_day(
    s: &mut FishCookSession,
    tick: &mut dyn FnMut(&mut FishCookSession, WalkInput),
) {
    if s.current_phase() == GamePhase::Title {
        s.confirm_action();
    }
    let stove = prop_xz(s, "stove");
    let stall = prop_xz(s, "stall");
    let dock = prop_xz(s, "dock");
    let mut saw_reel = false;
    let mut saw_cook = false;
    for _ in 0..5000 {
        if s.won() {
            assert_hud_ascii(s);
            assert!(saw_reel && saw_cook, "reel {saw_reel} cook {saw_cook}");
            return;
        }
        assert_hud_ascii(s);
        let beat = s.beat();
        if beat == "reel" {
            saw_reel = true;
        }
        if beat == "cook" {
            saw_cook = true;
        }
        let (lx, lz, attack) = match beat {
            "reel" => {
                let m = s.reel_marker();
                (0.0, 0.0, m >= s.reel_lo() && m <= s.reel_hi())
            }
            "cook" => {
                let h = s.heat();
                (0.0, 0.0, h >= s.heat_lo() && h <= s.heat_hi())
            }
            "catch" => {
                let (lx, lz) = steer(player_xz(s), stove);
                (lx, lz, false)
            }
            "cooked" => {
                if s.at_stall() {
                    (0.0, 0.0, true)
                } else {
                    let (lx, lz) = steer(player_xz(s), stall);
                    (lx, lz, false)
                }
            }
            "cast" => (0.0, 0.0, false),
            "player" => {
                if s.on_dock() {
                    (0.0, 0.0, true)
                } else {
                    let (lx, lz) = steer(player_xz(s), dock);
                    (lx, lz, false)
                }
            }
            _ => (0.0, 0.0, false),
        };
        tick(
            s,
            WalkInput {
                lx,
                lz,
                jump: false,
                attack,
                dodge: false,
            },
        );
    }
    panic!(
        "day did not clear beat={} coins={}/{} sold={} day={:.1} heat={:.2} on_dock={} at_stove={} at_stall={} pos={:?}",
        s.beat(),
        s.coins(),
        s.goal(),
        s.sold(),
        s.day_left(),
        s.heat(),
        s.on_dock(),
        s.at_stove(),
        s.at_stall(),
        player_xz(s)
    );
}

#[cfg(test)]
fn prop_xz(s: &FishCookSession, name: &str) -> (f32, f32) {
    s.doc
        .props
        .iter()
        .find(|p| p.name == name)
        .map(|p| (p.position[0], p.position[2]))
        .unwrap_or_else(|| panic!("missing {name}"))
}

#[cfg(test)]
fn player_xz(s: &FishCookSession) -> (f32, f32) {
    s.doc
        .player
        .as_ref()
        .map(|p| (p.position[0], p.position[2]))
        .unwrap_or((0.0, 0.0))
}

#[cfg(test)]
fn steer(from: (f32, f32), to: (f32, f32)) -> (f32, f32) {
    let dx = to.0 - from.0;
    let dz = to.1 - from.1;
    let len = (dx * dx + dz * dz).sqrt();
    if len < 0.2 {
        (0.0, 0.0)
    } else {
        (dx / len, dz / len)
    }
}

#[cfg(test)]
fn assert_hud_ascii(s: &FishCookSession) {
    let hud = s.hud_json(960, 540);
    let v: serde_json::Value = serde_json::from_str(&hud).unwrap();
    for t in v["texts"].as_array().unwrap() {
        let text = t["text"].as_str().unwrap();
        assert!(
            text.is_ascii() && !text.contains('\u{2014}'),
            "HUD must be ASCII, got {text}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const YARD: &str = include_str!("../tests/fixtures/fishcook_world.json");

    fn open() -> FishCookSession {
        FishCookSession::open(YARD).expect("fixture")
    }

    fn playing() -> FishCookSession {
        let mut s = open();
        s.confirm_action();
        s
    }

    fn dt() -> f32 {
        1.0 / 60.0
    }

    fn reach(s: &mut FishCookSession, beat: &str) {
        let stove = prop_xz(s, "stove");
        let dock = prop_xz(s, "dock");
        for _ in 0..800 {
            if s.beat() == beat {
                return;
            }
            let name = s.beat();
            let (lx, lz, attack) = match name {
                "reel" => {
                    let m = s.reel_marker();
                    (0.0, 0.0, m >= s.reel_lo() && m <= s.reel_hi())
                }
                "catch" => {
                    let (lx, lz) = steer(player_xz(s), stove);
                    (lx, lz, false)
                }
                "player" => {
                    if s.on_dock() {
                        (0.0, 0.0, true)
                    } else {
                        let (lx, lz) = steer(player_xz(s), dock);
                        (lx, lz, false)
                    }
                }
                _ => (0.0, 0.0, false),
            };
            s.apply_input(WalkInput {
                lx,
                lz,
                jump: false,
                attack,
                dodge: false,
            });
            s.step(dt());
        }
        panic!("did not reach {beat}, at {}", s.beat());
    }

    #[test]
    fn dump_is_fishcook_with_glb_yard() {
        let s = open();
        assert!(is_fishcook(&s.doc));
        assert_eq!(GAME_ID, "fishcook");
        let dump = s.world_json().unwrap();
        assert!(dump.contains("knight"));
        assert!(dump.contains("gazebo"));
        assert!(dump.contains("mill"));
        assert!(dump.contains("chest"));
        assert!(dump.contains("inn"));
        assert!(dump.contains("stylized_tree"));
        assert!(dump.contains("stylized_pine"));
        assert!(dump.contains("rock"));
        assert!(dump.contains("crate.glb"));
        assert_eq!(s.beat(), "player");
        assert_eq!(s.current_phase(), GamePhase::Title);
        assert_eq!(s.goal(), 8);
        assert!((s.day_left() - 90.0).abs() < 1e-3);
        assert_hud_ascii(&s);
        let dock =
            WorldDoc::from_json(include_str!("../tests/fixtures/fish_cast_world.json")).unwrap();
        assert!(fish::is_fish(&dock));
        assert!(!is_fishcook(&dock));
        let stove =
            WorldDoc::from_json(include_str!("../tests/fixtures/cook_stove_world.json")).unwrap();
        assert!(cook::is_cook(&stove));
        assert!(!is_fishcook(&stove));
    }

    #[test]
    fn title_ignores_attack_and_pointer_starts() {
        let mut s = open();
        s.apply_input(WalkInput {
            lx: 1.0,
            lz: 1.0,
            jump: false,
            attack: true,
            dodge: false,
        });
        for _ in 0..20 {
            s.step(dt());
        }
        assert_eq!(s.current_phase(), GamePhase::Title);
        assert_eq!(s.beat(), "player");
        assert_eq!(s.coins(), 0);
        s.pointer(12.0, 12.0, 960, 540);
        assert_eq!(s.current_phase(), GamePhase::Playing);
    }

    #[test]
    fn hold_pad_tap_casts_from_the_dock() {
        let mut s = playing();
        assert!(s.on_dock());
        let (x, y, _, _) = FishCookSession::hold_pad(960, 540);
        s.pointer(x + 8.0, y + 8.0, 960, 540);
        s.step(dt());
        assert_eq!(s.beat(), "cast");
        let mut s = playing();
        s.pointer(4.0, 4.0, 960, 540);
        s.step(dt());
        assert_eq!(s.beat(), "player");
    }

    #[test]
    fn held_reel_snaps_back_to_the_dock() {
        let mut s = playing();
        reach(&mut s, "reel");
        for _ in 0..180 {
            s.apply_input(WalkInput {
                lx: 0.0,
                lz: 0.0,
                jump: false,
                attack: true,
                dodge: false,
            });
            s.step(dt());
            if s.beat() == "player" {
                break;
            }
        }
        assert_eq!(s.beat(), "player");
        assert_eq!(s.coins(), 0);
        assert!(!s.won());
    }

    #[test]
    fn early_tap_resets_heat_and_a_miss_burns_the_fish() {
        let mut s = playing();
        reach(&mut s, "cook");
        s.apply_input(WalkInput {
            lx: 0.0,
            lz: 0.0,
            jump: false,
            attack: true,
            dodge: false,
        });
        s.step(dt());
        assert_eq!(s.beat(), "cook", "a raw tap is not a dish");
        assert!(s.heat() < 0.05, "heat {}", s.heat());
        s.apply_input(WalkInput::default());
        for _ in 0..200 {
            s.step(dt());
            if s.beat() == "player" {
                break;
            }
        }
        assert_eq!(s.beat(), "player");
        assert_eq!(s.coins(), 0);
        assert_eq!(s.drain_events("burnt").len(), 1);
    }

    #[test]
    fn day_clock_ends_short_of_the_goal() {
        let mut s = FishCookSession::open(
            r#"{"version":1,"genre":"fishcook","half":8,"timers":[
                {"id":"timer:day","name":"day","seconds":0.4,"remaining":0.4,"active":false},
                {"id":"timer:goal","name":"goal","seconds":8,"remaining":8,"active":false}
            ]}"#,
        )
        .unwrap();
        s.confirm_action();
        for _ in 0..40 {
            s.step(dt());
        }
        assert_eq!(s.current_phase(), GamePhase::Complete);
        assert!(!s.won());
        assert_eq!(s.coins(), 0);
        assert_eq!(s.beat(), "result");
        assert!(s.hud_json(320, 180).contains("short"));
        assert_hud_ascii(&s);
    }

    #[test]
    fn world_play_does_not_run_the_day() {
        let mut play = WorldPlay::from_json(YARD).unwrap();
        assert!(!play.is_fish());
        assert!(!play.is_cook());
        assert!(!play.is_shop());
        play.confirm();
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(!play.fish.waiting);
        assert!(!play.cook.cooking);
        assert!(!play.cook.done);
    }

    #[test]
    fn timer_hook_emits_when_it_elapses() {
        let mut s = open();
        let id = s.begin_timer("boil", 0.5, Some("boiled"));
        assert_eq!(id, "timer:boil");
        s.step(0.6);
        assert_eq!(s.drain_events("boiled").len(), 1);
        assert!(s.drain_events("missing").is_empty());
    }

    #[test]
    fn scripted_day_meets_the_coin_goal() {
        let mut s = open();
        drive_day(&mut s, &mut |session, input| {
            session.apply_input(input);
            session.step(dt());
        });
        assert!(s.won() && s.done());
        assert_eq!(s.current_phase(), GamePhase::Complete);
        assert_eq!(s.coins(), s.goal());
        assert_eq!(s.sold(), s.goal() / shop::SALE_PRICE);
        let hud = s.hud_json(640, 360);
        assert!(hud.contains("DAY END"));
        assert!(hud.contains("goal met"));
        let sold = s.drain_events("sold");
        assert_eq!(sold.len(), 1);
        assert_eq!(sold[0].count, s.sold());
    }

    #[test]
    fn wrong_genre_is_rejected() {
        let err = FishCookSession::open(include_str!("../tests/fixtures/crest_isle_world.json"))
            .unwrap_err();
        assert!(err.contains("fishcook"));
    }
}
