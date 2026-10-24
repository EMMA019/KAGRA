//! `GameSpec`: genre-agnostic game data, loaded from JSON.
//!
//! Core (every kit): identity and seed, the player, entity tables (glb +
//! hitbox + weight), hit tuning, camera, neon style, HUD, items, and baked
//! placements. Genre kits add one section; today that is `runner`
//! (`RunnerKit`: speed curve, jump, lanes, forward tracks, spawn-ahead).
//!
//! Python authors the spec (`kagra.game_spec.GameSpec`) and bakes it into the
//! world dump under `"game"`. Rust runs it on desktop, wasm, and Android.
//! Every default is Neon Cat, so `{}` is Neon Cat. `items` and `placements`
//! are reserved: parsed and validated, not simulated yet.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GameSpec {
    /// Which kit runs this spec.
    pub kind: GameKind,
    pub id: String,
    pub title: String,
    /// Seeds spark jitter. Obstacle and ring placement is index-derived.
    pub seed: u32,
    pub player: PlayerSpec,
    /// Entity table for things that end the run on contact.
    pub obstacles: Vec<EntityKind>,
    /// Shrink each kill box from the measured mesh so a graze survives.
    pub hit_inset: f32,
    /// Fraction of the mesh height a foot must clear.
    pub hit_top: f32,
    pub camera: CameraSpec,
    pub style: Style,
    /// HUD score panel top, in HUD pixels.
    pub hud_top: f32,
    /// In-play HUD look and the result screen (death wording, local ranking).
    pub hud: HudSpec,
    /// How many hits the cat survives, and the grace after one.
    pub lives: LivesSpec,
    /// Pick-ups with a fixed effect vocabulary (`ItemEffect`), placed by
    /// `item_rules` in rest and reward sections.
    pub items: Vec<ItemSpec>,
    pub item_rules: ItemRules,
    /// Knockable props (`mischief_rules` places them in free slots).
    pub mischief: Vec<MischiefSpec>,
    pub mischief_rules: MischiefRules,
    /// A route of ledges the cat can jump up onto and run along, bottom step
    /// first (`perch_rules` places one in free slots; empty = none).
    pub perch_route: Vec<PerchStep>,
    pub perch_rules: PerchRules,
    /// The cleaning robot that gives chase after a hit.
    pub chaser: ChaserSpec,
    /// Reserved. Seeded chunk placement baked by Python. Not simulated yet.
    pub placements: Vec<Placement>,
    /// Runner-kit section (used when `kind` is `runner`).
    pub runner: RunnerKit,
    /// Touch scheme and its thresholds.
    pub input: InputSpec,
    /// Declarative feel presets (see `game_core::feel`).
    pub feel: crate::game_core::feel::FeelSpec,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameKind {
    #[default]
    Runner,
}

/// How touches become movement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputScheme {
    /// Hold and drag sideways to steer; flick up/down to jump/roll.
    #[default]
    Drag,
    /// One swipe = one lane (with a cooldown), up/down as in drag.
    Swipe,
}

/// Touch input. Distances are fractions of the short screen side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InputSpec {
    pub scheme: InputScheme,
    /// Drag: finger travel that moves the cat one lane.
    pub drag_lane_frac: f32,
    /// Drag: smoothing time constant of the cat following the finger.
    pub drag_smooth_secs: f32,
    /// Drag: on release, ease to the nearest lane centre.
    pub drag_snap: bool,
    /// Flick: vertical travel that counts as a jump/roll flick.
    pub flick_dist: f32,
    /// Flick: max angle from vertical, degrees.
    pub flick_angle_deg: f32,
    /// Swipe scheme: lateral travel for one lane.
    pub swipe_dist: f32,
    /// Swipe scheme: no second lateral swipe in the same touch for this long.
    pub swipe_cooldown_secs: f32,
    /// Jump/roll presses wait this long for the cat to act on them.
    pub buffer_secs: f32,
    /// An up flick this soon after a down flick is the finger coming back,
    /// not a jump.
    pub reverse_guard_secs: f32,
}

impl Default for InputSpec {
    fn default() -> Self {
        Self {
            scheme: InputScheme::Drag,
            drag_lane_frac: 0.18,
            drag_smooth_secs: 0.05,
            drag_snap: true,
            flick_dist: 0.07,
            flick_angle_deg: 30.0,
            swipe_dist: 0.09,
            swipe_cooldown_secs: 0.3,
            buffer_secs: 0.15,
            reverse_guard_secs: 0.2,
        }
    }
}

/// Runner kit: forward scroll, lanes, and spawn-ahead.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RunnerKit {
    pub speed: SpeedCurve,
    pub jump: JumpSpec,
    pub lanes: LaneSpec,
    pub obstacle_track: Track,
    pub rings: RingSpec,
    pub gates: GateSpec,
    pub draw: DrawSpec,
    /// Optional pacing script over distance. Empty = one uniform stream.
    pub sections: Vec<Section>,
    /// The cat's body: standing and rolling height, roll length, step tolerance.
    pub body: BodySpec,
    /// Coin lines.
    pub coins: CoinSpec,
    /// How a run turns into score: distance and gates. Rings and coins carry
    /// their own `score`.
    pub score: ScoreSpec,
}

/// Score formula: `per_meter * distance + per_gate * gates + rings + coins`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScoreSpec {
    /// Points per metre run.
    pub per_meter: f32,
    /// Bonus for each stone arch (gate) the cat runs through.
    pub per_gate: u32,
}

impl Default for ScoreSpec {
    fn default() -> Self {
        Self {
            per_meter: 2.2,
            per_gate: 50,
        }
    }
}

/// The in-play HUD panels and what the result screen shows. Game-agnostic:
/// another kit fills the same fields and gets the same panels and ranking.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HudSpec {
    /// Size multiplier on top of the screen-width scale.
    pub scale: f32,
    pub panel: PanelSpec,
    pub result: ResultSpec,
}

impl Default for HudSpec {
    fn default() -> Self {
        Self {
            scale: 1.0,
            panel: PanelSpec::default(),
            result: ResultSpec::default(),
        }
    }
}

/// A 9-slice panel: rim colour, fill colour, and the number colour of coins.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PanelSpec {
    pub fill: [u8; 4],
    pub border: [u8; 4],
    /// Coin icon and coin count.
    pub coin: [u8; 4],
    /// Score number.
    pub score: [u8; 4],
    /// Rim and corner size in HUD units (before scaling).
    pub slice: f32,
}

impl Default for PanelSpec {
    fn default() -> Self {
        Self {
            fill: [14, 10, 32, 205],
            border: [90, 210, 230, 255],
            coin: [255, 205, 70, 255],
            score: [190, 248, 255, 255],
            slice: 3.0,
        }
    }
}

/// Columns the ranking can show. The sim reports each one by this name.
pub const BOARD_COLUMNS: [&str; 4] = ["time", "gates", "coins", "score"];

/// What the game-over screen shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResultSpec {
    /// Seconds the crash plays out (shake, flash, sparks) before the result.
    pub beat_secs: f32,
    pub board: BoardSpec,
    /// Death reason (the obstacle name) -> the line shown to the player.
    pub deaths: std::collections::BTreeMap<String, String>,
    /// Line for a reason `deaths` does not name.
    pub death_fallback: String,
}

impl Default for ResultSpec {
    fn default() -> Self {
        let deaths = [
            ("CRATE", "木箱にぶつかった"),
            ("BARREL", "樽にぶつかった"),
            ("BARRIER", "バリケードにぶつかった"),
            ("GENERATOR", "発電機にぶつかった"),
            ("OVERPASS", "ひさしに頭をぶつけた（下フリックでころがる）"),
            ("TRAIN", "電車にぶつかった"),
            ("WALL", "壁にぶつかった（すき間は下フリックでくぐる）"),
            ("METEOR", "隕石にぶつかった"),
            ("CRYSTALS", "クリスタルにぶつかった"),
            ("ROCK", "岩にぶつかった"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        Self {
            beat_secs: 0.7,
            board: BoardSpec::default(),
            deaths,
            death_fallback: "ぶつかった".into(),
        }
    }
}

/// The local top-N list. Saved on the device only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BoardSpec {
    pub enabled: bool,
    /// Rows kept and shown (1..=10).
    pub top_n: u32,
    /// Any of [`BOARD_COLUMNS`], in the order shown.
    pub columns: Vec<String>,
    /// Which column ranks the list first. One of `columns`.
    pub sort: String,
}

impl HudSpec {
    fn validate(&self) -> Result<(), String> {
        let r = &self.result;
        if !(self.scale > 0.0 && self.scale <= 4.0) {
            return Err("game spec: hud.scale must be in (0, 4]".into());
        }
        if !(0.0..=3.0).contains(&r.beat_secs) {
            return Err("game spec: hud.result.beat_secs must be 0..=3".into());
        }
        let b = &r.board;
        if !(1..=10).contains(&b.top_n) {
            return Err("game spec: hud.result.board.top_n must be 1..=10".into());
        }
        if b.columns.is_empty() {
            return Err("game spec: hud.result.board.columns must not be empty".into());
        }
        for c in &b.columns {
            if !BOARD_COLUMNS.contains(&c.as_str()) {
                return Err(format!(
                    "game spec: hud.result.board.columns: unknown {c:?} (use {BOARD_COLUMNS:?})"
                ));
            }
        }
        if !b.columns.contains(&b.sort) {
            return Err("game spec: hud.result.board.sort must be one of its columns".into());
        }
        Ok(())
    }
}

impl Default for BoardSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            top_n: 5,
            columns: BOARD_COLUMNS.iter().map(|c| c.to_string()).collect(),
            sort: "score".into(),
        }
    }
}

/// The cat's body for collision. Heights are metres above the feet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BodySpec {
    /// Standing height. Hits anything whose underside is below this.
    pub stand_height: f32,
    /// Height while rolling / sliding. The low hitbox.
    pub roll_height: f32,
    /// How long one roll lasts.
    pub roll_secs: f32,
    /// A rideable surface this far above the feet is a wall, not a step.
    pub step_tol: f32,
    /// After a roll the cat stands back up over this long. The body and the
    /// pose rise together, so the recovery is visible and fair.
    pub recover_secs: f32,
    /// Pose scale `[x, y, z]` at full crouch. A lowered, stretched body, not
    /// a flat squash. Kept under the opening height of a gap.
    pub crouch: [f32; 3],
}

impl Default for BodySpec {
    fn default() -> Self {
        Self {
            stand_height: 0.62,
            roll_height: 0.3,
            roll_secs: 0.55,
            step_tol: 0.18,
            recover_secs: 0.2,
            crouch: [1.05, 0.68, 1.2],
        }
    }
}

/// Coin lines: arcs over jumpable obstacles, low lines through gaps and
/// under beams, a trail along a rideable top, and weaving lines in the
/// empty slots of rest and reward sections.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CoinSpec {
    pub enabled: bool,
    pub glb: String,
    pub score: u32,
    /// Coins in an obstacle's line.
    pub per_line: u32,
    /// Coins in a free slot of a rest/reward section.
    pub free_line: u32,
    /// Metres between coins along z.
    pub spacing: f32,
    /// Height of an arc's top above its ends.
    pub arc_height: f32,
}

impl Default for CoinSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            glb: "assets/models/coin.glb".into(),
            score: 10,
            per_line: 5,
            free_line: 8,
            spacing: 0.85,
            arc_height: 0.85,
        }
    }
}

/// What a stretch of track is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SectionKind {
    /// Gentle start: few obstacles.
    Intro,
    /// Introduces a mechanic at low pressure.
    Learn,
    /// Full density.
    Challenge,
    /// No obstacles; rings only. Lets the player breathe.
    Rest,
    /// Payoff after a challenge. No obstacles.
    Reward,
}

impl SectionKind {
    /// Share of obstacle slots kept when `density` is not set.
    pub fn default_density(self) -> f32 {
        match self {
            SectionKind::Intro => 0.35,
            SectionKind::Learn => 0.7,
            SectionKind::Challenge => 1.0,
            SectionKind::Rest | SectionKind::Reward => 0.0,
        }
    }
}

/// One stretch of the course, in metres of travel. The list repeats.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Section {
    pub kind: SectionKind,
    /// Length along z in metres.
    pub length: f32,
    /// Declared difficulty 0..=5, checked by the placement validator.
    pub level: u8,
    /// Fraction of obstacle slots kept (0..=1). `None` = the kind's default.
    pub density: Option<f32>,
    /// Obstacle behaviours this stretch may use. Empty = all. This is how a
    /// learn section teaches one mechanic before the challenge mixes them.
    pub allow: Vec<Behavior>,
}

impl Default for Section {
    fn default() -> Self {
        Self {
            kind: SectionKind::Challenge,
            length: 120.0,
            level: 3,
            density: None,
            allow: Vec::new(),
        }
    }
}

impl Section {
    pub fn density(&self) -> f32 {
        self.density.unwrap_or_else(|| self.kind.default_density())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpeedCurve {
    /// m/s at t = 0.
    pub start: f32,
    /// m/s gained per second of play.
    pub ramp: f32,
    pub cap: f32,
    /// Longest z move per sim substep (anti-tunnelling).
    pub max_step: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JumpSpec {
    pub gravity: f32,
    pub velocity: f32,
    /// No obstacle kills for this long after start.
    pub grace_secs: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LaneSpec {
    /// Lane count for baked placements. The default track is continuous.
    pub count: u32,
    /// Lane spacing in metres.
    pub width: f32,
    /// The cat's |x| clamp (the side rails).
    pub limit: f32,
    /// Steering speed in m/s at full stick.
    pub steer: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlayerSpec {
    pub glb: String,
    pub scale: f32,
    /// Model-space foot offset (model units).
    pub foot: f32,
    /// sRGB multiply on the albedo.
    pub tint: [u32; 3],
    /// The two coloured side lights that edge the cat against the dark.
    pub rim: RimSpec,
    /// Neon eyes on the face.
    pub eyes: EyesSpec,
}

/// Nine lives: a hit costs one, the cat blinks and cannot be hit again for
/// `invuln_secs`, and the last life ends the run as before. `count` 1 is the
/// old one-hit rule (the geometry measurements run that way).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LivesSpec {
    /// 1..=9 (the HUD paw row has nine slots). The default is 3.
    pub count: u32,
    /// Seconds without damage after a hit.
    pub invuln_secs: f32,
    /// Blinks per second while protected.
    pub blink_hz: f32,
}

impl Default for LivesSpec {
    fn default() -> Self {
        Self {
            count: 3,
            invuln_secs: 1.6,
            blink_hz: 10.0,
        }
    }
}

impl LivesSpec {
    pub const MAX: u32 = 9;

    fn validate(&self) -> Result<(), String> {
        if !(1..=Self::MAX).contains(&self.count) {
            return Err(format!("game spec: lives.count must be 1..={}", Self::MAX));
        }
        if !(0.0..=10.0).contains(&self.invuln_secs) || !self.invuln_secs.is_finite() {
            return Err("game spec: lives.invuln_secs must be 0..=10".into());
        }
        if !(0.0..=30.0).contains(&self.blink_hz) || !self.blink_hz.is_finite() {
            return Err("game spec: lives.blink_hz must be 0..=30".into());
        }
        Ok(())
    }
}

/// Two point lights (screen-left, screen-right) just outside the cat's flanks so its side faces
/// catch a coloured edge (linear RGB). They have to stay outside the body: a
/// light inside a closed mesh lights nothing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RimSpec {
    pub left: [f32; 3],
    pub right: [f32; 3],
    pub intensity: f32,
    pub radius: f32,
    /// Distance of each light from the cat's centre line, in metres.
    pub side: f32,
    /// Height above the feet.
    pub height: f32,
}

impl Default for RimSpec {
    fn default() -> Self {
        Self {
            left: [1.0, 0.25, 0.75],
            right: [0.3, 0.9, 1.0],
            intensity: 3.6,
            radius: 0.55,
            side: 0.5,
            height: 0.5,
        }
    }
}

/// The cat's glow marks: two eyes on the face, plus ear glints, a collar
/// and a tail tip that read from behind. Each is its own switch. Offsets are in model
/// units (like `foot`) and follow the cat's scale and pose.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EyesSpec {
    pub enabled: bool,
    /// Linear RGB of the glow.
    pub color: [f32; 3],
    /// Emissive strength (bloom picks it up above ~1).
    pub glow: f32,
    /// Eye width in model units.
    pub size: f32,
    /// Half the gap between the eyes.
    pub spread: f32,
    /// Height above the feet.
    pub height: f32,
    /// Distance forward of the model origin (the face plane).
    pub forward: f32,
    /// Glints on the ear tips, so the cat reads from behind (the chase view).
    pub ears: bool,
    /// Ear glint offsets in model units: sideways from the middle, height
    /// above the feet, and forward of the model origin.
    pub ear_x: f32,
    pub ear_y: f32,
    pub ear_z: f32,
    /// A neon band around the body at collar height (model units), seen
    /// from behind as a bright line across the back and flanks.
    pub collar: bool,
    pub collar_color: [f32; 3],
    pub collar_y: f32,
    /// A glint on the tail tip: sideways, height above the feet, and
    /// forward of the model origin (negative is behind the cat).
    pub tail_tip: bool,
    pub tail_color: [f32; 3],
    pub tail_x: f32,
    pub tail_y: f32,
    pub tail_z: f32,
}

impl Default for EyesSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            color: [0.35, 1.0, 0.95],
            glow: 2.4,
            size: 0.13,
            spread: 0.24,
            height: 0.92,
            forward: 0.64,
            ears: true,
            ear_x: 0.4,
            ear_y: 1.5,
            ear_z: 0.05,
            collar: false,
            collar_color: [1.0, 0.25, 0.75],
            collar_y: 0.95,
            tail_tip: true,
            tail_color: [1.0, 0.25, 0.75],
            tail_x: 0.0,
            tail_y: 1.62,
            tail_z: -0.6,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EntityKind {
    /// Shown as `HIT <name>` on the result screen.
    pub name: String,
    pub glb: String,
    pub scale: [f32; 3],
    /// Mesh AABB in model space (from the GLB accessors).
    pub mesh_min: [f32; 3],
    pub mesh_max: [f32; 3],
    /// Integer weight in the deterministic spawn cycle.
    pub weight: u32,
    /// What the player must do about it. Default `jump_over` is the
    /// original crate rule.
    pub behavior: Behavior,
    /// More meshes that make up this obstacle (a ramp, a lintel, a wall
    /// side), each placed relative to the anchor.
    pub parts: Vec<Part>,
    /// Model-space y range of the base mesh that is solid (a roof slab of an
    /// open mesh). `None` = the whole mesh.
    pub hit_y: Option<[f32; 2]>,
    /// Neon edge colour override. `None` = the colour code of its behavior
    /// (`style.codes`).
    pub color: Option<[u8; 3]>,
}

/// What an obstacle asks of the player.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Behavior {
    /// Too tall to jump, wide enough to need a lane change.
    Solid,
    /// Low enough to hop (or to steer around).
    #[default]
    JumpOver,
    /// An overhead beam: roll under it. Spans every lane.
    RollUnder,
    /// A flat top the cat can run on, reached by a ramp or a jump.
    Rideable,
    /// A wall across every lane with one low opening only a rolling cat fits.
    CatGap,
}

impl Behavior {
    pub fn name(self) -> &'static str {
        match self {
            Behavior::Solid => "solid",
            Behavior::JumpOver => "jump_over",
            Behavior::RollUnder => "roll_under",
            Behavior::Rideable => "rideable",
            Behavior::CatGap => "cat_gap",
        }
    }
}

/// How a part acts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartRole {
    /// Kills on contact (the default).
    #[default]
    Solid,
    /// A flat top the cat can stand on; its sides kill.
    Ride,
    /// An incline up to a `Ride` top. Never kills.
    Ramp,
    /// Drawn only.
    Decor,
}

/// One mesh of a multi-mesh obstacle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Part {
    pub glb: String,
    pub scale: [f32; 3],
    pub mesh_min: [f32; 3],
    pub mesh_max: [f32; 3],
    /// Mesh centre in x/z relative to the anchor; y is the height of the
    /// mesh's lowest point above the floor.
    pub offset: [f32; 3],
    pub role: PartRole,
    /// Model-space y range that is solid (a roof slab of an open mesh).
    /// `None` = the whole mesh.
    pub hit_y: Option<[f32; 2]>,
    /// Ramp only: horizontal length of the incline from the mesh's back edge.
    pub run: f32,
    /// Emissive strength for a decor part that should shine (a lit opening,
    /// a neon bar). 0 = the ordinary dim fill.
    pub glow: f32,
}

impl Default for Part {
    fn default() -> Self {
        Self {
            glb: String::new(),
            scale: [1.0, 1.0, 1.0],
            mesh_min: [-0.5, 0.0, -0.5],
            mesh_max: [0.5, 1.0, 0.5],
            offset: [0.0, 0.0, 0.0],
            role: PartRole::Solid,
            hit_y: None,
            run: 0.0,
            glow: 0.0,
        }
    }
}

/// Index-derived track: `z = z0 + i * gap`, `x = sin(i * freq) * amp`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Track {
    pub z0: f32,
    pub gap: f32,
    pub amp: f32,
    pub freq: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RingSpec {
    pub glb: String,
    pub track: Track,
    pub score: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GateSpec {
    pub glb: String,
    pub z0: f32,
    pub gap: f32,
    /// Keep the pillars outside `lanes.limit`.
    pub scale_x: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DrawSpec {
    /// Seconds of travel drawn ahead of the cat.
    pub lead_secs: f32,
    pub min_obstacles_ahead: i32,
    pub fog_start: f32,
    pub fog_end: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CameraSpec {
    pub chase_back: f32,
    pub chase_lift: f32,
    pub chase_look: f32,
    pub chase_look_y: f32,
    pub fov: f32,
    pub fov_fast: f32,
    pub roll: f32,
    pub classic_back: f32,
    pub classic_lift: f32,
    pub classic_fov: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Style {
    pub sky: [u8; 4],
    /// Obstacle edge colours, alternating by index.
    pub edge_a: [u8; 4],
    pub edge_b: [u8; 4],
    /// Obstacle fill glow colours (linear), matching `edge_a` / `edge_b`.
    pub fill_a: [f32; 3],
    pub fill_b: [f32; 3],
    pub fill_emissive: f32,
    pub obstacle_color: [u32; 3],
    /// One neon colour per thing the player has to do, so a shape reads
    /// before it can be recognised. Used for the obstacle edge and its fill
    /// (`edge_a` / `edge_b` / `fill_a` / `fill_b` are older alternating tints
    /// that obstacles no longer use).
    pub codes: ColorCodes,
}

/// The colour language of the course.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ColorCodes {
    /// Low: hop it.
    pub jump_over: [u8; 3],
    /// High or hanging: roll under it.
    pub roll_under: [u8; 3],
    /// A wall: go round.
    pub solid: [u8; 3],
    /// A low opening in a wall: roll through.
    pub cat_gap: [u8; 3],
    /// A roof: ramp up and run on it.
    pub rideable: [u8; 3],
    /// Gates and arches the cat passes through. Friendly, never a threat.
    pub pass_through: [u8; 3],
}

impl Default for ColorCodes {
    fn default() -> Self {
        Self {
            jump_over: [255, 170, 40],
            roll_under: [255, 48, 186],
            solid: [255, 60, 70],
            cat_gap: [176, 110, 255],
            rideable: [60, 150, 255],
            pass_through: [150, 255, 225],
        }
    }
}

impl ColorCodes {
    pub fn for_behavior(&self, b: Behavior) -> [u8; 3] {
        match b {
            Behavior::JumpOver => self.jump_over,
            Behavior::RollUnder => self.roll_under,
            Behavior::Solid => self.solid,
            Behavior::CatGap => self.cat_gap,
            Behavior::Rideable => self.rideable,
        }
    }
}

impl EntityKind {
    /// Neon edge colour: the kind's own, else its behavior's code.
    pub fn edge_color(&self, codes: &ColorCodes) -> [u8; 3] {
        self.color
            .unwrap_or_else(|| codes.for_behavior(self.behavior))
    }
}

/// Fixed effect vocabulary. New effects need engine code, so the set is closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemEffect {
    /// Coins within reach fly to the cat.
    Magnet,
    /// Faster and nothing can hurt the cat.
    Boost,
    /// Jumps are much higher.
    Superjump,
    /// Everything scores twice.
    Double,
    /// Instant: one life back (score when the row is full).
    Meow,
}

/// The colour of a coin in the world and on the HUD chip.
pub const COIN_COLOR: [u8; 3] = [255, 214, 90];

/// The picture an orb shows, built from primitives. A closed set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemIcon {
    /// A U shape with pale tips.
    Horseshoe,
    /// Two chevrons pointing ahead.
    Chevron,
    /// An arrow pointing up.
    Arrow,
    /// The letters 2 and x.
    TwoX,
    /// A heart.
    Heart,
}

impl ItemIcon {
    pub const ALL: [ItemIcon; 5] = [
        ItemIcon::Horseshoe,
        ItemIcon::Chevron,
        ItemIcon::Arrow,
        ItemIcon::TwoX,
        ItemIcon::Heart,
    ];
}

impl ItemEffect {
    /// The orb colour when the spec leaves `color` at 0,0,0. Chosen to be far
    /// from each other, from the coin yellow and from the obstacle codes.
    pub fn default_color(self) -> [u8; 3] {
        match self {
            ItemEffect::Magnet => [255, 0, 128],
            ItemEffect::Boost => [255, 110, 0],
            ItemEffect::Superjump => [100, 255, 60],
            ItemEffect::Double => [255, 240, 0],
            ItemEffect::Meow => [255, 200, 235],
        }
    }

    pub fn default_icon(self) -> ItemIcon {
        match self {
            ItemEffect::Magnet => ItemIcon::Horseshoe,
            ItemEffect::Boost => ItemIcon::Chevron,
            ItemEffect::Superjump => ItemIcon::Arrow,
            ItemEffect::Double => ItemIcon::TwoX,
            ItemEffect::Meow => ItemIcon::Heart,
        }
    }

    pub const ALL: [ItemEffect; 5] = [
        ItemEffect::Magnet,
        ItemEffect::Boost,
        ItemEffect::Superjump,
        ItemEffect::Double,
        ItemEffect::Meow,
    ];

    /// Seconds an effect lasts when the spec leaves `duration` at 0.
    pub fn default_secs(self) -> f32 {
        match self {
            ItemEffect::Magnet => 8.0,
            ItemEffect::Boost => 3.5,
            ItemEffect::Superjump => 10.0,
            ItemEffect::Double => 10.0,
            ItemEffect::Meow => 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemSpec {
    pub effect: ItemEffect,
    #[serde(default)]
    pub glb: String,
    #[serde(default = "one")]
    pub weight: u32,
    #[serde(default)]
    pub duration: f32,
    /// Orb, toast, label and floor-ring colour. 0,0,0 = the effect's own.
    #[serde(default)]
    pub color: [u8; 3],
    /// The orb's picture. None = the effect's own.
    #[serde(default)]
    pub icon: Option<ItemIcon>,
}

fn one() -> u32 {
    1
}

impl ItemSpec {
    pub fn new(effect: ItemEffect, weight: u32) -> Self {
        Self {
            effect,
            glb: String::new(),
            weight,
            duration: 0.0,
            color: [0; 3],
            icon: None,
        }
    }

    /// The colour in use.
    pub fn rgb(&self) -> [u8; 3] {
        if self.color == [0; 3] {
            self.effect.default_color()
        } else {
            self.color
        }
    }

    pub fn shape(&self) -> ItemIcon {
        self.icon.unwrap_or_else(|| self.effect.default_icon())
    }

    /// Seconds the effect lasts.
    pub fn secs(&self) -> f32 {
        if self.duration > 0.0 {
            self.duration
        } else {
            self.effect.default_secs()
        }
    }
}

/// Where pick-ups appear: in the free slots of rest and reward sections,
/// halfway between two obstacle slots (clear of the coin line).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ItemRules {
    /// One pick-up every n-th free slot in a rest section (0 = none).
    pub rest_every: u32,
    /// The same for a reward section.
    pub reward_every: u32,
    /// Height of the pick-up above the track.
    pub hover: f32,
    /// Pick-up radius in metres.
    pub radius: f32,
    /// A magnet reaches coins within this many metres (sideways and ahead).
    pub magnet_radius: f32,
    /// The field also reaches this many metres further ahead of the cat, so
    /// coins start flying before the cat is on top of them.
    pub magnet_lead: f32,
    /// A pulled coin starts at this speed (m/s) and speeds up by
    /// `pull_accel` (m/s^2) until it lands on the cat.
    pub pull_speed: f32,
    pub pull_accel: f32,
    /// An effect blinks (label, bar, rings) for this many seconds before it ends.
    pub warn_secs: f32,
    /// Boost speed multiplier.
    pub boost_speed: f32,
    /// Super-jump launch speed multiplier.
    pub jump_scale: f32,
}

impl Default for ItemRules {
    fn default() -> Self {
        Self {
            rest_every: 3,
            reward_every: 2,
            hover: 0.7,
            radius: 1.1,
            magnet_radius: 4.0,
            magnet_lead: 4.0,
            pull_speed: 6.0,
            pull_accel: 70.0,
            warn_secs: 1.5,
            boost_speed: 1.5,
            jump_scale: 1.5,
        }
    }
}

impl ItemRules {
    fn validate(&self) -> Result<(), String> {
        if !(0.2..=3.0).contains(&self.radius) || !(0.0..=3.0).contains(&self.hover) {
            return Err("game spec: item_rules radius 0.2..=3, hover 0..=3".into());
        }
        if !(1.0..=12.0).contains(&self.magnet_radius) {
            return Err("game spec: item_rules.magnet_radius must be 1..=12".into());
        }
        if !(1.0..=2.5).contains(&self.boost_speed) || !(1.0..=2.5).contains(&self.jump_scale) {
            return Err("game spec: item_rules boost_speed and jump_scale must be 1..=2.5".into());
        }
        if !(0.0..=10.0).contains(&self.magnet_lead)
            || !(1.0..=40.0).contains(&self.pull_speed)
            || !(0.0..=400.0).contains(&self.pull_accel)
            || !(0.0..=5.0).contains(&self.warn_secs)
        {
            return Err(
                "game spec: item_rules magnet_lead 0..=10, pull_speed 1..=40, pull_accel 0..=400, warn_secs 0..=5"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Fixed mischief vocabulary: things a cat knocks over. New kinds need art
/// code, so the set is closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MischiefKind {
    /// A tall ceramic vase.
    Vase,
    /// A glass bottle.
    Bottle,
    /// A tin can.
    Can,
    /// A ball of yarn.
    Yarn,
}

impl MischiefKind {
    pub const ALL: [MischiefKind; 4] = [
        MischiefKind::Vase,
        MischiefKind::Bottle,
        MischiefKind::Can,
        MischiefKind::Yarn,
    ];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MischiefSpec {
    pub kind: MischiefKind,
    #[serde(default = "one")]
    pub weight: u32,
}

impl MischiefSpec {
    pub fn new(kind: MischiefKind, weight: u32) -> Self {
        Self { kind, weight }
    }
}

/// Where the knockable props stand (free slots, beside the track centre)
/// and what knocking one is worth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MischiefRules {
    /// One prop every n-th free slot in a rest section (0 = none).
    pub rest_every: u32,
    /// The same for a reward section.
    pub reward_every: u32,
    /// The same for every other section (intro, learn, challenge).
    pub other_every: u32,
    /// Sideways distance from the track centre, either side by hash.
    pub side: f32,
    /// Touch radius sideways in metres.
    pub radius: f32,
    /// Score for one knock.
    pub score: u32,
    /// A knock inside this many seconds of the last one extends the streak.
    pub streak_secs: f32,
    /// Extra score per streak step after the first.
    pub streak_bonus: u32,
    /// The streak stops growing here.
    pub streak_cap: u32,
}

impl Default for MischiefRules {
    fn default() -> Self {
        Self {
            rest_every: 2,
            reward_every: 3,
            other_every: 5,
            side: 1.3,
            radius: 0.75,
            score: 25,
            streak_secs: 2.5,
            streak_bonus: 10,
            streak_cap: 6,
        }
    }
}

impl MischiefRules {
    fn validate(&self) -> Result<(), String> {
        if !(0.0..=2.0).contains(&self.side) || !(0.2..=2.0).contains(&self.radius) {
            return Err("game spec: mischief_rules side 0..=2, radius 0.2..=2".into());
        }
        if !(0.2..=10.0).contains(&self.streak_secs) || !(1..=20).contains(&self.streak_cap) {
            return Err("game spec: mischief_rules streak_secs 0.2..=10, streak_cap 1..=20".into());
        }
        Ok(())
    }
}

/// The default props: weights set how often each shows up.
pub fn default_mischief() -> Vec<MischiefSpec> {
    vec![
        MischiefSpec::new(MischiefKind::Vase, 2),
        MischiefSpec::new(MischiefKind::Bottle, 2),
        MischiefSpec::new(MischiefKind::Can, 2),
        MischiefSpec::new(MischiefKind::Yarn, 1),
    ]
}

/// The kinds of ledge a perch route is made of. The set is closed (each has
/// its own art).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerchKind {
    /// An air-conditioning box with fans.
    AcUnit,
    /// A round pipe along a wall.
    Pipe,
    /// A shop sign on posts.
    Sign,
}

impl PerchKind {
    pub const ALL: [PerchKind; 3] = [PerchKind::AcUnit, PerchKind::Pipe, PerchKind::Sign];

    /// Top height in metres when the spec leaves `height` at 0.
    pub fn default_height(self) -> f32 {
        match self {
            PerchKind::AcUnit => 0.6,
            PerchKind::Pipe => 1.1,
            PerchKind::Sign => 1.6,
        }
    }
}

/// One step of a route. `height` is the top above the street, 0 = the kind's own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerchStep {
    pub kind: PerchKind,
    #[serde(default)]
    pub height: f32,
}

impl PerchStep {
    pub fn new(kind: PerchKind) -> Self {
        Self { kind, height: 0.0 }
    }

    pub fn top(&self) -> f32 {
        if self.height > 0.0 {
            self.height
        } else {
            self.kind.default_height()
        }
    }
}

/// AC unit, then a pipe, then a sign: three steps up.
pub fn default_perch_route() -> Vec<PerchStep> {
    PerchKind::ALL.into_iter().map(PerchStep::new).collect()
}

/// Where perch routes stand and what running along one pays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PerchRules {
    /// One route every n-th free slot in a rest section (0 = none).
    pub rest_every: u32,
    /// The same for a reward section.
    pub reward_every: u32,
    /// The same for every other section.
    pub other_every: u32,
    /// Sideways distance of the route's centre from the track centre.
    pub side: f32,
    /// Half the width of a ledge.
    pub half_width: f32,
    /// Length of each step along the track, the steps touching end to end.
    pub step_len: f32,
    /// A ledge this much above the feet is stepped onto without a jump
    /// (so the first step has to be higher than this to need one).
    pub step_up: f32,
    /// Score for the first landing on a route.
    pub land_score: u32,
    /// Score per second spent on a ledge.
    pub run_score: u32,
    /// Score for reaching the last step.
    pub route_bonus: u32,
    /// Coins per step, laid along the top.
    pub coins_per_step: u32,
}

impl Default for PerchRules {
    fn default() -> Self {
        Self {
            rest_every: 2,
            reward_every: 2,
            other_every: 6,
            side: 1.35,
            half_width: 0.7,
            step_len: 3.6,
            step_up: 0.55,
            land_score: 30,
            run_score: 40,
            route_bonus: 120,
            coins_per_step: 3,
        }
    }
}

impl PerchRules {
    fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.8).contains(&self.side) || !(0.4..=1.2).contains(&self.half_width) {
            return Err("game spec: perch_rules side 0..=1.8, half_width 0.4..=1.2".into());
        }
        if !(1.5..=8.0).contains(&self.step_len) || !(0.1..=1.0).contains(&self.step_up) {
            return Err("game spec: perch_rules step_len 1.5..=8, step_up 0.1..=1".into());
        }
        if self.coins_per_step > 6 {
            return Err("game spec: perch_rules coins_per_step 0..=6".into());
        }
        Ok(())
    }
}

/// How the cleaning robot behaves once a hit has called it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChaserSpec {
    pub enabled: bool,
    /// How long it keeps chasing, seconds.
    pub secs: f32,
    /// Distance behind the cat when it shows up, metres: right on its tail.
    pub near_gap: f32,
    /// Distance behind the cat when it gives up. The chase view is low and
    /// close, so past about 2 m the robot slips off the bottom of the
    /// screen (a test projects it through the real camera).
    pub far_gap: f32,
    /// Time runs this many times faster while the cat stands on a ledge
    /// (a cleaner cannot climb).
    pub perch_drain: f32,
    /// Score for scaring it off with a meow.
    pub scare_score: u32,
    /// Its light and trim colour.
    pub color: [u8; 3],
}

impl Default for ChaserSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            secs: 10.0,
            near_gap: 1.2,
            far_gap: 1.7,
            perch_drain: 2.5,
            scare_score: 50,
            color: [255, 80, 40],
        }
    }
}

impl ChaserSpec {
    fn validate(&self) -> Result<(), String> {
        if !(2.0..=60.0).contains(&self.secs) || !(1.0..=3.0).contains(&self.perch_drain) {
            return Err("game spec: chaser secs 2..=60, perch_drain 1..=3".into());
        }
        if !(0.8..=2.2).contains(&self.far_gap) || !(0.8..=self.far_gap).contains(&self.near_gap) {
            return Err("game spec: chaser near_gap 0.8..=far_gap, far_gap <= 2.2".into());
        }
        Ok(())
    }
}

/// The default pick-up table: weights set how often each shows up.
pub fn default_items() -> Vec<ItemSpec> {
    vec![
        ItemSpec::new(ItemEffect::Magnet, 3),
        ItemSpec::new(ItemEffect::Double, 3),
        ItemSpec::new(ItemEffect::Superjump, 2),
        ItemSpec::new(ItemEffect::Boost, 2),
        ItemSpec::new(ItemEffect::Meow, 1),
    ]
}

/// One baked spawn: what, which lane, how far.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub z: f32,
    pub lane: i32,
    /// Obstacle `name`, or an item effect (`item:magnet`).
    pub kind: String,
}

/// Neon Cat's pacing: teach hops, add rolls, mix everything, then breathe.
pub fn default_sections() -> Vec<Section> {
    let sec = |kind, length, level, allow: &[Behavior]| Section {
        kind,
        length,
        level,
        density: None,
        allow: allow.to_vec(),
    };
    vec![
        sec(SectionKind::Intro, 140.0, 1, &[Behavior::JumpOver]),
        sec(
            SectionKind::Learn,
            200.0,
            2,
            &[Behavior::JumpOver, Behavior::RollUnder],
        ),
        sec(SectionKind::Challenge, 260.0, 3, &[]),
        sec(SectionKind::Rest, 150.0, 0, &[]),
        sec(SectionKind::Challenge, 300.0, 3, &[]),
        sec(SectionKind::Reward, 120.0, 0, &[]),
    ]
}

impl Default for RunnerKit {
    fn default() -> Self {
        Self {
            speed: SpeedCurve::default(),
            jump: JumpSpec::default(),
            lanes: LaneSpec::default(),
            obstacle_track: Track {
                z0: 26.0,
                // Far enough apart that two jumps in a row leave a quarter
                // second of slack at the speed cap.
                gap: 18.0,
                amp: 1.2,
                freq: 0.85,
            },
            rings: RingSpec::default(),
            gates: GateSpec::default(),
            draw: DrawSpec::default(),
            sections: default_sections(),
            body: BodySpec::default(),
            coins: CoinSpec::default(),
            score: ScoreSpec::default(),
        }
    }
}

impl Default for GameSpec {
    fn default() -> Self {
        Self {
            kind: GameKind::Runner,
            id: "neon_cat".into(),
            title: "NEON CAT".into(),
            seed: 11,
            player: PlayerSpec::default(),
            obstacles: vec![
                EntityKind {
                    name: "CRATE".into(),
                    glb: "assets/models/kenney/platformer-kit/crate.glb".into(),
                    scale: [1.15, 1.15, 1.15],
                    mesh_min: [-0.25, 0.0, -0.25],
                    mesh_max: [0.25, 0.5, 0.25],
                    weight: 2,
                    ..EntityKind::default()
                },
                EntityKind {
                    name: "BARREL".into(),
                    glb: "assets/models/kenney/platformer-kit/barrel.glb".into(),
                    scale: [1.2, 1.2, 1.2],
                    mesh_min: [-0.258813, 0.0, -0.258813],
                    mesh_max: [0.258813, 0.47619, 0.258813],
                    weight: 1,
                    ..EntityKind::default()
                },
                EntityKind {
                    name: "BARRIER".into(),
                    glb: "assets/models/kenney/racing-kit/barrierRed.glb".into(),
                    scale: [4.4, 3.8, 3.4],
                    mesh_min: [0.0, 0.0, -0.123],
                    mesh_max: [0.25, 0.1312, 0.0],
                    weight: 2,
                    ..EntityKind::default()
                },
                EntityKind {
                    name: "GENERATOR".into(),
                    glb: "assets/models/kenney/space-kit/machine_generatorLarge.glb".into(),
                    scale: [1.5, 3.0, 1.3],
                    mesh_min: [1.5, 0.0, 0.85],
                    mesh_max: [2.5, 0.683013, 2.15],
                    weight: 1,
                    behavior: Behavior::Solid,
                    ..EntityKind::default()
                },
                EntityKind {
                    name: "OVERPASS".into(),
                    glb: "".into(),
                    scale: [1.0, 1.0, 1.0],
                    mesh_min: [-0.5, 0.0, -0.5],
                    mesh_max: [0.5, 1.0, 0.5],
                    weight: 1,
                    behavior: Behavior::RollUnder,
                    // A low-hanging sign on two posts: a clear gap under it.
                    parts: vec![
                        Part {
                            glb: "assets/models/kenney/space-kit/structure_closed.glb".into(),
                            scale: [4.7, 1.1, 0.25],
                            mesh_min: [1.5, 0.0, 1.0],
                            mesh_max: [2.5, 1.0, 2.0],
                            offset: [0.0, 0.5, 0.0],
                            ..Part::default()
                        },
                        Part {
                            glb: "assets/models/kenney/space-kit/structure_closed.glb".into(),
                            scale: [0.25, 1.6, 0.25],
                            mesh_min: [1.5, 0.0, 1.0],
                            mesh_max: [2.5, 1.0, 2.0],
                            offset: [2.5, 0.0, 0.0],
                            role: PartRole::Decor,
                            ..Part::default()
                        },
                        Part {
                            glb: "assets/models/kenney/space-kit/structure_closed.glb".into(),
                            scale: [0.25, 1.6, 0.25],
                            mesh_min: [1.5, 0.0, 1.0],
                            mesh_max: [2.5, 1.0, 2.0],
                            offset: [-2.5, 0.0, 0.0],
                            role: PartRole::Decor,
                            ..Part::default()
                        },
                    ],
                    ..EntityKind::default()
                },
                EntityKind {
                    name: "TRAIN".into(),
                    glb: "assets/models/kenney/space-kit/monorail_trainBox.glb".into(),
                    scale: [2.4, 1.0, 4.4],
                    mesh_min: [1.75, 0.0, 1.0],
                    mesh_max: [2.25, 0.8, 2.0],
                    weight: 1,
                    behavior: Behavior::Rideable,
                    parts: vec![Part {
                        glb: "assets/models/kenney/space-kit/terrain_ramp.glb".into(),
                        scale: [1.2, 1.552, 4.0],
                        mesh_min: [1.5, 0.0, 1.0],
                        mesh_max: [2.5, 0.5, 2.0],
                        offset: [0.0, 0.0, -2.2],
                        role: PartRole::Ramp,
                        run: 2.0,
                        ..Part::default()
                    }],
                    ..EntityKind::default()
                },
                EntityKind {
                    name: "WALL".into(),
                    glb: "".into(),
                    scale: [1.0, 1.0, 1.0],
                    mesh_min: [-0.5, 0.0, -0.5],
                    mesh_max: [0.5, 1.0, 0.5],
                    weight: 1,
                    behavior: Behavior::CatGap,
                    // One lane wide, half a metre high, lit from inside.
                    parts: vec![
                        Part {
                            glb: "assets/models/kenney/space-kit/structure_closed.glb".into(),
                            scale: [2.2, 1.6, 0.8],
                            mesh_min: [1.5, 0.0, 1.0],
                            mesh_max: [2.5, 1.0, 2.0],
                            offset: [1.7, 0.0, 0.0],
                            ..Part::default()
                        },
                        Part {
                            glb: "assets/models/kenney/space-kit/structure_closed.glb".into(),
                            scale: [2.2, 1.6, 0.8],
                            mesh_min: [1.5, 0.0, 1.0],
                            mesh_max: [2.5, 1.0, 2.0],
                            offset: [-1.7, 0.0, 0.0],
                            ..Part::default()
                        },
                        Part {
                            glb: "assets/models/kenney/space-kit/structure_closed.glb".into(),
                            scale: [1.2, 1.1, 0.8],
                            mesh_min: [1.5, 0.0, 1.0],
                            mesh_max: [2.5, 1.0, 2.0],
                            offset: [0.0, 0.5, 0.0],
                            ..Part::default()
                        },
                        Part {
                            glb: "assets/models/kenney/space-kit/structure_closed.glb".into(),
                            scale: [1.2, 0.5, 0.06],
                            mesh_min: [1.5, 0.0, 1.0],
                            mesh_max: [2.5, 1.0, 2.0],
                            offset: [0.0, 0.0, 0.0],
                            role: PartRole::Decor,
                            glow: 1.8,
                            ..Part::default()
                        },
                    ],
                    ..EntityKind::default()
                },
            ],
            hit_inset: 0.06,
            hit_top: 0.82,
            camera: CameraSpec::default(),
            style: Style::default(),
            hud_top: 96.0,
            hud: HudSpec::default(),
            lives: LivesSpec::default(),
            items: default_items(),
            item_rules: ItemRules::default(),
            mischief: default_mischief(),
            mischief_rules: MischiefRules::default(),
            perch_route: default_perch_route(),
            perch_rules: PerchRules::default(),
            chaser: ChaserSpec::default(),
            placements: Vec::new(),
            runner: RunnerKit::default(),
            input: InputSpec::default(),
            feel: Default::default(),
        }
    }
}

impl Default for SpeedCurve {
    fn default() -> Self {
        Self {
            start: 10.5,
            ramp: 0.22,
            cap: 20.0,
            max_step: 0.08,
        }
    }
}

impl Default for JumpSpec {
    fn default() -> Self {
        Self {
            gravity: -22.0,
            velocity: 7.1,
            grace_secs: 1.6,
        }
    }
}

impl Default for LaneSpec {
    fn default() -> Self {
        Self {
            count: 3,
            width: 1.2,
            limit: 2.15,
            steer: 5.6,
        }
    }
}

impl Default for PlayerSpec {
    fn default() -> Self {
        Self {
            glb: "assets/models/kenney/cube-pets/animal-cat.glb".into(),
            scale: 0.50,
            foot: 0.30,
            tint: [226, 206, 255],
            rim: RimSpec::default(),
            eyes: EyesSpec::default(),
        }
    }
}

impl Default for EntityKind {
    fn default() -> Self {
        Self {
            name: "BOX".into(),
            glb: String::new(),
            scale: [1.0, 1.0, 1.0],
            mesh_min: [-0.5, 0.0, -0.5],
            mesh_max: [0.5, 1.0, 0.5],
            weight: 1,
            behavior: Behavior::JumpOver,
            parts: Vec::new(),
            hit_y: None,
            color: None,
        }
    }
}

impl Default for Track {
    fn default() -> Self {
        Self {
            z0: 0.0,
            gap: 10.0,
            amp: 0.0,
            freq: 1.0,
        }
    }
}

impl Default for RingSpec {
    fn default() -> Self {
        Self {
            glb: "assets/models/kenney/racing-kit/overheadRound.glb".into(),
            track: Track {
                z0: 14.0,
                gap: 17.0,
                amp: 0.95,
                freq: 1.2,
            },
            score: 100,
        }
    }
}

impl Default for GateSpec {
    fn default() -> Self {
        Self {
            glb: "assets/models/neon_cat/gate_arch.glb".into(),
            z0: 10.0,
            gap: 24.0,
            scale_x: 2.15,
        }
    }
}

impl Default for DrawSpec {
    fn default() -> Self {
        Self {
            lead_secs: 2.5,
            min_obstacles_ahead: 5,
            fog_start: 38.0,
            fog_end: 110.0,
        }
    }
}

impl Default for CameraSpec {
    fn default() -> Self {
        Self {
            chase_back: 5.9,
            chase_lift: 2.35,
            chase_look: 13.0,
            chase_look_y: 1.12,
            fov: 50.0,
            fov_fast: 58.0,
            roll: 0.07,
            classic_back: 7.4,
            classic_lift: 1.72,
            classic_fov: 50.0,
        }
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            sky: [4, 2, 12, 255],
            edge_a: [40, 255, 255, 255],
            edge_b: [255, 42, 180, 255],
            fill_a: [0.10, 0.55, 0.65],
            fill_b: [0.60, 0.10, 0.42],
            fill_emissive: 0.8,
            obstacle_color: [36, 26, 54],
            codes: ColorCodes::default(),
        }
    }
}

impl Track {
    pub fn z(&self, index: i32) -> f32 {
        self.z0 + index as f32 * self.gap
    }

    pub fn x(&self, index: i32) -> f32 {
        (index as f32 * self.freq).sin() * self.amp
    }
}

impl GateSpec {
    pub fn z(&self, index: i32) -> f32 {
        self.z0 + index as f32 * self.gap
    }
}

impl GameSpec {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let spec: Self = serde_json::from_str(json).map_err(|e| format!("game spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn from_value(value: &serde_json::Value) -> Result<Self, String> {
        let spec: Self =
            serde_json::from_value(value.clone()).map_err(|e| format!("game spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    /// A route has to be climbable: the first step within reach of a plain
    /// jump, each step up no more than `step_up` above the one before, and
    /// the whole route inside a slot.
    fn validate_perch_route(&self) -> Result<(), String> {
        let r = &self.perch_rules;
        if self.perch_route.len() > 6 {
            return Err("game spec: perch_route has at most 6 steps".into());
        }
        let Some(first) = self.perch_route.first() else {
            return Ok(());
        };
        let apex = self.runner.jump.velocity * self.runner.jump.velocity
            / (2.0 * -self.runner.jump.gravity).max(1e-3);
        if self
            .perch_route
            .iter()
            .any(|p| !(0.2..=2.4).contains(&p.top()))
        {
            return Err("game spec: perch_route heights 0.2..=2.4".into());
        }
        if first.top() > apex * 0.9 + r.step_up {
            return Err(format!(
                "game spec: the first perch step ({:.2} m) is out of reach of a jump ({apex:.2} m)",
                first.top()
            ));
        }
        for w in self.perch_route.windows(2) {
            if w[1].top() - w[0].top() > r.step_up + 1e-4 {
                return Err("game spec: a perch step is too high above the one before it".into());
            }
        }
        let len = self.perch_route.len() as f32 * r.step_len;
        if len > self.runner.obstacle_track.gap - 3.0 {
            return Err("game spec: the perch route is longer than a slot".into());
        }
        Ok(())
    }

    /// Orb colours must tell apart from each other, from the coin and from
    /// the obstacle codes (the same sum-of-channels test as `style.codes`).
    fn validate_item_colors(&self) -> Result<(), String> {
        let dist = |a: [u8; 3], b: [u8; 3]| -> i32 {
            (0..3).map(|k| (a[k] as i32 - b[k] as i32).abs()).sum()
        };
        let c = &self.style.codes;
        let fixed = [
            ("coin", COIN_COLOR),
            ("jump_over", c.jump_over),
            ("roll_under", c.roll_under),
            ("solid", c.solid),
            ("cat_gap", c.cat_gap),
            ("rideable", c.rideable),
            ("pass_through", c.pass_through),
        ];
        for (i, a) in self.items.iter().enumerate() {
            for (name, f) in fixed {
                if dist(a.rgb(), f) < 90 {
                    return Err(format!(
                        "game spec: item {:?} colour is too close to {name}",
                        a.effect
                    ));
                }
            }
            for b in &self.items[i + 1..] {
                if a.effect != b.effect && dist(a.rgb(), b.rgb()) < 90 {
                    return Err(format!(
                        "game spec: items {:?} and {:?} are too alike to tell apart",
                        a.effect, b.effect
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        crate::game_core::feel::resolve(&self.feel)?;
        if self.obstacles.is_empty() {
            return Err("game spec: obstacles is empty".into());
        }
        if self.obstacles.iter().all(|o| o.weight == 0) {
            return Err("game spec: every obstacle weight is 0".into());
        }
        if self.runner.obstacle_track.gap <= 0.0
            || self.runner.rings.track.gap <= 0.0
            || self.runner.gates.gap <= 0.0
        {
            return Err("game spec: gaps must be > 0".into());
        }
        self.hud.validate()?;
        self.lives.validate()?;
        self.item_rules.validate()?;
        self.mischief_rules.validate()?;
        self.perch_rules.validate()?;
        self.chaser.validate()?;
        self.validate_perch_route()?;
        if self
            .items
            .iter()
            .any(|i| i.duration < 0.0 || !i.duration.is_finite())
        {
            return Err("game spec: item duration must be >= 0".into());
        }
        self.validate_item_colors()?;
        for (i, s) in self.runner.sections.iter().enumerate() {
            if s.length <= 0.0 || !s.length.is_finite() {
                return Err(format!("game spec: section {i} length must be > 0"));
            }
            if s.level > 5 {
                return Err(format!("game spec: section {i} level must be 0..=5"));
            }
            if let Some(d) = s.density {
                if !(0.0..=1.0).contains(&d) {
                    return Err(format!("game spec: section {i} density must be 0..=1"));
                }
            }
        }
        let c = &self.style.codes;
        let named = [
            ("jump_over", c.jump_over),
            ("roll_under", c.roll_under),
            ("solid", c.solid),
            ("cat_gap", c.cat_gap),
            ("rideable", c.rideable),
            ("pass_through", c.pass_through),
        ];
        for (i, (an, a)) in named.iter().enumerate() {
            for (bn, b) in &named[i + 1..] {
                let d: i32 = (0..3).map(|k| (a[k] as i32 - b[k] as i32).abs()).sum();
                if d < 90 {
                    return Err(format!(
                        "game spec: style.codes {an} and {bn} are too alike to tell apart"
                    ));
                }
            }
        }
        let b = &self.runner.body;
        if b.roll_height <= 0.0 || b.stand_height <= b.roll_height || b.roll_secs <= 0.0 {
            return Err(
                "game spec: body needs 0 < roll_height < stand_height, roll_secs > 0".into(),
            );
        }
        for o in &self.obstacles {
            if o.glb.is_empty() && o.parts.is_empty() {
                return Err(format!("game spec: obstacle {} has no mesh", o.name));
            }
            let ramps = o.parts.iter().filter(|p| p.role == PartRole::Ramp).count();
            let rides = o.parts.iter().filter(|p| p.role == PartRole::Ride).count()
                + usize::from(o.behavior == Behavior::Rideable && !o.glb.is_empty());
            if ramps > 0 && rides == 0 {
                return Err(format!(
                    "game spec: obstacle {} has a ramp but no ride top",
                    o.name
                ));
            }
            if o.parts
                .iter()
                .any(|p| p.role == PartRole::Ramp && p.run <= 0.0)
            {
                return Err(format!("game spec: obstacle {} ramp needs run > 0", o.name));
            }
        }
        if self.runner.speed.max_step <= 0.0 || self.runner.speed.cap < self.runner.speed.start {
            return Err("game spec: bad speed curve".into());
        }
        Ok(())
    }

    /// The section covering distance `z`, with its index. Sections repeat.
    pub fn section_at(&self, z: f32) -> Option<(usize, &Section)> {
        let list = &self.runner.sections;
        let total: f32 = list.iter().map(|s| s.length).sum();
        if list.is_empty() || total <= 0.0 {
            return None;
        }
        let mut d = (z - self.runner.obstacle_track.z0 + total).rem_euclid(total);
        for (i, s) in list.iter().enumerate() {
            if d < s.length {
                return Some((i, s));
            }
            d -= s.length;
        }
        list.last().map(|s| (list.len() - 1, s))
    }

    /// Does obstacle slot `index` exist? Always true without sections.
    /// Otherwise a deterministic per-index hash keeps `density` of the slots.
    pub fn obstacle_active(&self, index: i32) -> bool {
        let Some((_, s)) = self.section_at(self.runner.obstacle_track.z(index)) else {
            return true;
        };
        let d = s.density();
        if d >= 1.0 {
            return true;
        }
        let h = (index as u32).wrapping_mul(2_654_435_761) >> 16;
        (h as f32 / 65536.0) < d
    }

    /// The original three-crate course: no sections, no new kinds. Kept for
    /// the golden runs and for comparing against older baselines.
    pub fn legacy() -> Self {
        let mut s = Self::default();
        s.obstacles.truncate(3);
        for o in &mut s.obstacles {
            o.weight = 1;
        }
        s.runner.sections = Vec::new();
        s.runner.obstacle_track.gap = 14.0;
        // The recorded runs score distance and rings only.
        s.runner.score.per_gate = 0;
        s.lives.count = 1;
        s.items = Vec::new();
        s.mischief = Vec::new();
        s.perch_route = Vec::new();
        s.chaser.enabled = false;
        s
    }

    /// Any obstacle kind that lifts the cat (a ramp or a ride top)?
    pub fn has_rideables(&self) -> bool {
        self.obstacles.iter().any(|o| {
            o.behavior == Behavior::Rideable
                || o.parts
                    .iter()
                    .any(|p| matches!(p.role, PartRole::Ride | PartRole::Ramp))
        })
    }

    /// Obstacle kind at `index`. A fixed cycle where each allowed kind appears
    /// `weight` times in table order, so equal weights give `index % n`.
    /// A section's `allow` list narrows the table (an empty or unmatched
    /// list means all kinds).
    pub fn obstacle_kind(&self, index: i32) -> &EntityKind {
        let allow = self
            .section_at(self.runner.obstacle_track.z(index))
            .map(|(_, s)| s.allow.as_slice())
            .unwrap_or(&[]);
        let ok = |o: &EntityKind| allow.is_empty() || allow.contains(&o.behavior);
        let mut total: u32 = self
            .obstacles
            .iter()
            .filter(|o| ok(o))
            .map(|o| o.weight)
            .sum();
        let filter = total > 0;
        if !filter {
            total = self.obstacles.iter().map(|o| o.weight).sum();
        }
        let mut slot = index.rem_euclid(total.max(1) as i32) as u32;
        for kind in &self.obstacles {
            if filter && !ok(kind) {
                continue;
            }
            if slot < kind.weight {
                return kind;
            }
            slot -= kind.weight;
        }
        &self.obstacles[0]
    }

    /// Every static GLB the run can show, in a fixed order (renderer slots).
    pub fn static_glbs(&self) -> Vec<&str> {
        let mut out: Vec<&str> = vec![
            self.runner.gates.glb.as_str(),
            self.runner.rings.glb.as_str(),
        ];
        if self.runner.coins.enabled && !self.runner.coins.glb.is_empty() {
            out.push(self.runner.coins.glb.as_str());
        }
        for o in &self.obstacles {
            let parts = o.parts.iter().map(|p| p.glb.as_str());
            for glb in std::iter::once(o.glb.as_str()).chain(parts) {
                if !glb.is_empty() && !out.contains(&glb) {
                    out.push(glb);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_spec_rejects_a_bad_ranking() {
        let bad = |f: &dyn Fn(&mut GameSpec)| {
            let mut s = GameSpec::default();
            f(&mut s);
            s.validate().unwrap_err()
        };
        assert!(bad(&|s| s.hud.result.board.top_n = 0).contains("top_n"));
        assert!(bad(&|s| s.hud.result.board.top_n = 11).contains("top_n"));
        assert!(bad(&|s| s.hud.result.board.columns = vec![]).contains("columns"));
        assert!(bad(&|s| s.hud.result.board.columns = vec!["kills".into()]).contains("kills"));
        assert!(bad(&|s| {
            s.hud.result.board.columns = vec!["time".into()];
            s.hud.result.board.sort = "coins".into();
        })
        .contains("sort"));
        assert!(bad(&|s| s.hud.scale = 0.0).contains("scale"));
        let mut ok = GameSpec::default();
        ok.hud.result.board.columns = vec!["score".into(), "coins".into()];
        ok.validate().unwrap();
    }

    #[test]
    fn empty_json_is_neon_cat() {
        assert_eq!(GameSpec::from_json("{}").unwrap(), GameSpec::default());
    }

    #[test]
    fn round_trips_and_rejects_unknown_fields() {
        let spec = GameSpec::legacy();
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(GameSpec::from_json(&json).unwrap(), spec);
        assert!(GameSpec::from_json(r#"{"runner":{"sped":{}}}"#).is_err());
    }

    #[test]
    fn items_use_the_fixed_vocabulary() {
        let ok = r#"{"items":[{"effect":"magnet"},{"effect":"double","duration":3}]}"#;
        let spec = GameSpec::from_json(ok).unwrap();
        assert_eq!(spec.items[0].effect, ItemEffect::Magnet);
        assert!(GameSpec::from_json(r#"{"items":[{"effect":"laser"}]}"#).is_err());
    }

    #[test]
    fn sections_validate_and_repeat() {
        let mut v = serde_json::to_value(GameSpec::legacy()).unwrap();
        v["runner"]["sections"] = serde_json::json!([
            {"kind": "challenge", "length": 100.0, "level": 3},
            {"kind": "rest", "length": 50.0, "level": 0}]);
        let s = GameSpec::from_value(&v).unwrap();
        let z0 = s.runner.obstacle_track.z0;
        assert_eq!(s.section_at(z0 + 10.0).unwrap().0, 0);
        assert_eq!(s.section_at(z0 + 120.0).unwrap().0, 1);
        assert_eq!(s.section_at(z0 + 160.0).unwrap().0, 0, "the list repeats");
        for bad in [
            serde_json::json!([{"length": 0.0}]),
            serde_json::json!([{"level": 6}]),
            serde_json::json!([{"density": 1.5}]),
            serde_json::json!([{"kind": "boss"}]),
        ] {
            v["runner"]["sections"] = bad;
            assert!(GameSpec::from_value(&v).is_err());
        }
        assert!(
            GameSpec::legacy().obstacle_active(3),
            "no sections: all active"
        );
    }

    #[test]
    fn weighted_cycle() {
        let mut spec = GameSpec::legacy();
        let names = |s: &GameSpec| {
            (0..6)
                .map(|i| s.obstacle_kind(i).name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(&spec),
            ["CRATE", "BARREL", "BARRIER", "CRATE", "BARREL", "BARRIER"]
        );
        spec.obstacles[0].weight = 2;
        assert_eq!(
            names(&spec),
            ["CRATE", "CRATE", "BARREL", "BARRIER", "CRATE", "CRATE"]
        );
    }
}
