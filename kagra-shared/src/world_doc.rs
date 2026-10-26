//! Persistent world document (`docs/schemas/world.json` version 1).
//!
//! `Scene3D` is a **one-frame draw list** (camera, batches, fog). Collectathon
//! and driving already build that. Dump JSON lives here as `WorldDoc`, then
//! `compile_scene` turns it into a `Scene3D` for one frame. Heightfield
//! batches come from named demo fns (`open_world_height` / `island_height` /
//! `overworld_height`) or dump samples — production island mesh, not a
//! placeholder plane. Sprite/quad props (`model: "sprite"` / `"quad"`) compile
//! to a standing XY card in the same Scene3D — 2D and 3D share WorldDoc.
//! Coins use `Material::Metal` (existing GGX, metallic=1 / roughness=0.12).
//! Water uses `Material::Water` (prop name/model `water`, or the `water_y` plane).
//! Same shader family as Metal/Toon; not a Water Renderer / V2.
//! Lights are slot 0..3 1:1; an empty dump still gets default key+fill
//! (slots 0+1; 2 and 3 stay off). Outdoor dumps default IBL 0.35 + ACES
//! (shader Globals.env; no cubemap bind, no RendererV2). Capsules and props get a ground contact
//! blob (`MESH_PLANE` + instance alpha). glTF props use `gltf_load`. Walker may name a skinned glTF (`gltf` / `model`);
//! CPU-skin into Vertex3. `.vrm` is glTF-binary on the same path. Walker MToon uses Material::Toon (shadeColor + shadingToony). Capsule remains the fallback. Integer GPU mesh ids are not game objects. Live play is
//! `WorldPlay` (title → play → result, WASD → `WalkInput` → sit on
//! heightfield → pick up). Offscreen draw (feature = "render") is
//! `render_world_doc`. A real desktop window is `Renderer::new_for_window`
//! + example `window`. Not kagra-core `RendererV2` / `window.rs`.
//!
//! Python `Walk.wish` / `CharacterController` (accel 14 / decel 22 / 8-point
//! foot ring / step-up) is the leftover VRM motor. This crate does **not**
//! copy that solver and does not add Rapier. Shared tick matches collectathon
//! `WalkInput`: camera-relative wish, sit on `height_at`, optional jump.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::collectathon::open_world_height;
use crate::gltf_load::{
    is_tpose_humanoid_spec, is_walk_skinned_spec, is_walk_vrm_spec, mesh_from_embedded_gltf,
    mesh_from_glb, mesh_from_gltf_json, mesh_index_from_gltf_json, sample_skinned_look,
    skinned_from_embedded_gltf, skinned_from_glb, skinned_from_gltf_json,
    skinned_parts_from_embedded_gltf, skinned_parts_from_glb, skinned_parts_from_gltf_json,
    skinned_tpose_humanoid, unit_cube_gltf, walk_skinned_gltf, walk_skinned_vrm, WalkerPose,
};
use crate::scene3d::{
    primitives, Aabb, Camera, Instance, LocalLight, Material, MeshData, MeshId, Scene3D,
    SceneBuilder, Vertex3,
};
use crate::strokes::{FatLine, PointCloud};
use glam::{EulerRot, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// `docs/schemas/world.json` の version。他は拒否する。
pub const WORLD_DUMP_VERSION: u32 = 1;

const MESH_BOX: MeshId = MeshId(0);
const MESH_SPHERE: MeshId = MeshId(1);
const MESH_CAPSULE: MeshId = MeshId(2);
const MESH_PLANE: MeshId = MeshId(3);
pub(crate) const MESH_HEIGHTFIELD: MeshId = MeshId(4);
pub(crate) const MESH_QUAD: MeshId = MeshId(5);
const MESH_CONE: MeshId = MeshId(6);
const MESH_TORUS: MeshId = MeshId(7);
pub(crate) const MESH_GLTF_BASE: u32 = 8;

/// See [`WorldDoc::mesh_lod`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshLod {
    /// Fraction of the triangles to keep, 0.2..=1.
    pub keep: f32,
    /// A model is thinned when its path contains any of these.
    pub models: Vec<String>,
}

/// Persistent world. JSON source of truth. Not a draw list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldDoc {
    pub version: u32,
    /// Explicit genre id (`crest_isle`, `fish_cast`, …). None = free world:
    /// walk / camera / interact / timers / events only. Prop names are not
    /// reserved words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(default)]
    pub half: f32,
    #[serde(default)]
    pub floor_y: f32,
    #[serde(default = "default_gravity")]
    pub gravity: f32,
    #[serde(default)]
    pub water_y: Option<f32>,
    #[serde(default)]
    pub coins: u32,
    #[serde(default)]
    pub player: Option<WorldWalker>,
    #[serde(default)]
    pub props: Vec<WorldProp>,
    #[serde(default)]
    pub walkers: Vec<WorldWalker>,
    #[serde(default)]
    pub lights: Vec<WorldLight>,
    #[serde(default)]
    pub cameras: Vec<WorldCamera>,
    #[serde(default)]
    pub heightfield: Option<WorldHeightfield>,
    /// Diffuse IBL strength. None = 0.35 (outdoor). 0 = off.
    #[serde(default)]
    pub ibl: Option<f32>,
    /// Linear exposure. None = 1.0.
    #[serde(default)]
    pub exposure: Option<f32>,
    /// ACES tonemap. None = on.
    #[serde(default)]
    pub tonemap: Option<bool>,
    /// `"aces"`, `"agx"`, or `"neutral"`. None and unknown names stay on ACES.
    /// Ignored while `tonemap` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tonemap_curve: Option<String>,
    /// Procedural Rayleigh / Mie sky on the sky sphere. None = the studio gradient.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sky_atmosphere: Option<bool>,
    /// Picture tier. None keeps the historical picture (`KAGRA_QUALITY=high`
    /// still fills omitted SSAO / TAA / cascades / PCSS, and does not turn
    /// bloom on). `low` / `0` / `off` forces SSAO, TAA, DOF, motion, extra
    /// cascades, and PCSS off. `mobile` and `high` fill only omitted fields.
    /// An explicit field wins. `KAGRA_QUALITY=0` / `off` / `low` still forces
    /// that floor, ahead of this string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    /// Bloom intensity on the offscreen dump path. None = 0 so an old dump
    /// stays unbloomed, unless `quality` is `mobile` (0.22) or `high` (0.35).
    /// The desktop window example sets its own default. Env `high` does not
    /// turn bloom on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bloom: Option<f32>,
    /// Bloom extract threshold. None = 0.85.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bloom_threshold: Option<f32>,
    /// Vignette strength. None = 1 (the historical composite curve). 0 = off.
    /// Values above 1 darken the corners a second time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vignette: Option<f32>,
    /// Color-grade LUT preset: `off` / `warm` / `punch` / `cool`. None = off.
    /// Unknown names bake an identity strip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lut: Option<String>,
    /// Mix toward the LUT after ACES, 0..1. None = 1 when a preset is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lut_strength: Option<f32>,
    /// FXAA on the composite. None = on. Ignored while `taa` is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fxaa: Option<bool>,
    /// Thin the triangles of the glTF models whose path contains one of `models`
    /// (vertex clustering at load, same textures and silhouette), so a crowd of small
    /// figures costs less on the GPU. None = every model at full detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_lod: Option<MeshLod>,
    /// Half-resolution SSAO radius in pixels. None = off, unless `quality` is
    /// `mobile` (4) or `high` (8), or env `KAGRA_QUALITY=high` (8).
    /// `quality=low` and `KAGRA_QUALITY=0` / `off` / `low` force 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssao: Option<f32>,
    /// Temporal AA. None = off. `quality=high` and `KAGRA_QUALITY=high` turn it
    /// on when this is omitted. `quality=mobile` keeps FXAA. `quality=low` and
    /// `KAGRA_QUALITY=0` / `off` / `low` force it off. Replaces FXAA when on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taa: Option<bool>,
    /// DOF focus distance from the camera, in world units. Needs `dof_aperture`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dof_focus: Option<f32>,
    /// DOF aperture. None or 0 = off. Larger blurs pixels away from `dof_focus`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dof_aperture: Option<f32>,
    /// Camera motion-blur shutter, 0..1. None or 0 = off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_blur: Option<f32>,
    /// Directional shadow radius. None = 1, the historical 3×3 PCF.
    /// Larger values (about 2.5–4) use a wider tent so penumbras soften.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_softness: Option<f32>,
    /// Cascade count packed into the one 2048 map. None = 1 (the historical
    /// full map). `quality=mobile` fills 2, `quality=high` and
    /// `KAGRA_QUALITY=high` / `2` fill 3. An explicit count wins. `quality=low`
    /// and env `0` / `off` / `low` force 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_cascades: Option<u32>,
    /// Contact-hardening blocker search. None = off. `quality=high` and env
    /// `high` turn it on when this is omitted. `quality=mobile` leaves it off.
    /// `quality=low` and `KAGRA_QUALITY=0` / `off` / `low` force it off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_pcss: Option<bool>,
    /// Hemisphere sky, linear 0..1. None = the studio blue `(0.48, 0.56, 0.78)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hemi_sky: Option<[f32; 3]>,
    /// Hemisphere ground, linear 0..1. None = the studio warm `(0.42, 0.30, 0.18)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hemi_ground: Option<[f32; 3]>,
    /// Prefiltered env. `preset:studio` bakes a lat-long from the hemisphere.
    /// A PNG path is sampled as an equirect and blurred into roughness rows.
    /// None = the SH specular, so an old dump does not change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_map: Option<String>,
    /// Register mesh bounds and drop props outside the camera. None / false
    /// draws every prop, which a tight dump camera used to rely on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cull: Option<bool>,
    /// Distance where a non-vegetation prop becomes one billboard quad.
    /// None = full meshes. Vegetation keeps its own `lod_radius`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prop_lod: Option<f32>,
    /// Generic countdowns. WorldPlay::tick_timers advances `remaining`; on 0 it
    /// emits `on_done` as an event and flips `active` false. Cooking wait,
    /// fishing cast→bite, cooldowns all map onto this. Genre code stays out.
    #[serde(default)]
    pub timers: Vec<WorldTimer>,
    /// Named things that happened. WorldPlay::emit_event (aggregates by name) /
    /// take_events (consumes). Multiple systems read the same event (score, UI,
    /// save) without an in-process callback bus. Dump-visible.
    #[serde(default)]
    pub events: Vec<WorldEvent>,
    /// 風（SpringBone 布の外部加速度、m/s^2 相当）。デフォルト無風。
    #[serde(default)]
    pub wind: [f32; 3],
    /// Clear + fog color. None = outdoor sky `[130, 165, 205, 255]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sky: Option<[u8; 4]>,
    /// Auto grass plane when there is no heightfield. None = true (legacy).
    /// Indoor rooms dump `false` and place their own floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground: Option<bool>,
    /// Soft contact discs under props. None = on. A flying scene sets false
    /// so coplanar discs do not z-fight into black streaks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_blobs: Option<bool>,
    /// Fog start in meters. None = 48.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fog_start: Option<f32>,
    /// Fog end in meters. None = 220.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fog_end: Option<f32>,
    /// Fat lines fade out between these view depths (start, end). None = no
    /// fade. Lines are not fogged by the mesh shader, so a long rail would
    /// otherwise end in a hard cut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_fade: Option<[f32; 2]>,
    /// Rapier joints (`physics` feature). Empty dumps omit the key.
    /// `name` is `fixed` / `revolute` / `spherical`. Anchors are body-local.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joints: Vec<WorldJoint>,
    /// Camera-facing point sprites. One instanced draw. Empty omits the key.
    /// Not physics, and not the CPU `Emitter` props.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<PointCloud>,
    /// Screen-space fat lines, pixel width. One instanced draw. Empty omits
    /// the key. Not the CPU `Trail` quads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<FatLine>,
}

/// Generic countdown. Dump-visible: a fishing dump shows `cast` counting down,
/// then the `bite` event appears. Not a scheduler with callbacks — the engine
/// only advances time and emits one event at completion.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldTimer {
    pub id: String,
    #[serde(rename = "type", default = "timer_type")]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    /// Total seconds. Remaining starts here unless the dump says otherwise.
    #[serde(default)]
    pub seconds: f32,
    /// Remaining seconds. WorldPlay::tick_timers counts down while active.
    #[serde(default)]
    pub remaining: f32,
    /// Event name emitted when the countdown completes (e.g. "bite").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_done: Option<String>,
    /// False = parked (not counting down). A game starts it via emit/use.
    #[serde(default = "default_true")]
    pub active: bool,
}

fn timer_type() -> String {
    "timer".into()
}

/// A named event. `count` aggregates repeats (two fish = count 2). `data` is
/// an optional JSON payload (which fish, which prop). Events stay in the dump
/// until a system takes them, so save/UI/score can all react to the same one.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldEvent {
    pub id: String,
    #[serde(rename = "type", default = "event_type")]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

fn event_type() -> String {
    "event".into()
}

fn default_gravity() -> f32 {
    9.8
}

/// `KAGRA_QUALITY=0`, `off`, or `low` keeps SSAO / TAA / DOF / motion off
/// so a phone can stay on the bloom + FXAA path.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn heavy_post_allowed() -> bool {
    heavy_post_allowed_value(std::env::var("KAGRA_QUALITY").ok().as_deref())
}

fn heavy_post_allowed_value(quality: Option<&str>) -> bool {
    !matches!(quality, Some("0") | Some("off") | Some("low"))
}

/// `high` or `2` enables SSAO and TAA when the dump leaves them unset.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn quality_is_high() -> bool {
    quality_is_high_value(std::env::var("KAGRA_QUALITY").ok().as_deref())
}

fn quality_is_high_value(quality: Option<&str>) -> bool {
    matches!(quality, Some("high") | Some("2"))
}

/// Resolved picture. Env `0` / `off` / `low` is [`PictureTier::Low`] even when
/// the dump asks for mobile or high. A dump tier wins over env high.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PictureTier {
    Low,
    Mobile,
    High,
    /// `quality` omitted and the env is not high. Historical accessors.
    Legacy,
}

fn authored_tier(dump_quality: Option<&str>) -> Option<PictureTier> {
    match dump_quality.map(str::trim).filter(|s| !s.is_empty()) {
        Some("low" | "0" | "off") => Some(PictureTier::Low),
        Some("mobile") => Some(PictureTier::Mobile),
        Some("high" | "2") => Some(PictureTier::High),
        _ => None,
    }
}

fn picture_tier_value(dump_quality: Option<&str>, env_quality: Option<&str>) -> PictureTier {
    if !heavy_post_allowed_value(env_quality) {
        return PictureTier::Low;
    }
    match authored_tier(dump_quality) {
        Some(tier) => tier,
        None if quality_is_high_value(env_quality) => PictureTier::High,
        None => PictureTier::Legacy,
    }
}

fn picture_tier(dump_quality: Option<&str>) -> PictureTier {
    picture_tier_value(dump_quality, std::env::var("KAGRA_QUALITY").ok().as_deref())
}

/// Prop record. `position` / `yaw` / `pitch` / `roll` are world-space unless
/// `local` is set, in which case they are parent-space (up to 4 links).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldProp {
    pub id: String,
    #[serde(rename = "type", default = "prop_type")]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default)]
    pub yaw: f32,
    /// Nose up (radians). Applied as intrinsic Y, then X, then Z with `roll`.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub pitch: f32,
    /// Bank (radians). Positive lifts the local +X side.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub roll: f32,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub gltf: Option<String>,
    #[serde(default = "unit_scale")]
    pub scale: [f32; 3],
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub parent: Option<String>,
    /// When true, `position` / `yaw` / `pitch` / `roll` / `scale` are in the
    /// parent prop's space (`parent` * local). Crest coins leave this false:
    /// `parent` stays a link and `position` stays world-space.
    #[serde(default, skip_serializing_if = "is_false")]
    pub local: bool,
    #[serde(default)]
    pub color: Option<[u32; 3]>,
    /// 0..1. Coins dump 1.0. Default 0 (Lambert).
    #[serde(default)]
    pub metallic: f32,
    /// 0..1. Coins dump 0.12. Default 1 (dielectric).
    #[serde(default = "default_roughness")]
    pub roughness: f32,
    /// Unlit HDR add so a lamp or a coin can bloom on purpose. None = 0.
    /// The color is `emissive_color`, or the albedo when that is omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive: Option<f32>,
    /// Linear 0..1 RGB. None = use the albedo as the glow color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive_color: Option<[f32; 3]>,
    /// 0..1 clearcoat lobe. 0 keeps the mesh extension, or no coat.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub clearcoat: f32,
    /// Clearcoat roughness. 0 in the shader means 0.1 when a coat is on.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub clearcoat_roughness: f32,
    /// PNG path or `preset:worn` / `preset:split` / `preset:brushed`.
    /// G channel replaces the roughness scalar. None = scalar only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roughness_map: Option<String>,
    /// PNG path or the same presets. B channel replaces the metallic scalar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metalness_map: Option<String>,
    /// PNG path or `preset:core`. Multiplies the emissive color. Strength is `emissive`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive_map: Option<String>,
    /// Interaction metadata (talk / use / examine). None = plain scenery.
    /// `on_use` emits an event when the player presses J/attack in reach.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interact: Option<WorldInteract>,
    /// 剛体物理（Phase 1 / Rapier）: True = 動かない（壁・床・景観）、
    /// False = 落下・衝突・積み重なる動的剛体。デフォルト true（既存は静的）。
    /// dump キー `is_static`。
    #[serde(default = "default_true")]
    pub is_static: bool,
    /// 動的剛体の摩擦係数（デフォルト 0.85、old physics3d と同じ）。
    #[serde(default = "default_friction")]
    pub friction: f32,
    /// 動的剛体の反発係数（デフォルト 0.0 = 跳ねない）。
    #[serde(default)]
    pub restitution: f32,
    /// PNG path (repo-relative). compile_meshes clones the primitive and
    /// attaches albedo so box / plane / sprite / quad can show AI art.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// Repeat the albedo this many times (CPU tile; sampler stays Clamp).
    /// None = `[1, 1]`. Wood floors dump `[8, 6]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_scale: Option<[f32; 2]>,
    /// Rapier body kind: `static` / `dynamic` / `kinematic`.
    /// None = `is_static` (true → static, false → dynamic).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Collider shape: `box` / `sphere` / `capsule` / `cylinder` / `cone` / `none`.
    /// None = infer from `model` (unknown models stay out of the sim).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collider: Option<String>,
    /// Overlap events only. No contact response.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sensor: bool,
    /// Dynamic mass in kilograms. 0 = density from the collider volume.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub mass: f32,
}

/// Impulse joint between two prop / walker ids. Physics (`physics` feature)
/// builds it; the dump is the source of truth for Android later.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldJoint {
    #[serde(default)]
    pub id: String,
    /// Record type. `joint`, or the joint name if `name` is empty
    /// (`fixed` / `revolute` / `spherical`).
    #[serde(rename = "type", default = "joint_type")]
    pub kind: String,
    /// `fixed` / `revolute` / `spherical` (aliases: weld, hinge, ball).
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub body_a: String,
    #[serde(default)]
    pub body_b: String,
    /// Anchor in body A local space.
    #[serde(default)]
    pub anchor_a: [f32; 3],
    /// Anchor in body B local space.
    #[serde(default)]
    pub anchor_b: [f32; 3],
    /// Revolute axis in each body's local space. Default +Y.
    #[serde(default = "default_joint_axis")]
    pub axis: [f32; 3],
}

fn joint_type() -> String {
    "joint".into()
}

fn default_joint_axis() -> [f32; 3] {
    [0.0, 1.0, 0.0]
}

/// Prop interaction. The engine handles reach + on_use event; the game owns
/// the prompt text and what the event means (dialogue, reward, cast).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldInteract {
    /// "talk" | "use" | "examine". HUD glyph/color can key off this.
    #[serde(default)]
    pub kind: String,
    /// Short label shown on the HUD while in reach ("水辺", "cast").
    #[serde(default)]
    pub prompt: String,
    /// Event name emitted when the player uses it (e.g. "cast").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_use: Option<String>,
    /// Horizontal reach in world units. Default 2.5.
    #[serde(default = "default_reach")]
    pub reach: f32,
}

fn default_reach() -> f32 {
    2.5
}

fn default_friction() -> f32 {
    0.85
}

fn prop_type() -> String {
    "prop".into()
}

fn unit_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn default_true() -> bool {
    true
}

fn default_roughness() -> f32 {
    1.0
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// Yaw, then pitch (nose up), then roll. Positive yaw turns +Z toward +X.
pub(crate) fn prop_rotation(prop: &WorldProp) -> Quat {
    Quat::from_euler(EulerRot::YXZ, prop.yaw, -prop.pitch, prop.roll)
}

fn prop_local_matrix(prop: &WorldProp) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(prop.scale),
        prop_rotation(prop),
        Vec3::from_array(prop.position),
    )
}

/// Walker record (`walker:player`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldWalker {
    pub id: String,
    #[serde(rename = "type", default = "walker_type")]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub face: f32,
    #[serde(default)]
    pub on_ground: bool,
    /// Same dump keys as props (`model` name / `gltf` path). Capsule if unset.
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub gltf: Option<String>,
    /// Seconds into the walk clip. 0 = rest / T-pose. Dump-visible.
    #[serde(default)]
    pub clip: f32,
    /// First spring-bone local yaw (radians). Dump-visible. Changes idle/walk.
    #[serde(default)]
    pub hair: f32,
    /// Named expression weight (blink, else aa). Dump-visible. Idle blink / hold J.
    #[serde(default)]
    pub morph: f32,
    /// Head look yaw (radians, toward camera). Dump-visible.
    #[serde(default)]
    pub look_yaw: f32,
    /// Head look pitch (radians, toward camera). Dump-visible.
    #[serde(default)]
    pub look_pitch: f32,
    /// Last glTF/VRM load error (`path: reason`). Dump-visible. None if ok.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_error: Option<String>,
    /// Current animation state: "idle" / "walk" (engine-derived from wish) or a
    /// genre-set name ("cast", "reel", "hit") that survives while standing.
    /// Dump-visible, so the game state machine reads it without an engine hook.
    #[serde(default = "default_idle_anim")]
    pub anim: String,
    /// Selected VRM expression preset ("blink" = auto, "smile", "angry",
    /// "aa", ...). Dump-visible; games flip it via events / interact.
    #[serde(default = "default_expression")]
    pub expression: String,
    /// ロコモーションブレンド係数 0..1（0 = rest/idle、1 = walk クリップ）。
    /// 速度から連続的に変化させ、状態切替のポップを防ぐ（Phase 2）。
    #[serde(default)]
    pub anim_blend: f32,
    /// 上半身ジェスチャー: ノード名（humanoid 名 or node 名）→ 目標ローカル回転
    /// [x,y,z,w]。歩きクリップに乗る腕ジェスチャー等（Phase 2-2）。
    #[serde(default)]
    pub overlay_bones: HashMap<String, [f32; 4]>,
    /// `overlay_bones` のブレンド係数 0..1（0 = 無効、1 = 目標回転へ完全スラープ）。
    #[serde(default)]
    pub overlay_weight: f32,
    /// glTF clip to play by name ("Run", "Jump"). None = Walk / first clip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_name: Option<String>,
    /// Crossfade source clip name. See `anim_mixer::AnimMixer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_from: Option<String>,
    /// Seconds into `fade_from`.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub fade_from_clip: f32,
    /// Weight of `fade_from` 0..1 (1 = old clip only, 0 = done).
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub fade_from_weight: f32,
    /// Draw scale. None = (1, 1, 1). Squash/stretch writes this on the picture
    /// dump; the walker sim does not read it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<[f32; 3]>,
    /// sRGB multiply, including a textured glTF. None keeps the albedo
    /// (or the capsule tint). Dead / hurt still win.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u32; 3]>,
    /// PBR roughness. None keeps the historical 0.30 satin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roughness: Option<f32>,
}

fn default_idle_anim() -> String {
    "idle".into()
}

fn default_expression() -> String {
    "blink".into()
}

fn walker_type() -> String {
    "walker".into()
}

/// Light record (`light:0`). Stored; M3 lights/joints/TRS is not this slice.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldLight {
    pub id: String,
    #[serde(rename = "type", default = "light_type")]
    pub kind_type: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub slot: u32,
    #[serde(default)]
    pub intensity: f32,
    #[serde(default)]
    pub radius: f32,
    #[serde(default)]
    pub color: Option<[f32; 3]>,
    #[serde(default)]
    pub direction: Option<[f32; 3]>,
}

fn light_type() -> String {
    "light".into()
}

/// Camera record (`camera:main`). `fov` is degrees, matching Python `fov_deg`.
///
/// Dumps also write `screen_right` / `screen_left` (world-space unit vectors).
/// Those are derived from position→target; they are not stored.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct WorldCamera {
    pub id: String,
    #[serde(rename = "type", default = "camera_type")]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default)]
    pub target: [f32; 3],
    #[serde(default = "default_fov")]
    pub fov: f32,
    /// Dutch angle around the look axis, radians. Zero keeps world up.
    /// Omitted from dumps when it is flat. Steering does not read this.
    #[serde(default)]
    pub roll: f32,
}

/// One sentence an agent can read without guessing which world axis is which
/// side of the frame. Kept in the dump next to each camera's vectors.
pub const COORDINATE_NOTE: &str = "Right-handed, Y up. World +X is screen-right only when the camera looks toward -Z. The default camera sits at (0, 2, 6) looking at the origin, so +X is the right side of the frame and +Y is up. A chase camera looking along +Z (behind a yaw-0 actor; move-forward is +Z) has screen_right = -X and screen_left = +X. D, ArrowRight, and +lx mean screen-right, not world +X. Each camera's screen_right and screen_left are world-space unit vectors. 2D HUD pixels start at the top-left; +x is right and +y is down. glTF/VRM bind poses face -Z (the model's right hand is +X).";

#[derive(Serialize)]
struct WorldCameraDump<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    kind: &'a str,
    name: &'a str,
    position: [f32; 3],
    target: [f32; 3],
    fov: f32,
    #[serde(skip_serializing_if = "roll_is_flat")]
    roll: f32,
    screen_right: [f32; 3],
    screen_left: [f32; 3],
}

fn roll_is_flat(roll: &f32) -> bool {
    roll.abs() < 1e-5
}

/// World up, spun around the look axis. `roll` 0 is exactly +Y.
fn rolled_up(eye: Vec3, target: Vec3, roll: f32) -> Vec3 {
    if roll.abs() < 1e-6 {
        return Vec3::Y;
    }
    let mut forward = target - eye;
    if forward.length_squared() < 1e-8 {
        return Vec3::Y;
    }
    forward = forward.normalize();
    let up = Vec3::Y;
    let (sin, cos) = roll.sin_cos();
    let spun = up * cos + forward.cross(up) * sin + forward * forward.dot(up) * (1.0 - cos);
    if spun.length_squared() < 1e-8 {
        Vec3::Y
    } else {
        spun.normalize()
    }
}

fn json_axis(v: f32) -> f32 {
    if v == 0.0 {
        0.0
    } else {
        v
    }
}

fn json_vec3(v: Vec3) -> [f32; 3] {
    let a = v.to_array();
    [json_axis(a[0]), json_axis(a[1]), json_axis(a[2])]
}

impl Serialize for WorldCamera {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let eye = Vec3::from_array(self.position);
        let target = Vec3::from_array(self.target);
        let right = crate::scene3d::screen_basis(eye, target, rolled_up(eye, target, self.roll)).1;
        WorldCameraDump {
            id: &self.id,
            kind: &self.kind,
            name: &self.name,
            position: self.position,
            target: self.target,
            fov: self.fov,
            roll: self.roll,
            screen_right: json_vec3(right),
            screen_left: json_vec3(-right),
        }
        .serialize(serializer)
    }
}

fn camera_type() -> String {
    "camera".into()
}

/// Put `coordinates` at the top of a pretty-printed dump without re-encoding
/// the rest of the document.
fn insert_coordinate_note(body: &str) -> String {
    let coords = serde_json::json!({
        "handedness": "right",
        "up": [0.0, 1.0, 0.0],
        "note": COORDINATE_NOTE,
    });
    let pretty = serde_json::to_string_pretty(&coords).unwrap_or_else(|_| "{}".into());
    let mut block = String::from("\n  \"coordinates\": ");
    for (i, line) in pretty.lines().enumerate() {
        if i == 0 {
            block.push_str(line);
        } else {
            block.push_str("\n  ");
            block.push_str(line);
        }
    }
    let Some(rest) = body.strip_prefix('{') else {
        return body.to_string();
    };
    format!("{{{block},{rest}")
}

fn default_fov() -> f32 {
    30.0
}

/// Terrain tile key (`tile:ix,iz`). UV/streaming are names only this slice.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldTerrainTile {
    pub id: String,
    #[serde(rename = "type", default = "tile_type")]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ix: i32,
    #[serde(default)]
    pub iz: i32,
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default)]
    pub loaded: bool,
    #[serde(default)]
    pub has_mesh: bool,
    #[serde(default)]
    pub albedo_ok: bool,
}

fn tile_type() -> String {
    "terrain_tile".into()
}

/// Heightfield UV keys. Values are stored; Crest Isle streaming is not retuned.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldHeightfieldUv {
    #[serde(default)]
    pub half: Option<f32>,
    #[serde(default)]
    pub period: Option<f32>,
    #[serde(default)]
    pub blend: f32,
    #[serde(default)]
    pub pad: f32,
    #[serde(default)]
    pub rect: Option<[f32; 4]>,
}

/// Named height fn + samples + tile keys. Live Python fn cannot dump.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldHeightfield {
    #[serde(rename = "fn", default)]
    pub fn_name: Option<String>,
    #[serde(default)]
    pub tile: Option<f32>,
    #[serde(default)]
    pub stream_radius: Option<f32>,
    #[serde(default)]
    pub cells: Option<i32>,
    #[serde(default)]
    pub lod_radius: Option<f32>,
    #[serde(default)]
    pub lod_cells: Option<i32>,
    #[serde(default)]
    pub uv: Option<WorldHeightfieldUv>,
    #[serde(default)]
    pub tiles: Vec<WorldTerrainTile>,
    #[serde(default)]
    pub samples: Vec<[f32; 3]>,
}

impl WorldDoc {
    /// 0 = ACES, 1 = AgX, 2 = Khronos PBR Neutral.
    pub fn tonemap_code(&self) -> f32 {
        match self
            .tonemap_curve
            .as_deref()
            .map(str::trim)
            .map(|s| s.to_ascii_lowercase())
            .as_deref()
        {
            Some("agx") => 1.0,
            Some("neutral") => 2.0,
            _ => 0.0,
        }
    }

    /// True when `genre` is exactly `id`. Missing genre is never a match.
    pub fn genre_is(&self, id: &str) -> bool {
        self.genre.as_deref() == Some(id)
    }

    /// Ingest `World.dump()` JSON (`docs/schemas/world.json` version 1).
    pub fn from_json(json: &str) -> Result<Self, String> {
        let doc: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if doc.version != WORLD_DUMP_VERSION {
            return Err(format!(
                "unsupported world dump version {} (want {})",
                doc.version, WORLD_DUMP_VERSION
            ));
        }
        Ok(doc)
    }

    /// Emit world dump JSON. GPU mesh ids / draw batches are not written.
    ///
    /// Each camera gains `screen_right` / `screen_left`, and the document
    /// gains a `coordinates` note, so a reader does not have to guess which
    /// world axis is which side of the frame. Both are derived; `from_json`
    /// ignores them.
    pub fn to_json(&self) -> Result<String, String> {
        let mut out = self.clone();
        if out.version == 0 {
            out.version = WORLD_DUMP_VERSION;
        }
        // Serialize the struct directly so f32 text stays the same as before.
        // `coordinates` is spliced in afterwards; a trip through `Value` would
        // reprint every float.
        let body = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
        Ok(insert_coordinate_note(&body))
    }

    /// World dump plus a `profile` object (draws, triangles, texture bytes,
    /// budget warnings). CPU and GPU times stay empty until a renderer
    /// measured a frame and [`crate::profile::splice_profile`] is called
    /// with that profile instead.
    pub fn to_json_with_profile(&self) -> Result<String, String> {
        let dump = self.to_json()?;
        Ok(crate::profile::splice_profile(
            &dump,
            &crate::profile::FrameProfile::from_world(self),
        ))
    }

    /// Bloom intensity for `render_world_doc`. None stays off, except a dump
    /// tier of `mobile` (0.22) or `high` (0.35) while heavy post is allowed.
    /// Env high does not turn bloom on. An authored value is kept, including
    /// under the low floor.
    pub fn bloom_intensity(&self) -> f32 {
        if let Some(v) = self.bloom {
            return v.max(0.0);
        }
        let env = std::env::var("KAGRA_QUALITY").ok();
        if !heavy_post_allowed_value(env.as_deref()) {
            return 0.0;
        }
        match authored_tier(self.quality.as_deref()) {
            Some(PictureTier::Mobile) => 0.22,
            Some(PictureTier::High) => 0.35,
            _ => 0.0,
        }
    }

    /// Bloom extract threshold. None = 0.85.
    pub fn bloom_threshold(&self) -> f32 {
        self.bloom_threshold.unwrap_or(0.85).clamp(0.0, 2.0)
    }

    /// Vignette mix. None = 1, the curve that shipped with the bloom composite.
    pub fn vignette_strength(&self) -> f32 {
        self.vignette.unwrap_or(1.0).clamp(0.0, 2.0)
    }

    /// LUT preset name. Empty / missing / `off` skip the grade.
    pub fn lut_preset(&self) -> &str {
        match self.lut.as_deref().map(str::trim) {
            None | Some("") | Some("off") => "off",
            Some(name) => name,
        }
    }

    /// 0 when the preset is off, otherwise `lut_strength` or 1.
    pub fn lut_mix(&self) -> f32 {
        if self.lut_preset() == "off" {
            0.0
        } else {
            self.lut_strength.unwrap_or(1.0).clamp(0.0, 1.0)
        }
    }

    /// FXAA. None = on.
    pub fn fxaa_enabled(&self) -> bool {
        self.fxaa.unwrap_or(true)
    }

    /// SSAO radius in pixels. Omitted fields follow the tier. Low forces 0.
    pub fn ssao_radius(&self) -> f32 {
        let authored = self.ssao.map(|r| r.clamp(0.0, 64.0));
        match picture_tier(self.quality.as_deref()) {
            PictureTier::Low => 0.0,
            PictureTier::Mobile => authored.unwrap_or(4.0),
            PictureTier::High => authored.unwrap_or(8.0),
            PictureTier::Legacy => authored.unwrap_or(0.0),
        }
    }

    /// TAA. Mobile keeps it off unless the dump sets it. Low forces it off.
    pub fn taa_enabled(&self) -> bool {
        match picture_tier(self.quality.as_deref()) {
            PictureTier::Low => false,
            PictureTier::Mobile | PictureTier::Legacy => self.taa.unwrap_or(false),
            PictureTier::High => self.taa.unwrap_or(true),
        }
    }

    /// `(focus, aperture)` when DOF should run. Tiers do not turn it on.
    pub fn dof(&self) -> Option<(f32, f32)> {
        if matches!(picture_tier(self.quality.as_deref()), PictureTier::Low) {
            return None;
        }
        let aperture = self.dof_aperture.unwrap_or(0.0);
        if aperture <= 1e-4 {
            return None;
        }
        Some((
            self.dof_focus.unwrap_or(8.0).max(0.05),
            aperture.clamp(0.0, 8.0),
        ))
    }

    /// Camera shutter. None = 0. Tiers do not turn it on. Low forces 0.
    pub fn motion_shutter(&self) -> f32 {
        if matches!(picture_tier(self.quality.as_deref()), PictureTier::Low) {
            return 0.0;
        }
        self.motion_blur.unwrap_or(0.0).clamp(0.0, 1.0)
    }

    /// Shadow kernel radius. None = 1 (the 3×3 PCF that already shipped).
    pub fn shadow_softness(&self) -> f32 {
        self.shadow_softness.unwrap_or(1.0).clamp(0.25, 8.0)
    }

    /// Cascades in the one sun map. Mobile fills 2, high fills 3. Low forces 1.
    pub fn shadow_cascade_count(&self) -> u32 {
        let authored = self.shadow_cascades.map(|n| n.clamp(1, 3));
        match picture_tier(self.quality.as_deref()) {
            PictureTier::Low => 1,
            PictureTier::Mobile => authored.unwrap_or(2),
            PictureTier::High => authored.unwrap_or(3),
            PictureTier::Legacy => authored.unwrap_or(1),
        }
    }

    /// PCSS blocker search. High fills it on. Mobile and legacy leave it off.
    /// Low forces it off.
    pub fn shadow_pcss_enabled(&self) -> bool {
        match picture_tier(self.quality.as_deref()) {
            PictureTier::Low => false,
            PictureTier::Mobile | PictureTier::Legacy => self.shadow_pcss.unwrap_or(false),
            PictureTier::High => self.shadow_pcss.unwrap_or(true),
        }
    }

    /// Studio blue when the dump omits `hemi_sky`.
    pub fn hemi_sky_color(&self) -> [f32; 3] {
        self.hemi_sky
            .unwrap_or([0.48, 0.56, 0.78])
            .map(|c| c.clamp(0.0, 4.0))
    }

    /// Studio warm ground when the dump omits `hemi_ground`.
    pub fn hemi_ground_color(&self) -> [f32; 3] {
        self.hemi_ground
            .unwrap_or([0.42, 0.30, 0.18])
            .map(|c| c.clamp(0.0, 4.0))
    }

    /// 64×64 prefiltered strip, or None when `env_map` is omitted.
    pub fn baked_env(&self) -> Option<Vec<u8>> {
        let spec = self
            .env_map
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())?;
        // The strip is a pure function of these three, and `compile_scene` asks for it every
        // frame (it was re-baked, a powf per texel and three blurs, and re-read from disk for
        // a PNG). Keep the last answer.
        let (sky, ground) = (self.hemi_sky_color(), self.hemi_ground_color());
        type Key = (String, [f32; 3], [f32; 3]);
        thread_local! {
            static LAST: std::cell::RefCell<Option<(Key, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
        }
        let key: Key = (spec.to_string(), sky, ground);
        if let Some(hit) = LAST.with(|l| {
            l.borrow()
                .as_ref()
                .filter(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        }) {
            return Some(hit);
        }
        let strip = bake_env_strip(
            sky,
            ground,
            if spec == "preset:studio" {
                None
            } else {
                load_png_albedo(spec)
            },
        );
        LAST.with(|l| *l.borrow_mut() = Some((key, strip.clone())));
        Some(strip)
    }

    /// Probe walker `gltf` paths. Missing/parse failures are dump-visible;
    /// `compile_scene` still draws a skinned tpose, never a silent capsule.
    pub fn refresh_asset_status(&mut self) {
        let mut seen = HashSet::new();
        let mut errs: Vec<(String, Option<String>)> = Vec::new();
        for w in self.walkers.iter().chain(self.player.iter()) {
            if !seen.insert(w.id.clone()) {
                continue;
            }
            let err = walker_gltf_spec(w).and_then(walker_asset_error);
            errs.push((w.id.clone(), err));
        }
        for (id, err) in errs {
            for w in &mut self.walkers {
                if w.id == id {
                    w.load_error = err.clone();
                }
            }
            if let Some(w) = self.player.as_mut() {
                if w.id == id {
                    w.load_error = err.clone();
                }
            }
        }
    }

    /// Stable string ids in dump order (props, walkers, lights, cameras, tiles).
    pub fn stable_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        ids.extend(self.props.iter().map(|p| p.id.clone()));
        ids.extend(self.walkers.iter().map(|w| w.id.clone()));
        ids.extend(self.lights.iter().map(|l| l.id.clone()));
        ids.extend(self.cameras.iter().map(|c| c.id.clone()));
        if let Some(hf) = &self.heightfield {
            ids.extend(hf.tiles.iter().map(|t| t.id.clone()));
        }
        ids
    }

    /// World matrix per prop id. `local: true` multiplies up to 4 parents.
    /// A disabled prop still contributes a transform so it can be a hinge.
    fn prop_world_mats(&self) -> HashMap<String, Mat4> {
        let mut memo = HashMap::new();
        let mut stack = Vec::new();
        for prop in &self.props {
            self.prop_world_mat(&prop.id, 0, &mut stack, &mut memo);
        }
        memo
    }

    fn prop_world_mat(
        &self,
        id: &str,
        depth: u32,
        stack: &mut Vec<String>,
        memo: &mut HashMap<String, Mat4>,
    ) -> Mat4 {
        if let Some(ready) = memo.get(id) {
            return *ready;
        }
        let Some(prop) = self.props.iter().find(|p| p.id == id) else {
            return Mat4::IDENTITY;
        };
        let local = prop_local_matrix(prop);
        let world = if prop.local && depth < 4 {
            match prop
                .parent
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Some(_) if stack.iter().any(|seen| seen == id) => local,
                Some(parent) => {
                    stack.push(id.to_string());
                    let parent_m = self.prop_world_mat(parent, depth + 1, stack, memo);
                    stack.pop();
                    parent_m * local
                }
                None => local,
            }
        } else {
            local
        };
        memo.insert(id.to_string(), world);
        world
    }

    /// Local bounds of every mesh, for frustum culling. `compile_meshes` rebuilds every
    /// primitive and re-reads and decodes every glTF and its textures, which cost ~120 ms a
    /// call on a doc with 17 models, and `compile_scene` runs once per frame. So the bounds
    /// of a doc whose meshes cannot change between frames (no heightfield, no skinned
    /// walker pose) are kept per mesh key (`gpu_mesh_key`, a few microseconds) and rebuilt
    /// only when that key changes.
    fn cull_bounds(&self) -> Vec<(MeshId, Aabb)> {
        let build = |doc: &WorldDoc| -> Vec<(MeshId, Aabb)> {
            doc.compile_meshes()
                .into_iter()
                .filter_map(|(id, mesh)| {
                    let bounds = mesh.bounds();
                    (!bounds.is_empty()).then_some((id, bounds))
                })
                .collect()
        };
        let steady = self.heightfield.is_none()
            && !self
                .gltf_slots()
                .iter()
                .any(|s| matches!(s, GltfSlot::Skinned { .. }));
        if !steady {
            return build(self);
        }
        type Entry = (String, Vec<(MeshId, Aabb)>);
        thread_local! {
            static CACHE: std::cell::RefCell<Vec<Entry>> = const { std::cell::RefCell::new(Vec::new()) };
        }
        let key = self.gpu_mesh_key();
        if let Some(hit) = CACHE.with(|c| {
            c.borrow()
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        }) {
            return hit;
        }
        let built = build(self);
        CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if c.len() >= 4 {
                c.remove(0);
            }
            c.push((key, built.clone()));
        });
        built
    }

    /// One-frame draw list. Heightfield + glTF/box/sphere/capsule + sprite/quad.
    /// Empty lights get default key+fill (max 4 slots). Capsules/props get a
    /// ground contact blob. Does not mutate this document. GPU upload uses `compile_meshes`.
    pub fn compile_scene(&self, aspect: f32) -> Scene3D {
        let camera = self.draw_camera();
        let mut b = SceneBuilder::new(&camera, aspect.max(1e-3));
        // Opt-in. An omitted `cull` stays unregistered so every prop still draws.
        if self.cull == Some(true) {
            for (id, bounds) in self.cull_bounds() {
                b.register(id, bounds);
            }
        }
        let (gltf_ids, walker_gltf_ids) = self.gltf_mesh_ids();
        let tex_ids = self.texture_mesh_ids();

        let sky = self.sky.unwrap_or([130, 165, 205, 255]);
        b.push_material(
            MESH_SPHERE,
            Mat4::from_scale_rotation_translation(Vec3::splat(360.0), Quat::IDENTITY, camera.eye),
            sky,
            Material::Sky,
        );
        if self.heightfield.is_some() {
            b.push_material(
                MESH_HEIGHTFIELD,
                Mat4::IDENTITY,
                [78, 138, 64, 255],
                Material::Grass,
            );
        } else if self.ground.unwrap_or(true) {
            // Dump without heightfield/water/props is otherwise a clear-color void.
            // Indoor rooms set `ground: false` and bring their own floor.
            let span = (self.half.abs() * 2.0).max(8.0);
            b.push_material(
                MESH_PLANE,
                Mat4::from_scale_rotation_translation(
                    Vec3::new(span, 1.0, span),
                    Quat::IDENTITY,
                    Vec3::new(0.0, self.floor_y, 0.0),
                ),
                [78, 138, 64, 255],
                Material::Grass,
            );
        }

        if let Some(wy) = self.water_y {
            let span = (self.half * 2.0).max(8.0);
            b.push_material(
                MESH_PLANE,
                Mat4::from_scale_rotation_translation(
                    Vec3::new(span, 1.0, span),
                    Quat::IDENTITY,
                    Vec3::new(0.0, wy - 0.04, 0.0),
                ),
                [42, 92, 110, 255],
                Material::Water,
            );
        }

        let lod_r = self.vegetation_lod_radius();
        let prop_lod = self.prop_lod.filter(|r| *r > 0.0);
        // Parent links are rare. Without one, a prop's world matrix is its own local one, so
        // skip the id-keyed map (it cloned and hashed every id, disabled pool slots included,
        // and looked each one up by a linear search: a few ms a frame on a phone).
        let worlds = self
            .props
            .iter()
            .any(|p| p.local)
            .then(|| self.prop_world_mats());
        for prop in &self.props {
            if !prop.enabled {
                continue;
            }
            let model = match &worlds {
                Some(w) => w.get(&prop.id).copied().unwrap_or(Mat4::IDENTITY),
                None => prop_local_matrix(prop),
            };
            let pos = model.transform_point3(Vec3::ZERO);
            let scale = Vec3::from_array(prop.scale);
            let mat = material_for_prop(prop);
            let veg = is_vegetation_prop(prop);
            let cheap = veg && camera.eye.distance(pos) > lod_r;
            if cheap {
                let h = scale.y.abs().max(scale.x.abs()).max(0.4);
                let billboard = Mat4::from_scale_rotation_translation(
                    Vec3::new(h * 0.7, h, 1.0),
                    Quat::from_rotation_y(billboard_yaw(camera.eye, pos)),
                    pos,
                );
                b.push_material(MESH_QUAD, billboard, color_u8(prop.color), mat);
                continue;
            }
            if let Some(radius) = prop_lod {
                if !veg && camera.eye.distance(pos) > radius {
                    let h = scale.y.abs().max(scale.x.abs()).max(0.35);
                    let card = Mat4::from_scale_rotation_translation(
                        Vec3::new(h * 0.7, h, 1.0),
                        Quat::from_rotation_y(billboard_yaw(camera.eye, pos)),
                        pos,
                    );
                    b.push_material(MESH_QUAD, card, color_u8(prop.color), Material::Solid);
                    continue;
                }
            }
            let mesh = mesh_for_prop(prop, &gltf_ids, &tex_ids);
            let has_tex = prop
                .texture
                .as_deref()
                .map(str::trim)
                .is_some_and(|s| !s.is_empty())
                || prop
                    .gltf
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|s| !s.is_empty());
            let col = if has_tex && prop.color.is_none() {
                [255, 255, 255, 255]
            } else {
                color_u8(prop.color)
            };
            let (metallic, roughness) = if is_coin_prop(prop) && prop.metallic < 0.01 {
                (1.0, 0.12)
            } else {
                // Default roughness in WorldProp is 1.0 when omitted in JSON;
                // map default 1.0 on non-vegetation props to a natural satin 0.38
                // unless explicitly set to another value.
                let r = if (prop.roughness - 1.0).abs() < 1e-4 && !veg {
                    0.38
                } else {
                    prop.roughness
                };
                (prop.metallic, r)
            };
            let mut inst = Instance::with_pbr(model, col, mat, metallic, roughness);
            inst.emissive = prop.emissive.unwrap_or(0.0).max(0.0);
            if let Some(c) = prop.emissive_color {
                inst.emissive_color = c;
            }
            inst.emissive_authored = prop.emissive.is_some() || prop.emissive_color.is_some();
            inst.clearcoat = prop.clearcoat.clamp(0.0, 1.0);
            inst.clearcoat_roughness = prop.clearcoat_roughness.clamp(0.0, 1.0);
            b.push_instance(mesh, inst);
        }

        self.push_vista(&mut b, &camera);

        let mut seen = std::collections::HashSet::new();
        let hide_local = self
            .cameras
            .first()
            .map(|c| c.name == "eye")
            .unwrap_or(false);
        let local_id = self.player.as_ref().map(|p| p.id.as_str());
        for walk in self.walkers.iter().chain(self.player.iter()) {
            if !seen.insert(walk.id.as_str()) {
                continue;
            }
            // First-person: camera sits in the capsule. glTF（VRM）なら
            // firstPerson 注釈でパーツをフィルタするためスキップしない
            // （下の gltf 分岐で ThirdPersonOnly/Auto を隠す）。カプセル等
            // glTF なしは全体を隠す。
            if hide_local && local_id == Some(walk.id.as_str()) && walker_gltf_spec(walk).is_none()
            {
                continue;
            }
            // Named glTF (dump `gltf` / `model`) is CPU-skinned at `clip` and
            // drawn at walker pose. `model: capsule` must not win when `gltf`
            // is set. Capsule + head remains the fallback only if no glTF spec.
            // Action genre: name "hurt" / "dead" is dump-visible and tinted here.
            // Action boss waits off-stage until minions fall (`name == "wait"`).
            if walk.name == "wait" {
                continue;
            }
            let pos = Vec3::from_array(walk.position);
            let yaw = Quat::from_rotation_y(walk.yaw);
            let sc = Vec3::from_array(walk.scale.unwrap_or([1.0, 1.0, 1.0]));
            let dead = walk.name == "dead" || walk.name == "ko";
            let hurt = walk.name == "hurt";
            let body_col = if dead {
                [70, 74, 82, 255]
            } else if hurt {
                [220, 64, 64, 255]
            } else if let Some(tint) = walk.color {
                [
                    tint[0].min(255) as u8,
                    tint[1].min(255) as u8,
                    tint[2].min(255) as u8,
                    255,
                ]
            } else {
                [62, 176, 184, 255]
            };
            let walker_rough = walk.roughness.unwrap_or(0.30).clamp(0.04, 1.0);
            if let Some(meshes) = walker_gltf_ids.get(&walk.id) {
                let model = Mat4::from_scale_rotation_translation(sc, yaw, pos);
                let col = if dead || hurt || walk.color.is_some() {
                    body_col
                } else if walker_gltf_spec(walk)
                    .map(walker_spec_has_albedo)
                    .unwrap_or(false)
                {
                    [255, 255, 255, 255]
                } else {
                    body_col
                };
                let mat = if walker_gltf_spec(walk)
                    .map(walker_spec_has_mtoon)
                    .unwrap_or(false)
                {
                    Material::Toon
                } else {
                    Material::Solid
                };
                for (pi, &mesh) in meshes.iter().enumerate() {
                    // 一人称（"eye"）: ThirdPersonOnly / Auto パーツを隠す。
                    // firstPerson 注釈が読める glTF だけフィルタし、読めない
                    // （カプセル等）は従来どおり全体を隠す（上で continue）。
                    if hide_local && local_id == Some(walk.id.as_str()) {
                        let hidden = walker_gltf_spec(walk)
                            .and_then(load_skinned_parts)
                            .and_then(|parts| parts.get(pi).cloned())
                            .map(|part| crate::gltf_load::part_hidden_in_first_person(&part))
                            .unwrap_or(true);
                        if hidden {
                            continue;
                        }
                    }
                    b.push_pbr(mesh, model, col, mat, 0.08, walker_rough);
                }
                continue;
            }
            let body_h = if dead { 0.28 } else { 0.95 };
            let body = Mat4::from_scale_rotation_translation(
                Vec3::new(0.56 * sc.x, body_h * sc.y, 0.56 * sc.z),
                yaw,
                pos,
            );
            b.push(MESH_CAPSULE, body, body_col);
            let head_lift = if dead { 0.18 } else { 0.62 };
            let head = Mat4::from_scale_rotation_translation(
                Vec3::new(0.38 * sc.x, 0.32 * sc.y, 0.38 * sc.z),
                yaw,
                pos + Vec3::Y * head_lift * sc.y,
            );
            let head_col = if dead {
                [90, 86, 82, 255]
            } else {
                [236, 214, 176, 255]
            };
            b.push(MESH_BOX, head, head_col);
        }

        if self.contact_blobs != Some(false) {
            self.push_contact_blobs(&mut b, hide_local, local_id);
        }

        let (light_dir, ambient, local_lights) = self.draw_lights();
        let sky = self.sky.unwrap_or([130, 165, 205, 255]);
        Scene3D {
            camera,
            clear: sky,
            light_dir,
            ambient,
            local_lights,
            fog_color: sky,
            fog_start: self.fog_start.unwrap_or(48.0),
            fog_end: self.fog_end.unwrap_or(220.0),
            line_fade: self.line_fade.unwrap_or([0.0, 0.0]),
            ibl: self.ibl.unwrap_or(0.35),
            exposure: self.exposure.unwrap_or(1.0),
            tonemap: self.tonemap.unwrap_or(true),
            tonemap_code: self.tonemap_code(),
            sky_atmosphere: self.sky_atmosphere.unwrap_or(false),
            shadow_softness: self.shadow_softness(),
            shadow_cascades: self.shadow_cascade_count(),
            shadow_pcss: self.shadow_pcss_enabled(),
            hemi_sky: self.hemi_sky_color(),
            hemi_ground: self.hemi_ground_color(),
            env_pixels: self.baked_env(),
            points: self.points.clone(),
            lines: self.lines.clone(),
            batches: b.finish(),
        }
    }

    fn draw_camera(&self) -> Camera {
        if let Some(cam) = self.cameras.first() {
            let eye = Vec3::from_array(cam.position);
            let target = Vec3::from_array(cam.target);
            return Camera {
                eye,
                target,
                up: rolled_up(eye, target, cam.roll),
                fov_y: cam.fov.to_radians(),
                near: 0.2,
                far: 400.0,
            };
        }
        if let Some(p) = self.player.as_ref().or(self.walkers.first()) {
            let pos = Vec3::from_array(p.position);
            return Camera {
                eye: pos + Vec3::new(0.0, 4.4, 12.2),
                target: pos + Vec3::Y * 1.25,
                up: Vec3::Y,
                fov_y: 54f32.to_radians(),
                near: 0.2,
                far: 220.0,
            };
        }
        Camera::default()
    }

    fn draw_lights(&self) -> (Vec3, f32, [LocalLight; 4]) {
        // Empty dump: still light the shared picture (key+fill). Occupied
        // slots stay 1:1; unused stay OFF (max 4 indoor slots, no leak).
        if self.lights.is_empty() {
            let local = self.default_key_fill();
            let sun = if local[0].direction.length_squared() > 1e-8 {
                (-local[0].direction.normalize(), 0.42)
            } else {
                (Vec3::new(-0.4, 1.0, 0.3).normalize(), 0.35)
            };
            return (sun.0, sun.1, local);
        }
        let mut local = [LocalLight::OFF; 4];
        for lit in &self.lights {
            if lit.slot > 3 {
                continue;
            }
            let color = lit.color.unwrap_or([1.0, 0.96, 0.88]);
            let dir = lit.direction.map(Vec3::from_array).unwrap_or(Vec3::ZERO);
            local[lit.slot as usize] = LocalLight {
                position: Vec3::from_array(lit.position),
                direction: dir,
                color,
                intensity: lit.intensity.max(0.0),
                radius: lit.radius.max(0.0),
                spot: lit.kind.eq_ignore_ascii_case("spot"),
            };
        }
        let sun = if local[0].intensity > 1e-5 {
            if local[0].direction.length_squared() > 1e-8 {
                (-local[0].direction.normalize(), 0.42)
            } else if local[0].position.length_squared() > 1e-8 {
                (local[0].position.normalize(), 0.42)
            } else {
                (Vec3::new(-0.4, 1.0, 0.3).normalize(), 0.35)
            }
        } else {
            self.draw_light()
        };
        (sun.0, sun.1, local)
    }

    /// Indoor key (slot 0) + cool fill (slot 1). Slots 2 and 3 stay OFF.
    fn default_key_fill(&self) -> [LocalLight; 4] {
        let a = self.scene_anchor();
        let mut local = [LocalLight::OFF; 4];
        local[0] = LocalLight {
            position: a + Vec3::new(6.0, 16.0, -8.0),
            direction: Vec3::new(-0.18, -1.0, 0.22),
            color: [1.0, 0.96, 0.86],
            intensity: 1.15,
            radius: 36.0,
            spot: true,
        };
        local[1] = LocalLight {
            position: a + Vec3::new(-12.0, 8.0, 6.0),
            direction: Vec3::ZERO,
            color: [0.55, 0.72, 1.0],
            intensity: 0.45,
            radius: 28.0,
            spot: false,
        };
        local
    }

    fn scene_anchor(&self) -> Vec3 {
        if let Some(p) = self.player.as_ref().or(self.walkers.first()) {
            return Vec3::from_array(p.position);
        }
        if let Some(c) = self.cameras.first() {
            return Vec3::from_array(c.target);
        }
        Vec3::ZERO
    }

    /// Ground contact disc (MESH_PLANE + instance alpha). Shared picture,
    /// not a second shadow pass / SSAO / V2 umbra.
    fn push_contact_blobs(&self, b: &mut SceneBuilder, hide_local: bool, local_id: Option<&str>) {
        let mut seen = HashSet::new();
        for walk in self.walkers.iter().chain(self.player.iter()) {
            if !seen.insert(walk.id.as_str()) {
                continue;
            }
            if hide_local && local_id == Some(walk.id.as_str()) {
                continue;
            }
            let pos = Vec3::from_array(walk.position);
            self.push_contact_blob(b, pos.x, pos.z, 0.62);
        }
        for prop in &self.props {
            if !prop.enabled {
                continue;
            }
            if prop.model.eq_ignore_ascii_case("plane")
                || prop.model.eq_ignore_ascii_case("water")
                || prop.name.eq_ignore_ascii_case("water")
                || is_vegetation_prop(prop)
            {
                continue;
            }
            if prop.local
                || !prop.gltf.is_none() && prop.name == "plane"
                || prop.model.eq_ignore_ascii_case("torus")
                || prop.model.eq_ignore_ascii_case("ring")
                || prop.name.eq_ignore_ascii_case("cloud")
                || prop.name.eq_ignore_ascii_case("ring")
                || prop.name.eq_ignore_ascii_case("plane")
                || prop.name.eq_ignore_ascii_case("tile")
                || prop.name.eq_ignore_ascii_case("studio_floor")
                || prop.name.eq_ignore_ascii_case("board_frame")
                || prop.name.eq_ignore_ascii_case("brass_trim")
                || prop.name.eq_ignore_ascii_case("backdrop")
                || prop.name.eq_ignore_ascii_case("lane")
                || prop.name.eq_ignore_ascii_case("marker")
            {
                continue;
            }
            let pos = Vec3::from_array(prop.position);
            let ground_y = self.height_at(pos.x, pos.z);
            if (pos.y - ground_y) > 2.5 && !prop.name.starts_with("piece_") {
                continue;
            }
            let sx = prop.scale[0].abs().max(0.2);
            let sz = prop.scale[2].abs().max(0.2);
            let is_chess_piece = prop.name.starts_with("piece_");
            if is_chess_piece {
                // Tight ambient-occlusion ring under the piece base + soft directional cast shadow on the board tile
                self.push_contact_blob_alpha(b, pos.x, 0.033, pos.z, 0.23, 168);
                self.push_contact_blob_alpha(b, pos.x - 0.055, 0.0318, pos.z - 0.045, 0.25, 118);
                continue;
            }
            let radius = (sx.max(sz) * 0.7).clamp(0.28, 2.4);
            let by = self.height_at(pos.x, pos.z) + 0.03;
            self.push_contact_blob_at(b, pos.x, by, pos.z, radius);
        }
    }

    fn push_contact_blob(&self, b: &mut SceneBuilder, x: f32, z: f32, radius: f32) {
        let y = self.height_at(x, z) + 0.03;
        self.push_contact_blob_at(b, x, y, z, radius);
    }

    fn push_contact_blob_at(&self, b: &mut SceneBuilder, x: f32, y: f32, z: f32, radius: f32) {
        self.push_contact_blob_alpha(b, x, y, z, radius, 135);
    }

    fn push_contact_blob_alpha(
        &self,
        b: &mut SceneBuilder,
        x: f32,
        y: f32,
        z: f32,
        radius: f32,
        alpha: u8,
    ) {
        let model = Mat4::from_scale_rotation_translation(
            Vec3::new(radius * 2.0, 1.0, radius * 2.0),
            Quat::IDENTITY,
            Vec3::new(x, y, z),
        );
        b.push_material(MESH_PLANE, model, [12, 10, 14, alpha], Material::Solid);
    }

    fn draw_light(&self) -> (Vec3, f32) {
        if let Some(lit) = self.lights.first() {
            if let Some(dir) = lit.direction {
                let v = Vec3::from_array(dir);
                if v.length_squared() > 1e-8 {
                    return (-v.normalize(), 0.42);
                }
            }
            let pos = Vec3::from_array(lit.position);
            if pos.length_squared() > 1e-8 {
                return (pos.normalize(), 0.42);
            }
        }
        (Vec3::new(-0.4, 1.0, 0.3).normalize(), 0.35)
    }

    /// Named demo fn, else nearest dump sample, else `floor_y`.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        if let Some(hf) = &self.heightfield {
            if let Some(name) = hf.fn_name.as_deref() {
                match name {
                    "open_world_height" => return open_world_height(x, z),
                    "island_height" => return island_height(x, z),
                    "overworld_height" => return overworld_height(x, z),
                    _ => {}
                }
            }
            if !hf.samples.is_empty() {
                return nearest_sample(&hf.samples, x, z);
            }
        }
        self.floor_y
    }

    /// Primitive slots 0..3, heightfield (4), sprite/quad (5), then one mesh per unique glTF.
    pub fn compile_meshes(&self) -> Vec<(MeshId, MeshData)> {
        let mut out = compile_meshes();
        out.push((MESH_HEIGHTFIELD, self.heightfield_mesh()));
        let cone_segs = self.vegetation_lod_cells();
        out.push((MESH_CONE, primitives::cone_mesh(0.5, 1.0, cone_segs)));
        out.push((MESH_TORUS, primitives::torus_mesh(0.5, 0.075, 48, 20)));
        for (i, slot) in self.gltf_slots().into_iter().enumerate() {
            let mesh = match &slot {
                GltfSlot::Rest(spec) => gltf_mesh_for(spec).map(|mut mesh| {
                    self.attach_surface_maps(&mut mesh, spec);
                    if let Some(l) = &self.mesh_lod {
                        if l.models.iter().any(|m| spec.contains(m.as_str())) {
                            decimate_mesh(&mut mesh, l.keep);
                        }
                    }
                    mesh
                }),
                GltfSlot::Skinned {
                    spec,
                    part,
                    clip,
                    hair,
                    morph,
                    expression,
                    look_yaw,
                    look_pitch,
                    anim_blend,
                    overlay_bones,
                    overlay_weight,
                    anim,
                    clip_name,
                    fade_from,
                    fade_from_clip,
                    fade_from_weight,
                    ..
                } => gltf_skinned_mesh_for(
                    spec,
                    *part,
                    crate::gltf_load::WalkerPose {
                        clip: if *clip > 0.0 { Some(*clip) } else { None },
                        hair: *hair,
                        expression: expression.clone(),
                        morph: *morph,
                        look_yaw: *look_yaw,
                        look_pitch: *look_pitch,
                        anim_blend: *anim_blend,
                        overlay_bones: overlay_bones.clone(),
                        overlay_weight: *overlay_weight,
                        anim: anim.clone(),
                        clip_name: clip_name.clone(),
                        fade_from: fade_from.clone(),
                        fade_from_clip: *fade_from_clip,
                        fade_from_weight: *fade_from_weight,
                    },
                )
                .or_else(|| {
                    if *part == 0 {
                        walker_tpose_mesh(*clip, *hair, *morph, *look_yaw, *look_pitch)
                    } else {
                        None
                    }
                }),
            }
            .unwrap_or_else(|| primitives::box_mesh(Vec3::ONE));
            out.push((MeshId(MESH_GLTF_BASE + i as u32), mesh));
        }
        let tex_base = MESH_GLTF_BASE + self.gltf_slots().len() as u32;
        for (i, prop) in self.textured_props().into_iter().enumerate() {
            if let Some(mesh) = textured_prop_mesh(prop) {
                out.push((MeshId(tex_base + i as u32), mesh));
            }
        }
        out
    }

    fn heightfield_mesh(&self) -> MeshData {
        if self.heightfield.is_none() {
            return primitives::plane_mesh(1.0, 1.0);
        }
        let half = self.half.max(4.0);
        // Production island mesh (not a placeholder plane). 48 cells over
        // Crest's 160 m span is ~3.3 m — readable hills, not a billboard.
        // Dump cells/lod_cells are Python 16 m tile tessellation (Crest 8/6);
        // they drive vegetation LOD, not this island mesh (Relic Run UV stays).
        let cells = 48u32;
        let step = (half * 2.0) / cells as f32;
        let mut mesh = MeshData::default();
        for iz in 0..=cells {
            for ix in 0..=cells {
                let x = -half + ix as f32 * step;
                let z = -half + iz as f32 * step;
                let y = self.height_at(x, z);
                let dx = (self.height_at(x + step, z) - self.height_at(x - step, z)) / (2.0 * step);
                let dz = (self.height_at(x, z + step) - self.height_at(x, z - step)) / (2.0 * step);
                let n = Vec3::new(-dx, 1.0, -dz).normalize_or(Vec3::Y);
                mesh.vertices.push(Vertex3::new(Vec3::new(x, y, z), n));
            }
        }
        let stride = cells + 1;
        for iz in 0..cells {
            for ix in 0..cells {
                let i = iz * stride + ix;
                mesh.indices.extend_from_slice(&[
                    i,
                    i + stride,
                    i + 1,
                    i + 1,
                    i + stride,
                    i + stride + 1,
                ]);
            }
        }
        mesh
    }

    /// Stable while a walker only changes pose. A different glTF or texture
    /// set must reupload; clip / hair / look stay on the same slots.
    pub fn gpu_mesh_key(&self) -> String {
        let mut parts = Vec::new();
        for slot in self.gltf_slots() {
            match slot {
                GltfSlot::Rest(spec) => parts.push(format!("r:{spec}")),
                GltfSlot::Skinned { spec, part, .. } => parts.push(format!("s:{spec}:{part}")),
            }
        }
        for prop in self.textured_props() {
            let tex = prop_texture_path(prop).unwrap_or("");
            parts.push(format!("t:{}:{tex}", prop.id));
        }
        for prop in &self.props {
            if prop_has_maps(prop) {
                parts.push(format!(
                    "map:{}:{}:{}:{}",
                    prop.id,
                    prop.roughness_map.as_deref().unwrap_or(""),
                    prop.metalness_map.as_deref().unwrap_or(""),
                    prop.emissive_map.as_deref().unwrap_or(""),
                ));
            }
        }
        parts.push(format!("lod:{}", self.vegetation_lod_cells()));
        if let Some(l) = &self.mesh_lod {
            parts.push(format!("mlod:{}:{}", l.keep, l.models.join(",")));
        }
        parts.join("|")
    }

    pub(crate) fn gltf_slots(&self) -> Vec<GltfSlot> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for prop in &self.props {
            if !prop.enabled {
                continue;
            }
            let Some(raw) = prop.gltf.as_deref() else {
                continue;
            };
            let spec = raw.trim();
            if spec.is_empty() {
                continue;
            }
            if seen.insert(spec.to_string()) {
                out.push(GltfSlot::Rest(spec.to_string()));
            }
        }
        let mut seen_w = HashSet::new();
        for walk in self.walkers.iter().chain(self.player.iter()) {
            if !seen_w.insert(walk.id.as_str()) {
                continue;
            }
            if let Some(spec) = walker_gltf_spec(walk) {
                let n = skinned_part_count(spec);
                for part in 0..n {
                    out.push(GltfSlot::Skinned {
                        walker_id: walk.id.clone(),
                        spec: spec.to_string(),
                        part,
                        clip: walk.clip,
                        hair: walk.hair,
                        morph: walk.morph,
                        expression: walk.expression.clone(),
                        look_yaw: walk.look_yaw,
                        look_pitch: walk.look_pitch,
                        anim_blend: walk.anim_blend,
                        overlay_bones: walk.overlay_bones.clone(),
                        overlay_weight: walk.overlay_weight,
                        anim: walk.anim.clone(),
                        clip_name: walk.clip_name.clone(),
                        fade_from: walk.fade_from.clone(),
                        fade_from_clip: walk.fade_from_clip,
                        fade_from_weight: walk.fade_from_weight,
                    });
                }
            }
        }
        out
    }

    fn gltf_mesh_ids(&self) -> (HashMap<String, MeshId>, HashMap<String, Vec<MeshId>>) {
        let mut props = HashMap::new();
        let mut walkers: HashMap<String, Vec<MeshId>> = HashMap::new();
        for (i, slot) in self.gltf_slots().into_iter().enumerate() {
            let id = MeshId(MESH_GLTF_BASE + i as u32);
            match slot {
                GltfSlot::Rest(spec) => {
                    props.insert(spec, id);
                }
                GltfSlot::Skinned { walker_id, .. } => {
                    walkers.entry(walker_id).or_default().push(id);
                }
            }
        }
        (props, walkers)
    }

    fn textured_props(&self) -> Vec<&WorldProp> {
        self.props
            .iter()
            .filter(|p| {
                if !p.enabled {
                    return false;
                }
                if prop_texture_path(p).is_some() {
                    return true;
                }
                // Maps on a glTF ride that mesh. A bare primitive needs its own slot.
                prop_gltf_spec(p).is_none() && prop_has_maps(p)
            })
            .collect()
    }

    fn attach_surface_maps(&self, mesh: &mut MeshData, spec: &str) {
        let Some(prop) = self.props.iter().find(|p| {
            p.enabled
                && prop_texture_path(p).is_none()
                && prop_gltf_spec(p) == Some(spec)
                && prop_has_maps(p)
        }) else {
            return;
        };
        apply_surface_maps(mesh, prop);
    }

    fn texture_mesh_ids(&self) -> HashMap<String, MeshId> {
        let base = MESH_GLTF_BASE + self.gltf_slots().len() as u32;
        let mut out = HashMap::new();
        for (i, prop) in self.textured_props().into_iter().enumerate() {
            out.insert(prop.id.clone(), MeshId(base + i as u32));
        }
        out
    }

    fn vegetation_lod_radius(&self) -> f32 {
        self.heightfield
            .as_ref()
            .and_then(|h| h.lod_radius)
            .unwrap_or(28.0)
            .max(1.0)
    }

    fn vegetation_lod_cells(&self) -> u32 {
        self.heightfield
            .as_ref()
            .and_then(|h| h.lod_cells)
            .unwrap_or(8)
            .clamp(6, 12) as u32
    }

    fn dumped_vegetation_count(&self) -> usize {
        self.props
            .iter()
            .filter(|p| p.enabled && is_vegetation_prop(p))
            .count()
    }

    /// Kenney-style grove as instanced primitives. Not dumped (scenery).
    /// Distance LOD: full trunk+cone vs existing billboard quad. Do not thin.
    fn push_vista(&self, b: &mut SceneBuilder, camera: &Camera) {
        if self.heightfield.is_none() || self.half < 40.0 {
            return;
        }
        if self.dumped_vegetation_count() >= 8 {
            return;
        }
        let lod_r = self.vegetation_lod_radius();
        let eye = camera.eye;
        let water = self.water_y.unwrap_or(0.0);
        let trees = [
            (-3.4, 1.2, 2.15, true, 0.40),
            (4.1, 2.0, 1.85, false, 1.10),
            (2.6, 6.4, 2.25, true, -0.35),
            (-5.2, 4.8, 1.95, false, 0.80),
            (6.8, 3.1, 2.05, true, 0.20),
            (-2.0, 8.6, 1.75, false, -0.90),
            (8.8, 9.4, 1.55, false, 0.50),
            (-6.6, 11.2, 1.45, false, 1.30),
            (3.8, 12.5, 2.10, true, -0.20),
            (11.2, 2.4, 1.80, false, 2.00),
            (-1.2, 14.8, 2.00, true, 0.70),
            (5.4, 16.2, 1.70, false, -0.55),
            (9.6, 13.0, 1.90, true, 0.15),
            (12.4, 18.5, 3.20, true, 0.40),
            (15.0, 21.0, 3.50, true, -0.30),
            (18.2, 24.6, 3.80, true, 0.80),
            (6.2, 22.4, 3.40, true, 1.10),
            (-4.8, 9.8, 2.60, false, 0.25),
            (-11.2, 3.4, 2.40, false, 0.60),
            (-10.4, 8.2, 2.20, false, -0.40),
            (-9.6, 13.6, 2.50, false, 1.20),
            (1.6, 10.2, 2.00, false, 0.10),
            (-7.8, 2.2, 1.70, false, 0.90),
            (7.2, 7.8, 1.60, true, -1.40),
            (-3.8, 17.4, 1.95, true, 0.45),
            (8.0, 40.0, 2.40, true, 0.20),
            (10.8, 46.0, 2.80, true, -0.40),
            (4.2, 48.5, 2.50, true, 0.70),
        ];
        for (x, z, s, pine, yaw) in trees {
            push_tree(
                b,
                eye,
                lod_r,
                x,
                z,
                s,
                pine,
                yaw,
                self.height_at(x, z),
                water,
            );
        }
        let mut n = 0u32;
        let mut z: f32 = -32.0;
        while z < 56.0 {
            let mut x: f32 = -28.0;
            while x < 28.0 {
                let jx = ((n.wrapping_mul(37) + 11) % 100) as f32 / 100.0 * 2.4 - 1.2;
                let jz = ((n.wrapping_mul(17) + 4) % 100) as f32 / 100.0 * 2.4 - 1.2;
                let px = x + jx;
                let pz = z + jz;
                let keep = (n.wrapping_mul(13) + 7) % 10 > 3;
                n += 1;
                if keep {
                    let near_sig = trees
                        .iter()
                        .any(|(tx, tz, _, _, _)| (px - tx).hypot(pz - tz) < 3.2);
                    let spawn = px.hypot(pz + 8.0) < 3.0;
                    if !near_sig && !spawn {
                        let g = self.height_at(px, pz);
                        let pine = g > 2.2 || n.is_multiple_of(3);
                        let scale = 1.45 + (n % 7) as f32 * 0.12;
                        let yaw = n as f32 * 0.37;
                        push_tree(b, eye, lod_r, px, pz, scale, pine, yaw, g, water);
                    }
                }
                x += 7.0;
            }
            z += 7.0;
        }
        let flowers = [
            [210u8, 70, 70, 255],
            [230, 200, 60, 255],
            [150, 90, 200, 255],
            [80, 160, 70, 255],
        ];
        n = 0;
        let mut z: f32 = -6.0;
        while z < 22.0 {
            let mut x: f32 = -11.0;
            while x < 14.0 {
                if x.hypot(z + 8.0) >= 1.8 && x > -10.0 {
                    let jx = ((n.wrapping_mul(37) + 11) % 100) as f32 / 100.0 * 0.7 - 0.35;
                    let jz = ((n.wrapping_mul(17) + 4) % 100) as f32 / 100.0 * 0.55 - 0.27;
                    let px = x + jx;
                    let pz = z + jz;
                    let g = self.height_at(px, pz);
                    if g > water + 0.05 {
                        let col = flowers[(n as usize) % flowers.len()];
                        let h = 0.22 + (n % 5) as f32 * 0.04;
                        let pos = Vec3::new(px, g + h * 0.5, pz);
                        if eye.distance(pos) > lod_r {
                            let model = Mat4::from_scale_rotation_translation(
                                Vec3::new(h * 0.9, h, 1.0),
                                Quat::from_rotation_y(billboard_yaw(eye, pos)),
                                pos,
                            );
                            b.push_material(MESH_QUAD, model, col, Material::Solid);
                        } else {
                            let model = Mat4::from_scale_rotation_translation(
                                Vec3::new(0.16, h, 0.16),
                                Quat::from_rotation_y(n as f32 * 0.47),
                                pos,
                            );
                            b.push(MESH_BOX, model, col);
                        }
                    }
                    n += 1;
                }
                x += 2.6;
            }
            z += 2.4;
        }
        let rocks = [
            (1.4, 0.8, 1.4, 0.9, 1.2, 0.20),
            (-1.6, 3.2, 1.6, 0.7, 1.3, 0.90),
            (5.0, 1.1, 1.3, 0.6, 1.1, -0.40),
            (-9.2, 5.4, 1.5, 1.0, 1.4, 0.55),
            (12.0, 7.2, 1.8, 0.8, 1.3, -0.80),
            (2.2, 4.6, 0.9, 1.2, 0.8, 0.30),
            (9.0, 19.4, 1.6, 1.1, 1.5, 0.15),
            (4.6, 9.0, 1.3, 0.7, 1.4, 0.70),
            (-6.0, 15.4, 1.1, 1.3, 0.9, -0.25),
        ];
        for (x, z, sx, sy, sz, yaw) in rocks {
            let g = self.height_at(x, z);
            let m = Mat4::from_scale_rotation_translation(
                Vec3::new(sx, sy, sz),
                Quat::from_rotation_y(yaw),
                Vec3::new(x, g + sy * 0.45, z),
            );
            b.push(MESH_BOX, m, [128, 118, 108, 255]);
        }
    }
}

fn is_vegetation_prop(prop: &WorldProp) -> bool {
    let mut n = String::new();
    n.push_str(&prop.name);
    n.push(' ');
    n.push_str(&prop.model);
    if let Some(g) = prop.gltf.as_deref() {
        n.push(' ');
        n.push_str(g);
    }
    let n = n.to_ascii_lowercase();
    n.contains("tree")
        || n.contains("pine")
        || n.contains("grass")
        || n.contains("bush")
        || n.contains("plant")
        || n.contains("flower")
        || n.contains("weed")
        || n.contains("foliage")
        || n.contains("nature")
}

fn billboard_yaw(eye: Vec3, pos: Vec3) -> f32 {
    (eye.x - pos.x).atan2(eye.z - pos.z)
}

#[allow(clippy::too_many_arguments)]
fn push_tree(
    b: &mut SceneBuilder,
    eye: Vec3,
    lod_r: f32,
    x: f32,
    z: f32,
    scale: f32,
    pine: bool,
    yaw: f32,
    ground: f32,
    water_y: f32,
) {
    if ground < water_y + 0.05 {
        return;
    }
    let pos = Vec3::new(x, ground, z);
    if eye.distance(pos) > lod_r {
        let h = scale * if pine { 3.6 } else { 2.6 };
        let model = Mat4::from_scale_rotation_translation(
            Vec3::new(h * 0.55, h, 1.0),
            Quat::from_rotation_y(billboard_yaw(eye, pos)),
            pos + Vec3::Y * (h * 0.45),
        );
        let col = if pine {
            [46, 102, 58, 255]
        } else {
            [62, 132, 52, 255]
        };
        b.push_material(MESH_QUAD, model, col, Material::Solid);
        return;
    }
    let q = Quat::from_rotation_y(yaw);
    let trunk_h = scale * if pine { 1.15 } else { 0.85 };
    let trunk = Mat4::from_scale_rotation_translation(
        Vec3::new(0.22 * scale, trunk_h, 0.22 * scale),
        q,
        pos,
    );
    b.push(MESH_CAPSULE, trunk, [92, 62, 38, 255]);
    let foliage_h = scale * if pine { 2.6 } else { 1.9 };
    let foliage_r = scale * if pine { 0.95 } else { 1.35 };
    let fol = Mat4::from_scale_rotation_translation(
        Vec3::new(foliage_r, foliage_h, foliage_r),
        q,
        Vec3::new(x, ground + trunk_h * 0.55, z),
    );
    b.push(
        MESH_CONE,
        fol,
        if pine {
            [46, 102, 58, 255]
        } else {
            [62, 132, 52, 255]
        },
    );
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum GltfSlot {
    Rest(String),
    Skinned {
        walker_id: String,
        spec: String,
        part: usize,
        clip: f32,
        hair: f32,
        morph: f32,
        expression: String,
        look_yaw: f32,
        look_pitch: f32,
        anim_blend: f32,
        overlay_bones: HashMap<String, [f32; 4]>,
        overlay_weight: f32,
        anim: String,
        clip_name: Option<String>,
        fade_from: Option<String>,
        fade_from_clip: f32,
        fade_from_weight: f32,
    },
}

fn walker_spec_has_mtoon(spec: &str) -> bool {
    if let Some(parts) = load_skinned_parts(spec) {
        return parts.iter().any(|s| s.rest.mtoon.is_some());
    }
    gltf_mesh_for(spec)
        .map(|m| m.mtoon.is_some())
        .unwrap_or(false)
}

fn walker_spec_has_albedo(spec: &str) -> bool {
    if let Some(parts) = load_skinned_parts(spec) {
        return parts.iter().any(|s| s.rest.albedo.is_some());
    }
    gltf_mesh_for(spec)
        .map(|m| m.albedo.is_some())
        .unwrap_or(false)
}

fn walker_gltf_spec(w: &WorldWalker) -> Option<&str> {
    if let Some(g) = w.gltf.as_deref().map(str::trim) {
        if !g.is_empty() {
            return Some(g);
        }
    }
    let m = w.model.trim();
    if m.is_empty() {
        return None;
    }
    let lower = m.to_ascii_lowercase();
    if lower.ends_with(".gltf")
        || lower.ends_with(".glb")
        || lower.ends_with(".vrm")
        || !crate::assets::resolve_alias(m).is_empty()
    {
        return Some(m);
    }
    None
}

fn prop_texture_path(prop: &WorldProp) -> Option<&str> {
    prop.texture
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn prop_gltf_spec(prop: &WorldProp) -> Option<&str> {
    prop.gltf
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn map_spec(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn prop_has_maps(prop: &WorldProp) -> bool {
    map_spec(&prop.roughness_map).is_some()
        || map_spec(&prop.metalness_map).is_some()
        || map_spec(&prop.emissive_map).is_some()
}

/// Does `load_png_albedo(path)` have a file to read? (No decode.)
pub fn texture_exists(path: &str) -> bool {
    let spec = path.trim();
    if spec.is_empty() {
        return false;
    }
    [
        PathBuf::from(spec),
        PathBuf::from(".").join(spec),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(spec),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(spec),
    ]
    .iter()
    .any(crate::assets::is_file)
}

/// Load a PNG as albedo. Repo-relative paths resolve from cwd or the crate parent.
pub fn load_png_albedo(path: &str) -> Option<crate::scene3d::AlbedoRgba> {
    let spec = path.trim();
    if spec.is_empty() {
        return None;
    }
    let candidates = [
        PathBuf::from(spec),
        PathBuf::from(".").join(spec),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(spec),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(spec),
    ];
    let file = candidates.iter().find(|p| crate::assets::is_file(p))?;
    let bytes = crate::assets::read_bytes(file).ok()?;
    crate::gltf_load::decode_png(&bytes).ok()
}

fn textured_prop_mesh(prop: &WorldProp) -> Option<MeshData> {
    let albedo = prop_texture_path(prop).and_then(load_png_albedo);
    if albedo.is_none() && !prop_has_maps(prop) {
        return None;
    }
    let mut mesh = match prop.model.to_ascii_lowercase().as_str() {
        "plane" | "water" => primitives::plane_mesh(1.0, 1.0),
        "sprite" | "quad" => primitives::quad_mesh(1.0, 1.0),
        "sphere" | "cylinder" | "capsule" => primitives::cylinder_mesh(0.5, 1.0, 12),
        _ => primitives::box_mesh(Vec3::ONE),
    };
    if let Some(albedo) = albedo {
        mesh.albedo = Some(tile_albedo(albedo, prop.uv_scale));
    }
    apply_surface_maps(&mut mesh, prop);
    Some(mesh)
}

fn apply_surface_maps(mesh: &mut MeshData, prop: &WorldProp) {
    if let Some(orm) = build_orm(map_spec(&prop.roughness_map), map_spec(&prop.metalness_map)) {
        mesh.orm_rough = orm.rough;
        mesh.orm_metal = orm.metal;
        mesh.orm = Some(orm.image);
    }
    if let Some(spec) = map_spec(&prop.emissive_map) {
        mesh.emissive_map = load_surface_image(spec);
    }
}

struct OrmImage {
    image: crate::scene3d::AlbedoRgba,
    rough: bool,
    metal: bool,
}

fn build_orm(rough: Option<&str>, metal: Option<&str>) -> Option<OrmImage> {
    if rough.is_none() && metal.is_none() {
        return None;
    }
    if let (Some(r), m) = (rough, metal) {
        if let Some(preset) = preset_orm(r) {
            let same = m.is_none() || m == Some(r);
            if same {
                return Some(OrmImage {
                    image: preset,
                    rough: true,
                    metal: true,
                });
            }
        }
    }
    if rough.is_none() {
        if let Some(m) = metal.and_then(preset_orm) {
            return Some(OrmImage {
                image: m,
                rough: true,
                metal: true,
            });
        }
    }
    let rough_img = rough.and_then(load_surface_image);
    let metal_img = metal.and_then(load_surface_image);
    if rough_img.is_none() && metal_img.is_none() {
        return None;
    }
    let (w, h) = match (&rough_img, &metal_img) {
        (Some(a), _) => (a.width, a.height),
        (_, Some(a)) => (a.width, a.height),
        _ => (1, 1),
    };
    let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
    for i in 0..(w as usize * h as usize) {
        let g = rough_img
            .as_ref()
            .map(|img| sample_luma(img, i))
            .unwrap_or(255);
        let b = metal_img
            .as_ref()
            .map(|img| sample_luma(img, i))
            .unwrap_or(0);
        let o = i * 4;
        rgba[o] = 255;
        rgba[o + 1] = g;
        rgba[o + 2] = b;
        rgba[o + 3] = 255;
    }
    Some(OrmImage {
        image: crate::scene3d::AlbedoRgba {
            width: w,
            height: h,
            rgba: rgba.into(),
        },
        rough: rough_img.is_some(),
        metal: metal_img.is_some(),
    })
}

fn sample_luma(img: &crate::scene3d::AlbedoRgba, index: usize) -> u8 {
    let n = (img.width as usize)
        .saturating_mul(img.height as usize)
        .max(1);
    let i = (index % n) * 4;
    if i + 2 >= img.rgba.len() {
        return 128;
    }
    let r = img.rgba[i] as u32;
    let g = img.rgba[i + 1] as u32;
    let b = img.rgba[i + 2] as u32;
    ((r * 3 + g * 6 + b) / 10) as u8
}

fn load_surface_image(spec: &str) -> Option<crate::scene3d::AlbedoRgba> {
    if let Some(img) = preset_orm(spec).or_else(|| preset_emissive(spec)) {
        return Some(img);
    }
    load_png_albedo(spec)
}

/// `preset:worn` / `preset:split` / `preset:brushed`. Linear bytes (ORM is Unorm).
fn preset_orm(spec: &str) -> Option<crate::scene3d::AlbedoRgba> {
    let name = spec.strip_prefix("preset:")?;
    let n = 64u32;
    let mut rgba = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let u = x as f32 / (n - 1) as f32;
            let band = (u * 6.0).sin() * 0.5 + 0.5;
            let pit = if (x * 13 + y * 7) % 17 == 0 { 1.0 } else { 0.0 };
            let (rough, metal) = match name {
                "split" => {
                    if u < 0.5 {
                        (0.12, 1.0)
                    } else {
                        (0.92, 0.0)
                    }
                }
                "brushed" => (0.18 + 0.55 * ((u * 18.0).sin() * 0.5 + 0.5), 1.0),
                "worn" => (
                    (0.22 + 0.62 * band + 0.18 * pit).clamp(0.04, 1.0),
                    (0.04 + 0.62 * (1.0 - band) * (1.0 - pit * 0.4)).clamp(0.0, 1.0),
                ),
                _ => return None,
            };
            let i = ((y * n + x) * 4) as usize;
            rgba[i] = 255;
            rgba[i + 1] = (rough.clamp(0.0, 1.0) * 255.0).round() as u8;
            rgba[i + 2] = (metal.clamp(0.0, 1.0) * 255.0).round() as u8;
            rgba[i + 3] = 255;
        }
    }
    Some(crate::scene3d::AlbedoRgba {
        width: n,
        height: n,
        rgba: rgba.into(),
    })
}

/// 64×64 linear RGBA. Four roughness rows (16 px tall) of a lat-long.
/// `image` replaces the sharp row when a PNG equirect was loaded.
pub fn bake_env_strip(
    sky: [f32; 3],
    ground: [f32; 3],
    image: Option<crate::scene3d::AlbedoRgba>,
) -> Vec<u8> {
    const W: u32 = 64;
    const ROW: u32 = 16;
    const LEVELS: u32 = 4;
    let mut rows = Vec::with_capacity(LEVELS as usize);
    rows.push(equirect_row(sky, ground, image.as_ref(), W, ROW));
    for level in 1..LEVELS {
        let prev = rows.last().expect("row");
        rows.push(blur_wrap(prev, W, ROW, level));
    }
    let mut rgba = vec![0u8; (W * ROW * LEVELS * 4) as usize];
    for (i, row) in rows.iter().enumerate() {
        let dst = i * row.len();
        rgba[dst..dst + row.len()].copy_from_slice(row);
    }
    rgba
}

fn equirect_row(
    sky: [f32; 3],
    ground: [f32; 3],
    image: Option<&crate::scene3d::AlbedoRgba>,
    w: u32,
    h: u32,
) -> Vec<u8> {
    let sun = Vec3::new(-0.4, 1.0, 0.3).normalize();
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32;
            let v = (y as f32 + 0.5) / h as f32;
            let col = if let Some(img) = image {
                sample_equirect(img, u, v)
            } else {
                let theta = (u - 0.5) * std::f32::consts::TAU;
                let phi = v * std::f32::consts::PI;
                let dir = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
                let t = smoothstep(-0.15, 0.35, dir.y);
                let mut col = [
                    ground[0] + (sky[0] - ground[0]) * t,
                    ground[1] + (sky[1] - ground[1]) * t,
                    ground[2] + (sky[2] - ground[2]) * t,
                ];
                let sun_d = dir.dot(sun).max(0.0);
                let disk = sun_d.powf(40.0);
                col[0] += disk * 1.4;
                col[1] += disk * 1.25;
                col[2] += disk * 0.9;
                col
            };
            let i = ((y * w + x) * 4) as usize;
            rgba[i] = to_unorm(col[0]);
            rgba[i + 1] = to_unorm(col[1]);
            rgba[i + 2] = to_unorm(col[2]);
            rgba[i + 3] = 255;
        }
    }
    rgba
}

fn sample_equirect(img: &crate::scene3d::AlbedoRgba, u: f32, v: f32) -> [f32; 3] {
    let w = img.width.max(1) as f32;
    let h = img.height.max(1) as f32;
    let x = (u.clamp(0.0, 0.999) * w) as usize;
    let y = (v.clamp(0.0, 0.999) * h) as usize;
    let i = (y * img.width as usize + x) * 4;
    if i + 2 >= img.rgba.len() {
        return [0.5, 0.5, 0.5];
    }
    [
        img.rgba[i] as f32 / 255.0,
        img.rgba[i + 1] as f32 / 255.0,
        img.rgba[i + 2] as f32 / 255.0,
    ]
}

fn blur_wrap(src: &[u8], w: u32, h: u32, radius: u32) -> Vec<u8> {
    let r = radius as i32;
    let mut out = vec![0u8; src.len()];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut acc = [0u32; 3];
            let mut n = 0u32;
            for oy in -r..=r {
                for ox in -r..=r {
                    let sx = (x + ox).rem_euclid(w as i32) as u32;
                    let sy = (y + oy).clamp(0, h as i32 - 1) as u32;
                    let i = ((sy * w + sx) * 4) as usize;
                    acc[0] += src[i] as u32;
                    acc[1] += src[i + 1] as u32;
                    acc[2] += src[i + 2] as u32;
                    n += 1;
                }
            }
            let i = ((y as u32 * w + x as u32) * 4) as usize;
            out[i] = (acc[0] / n) as u8;
            out[i + 1] = (acc[1] / n) as u8;
            out[i + 2] = (acc[2] / n) as u8;
            out[i + 3] = 255;
        }
    }
    out
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn to_unorm(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn preset_emissive(spec: &str) -> Option<crate::scene3d::AlbedoRgba> {
    let name = spec.strip_prefix("preset:")?;
    if name != "core" {
        return None;
    }
    let n = 64u32;
    let mut rgba = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let u = x as f32 / (n - 1) as f32 - 0.5;
            let v = y as f32 / (n - 1) as f32 - 0.5;
            let d = (u * u + v * v).sqrt();
            let glow = (1.0 - d * 2.2).clamp(0.0, 1.0);
            let byte = (glow * 255.0).round() as u8;
            let i = ((y * n + x) * 4) as usize;
            rgba[i] = byte;
            rgba[i + 1] = byte;
            rgba[i + 2] = byte;
            rgba[i + 3] = 255;
        }
    }
    Some(crate::scene3d::AlbedoRgba {
        width: n,
        height: n,
        rgba: rgba.into(),
    })
}

fn tile_albedo(
    src: crate::scene3d::AlbedoRgba,
    scale: Option<[f32; 2]>,
) -> crate::scene3d::AlbedoRgba {
    let (nx, ny) = match scale {
        Some([u, v]) => (u.max(1.0).round() as u32, v.max(1.0).round() as u32),
        None => (1, 1),
    };
    if nx <= 1 && ny <= 1 {
        return src;
    }
    let w = src.width.saturating_mul(nx).max(1);
    let h = src.height.saturating_mul(ny).max(1);
    let sw = src.width as usize;
    let mut rgba = vec![0u8; w as usize * h as usize * 4];
    for ty in 0..ny {
        for y in 0..src.height {
            let src_row = (y as usize * sw) * 4;
            for tx in 0..nx {
                let dst = (((ty * src.height + y) as usize * w as usize) + tx as usize * sw) * 4;
                rgba[dst..dst + sw * 4].copy_from_slice(&src.rgba[src_row..src_row + sw * 4]);
            }
        }
    }
    crate::scene3d::AlbedoRgba {
        width: w,
        height: h,
        rgba: rgba.into(),
    }
}

fn mesh_for_prop(
    prop: &WorldProp,
    gltf_ids: &HashMap<String, MeshId>,
    tex_ids: &HashMap<String, MeshId>,
) -> MeshId {
    if let Some(id) = tex_ids.get(&prop.id) {
        return *id;
    }
    if let Some(spec) = prop.gltf.as_deref().map(str::trim) {
        if !spec.is_empty() {
            if let Some(id) = gltf_ids.get(spec) {
                return *id;
            }
        }
    }
    match prop.model.to_ascii_lowercase().as_str() {
        "sphere" if is_coin_prop(prop) => MESH_CAPSULE,
        "sphere" => MESH_SPHERE,
        "cylinder" | "capsule" => MESH_CAPSULE,
        "plane" | "water" => MESH_PLANE,
        "sprite" | "quad" => MESH_QUAD,
        "torus" | "ring" => MESH_TORUS,
        "cone" => MESH_CONE,
        _ => MESH_BOX,
    }
}

fn is_coin_prop(prop: &WorldProp) -> bool {
    prop.name.eq_ignore_ascii_case("coin") || prop.metallic >= 0.5
}

fn is_water_prop(prop: &WorldProp) -> bool {
    prop.name.eq_ignore_ascii_case("water") || prop.model.eq_ignore_ascii_case("water")
}

/// Metal coins/props: dump `metallic>=0.5` or name coin. Shader GGX uses the
/// coin defaults (metallic=1, roughness=0.12) so they read as metal, not plastic.
/// Water: dump name/model `water` (plane mesh). Same shader family as Metal/Toon.
fn material_for_prop(prop: &WorldProp) -> Material {
    if is_water_prop(prop) {
        Material::Water
    } else if is_coin_prop(prop) {
        Material::Metal
    } else {
        Material::Solid
    }
}

/// Python `kagra.land.island_height` — data, not a live Python fn.
pub fn island_height(x: f32, z: f32) -> f32 {
    let r = (x * x + z * z).sqrt();
    let shelf = 0.38 - 0.052 * r;
    let hill = 4.3 * (-((x - 9.0).powi(2) + (z - 6.0).powi(2)) / 28.0).exp();
    let bay = -2.7 * (-((x + 11.0).powi(2) + z * z) / 36.0).exp();
    shelf + hill + bay
}

/// Python `kagra.land.overworld_height` (island + plaza stair/ramp).
pub fn overworld_height(x: f32, z: f32) -> f32 {
    let mut y = island_height(x, z);
    if (-5.2..=-3.2).contains(&x) && (1.5..=5.8).contains(&z) {
        let n = 6.0;
        let t = ((z - 1.5) / (5.8 - 1.5)).clamp(0.0, 0.999_999);
        let step = (t * n).floor();
        y = y.max(0.42 + (1.85 - 0.42) * (step + 1.0) / n);
    }
    if (2.5..=7.0).contains(&x) && (-7.0..=-4.5).contains(&z) {
        let t = ((x - 2.5) / (7.0 - 2.5)).clamp(0.0, 1.0);
        y = y.max(0.35 + (1.7 - 0.35) * t);
    }
    y
}

fn nearest_sample(samples: &[[f32; 3]], x: f32, z: f32) -> f32 {
    let mut best_y = samples[0][2];
    let mut best_d = f32::MAX;
    for row in samples {
        let dx = row[0] - x;
        let dz = row[1] - z;
        let d = dx * dx + dz * dz;
        if d < best_d {
            best_d = d;
            best_y = row[2];
        }
    }
    best_y
}

fn existing_asset_path(spec: &str) -> Option<PathBuf> {
    let base = spec.split('#').next().unwrap_or(spec);
    crate::assets::resolve_asset(base)
}

/// Bare `crate.glb` / `cube` is the embedded unit cube (Crest, puzzles).
/// A path such as `assets/.../crate.glb` is a real file and must not take
/// that alias, or the sim collides with a mesh the screen never shows.
fn bare_primitive_alias(spec: &str) -> bool {
    let spec = spec.trim();
    if spec.contains('/') || spec.contains('\\') {
        return false;
    }
    matches!(
        spec.to_ascii_lowercase().as_str(),
        "cube.glb" | "cube.gltf" | "crate.glb" | "crate.gltf" | "cube"
    )
}

fn split_mesh_selector(spec: &str) -> (&str, Option<usize>) {
    if let Some((base, frag)) = spec.split_once('#') {
        if let Ok(idx) = frag.trim().parse::<usize>() {
            return (base.trim(), Some(idx));
        }
    }
    match spec.trim().to_ascii_lowercase().as_str() {
        "chess_rook" => ("chess_set", Some(0)),
        "chess_pawn" => ("chess_set", Some(1)),
        "chess_bishop" => ("chess_set", Some(2)),
        "chess_queen" => ("chess_set", Some(3)),
        "chess_king" => ("chess_set", Some(4)),
        "chess_knight" => ("chess_set", Some(5)),
        "chess_board" => ("chess_set", Some(6)),
        _ => (spec.trim(), None),
    }
}

fn walker_tpose_mesh(
    clip: f32,
    hair: f32,
    morph: f32,
    look_yaw: f32,
    look_pitch: f32,
) -> Option<MeshData> {
    let skin = skinned_tpose_humanoid().ok()?;
    let t = if clip <= 0.0 { None } else { Some(clip) };
    Some(sample_skinned_look(
        &skin, t, hair, "blink", morph, look_yaw, look_pitch,
    ))
}

fn walker_asset_error(spec: &str) -> Option<String> {
    probe_walker_asset(spec).err()
}

fn probe_walker_asset(spec: &str) -> Result<(), String> {
    let spec = spec.trim();
    if spec.starts_with('{')
        || is_walk_skinned_spec(spec)
        || is_walk_vrm_spec(spec)
        || is_tpose_humanoid_spec(spec)
    {
        return Ok(());
    }
    if bare_primitive_alias(spec) {
        return Ok(());
    }
    let path = existing_asset_path(spec)
        .ok_or_else(|| format!("{spec}: missing (repo root / KAGRA_ROOT / window.exe dir)"))?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or(spec);
    let p_lower = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if p_lower == "gltf" {
        let json = crate::assets::read_to_string(&path).map_err(|e| format!("{name}: {e}"))?;
        skinned_from_gltf_json(&json, |uri| {
            let base = path.parent().unwrap_or(Path::new("."));
            crate::assets::read_bytes(base.join(uri)).map_err(|e| e.to_string())
        })
        .map(|_| ())
        .or_else(|e| {
            mesh_from_gltf_json(&json, |uri| {
                let base = path.parent().unwrap_or(Path::new("."));
                crate::assets::read_bytes(base.join(uri)).map_err(|err| err.to_string())
            })
            .map(|_| ())
            .map_err(|_| format!("{name}: {e}"))
        })
    } else if p_lower == "glb" || p_lower == "vrm" {
        let bytes = crate::assets::read_bytes(&path).map_err(|e| format!("{name}: {e}"))?;
        skinned_from_glb(&bytes).map(|_| ()).or_else(|e| {
            mesh_from_glb(&bytes)
                .map(|_| ())
                .map_err(|_| format!("{name}: {e}"))
        })
    } else {
        Ok(())
    }
}

fn is_emma_vrm_stem(stem: &str) -> bool {
    stem.eq_ignore_ascii_case("emma.vrm")
}

/// Thin a mesh to about `keep` of its triangles by vertex clustering: vertices that share a
/// grid cell, a facing octant and a texel neighbourhood collapse onto the one nearest the cell's
/// centroid, so positions stay on the surface, hard edges stay hard and the texture lookup is
/// untouched. The cell size is found by bisection. Deterministic (ids by first occurrence).
pub(crate) fn decimate_mesh(m: &mut MeshData, keep: f32) {
    let tris = m.indices.len() / 3;
    let keep = keep.clamp(0.2, 1.0);
    if keep >= 0.98 || tris < 80 || m.vertices.is_empty() {
        return;
    }
    let target = (tris as f32 * keep).ceil() as usize;
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in &m.vertices {
        for a in 0..3 {
            lo[a] = lo[a].min(v.pos[a]);
            hi[a] = hi[a].max(v.pos[a]);
        }
    }
    let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
    if diag < 1e-5 {
        return;
    }
    type Key = (i32, i32, i32, u8, i32, i32);
    let key_of = |v: &Vertex3, cell: f32| -> Key {
        let q = |x: f32, o: f32| ((x - o) / cell).floor() as i32;
        let oct = (v.normal[0] >= 0.0) as u8
            | ((v.normal[1] >= 0.0) as u8) << 1
            | ((v.normal[2] >= 0.0) as u8) << 2;
        (
            q(v.pos[0], lo[0]),
            q(v.pos[1], lo[1]),
            q(v.pos[2], lo[2]),
            oct,
            (v.uv[0] * 10.0).floor() as i32,
            (v.uv[1] * 10.0).floor() as i32,
        )
    };
    // The cell id of every vertex at a cell size, ids by first occurrence.
    let cluster = |cell: f32| -> (Vec<u32>, usize) {
        let mut ids: std::collections::HashMap<Key, u32> = std::collections::HashMap::new();
        let mut of = Vec::with_capacity(m.vertices.len());
        for v in &m.vertices {
            let next = ids.len() as u32;
            of.push(*ids.entry(key_of(v, cell)).or_insert(next));
        }
        let n = ids.len();
        (of, n)
    };
    let surviving = |of: &[u32]| -> usize {
        m.indices
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|t| {
                let (a, b, c) = (of[t[0] as usize], of[t[1] as usize], of[t[2] as usize]);
                a != b && b != c && a != c
            })
            .count()
    };
    let (mut lo_c, mut hi_c) = (diag / 600.0, diag / 2.0);
    for _ in 0..14 {
        let mid = (lo_c * hi_c).sqrt();
        let (of, _) = cluster(mid);
        if surviving(&of) >= target {
            lo_c = mid;
        } else {
            hi_c = mid;
        }
    }
    let (of, n) = cluster(lo_c);
    // The representative of each cell: the member nearest the members' centroid.
    let mut sum = vec![[0.0f32; 4]; n];
    for (v, &c) in m.vertices.iter().zip(&of) {
        let s = &mut sum[c as usize];
        s[0] += v.pos[0];
        s[1] += v.pos[1];
        s[2] += v.pos[2];
        s[3] += 1.0;
    }
    let mut best = vec![(f32::MAX, 0usize); n];
    for (i, (v, &c)) in m.vertices.iter().zip(&of).enumerate() {
        let s = sum[c as usize];
        let d = (0..3)
            .map(|a| (v.pos[a] - s[a] / s[3]).powi(2))
            .sum::<f32>();
        if d < best[c as usize].0 {
            best[c as usize] = (d, i);
        }
    }
    let verts: Vec<Vertex3> = best.iter().map(|&(_, i)| m.vertices[i]).collect();
    let mut indices = Vec::with_capacity(target * 3 + 3);
    for t in m.indices.as_chunks::<3>().0 {
        let (a, b, c) = (of[t[0] as usize], of[t[1] as usize], of[t[2] as usize]);
        if a != b && b != c && a != c {
            indices.extend_from_slice(&[a, b, c]);
        }
    }
    if indices.len() >= 3 {
        m.vertices = verts;
        m.indices = indices;
    }
}

fn gltf_mesh_for(spec: &str) -> Option<MeshData> {
    let spec = spec.trim();
    if spec.starts_with('{') {
        return mesh_from_embedded_gltf(spec).ok();
    }
    if is_walk_skinned_spec(spec) {
        return mesh_from_embedded_gltf(&walk_skinned_gltf()).ok();
    }
    if is_walk_vrm_spec(spec) {
        return mesh_from_glb(&walk_skinned_vrm()).ok();
    }
    if bare_primitive_alias(spec) {
        return mesh_from_embedded_gltf(&unit_cube_gltf()).ok();
    }
    let (base_spec, mesh_sel) = split_mesh_selector(spec);
    if let Some(path) = existing_asset_path(base_spec) {
        let p_lower = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if p_lower == "gltf" {
            let json = crate::assets::read_to_string(&path).ok()?;
            let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            if let Some(idx) = mesh_sel {
                return mesh_index_from_gltf_json(&json, idx, |uri| {
                    crate::assets::read_bytes(base.join(uri)).map_err(|e| e.to_string())
                })
                .ok();
            }
            return mesh_from_gltf_json(&json, |uri| {
                crate::assets::read_bytes(base.join(uri)).map_err(|e| e.to_string())
            })
            .ok();
        }
        if p_lower == "glb" || p_lower == "vrm" {
            let bytes = crate::assets::read_bytes(&path).ok()?;
            return mesh_from_glb(&bytes).ok();
        }
    }
    None
}

fn gltf_skinned_mesh_for(spec: &str, part: usize, pose: WalkerPose) -> Option<MeshData> {
    if let Some(parts) = load_skinned_parts(spec) {
        let skin = parts.get(part)?;
        return Some(crate::gltf_load::sample_skinned_pose(skin, &pose));
    }
    if part == 0 {
        return gltf_mesh_for(spec);
    }
    None
}

fn skinned_parts_cache() -> &'static Mutex<HashMap<String, Arc<Vec<crate::gltf_load::SkinnedMesh>>>>
{
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<Vec<crate::gltf_load::SkinnedMesh>>>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Drop CPU-skinned meshes so a reloaded bundle (browser or KAGRA Dev) is read again.
pub(crate) fn clear_runtime_asset_cache() {
    if let Ok(mut cache) = skinned_parts_cache().lock() {
        cache.clear();
    }
}

fn skinned_part_count(spec: &str) -> usize {
    load_skinned_parts(spec)
        .map(|p| p.len().max(1))
        .unwrap_or(1)
}

/// Rest-pose lowest vertex Y across every primitive (VRM feet sit on this).
pub(crate) fn walker_rest_min_y(spec: &str) -> f32 {
    let Some(parts) = load_skinned_parts(spec) else {
        return 0.0;
    };
    let mut min_y = 0.0f32;
    let mut any = false;
    for s in parts.iter() {
        for v in &s.rest.vertices {
            if !any || v.pos[1] < min_y {
                min_y = v.pos[1];
                any = true;
            }
        }
    }
    if any {
        min_y
    } else {
        0.0
    }
}

pub(crate) fn load_skinned_parts(spec: &str) -> Option<Arc<Vec<crate::gltf_load::SkinnedMesh>>> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    if let Ok(cache) = skinned_parts_cache().lock() {
        if let Some(hit) = cache.get(spec) {
            return if hit.is_empty() {
                None
            } else {
                Some(Arc::clone(hit))
            };
        }
    }
    let loaded = load_skinned_parts_uncached(spec).unwrap_or_default();
    let arc = Arc::new(loaded);
    if let Ok(mut cache) = skinned_parts_cache().lock() {
        cache.insert(spec.to_string(), Arc::clone(&arc));
    }
    if arc.is_empty() {
        None
    } else {
        Some(arc)
    }
}

fn load_skinned_parts_uncached(spec: &str) -> Option<Vec<crate::gltf_load::SkinnedMesh>> {
    let spec = spec.trim();
    if spec.starts_with('{') {
        return skinned_parts_from_embedded_gltf(spec).ok();
    }
    if is_walk_skinned_spec(spec) {
        return skinned_parts_from_embedded_gltf(&walk_skinned_gltf()).ok();
    }
    if is_walk_vrm_spec(spec) {
        return skinned_parts_from_glb(&walk_skinned_vrm()).ok();
    }
    if is_tpose_humanoid_spec(spec) {
        return skinned_tpose_humanoid().ok().map(|s| vec![s]);
    }
    if bare_primitive_alias(spec) {
        return skinned_from_embedded_gltf(&unit_cube_gltf())
            .ok()
            .map(|s| vec![s]);
    }
    if let Some(path) = existing_asset_path(spec) {
        let p_lower = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if p_lower == "gltf" {
            if let Ok(json) = crate::assets::read_to_string(&path) {
                let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
                if let Ok(parts) = skinned_parts_from_gltf_json(&json, |uri| {
                    crate::assets::read_bytes(base.join(uri)).map_err(|e| e.to_string())
                }) {
                    if !parts.is_empty() {
                        return Some(parts);
                    }
                }
            }
        }
        if p_lower == "glb" || p_lower == "vrm" {
            if let Ok(bytes) = crate::assets::read_bytes(&path) {
                if let Ok(parts) = skinned_parts_from_glb(&bytes) {
                    if !parts.is_empty() {
                        return Some(parts);
                    }
                }
            }
        }
    }
    let lower = spec.to_ascii_lowercase();
    let stem = Path::new(&lower)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(lower.as_str());
    if is_emma_vrm_stem(stem)
        || lower.ends_with(".vrm")
        || lower.ends_with(".gltf")
        || lower.ends_with(".glb")
    {
        return skinned_tpose_humanoid().ok().map(|s| vec![s]);
    }
    None
}

pub(crate) fn load_skinned(spec: &str) -> Option<crate::gltf_load::SkinnedMesh> {
    load_skinned_parts(spec)?.first().cloned()
}

fn color_u8(rgb: Option<[u32; 3]>) -> [u8; 4] {
    match rgb {
        Some([r, g, b]) => [r.min(255) as u8, g.min(255) as u8, b.min(255) as u8, 255],
        None => [230, 230, 235, 255],
    }
}

/// Primitive meshes that `compile_scene` refers to by `MeshId` (includes sprite/quad).
pub fn compile_meshes() -> Vec<(MeshId, crate::scene3d::MeshData)> {
    vec![
        (MESH_BOX, primitives::box_mesh(Vec3::ONE)),
        (MESH_SPHERE, primitives::sphere_mesh(0.5, 24)),
        (MESH_CAPSULE, primitives::cylinder_mesh(0.5, 1.0, 20)),
        (MESH_PLANE, primitives::plane_mesh(1.0, 1.0)),
        (MESH_QUAD, primitives::quad_mesh(1.0, 1.0)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectathon::open_world_height;

    const CREST_ISLE_DUMP: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const ORB_RUSH_DUMP: &str = include_str!("../tests/fixtures/orb_rush_world.json");
    const WORLD_SCHEMA: &str = include_str!("../../docs/schemas/world.json");

    fn assert_roundtrip_ids_positions_parent_height(json: &str) {
        let doc = WorldDoc::from_json(json).expect("parse dump");
        assert_eq!(doc.version, WORLD_DUMP_VERSION);
        let again = doc.to_json().expect("emit dump");
        let doc2 = WorldDoc::from_json(&again).expect("reparse dump");
        assert_eq!(doc.stable_ids(), doc2.stable_ids());
        assert_eq!(doc.props.len(), doc2.props.len());
        for (a, b) in doc.props.iter().zip(doc2.props.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.position, b.position);
            assert_eq!(a.parent, b.parent);
        }
        match (&doc.heightfield, &doc2.heightfield) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert_eq!(a.fn_name, b.fn_name);
                let keys_a: Vec<_> = a
                    .tiles
                    .iter()
                    .map(|t| (t.id.as_str(), t.ix, t.iz))
                    .collect();
                let keys_b: Vec<_> = b
                    .tiles
                    .iter()
                    .map(|t| (t.id.as_str(), t.ix, t.iz))
                    .collect();
                assert_eq!(keys_a, keys_b);
            }
            other => panic!("heightfield roundtrip mismatch: {other:?}"),
        }
    }

    #[test]
    fn decimation_keeps_the_shape_and_the_texture_lookup() {
        let mut m = primitives::sphere_mesh(0.5, 24);
        let full = m.indices.len() / 3;
        decimate_mesh(&mut m, 0.5);
        let now = m.indices.len() / 3;
        assert!(now < full * 7 / 10 && now > full / 4, "{now} of {full}");
        assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
        assert!(
            m.vertices
                .iter()
                .all(
                    |v| (v.pos[0].powi(2) + v.pos[1].powi(2) + v.pos[2].powi(2) - 0.25).abs()
                        < 1e-3
                ),
            "vertices stay on the surface"
        );
        let mut again = primitives::sphere_mesh(0.5, 24);
        decimate_mesh(&mut again, 0.5);
        assert_eq!(m.indices, again.indices, "deterministic");
        let mut keep_all = primitives::sphere_mesh(0.5, 24);
        decimate_mesh(&mut keep_all, 1.0);
        assert_eq!(keep_all.indices.len() / 3, full);
        if let Some(mut g) =
            gltf_mesh_for("assets/models/kenney/graveyard-kit/character-vampire.glb")
        {
            let full = g.indices.len() / 3;
            decimate_mesh(&mut g, 0.6);
            let now = g.indices.len() / 3;
            eprintln!("vampire {full} -> {now}");
            assert!(now < full * 8 / 10, "a real model thins: {now} of {full}");
            assert!(g.indices.iter().all(|&i| (i as usize) < g.vertices.len()));
        }
    }

    #[test]
    fn mesh_lod_changes_the_gpu_mesh_key_and_round_trips() {
        let mut doc = WorldDoc::default();
        let a = doc.gpu_mesh_key();
        doc.mesh_lod = Some(MeshLod {
            keep: 0.5,
            models: vec!["character-".into()],
        });
        assert_ne!(a, doc.gpu_mesh_key());
        let back = WorldDoc::from_json(&doc.to_json().unwrap()).unwrap();
        assert_eq!(back.mesh_lod, doc.mesh_lod);
        assert!(!WorldDoc::default().to_json().unwrap().contains("mesh_lod"));
    }

    #[test]
    fn world_schema_version_matches_world_doc() {
        let v: serde_json::Value = serde_json::from_str(WORLD_SCHEMA).unwrap();
        assert_eq!(v["properties"]["version"]["const"], WORLD_DUMP_VERSION);
        let defs = &v["$defs"];
        for key in [
            "prop",
            "walker",
            "light",
            "camera",
            "heightfield",
            "terrain_tile",
        ] {
            assert!(defs.get(key).is_some(), "schema missing $defs.{key}");
        }
        let walker = &defs["walker"]["properties"];
        assert!(
            walker.get("gltf").is_some(),
            "walker dump style includes gltf"
        );
        assert!(
            walker.get("model").is_some(),
            "walker dump style includes model"
        );
    }

    #[test]
    fn crest_isle_dump_roundtrips_as_world_doc() {
        let doc = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        assert_eq!(doc.half, 80.0);
        assert_eq!(doc.water_y, Some(0.0));
        let hf = doc.heightfield.as_ref().expect("heightfield");
        assert_eq!(hf.fn_name.as_deref(), Some("open_world_height"));
        assert_eq!(hf.tile, Some(16.0));
        let tile_ids: Vec<_> = hf.tiles.iter().map(|t| t.id.as_str()).collect();
        assert!(tile_ids.contains(&"tile:0,0"));
        assert!(tile_ids.contains(&"tile:-1,0"));
        let crate_p = doc.props.iter().find(|p| p.name == "crate").expect("crate");
        let coin = doc.props.iter().find(|p| p.name == "coin").expect("coin");
        assert_eq!(crate_p.id, "prop:crate");
        assert_eq!(crate_p.gltf.as_deref(), Some("crate.glb"));
        assert_eq!(coin.parent.as_deref(), Some("prop:crate"));
        assert_eq!(coin.position, [2.3, 1.1, -1.0]);
        assert_eq!(doc.player.as_ref().unwrap().id, "walker:player");
        assert_eq!(doc.player.as_ref().unwrap().position, [0.0, 1.2, -8.0]);
        assert_eq!(doc.cameras[0].id, "camera:main");
        assert_roundtrip_ids_positions_parent_height(CREST_ISLE_DUMP);
        let json = doc.to_json().unwrap();
        assert!(!json.contains("mesh_id"), "GPU mesh ids must not dump");
        assert!(!json.contains("batches"));
    }

    fn instance_pos(m: Mat4) -> Vec3 {
        m.transform_point3(Vec3::ZERO)
    }

    #[test]
    fn pitch_and_local_parent_compose_without_moving_world_coins() {
        let crest = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        let scene = crest.compile_scene(1.0);
        let coin = crest.props.iter().find(|p| p.name == "coin").unwrap();
        assert!(!coin.local, "existing parent links stay world-space");
        assert!(
            scene
                .batches
                .iter()
                .flat_map(|b| b.instances.iter())
                .any(|i| {
                    instance_pos(i.model).distance(Vec3::from_array(coin.position)) < 0.05
                }),
            "coin parent must not offset a world-space position"
        );

        let doc = WorldDoc::from_json(
            r#"{
                "version": 1,
                "ground": false,
                "props": [
                    {"id":"root","type":"prop","name":"root","position":[0,10,0],
                     "pitch":1.5707963,"model":"box","enabled":false},
                    {"id":"nose","type":"prop","name":"nose","parent":"root","local":true,
                     "position":[0,0,2],"model":"box","scale":[1,1,1]},
                    {"id":"tip","type":"prop","name":"tip","parent":"nose","local":true,
                     "position":[0,0,1],"model":"box","scale":[0.1,0.1,0.1]},
                    {"id":"gate","type":"prop","name":"ring","model":"ring",
                     "position":[4,10,0],"scale":[6,6,6]}
                ]
            }"#,
        )
        .unwrap();
        assert!((doc.props[0].pitch - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        let again = WorldDoc::from_json(&doc.to_json().unwrap()).unwrap();
        assert!((again.props[0].pitch - doc.props[0].pitch).abs() < 1e-5);
        assert!(again.props[1].local);
        let worlds = doc.prop_world_mats();
        let nose = instance_pos(worlds["nose"]);
        assert!(
            nose.distance(Vec3::new(0.0, 12.0, 0.0)) < 0.08,
            "nose-up parent should lift a +Z child, got {nose:?}"
        );
        let tip = instance_pos(worlds["tip"]);
        assert!(
            tip.distance(Vec3::new(0.0, 13.0, 0.0)) < 0.12,
            "second local link should keep climbing, got {tip:?}"
        );
        let scene = doc.compile_scene(1.0);
        assert!(
            scene.batches.iter().any(|b| b.mesh == MESH_TORUS),
            "model ring compiles to the torus slot"
        );
        assert!(
            !scene.batches.iter().any(|b| {
                b.mesh == MESH_BOX
                    && instance_pos(b.instances[0].model).distance(Vec3::new(0.0, 10.0, 0.0)) < 0.2
            }),
            "disabled root hinge is not drawn"
        );
        let ids: Vec<u32> = {
            let mut v: Vec<u32> = doc.compile_meshes().iter().map(|(id, _)| id.0).collect();
            v.sort_unstable();
            v
        };
        assert!(
            ids.windows(2).all(|w| w[1] == w[0] + 1),
            "compile mesh ids must stay dense, got {ids:?}"
        );
        assert_eq!(*ids.last().unwrap(), MESH_TORUS.0);
    }

    #[test]
    fn village_multi_material_glb_and_kaykit_walker_load() {
        let doc = WorldDoc::from_json(
            r#"{
                "version": 1,
                "ground": false,
                "walkers": [
                    {"id":"w:k","type":"walker","name":"knight","gltf":"knight","clip":0.3,"anim_blend":1.0}
                ],
                "props": [
                    {"id":"p:inn","type":"prop","name":"inn","gltf":"inn","position":[0,0,0]}
                ]
            }"#,
        )
        .unwrap();
        let meshes = doc.compile_meshes();
        assert!(
            meshes
                .iter()
                .any(|(id, m)| id.0 >= MESH_GLTF_BASE && m.albedo.is_some()),
            "multi-prim village glb and kaykit character must compile with albedo"
        );
    }

    #[test]
    fn orb_rush_dump_roundtrips_as_world_doc() {
        let doc = WorldDoc::from_json(ORB_RUSH_DUMP).unwrap();
        assert_eq!(doc.half, 6.0);
        assert!(doc.heightfield.is_none());
        assert_eq!(doc.player.as_ref().unwrap().id, "walker:player");
        assert_eq!(doc.player.as_ref().unwrap().position, [0.0, 0.0, 0.0]);
        let stars: Vec<_> = doc.props.iter().filter(|p| p.name == "star").collect();
        let bombs: Vec<_> = doc.props.iter().filter(|p| p.name == "bomb").collect();
        assert_eq!(stars.len(), 2);
        assert_eq!(bombs.len(), 1);
        assert_eq!(stars[0].id, "prop:star-a");
        assert_eq!(stars[0].position, [1.5, 0.5, -1.0]);
        assert!(stars.iter().all(|p| p.parent.is_none()));
        assert_eq!(doc.cameras[0].id, "camera:main");
        assert_roundtrip_ids_positions_parent_height(ORB_RUSH_DUMP);
    }

    #[test]
    fn world_doc_compiles_to_scene3d_draw_list() {
        let crest = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        let scene = crest.compile_scene(16.0 / 9.0);
        assert!((scene.camera.fov_y - 54f32.to_radians()).abs() < 1e-5);
        assert!((scene.camera.eye - Vec3::new(0.0, 5.65, 4.2)).length() < 1e-4);
        assert!(!scene.batches.is_empty(), "compiled frame needs batches");
        assert!(scene.instance_count() >= 3, "ground + props + walker");
        assert!(
            scene.batches.iter().any(|b| b.mesh == MESH_HEIGHTFIELD),
            "Crest dump must compile a heightfield, not a flat plane"
        );
        assert!(
            scene.batches.iter().any(|b| b.mesh.0 >= MESH_GLTF_BASE),
            "Crest crate.glb must compile as a glTF slot"
        );
        // Scene3D stays a draw list: no dump fields to roundtrip from it.
        let orb = WorldDoc::from_json(ORB_RUSH_DUMP).unwrap();
        let scene = orb.compile_scene(1.0);
        assert!((scene.camera.fov_y - 38f32.to_radians()).abs() < 1e-5);
        assert!(!scene.batches.is_empty());
        assert!(scene.instance_count() >= 3, "stars + bomb + walker");
    }

    #[test]
    fn world_dump_rejects_unknown_version() {
        let err = WorldDoc::from_json("{\"version\": 2}").unwrap_err();
        assert!(err.contains("version"), "{err}");
    }

    #[test]
    fn compile_meshes_cover_batch_ids() {
        let meshes = compile_meshes();
        assert_eq!(meshes.len(), 5);
        assert!(meshes.iter().all(|(_, m)| !m.vertices.is_empty()));
        for json in [CREST_ISLE_DUMP, ORB_RUSH_DUMP] {
            let doc = WorldDoc::from_json(json).unwrap();
            let compiled = doc.compile_meshes();
            assert!(compiled.iter().all(|(_, m)| !m.vertices.is_empty()));
            let ids: std::collections::HashSet<_> = compiled.iter().map(|(id, _)| id.0).collect();
            let scene = doc.compile_scene(1.0);
            for batch in &scene.batches {
                assert!(
                    ids.contains(&batch.mesh.0),
                    "compiled batch mesh {} is not in WorldDoc::compile_meshes()",
                    batch.mesh.0
                );
            }
        }
    }

    #[test]
    fn compile_scene_emits_heightfield_and_gltf_batches() {
        let mut doc = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        doc.props.push(WorldProp {
            id: "prop:cube".into(),
            kind: "prop".into(),
            name: "cube".into(),
            position: [1.0, 1.2, -2.0],
            gltf: Some("cube.glb".into()),
            scale: [1.0, 1.0, 1.0],
            enabled: true,
            ..Default::default()
        });
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.batches.iter().any(|b| b.mesh == MESH_HEIGHTFIELD),
            "Crest dump must emit a heightfield batch, not a flat plane stand-in"
        );
        assert!(
            scene.batches.iter().any(|b| b.mesh.0 >= MESH_GLTF_BASE),
            "glTF cube.glb prop must get its own mesh slot"
        );
        let meshes = doc.compile_meshes();
        let hf = meshes
            .iter()
            .find(|(id, _)| *id == MESH_HEIGHTFIELD)
            .expect("heightfield mesh");
        assert!(
            hf.1.vertices.len() > 100,
            "heightfield grid, got {} verts",
            hf.1.vertices.len()
        );
        let gltf = meshes
            .iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("gltf mesh");
        assert_eq!(gltf.1.vertices.len(), 24);
        assert!((doc.height_at(0.0, -8.0) - open_world_height(0.0, -8.0)).abs() < 1e-4);
    }

    #[test]
    fn height_at_uses_samples_when_fn_unknown() {
        let mut doc = WorldDoc::from_json(ORB_RUSH_DUMP).unwrap();
        doc.heightfield = Some(WorldHeightfield {
            fn_name: Some("unknown_live_fn".into()),
            samples: vec![[0.0, 0.0, 1.5], [4.0, 0.0, 3.0]],
            ..Default::default()
        });
        assert!((doc.height_at(0.1, 0.0) - 1.5).abs() < 1e-4);
        assert!((doc.height_at(4.0, 0.1) - 3.0).abs() < 1e-4);
        let scene = doc.compile_scene(1.0);
        assert!(scene.batches.iter().any(|b| b.mesh == MESH_HEIGHTFIELD));
    }

    #[test]
    fn named_island_height_matches_python_shelf() {
        let y = island_height(0.0, 0.0);
        assert!((y - 0.38).abs() < 0.05, "{y}");
        let doc = WorldDoc {
            version: WORLD_DUMP_VERSION,
            heightfield: Some(WorldHeightfield {
                fn_name: Some("island_height".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!((doc.height_at(0.0, 0.0) - y).abs() < 1e-5);
    }

    #[test]
    fn heightfield_mesh_is_an_island_not_a_plane() {
        let doc = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        let meshes = doc.compile_meshes();
        let hf = meshes
            .iter()
            .find(|(id, _)| *id == MESH_HEIGHTFIELD)
            .expect("heightfield");
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        for v in &hf.1.vertices {
            min_y = min_y.min(v.pos[1]);
            max_y = max_y.max(v.pos[1]);
        }
        assert!(max_y - min_y > 6.0, "island relief, span {}", max_y - min_y);
        assert!(
            hf.1.vertices.len() > 400,
            "got {} verts",
            hf.1.vertices.len()
        );
        let scene = doc.compile_scene(16.0 / 9.0);
        let grass = scene
            .batches
            .iter()
            .find(|b| b.mesh == MESH_HEIGHTFIELD)
            .expect("heightfield batch");
        assert!(
            grass
                .instances
                .iter()
                .all(|i| i.material == Material::Grass),
            "heightfield albedo is Grass, not a bald Lambert plane"
        );
    }

    #[test]
    fn coins_compile_as_metal() {
        let mut doc = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        if !doc.props.iter().any(|p| p.name == "coin") {
            doc.props.push(WorldProp {
                id: "prop:coin".into(),
                kind: "prop".into(),
                name: "coin".into(),
                position: [2.3, 1.1, -1.0],
                model: "sphere".into(),
                scale: [0.42, 0.08, 0.42],
                enabled: true,
                color: Some([255, 208, 64]),
                metallic: 1.0,
                roughness: 0.12,
                ..Default::default()
            });
        }
        let scene = doc.compile_scene(16.0 / 9.0);
        let metal = scene
            .batches
            .iter()
            .flat_map(|b| b.instances.iter())
            .filter(|i| i.material == Material::Metal)
            .count();
        assert!(
            metal >= 1,
            "coin must compile as Material::Metal, got {metal}"
        );
    }

    #[test]
    fn light_slots_are_one_to_one_no_leak() {
        let mut doc = WorldDoc::from_json(ORB_RUSH_DUMP).unwrap();
        doc.lights = vec![
            WorldLight {
                id: "light:0".into(),
                kind_type: "light".into(),
                name: "key".into(),
                position: [6.0, 18.0, -8.0],
                kind: "spot".into(),
                slot: 0,
                intensity: 1.15,
                radius: 36.0,
                color: Some([1.0, 0.96, 0.86]),
                direction: Some([-0.18, -1.0, 0.22]),
            },
            WorldLight {
                id: "light:2".into(),
                kind_type: "light".into(),
                name: "fill".into(),
                position: [-8.0, 10.0, 4.0],
                kind: "point".into(),
                slot: 2,
                intensity: 0.55,
                radius: 22.0,
                color: Some([0.6, 0.7, 1.0]),
                direction: None,
            },
            WorldLight {
                id: "light:9".into(),
                kind_type: "light".into(),
                name: "overflow".into(),
                position: [0.0, 50.0, 0.0],
                kind: "point".into(),
                slot: 9,
                intensity: 9.0,
                radius: 1.0,
                color: Some([1.0, 0.0, 0.0]),
                direction: None,
            },
        ];
        let scene = doc.compile_scene(1.0);
        assert!(scene.local_lights[0].intensity > 1.0);
        assert!((scene.local_lights[0].position.x - 6.0).abs() < 1e-4);
        assert!(
            scene.local_lights[1].intensity == 0.0,
            "slot 1 must stay empty (no leak from 0 or 2)"
        );
        assert!((scene.local_lights[2].intensity - 0.55).abs() < 1e-4);
        assert!(scene.local_lights[2].position.x < 0.0);
        assert!(
            scene.local_lights[3].intensity == 0.0,
            "slot 9 must not land in slot 3"
        );
        assert!(scene.local_lights[0].spot);
        assert!(!scene.local_lights[2].spot);
    }

    #[test]
    fn empty_lights_get_default_key_and_fill() {
        let doc = WorldDoc::from_json(ORB_RUSH_DUMP).unwrap();
        assert!(
            doc.lights.is_empty(),
            "orb rush fixture must stay empty in the dump"
        );
        let scene = doc.compile_scene(1.0);
        assert!(
            scene.local_lights[0].intensity > 1.0,
            "slot 0 key, got {}",
            scene.local_lights[0].intensity
        );
        assert!(scene.local_lights[0].spot, "key is the crest-style spot");
        assert!(
            scene.local_lights[1].intensity > 0.3,
            "slot 1 fill, got {}",
            scene.local_lights[1].intensity
        );
        assert!(!scene.local_lights[1].spot);
        assert_eq!(
            scene.local_lights[2].intensity, 0.0,
            "empty dump must not invent slot 2"
        );
        assert_eq!(
            scene.local_lights[3].intensity, 0.0,
            "empty dump must not invent slot 3"
        );
        assert_eq!(scene.local_lights.len(), 4);
        // compile_scene must not mutate the dump
        assert!(doc.lights.is_empty());
    }

    #[test]
    fn outdoor_dump_defaults_ibl_and_aces() {
        let empty = WorldDoc::from_json(ORB_RUSH_DUMP).unwrap();
        assert!(
            empty.ibl.is_none() && empty.tonemap.is_none(),
            "fixtures omit look fields"
        );
        let scene = empty.compile_scene(1.0);
        assert!((scene.ibl - 0.35).abs() < 1e-5, "default IBL {}", scene.ibl);
        assert!((scene.exposure - 1.0).abs() < 1e-5);
        assert!(scene.tonemap, "ACES default on");
        let crest = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        let cs = crest.compile_scene(16.0 / 9.0);
        assert!(cs.tonemap);
        assert!(cs.ibl > 0.0);
        let off: WorldDoc =
            serde_json::from_str(r#"{"version":1,"tonemap":false,"ibl":0.0}"#).unwrap();
        let s = off.compile_scene(1.0);
        assert!(!s.tonemap);
        assert_eq!(s.ibl, 0.0);
    }

    #[test]
    fn tonemap_curve_clearcoat_and_sky_reach_the_scene() {
        let doc: WorldDoc = serde_json::from_str(
            r#"{"version":1,"tonemap_curve":"agx","sky_atmosphere":true,"ground":false,"props":[{"id":"car","type":"prop","model":"box","position":[0.0,1.0,0.0],"clearcoat":0.8,"clearcoat_roughness":0.15}]}"#,
        )
        .unwrap();
        let scene = doc.compile_scene(1.0);
        assert!((scene.tonemap_code - 1.0).abs() < 1e-5);
        assert!(scene.sky_atmosphere);
        let coat = scene
            .batches
            .iter()
            .flat_map(|b| &b.instances)
            .find(|i| i.clearcoat > 0.0)
            .expect("coated box");
        assert!((coat.clearcoat - 0.8).abs() < 1e-4);
        assert!((coat.clearcoat_roughness - 0.15).abs() < 1e-4);
        let neutral: WorldDoc =
            serde_json::from_str(r#"{"version":1,"tonemap_curve":"Neutral"}"#).unwrap();
        assert!((neutral.compile_scene(1.0).tonemap_code - 2.0).abs() < 1e-5);
        let aces: WorldDoc =
            serde_json::from_str(r#"{"version":1,"tonemap_curve":"film"}"#).unwrap();
        assert_eq!(aces.compile_scene(1.0).tonemap_code, 0.0);
        assert!(!aces.compile_scene(1.0).sky_atmosphere);
    }

    #[test]
    fn post_fields_default_to_the_legacy_picture() {
        let doc = WorldDoc::from_json(r#"{"version":1}"#).unwrap();
        assert_eq!(doc.bloom_intensity(), 0.0);
        assert!((doc.bloom_threshold() - 0.85).abs() < 1e-6);
        assert!((doc.vignette_strength() - 1.0).abs() < 1e-6);
        assert_eq!(doc.lut_preset(), "off");
        assert_eq!(doc.lut_mix(), 0.0);
        assert!(doc.fxaa_enabled());
        if super::heavy_post_allowed() && !super::quality_is_high() {
            assert_eq!(doc.ssao_radius(), 0.0);
            assert!(!doc.taa_enabled());
        }
        assert!(doc.dof().is_none());
        assert_eq!(doc.motion_shutter(), 0.0);
        assert!((doc.shadow_softness() - 1.0).abs() < 1e-6);
        assert!(doc.baked_env().is_none(), "omit env_map and stay on the SH");
        assert_eq!(doc.hemi_sky_color(), [0.48, 0.56, 0.78]);
        assert_eq!(doc.hemi_ground_color(), [0.42, 0.30, 0.18]);
        assert!(doc.cull.is_none());
        assert!(doc.prop_lod.is_none());
        let legacy = doc.to_json().unwrap();
        assert!(!legacy.contains("\"bloom\""));
        assert!(!legacy.contains("\"ssao\""));
        assert!(!legacy.contains("\"taa\""));
        assert!(!legacy.contains("\"dof_focus\""));
        assert!(!legacy.contains("\"motion_blur\""));
        assert!(!legacy.contains("\"shadow_softness\""));
        assert!(!legacy.contains("\"shadow_cascades\""));
        assert!(!legacy.contains("\"shadow_pcss\""));
        assert!(!legacy.contains("\"quality\""));
        if super::heavy_post_allowed() && !super::quality_is_high() {
            assert_eq!(doc.shadow_cascade_count(), 1);
            assert!(!doc.shadow_pcss_enabled());
        }
        assert!(!legacy.contains("\"hemi_sky\""));
        assert!(!legacy.contains("\"env_map\""));
        assert!(!legacy.contains("\"cull\""));
        assert!(!legacy.contains("\"prop_lod\""));
        let on = WorldDoc::from_json(
            r#"{"version":1,"bloom":0.4,"bloom_threshold":0.7,"vignette":0.0,"lut":"warm","lut_strength":0.5,"fxaa":false}"#,
        )
        .unwrap();
        assert!((on.bloom_intensity() - 0.4).abs() < 1e-6);
        assert!((on.bloom_threshold() - 0.7).abs() < 1e-6);
        assert_eq!(on.vignette_strength(), 0.0);
        assert_eq!(on.lut_preset(), "warm");
        assert!((on.lut_mix() - 0.5).abs() < 1e-6);
        assert!(!on.fxaa_enabled());
        let post = WorldDoc::from_json(
            r#"{"version":1,"ssao":12.0,"taa":true,"dof_focus":4.0,"dof_aperture":0.35,"motion_blur":0.5}"#,
        )
        .unwrap();
        if super::heavy_post_allowed() {
            assert!((post.ssao_radius() - 12.0).abs() < 1e-6);
            assert!(post.taa_enabled());
            assert_eq!(post.dof(), Some((4.0, 0.35)));
            assert!((post.motion_shutter() - 0.5).abs() < 1e-6);
        } else {
            assert_eq!(post.ssao_radius(), 0.0);
            assert!(!post.taa_enabled());
            assert!(post.dof().is_none());
            assert_eq!(post.motion_shutter(), 0.0);
        }
    }

    #[test]
    fn quality_tier_gates_heavy_post() {
        assert!(super::heavy_post_allowed_value(None));
        assert!(super::heavy_post_allowed_value(Some("1")));
        assert!(!super::heavy_post_allowed_value(Some("0")));
        assert!(!super::heavy_post_allowed_value(Some("off")));
        assert!(!super::heavy_post_allowed_value(Some("low")));
        assert!(super::quality_is_high_value(Some("high")));
        assert!(super::quality_is_high_value(Some("2")));
        assert!(!super::quality_is_high_value(None));
        assert!(!super::quality_is_high_value(Some("1")));
    }

    #[test]
    fn picture_tier_resolves_dump_ahead_of_env_high() {
        assert_eq!(
            super::picture_tier_value(None, None),
            super::PictureTier::Legacy
        );
        assert_eq!(
            super::picture_tier_value(None, Some("high")),
            super::PictureTier::High
        );
        assert_eq!(
            super::picture_tier_value(Some("mobile"), Some("high")),
            super::PictureTier::Mobile
        );
        assert_eq!(
            super::picture_tier_value(Some(" high "), None),
            super::PictureTier::High
        );
        assert_eq!(
            super::picture_tier_value(Some("low"), Some("high")),
            super::PictureTier::Low
        );
        assert_eq!(
            super::picture_tier_value(Some("mobile"), Some("0")),
            super::PictureTier::Low
        );
        assert_eq!(
            super::picture_tier_value(Some("nope"), None),
            super::PictureTier::Legacy
        );
        assert_eq!(
            super::picture_tier_value(Some("2"), Some("1")),
            super::PictureTier::High
        );
    }

    #[test]
    fn quality_dump_tiers_fill_omitted_fields() {
        let mobile = WorldDoc::from_json(r#"{"version":1,"quality":"mobile"}"#).unwrap();
        let high = WorldDoc::from_json(r#"{"version":1,"quality":"high"}"#).unwrap();
        let low = WorldDoc::from_json(
            r#"{"version":1,"quality":"low","ssao":12.0,"taa":true,"shadow_cascades":3,"shadow_pcss":true,"dof_focus":4.0,"dof_aperture":0.35,"motion_blur":0.5,"bloom":0.4}"#,
        )
        .unwrap();
        let explicit = WorldDoc::from_json(
            r#"{"version":1,"quality":"mobile","ssao":1.5,"taa":true,"shadow_cascades":1,"shadow_pcss":true,"bloom":0.05}"#,
        )
        .unwrap();
        assert!((low.bloom_intensity() - 0.4).abs() < 1e-6);
        assert_eq!(low.ssao_radius(), 0.0);
        assert!(!low.taa_enabled());
        assert!(low.dof().is_none());
        assert_eq!(low.motion_shutter(), 0.0);
        assert_eq!(low.shadow_cascade_count(), 1);
        assert!(!low.shadow_pcss_enabled());
        if super::heavy_post_allowed() {
            assert!((mobile.ssao_radius() - 4.0).abs() < 1e-6);
            assert!(!mobile.taa_enabled());
            assert_eq!(mobile.shadow_cascade_count(), 2);
            assert!(!mobile.shadow_pcss_enabled());
            assert!((mobile.bloom_intensity() - 0.22).abs() < 1e-6);
            assert!(mobile.dof().is_none());
            assert!((high.ssao_radius() - 8.0).abs() < 1e-6);
            assert!(high.taa_enabled());
            assert_eq!(high.shadow_cascade_count(), 3);
            assert!(high.shadow_pcss_enabled());
            assert!((high.bloom_intensity() - 0.35).abs() < 1e-6);
            assert!(high.dof().is_none());
            assert!((explicit.ssao_radius() - 1.5).abs() < 1e-6);
            assert!(explicit.taa_enabled());
            assert_eq!(explicit.shadow_cascade_count(), 1);
            assert!(explicit.shadow_pcss_enabled());
            assert!((explicit.bloom_intensity() - 0.05).abs() < 1e-6);
        } else {
            assert_eq!(mobile.ssao_radius(), 0.0);
            assert_eq!(mobile.shadow_cascade_count(), 1);
            assert_eq!(mobile.bloom_intensity(), 0.0);
            assert_eq!(high.ssao_radius(), 0.0);
            assert!(!high.taa_enabled());
            assert_eq!(high.shadow_cascade_count(), 1);
            assert!(!high.shadow_pcss_enabled());
            assert_eq!(high.bloom_intensity(), 0.0);
        }
        let again = mobile.to_json().unwrap();
        assert!(again.contains("\"quality\": \"mobile\""));
    }

    #[test]
    fn shadow_cascades_follow_the_dump() {
        let doc =
            WorldDoc::from_json(r#"{"version":1,"shadow_cascades":3,"shadow_pcss":true}"#).unwrap();
        if super::heavy_post_allowed() {
            assert_eq!(doc.shadow_cascade_count(), 3);
            assert!(doc.shadow_pcss_enabled());
            let scene = doc.compile_scene(1.0);
            assert_eq!(scene.shadow_cascades, 3);
            assert!(scene.shadow_pcss);
        } else {
            assert_eq!(doc.shadow_cascade_count(), 1);
            assert!(!doc.shadow_pcss_enabled());
        }
        let one = WorldDoc::from_json(r#"{"version":1,"shadow_cascades":1,"shadow_pcss":false}"#)
            .unwrap();
        assert_eq!(one.shadow_cascade_count(), 1);
        assert!(!one.shadow_pcss_enabled());
    }

    #[test]
    fn emissive_and_orm_reach_the_picture() {
        let doc = WorldDoc::from_json(
            r#"{"version":1,"ground":false,"shadow_softness":3.0,"props":[{"id":"lamp","type":"prop","model":"box","position":[0.0,1.0,0.0],"emissive":2.5,"emissive_color":[1.0,0.4,0.1],"roughness_map":"preset:split","metalness_map":"preset:split"}]}"#,
        )
        .unwrap();
        assert!((doc.shadow_softness() - 3.0).abs() < 1e-6);
        let scene = doc.compile_scene(1.0);
        assert!((scene.shadow_softness - 3.0).abs() < 1e-6);
        let lamp = scene
            .batches
            .iter()
            .find(|b| b.mesh.0 >= MESH_GLTF_BASE)
            .expect("mapped box");
        let inst = &lamp.instances[0];
        assert!(
            (inst.emissive - 2.5).abs() < 1e-4,
            "emissive {}",
            inst.emissive
        );
        assert!((inst.emissive_color[0] - 1.0).abs() < 1e-4);
        let meshes = doc.compile_meshes();
        let mapped = meshes
            .iter()
            .find(|(id, m)| id.0 >= MESH_GLTF_BASE && m.orm.is_some())
            .expect("orm mesh");
        assert!(mapped.1.orm_rough && mapped.1.orm_metal);
        let img = mapped.1.orm.as_ref().unwrap();
        let left_b = img.rgba[2];
        let right_g = img.rgba[(63 * 4 + 1) as usize];
        let right_b = img.rgba[(63 * 4 + 2) as usize];
        assert!(left_b > 200, "split left should be metal, B={left_b}");
        assert!(right_g > 200, "split right should be rough, G={right_g}");
        assert!(
            right_b < 20,
            "split right should be dielectric, B={right_b}"
        );
    }

    #[test]
    fn studio_env_zenith_is_bluer_than_the_nadir() {
        let doc = WorldDoc::from_json(
            r#"{"version":1,"hemi_sky":[0.20,0.45,0.95],"hemi_ground":[0.55,0.22,0.08],"env_map":"preset:studio"}"#,
        )
        .unwrap();
        let px = doc.baked_env().expect("strip");
        assert_eq!(px.len(), 64 * 64 * 4);
        let scene = doc.compile_scene(1.0);
        assert!(scene
            .env_pixels
            .as_ref()
            .is_some_and(|p| p.len() == px.len()));
        assert!((scene.hemi_sky[2] - 0.95).abs() < 1e-5);
        // First roughness row: y=0 is the zenith texel, y=15 is the nadir.
        let zen = 32 * 4;
        let nad = (15 * 64 + 32) * 4;
        assert!(
            px[zen + 2] > px[zen] + 40,
            "zenith should be blue, rgb={},{},{}",
            px[zen],
            px[zen + 1],
            px[zen + 2]
        );
        assert!(
            px[nad] > px[nad + 2] + 40,
            "nadir should be warm, rgb={},{},{}",
            px[nad],
            px[nad + 1],
            px[nad + 2]
        );
        // The sun disk is a few texels wide. Row 3 blurs it, so the peak drops.
        let mut peak = 0usize;
        let mut peak_l = 0u32;
        for i in 0..(16 * 64) {
            let o = i * 4;
            let l = px[o] as u32 + px[o + 1] as u32 + px[o + 2] as u32;
            if l > peak_l {
                peak_l = l;
                peak = i;
            }
        }
        let blurred = (peak + 3 * 16 * 64) * 4;
        let blur_l = px[blurred] as u32 + px[blurred + 1] as u32 + px[blurred + 2] as u32;
        assert!(
            peak_l > blur_l + 20,
            "roughness row 3 should blur the sun, {peak_l} -> {blur_l}"
        );
    }

    #[test]
    fn walker_scale_reaches_the_capsule() {
        let plain = WorldDoc::from_json(
            r#"{"version":1,"ground":false,"player":{"id":"p","type":"walker","position":[0.0,1.0,0.0]}}"#,
        )
        .unwrap();
        let scene = plain.compile_scene(1.0);
        let body = scene
            .batches
            .iter()
            .find(|b| b.mesh == MESH_CAPSULE)
            .expect("capsule");
        let sy = body.instances[0].model.y_axis.length();
        assert!((sy - 0.95).abs() < 1e-3, "default height {sy}");
        let squashed = WorldDoc::from_json(
            r#"{"version":1,"ground":false,"player":{"id":"p","type":"walker","position":[0.0,1.0,0.0],"scale":[2.0,0.5,2.0]}}"#,
        )
        .unwrap();
        let scene = squashed.compile_scene(1.0);
        let body = scene
            .batches
            .iter()
            .find(|b| b.mesh == MESH_CAPSULE)
            .expect("squashed capsule");
        let sy = body.instances[0].model.y_axis.length();
        let sx = body.instances[0].model.x_axis.length();
        assert!((sy - 0.475).abs() < 1e-3, "squashed height {sy}");
        assert!((sx - 1.12).abs() < 1e-3, "squashed width {sx}");
    }

    #[test]
    fn cull_drops_a_box_behind_the_camera() {
        let doc = |cull: &str| {
            WorldDoc::from_json(&format!(
                r#"{{"version":1,"ground":false,{cull}"cameras":[{{"id":"c","type":"camera","position":[0.0,2.0,6.0],"target":[0.0,1.0,0.0],"fov":50.0}}],"props":[{{"id":"front","type":"prop","model":"box","position":[0.0,1.0,0.0]}},{{"id":"back","type":"prop","model":"box","position":[0.0,1.0,40.0]}}]}}"#
            ))
            .unwrap()
        };
        let boxes = |scene: Scene3D| {
            scene
                .batches
                .iter()
                .filter(|b| b.mesh == MESH_BOX)
                .map(|b| b.instances.len())
                .sum::<usize>()
        };
        let open = doc("");
        let closed = doc(r#""cull":true,"#);
        assert_eq!(
            boxes(open.compile_scene(1.0)),
            2,
            "omit cull and both boxes draw"
        );
        assert_eq!(
            boxes(closed.compile_scene(1.0)),
            1,
            "the box behind the camera drops"
        );
    }

    /// The per-frame cull bounds are cached, and the cache can never give another doc's answer.
    #[test]
    fn cull_bounds_cache_matches_a_fresh_build_and_follows_the_models() {
        let fresh = |doc: &WorldDoc| -> Vec<(MeshId, crate::scene3d::Aabb)> {
            doc.compile_meshes()
                .into_iter()
                .filter_map(|(id, m)| {
                    let b = m.bounds();
                    (!b.is_empty()).then_some((id, b))
                })
                .collect()
        };
        let mut a = WorldDoc::from_json(r#"{"version":1,"cull":true,"props":[]}"#).unwrap();
        a.props.push(WorldProp {
            id: "m".into(),
            gltf: Some(crate::gltf_load::unit_cube_gltf()),
            ..serde_json::from_str::<WorldProp>(r#"{"id":"m"}"#).unwrap()
        });
        let want = fresh(&a);
        assert_eq!(a.cull_bounds(), want, "first call builds");
        assert_eq!(a.cull_bounds(), want, "second call is the cached copy");
        // A different model set has another key, so it is rebuilt, not served from the cache.
        let b = WorldDoc::from_json(r#"{"version":1,"cull":true,"props":[]}"#).unwrap();
        assert_eq!(b.cull_bounds(), fresh(&b));
        assert_ne!(b.cull_bounds().len(), a.cull_bounds().len());
        // Many more docs than the cache holds still answer correctly.
        for i in 0..6 {
            let mut c = b.clone();
            c.props.push(WorldProp {
                id: format!("p{i}"),
                gltf: Some(crate::gltf_load::unit_cube_gltf()),
                name: format!("{i}"),
                ..serde_json::from_str::<WorldProp>(r#"{"id":"m"}"#).unwrap()
            });
            assert_eq!(c.cull_bounds(), fresh(&c));
        }
        assert_eq!(a.cull_bounds(), want);
    }

    #[test]
    fn the_baked_env_strip_is_kept_but_follows_the_sky() {
        let mut doc =
            WorldDoc::from_json(r#"{"version":1,"env_map":"preset:studio","props":[]}"#).unwrap();
        let first = doc.baked_env().expect("strip");
        assert_eq!(
            doc.baked_env().as_ref(),
            Some(&first),
            "same inputs, same strip"
        );
        doc.hemi_sky = Some([0.9, 0.1, 0.1]);
        let red = doc.baked_env().expect("strip");
        assert_ne!(red, first, "a new sky is a new strip, not the cached one");
        assert_eq!(
            red,
            bake_env_strip(doc.hemi_sky_color(), doc.hemi_ground_color(), None)
        );
    }

    #[test]
    fn prop_lod_turns_a_far_box_into_a_quad() {
        let doc = WorldDoc::from_json(
            r#"{"version":1,"ground":false,"prop_lod":4.0,"cameras":[{"id":"c","type":"camera","position":[0.0,2.0,0.0],"target":[0.0,1.0,-1.0],"fov":50.0}],"props":[{"id":"near","type":"prop","model":"box","position":[0.0,1.0,-2.0]},{"id":"far","type":"prop","model":"box","position":[0.0,1.0,-40.0]}]}"#,
        )
        .unwrap();
        let scene = doc.compile_scene(1.0);
        let n = |id: MeshId| {
            scene
                .batches
                .iter()
                .filter(|b| b.mesh == id)
                .map(|b| b.instances.len())
                .sum::<usize>()
        };
        assert_eq!(n(MESH_BOX), 1, "the near box stays a box");
        assert!(n(MESH_QUAD) >= 1, "the far box becomes a billboard");
        let full = WorldDoc::from_json(
            r#"{"version":1,"ground":false,"cameras":[{"id":"c","type":"camera","position":[0.0,2.0,0.0],"target":[0.0,1.0,-1.0],"fov":50.0}],"props":[{"id":"near","type":"prop","model":"box","position":[0.0,1.0,-2.0]},{"id":"far","type":"prop","model":"box","position":[0.0,1.0,-40.0]}]}"#,
        )
        .unwrap();
        let scene = full.compile_scene(1.0);
        let boxes = scene
            .batches
            .iter()
            .filter(|b| b.mesh == MESH_BOX)
            .map(|b| b.instances.len())
            .sum::<usize>();
        assert_eq!(boxes, 2, "omit prop_lod and both boxes stay boxes");
    }

    #[test]
    fn crest_keeps_dumped_key_fill_rim() {
        let doc = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        assert_eq!(doc.lights.len(), 3);
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!((scene.local_lights[0].intensity - 1.15).abs() < 1e-4);
        assert!((scene.local_lights[1].intensity - 0.45).abs() < 1e-4);
        assert!((scene.local_lights[2].intensity - 0.35).abs() < 1e-4);
        assert_eq!(scene.local_lights[3].intensity, 0.0);
    }

    #[test]
    fn coin_without_metallic_field_still_compiles_as_metal() {
        let mut doc = WorldDoc::from_json(ORB_RUSH_DUMP).unwrap();
        doc.props.push(WorldProp {
            id: "prop:coin-bare".into(),
            kind: "prop".into(),
            name: "coin".into(),
            position: [0.5, 0.4, 0.5],
            model: "sphere".into(),
            scale: [0.4, 0.08, 0.4],
            enabled: true,
            color: Some([255, 208, 64]),
            ..Default::default()
        });
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "coin" && p.metallic < 0.5));
        let scene = doc.compile_scene(1.0);
        let metal = scene
            .batches
            .iter()
            .flat_map(|b| b.instances.iter())
            .filter(|i| i.material == Material::Metal)
            .count();
        assert!(
            metal >= 1,
            "bare coin must still be Material::Metal, got {metal}"
        );
    }

    #[test]
    fn water_prop_and_water_y_compile_as_water_material() {
        let crest = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        let cs = crest.compile_scene(16.0 / 9.0);
        let crest_water = cs
            .batches
            .iter()
            .flat_map(|b| b.instances.iter())
            .filter(|i| i.material == Material::Water)
            .count();
        assert!(
            crest_water >= 1,
            "Crest water_y plane must be Material::Water, got {crest_water}"
        );

        const WATER: &str = include_str!("../tests/fixtures/water_plane_world.json");
        let doc = WorldDoc::from_json(WATER).unwrap();
        assert!(
            doc.props
                .iter()
                .any(|p| p.name.eq_ignore_ascii_case("water")),
            "fixture names a water prop (existing dump style)"
        );
        let scene = doc.compile_scene(16.0 / 9.0);
        let n = scene
            .batches
            .iter()
            .flat_map(|b| b.instances.iter())
            .filter(|i| i.material == Material::Water)
            .count();
        assert!(n >= 1, "lake prop must compile as Material::Water, got {n}");

        const EMMA: &str = include_str!("../tests/fixtures/emma_walker_world.json");
        let emma = WorldDoc::from_json(EMMA).unwrap();
        let es = emma.compile_scene(16.0 / 9.0);
        assert!(es.instance_count() >= 1, "emma_walker still compiles");
    }

    #[test]
    fn contact_blob_under_capsule_and_prop() {
        let crest = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        let scene = crest.compile_scene(16.0 / 9.0);
        let blobs: Vec<_> = scene
            .batches
            .iter()
            .filter(|b| b.mesh == MESH_PLANE)
            .flat_map(|b| b.instances.iter())
            .filter(|i| i.color[3] < 200)
            .collect();
        assert!(
            blobs.len() >= 2,
            "walker + crate/coin need ground contact blobs, got {}",
            blobs.len()
        );
        let walker = crest.player.as_ref().unwrap().position;
        let gy = crest.height_at(walker[0], walker[2]);
        assert!(
            blobs.iter().any(|i| {
                let t = i.model.w_axis;
                (t.x - walker[0]).abs() < 0.05
                    && (t.z - walker[2]).abs() < 0.05
                    && (t.y - (gy + 0.03)).abs() < 0.05
            }),
            "walker contact blob must sit on the heightfield"
        );

        let orb = WorldDoc::from_json(ORB_RUSH_DUMP).unwrap();
        let scene = orb.compile_scene(1.0);
        let blobs: Vec<_> = scene
            .batches
            .iter()
            .filter(|b| b.mesh == MESH_PLANE)
            .flat_map(|b| b.instances.iter())
            .filter(|i| i.color[3] < 200)
            .collect();
        assert!(
            blobs.len() >= 4,
            "orb rush: walker + 3 props, got {}",
            blobs.len()
        );
    }

    #[test]
    fn walker_gltf_path_is_dump_queryable_and_compiles_skinned_not_capsule() {
        const DUMP: &str = include_str!("../tests/fixtures/skinned_walker_world.json");
        let doc = WorldDoc::from_json(DUMP).expect("parse");
        let w = doc.player.as_ref().expect("player");
        assert_eq!(w.id, "walker:player");
        assert_eq!(w.gltf.as_deref(), Some("walk_skinned.gltf"));
        assert_eq!(w.model, "capsule");
        assert_eq!(w.clip, 0.0);
        let json = doc.to_json().unwrap();
        assert!(json.contains("walk_skinned.gltf"), "gltf path queryable");
        let scene = doc.compile_scene(1.0);
        assert!(
            scene.batches.iter().any(|b| b.mesh.0 >= MESH_GLTF_BASE),
            "skinned walker must use a glTF mesh slot, not the capsule"
        );
        assert!(
            !scene.batches.iter().any(|b| b.mesh == MESH_CAPSULE),
            "capsule fallback must not draw when walker names a glTF"
        );
        let meshes = doc.compile_meshes();
        let skinned = meshes
            .iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("skinned mesh");
        assert_eq!(skinned.1.vertices.len(), 8);
        assert_eq!(skinned.1.indices.len(), 36);
        assert!(
            skinned.1.vertices.iter().any(|v| v.uv != [0.0, 0.0]),
            "skinned walker must sample TEXCOORD_0"
        );
        let alb = skinned.1.albedo.as_ref().expect("baseColor on walker mesh");
        assert!(alb.width >= 2 && alb.height >= 2);
        let batch = scene
            .batches
            .iter()
            .find(|b| b.mesh.0 >= MESH_GLTF_BASE)
            .expect("walker batch");
        assert_eq!(
            batch.instances[0].color,
            [255, 255, 255, 255],
            "textured walker uses white instance color so the PNG shows"
        );
    }

    #[test]
    fn walker_clip_changes_skinned_vertices() {
        const DUMP: &str = include_str!("../tests/fixtures/skinned_walker_world.json");
        let mut rest = WorldDoc::from_json(DUMP).unwrap();
        rest.player.as_mut().unwrap().clip = 0.0;
        let mut walk = rest.clone();
        walk.player.as_mut().unwrap().clip = 0.25;
        walk.player.as_mut().unwrap().anim_blend = 1.0; // フルクリップを見る
        if let Some(w) = walk.walkers.first_mut() {
            w.clip = 0.25;
            w.anim_blend = 1.0;
        }
        let a = rest
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .unwrap()
            .1;
        let b = walk
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .unwrap()
            .1;
        let mut max_d = 0.0f32;
        for (va, vb) in a.vertices.iter().zip(b.vertices.iter()) {
            let d = (Vec3::from_array(va.pos) - Vec3::from_array(vb.pos)).length();
            max_d = max_d.max(d);
        }
        assert!(
            max_d > 0.05,
            "clip 0.25 must move verts off T-pose, max_d={max_d}"
        );
    }

    #[test]
    fn walker_vrm_path_is_dump_queryable_and_compiles_not_capsule() {
        const DUMP: &str = include_str!("../tests/fixtures/vrm_walker_world.json");
        let doc = WorldDoc::from_json(DUMP).expect("parse");
        let w = doc.player.as_ref().expect("player");
        assert_eq!(w.id, "walker:player");
        assert_eq!(w.gltf.as_deref(), Some("walk_skinned.vrm"));
        assert_eq!(w.model, "capsule");
        let json = doc.to_json().unwrap();
        assert!(json.contains("walk_skinned.vrm"), "vrm path queryable");
        let scene = doc.compile_scene(1.0);
        assert!(
            scene.batches.iter().any(|b| b.mesh.0 >= MESH_GLTF_BASE),
            "VRM walker must use a glTF mesh slot, not the capsule"
        );
        assert!(
            !scene.batches.iter().any(|b| b.mesh == MESH_CAPSULE),
            "capsule fallback must not draw when walker names a VRM"
        );
        let meshes = doc.compile_meshes();
        let skinned = meshes
            .iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("vrm mesh");
        assert_eq!(skinned.1.vertices.len(), 8);
        assert_eq!(skinned.1.indices.len(), 36);
        assert!(skinned.1.vertices.iter().any(|v| v.uv != [0.0, 0.0]));
        assert!(skinned.1.albedo.as_ref().is_some_and(|a| a.width >= 2));
        assert_eq!(w.hair, 0.0);
        assert!(json.contains("\"hair\""), "hair yaw dump-visible");
        assert_eq!(w.look_yaw, 0.0);
        assert_eq!(w.look_pitch, 0.0);
        assert!(json.contains("look_yaw"), "look yaw dump-visible");
        assert!(json.contains("look_pitch"), "look pitch dump-visible");
    }

    #[test]
    fn walker_hair_yaw_moves_compiled_verts() {
        const DUMP: &str = include_str!("../tests/fixtures/vrm_walker_world.json");
        let mut rest = WorldDoc::from_json(DUMP).expect("parse");
        rest.player.as_mut().unwrap().hair = 0.0;
        if let Some(w) = rest.walkers.first_mut() {
            w.hair = 0.0;
        }
        let mut sag = rest.clone();
        sag.player.as_mut().unwrap().hair = 0.4;
        if let Some(w) = sag.walkers.first_mut() {
            w.hair = 0.4;
        }
        let rest_m = rest
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("rest")
            .1;
        let sag_m = sag
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("sag")
            .1;
        let mut max_d = 0.0f32;
        for (a, b) in rest_m.vertices.iter().zip(sag_m.vertices.iter()) {
            max_d =
                max_d.max((glam::Vec3::from_array(a.pos) - glam::Vec3::from_array(b.pos)).length());
        }
        assert!(
            max_d > 0.01,
            "dump hair yaw must move CPU-skinned verts, max_d={max_d}"
        );
    }

    #[test]
    fn walker_morph_weight_is_dump_queryable_and_moves_verts() {
        const DUMP: &str = include_str!("../tests/fixtures/vrm_walker_world.json");
        let mut rest = WorldDoc::from_json(DUMP).expect("parse");
        let w = rest.player.as_ref().unwrap();
        assert_eq!(w.morph, 0.0);
        let json = rest.to_json().expect("json");
        assert!(json.contains("\"morph\""), "morph weight dump-visible");
        rest.player.as_mut().unwrap().morph = 0.0;
        if let Some(w) = rest.walkers.first_mut() {
            w.morph = 0.0;
        }
        let mut blink = rest.clone();
        blink.player.as_mut().unwrap().morph = 1.0;
        if let Some(w) = blink.walkers.first_mut() {
            w.morph = 1.0;
        }
        let rest_m = rest
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("rest")
            .1;
        let blink_m = blink
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("blink")
            .1;
        let mut max_d = 0.0f32;
        for (a, b) in rest_m.vertices.iter().zip(blink_m.vertices.iter()) {
            max_d =
                max_d.max((glam::Vec3::from_array(a.pos) - glam::Vec3::from_array(b.pos)).length());
        }
        assert!(
            max_d > 0.05,
            "dump morph 1 must move CPU-skinned verts, max_d={max_d}"
        );
    }

    #[test]
    fn walker_look_yaw_is_dump_queryable_and_moves_verts() {
        const DUMP: &str = include_str!("../tests/fixtures/vrm_walker_world.json");
        let mut rest = WorldDoc::from_json(DUMP).expect("parse");
        let w = rest.player.as_ref().unwrap();
        assert_eq!(w.look_yaw, 0.0);
        assert_eq!(w.look_pitch, 0.0);
        let json = rest.to_json().expect("json");
        assert!(json.contains("look_yaw"), "look yaw dump-visible");
        rest.player.as_mut().unwrap().look_yaw = 0.0;
        if let Some(w) = rest.walkers.first_mut() {
            w.look_yaw = 0.0;
        }
        let mut turned = rest.clone();
        turned.player.as_mut().unwrap().look_yaw = 0.6;
        turned.player.as_mut().unwrap().look_pitch = 0.2;
        if let Some(w) = turned.walkers.first_mut() {
            w.look_yaw = 0.6;
            w.look_pitch = 0.2;
        }
        let rest_m = rest
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("rest")
            .1;
        let look_m = turned
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("look")
            .1;
        let mut max_d = 0.0f32;
        for (a, b) in rest_m.vertices.iter().zip(look_m.vertices.iter()) {
            max_d =
                max_d.max((glam::Vec3::from_array(a.pos) - glam::Vec3::from_array(b.pos)).length());
        }
        assert!(
            max_d > 0.01,
            "dump look yaw/pitch must move CPU-skinned verts, max_d={max_d}"
        );
    }

    #[test]
    fn crest_isle_walker_stays_capsule_not_vrm() {
        let doc = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        let w = doc.player.as_ref().unwrap();
        assert!(w.gltf.is_none() || w.gltf.as_deref() == Some(""));
        assert!(!w.model.to_ascii_lowercase().ends_with(".vrm"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.batches.iter().any(|b| b.mesh == MESH_CAPSULE),
            "Crest walker stays capsule unless dump names glTF/VRM"
        );
        assert!(
            scene
                .batches
                .iter()
                .flat_map(|b| b.instances.iter())
                .all(|i| i.material != Material::Toon),
            "Crest Isle must not pick up walker MToon"
        );
    }

    #[test]
    fn vrm_walker_compiles_as_toon_not_lambert() {
        const DUMP: &str = include_str!("../tests/fixtures/vrm_walker_world.json");
        let doc = WorldDoc::from_json(DUMP).unwrap();
        let scene = doc.compile_scene(1.0);
        let batch = scene
            .batches
            .iter()
            .find(|b| b.mesh.0 >= MESH_GLTF_BASE)
            .expect("vrm walker batch");
        assert_eq!(
            batch.instances[0].material,
            Material::Toon,
            "VRM walker with MToon must not be a plastic Lambert blob"
        );
        let meshes = doc.compile_meshes();
        let skinned = meshes
            .iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("vrm mesh");
        let m = skinned.1.mtoon.expect("mtoon on compiled vrm mesh");
        assert!(m.shade_color[0] < 0.5);
        assert!(m.shading_toony > 0.8);
    }

    #[test]
    fn mixamo_walker_compiles_as_toon() {
        const DUMP: &str = include_str!("../tests/fixtures/mixamo_walker_world.json");
        let doc = WorldDoc::from_json(DUMP).unwrap();
        assert_eq!(
            doc.player.as_ref().unwrap().gltf.as_deref(),
            Some("tpose_humanoid.vrm")
        );
        let scene = doc.compile_scene(1.0);
        let batch = scene
            .batches
            .iter()
            .find(|b| b.mesh.0 >= MESH_GLTF_BASE)
            .expect("mixamo walker batch");
        assert_eq!(batch.instances[0].material, Material::Toon);
        assert!(
            !scene.batches.iter().any(|b| b.mesh == MESH_CAPSULE),
            "mixamo walker is a VRM mesh, not the capsule"
        );
    }

    #[test]
    fn emma_walker_dump_is_repo_relative_and_compiles() {
        const DUMP: &str = include_str!("../tests/fixtures/emma_walker_world.json");
        let mut doc = WorldDoc::from_json(DUMP).unwrap();
        let spec = doc
            .player
            .as_ref()
            .unwrap()
            .gltf
            .clone()
            .expect("emma gltf");
        assert_eq!(spec, "assets/Emma.vrm");
        assert!(!spec.contains('\\') && !spec.contains(':'));
        assert_eq!(doc.player.as_ref().unwrap().model, "capsule");
        assert!(doc.heightfield.is_none());
        assert!(doc.props.is_empty());
        assert!(doc.water_y.is_none());
        doc.refresh_asset_status();
        let json = doc.to_json().unwrap();
        if existing_asset_path(&spec).is_none() {
            assert!(
                json.contains("load_error") && json.contains("Emma.vrm"),
                "missing Emma.vrm must be dump-visible, got {json}"
            );
        } else if let Some(err) = &doc.player.as_ref().unwrap().load_error {
            assert!(err.contains("Emma") || err.contains(".vrm"), "{err}");
        }
        let scene = doc.compile_scene(1.0);
        let batch = scene
            .batches
            .iter()
            .find(|b| b.mesh.0 >= MESH_GLTF_BASE)
            .expect("emma walker batch (Emma.vrm or tpose_humanoid fallback)");
        assert_eq!(batch.instances[0].material, Material::Toon);
        assert!(
            !scene.batches.iter().any(|b| b.mesh == MESH_CAPSULE),
            "emma dump must not surprise-swap to the capsule"
        );
        let ground = scene.batches.iter().any(|b| {
            (b.mesh == MESH_PLANE || b.mesh == MESH_HEIGHTFIELD)
                && b.instances
                    .iter()
                    .any(|i| i.material == Material::Grass && i.color[3] == 255)
        });
        assert!(ground, "emma dump must compile a floor, not a blue void");
        let skinned = doc
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("skinned")
            .1;
        let cyl = primitives::cylinder_mesh(0.5, 1.0, 12);
        assert_ne!(
            skinned.vertices.len(),
            cyl.vertices.len(),
            "fallback must be a skinned mesh, not a cylinder"
        );
        assert!(
            skinned.vertices.len() >= 8,
            "skinned walker verts {}",
            skinned.vertices.len()
        );
        if existing_asset_path(&spec).is_some() {
            let n_parts = doc
                .compile_meshes()
                .iter()
                .filter(|(id, _)| id.0 >= MESH_GLTF_BASE)
                .count();
            assert!(
                n_parts >= 3,
                "Emma.vrm on disk must draw Body/Face/Hair prims, got {n_parts}"
            );
            assert!(
                doc.compile_meshes()
                    .iter()
                    .any(|(id, m)| id.0 >= MESH_GLTF_BASE && m.albedo.is_some()),
                "Emma.vrm baseColor must bind"
            );
        }
    }

    #[test]
    fn walker_gltf_load_fail_is_skinned_not_capsule() {
        const DUMP: &str = include_str!("../tests/fixtures/emma_walker_world.json");
        let mut doc = WorldDoc::from_json(DUMP).unwrap();
        let bad = "assets/does_not_exist_walker.vrm";
        if let Some(w) = doc.player.as_mut() {
            w.gltf = Some(bad.into());
            w.model = "capsule".into();
        }
        for w in &mut doc.walkers {
            w.gltf = Some(bad.into());
            w.model = "capsule".into();
        }
        doc.refresh_asset_status();
        let json = doc.to_json().unwrap();
        assert!(
            json.contains("load_error") && json.contains("does_not_exist_walker"),
            "load fail must be dump-visible, got {json}"
        );
        assert_eq!(doc.player.as_ref().unwrap().model, "capsule");
        let scene = doc.compile_scene(1.0);
        assert!(
            scene.batches.iter().any(|b| b.mesh.0 >= MESH_GLTF_BASE),
            "named gltf must keep a skinned slot"
        );
        assert!(
            !scene.batches.iter().any(|b| b.mesh == MESH_CAPSULE),
            "model:capsule must not win when gltf is set"
        );
        let skinned = doc
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= MESH_GLTF_BASE)
            .expect("tpose fallback")
            .1;
        assert_eq!(
            skinned.vertices.len(),
            8,
            "tpose_humanoid fallback, not a cylinder"
        );
    }

    #[test]
    fn n_trees_share_one_instance_batch() {
        let props: Vec<WorldProp> = (0..12)
            .map(|i| WorldProp {
                id: format!("prop:tree-{i}"),
                kind: "prop".into(),
                name: "tree".into(),
                model: "tree".into(),
                position: [i as f32 * 1.1 - 6.0, 1.0, 0.0],
                scale: [1.0, 2.0, 1.0],
                enabled: true,
                ..Default::default()
            })
            .collect();
        let doc = WorldDoc {
            version: WORLD_DUMP_VERSION,
            half: 40.0,
            cameras: vec![WorldCamera {
                id: "camera:main".into(),
                kind: "camera".into(),
                name: "main".into(),
                position: [0.0, 4.0, 8.0],
                target: [0.0, 1.0, 0.0],
                fov: 54.0,
                roll: 0.0,
            }],
            heightfield: Some(WorldHeightfield {
                fn_name: Some("open_world_height".into()),
                lod_radius: Some(28.0),
                lod_cells: Some(6),
                ..Default::default()
            }),
            props,
            ..Default::default()
        };
        assert_eq!(doc.dumped_vegetation_count(), 12);
        let scene = doc.compile_scene(1.0);
        let tree_batch = scene
            .batches
            .iter()
            .find(|b| b.mesh == MESH_BOX && b.instances.len() >= 12)
            .expect("12 dumped trees must be one MESH_BOX batch");
        assert_eq!(tree_batch.instances.len(), 12, "N trees = 1 GPU batch");
        let stats = scene.render_stats();
        assert_eq!(stats.batches, scene.batches.len() as u32);
        assert!(stats.instances >= 12);
        assert!(
            scene.batches.iter().filter(|b| b.mesh == MESH_BOX).count() <= 2,
            "repeated trees must not be unique draws"
        );
    }

    #[test]
    fn far_trees_use_billboard_not_thinned() {
        let mut props = Vec::new();
        for i in 0..8 {
            props.push(WorldProp {
                id: format!("prop:tree-near-{i}"),
                kind: "prop".into(),
                name: "tree".into(),
                model: "tree".into(),
                position: [i as f32 * 0.8 - 3.0, 1.0, 0.0],
                scale: [1.0, 2.0, 1.0],
                enabled: true,
                ..Default::default()
            });
        }
        for i in 0..8 {
            props.push(WorldProp {
                id: format!("prop:tree-far-{i}"),
                kind: "prop".into(),
                name: "pine".into(),
                model: "tree".into(),
                position: [i as f32 * 0.8 - 3.0, 1.0, 40.0],
                scale: [1.0, 2.4, 1.0],
                enabled: true,
                ..Default::default()
            });
        }
        let doc = WorldDoc {
            version: WORLD_DUMP_VERSION,
            half: 40.0,
            cameras: vec![WorldCamera {
                id: "camera:main".into(),
                kind: "camera".into(),
                name: "main".into(),
                position: [0.0, 4.0, 0.0],
                target: [0.0, 1.0, 0.0],
                fov: 54.0,
                roll: 0.0,
            }],
            heightfield: Some(WorldHeightfield {
                lod_radius: Some(10.0),
                lod_cells: Some(6),
                ..Default::default()
            }),
            props,
            ..Default::default()
        };
        let scene = doc.compile_scene(1.0);
        let full = scene
            .batches
            .iter()
            .filter(|b| b.mesh == MESH_BOX)
            .map(|b| b.instances.len())
            .sum::<usize>();
        let cheap = scene
            .batches
            .iter()
            .filter(|b| b.mesh == MESH_QUAD)
            .map(|b| b.instances.len())
            .sum::<usize>();
        assert_eq!(full, 8, "near trees stay full mesh, got {full}");
        assert_eq!(cheap, 8, "far trees become one billboard each, got {cheap}");
        assert_eq!(full + cheap, 16, "do not thin vegetation");
        let quad_batches = scene.batches.iter().filter(|b| b.mesh == MESH_QUAD).count();
        assert_eq!(quad_batches, 1, "far trees share one instanced draw");
    }

    #[test]
    fn crest_isle_vegetation_is_dense_and_instanced() {
        let doc = WorldDoc::from_json(CREST_ISLE_DUMP).unwrap();
        let hf = doc.heightfield.as_ref().expect("heightfield");
        assert_eq!(hf.lod_radius, Some(28.0));
        assert_eq!(hf.lod_cells, Some(6));
        let scene = doc.compile_scene(16.0 / 9.0);
        let stats = scene.render_stats();
        assert!(
            stats.instances >= 80,
            "Crest grove must stay dense, instances={}",
            stats.instances
        );
        let cone = scene
            .batches
            .iter()
            .find(|b| b.mesh == MESH_CONE)
            .expect("near trees use cone foliage");
        assert!(
            cone.instances.len() >= 8,
            "cone foliage must instance, got {}",
            cone.instances.len()
        );
        let cone_draws = scene.batches.iter().filter(|b| b.mesh == MESH_CONE).count();
        assert_eq!(cone_draws, 1, "same-mesh foliage is one draw");
        let meshes = doc.compile_meshes();
        assert!(
            meshes
                .iter()
                .any(|(id, m)| *id == MESH_CONE && !m.vertices.is_empty()),
            "lod_cells cone is in compile_meshes"
        );
        let crate_p = doc.props.iter().find(|p| p.name == "crate").unwrap();
        assert!(!is_vegetation_prop(crate_p));
    }

    #[test]
    fn prop_texture_compiles_to_albedo_mesh() {
        let mut doc = WorldDoc {
            version: WORLD_DUMP_VERSION,
            ..Default::default()
        };
        doc.props.push(WorldProp {
            id: "prop:sign".into(),
            kind: "prop".into(),
            name: "sign".into(),
            model: "quad".into(),
            texture: Some("kagra-shared/tests/fixtures/tex_check.png".into()),
            enabled: true,
            scale: [1.0, 1.0, 1.0],
            ..Default::default()
        });
        let meshes = doc.compile_meshes();
        assert!(
            meshes
                .iter()
                .any(|(_, m)| m.albedo.as_ref().is_some_and(|a| a.width == 2)),
            "textured prop must compile with PNG albedo"
        );
        let scene = doc.compile_scene(1.0);
        let ids = doc.texture_mesh_ids();
        let mid = *ids.get("prop:sign").expect("texture slot");
        assert!(
            scene.batches.iter().any(|b| b.mesh == mid),
            "compile_scene must draw the textured mesh"
        );
    }

    #[test]
    fn indoor_sky_and_no_auto_ground() {
        let outdoor = WorldDoc {
            version: WORLD_DUMP_VERSION,
            ..Default::default()
        };
        let out_scene = outdoor.compile_scene(1.0);
        assert_eq!(out_scene.clear, [130, 165, 205, 255]);
        assert!(
            out_scene.batches.iter().any(|b| b.mesh == MESH_PLANE),
            "legacy dump still gets the grass plane"
        );

        let indoor = WorldDoc {
            version: WORLD_DUMP_VERSION,
            sky: Some([18, 14, 16, 255]),
            ground: Some(false),
            ..Default::default()
        };
        let in_scene = indoor.compile_scene(1.0);
        assert_eq!(in_scene.clear, [18, 14, 16, 255]);
        assert_eq!(in_scene.fog_color, [18, 14, 16, 255]);
        assert!(
            !in_scene.batches.iter().any(|b| b.mesh == MESH_PLANE),
            "ground:false must not emit the default grass"
        );
    }

    #[test]
    fn uv_scale_tiles_albedo() {
        let mut doc = WorldDoc {
            version: WORLD_DUMP_VERSION,
            ..Default::default()
        };
        doc.props.push(WorldProp {
            id: "prop:floor".into(),
            kind: "prop".into(),
            model: "box".into(),
            texture: Some("kagra-shared/tests/fixtures/tex_check.png".into()),
            uv_scale: Some([4.0, 2.0]),
            enabled: true,
            scale: [1.0, 1.0, 1.0],
            ..Default::default()
        });
        let meshes = doc.compile_meshes();
        assert!(
            meshes.iter().any(|(_, m)| m
                .albedo
                .as_ref()
                .is_some_and(|a| a.width == 8 && a.height == 4)),
            "2x2 PNG tiled 4x2 must become 8x4"
        );
    }
}
