//! PyO3 bindings: Python ゲームマスターが Rust（shared wgpu 30）を呼ぶ橋。
//!
//! モジュール分割:
//! - `world_doc` / `world_play` — 既存の世界データ + マルチジャンル tick
//! - `render` / `measure` — 描画と HUD 計測
//! - `physics` — Rapier（feature）
//! - `<genre>` — `GenreSession` 実装ごとの薄いラッパ（例: `puzzle`, `platformer`, `novel`, `race`, `sports`, `action`, `fight`, `rhythm`, `td`, `rpg`, `shop`, `fishcook`, `stealth`, `survival`）

mod action;
mod common;
mod fight;
mod fishcook;
mod measure;
mod novel;
mod platformer;
mod puzzle;
mod race;
mod render;
mod rhythm;
mod rpg;
mod shop;
mod sports;
mod stealth;
mod survival;
mod td;
mod world_doc;
mod world_play;

#[cfg(feature = "physics")]
mod physics;

use pyo3::prelude::*;

use action::PyActionSession;
use fight::PyFightSession;
use fishcook::PyFishCookSession;
use measure::measure_text_py;
use novel::PyNovelSession;
use platformer::PyPlatformerSession;
use puzzle::{PyPuzzlePadSession, PyPuzzleSession};
use race::PyRaceSession;
use render::render_world_doc_py;
use rhythm::PyRhythmSession;
use rpg::PyRpgSession;
use shop::PyShopSession;
use sports::PySportsSession;
use stealth::PyStealthSession;
use survival::PySurvivalSession;
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
    m.add_class::<PyPuzzleSession>()?;
    m.add_class::<PyNovelSession>()?;
    m.add_class::<PyPlatformerSession>()?;
    m.add_class::<PyRaceSession>()?;
    m.add_class::<PySportsSession>()?;
    m.add_class::<PyActionSession>()?;
    m.add_class::<PyFightSession>()?;
    m.add_class::<PyRhythmSession>()?;
    m.add_class::<PyRpgSession>()?;
    m.add_class::<PyShopSession>()?;
    m.add_class::<PyTdSession>()?;
    m.add_class::<PyFishCookSession>()?;
    m.add_class::<PyStealthSession>()?;
    m.add_class::<PySurvivalSession>()?;
    m.add_function(wrap_pyfunction!(render_world_doc_py, m)?)?;
    m.add_function(wrap_pyfunction!(measure_text_py, m)?)?;
    #[cfg(feature = "physics")]
    m.add_class::<PyPhysicsWorld>()?;
    m.add("__version__", crate::KAGRA_SHARED_VERSION)?;
    Ok(())
}
