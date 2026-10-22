//! `Lockstep` — input-sync buffer for the Python bridge.
//!
//! Sockets stay in `kagra/netplay.py`. This class only quantizes sticks,
//! buffers a tick until every slot is present, and speaks [`NetMessage`] JSON.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::netplay::{fixed_dt, Lockstep, NetMessage};

use super::common::pyerr;

/// One stick: lx, lz, jump, attack, dodge.
type StickRow = (f32, f32, bool, bool, bool);

/// Two-slot (or one-slot) lockstep. `submit_local` returns JSON to send.
#[pyclass(name = "Lockstep")]
pub struct PyLockstep {
    inner: Lockstep,
}

#[pymethods]
impl PyLockstep {
    #[new]
    fn new(slot: u8, players: u8) -> PyResult<Self> {
        Ok(Self {
            inner: Lockstep::new(slot, players).map_err(pyerr)?,
        })
    }

    #[staticmethod]
    fn hello(room: &str, name: &str) -> PyResult<String> {
        NetMessage::hello(room, name).to_json().map_err(pyerr)
    }

    /// Canonical JSON for any [`NetMessage`], or an error if it does not parse.
    #[staticmethod]
    fn decode(json: &str) -> PyResult<String> {
        let msg = NetMessage::from_json(json).map_err(pyerr)?;
        msg.to_json().map_err(pyerr)
    }

    /// `num/den` as the same `f32` the sim uses. Do not divide in Python.
    #[staticmethod]
    fn dt(num: u32, den: u32) -> f32 {
        fixed_dt(num, den)
    }

    #[getter]
    fn slot(&self) -> u8 {
        self.inner.local_slot()
    }

    #[getter]
    fn players(&self) -> u8 {
        self.inner.players()
    }

    fn submit_local(
        &mut self,
        tick: u32,
        lx: f32,
        lz: f32,
        jump: bool,
        attack: bool,
        dodge: bool,
    ) -> PyResult<String> {
        let msg = self
            .inner
            .submit_local(
                tick,
                WalkInput {
                    lx,
                    lz,
                    jump,
                    attack,
                    dodge,
                },
            )
            .map_err(pyerr)?;
        msg.to_json().map_err(pyerr)
    }

    fn ingest(&mut self, json: &str) -> PyResult<()> {
        let msg = NetMessage::from_json(json).map_err(pyerr)?;
        self.inner.ingest(&msg).map_err(pyerr)
    }

    fn ready(&self, tick: u32) -> bool {
        self.inner.ready(tick)
    }

    /// Slot-order sticks `(lx, lz, jump, attack, dodge)`, or `None` if not ready.
    fn inputs(&self, tick: u32) -> Option<Vec<StickRow>> {
        self.inner.inputs(tick).map(|rows| {
            rows.into_iter()
                .map(|w| (w.lx, w.lz, w.jump, w.attack, w.dodge))
                .collect()
        })
    }
}
