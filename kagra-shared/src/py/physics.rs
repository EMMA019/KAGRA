//! `PhysicsWorld` Python binding (Rapier, optional feature).

use pyo3::prelude::*;

use crate::world_doc::WorldDoc;

use super::common::pyerr;

/// Rapier 剛体物理（Phase 1）。dump JSON からワールドを作り、step で進め、
/// to_json で位置を書き戻す。
#[cfg(feature = "physics")]
#[pyclass(name = "PhysicsWorld")]
pub struct PyPhysicsWorld {
    pub(crate) inner: crate::physics::PhysicsWorld,
    doc: WorldDoc,
}

#[cfg(feature = "physics")]
#[pymethods]
impl PyPhysicsWorld {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let doc = WorldDoc::from_json(json).map_err(pyerr)?;
        Ok(Self {
            inner: crate::physics::PhysicsWorld::from_doc(&doc),
            doc,
        })
    }

    fn step(&mut self, dt: f32) {
        self.inner.step(dt);
    }

    fn to_json(&self) -> PyResult<String> {
        let mut doc = self.doc.clone();
        self.inner.sync(&mut doc);
        doc.to_json().map_err(pyerr)
    }

    fn dump(&self) -> PyResult<String> {
        self.doc.to_json().map_err(pyerr)
    }

    fn position(&self, id: &str) -> Option<[f32; 3]> {
        self.inner.position(id)
    }

    fn is_dynamic(&self, id: &str) -> bool {
        self.inner.is_dynamic(id)
    }

    fn is_kinematic(&self, id: &str) -> bool {
        self.inner.is_kinematic(id)
    }

    fn sync_walkers(&mut self, json: &str) -> PyResult<()> {
        let doc = WorldDoc::from_json(json).map_err(pyerr)?;
        self.inner.sync_walkers(&doc);
        Ok(())
    }

    fn set_walker_position(&mut self, id: &str, p: [f32; 3]) -> bool {
        self.inner.set_walker_position(id, p)
    }

    fn set_velocity(&mut self, id: &str, v: [f32; 3]) -> bool {
        self.inner.set_velocity(id, v)
    }

    fn set_position(&mut self, id: &str, p: [f32; 3]) -> bool {
        self.inner.set_position(id, p)
    }
}
