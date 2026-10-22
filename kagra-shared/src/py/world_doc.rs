//! `WorldDoc` Python binding.

use pyo3::prelude::*;

use crate::world_doc::WorldDoc;

use super::common::pyerr;

/// dump JSON の読み書き。世界の「データ」としての入口。
#[pyclass(name = "WorldDoc")]
pub struct PyWorldDoc {
    pub(crate) inner: WorldDoc,
}

#[pymethods]
impl PyWorldDoc {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: WorldDoc::from_json(json).map_err(pyerr)?,
        })
    }

    fn to_json(&self) -> PyResult<String> {
        self.inner.to_json().map_err(pyerr)
    }

    /// Player の位置 [x, y, z]（無ければ None）。
    fn player_position(&self) -> Option<[f32; 3]> {
        self.inner.player.as_ref().map(|w| w.position)
    }

    /// 指定 name の prop 一覧（id, position）。
    fn props_named(&self, name: &str) -> Vec<(String, [f32; 3])> {
        self.inner
            .props
            .iter()
            .filter(|p| p.name == name && p.enabled)
            .map(|p| (p.id.clone(), p.position))
            .collect()
    }
}
