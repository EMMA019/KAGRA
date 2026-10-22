//! `PlatformerSession` — `box_hop` GenreSession Python binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{GenreSession, PlatformerSession};

use super::common::{phase_str, pyerr};

/// Side-scrolling platformer (`box_hop`). Implements [`GenreSession`] on the Rust side.
#[pyclass(name = "PlatformerSession")]
pub struct PyPlatformerSession {
    inner: PlatformerSession,
}

#[pymethods]
impl PyPlatformerSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: PlatformerSession::from_json(json).map_err(pyerr)?,
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
    fn dead(&self) -> bool {
        self.inner.dead()
    }

    #[getter]
    fn won(&self) -> bool {
        self.inner.won()
    }

    #[getter]
    fn landed(&self) -> u32 {
        self.inner.landed()
    }

    /// `title` / `play` / `pause` / `result`.
    #[getter]
    fn flow_phase(&self) -> &'static str {
        self.inner.flow_phase()
    }

    #[getter]
    fn best_score(&self) -> u32 {
        self.inner.best_score()
    }

    #[getter]
    fn score(&self) -> u32 {
        self.inner.score()
    }

    fn flow_key(&mut self, key: &str) -> bool {
        self.inner.apply_flow_key(key)
    }

    fn flow_pointer(&mut self, x: f32, y: f32, width: u32, height: u32) -> bool {
        self.inner.apply_flow_pointer(x, y, width, height)
    }

    fn load_progress(&mut self) {
        self.inner.load_progress()
    }

    fn save_progress(&self) {
        self.inner.save_progress()
    }

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
