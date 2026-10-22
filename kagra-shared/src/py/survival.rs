//! `SurvivalSession` — camp survival GenreSession Python binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{GenreSession, SurvivalSession};

use super::common::{phase_str, pyerr};

/// Three-day camp (`survival`). Clock, hunger, gathering, and crafting stay in Rust.
#[pyclass(name = "SurvivalSession")]
pub struct PySurvivalSession {
    inner: SurvivalSession,
}

#[pymethods]
impl PySurvivalSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: SurvivalSession::from_json(json).map_err(pyerr)?,
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
    fn day(&self) -> u32 {
        self.inner.day()
    }

    #[getter]
    fn days_need(&self) -> u32 {
        self.inner.days_need()
    }

    #[getter]
    fn time_of_day(&self) -> f32 {
        self.inner.time_of_day()
    }

    #[getter]
    fn night(&self) -> bool {
        self.inner.night()
    }

    #[getter]
    fn safe(&self) -> bool {
        self.inner.is_safe()
    }

    #[getter]
    fn hunger(&self) -> f32 {
        self.inner.hunger()
    }

    #[getter]
    fn health(&self) -> f32 {
        self.inner.health()
    }

    #[getter]
    fn wood(&self) -> u32 {
        self.inner.wood()
    }

    #[getter]
    fn food(&self) -> u32 {
        self.inner.food()
    }

    #[getter]
    fn fire(&self) -> bool {
        self.inner.fire()
    }

    #[getter]
    fn shelter(&self) -> bool {
        self.inner.shelter()
    }

    #[getter]
    fn won(&self) -> bool {
        self.inner.won()
    }

    #[getter]
    fn dead(&self) -> bool {
        self.inner.dead()
    }

    #[getter]
    fn status(&self) -> String {
        self.inner.status()
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

    fn demo_input(&self) -> (f32, f32, bool, bool, bool) {
        let input = self.inner.demo_input();
        (input.lx, input.lz, input.jump, input.attack, input.dodge)
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
