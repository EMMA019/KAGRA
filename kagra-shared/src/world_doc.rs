//! Persistent world document (`docs/schemas/world.json` version 1).
//!
//! `Scene3D` is a **one-frame draw list** (camera, batches, fog). Collectathon
//! and driving already build that. Dump JSON lives here as `WorldDoc`, then
//! `compile_scene` turns it into a `Scene3D` for one frame. Heightfield
//! batches come from named demo fns (`open_world_height` / `island_height` /
//! `overworld_height`) or dump samples. glTF props use `gltf_load` (capsule
//! player; VRM skin is not ported). Integer GPU mesh ids are not game
//! objects. Live play is `WorldPlay` (WASD → `WalkInput` → sit on
//! heightfield). Offscreen draw (feature = "render") is `render_world_doc`.
//! A real desktop window is `Renderer::new_for_window` + example `window`.
//! Not kagra-core `RendererV2` / `window.rs`.
//!
//! Python `Walk.wish` / `CharacterController` (accel 14 / decel 22 / 8-point
//! foot ring / step-up) is the leftover VRM motor. This crate does **not**
//! copy that solver and does not add Rapier. Shared tick matches collectathon
//! `WalkInput`: camera-relative wish, sit on `height_at`, optional jump.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::collectathon::open_world_height;
use crate::gltf_load::{mesh_from_embedded_gltf, mesh_from_gltf_json, unit_cube_gltf};
use crate::scene3d::{
    primitives, Camera, Material, MeshData, MeshId, Scene3D, SceneBuilder, Vertex3,
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
pub(crate) const MESH_GLTF_BASE: u32 = 5;

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

    /// One-frame draw list. Heightfield + glTF/box/sphere/capsule primitives.
    /// Does not mutate this document. GPU upload uses `compile_meshes`.
    pub fn compile_scene(&self, aspect: f32) -> Scene3D {
        let camera = self.draw_camera();
        let mut b = SceneBuilder::new(&camera, aspect.max(1e-3));
        // Do not register bounds: a dump camera can be tight, and compile must
        // still emit the document's objects (unregistered meshes are never culled).
        let gltf_ids = self.gltf_mesh_ids();

        if self.heightfield.is_some() {
            b.push_material(
                MESH_HEIGHTFIELD,
                Mat4::IDENTITY,
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
                Material::Solid,
            );
        }

        for prop in &self.props {
            if !prop.enabled {
                continue;
            }
            let mesh = mesh_for_prop(prop, &gltf_ids);
            let pos = Vec3::from_array(prop.position);
            let scale = Vec3::from_array(prop.scale);
            let model =
                Mat4::from_scale_rotation_translation(scale, Quat::from_rotation_y(prop.yaw), pos);
            b.push(mesh, model, color_u8(prop.color));
        }

        let mut seen = std::collections::HashSet::new();
        for walk in self.walkers.iter().chain(self.player.iter()) {
            if !seen.insert(walk.id.as_str()) {
                continue;
            }
            let pos = Vec3::from_array(walk.position);
            let model = Mat4::from_scale_rotation_translation(
                Vec3::new(0.56, 0.95, 0.56),
                Quat::from_rotation_y(walk.yaw),
                pos,
            );
            b.push(MESH_CAPSULE, model, [64, 180, 176, 255]);
        }

        let (light_dir, ambient) = self.draw_light();
        let sky = [130, 165, 205, 255];
        Scene3D {
            camera,
            clear: sky,
            light_dir,
            ambient,
            fog_color: sky,
            fog_start: 48.0,
            fog_end: 220.0,
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

    /// Primitive slots 0..3 plus heightfield (4) plus one mesh per unique glTF.
    pub fn compile_meshes(&self) -> Vec<(MeshId, MeshData)> {
        let mut out = compile_meshes();
        out.push((MESH_HEIGHTFIELD, self.heightfield_mesh()));
        for (i, spec) in self.gltf_specs().into_iter().enumerate() {
            let mesh = gltf_mesh_for(&spec).unwrap_or_else(|| primitives::box_mesh(Vec3::ONE));
            out.push((MeshId(MESH_GLTF_BASE + i as u32), mesh));
        }
        out
    }

    fn heightfield_mesh(&self) -> MeshData {
        if self.heightfield.is_none() {
            return primitives::plane_mesh(1.0, 1.0);
        }
        let half = self.half.max(4.0);
        let cells = 32u32;
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

    fn gltf_specs(&self) -> Vec<String> {
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
                out.push(spec.to_string());
            }
        }
        out
    }

    fn gltf_mesh_ids(&self) -> HashMap<String, MeshId> {
        self.gltf_specs()
            .into_iter()
            .enumerate()
            .map(|(i, spec)| (spec, MeshId(MESH_GLTF_BASE + i as u32)))
            .collect()
    }
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
        "sphere" => MESH_SPHERE,
        "cylinder" | "capsule" => MESH_CAPSULE,
        "plane" => MESH_PLANE,
        _ => MESH_BOX,
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
    if matches!(
        stem,
        "cube.glb" | "cube.gltf" | "crate.glb" | "crate.gltf" | "cube"
    ) {
        return mesh_from_embedded_gltf(&unit_cube_gltf()).ok();
    }
    let path = Path::new(spec);
    if path.is_file() && lower.ends_with(".gltf") {
        let json = std::fs::read_to_string(path).ok()?;
        let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        return mesh_from_gltf_json(&json, |uri| {
            std::fs::read(base.join(uri)).map_err(|e| e.to_string())
        })
        .ok();
    }
    None
}

fn color_u8(rgb: Option<[u32; 3]>) -> [u8; 4] {
    match rgb {
        Some([r, g, b]) => [r.min(255) as u8, g.min(255) as u8, b.min(255) as u8, 255],
        None => [230, 230, 235, 255],
    }
}

/// Primitive meshes that `compile_scene` refers to by `MeshId`.
pub fn compile_meshes() -> Vec<(MeshId, crate::scene3d::MeshData)> {
    vec![
        (MESH_BOX, primitives::box_mesh(Vec3::ONE)),
        (MESH_SPHERE, primitives::box_mesh(Vec3::ONE)), // stand-in; sphere mesh is later
        (MESH_CAPSULE, primitives::cylinder_mesh(0.5, 1.0, 12)),
        (MESH_PLANE, primitives::plane_mesh(1.0, 1.0)),
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
        assert_eq!(meshes.len(), 4);
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
}
