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

use crate::collectathon::open_world_height;
use crate::gltf_load::{
    is_tpose_humanoid_spec, is_walk_skinned_spec, is_walk_vrm_spec, mesh_from_embedded_gltf,
    mesh_from_glb, mesh_from_gltf_json, sample_skinned_look, skinned_from_embedded_gltf,
    skinned_from_glb, skinned_from_gltf_json, skinned_tpose_humanoid, unit_cube_gltf,
    walk_skinned_gltf, walk_skinned_vrm,
};
use crate::scene3d::{
    primitives, Camera, LocalLight, Material, MeshData, MeshId, Scene3D, SceneBuilder, Vertex3,
};
use glam::{Mat4, Quat, Vec3};
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
pub(crate) const MESH_GLTF_BASE: u32 = 7;

/// Persistent world. JSON source of truth. Not a draw list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldDoc {
    pub version: u32,
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
}

fn default_gravity() -> f32 {
    9.8
}

/// Prop record. Parent is a string id (one level; n-level TRS is later).
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
    #[serde(default)]
    pub color: Option<[u32; 3]>,
    /// 0..1. Coins dump 1.0. Default 0 (Lambert).
    #[serde(default)]
    pub metallic: f32,
    /// 0..1. Coins dump 0.12. Default 1 (dielectric).
    #[serde(default = "default_roughness")]
    pub roughness: f32,
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
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
}

fn camera_type() -> String {
    "camera".into()
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
    pub fn to_json(&self) -> Result<String, String> {
        let mut out = self.clone();
        if out.version == 0 {
            out.version = WORLD_DUMP_VERSION;
        }
        serde_json::to_string_pretty(&out).map_err(|e| e.to_string())
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

    /// One-frame draw list. Heightfield + glTF/box/sphere/capsule + sprite/quad.
    /// Empty lights get default key+fill (max 4 slots). Capsules/props get a
    /// ground contact blob. Does not mutate this document. GPU upload uses `compile_meshes`.
    pub fn compile_scene(&self, aspect: f32) -> Scene3D {
        let camera = self.draw_camera();
        let mut b = SceneBuilder::new(&camera, aspect.max(1e-3));
        // Do not register bounds: a dump camera can be tight, and compile must
        // still emit the document's objects (unregistered meshes are never culled).
        let (gltf_ids, walker_gltf_ids) = self.gltf_mesh_ids();

        if self.heightfield.is_some() {
            b.push_material(
                MESH_HEIGHTFIELD,
                Mat4::IDENTITY,
                [78, 138, 64, 255],
                Material::Grass,
            );
        } else {
            // Dump without heightfield/water/props is otherwise a clear-color void.
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
        for prop in &self.props {
            if !prop.enabled {
                continue;
            }
            let pos = Vec3::from_array(prop.position);
            let scale = Vec3::from_array(prop.scale);
            let mat = material_for_prop(prop);
            let veg = is_vegetation_prop(prop);
            let cheap = veg && camera.eye.distance(pos) > lod_r;
            if cheap {
                let h = scale.y.abs().max(scale.x.abs()).max(0.4);
                let model = Mat4::from_scale_rotation_translation(
                    Vec3::new(h * 0.7, h, 1.0),
                    Quat::from_rotation_y(billboard_yaw(camera.eye, pos)),
                    pos,
                );
                b.push_material(MESH_QUAD, model, color_u8(prop.color), mat);
                continue;
            }
            let mesh = mesh_for_prop(prop, &gltf_ids);
            let model =
                Mat4::from_scale_rotation_translation(scale, Quat::from_rotation_y(prop.yaw), pos);
            b.push_material(mesh, model, color_u8(prop.color), mat);
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
            // First-person: camera sits in the capsule. Skip local body/head so
            // we do not clip into a white interior. Walker stays in the dump.
            if hide_local && local_id == Some(walk.id.as_str()) {
                continue;
            }
            // Named glTF (dump `gltf` / `model`) is CPU-skinned at `clip` and
            // drawn at walker pose. `model: capsule` must not win when `gltf`
            // is set. Capsule + head remains the fallback only if no glTF spec.
            // Action genre: name "hurt" / "dead" is dump-visible and tinted here.
            let pos = Vec3::from_array(walk.position);
            let yaw = Quat::from_rotation_y(walk.yaw);
            let dead = walk.name == "dead";
            let hurt = walk.name == "hurt";
            let body_col = if dead {
                [70, 74, 82, 255]
            } else if hurt {
                [220, 64, 64, 255]
            } else {
                [62, 176, 184, 255]
            };
            if let Some(&mesh) = walker_gltf_ids.get(&walk.id) {
                let model = Mat4::from_scale_rotation_translation(Vec3::ONE, yaw, pos);
                let col = if walker_gltf_spec(walk)
                    .map(walker_spec_has_albedo)
                    .unwrap_or(false)
                {
                    if dead {
                        [70, 74, 82, 255]
                    } else if hurt {
                        [220, 64, 64, 255]
                    } else {
                        [255, 255, 255, 255]
                    }
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
                b.push_material(mesh, model, col, mat);
                continue;
            }
            let body_h = if dead { 0.28 } else { 0.95 };
            let body =
                Mat4::from_scale_rotation_translation(Vec3::new(0.56, body_h, 0.56), yaw, pos);
            b.push(MESH_CAPSULE, body, body_col);
            let head = Mat4::from_scale_rotation_translation(
                Vec3::new(0.38, 0.32, 0.38),
                yaw,
                pos + Vec3::Y * (if dead { 0.18 } else { 0.62 }),
            );
            let head_col = if dead {
                [90, 86, 82, 255]
            } else {
                [236, 214, 176, 255]
            };
            b.push(MESH_BOX, head, head_col);
        }

        self.push_contact_blobs(&mut b, hide_local, local_id);

        let (light_dir, ambient, local_lights) = self.draw_lights();
        let sky = [130, 165, 205, 255];
        Scene3D {
            camera,
            clear: sky,
            light_dir,
            ambient,
            local_lights,
            fog_color: sky,
            fog_start: 48.0,
            fog_end: 220.0,
            ibl: self.ibl.unwrap_or(0.35),
            exposure: self.exposure.unwrap_or(1.0),
            tonemap: self.tonemap.unwrap_or(true),
            batches: b.finish(),
        }
    }

    fn draw_camera(&self) -> Camera {
        if let Some(cam) = self.cameras.first() {
            return Camera {
                eye: Vec3::from_array(cam.position),
                target: Vec3::from_array(cam.target),
                up: Vec3::Y,
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
            let pos = Vec3::from_array(prop.position);
            let sx = prop.scale[0].abs().max(0.2);
            let sz = prop.scale[2].abs().max(0.2);
            let radius = (sx.max(sz) * 0.7).clamp(0.28, 2.4);
            self.push_contact_blob(b, pos.x, pos.z, radius);
        }
    }

    fn push_contact_blob(&self, b: &mut SceneBuilder, x: f32, z: f32, radius: f32) {
        let y = self.height_at(x, z) + 0.03;
        let model = Mat4::from_scale_rotation_translation(
            Vec3::new(radius * 2.0, 1.0, radius * 2.0),
            Quat::IDENTITY,
            Vec3::new(x, y, z),
        );
        b.push_material(MESH_PLANE, model, [18, 14, 12, 120], Material::Solid);
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
        for (i, slot) in self.gltf_slots().into_iter().enumerate() {
            let mesh = match &slot {
                GltfSlot::Rest(spec) => gltf_mesh_for(spec),
                GltfSlot::Skinned {
                    spec,
                    clip,
                    hair,
                    morph,
                    look_yaw,
                    look_pitch,
                    ..
                } => gltf_skinned_mesh_for(spec, *clip, *hair, *morph, *look_yaw, *look_pitch)
                    .or_else(|| walker_tpose_mesh(*clip, *hair, *morph, *look_yaw, *look_pitch)),
            }
            .unwrap_or_else(|| primitives::box_mesh(Vec3::ONE));
            out.push((MeshId(MESH_GLTF_BASE + i as u32), mesh));
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

    fn gltf_slots(&self) -> Vec<GltfSlot> {
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
                out.push(GltfSlot::Skinned {
                    walker_id: walk.id.clone(),
                    spec: spec.to_string(),
                    clip: walk.clip,
                    hair: walk.hair,
                    morph: walk.morph,
                    look_yaw: walk.look_yaw,
                    look_pitch: walk.look_pitch,
                });
            }
        }
        out
    }

    fn gltf_mesh_ids(&self) -> (HashMap<String, MeshId>, HashMap<String, MeshId>) {
        let mut props = HashMap::new();
        let mut walkers = HashMap::new();
        for (i, slot) in self.gltf_slots().into_iter().enumerate() {
            let id = MeshId(MESH_GLTF_BASE + i as u32);
            match slot {
                GltfSlot::Rest(spec) => {
                    props.insert(spec, id);
                }
                GltfSlot::Skinned { walker_id, .. } => {
                    walkers.insert(walker_id, id);
                }
            }
        }
        (props, walkers)
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

enum GltfSlot {
    Rest(String),
    Skinned {
        walker_id: String,
        spec: String,
        clip: f32,
        hair: f32,
        morph: f32,
        look_yaw: f32,
        look_pitch: f32,
    },
}

fn walker_spec_has_mtoon(spec: &str) -> bool {
    if let Some(skin) = load_skinned(spec) {
        return skin.rest.mtoon.is_some();
    }
    gltf_mesh_for(spec)
        .map(|m| m.mtoon.is_some())
        .unwrap_or(false)
}

fn walker_spec_has_albedo(spec: &str) -> bool {
    if let Some(skin) = load_skinned(spec) {
        return skin.rest.albedo.is_some();
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
    if lower.ends_with(".gltf") || lower.ends_with(".glb") || lower.ends_with(".vrm") {
        return Some(m);
    }
    None
}

fn mesh_for_prop(prop: &WorldProp, gltf_ids: &HashMap<String, MeshId>) -> MeshId {
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
    crate::assets::resolve_asset(spec)
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
        &skin, t, hair, morph, look_yaw, look_pitch,
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
    let lower = spec.to_ascii_lowercase();
    let stem = Path::new(&lower)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(lower.as_str());
    if matches!(
        stem,
        "cube.glb" | "cube.gltf" | "crate.glb" | "crate.gltf" | "cube"
    ) {
        return Ok(());
    }
    let path = existing_asset_path(spec)
        .ok_or_else(|| format!("{spec}: missing (repo root / KAGRA_ROOT / window.exe dir)"))?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or(spec);
    if lower.ends_with(".gltf") {
        let json = std::fs::read_to_string(&path).map_err(|e| format!("{name}: {e}"))?;
        skinned_from_gltf_json(&json, |uri| {
            let base = path.parent().unwrap_or(Path::new("."));
            std::fs::read(base.join(uri)).map_err(|e| e.to_string())
        })
        .map(|_| ())
        .or_else(|e| {
            mesh_from_gltf_json(&json, |uri| {
                let base = path.parent().unwrap_or(Path::new("."));
                std::fs::read(base.join(uri)).map_err(|err| err.to_string())
            })
            .map(|_| ())
            .map_err(|_| format!("{name}: {e}"))
        })
    } else if lower.ends_with(".glb") || lower.ends_with(".vrm") {
        let bytes = std::fs::read(&path).map_err(|e| format!("{name}: {e}"))?;
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

fn gltf_mesh_for(spec: &str) -> Option<MeshData> {
    let spec = spec.trim();
    if spec.starts_with('{') {
        return mesh_from_embedded_gltf(spec).ok();
    }
    let lower = spec.to_ascii_lowercase();
    let stem = Path::new(&lower)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(lower.as_str());
    if is_walk_skinned_spec(spec) {
        return mesh_from_embedded_gltf(&walk_skinned_gltf()).ok();
    }
    if is_walk_vrm_spec(spec) {
        return mesh_from_glb(&walk_skinned_vrm()).ok();
    }
    if matches!(
        stem,
        "cube.glb" | "cube.gltf" | "crate.glb" | "crate.gltf" | "cube"
    ) {
        return mesh_from_embedded_gltf(&unit_cube_gltf()).ok();
    }
    if let Some(path) = existing_asset_path(spec) {
        if lower.ends_with(".gltf") {
            let json = std::fs::read_to_string(&path).ok()?;
            let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            return mesh_from_gltf_json(&json, |uri| {
                std::fs::read(base.join(uri)).map_err(|e| e.to_string())
            })
            .ok();
        }
        if lower.ends_with(".glb") || lower.ends_with(".vrm") {
            let bytes = std::fs::read(&path).ok()?;
            return mesh_from_glb(&bytes).ok();
        }
    }
    None
}

fn gltf_skinned_mesh_for(
    spec: &str,
    clip: f32,
    hair: f32,
    morph: f32,
    look_yaw: f32,
    look_pitch: f32,
) -> Option<MeshData> {
    if let Some(skin) = load_skinned(spec) {
        if clip <= 0.0 {
            return Some(sample_skinned_look(
                &skin, None, hair, morph, look_yaw, look_pitch,
            ));
        }
        return Some(sample_skinned_look(
            &skin,
            Some(clip),
            hair,
            morph,
            look_yaw,
            look_pitch,
        ));
    }
    gltf_mesh_for(spec)
}

pub(crate) fn load_skinned(spec: &str) -> Option<crate::gltf_load::SkinnedMesh> {
    let spec = spec.trim();
    if spec.starts_with('{') {
        return skinned_from_embedded_gltf(spec).ok();
    }
    if is_walk_skinned_spec(spec) {
        return skinned_from_embedded_gltf(&walk_skinned_gltf()).ok();
    }
    if is_walk_vrm_spec(spec) {
        return skinned_from_glb(&walk_skinned_vrm()).ok();
    }
    if is_tpose_humanoid_spec(spec) {
        return skinned_tpose_humanoid().ok();
    }
    let lower = spec.to_ascii_lowercase();
    let stem = Path::new(&lower)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(lower.as_str());
    if matches!(
        stem,
        "cube.glb" | "cube.gltf" | "crate.glb" | "crate.gltf" | "cube"
    ) {
        return skinned_from_embedded_gltf(&unit_cube_gltf()).ok();
    }
    if let Some(path) = existing_asset_path(spec) {
        if lower.ends_with(".gltf") {
            if let Ok(json) = std::fs::read_to_string(&path) {
                let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
                if let Ok(skin) = skinned_from_gltf_json(&json, |uri| {
                    std::fs::read(base.join(uri)).map_err(|e| e.to_string())
                }) {
                    return Some(skin);
                }
            }
            // File present but unreadable: do not silent-capsule. Fall through.
        }
        if lower.ends_with(".glb") || lower.ends_with(".vrm") {
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(skin) = skinned_from_glb(&bytes) {
                    return Some(skin);
                }
            }
            // File present but unreadable: do not silent-capsule. Fall through.
        }
    }
    if is_emma_vrm_stem(stem)
        || lower.ends_with(".vrm")
        || lower.ends_with(".gltf")
        || lower.ends_with(".glb")
    {
        // Official dump points at assets/Emma.vrm (gitignored, large).
        // CI / missing / parse fail: tiny clip-less tpose_humanoid (Mixamo walk).
        return skinned_tpose_humanoid().ok();
    }
    None
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
        (MESH_SPHERE, primitives::cylinder_mesh(0.5, 1.0, 12)),
        (MESH_CAPSULE, primitives::cylinder_mesh(0.5, 1.0, 12)),
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
        if let Some(w) = walk.walkers.first_mut() {
            w.clip = 0.25;
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
}
