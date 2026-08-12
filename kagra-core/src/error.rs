// src/error.rs
use std::sync::{Mutex, MutexGuard};

use thiserror::Error;
use pyo3::{PyErr, PyResult, exceptions::PyRuntimeError};

#[derive(Error, Debug)]
pub enum KaguraError {
    #[error("GPU error: {0}")]
    Gpu(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("VRM parsing error: {0}")]
    VrmParse(String),

    #[error("FBX loading error: {0}")]
    FbxLoad(String),

    #[error("Texture load failed: {0}")]
    Texture(String),

    #[error("Buffer overflow: needed {needed} but capacity {capacity}")]
    BufferOverflow { needed: u64, capacity: u64 },

    #[error("Mutex poisoned")]
    MutexPoisoned,

    #[error("Rig not found: {0}")]
    RigNotFound(u32),

    #[error("Other: {0}")]
    Other(String),
}

impl From<KaguraError> for PyErr {
    fn from(err: KaguraError) -> PyErr {
        PyRuntimeError::new_err(err.to_string())
    }
}

pub type KaguraResult<T> = Result<T, KaguraError>;

/// Mutex をロックし、poison 時は `KaguraError::MutexPoisoned` を返す。
pub fn lock_mutex<'a, T>(mutex: &'a Mutex<T>) -> KaguraResult<MutexGuard<'a, T>> {
    mutex.lock().map_err(|_| KaguraError::MutexPoisoned)
}

/// Python API 境界用。poison 時は `PyRuntimeError` になる。
pub fn lock_py<'a, T>(mutex: &'a Mutex<T>) -> PyResult<MutexGuard<'a, T>> {
    lock_mutex(mutex).map_err(Into::into)
}

/// イベントループ等、`Result` を返せない経路用。
/// poison してもパニックせず、内側のガードを回収する。
pub fn lock_recover<'a, T>(mutex: &'a Mutex<T>) -> MutexGuard<'a, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        log::error!("Mutex poisoned; recovering with into_inner()");
        poisoned.into_inner()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn lock_mutex_ok() {
        let m = Mutex::new(7);
        assert_eq!(*lock_mutex(&m).unwrap(), 7);
    }

    #[test]
    fn lock_mutex_reports_poison() {
        let m = Arc::new(Mutex::new(0));
        let m2 = Arc::clone(&m);
        let _ = thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("poison");
        })
        .join();
        let result = lock_mutex(&m);
        match result {
            Err(KaguraError::MutexPoisoned) => {}
            other => panic!("expected MutexPoisoned, got {:?}", other),
        }
    }

    #[test]
    fn lock_recover_after_poison() {
        let m = Arc::new(Mutex::new(42));
        let m2 = Arc::clone(&m);
        let _ = thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("poison");
        })
        .join();
        assert_eq!(*lock_recover(&m), 42);
    }
}
