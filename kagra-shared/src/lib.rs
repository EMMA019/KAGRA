//! KAGRA shared library — desktop / Wasm / Android / iOS 共通コア。
//!
//! - 入力: PointerEvent / VirtualPad / DriveInput（`docs/schemas/input_events.json` と同期）
//! - セッション: 画面サイズ・ポーズ・フレームカウンタ
//! - シーン: 描画内容の記述（2D も 3D も GPU 非依存なのでテストできる）
//! - 車両: 自転車モデルの運動と追従カメラ（純関数）
//! - 描画: wgpu 3D + 2D HUD（feature = "render"）
//! - FFI: C ABI（`ffi`）と任意の wasm-bindgen（feature = "wasm"）

pub mod assets;
pub mod audio;
pub mod driving;
pub mod ffi;
pub mod gltf_load;
pub mod input;
pub mod road;
pub mod save;
pub mod scene;
pub mod scene3d;
pub mod session;
pub mod vehicle;

#[cfg(feature = "render")]
pub mod render;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use assets::{resolve_alias, AssetKind};
pub use audio::AudioLevels;
pub use driving::{DrivingScene, MeshIds, MeshSet};
pub use gltf_load::{mesh_from_embedded_gltf, mesh_from_gltf_json};
pub use input::{KeyEvent, PointerEvent, PointerPhase, VirtualPad};
pub use road::{LodLevel, RoadChunk, RoadFrame, RoadPath, RoadStreamer};
pub use save::{SaveGame, Settings};
pub use scene::{DemoScene, DrawList, Quad};
pub use scene3d::{Aabb, Camera, Frustum, Instance, Material, MeshData, MeshId, Scene3D, Vertex3};
pub use session::{FrameStats, SharedSession};
pub use vehicle::{ChaseCamera, DriveInput, Truck, TruckSpec};

#[cfg(feature = "render")]
pub use render::{Renderer, SurfaceSource};

/// セマンティックバージョン（ネイティブ／Wasm 双方で照会可能）。
pub const KAGRA_SHARED_VERSION: &str = env!("CARGO_PKG_VERSION");
