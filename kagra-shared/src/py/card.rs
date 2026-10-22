//! `CardSession` — card duel `GenreSession` binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{CardSession, GenreSession};

use super::common::{phase_str, pyerr};

/// 1v1 card duel (`card`). Rules stay in Rust. The dump `card` object is the match.
#[pyclass(name = "CardSession")]
pub struct PyCardSession {
    inner: CardSession,
}

#[pymethods]
impl PyCardSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: CardSession::from_json(json).map_err(pyerr)?,
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
    fn you_life(&self) -> i32 {
        self.inner.you_life()
    }

    #[getter]
    fn foe_life(&self) -> i32 {
        self.inner.foe_life()
    }

    #[getter]
    fn mana(&self) -> u8 {
        self.inner.mana()
    }

    #[getter]
    fn mana_max(&self) -> u8 {
        self.inner.mana_max()
    }

    #[getter]
    fn turn(&self) -> u32 {
        self.inner.turn()
    }

    #[getter]
    fn active(&self) -> u8 {
        self.inner.active()
    }

    #[getter]
    fn winner(&self) -> Option<u8> {
        self.inner.winner()
    }

    #[getter]
    fn you_hand(&self) -> usize {
        self.inner.you_hand()
    }

    #[getter]
    fn foe_hand(&self) -> usize {
        self.inner.foe_hand()
    }

    #[getter]
    fn you_board(&self) -> usize {
        self.inner.you_board()
    }

    #[getter]
    fn foe_board(&self) -> usize {
        self.inner.foe_board()
    }

    fn status(&self) -> String {
        self.inner.status()
    }

    fn hint(&self) -> &'static str {
        self.inner.hint()
    }

    /// Player view. Opponent hand and deck order are counts only.
    fn view_json(&self) -> String {
        self.inner.view_json()
    }

    fn play(&mut self, index: usize) -> bool {
        self.inner.play(index)
    }

    /// `target` -1 is the opposing hero. Any other value is a board index.
    fn attack(&mut self, index: usize, target: i32) -> bool {
        self.inner.attack(index, target)
    }

    fn end_turn(&mut self) -> bool {
        self.inner.end_turn()
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
            Some(raw) if !raw.trim().is_empty() => Some(serde_json::from_str(raw).map_err(pyerr)?),
            _ => None,
        };
        self.inner.emit_event(name, payload).map_err(pyerr)?;
        Ok(())
    }

    fn take_events(&mut self, name: &str) -> Vec<String> {
        self.inner
            .take_events(name)
            .iter()
            .filter_map(|event| serde_json::to_string(event).ok())
            .collect()
    }

    fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        self.inner.start_timer(name, seconds, on_done)
    }

    fn step_interact(&mut self) {
        self.inner.step_interact();
    }
}
