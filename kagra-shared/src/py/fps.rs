//! `FpsSession` — `fps_range` GenreSession Python binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{FpsSession, GenreSession};

use super::common::{phase_str, pyerr};

/// First-person arena (`fps_range`). Look, hitscan, ammo, waves, and HP stay in Rust.
#[pyclass(name = "FpsSession")]
pub struct PyFpsSession {
    inner: FpsSession,
}

#[pymethods]
impl PyFpsSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: FpsSession::from_json(json).map_err(pyerr)?,
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
    fn ammo(&self) -> u32 {
        self.inner.ammo()
    }

    #[getter]
    fn mag(&self) -> u32 {
        self.inner.mag()
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
    fn wave(&self) -> u32 {
        self.inner.wave()
    }

    #[getter]
    fn waves(&self) -> u32 {
        self.inner.waves()
    }

    #[getter]
    fn won(&self) -> bool {
        self.inner.won()
    }

    #[getter]
    fn lost(&self) -> bool {
        self.inner.lost()
    }

    #[getter]
    fn reloading(&self) -> bool {
        self.inner.reloading()
    }

    #[getter]
    fn look_yaw(&self) -> f32 {
        self.inner.look_yaw()
    }

    #[getter]
    fn look_pitch(&self) -> f32 {
        self.inner.look_pitch()
    }

    /// Mouse / right-side drag. Radians. Pitch is clamped in Rust.
    fn add_look(&mut self, dyaw: f32, dpitch: f32) {
        self.inner.add_look(dyaw, dpitch);
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
