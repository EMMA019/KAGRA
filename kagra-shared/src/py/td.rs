//! `TdSession` — `td_lane` GenreSession Python binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{GenreSession, TdSession};

use super::common::{phase_str, pyerr};

/// Tower defense (`td_lane`). Waves, gold, lives, and placement stay in Rust.
#[pyclass(name = "TdSession")]
pub struct PyTdSession {
    inner: TdSession,
}

#[pymethods]
impl PyTdSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: TdSession::from_json(json).map_err(pyerr)?,
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
    fn gold(&self) -> u32 {
        self.inner.gold()
    }

    #[getter]
    fn lives(&self) -> u32 {
        self.inner.lives()
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
    fn selected(&self) -> u8 {
        self.inner.selected()
    }

    #[getter]
    fn kills(&self) -> u32 {
        self.inner.kills()
    }

    #[getter]
    fn leaks(&self) -> u32 {
        self.inner.leaks()
    }

    #[getter]
    fn won(&self) -> bool {
        self.inner.won()
    }

    #[getter]
    fn done(&self) -> bool {
        self.inner.done()
    }

    /// 0 arrow, 1 cannon, 2 mage.
    fn select_tower(&mut self, kind: u8) {
        self.inner.select_tower(kind);
    }

    /// Screen tap or mouse click. Title / result starts or retries.
    /// Play selects a tower button or places on a slot.
    fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) {
        self.inner.pointer(x, y, width, height);
    }

    /// World point to screen pixels (top-left origin). None if off-camera.
    fn world_to_screen(
        &self,
        x: f32,
        y: f32,
        z: f32,
        width: u32,
        height: u32,
    ) -> Option<(f32, f32)> {
        self.inner.world_to_screen(x, y, z, width, height)
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
