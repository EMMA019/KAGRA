//! Persistent world document (`docs/schemas/world.json` version 1).
//!
//! `Scene3D` is a **one-frame draw list** (camera, batches, fog). Collectathon
//! and driving already build that. Dump JSON lives here as `WorldDoc`, then
//! `compile_scene` turns it into a `Scene3D` for one frame (box / sphere /
//! capsule primitives). Integer GPU mesh ids are not game objects.
//! Offscreen draw (feature = "render") is `render_world_doc`: upload
//! `compile_meshes`, draw the batches, read RGBA. Not a kagra-core window.

use crate::scene3d::{primitives, Camera, Material, MeshId, Scene3D, SceneBuilder};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// `docs/schemas/world.json` の version。他は拒否する。
pub const WORLD_DUMP_VERSION: u32 = 1;

const MESH_BOX: MeshId = MeshId(0);
const MESH_SPHERE: MeshId = MeshId(1);
const MESH_CAPSULE: MeshId = MeshId(2);
const MESH_PLANE: MeshId = MeshId(3);

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

    /// One-frame draw list. Capsules / box / sphere / plane primitives.
    /// Does not mutate this document. GPU upload is a later slice.
    pub fn compile_scene(&self, aspect: f32) -> Scene3D {
        let camera = self.draw_camera();
        let mut b = SceneBuilder::new(&camera, aspect.max(1e-3));
        // Do not register bounds: a dump camera can be tight, and compile must
        // still emit the document's objects (unregistered meshes are never culled).

        if self.heightfield.is_some() {
            let span = (self.half * 2.0).max(4.0);
            let y = self.floor_y;
            b.push_material(
                MESH_PLANE,
                Mat4::from_scale_rotation_translation(
                    Vec3::new(span, 1.0, span),
                    Quat::IDENTITY,
                    Vec3::new(0.0, y, 0.0),
                ),
                [78, 138, 64, 255],
                Material::Grass,
            );
        }

        for prop in &self.props {
            if !prop.enabled {
                continue;
            }
            let mesh = mesh_for_prop(prop);
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
}

fn mesh_for_prop(prop: &WorldProp) -> MeshId {
    if prop.gltf.is_some() {
        return MESH_BOX;
    }
    match prop.model.to_ascii_lowercase().as_str() {
        "sphere" => MESH_SPHERE,
        "cylinder" | "capsule" => MESH_CAPSULE,
        "plane" => MESH_PLANE,
        _ => MESH_BOX,
    }
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
        let ids: std::collections::HashSet<_> = meshes.iter().map(|(id, _)| id.0).collect();
        for json in [CREST_ISLE_DUMP, ORB_RUSH_DUMP] {
            let scene = WorldDoc::from_json(json).unwrap().compile_scene(1.0);
            for batch in &scene.batches {
                assert!(
                    ids.contains(&batch.mesh.0),
                    "compiled batch mesh {} is not in compile_meshes()",
                    batch.mesh.0
                );
            }
        }
    }
}
