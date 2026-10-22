//! Python `kagra.audio` bridge: BGM, SFX, groups, mute, save directory.

use std::path::PathBuf;

use pyo3::prelude::*;

use super::common::pyerr;

#[pyfunction]
#[pyo3(name = "play_bgm")]
pub fn play_bgm_py(name: &str, fade: f32) {
    crate::mixer::play_bgm(name, fade);
}

#[pyfunction]
#[pyo3(name = "stop_bgm")]
pub fn stop_bgm_py(fade: f32) {
    crate::mixer::stop_bgm(fade);
}

#[pyfunction]
#[pyo3(name = "play_sfx")]
pub fn play_sfx_py(name: &str) {
    crate::mixer::cue_sfx(name);
}

#[pyfunction]
#[pyo3(name = "set_group_volume")]
pub fn set_group_volume_py(group: &str, volume: f32) -> PyResult<()> {
    crate::mixer::set_group(group, volume).map_err(pyerr)
}

#[pyfunction]
#[pyo3(name = "set_mute")]
pub fn set_mute_py(muted: bool) {
    crate::mixer::set_mute(muted);
}

#[pyfunction]
#[pyo3(name = "mixer_state")]
pub fn mixer_state_py() -> String {
    crate::mixer::state_json()
}

#[pyfunction]
#[pyo3(name = "audio_cues")]
pub fn cues_json_py() -> String {
    crate::mixer::cues_json()
}

#[pyfunction]
#[pyo3(name = "set_save_dir")]
pub fn set_save_dir_py(path: &str) {
    let path = if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    };
    crate::flow::set_save_root(path);
}

#[pyfunction]
#[pyo3(name = "save_dir")]
pub fn save_dir_py() -> String {
    crate::flow::save_dir().to_string_lossy().into_owned()
}
