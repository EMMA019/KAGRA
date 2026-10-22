//! `PuzzlePadSession` — reference `GenreSession` Python binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{GenreSession, PuzzlePadSession, PuzzleSession};

use super::common::{phase_str, pyerr};

/// Push-box puzzle (`puzzle_pad`). Implements [`GenreSession`] on the Rust side.
#[pyclass(name = "PuzzlePadSession")]
pub struct PyPuzzlePadSession {
    inner: PuzzlePadSession,
}

#[pymethods]
impl PyPuzzlePadSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: PuzzlePadSession::from_json(json).map_err(pyerr)?,
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
    fn pushing(&self) -> bool {
        self.inner.pushing()
    }

    #[getter]
    fn solved(&self) -> bool {
        self.inner.solved()
    }

    #[getter]
    fn done(&self) -> bool {
        self.inner.done()
    }

    #[getter]
    fn latch_open(&self) -> bool {
        self.inner.latch_open()
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

/// Slide, lights-out grid, and match (`genre: puzzle`). Not [`PyPuzzlePadSession`].
#[pyclass(name = "PuzzleSession")]
pub struct PyPuzzleSession {
    inner: PuzzleSession,
}

#[pymethods]
impl PyPuzzleSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: PuzzleSession::from_json(json).map_err(pyerr)?,
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
    fn level(&self) -> usize {
        self.inner.level()
    }

    #[getter]
    fn level_count(&self) -> usize {
        self.inner.level_count()
    }

    #[getter]
    fn kind(&self) -> &'static str {
        self.inner.kind_name()
    }

    #[getter]
    fn cols(&self) -> usize {
        self.inner.cols()
    }

    #[getter]
    fn rows(&self) -> usize {
        self.inner.rows()
    }

    #[getter]
    fn moves(&self) -> u32 {
        self.inner.moves()
    }

    #[getter]
    fn cleared(&self) -> u32 {
        self.inner.cleared()
    }

    #[getter]
    fn quota(&self) -> u32 {
        self.inner.quota()
    }

    #[getter]
    fn solved(&self) -> bool {
        self.inner.solved()
    }

    #[getter]
    fn won(&self) -> bool {
        self.inner.won()
    }

    fn cursor(&self) -> (i32, i32) {
        self.inner.cursor()
    }

    fn cells(&self) -> Vec<u32> {
        self.inner.cells()
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

    fn swipe(&mut self, dx: i32, dy: i32) -> bool {
        self.inner.swipe(dx, dy)
    }

    fn swipe_at(&mut self, col: i32, row: i32, dx: i32, dy: i32) -> bool {
        self.inner.swipe_at(col, row, dx, dy)
    }

    fn tap(&mut self, col: i32, row: i32) -> bool {
        self.inner.tap(col, row)
    }

    fn cell_at(&self, px: f32, py: f32, width: f32, height: f32) -> Option<(i32, i32)> {
        self.inner.cell_at(px, py, width, height)
    }

    fn play_demo_move(&mut self) -> bool {
        self.inner.play_demo_move()
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
