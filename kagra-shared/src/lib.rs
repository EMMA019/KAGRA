//! KAGRA shared library — desktop / Wasm / Android / iOS 共通コア。
//!
//! - 入力: PointerEvent / VirtualPad（`docs/schemas/input_events.json` と同期）
//! - セッション: 画面サイズ・ポーズ・フレームカウンタ
//! - シーン: 描画内容の記述（GPU 非依存なのでテストできる）
//! - 描画: wgpu 2D（feature = "render"）
//! - FFI: C ABI（`ffi`）と任意の wasm-bindgen（feature = "wasm"）

pub mod assets;
pub mod ffi;
pub mod input;
pub mod scene;
pub mod session;

#[cfg(feature = "render")]
pub mod render;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use assets::{resolve_alias, AssetKind};
pub use input::{KeyEvent, PointerEvent, PointerPhase, VirtualPad};
pub use scene::{DemoScene, DrawList, Quad};
pub use session::{FrameStats, SharedSession};

#[cfg(feature = "render")]
pub use render::{Renderer, SurfaceSource};

/// セマンティックバージョン（ネイティブ／Wasm 双方で照会可能）。
pub const KAGRA_SHARED_VERSION: &str = env!("CARGO_PKG_VERSION");
