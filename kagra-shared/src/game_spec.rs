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
    /// Reserved. Power-ups with a fixed effect vocabulary. Not simulated yet.
    pub items: Vec<ItemSpec>,
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
}

impl Default for BodySpec {
    fn default() -> Self {
        Self {
            stand_height: 0.62,
            roll_height: 0.3,
            roll_secs: 0.55,
            step_tol: 0.18,
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
    Shield,
    Magnet,
    Slowmo,
    Double,
    Boost,
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
}

fn one() -> u32 {
    1
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
                gap: 14.0,
                amp: 1.2,
                freq: 0.85,
            },
            rings: RingSpec::default(),
            gates: GateSpec::default(),
            draw: DrawSpec::default(),
            sections: default_sections(),
            body: BodySpec::default(),
            coins: CoinSpec::default(),
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
                    glb: "assets/models/kenney/space-kit/hangar_smallA.glb".into(),
                    scale: [2.2, 0.56, 0.9],
                    mesh_min: [1.0, 0.0, 0.5],
                    mesh_max: [3.0, 1.0, 2.5],
                    weight: 1,
                    behavior: Behavior::RollUnder,
                    hit_y: Some([0.9, 1.0]),
                    parts: vec![Part {
                        glb: "assets/models/kenney/space-kit/structure_closed.glb".into(),
                        scale: [4.6, 1.0, 0.5],
                        mesh_min: [1.5, 0.0, 1.0],
                        mesh_max: [2.5, 1.0, 2.0],
                        offset: [0.0, 0.56, 0.0],
                        ..Part::default()
                    }],
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
                    parts: vec![
                        Part {
                            glb: "assets/models/kenney/space-kit/structure_closed.glb".into(),
                            scale: [2.2, 1.6, 0.8],
                            mesh_min: [1.5, 0.0, 1.0],
                            mesh_max: [2.5, 1.0, 2.0],
                            offset: [1.55, 0.0, 0.0],
                            ..Part::default()
                        },
                        Part {
                            glb: "assets/models/kenney/space-kit/structure_closed.glb".into(),
                            scale: [2.2, 1.6, 0.8],
                            mesh_min: [1.5, 0.0, 1.0],
                            mesh_max: [2.5, 1.0, 2.0],
                            offset: [-1.55, 0.0, 0.0],
                            ..Part::default()
                        },
                        Part {
                            glb: "assets/models/kenney/space-kit/structure_closed.glb".into(),
                            scale: [0.9, 1.15, 0.8],
                            mesh_min: [1.5, 0.0, 1.0],
                            mesh_max: [2.5, 1.0, 2.0],
                            offset: [0.0, 0.45, 0.0],
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
            items: Vec::new(),
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
            glb: "assets/models/quaternius/modular-dungeons-pack/Arch.glb".into(),
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
            fill_emissive: 0.35,
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
        let ok = r#"{"items":[{"effect":"magnet"},{"effect":"slowmo","duration":3}]}"#;
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
