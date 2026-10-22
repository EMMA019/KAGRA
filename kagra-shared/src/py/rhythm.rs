//! `RhythmSession` — 4-lane chart `GenreSession` binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{GenreSession, RhythmSession};

use super::common::{phase_str, pyerr};

/// 4-lane chart (`rhythm`). Windows, judgement, combo, and score stay in Rust.
#[pyclass(name = "RhythmSession")]
pub struct PyRhythmSession {
    inner: RhythmSession,
}

#[pymethods]
impl PyRhythmSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: RhythmSession::from_json(json).map_err(pyerr)?,
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
    fn score(&self) -> u32 {
        self.inner.score()
    }

    #[getter]
    fn combo(&self) -> u32 {
        self.inner.combo()
    }

    #[getter]
    fn max_combo(&self) -> u32 {
        self.inner.max_combo()
    }

    #[getter]
    fn perfect_count(&self) -> u32 {
        self.inner.perfect_count()
    }

    #[getter]
    fn good_count(&self) -> u32 {
        self.inner.good_count()
    }

    #[getter]
    fn miss_count(&self) -> u32 {
        self.inner.miss_count()
    }

    #[getter]
    fn note_count(&self) -> u32 {
        self.inner.note_count()
    }

    #[getter]
    fn lane_count(&self) -> u32 {
        self.inner.lane_count()
    }

    #[getter]
    fn song_time(&self) -> f32 {
        self.inner.song_time()
    }

    #[getter]
    fn song_length(&self) -> f32 {
        self.inner.song_length()
    }

    #[getter]
    fn bpm(&self) -> f32 {
        self.inner.bpm()
    }

    #[getter]
    fn last_judge(&self) -> &'static str {
        self.inner.last_judge()
    }

    #[getter]
    fn rank(&self) -> &'static str {
        self.inner.rank()
    }

    fn tick(&mut self, dt: f32) {
        self.inner.tick(dt);
    }

    fn confirm(&mut self) {
        self.inner.confirm();
    }

    fn retry(&mut self) {
        self.inner.retry();
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

    /// Edge-triggered lane (0..3). Returns `perfect`, `good`, or `""`.
    fn press_lane(&mut self, lane: u32) -> &'static str {
        self.inner.press_lane(lane)
    }

    /// Screen tap. Title / result confirms. Play returns the lane, or -1.
    fn pointer(&mut self, x: f32, y: f32, width: f32, height: f32) -> i32 {
        self.inner.pointer(x, y, width, height)
    }

    /// Hit every note in the perfect window. Headless / demo clear.
    fn set_auto(&mut self, on: bool) {
        self.inner.set_auto(on);
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
