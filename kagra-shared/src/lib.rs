//! KAGRA shared library — desktop / Wasm / Android / iOS 共通コア。
//!
//! - 入力: PointerEvent / VirtualPad / DriveInput（`docs/schemas/input_events.json` と同期）
//! - セッション: 画面サイズ・ポーズ・フレームカウンタ
//! - シーン: 描画内容の記述（2D も 3D も GPU 非依存なのでテストできる）
//! - 車両: 自転車モデルの運動と追従カメラ（純関数）
//! - 描画: wgpu 3D + 2D HUD（feature = "render"）
//! - FFI: C ABI（`ffi`）と任意の wasm-bindgen（feature = "wasm"）

pub mod action;
pub mod action2d;
pub mod assets;
pub mod audio;
pub mod collectathon;
pub mod collide;
pub mod cook;
pub mod driving;
pub mod ffi;
pub mod fight;
pub mod fish;
pub mod fps;
pub mod game;
pub mod gltf_load;
pub mod input;
pub mod lookat;
pub mod map;
pub mod mission;
pub mod mixamo;
pub mod morph;
pub mod novel;
pub mod platformer;
pub mod puzzle;
pub mod race;
pub mod rhythm;
pub mod road;
pub mod rpg;
pub mod save;
pub mod scene;
pub mod scene3d;
pub mod session;
pub mod shop;
pub mod sim;
pub mod sports;
pub mod spring;
pub mod sprite;
pub mod stealth;
pub mod survival;
pub mod td;
pub mod traffic;
pub mod ui;
pub mod vehicle;
pub mod world;
pub mod world_doc;
pub mod world_play;

#[cfg(feature = "render")]
pub mod render;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use assets::{resolve_alias, AssetKind};
pub use audio::AudioLevels;
pub use collectathon::{
    CollectathonScene, IsleGame, WalkInput, GAME_ID as ISLE_GAME_ID, GAME_TITLE as ISLE_GAME_TITLE,
};
pub use collide::Obb2;
pub use driving::{DrivingScene, MeshIds, MeshSet};
pub use game::{DemoGame, GamePhase, GAME_ID, GAME_TITLE};
pub use gltf_load::{
    mesh_from_embedded_gltf, mesh_from_glb, mesh_from_gltf_json, sample_skinned,
    skinned_from_embedded_gltf, skinned_from_glb, walk_skinned_gltf, walk_skinned_vrm,
};
pub use input::{KeyEvent, PointerEvent, PointerPhase, VirtualPad};
pub use map::{MapBuilding, MapEdge, RoadNetwork, DEMO_CITY_JSON, SHIBUYA_DEMO_JSON};
pub use mission::{Mission, MissionPhase};
pub use road::{LodLevel, RoadChunk, RoadFrame, RoadPath, RoadStreamer};
pub use save::{SaveGame, Settings};
pub use scene::{DemoScene, DrawList, Quad};
pub use scene3d::{
    Aabb, AlbedoRgba, Camera, Frustum, Instance, LocalLight, Material, MeshData, MeshId,
    RenderStats, Scene3D, Vertex3,
};
pub use session::{FrameStats, SharedSession};
pub use traffic::{TrafficCar, TrafficSystem};
pub use ui::{PauseMenu, UiAction, UiMode};
pub use vehicle::{ChaseCamera, DriveInput, Truck, TruckSpec};
pub use world_doc::{
    compile_meshes, WorldCamera, WorldDoc, WorldHeightfield, WorldLight, WorldProp,
    WorldTerrainTile, WorldWalker, WORLD_DUMP_VERSION,
};
pub use world_play::WorldPlay;

#[cfg(feature = "render")]
pub use render::{render_world_doc, Renderer, SurfaceSource};

/// セマンティックバージョン（ネイティブ／Wasm 双方で照会可能）。
pub const KAGRA_SHARED_VERSION: &str = env!("CARGO_PKG_VERSION");
