//! `NovelSession` — visual novel `GenreSession` binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{GenreSession, NovelSession};

use super::common::{phase_str, pyerr};

/// Branching text (`novel_pages`). Lines and tap-to-advance stay in Rust.
#[pyclass(name = "NovelSession")]
pub struct PyNovelSession {
    inner: NovelSession,
}

#[pymethods]
impl PyNovelSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: NovelSession::from_json(json).map_err(pyerr)?,
        })
    }

    #[getter]
    fn genre_id(&self) -> &'static str {
        self.inner.genre_id()
    }

    #[getter]
    fn phase(&self) -> &'static str {
        phase_str(self.inner.phase())
    }

    #[getter]
    fn page(&self) -> u32 {
        self.inner.page()
    }

    #[getter]
    fn choice(&self) -> u32 {
        self.inner.choice()
    }

    #[getter]
    fn done(&self) -> bool {
        self.inner.done()
    }

    #[getter]
    fn flag(&self) -> Option<String> {
        self.inner.flag()
    }

    #[getter]
    fn speaker(&self) -> &'static str {
        self.inner.speaker()
    }

    #[getter]
    fn body(&self) -> &'static str {
        self.inner.body()
    }

    fn choice_label(&self, index: u32) -> &'static str {
        self.inner.choice_label(index)
    }

    /// Screen tap. Title / result starts or retries. Play advances or picks.
    fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) {
        self.inner.pointer(x, y, width, height);
    }

    /// HUD JSON (`quads` + Japanese `texts`) for `draw_world`.
    fn hud_json(&self, width: u32, height: u32) -> String {
        self.inner.hud_json(width, height)
    }

    fn tick(&mut self, dt: f32) {
        self.inner.tick(dt);
    }

    fn confirm(&mut self) {
        self.inner.confirm();
    }

    fn dump(&self) -> PyResult<String> {
        self.inner.dump().map_err(pyerr)
    }

    fn set_input(&mut self, lx: f32, lz: f32, jump: bool, attack: bool, dodge: bool) {
        self.inner.set_input(
            WalkInput {
                lx,
                lz,
                jump,
                attack,
                dodge,
            }
            .clamped(),
        );
    }

    fn emit_event(&mut self, name: &str, data_json: Option<&str>) -> PyResult<()> {
        let payload = match data_json {
            Some(s) if !s.trim().is_empty() => Some(serde_json::from_str(s).map_err(pyerr)?),
            _ => None,
        };
        self.inner.emit_event(name, payload).map_err(pyerr)?;
        Ok(())
    }

    fn take_events(&mut self, name: &str) -> Vec<String> {
        self.inner
            .take_events(name)
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect()
    }

    fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        self.inner.start_timer(name, seconds, on_done)
    }

    fn step_interact(&mut self) {
        self.inner.step_interact();
    }
}
