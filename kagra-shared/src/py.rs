//! PyO3 bindings: Python ゲームマスターが Rust（shared wgpu 30）を呼ぶ橋。
//!
//! - `WorldDoc` — dump JSON の読み書き（世界はデータ）
//! - `WorldPlay` — 1 tick のステップ、入力、接着 API（emit_event /
//!   take_events / start_timer / step_interact）
//! - `render_world_doc` — dump → RGBA（オフスクリーン描画）
//!
//! Python 側はこれを使って「ループを所有」する。ジャンルロジック（敵 AI・
//! ターン・識別）は Python で書く。Rust は世界のデータ化・tick・描画を担う。

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::world_doc::WorldDoc;
use crate::world_play::WorldPlay;

fn pyerr<E: std::fmt::Display>(e: E) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// dump JSON の読み書き。世界の「データ」としての入口。
#[pyclass(name = "WorldDoc")]
struct PyWorldDoc {
    inner: WorldDoc,
}

#[pymethods]
impl PyWorldDoc {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: WorldDoc::from_json(json).map_err(pyerr)?,
        })
    }

    fn to_json(&self) -> PyResult<String> {
        self.inner.to_json().map_err(pyerr)
    }

    /// Player の位置 [x, y, z]（無ければ None）。
    fn player_position(&self) -> Option<[f32; 3]> {
        self.inner.player.as_ref().map(|w| w.position)
    }

    /// 指定 name の prop 一覧（id, position）。
    fn props_named(&self, name: &str) -> Vec<(String, [f32; 3])> {
        self.inner
            .props
            .iter()
            .filter(|p| p.name == name && p.enabled)
            .map(|p| (p.id.clone(), p.position))
            .collect()
    }
}

/// ゲームループの 1 ステップ。tick が世界を進め、dump() で読む。
#[pyclass(name = "WorldPlay")]
struct PyWorldPlay {
    inner: WorldPlay,
}

#[pymethods]
impl PyWorldPlay {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: WorldPlay::from_json(json).map_err(pyerr)?,
        })
    }

    /// 1 ステップ進める。`dt` は秒（1/60 など）。タイトル待ちは start() で。
    fn tick(&mut self, dt: f32) {
        self.inner.tick(dt);
    }

    /// タイトル / 結果画面で Space 相当。プレイ中は無視。
    fn confirm(&mut self) {
        self.inner.confirm();
    }

    /// 現在の世界を dump JSON で返す（Python が読んでロジックに使う）。
    fn dump(&self) -> PyResult<String> {
        self.inner.doc.to_json().map_err(pyerr)
    }

    /// WASD 入力。lx/lz は -1..1（カメラ基準）。attack = J / click。
    fn set_input(&mut self, lx: f32, lz: f32, jump: bool, attack: bool, dodge: bool) {
        self.inner.input = WalkInput {
            lx,
            lz,
            jump,
            attack,
            dodge,
        }
        .clamped();
    }

    /// 接着 API: 出来事を記録（同名は集約）。
    fn emit_event(&mut self, name: &str, data_json: Option<&str>) -> PyResult<()> {
        let payload = match data_json {
            Some(s) if !s.trim().is_empty() => Some(serde_json::from_str(s).map_err(pyerr)?),
            _ => None,
        };
        self.inner.emit_event(name, payload);
        Ok(())
    }

    /// 接着 API: 指定名の出来事を消費して JSON 文字列のリストで返す。
    fn take_events(&mut self, name: &str) -> Vec<String> {
        self.inner
            .take_events(name)
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect()
    }

    /// 接着 API: タイマーを起動。on_done は 0 秒時にイベントとして発火。
    fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        self.inner.start_timer(name, seconds, on_done)
    }

    /// 接着 API: J を押した状態で、近くの interactable があれば on_use を発火。
    fn step_interact(&mut self) {
        self.inner.step_interact();
    }

    /// 現在のアニメーション状態（walker.anim）。
    fn anim(&self) -> String {
        self.inner
            .doc
            .player
            .as_ref()
            .map(|w| w.anim.clone())
            .unwrap_or_default()
    }
}

/// dump JSON を shared wgpu 30 でオフスクリーン描画し、RGBA8 を bytes で返す。
#[pyfunction(name = "render_world_doc")]
fn render_world_doc_py(
    py: Python<'_>,
    json: &str,
    width: u32,
    height: u32,
) -> PyResult<pyo3::Py<pyo3::types::PyBytes>> {
    let doc = WorldDoc::from_json(json).map_err(pyerr)?;
    let rgba = crate::render::render_world_doc(&doc, width, height).map_err(pyerr)?;
    Ok(pyo3::types::PyBytes::new(py, &rgba).into())
}

#[pymodule]
fn kagra_shared(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyWorldDoc>()?;
    m.add_class::<PyWorldPlay>()?;
    m.add_function(wrap_pyfunction!(render_world_doc_py, m)?)?;
    m.add("__version__", crate::KAGRA_SHARED_VERSION)?;
    Ok(())
}
