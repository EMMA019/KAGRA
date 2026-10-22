//! Text measurement for HUD layout.

use pyo3::prelude::*;

/// テキスト行幅（ピクセル）。UI パネルの折り返し用。
#[pyfunction(name = "measure_text")]
pub fn measure_text_py(text: &str, size: f32) -> f32 {
    crate::font::measure_text(text, size)
}
