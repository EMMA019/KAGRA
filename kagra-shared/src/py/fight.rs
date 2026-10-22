//! `FightSession` — `fight_hitstun` GenreSession Python binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{FightSession, GenreSession};

use super::common::{phase_str, pyerr};

/// 1v1 fight (`fight_hitstun`). Hitstun, guard, combos, and opponent AI stay in Rust.
#[pyclass(name = "FightSession")]
pub struct PyFightSession {
    inner: FightSession,
}

#[pymethods]
impl PyFightSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: FightSession::from_json(json).map_err(pyerr)?,
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
    fn opp_hp(&self) -> u32 {
        self.inner.opp_hp()
    }

    #[getter]
    fn opp_hp_max(&self) -> u32 {
        self.inner.opp_hp_max()
    }

    #[getter]
    fn hits(&self) -> u32 {
        self.inner.hits()
    }

    #[getter]
    fn combo(&self) -> u32 {
        self.inner.combo()
    }

    #[getter]
    fn won(&self) -> bool {
        self.inner.won()
    }

    #[getter]
    fn ko(&self) -> bool {
        self.inner.ko()
    }

    #[getter]
    fn done(&self) -> bool {
        self.inner.done()
    }

    #[getter]
    fn guarding(&self) -> bool {
        self.inner.guarding()
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
        self.set_player_input(0, lx, lz, jump, attack, dodge);
    }

    /// Slot 0 is the local fighter. Slot 1 replaces the opponent AI.
    fn set_player_input(
        &mut self,
        slot: u8,
        lx: f32,
        lz: f32,
        jump: bool,
        attack: bool,
        dodge: bool,
    ) {
        self.inner.set_player_input(
            slot,
            WalkInput {
                lx,
                lz,
                jump,
                attack,
                dodge,
            },
        );
    }

    #[getter]
    fn player_slots(&self) -> u8 {
        self.inner.player_slots()
    }

    #[getter]
    fn fingerprint(&self) -> u64 {
        self.inner.fingerprint()
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
