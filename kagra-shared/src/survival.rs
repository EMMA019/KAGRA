//! Hunger/stamina meter ticks down as an M3 slice on play_world.
//!
//! Sibling of sim. Player walks a capsule; a box is the camp; a box is
//! the ration. Meter (`coins`) drains over time. Standing in the camp or
//! picking the ration fills it. Empty meter is dump-visible `starve`;
//! refill to full is dump-visible `ok`. Title -> play -> result reuses
//! `WorldPlay` / `GamePhase`. Capsules/boxes, not VRM. Indoor lights stay
//! 4 slots. Camp/ration stay readable (contact blob plus metal GGX
//! inherited). Chase camera follows play. Does not rewrite other genre
//! loops. No Rapier, SSAO, GI, or ECS. Picture slice stays.
//!
//! `SurvivalSession` (`genre: survival`) is a separate camp loop in this
//! module: three day/night cycles, hunger, health, wood and food, a campfire,
//! and a shelter. It does not plug into `WorldPlay` — the meter slice keeps
//! `survival_meter`.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{
    WorldCamera, WorldDoc, WorldEvent, WorldLight, WorldProp, WorldTimer, WorldWalker,
};

pub const GAME_ID: &str = "survival_meter";
/// Camp loop (`SurvivalSession`). Not the `survival_meter` WorldPlay slice.
pub const CAMP_ID: &str = "survival";
pub const DAYS_TO_WIN: u32 = 3;
pub const DAY_SECONDS: f32 = 22.0;
pub const BODY_H: f32 = 0.95;
pub const PLAYER_R: f32 = 0.46;
pub const NEED: u32 = 8;
pub const DRAIN_PER_SEC: f32 = 3.2;
pub const FILL_PER_SEC: f32 = 6.0;
pub const RATION_FILL: f32 = 4.0;
pub const PICK_REACH: f32 = 1.35;
pub const CAM_BACK: f32 = 7.2;
pub const CAM_UP: f32 = 4.8;
pub const CAM_LOOK: f32 = 0.55;
pub const NAME_PLAYER: &str = "player";
pub const NAME_FILLING: &str = "filling";
pub const NAME_OK: &str = "ok";
pub const NAME_STARVE: &str = "starve";

const FLAG_OK_COLOR: [u32; 3] = [70, 180, 110];
const FILL_PIP: [u8; 4] = [240, 196, 72, 255];
const OK_PIP: [u8; 4] = [70, 180, 110, 255];
const LOW_PIP: [u8; 4] = [196, 64, 54, 255];

/// Live survival around a dump. Filling/ok/starve stay here; `name` + flag
/// enable and `coins` in the dump are the query source of truth. Meter is
/// time, not Rapier.
#[derive(Clone, Debug)]
pub struct SurvivalGame {
    pub filling: bool,
    pub ok: bool,
    pub starve: bool,
    pub done: bool,
    pub hunger: f32,
    pub coins: u32,
    pub picked: bool,
}

impl Default for SurvivalGame {
    fn default() -> Self {
        Self {
            filling: false,
            ok: false,
            starve: false,
            done: false,
            hunger: NEED as f32,
            coins: NEED,
            picked: false,
        }
    }
}

impl SurvivalGame {
    pub fn from_doc(_doc: &WorldDoc) -> Self {
        Self::default()
    }
}

pub fn is_survival(doc: &WorldDoc) -> bool {
    doc.genre_is(GAME_ID)
}

fn player_ref(doc: &WorldDoc) -> Option<&WorldWalker> {
    doc.player.as_ref().or(doc.walkers.first())
}

fn write_player(doc: &mut WorldDoc, walker: WorldWalker) {
    if let Some(existing) = doc.player.as_mut() {
        *existing = walker.clone();
    } else {
        doc.player = Some(walker.clone());
    }
    let mut found = false;
    for w in &mut doc.walkers {
        if w.id == walker.id {
            *w = walker.clone();
            found = true;
        }
    }
    if !found {
        if let Some(first) = doc.walkers.first_mut() {
            *first = walker;
        } else {
            doc.walkers.push(walker);
        }
    }
}

/// Sit capsules/boxes on the floor, keep the flag off, meter full, chase cam.
pub fn seed(doc: &mut WorldDoc) {
    if !is_survival(doc) {
        return;
    }
    sit_named(doc, NAME_PLAYER);
    sit_props(doc);
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = false;
        }
        if p.name == "ration" {
            p.enabled = true;
        }
    }
    doc.coins = NEED;
    place_chase_camera(doc);
}

/// Chase cam behind the player looking at the camp so the pad reads.
pub fn place_chase_camera(doc: &mut WorldDoc) {
    let Some(w) = player_ref(doc) else {
        return;
    };
    let px = w.position[0];
    let py = w.position[1];
    let pz = w.position[2];
    let yaw = w.yaw;
    let (s, c) = yaw.sin_cos();
    let (zx, zy, zz) = named_prop(doc, "camp")
        .map(|p| (p.position[0], p.position[1], p.position[2]))
        .unwrap_or((px, py, pz - 2.0));
    let eye = [px - s * CAM_BACK, py + CAM_UP, pz - c * CAM_BACK];
    let target = [
        zx * 0.55 + px * 0.45,
        zy.max(CAM_LOOK),
        zz * 0.55 + pz * 0.45,
    ];
    let fov = doc.cameras.first().map(|cam| cam.fov).unwrap_or(52.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye;
        cam.target = target;
        cam.name = "chase".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "chase".into(),
            position: eye,
            target,
            fov,
            roll: 0.0,
        });
    }
}

/// Meter drains unless the player stands in camp or picks a ration.
/// Caller already walked; chase cam wins.
pub fn tick(doc: &mut WorldDoc, game: &mut SurvivalGame, _input: WalkInput, dt: f32) {
    if game.done {
        write_beat(doc, game);
        place_chase_camera(doc);
        return;
    }
    let inside = player_in_camp(doc);
    let picked_now = pick_ration(doc);
    if picked_now {
        game.picked = true;
    }
    let restoring = inside || picked_now;
    game.filling = restoring && !game.ok && !game.starve;
    let was = game.hunger;
    if restoring {
        let mut add = 0.0;
        if inside {
            add += FILL_PER_SEC * dt;
        }
        if picked_now {
            add += RATION_FILL;
        }
        game.hunger = (game.hunger + add).min(NEED as f32);
        if game.hunger >= NEED as f32 - 1e-3 && was < NEED as f32 - 1e-3 {
            game.ok = true;
            game.done = true;
            game.filling = false;
            game.hunger = NEED as f32;
        }
    } else {
        game.hunger = (game.hunger - DRAIN_PER_SEC * dt).max(0.0);
        if game.hunger <= 1e-3 {
            game.starve = true;
            game.done = true;
            game.filling = false;
            game.hunger = 0.0;
        }
    }
    game.coins = game.hunger.round().clamp(0.0, NEED as f32) as u32;
    write_beat(doc, game);
    place_chase_camera(doc);
}

fn beat_name(game: &SurvivalGame) -> &'static str {
    if game.starve {
        NAME_STARVE
    } else if game.ok {
        NAME_OK
    } else if game.filling {
        NAME_FILLING
    } else {
        NAME_PLAYER
    }
}

fn write_beat(doc: &mut WorldDoc, game: &SurvivalGame) {
    sit_named(doc, beat_name(game));
    doc.coins = game.coins;
    if game.ok {
        set_flag_prop(doc);
    }
}

fn set_flag_prop(doc: &mut WorldDoc) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = true;
            p.color = Some(FLAG_OK_COLOR);
        }
    }
}

fn sit_named(doc: &mut WorldDoc, name: &str) {
    let Some(p) = player_ref(doc) else {
        return;
    };
    let id = p.id.clone();
    let x = p.position[0];
    let z = p.position[2];
    let yaw = p.yaw;
    let y = doc.height_at(x, z) + BODY_H;
    write_player(
        doc,
        WorldWalker {
            id,
            kind: "walker".into(),
            name: name.into(),
            position: [x, y, z],
            yaw,
            face: yaw,
            on_ground: true,
            ..Default::default()
        },
    );
}

fn sit_props(doc: &mut WorldDoc) {
    let mut updates: Vec<(usize, f32)> = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        let extra = match p.name.as_str() {
            "camp" | "ration" | "flag" | "floor" | "post" => p.scale[1].abs() * 0.5,
            _ => continue,
        };
        let y = doc.height_at(p.position[0], p.position[2]) + extra;
        updates.push((i, y));
    }
    for (i, y) in updates {
        if let Some(p) = doc.props.get_mut(i) {
            p.position[1] = y;
        }
    }
}

fn named_prop<'a>(doc: &'a WorldDoc, name: &str) -> Option<&'a WorldProp> {
    doc.props.iter().find(|p| p.name == name && p.enabled)
}

fn player_in_camp(doc: &WorldDoc) -> bool {
    let Some(player) = player_ref(doc) else {
        return false;
    };
    let Some(camp) = named_prop(doc, "camp") else {
        return false;
    };
    let hx = camp.scale[0].abs() * 0.5 + PLAYER_R;
    let hz = camp.scale[2].abs() * 0.5 + PLAYER_R;
    let dx = player.position[0] - camp.position[0];
    let dz = player.position[2] - camp.position[2];
    dx.abs() <= hx && dz.abs() <= hz
}

fn pick_ration(doc: &mut WorldDoc) -> bool {
    let Some(player) = player_ref(doc) else {
        return false;
    };
    let px = player.position[0];
    let pz = player.position[2];
    let mut picked = false;
    for p in &mut doc.props {
        if !p.enabled || p.name != "ration" {
            continue;
        }
        let dx = px - p.position[0];
        let dz = pz - p.position[2];
        if (dx * dx + dz * dz).sqrt() <= PICK_REACH {
            p.enabled = false;
            picked = true;
        }
    }
    picked
}

pub fn build_hud(game: &SurvivalGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let mut quads = Vec::new();
    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [12, 14, 18, 160]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [22, 24, 32, 230],
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.58,
                w * 0.36,
                52.0 * scale,
                [240, 196, 72, 255],
            ));
        }
        GamePhase::Playing => {
            let frac = (game.hunger / NEED as f32).clamp(0.0, 1.0);
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                160.0 * scale,
                18.0 * scale,
                [28, 24, 20, 160],
            ));
            let pip = if game.filling {
                FILL_PIP
            } else if frac < 0.34 {
                LOW_PIP
            } else {
                [80, 84, 78, 200]
            };
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                (160.0 * scale * frac).max(if frac > 0.0 { 2.0 } else { 0.0 }),
                18.0 * scale,
                pip,
            ));
            quads.push(Quad::new(
                184.0 * scale,
                16.0 * scale,
                22.0 * scale,
                22.0 * scale,
                if game.ok { OK_PIP } else { [36, 48, 40, 160] },
            ));
        }
        GamePhase::Complete => {
            if game.starve {
                quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [28, 12, 12, 210]));
                quads.push(Quad::new(
                    w * 0.32,
                    h * 0.62,
                    w * 0.36,
                    48.0 * scale,
                    [196, 64, 54, 240],
                ));
            } else {
                quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 28, 18, 210]));
                quads.push(Quad::new(
                    w * 0.32,
                    h * 0.62,
                    w * 0.36,
                    48.0 * scale,
                    [70, 180, 110, 240],
                ));
            }
        }
    }
    DrawList {
        clear: [24, 28, 34, 255],
        quads,
        ..Default::default()
    }
}

const NIGHT_AT: f32 = 0.62;
const WALK_SPEED: f32 = 7.6;
const HUNGER_MAX: f32 = 100.0;
const HEALTH_MAX: f32 = 100.0;
const HUNGER_DRAIN: f32 = 2.05;
const EAT_GAIN: f32 = 48.0;
const EAT_BELOW: f32 = 40.0;
const STARVE_DPS: f32 = 22.0;
const NIGHT_DPS: f32 = 26.0;
const FIRE_COST: u32 = 4;
const SHELTER_COST: u32 = 4;
const WOOD_YIELD: u32 = 2;
const GATHER_REACH: f32 = 1.85;
const GATHER_CD: f32 = 0.28;
const CRAFT_REACH: f32 = 2.6;
const FIRE_SAFE: f32 = 3.3;
const SHELTER_SAFE: f32 = 3.8;
const DUSK_AT: f32 = 0.50;
const DAY_SKY: [u8; 4] = [176, 214, 242, 255];
const NIGHT_SKY: [u8; 4] = [10, 14, 32, 255];

pub fn is_camp(doc: &WorldDoc) -> bool {
    doc.genre_is(CAMP_ID)
}

#[derive(Clone, Debug)]
struct CampGame {
    day: u32,
    clock: f32,
    hunger: f32,
    health: f32,
    wood: u32,
    food: u32,
    fire: bool,
    shelter: bool,
    won: bool,
    dead: bool,
    night: bool,
    saw_night: bool,
    gather_cd: f32,
    eat_cd: f32,
    craft_cd: f32,
}

impl CampGame {
    fn fresh() -> Self {
        Self {
            day: 1,
            clock: 0.0,
            hunger: HUNGER_MAX,
            health: HEALTH_MAX,
            wood: 0,
            food: 0,
            fire: false,
            shelter: false,
            won: false,
            dead: false,
            night: false,
            saw_night: false,
            gather_cd: 0.0,
            eat_cd: 0.0,
            craft_cd: 0.0,
        }
    }
}

/// Three-day camp (`genre: survival`). Walk, gather, craft, and the clock
/// stay here. Python sends stick / gather / eat / build and draws the dump.
#[derive(Clone, Debug)]
pub struct SurvivalSession {
    doc: WorldDoc,
    seed: String,
    camp: CampGame,
    input: WalkInput,
    phase: GamePhase,
    look_yaw: f32,
}

impl SurvivalSession {
    pub fn open(json: &str) -> Result<Self, String> {
        let doc = WorldDoc::from_json(json)?;
        if !is_camp(&doc) {
            return Err(format!(
                "WorldDoc.genre must be {:?}, got {:?}",
                CAMP_ID, doc.genre
            ));
        }
        if doc.player.is_none() {
            return Err("survival dump needs a player".into());
        }
        if !doc.props.iter().any(|p| p.name == "campfire") {
            return Err("survival dump needs a campfire prop".into());
        }
        if !doc.props.iter().any(|p| p.name == "shelter") {
            return Err("survival dump needs a shelter prop".into());
        }
        let mut session = Self {
            doc,
            seed: json.to_string(),
            camp: CampGame::fresh(),
            input: WalkInput::default(),
            phase: GamePhase::Title,
            look_yaw: 0.0,
        };
        session.prepare();
        Ok(session)
    }

    pub fn current_phase(&self) -> GamePhase {
        self.phase
    }

    pub fn day(&self) -> u32 {
        self.camp.day
    }

    pub fn days_need(&self) -> u32 {
        DAYS_TO_WIN
    }

    pub fn time_of_day(&self) -> f32 {
        (self.camp.clock / DAY_SECONDS).clamp(0.0, 1.0)
    }

    pub fn night(&self) -> bool {
        self.camp.night
    }

    pub fn hunger(&self) -> f32 {
        self.camp.hunger
    }

    pub fn health(&self) -> f32 {
        self.camp.health
    }

    pub fn wood(&self) -> u32 {
        self.camp.wood
    }

    pub fn food(&self) -> u32 {
        self.camp.food
    }

    pub fn fire(&self) -> bool {
        self.camp.fire
    }

    pub fn shelter(&self) -> bool {
        self.camp.shelter
    }

    pub fn won(&self) -> bool {
        self.camp.won
    }

    pub fn dead(&self) -> bool {
        self.camp.dead
    }

    pub fn is_safe(&self) -> bool {
        let (px, pz) = self.player_xz();
        self.is_safe_at(px, pz)
    }

    /// ASCII HUD line. Hyphen, not an em dash.
    pub fn status(&self) -> String {
        if self.camp.won {
            return "survived 3 days".into();
        }
        if self.camp.dead {
            return if self.camp.hunger <= 0.05 {
                "down - starved".into()
            } else {
                "down - night".into()
            };
        }
        let phase = if self.camp.night { "night" } else { "day" };
        let mut line = format!(
            "day {}/{} - {}  HP {:.0}  HUN {:.0}  wood {}  food {}",
            self.camp.day.min(DAYS_TO_WIN),
            DAYS_TO_WIN,
            phase,
            self.camp.health,
            self.camp.hunger,
            self.camp.wood,
            self.camp.food
        );
        if self.camp.fire {
            line.push_str("  fire");
        }
        if self.camp.shelter {
            line.push_str("  shelter");
        }
        if self.camp.night && self.is_safe() {
            line.push_str("  safe");
        }
        line
    }

    pub fn apply_input(&mut self, input: WalkInput) {
        self.input = input.clamped();
    }

    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        if dt <= 0.0 || self.phase != GamePhase::Playing || self.camp.won || self.camp.dead {
            return;
        }
        let input = self.input;
        self.act(input, dt);
        self.step_move(input, dt);
        self.tick_timers(dt);
        self.advance_clock(dt);
        self.apply_vitals(dt);
        self.finish_if_needed();
        self.present();
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
        match self.phase {
            GamePhase::Title => {
                self.phase = GamePhase::Playing;
                self.present();
            }
            GamePhase::Playing => {}
            GamePhase::Complete => {
                let seed = self.seed.clone();
                if let Ok(doc) = WorldDoc::from_json(&seed) {
                    self.doc = doc;
                    self.camp = CampGame::fresh();
                    self.input = WalkInput::default();
                    self.phase = GamePhase::Playing;
                    self.prepare();
                }
            }
        }
    }

    pub fn record_event(&mut self, name: &str, data: Option<serde_json::Value>) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(ev) = self.doc.events.iter_mut().find(|e| e.name == name) {
            ev.count = ev.count.saturating_add(1);
            if data.is_some() {
                ev.data = data;
            }
            return;
        }
        self.doc.events.push(WorldEvent {
            id: format!("event:{}", self.doc.events.len()),
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

    /// Gather the nearest wood or food in reach (attack / interact).
    pub fn use_nearby(&mut self) {
        if self.phase != GamePhase::Playing || self.camp.won || self.camp.dead {
            return;
        }
        let (px, pz) = self.player_xz();
        if self.gather_at(px, pz) {
            self.camp.gather_cd = GATHER_CD;
            self.present();
        }
    }

    /// Scripted stick for headless play. Human input stays on the host.
    pub fn demo_input(&self) -> WalkInput {
        if self.phase != GamePhase::Playing || self.camp.won || self.camp.dead {
            return WalkInput::default();
        }
        let (px, pz) = self.player_xz();
        let eat = self.camp.hunger < EAT_BELOW && self.camp.food > 0;
        let frac = self.time_of_day();
        if self.camp.fire {
            let far = prop_dist(&self.doc, "campfire", px, pz).unwrap_or(99.0);
            if (self.camp.night || frac >= DUSK_AT) && far > FIRE_SAFE - 0.45 {
                let (lx, lz, _) = steer_toward(px, pz, camp_anchor(&self.doc), self.look_yaw, 0.85);
                return WalkInput {
                    lx,
                    lz,
                    jump: eat,
                    attack: false,
                    dodge: false,
                };
            }
        }
        if !self.camp.fire && self.camp.wood >= FIRE_COST {
            return self.craft_toward(px, pz, eat);
        }
        if !self.camp.fire && self.camp.wood < FIRE_COST {
            if let Some(spot) = nearest_xz(&self.doc, px, pz, is_wood) {
                return self.gather_toward(px, pz, spot, eat);
            }
        }
        if self.camp.fire && self.camp.food == 0 {
            if let Some(spot) = nearest_xz(&self.doc, px, pz, is_food) {
                return self.gather_toward(px, pz, spot, eat);
            }
        }
        if self.camp.fire && !self.camp.shelter && self.camp.wood >= SHELTER_COST {
            return self.craft_toward(px, pz, eat);
        }
        if self.camp.fire && !self.camp.shelter && self.camp.wood < SHELTER_COST {
            if let Some(spot) = nearest_xz(&self.doc, px, pz, is_wood) {
                return self.gather_toward(px, pz, spot, eat);
            }
        }
        if self.camp.food < 2 {
            if let Some(spot) = nearest_xz(&self.doc, px, pz, is_food) {
                return self.gather_toward(px, pz, spot, eat);
            }
        }
        if self.camp.fire {
            let (lx, lz, _) = steer_toward(px, pz, camp_anchor(&self.doc), self.look_yaw, 0.9);
            return WalkInput {
                lx,
                lz,
                jump: eat,
                attack: false,
                dodge: false,
            };
        }
        WalkInput {
            lx: 0.0,
            lz: 0.0,
            jump: eat,
            attack: false,
            dodge: false,
        }
    }

    fn craft_toward(&self, px: f32, pz: f32, eat: bool) -> WalkInput {
        let (lx, lz, there) = steer_toward(px, pz, camp_anchor(&self.doc), self.look_yaw, 1.15);
        WalkInput {
            lx,
            lz,
            jump: eat,
            attack: false,
            dodge: there,
        }
    }

    fn gather_toward(&self, px: f32, pz: f32, spot: (f32, f32), eat: bool) -> WalkInput {
        let (lx, lz, there) = steer_toward(px, pz, spot, self.look_yaw, 1.15);
        WalkInput {
            lx,
            lz,
            jump: eat,
            attack: there,
            dodge: false,
        }
    }

    fn prepare(&mut self) {
        for p in &mut self.doc.props {
            if p.name == "campfire" || p.name == "shelter" {
                p.enabled = false;
            }
        }
        self.camp.fire = false;
        self.camp.shelter = false;
        if let Some(w) = self.doc.player.clone() {
            self.look_yaw = w.yaw;
            let y = self.doc.height_at(w.position[0], w.position[2]) + 0.02;
            let mut w = w;
            w.position[1] = y;
            w.on_ground = true;
            if w.name.is_empty() {
                w.name = "player".into();
            }
            write_player(&mut self.doc, w);
        }
        self.doc.coins = 0;
        self.present();
    }

    fn act(&mut self, input: WalkInput, dt: f32) {
        self.camp.gather_cd = (self.camp.gather_cd - dt).max(0.0);
        self.camp.eat_cd = (self.camp.eat_cd - dt).max(0.0);
        self.camp.craft_cd = (self.camp.craft_cd - dt).max(0.0);
        let (px, pz) = self.player_xz();
        if input.attack && self.camp.gather_cd <= 0.0 && self.gather_at(px, pz) {
            self.camp.gather_cd = GATHER_CD;
        }
        if input.jump && self.camp.eat_cd <= 0.0 && self.try_eat() {
            self.camp.eat_cd = 0.4;
        }
        if input.dodge && self.camp.craft_cd <= 0.0 && self.try_craft(px, pz) {
            self.camp.craft_cd = 0.35;
        }
    }

    fn gather_at(&mut self, px: f32, pz: f32) -> bool {
        let Some(i) = nearest_within(&self.doc, px, pz, GATHER_REACH, |p| {
            is_wood(p) || is_food(p)
        }) else {
            return false;
        };
        let wood = is_wood(&self.doc.props[i]);
        self.doc.props[i].enabled = false;
        if wood {
            self.camp.wood = self.camp.wood.saturating_add(WOOD_YIELD);
            emit(&mut self.doc, "gather_wood");
        } else {
            self.camp.food = self.camp.food.saturating_add(1);
            emit(&mut self.doc, "gather_food");
        }
        true
    }

    fn try_eat(&mut self) -> bool {
        if self.camp.food == 0 || self.camp.hunger >= HUNGER_MAX - 0.5 {
            return false;
        }
        self.camp.food -= 1;
        self.camp.hunger = (self.camp.hunger + EAT_GAIN).min(HUNGER_MAX);
        emit(&mut self.doc, "eat");
        true
    }

    fn try_craft(&mut self, px: f32, pz: f32) -> bool {
        if !self.at_campfire(px, pz) {
            return false;
        }
        if !self.camp.fire && self.camp.wood >= FIRE_COST {
            self.camp.wood -= FIRE_COST;
            self.camp.fire = true;
            emit(&mut self.doc, "craft_fire");
            return true;
        }
        if self.camp.fire && !self.camp.shelter && self.camp.wood >= SHELTER_COST {
            self.camp.wood -= SHELTER_COST;
            self.camp.shelter = true;
            emit(&mut self.doc, "build_shelter");
            return true;
        }
        false
    }

    fn step_move(&mut self, input: WalkInput, dt: f32) {
        let half = self.doc.half.max(8.0);
        let (s, c) = self.look_yaw.sin_cos();
        let wish_x = -c * input.lx + s * input.lz;
        let wish_z = s * input.lx + c * input.lz;
        let wish_len = (wish_x * wish_x + wish_z * wish_z).sqrt();
        let Some(mut w) = player_ref(&self.doc).cloned() else {
            return;
        };
        let mut x = w.position[0];
        let mut z = w.position[2];
        if wish_len > 0.08 {
            let inv = 1.0 / wish_len;
            let dir_x = wish_x * inv;
            let dir_z = wish_z * inv;
            let speed = WALK_SPEED * wish_len.min(1.0);
            x += dir_x * speed * dt;
            z += dir_z * speed * dt;
            let yaw = dir_x.atan2(dir_z);
            w.yaw = yaw;
            w.face = yaw;
            self.look_yaw = yaw;
            w.anim = "walk".into();
            w.clip += dt;
        } else if w.anim == "walk" {
            w.anim = "idle".into();
            w.clip = 0.0;
        }
        let pad = 1.2;
        x = x.clamp(-half + pad, half - pad);
        z = z.clamp(-half + pad, half - pad);
        let y = self.doc.height_at(x, z) + 0.02;
        w.position = [x, y, z];
        w.on_ground = true;
        if w.name.is_empty() {
            w.name = "player".into();
        }
        write_player(&mut self.doc, w);
    }

    fn tick_timers(&mut self, dt: f32) {
        let mut done: Vec<String> = Vec::new();
        for t in &mut self.doc.timers {
            if t.name == "day" || !t.active {
                continue;
            }
            t.remaining = (t.remaining - dt).max(0.0);
            if t.remaining <= 1e-4 {
                t.active = false;
                if let Some(ev) = t.on_done.clone() {
                    done.push(ev);
                }
            }
        }
        for ev in done {
            emit(&mut self.doc, &ev);
        }
    }

    fn advance_clock(&mut self, dt: f32) {
        self.camp.clock += dt;
        while self.camp.clock >= DAY_SECONDS {
            self.camp.clock -= DAY_SECONDS;
            self.camp.day = self.camp.day.saturating_add(1);
            emit(&mut self.doc, "dawn");
        }
        let night = self.time_of_day() >= NIGHT_AT;
        if night && !self.camp.saw_night {
            self.camp.saw_night = true;
            emit(&mut self.doc, "night");
        }
        if !night {
            self.camp.saw_night = false;
        }
        self.camp.night = night;
    }

    fn apply_vitals(&mut self, dt: f32) {
        self.camp.hunger = (self.camp.hunger - HUNGER_DRAIN * dt).max(0.0);
        if self.camp.hunger <= 0.05 {
            self.camp.health -= STARVE_DPS * dt;
        }
        let (px, pz) = self.player_xz();
        if self.camp.night && !self.is_safe_at(px, pz) {
            self.camp.health -= NIGHT_DPS * dt;
        }
        self.camp.health = self.camp.health.clamp(0.0, HEALTH_MAX);
    }

    fn finish_if_needed(&mut self) {
        if self.camp.health <= 0.05 {
            self.camp.health = 0.0;
            self.camp.dead = true;
            self.camp.won = false;
            self.phase = GamePhase::Complete;
            emit(&mut self.doc, "dead");
            return;
        }
        if self.camp.day > DAYS_TO_WIN && self.camp.fire && self.camp.shelter {
            self.camp.won = true;
            self.phase = GamePhase::Complete;
            emit(&mut self.doc, "win");
        }
    }

    fn present(&mut self) {
        let fire = self.camp.fire;
        let shelter = self.camp.shelter;
        for p in &mut self.doc.props {
            if p.name == "campfire" {
                p.enabled = fire;
            } else if p.name == "shelter" {
                p.enabled = shelter;
            }
        }
        self.doc.coins = self.camp.wood;
        let night_amt = night_amount(self.time_of_day());
        self.doc.sky = Some(mix_sky(night_amt));
        self.doc.exposure = Some(1.08 - 0.55 * night_amt);
        self.doc.ibl = Some(0.42 - 0.30 * night_amt);
        self.write_day_timer();
        self.write_lights(night_amt);
        self.place_camera();
    }

    fn write_day_timer(&mut self) {
        let remaining = (DAY_SECONDS - self.camp.clock).max(0.0);
        if let Some(t) = self.doc.timers.iter_mut().find(|t| t.name == "day") {
            t.seconds = DAY_SECONDS;
            t.remaining = remaining;
            t.active = self.phase == GamePhase::Playing && !self.camp.won && !self.camp.dead;
            return;
        }
        self.doc.timers.push(WorldTimer {
            id: "timer:day".into(),
            kind: "timer".into(),
            name: "day".into(),
            seconds: DAY_SECONDS,
            remaining,
            on_done: None,
            active: self.phase == GamePhase::Playing && !self.camp.won && !self.camp.dead,
        });
    }

    fn write_lights(&mut self, night_amt: f32) {
        let (fx, fz) = camp_anchor(&self.doc);
        let key = 1.45 * (1.0 - night_amt) + 0.05;
        paint_light(
            &mut self.doc,
            0,
            "sun",
            [5.0, 9.5, 8.0],
            key,
            40.0,
            mix_rgb([1.0, 0.94, 0.82], [0.35, 0.45, 0.75], night_amt),
        );
        paint_light(
            &mut self.doc,
            1,
            "fill",
            [-6.0, 4.0, 2.0],
            0.38 * (1.0 - night_amt) + 0.04,
            24.0,
            [0.75, 0.84, 0.95],
        );
        let fire_i = if self.camp.fire { 2.8 } else { 0.0 };
        paint_light(
            &mut self.doc,
            2,
            "fire",
            [fx, 1.15, fz],
            fire_i,
            9.0,
            [1.0, 0.55, 0.18],
        );
        paint_light(
            &mut self.doc,
            3,
            "moon",
            [-3.0, 7.0, -6.0],
            0.12 + 0.42 * night_amt,
            28.0,
            [0.55, 0.68, 0.95],
        );
    }

    fn place_camera(&mut self) {
        let Some(w) = player_ref(&self.doc) else {
            return;
        };
        let px = w.position[0];
        let py = w.position[1];
        let pz = w.position[2];
        let (s, c) = self.look_yaw.sin_cos();
        let eye = [px - s * 7.4, py + 4.6, pz - c * 7.4];
        let target = [px, py + 1.15, pz];
        let fov = self.doc.cameras.first().map(|cam| cam.fov).unwrap_or(52.0);
        if let Some(cam) = self.doc.cameras.first_mut() {
            cam.position = eye;
            cam.target = target;
            cam.name = "chase".into();
        } else {
            self.doc.cameras.push(WorldCamera {
                id: "camera:main".into(),
                kind: "camera".into(),
                name: "chase".into(),
                position: eye,
                target,
                fov,
                roll: 0.0,
            });
        }
    }

    fn player_xz(&self) -> (f32, f32) {
        player_ref(&self.doc)
            .map(|w| (w.position[0], w.position[2]))
            .unwrap_or((0.0, 0.0))
    }

    fn at_campfire(&self, px: f32, pz: f32) -> bool {
        prop_dist(&self.doc, "campfire", px, pz).is_some_and(|d| d <= CRAFT_REACH)
    }

    fn is_safe_at(&self, px: f32, pz: f32) -> bool {
        if self.camp.fire
            && prop_dist(&self.doc, "campfire", px, pz).is_some_and(|d| d <= FIRE_SAFE)
        {
            return true;
        }
        if self.camp.shelter
            && prop_dist(&self.doc, "shelter", px, pz).is_some_and(|d| d <= SHELTER_SAFE)
        {
            return true;
        }
        false
    }
}

fn is_wood(p: &WorldProp) -> bool {
    matches!(p.name.as_str(), "tree" | "pine" | "wood")
}

fn is_food(p: &WorldProp) -> bool {
    p.name == "food"
}

fn emit(doc: &mut WorldDoc, name: &str) {
    if let Some(ev) = doc.events.iter_mut().find(|e| e.name == name) {
        ev.count = ev.count.saturating_add(1);
        return;
    }
    doc.events.push(WorldEvent {
        id: format!("event:{}", doc.events.len()),
        kind: "event".into(),
        name: name.into(),
        count: 1,
        data: None,
    });
}

fn prop_dist(doc: &WorldDoc, name: &str, px: f32, pz: f32) -> Option<f32> {
    doc.props.iter().find(|p| p.name == name).map(|p| {
        let dx = p.position[0] - px;
        let dz = p.position[2] - pz;
        dx.hypot(dz)
    })
}

fn camp_anchor(doc: &WorldDoc) -> (f32, f32) {
    doc.props
        .iter()
        .find(|p| p.name == "campfire")
        .map(|p| (p.position[0], p.position[2]))
        .unwrap_or((0.0, -1.0))
}

fn nearest_within(
    doc: &WorldDoc,
    px: f32,
    pz: f32,
    reach: f32,
    pred: impl Fn(&WorldProp) -> bool,
) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, p) in doc.props.iter().enumerate() {
        if !p.enabled || !pred(p) {
            continue;
        }
        let d = (p.position[0] - px).hypot(p.position[2] - pz);
        if d <= reach && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

fn nearest_xz(
    doc: &WorldDoc,
    px: f32,
    pz: f32,
    pred: impl Fn(&WorldProp) -> bool,
) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32, f32)> = None;
    for p in &doc.props {
        if !p.enabled || !pred(p) {
            continue;
        }
        let d = (p.position[0] - px).hypot(p.position[2] - pz);
        if best.is_none_or(|(_, _, bd)| d < bd) {
            best = Some((p.position[0], p.position[2], d));
        }
    }
    best.map(|(x, z, _)| (x, z))
}

/// Stick relative to `yaw`. `arrived` is true inside `stop` meters.
fn steer_toward(px: f32, pz: f32, target: (f32, f32), yaw: f32, stop: f32) -> (f32, f32, bool) {
    let dx = target.0 - px;
    let dz = target.1 - pz;
    let dist = dx.hypot(dz);
    if dist <= stop {
        return (0.0, 0.0, true);
    }
    let (s, c) = yaw.sin_cos();
    let inv = 1.0 / dist;
    let dir_x = dx * inv;
    let dir_z = dz * inv;
    let lx = (dir_x * -c + dir_z * s).clamp(-1.0, 1.0);
    let lz = (dir_x * s + dir_z * c).clamp(-1.0, 1.0);
    (lx, lz, false)
}

fn night_amount(frac: f32) -> f32 {
    let dusk = (NIGHT_AT - 0.08).max(0.0);
    let dawn = 0.94;
    if frac < dusk {
        0.0
    } else if frac < NIGHT_AT {
        (frac - dusk) / (NIGHT_AT - dusk)
    } else if frac < dawn {
        1.0
    } else {
        ((1.0 - frac) / (1.0 - dawn)).clamp(0.0, 1.0)
    }
}

fn mix_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8
}

fn mix_sky(t: f32) -> [u8; 4] {
    [
        mix_u8(DAY_SKY[0], NIGHT_SKY[0], t),
        mix_u8(DAY_SKY[1], NIGHT_SKY[1], t),
        mix_u8(DAY_SKY[2], NIGHT_SKY[2], t),
        255,
    ]
}

fn mix_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn paint_light(
    doc: &mut WorldDoc,
    slot: u32,
    name: &str,
    position: [f32; 3],
    intensity: f32,
    radius: f32,
    color: [f32; 3],
) {
    if let Some(light) = doc.lights.iter_mut().find(|l| l.slot == slot) {
        light.name = name.into();
        light.position = position;
        light.kind = "point".into();
        light.intensity = intensity;
        light.radius = radius;
        light.color = Some(color);
        return;
    }
    doc.lights.push(WorldLight {
        id: format!("light:{slot}"),
        kind_type: "light".into(),
        name: name.into(),
        position,
        kind: "point".into(),
        slot,
        intensity,
        radius,
        color: Some(color),
        direction: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const CAMP: &str = include_str!("../tests/fixtures/survival_meter_world.json");
    const CLEARING: &str = include_str!("../tests/fixtures/survival_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const TOWN: &str = include_str!("../tests/fixtures/rpg_town_world.json");
    const PAGES: &str = include_str!("../tests/fixtures/novel_pages_world.json");
    const HIDE: &str = include_str!("../tests/fixtures/stealth_hide_world.json");
    const RING: &str = include_str!("../tests/fixtures/fight_hitstun_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");
    const ROOM: &str = include_str!("../tests/fixtures/puzzle_pad_world.json");
    const HOP: &str = include_str!("../tests/fixtures/box_hop_world.json");
    const PITCH: &str = include_str!("../tests/fixtures/sports_goal_world.json");
    const YARD: &str = include_str!("../tests/fixtures/sim_meter_world.json");
    const SIDE: &str = include_str!("../tests/fixtures/action_side_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(CAMP).unwrap();
        play.confirm();
        play
    }

    fn put_player(play: &mut WorldPlay, x: f32, z: f32) {
        let y = play.doc.height_at(x, z) + BODY_H;
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [x, y, z];
        }
        let walker = play.doc.player.clone().unwrap();
        write_player(&mut play.doc, walker);
    }

    #[test]
    fn dump_is_survival_not_other_genres() {
        let doc = WorldDoc::from_json(CAMP).unwrap();
        assert!(is_survival(&doc));
        assert_eq!(GAME_ID, "survival_meter");
        assert!(!crate::sim::is_sim(&doc));
        assert!(!crate::puzzle::is_puzzle(&doc));
        assert!(!crate::stealth::is_stealth(&doc));
        assert!(!crate::novel::is_novel(&doc));
        assert!(!crate::rpg::is_rpg(&doc));
        assert!(!crate::fight::is_fight(&doc));
        assert!(!crate::action::is_action(&doc));
        assert!(!crate::action2d::is_action2d(&doc));
        assert!(!crate::fps::is_fps(&doc));
        assert!(!crate::td::is_td(&doc));
        assert!(!crate::race::is_race(&doc));
        assert!(!crate::platformer::is_platformer(&doc));
        assert!(!crate::sports::is_sports(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_survival(&crest));
        let yard = WorldDoc::from_json(YARD).unwrap();
        assert!(crate::sim::is_sim(&yard));
        assert!(!is_survival(&yard), "sim zone must not count as survival");
        let side = WorldDoc::from_json(SIDE).unwrap();
        assert!(crate::action2d::is_action2d(&side));
        assert!(!is_survival(&side));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "camp" && p.model == "box"));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "ration" && p.model == "box" && p.enabled));
        assert!(doc.props.iter().any(|p| p.name == "flag" && !p.enabled));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "floor" && p.model == "box"));
        let camp = doc.props.iter().find(|p| p.name == "camp").unwrap();
        assert!(
            camp.metallic >= 0.5,
            "camp rim should read metal GGX, metallic={}",
            camp.metallic
        );
        let ration = doc.props.iter().find(|p| p.name == "ration").unwrap();
        assert!(
            ration.metallic >= 0.5,
            "ration should read metal GGX, metallic={}",
            ration.metallic
        );
        assert_eq!(doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("camp"));
        assert!(json.contains("ration"));
        assert!(json.contains("flag"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 4,
            "floor + camp + ration must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(CAMP).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_survival());
        assert!(!play.is_sim());
        assert!(!play.is_action2d());
        assert!(!play.is_sports());
        assert!(!play.is_collectathon());
        let start = play.doc.player.as_ref().unwrap().position;
        let start_coins = play.doc.coins;
        play.input.lx = 1.0;
        play.input.lz = 1.0;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.survival.done, "title must not drain");
        assert_eq!(play.doc.player.as_ref().unwrap().position, start);
        assert_eq!(play.doc.coins, start_coins);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "chase");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn idle_ticks_down_then_starve() {
        let mut play = play_started();
        assert_eq!(play.doc.coins, NEED);
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.survival.done);
        assert!(
            play.doc.coins < NEED,
            "meter should drain, coins={}",
            play.doc.coins
        );
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        for _ in 0..240 {
            play.tick(1.0 / 60.0);
            if play.survival.done {
                break;
            }
        }
        assert!(play.survival.starve);
        assert!(play.survival.done);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_STARVE);
        assert_eq!(play.doc.coins, 0);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("starve"), "starve must be dump-visible");
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "starve does not raise the ok flag");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "starve overlay");
    }

    #[test]
    fn stand_in_camp_fills_to_ok() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 6.5);
        for _ in 0..60 {
            play.tick(1.0 / 60.0);
        }
        assert!(play.doc.coins < NEED, "drain away from camp first");
        let camp = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "camp")
            .unwrap()
            .position;
        put_player(&mut play, camp[0], camp[2]);
        for _ in 0..180 {
            play.tick(1.0 / 60.0);
            if play.survival.done {
                break;
            }
        }
        assert!(play.survival.ok);
        assert!(play.survival.done);
        assert!(!play.survival.starve);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_OK);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "ok flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_OK_COLOR));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("ok"), "ok must be dump-visible");
        assert_eq!(play.doc.coins, NEED);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "ok overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.survival.done);
        assert!(!play.survival.ok);
        assert!(!play.survival.starve);
        assert_eq!(play.doc.coins, NEED);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "retry restores flag off");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        let ration = play.doc.props.iter().find(|p| p.name == "ration").unwrap();
        assert!(ration.enabled, "retry restores ration");
    }

    #[test]
    fn pick_ration_fills_meter() {
        let mut play = play_started();
        put_player(&mut play, 0.0, 6.5);
        for _ in 0..90 {
            play.tick(1.0 / 60.0);
        }
        let before = play.doc.coins;
        assert!(
            before <= NEED - 2,
            "need room to see a pick fill, coins={before}"
        );
        let ration = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "ration")
            .unwrap()
            .position;
        put_player(&mut play, ration[0], ration[2]);
        play.tick(1.0 / 60.0);
        assert!(play.survival.picked);
        let ration = play.doc.props.iter().find(|p| p.name == "ration").unwrap();
        assert!(!ration.enabled, "picked ration disables in the dump");
        assert!(
            play.doc.coins > before,
            "pick should fill, before={before} after={}",
            play.doc.coins
        );
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("filling") || dump.contains("ok"));
    }

    #[test]
    fn walk_forward_oks_from_spawn() {
        let mut play = play_started();
        let cam0 = play.doc.cameras[0].position;
        play.input.lz = 1.0;
        for _ in 0..300 {
            play.tick(1.0 / 60.0);
            if play.survival.done {
                break;
            }
        }
        assert!(
            play.survival.ok && play.survival.done,
            "W from spawn should pick/camp and ok, starve={} filling={} coins={} pos={:?}",
            play.survival.starve,
            play.survival.filling,
            play.doc.coins,
            play.doc.player.as_ref().unwrap().position
        );
        assert!(!play.survival.starve);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_OK);
        assert_eq!(play.doc.cameras[0].name, "chase");
        let cam1 = play.doc.cameras[0].position;
        let d = (cam1[0] - cam0[0]).abs() + (cam1[2] - cam0[2]).abs();
        assert!(d > 0.2, "camera should follow play, delta={d}");
    }

    #[test]
    fn other_genres_still_own_their_dumps() {
        let town = WorldPlay::from_json(TOWN).unwrap();
        assert!(town.is_rpg());
        assert!(!town.is_survival());
        let novel = WorldPlay::from_json(PAGES).unwrap();
        assert!(novel.is_novel());
        assert!(!novel.is_survival());
        let stealth = WorldPlay::from_json(HIDE).unwrap();
        assert!(stealth.is_stealth());
        assert!(!stealth.is_survival());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_survival());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_survival());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_survival());
        let puzzle = WorldPlay::from_json(ROOM).unwrap();
        assert!(puzzle.is_puzzle());
        assert!(!puzzle.is_survival());
        let hop = WorldPlay::from_json(HOP).unwrap();
        assert!(hop.is_platformer());
        assert!(!hop.is_survival());
        let sports = WorldPlay::from_json(PITCH).unwrap();
        assert!(sports.is_sports());
        assert!(!sports.is_survival());
        let sim = WorldPlay::from_json(YARD).unwrap();
        assert!(sim.is_sim());
        assert!(!sim.is_survival());
        let side = WorldPlay::from_json(SIDE).unwrap();
        assert!(side.is_action2d());
        assert!(!side.is_survival());
    }

    fn camp_place(s: &mut SurvivalSession, x: f32, z: f32) {
        let y = s.doc.height_at(x, z) + 0.02;
        let mut w = s.doc.player.clone().unwrap();
        w.position = [x, y, z];
        w.on_ground = true;
        write_player(&mut s.doc, w);
    }

    #[test]
    fn camp_dump_is_not_the_meter_slice() {
        let doc = WorldDoc::from_json(CLEARING).unwrap();
        assert!(is_camp(&doc));
        assert!(!is_survival(&doc));
        assert_eq!(CAMP_ID, "survival");
        let meter = WorldDoc::from_json(CAMP).unwrap();
        assert!(is_survival(&meter));
        assert!(!is_camp(&meter));
        let err = SurvivalSession::open(CAMP).unwrap_err();
        assert!(err.contains("survival"));
        let err = SurvivalSession::open(CREST).unwrap_err();
        assert!(err.contains("survival"));
    }

    #[test]
    fn camp_gather_and_craft_fire() {
        let mut s = SurvivalSession::open(CLEARING).unwrap();
        assert_eq!(s.current_phase(), GamePhase::Title);
        s.confirm_action();
        assert_eq!(s.current_phase(), GamePhase::Playing);
        let (tx, tz) = s
            .doc
            .props
            .iter()
            .find(|p| p.name == "tree" && p.enabled)
            .map(|p| (p.position[0], p.position[2]))
            .unwrap();
        camp_place(&mut s, tx, tz);
        s.apply_input(WalkInput {
            attack: true,
            ..WalkInput::default()
        });
        s.step(1.0 / 60.0);
        assert_eq!(s.wood(), 2, "one tree is 2 wood");
        assert!(s.doc.props.iter().any(|p| p.name == "tree" && !p.enabled));
        s.camp.wood = 4;
        let (fx, fz) = camp_anchor(&s.doc);
        camp_place(&mut s, fx, fz);
        s.apply_input(WalkInput {
            dodge: true,
            ..WalkInput::default()
        });
        s.step(1.0 / 60.0);
        assert!(s.fire());
        assert!(s
            .doc
            .props
            .iter()
            .any(|p| p.name == "campfire" && p.enabled));
        assert!(!s.status().contains('\u{2014}'));
        assert!(s.status().contains("fire"));
    }

    #[test]
    fn camp_idle_dies_at_night() {
        let mut s = SurvivalSession::open(CLEARING).unwrap();
        s.confirm_action();
        s.apply_input(WalkInput::default());
        for _ in 0..2200 {
            s.step(1.0 / 60.0);
            if s.dead() {
                break;
            }
        }
        assert!(s.dead() && !s.won(), "status {}", s.status());
        assert_eq!(s.current_phase(), GamePhase::Complete);
        assert!(s.status().starts_with("down"));
    }

    #[test]
    fn camp_autopilot_survives_three_days() {
        let mut s = SurvivalSession::open(CLEARING).unwrap();
        s.confirm_action();
        let mut frames = 0;
        for _ in 0..7000 {
            frames += 1;
            if s.won() || s.dead() {
                break;
            }
            s.apply_input(s.demo_input());
            s.step(1.0 / 60.0);
        }
        let pos = s.doc.player.as_ref().unwrap().position;
        assert!(
            s.won() && !s.dead() && s.fire() && s.shelter(),
            "frames={frames} won={} dead={} day={} t={:.2} night={} hp={:.0} hun={:.0} wood={} food={} fire={} shelter={} pos={pos:?} status={}",
            s.won(),
            s.dead(),
            s.day(),
            s.time_of_day(),
            s.night(),
            s.health(),
            s.hunger(),
            s.wood(),
            s.food(),
            s.fire(),
            s.shelter(),
            s.status(),
        );
        assert!(s.day() > DAYS_TO_WIN);
        assert_eq!(s.status(), "survived 3 days");
        assert!(s.drain_events("gather_wood").iter().any(|e| e.count >= 1));
        assert!(s.drain_events("gather_food").iter().any(|e| e.count >= 1));
        assert!(s.drain_events("craft_fire").iter().any(|e| e.count >= 1));
        assert!(s.drain_events("build_shelter").iter().any(|e| e.count >= 1));
        assert!(s.drain_events("eat").iter().any(|e| e.count >= 1));
        assert_eq!(s.current_phase(), GamePhase::Complete);
        let dump = s.world_json().unwrap();
        assert!(dump.contains("knight"));
        assert!(dump.contains("stylized_tree"));
        assert!(dump.contains("Bonfire"));
        assert!(dump.contains("gazebo"));
    }
}
