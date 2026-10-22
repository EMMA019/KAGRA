//! `FishCookSession` — `fishcook` GenreSession Python binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{FishCookSession, GenreSession};

use super::common::{phase_str, pyerr};

/// Cast, tension reel, cook, and sell (`fishcook`). The day stays in Rust.
#[pyclass(name = "FishCookSession")]
pub struct PyFishCookSession {
    inner: FishCookSession,
}

#[pymethods]
impl PyFishCookSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: FishCookSession::from_json(json).map_err(pyerr)?,
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
    fn beat(&self) -> &'static str {
        self.inner.beat()
    }

    #[getter]
    fn coins(&self) -> u32 {
        self.inner.coins()
    }

    #[getter]
    fn goal(&self) -> u32 {
        self.inner.goal()
    }

    #[getter]
    fn sold(&self) -> u32 {
        self.inner.sold()
    }

    #[getter]
    fn day_left(&self) -> f32 {
        self.inner.day_left()
    }

    #[getter]
    fn reel_marker(&self) -> f32 {
        self.inner.reel_marker()
    }

    #[getter]
    fn reel_fill(&self) -> f32 {
        self.inner.reel_fill()
    }

    #[getter]
    fn reel_lo(&self) -> f32 {
        self.inner.reel_lo()
    }

    #[getter]
    fn reel_hi(&self) -> f32 {
        self.inner.reel_hi()
    }

    #[getter]
    fn heat(&self) -> f32 {
        self.inner.heat()
    }

    #[getter]
    fn heat_lo(&self) -> f32 {
        self.inner.heat_lo()
    }

    #[getter]
    fn heat_hi(&self) -> f32 {
        self.inner.heat_hi()
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
    fn on_dock(&self) -> bool {
        self.inner.on_dock()
    }

    #[getter]
    fn at_stove(&self) -> bool {
        self.inner.at_stove()
    }

    #[getter]
    fn at_stall(&self) -> bool {
        self.inner.at_stall()
    }

    /// Screen tap. Title / result confirms. Play: HOLD pad is one attack edge.
    fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) {
        self.inner.pointer(x, y, width, height);
    }

    /// HUD JSON (`quads` + ASCII `texts`) for `draw_world`.
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

    /// `lx` is world +X, `lz` is world +Z. `attack` held reels; an edge casts,
    /// plates a dish, or sells.
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
