//! `WorldPlay` Python binding (multi-genre loop).

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::world_play::WorldPlay;

use super::common::pyerr;

/// ゲームループの 1 ステップ。tick が世界を進め、dump() で読む。
#[pyclass(name = "WorldPlay")]
pub struct PyWorldPlay {
    pub(crate) inner: WorldPlay,
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
