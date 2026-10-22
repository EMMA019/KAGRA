//! `RpgSession` — `town_gate` GenreSession Python binding.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{GenreSession, RpgSession};

use super::common::{phase_str, pyerr};

/// Town RPG (`town_gate`). Stats, turn battle, inventory, gold, and the
/// village shop stay in Rust.
#[pyclass(name = "RpgSession")]
pub struct PyRpgSession {
    inner: RpgSession,
}

#[pymethods]
impl PyRpgSession {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: RpgSession::from_json(json).map_err(pyerr)?,
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
    fn max_hp(&self) -> u32 {
        self.inner.max_hp()
    }

    #[getter]
    fn foe_hp(&self) -> u32 {
        self.inner.foe_hp()
    }

    #[getter]
    fn level(&self) -> u32 {
        self.inner.level()
    }

    #[getter]
    fn xp(&self) -> u32 {
        self.inner.xp()
    }

    #[getter]
    fn atk(&self) -> u32 {
        self.inner.atk()
    }

    #[getter]
    fn def(&self) -> u32 {
        self.inner.def()
    }

    #[getter]
    fn gold(&self) -> u32 {
        self.inner.gold()
    }

    #[getter]
    fn talking(&self) -> bool {
        self.inner.talking()
    }

    #[getter]
    fn menu(&self) -> bool {
        self.inner.menu()
    }

    #[getter]
    fn combat(&self) -> bool {
        self.inner.combat()
    }

    #[getter]
    fn shop_open(&self) -> bool {
        self.inner.shop_open()
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
    fn leveled(&self) -> bool {
        self.inner.leveled()
    }

    #[getter]
    fn village(&self) -> bool {
        self.inner.village()
    }

    #[getter]
    fn scene(&self) -> String {
        self.inner.scene()
    }

    fn inventory(&self) -> Vec<String> {
        self.inner.inventory()
    }

    fn has_item(&self, item: &str) -> bool {
        self.inner.has_item(item)
    }

    fn has_flag(&self, name: &str) -> bool {
        self.inner.has_flag(name)
    }

    fn subtitle(&self) -> String {
        self.inner.subtitle()
    }

    /// Screen tap. Title / result starts or retries.
    /// Play hits 話す / 買う / 戦う / 持ち物.
    fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) {
        self.inner.pointer(x, y, width, height);
    }

    /// HUD JSON (`quads` + Japanese / ASCII `texts`) for `draw_world`.
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
