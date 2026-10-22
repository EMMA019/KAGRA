//! PyO3 bindings: Python ゲームマスターが Rust（shared wgpu 30）を呼ぶ橋。
//!
//! モジュール分割:
//! - `world_doc` / `world_play` — 既存の世界データ + マルチジャンル tick
//! - `render` / `measure` — 描画と HUD 計測
//! - `physics` — Rapier（feature）
//! - `<genre>` — `GenreSession` 実装ごとの薄いラッパ（例: `puzzle`, `platformer`, `novel`, `td`）

mod common;
mod measure;
mod novel;
mod platformer;
mod puzzle;
mod render;
mod td;
mod world_doc;
mod world_play;

#[cfg(feature = "physics")]
mod physics;

use pyo3::prelude::*;

use measure::measure_text_py;
use novel::PyNovelSession;
use platformer::PyPlatformerSession;
use puzzle::PyPuzzlePadSession;
use render::render_world_doc_py;
use td::PyTdSession;
use world_doc::PyWorldDoc;
use world_play::PyWorldPlay;

#[cfg(feature = "physics")]
use physics::PyPhysicsWorld;

#[pymodule]
fn kagra_shared(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyWorldDoc>()?;
    m.add_class::<PyWorldPlay>()?;
    m.add_class::<PyPuzzlePadSession>()?;
    m.add_class::<PyNovelSession>()?;
    m.add_class::<PyPlatformerSession>()?;
    m.add_class::<PyTdSession>()?;
    m.add_function(wrap_pyfunction!(render_world_doc_py, m)?)?;
    m.add_function(wrap_pyfunction!(measure_text_py, m)?)?;
    #[cfg(feature = "physics")]
    m.add_class::<PyPhysicsWorld>()?;
    m.add("__version__", crate::KAGRA_SHARED_VERSION)?;
    Ok(())
}
