//! `StealthSession` — `stealth_hide` GenreSession Python binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{GenreSession, StealthSession};

use super::common::{phase_str, pyerr};

/// Stealth (`stealth_hide`). Patrols, cones, noise, and the alert meter stay in Rust.
#[pyclass(name = "StealthSession")]
pub struct PyStealthSession {
    inner: StealthSession,
}

#[pymethods]
impl PyStealthSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: StealthSession::from_json(json).map_err(pyerr)?,
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
    fn hidden(&self) -> bool {
        self.inner.hidden()
    }

    #[getter]
    fn seen(&self) -> bool {
        self.inner.seen()
    }

    #[getter]
    fn done(&self) -> bool {
        self.inner.done()
    }

    #[getter]
    fn clear(&self) -> bool {
        self.inner.clear()
    }

    #[getter]
    fn alert(&self) -> f32 {
        self.inner.alert()
    }

    #[getter]
    fn alert_state(&self) -> &'static str {
        self.inner.alert_state()
    }

    #[getter]
    fn has_key(&self) -> bool {
        self.inner.has_key()
    }

    #[getter]
    fn crouching(&self) -> bool {
        self.inner.crouching()
    }

    #[getter]
    fn noise(&self) -> f32 {
        self.inner.noise()
    }

    /// Scripted crouch route. Returns `(lx, lz, crouch)`.
    fn demo_input(&self) -> (f32, f32, bool) {
        let wish = self.inner.demo_input();
        (wish.lx, wish.lz, wish.dodge)
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
