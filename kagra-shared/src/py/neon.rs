//! `NeonRun` — Lumen flyer. Not a WorldPlay genre.

use pyo3::prelude::*;

use crate::neon_run::NeonRun;

use super::common::pyerr;

#[pyclass(name = "NeonRun")]
pub struct PyNeonRun {
    inner: NeonRun,
}

#[pymethods]
impl PyNeonRun {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: NeonRun::from_json(json).map_err(pyerr)?,
        })
    }

    #[getter]
    fn phase(&self) -> &'static str {
        self.inner.phase_name()
    }

    #[getter]
    fn flow_phase(&self) -> &'static str {
        self.inner.phase_name()
    }

    #[getter]
    fn score(&self) -> u32 {
        self.inner.score()
    }

    #[getter]
    fn best_score(&self) -> u32 {
        self.inner.best_score()
    }

    #[getter]
    fn orbs(&self) -> u32 {
        self.inner.orbs
    }

    #[getter]
    fn alive(&self) -> bool {
        self.inner.alive
    }

    fn set_input(&mut self, lx: f32, ly: f32, jump: bool) {
        self.inner.set_steer(lx, ly, jump);
    }

    fn tick(&mut self, dt: f32) {
        self.inner.tick(dt);
    }

    fn confirm(&mut self) {
        self.inner.confirm();
    }

    fn flow_key(&mut self, key: &str) -> bool {
        self.inner.on_key(key, true)
    }

    fn flow_pointer(&mut self, x: f32, y: f32, width: u32, height: u32) -> bool {
        self.inner.pointer(x, y, width, height)
    }

    fn dump(&self) -> PyResult<String> {
        Ok(self.inner.dump_json())
    }

    fn state_json(&self) -> String {
        self.inner.state_json()
    }

    fn take_feel(&mut self) -> String {
        self.inner.take_feel_json()
    }

    fn hud_json(&self, width: u32, height: u32) -> String {
        self.inner.hud_json(width, height)
    }
}
