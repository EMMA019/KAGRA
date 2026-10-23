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
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameKind {
    #[default]
    Runner,
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
                    weight: 1,
                },
                EntityKind {
                    name: "BARREL".into(),
                    glb: "assets/models/kenney/platformer-kit/barrel.glb".into(),
                    scale: [1.2, 1.2, 1.2],
                    mesh_min: [-0.258813, 0.0, -0.258813],
                    mesh_max: [0.258813, 0.476190, 0.258813],
                    weight: 1,
                },
                EntityKind {
                    name: "BARRIER".into(),
                    glb: "assets/models/kenney/racing-kit/barrierRed.glb".into(),
                    scale: [4.4, 3.8, 3.4],
                    mesh_min: [0.0, 0.0, -0.123],
                    mesh_max: [0.25, 0.1312, 0.0],
                    weight: 1,
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
        if self.runner.speed.max_step <= 0.0 || self.runner.speed.cap < self.runner.speed.start {
            return Err("game spec: bad speed curve".into());
        }
        Ok(())
    }

    /// Obstacle kind at `index`. A fixed cycle where each kind appears
    /// `weight` times in table order, so equal weights give `index % n`.
    pub fn obstacle_kind(&self, index: i32) -> &EntityKind {
        let total: u32 = self.obstacles.iter().map(|o| o.weight).sum();
        let mut slot = index.rem_euclid(total.max(1) as i32) as u32;
        for kind in &self.obstacles {
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
        for o in &self.obstacles {
            if !out.contains(&o.glb.as_str()) {
                out.push(o.glb.as_str());
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
        let spec = GameSpec::default();
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
    fn weighted_cycle() {
        let mut spec = GameSpec::default();
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
