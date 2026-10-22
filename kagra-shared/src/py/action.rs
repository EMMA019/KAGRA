//! `ActionSession` — `action_arena` GenreSession Python binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{ActionSession, GenreSession};

use super::common::{phase_str, pyerr};

/// Arena combat (`action_arena`). Hitboxes, HP, combos, and enemy AI stay in Rust.
#[pyclass(name = "ActionSession")]
pub struct PyActionSession {
    inner: ActionSession,
}

#[pymethods]
impl PyActionSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: ActionSession::from_json(json).map_err(pyerr)?,
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
    fn hp(&self) -> u32 {
        self.inner.hp()
    }

    #[getter]
    fn hp_max(&self) -> u32 {
        self.inner.hp_max()
    }

    #[getter]
    fn hits(&self) -> u32 {
        self.inner.hits()
    }

    #[getter]
    fn kills(&self) -> u32 {
        self.inner.kills()
    }

    #[getter]
    fn combo(&self) -> u32 {
        self.inner.combo()
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
    fn boss_up(&self) -> bool {
        self.inner.boss_up()
    }

    #[getter]
    fn boss_hp(&self) -> u32 {
        self.inner.boss_hp()
    }

    #[getter]
    fn boss_hp_max(&self) -> u32 {
        self.inner.boss_hp_max()
    }

    #[getter]
    fn foes_left(&self) -> u32 {
        self.inner.foes_left()
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
