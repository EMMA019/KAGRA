//! `RaceSession` — `race_drive` GenreSession Python binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{GenreSession, RaceSession};

use super::common::{phase_str, pyerr};

/// Racing (`race_drive`). Lap clock, ghost, and the vehicle stay in Rust.
#[pyclass(name = "RaceSession")]
pub struct PyRaceSession {
    inner: RaceSession,
}

#[pymethods]
impl PyRaceSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: RaceSession::from_json(json).map_err(pyerr)?,
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
    fn laps(&self) -> u32 {
        self.inner.laps()
    }

    #[getter]
    fn laps_need(&self) -> u32 {
        self.inner.laps_need()
    }

    #[getter]
    fn lap_time(&self) -> f32 {
        self.inner.lap_time()
    }

    #[getter]
    fn best_lap(&self) -> f32 {
        self.inner.best_lap()
    }

    #[getter]
    fn race_time(&self) -> f32 {
        self.inner.race_time()
    }

    #[getter]
    fn speed(&self) -> f32 {
        self.inner.speed()
    }

    #[getter]
    fn won(&self) -> bool {
        self.inner.won()
    }

    #[getter]
    fn done(&self) -> bool {
        self.inner.done()
    }

    #[getter]
    fn ghost(&self) -> bool {
        self.inner.ghost()
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
