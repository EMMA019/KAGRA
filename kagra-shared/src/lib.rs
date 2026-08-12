//! KAGRA shared library — desktop / Wasm / Android / iOS 共通コア。
//!
//! - 入力: PointerEvent / VirtualPad（`docs/schemas/input_events.json` と同期）
//! - セッション: 画面サイズ・ポーズ・フレームカウンタ
//! - FFI: C ABI（`ffi`）と任意の wasm-bindgen（feature = "wasm"）

pub mod assets;
pub mod ffi;
pub mod input;
pub mod session;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use assets::{resolve_alias, AssetKind};
pub use input::{KeyEvent, PointerEvent, PointerPhase, VirtualPad};
pub use session::{FrameStats, SharedSession};

/// セマンティックバージョン（ネイティブ／Wasm 双方で照会可能）。
pub const KAGRA_SHARED_VERSION: &str = env!("CARGO_PKG_VERSION");
