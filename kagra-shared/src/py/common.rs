//! Shared PyO3 helpers for genre bindings.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

pub(crate) fn pyerr<E: std::fmt::Display>(e: E) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

pub(crate) fn phase_str(phase: crate::game::GamePhase) -> &'static str {
    match phase {
        crate::game::GamePhase::Title => "title",
        crate::game::GamePhase::Playing => "playing",
        crate::game::GamePhase::Complete => "complete",
    }
}
