//! `PhysicsWorld` Python binding (Rapier, optional feature).
//!
//! The Rust type in `crate::physics` is the surface Android will call later.
//! This module only converts dump JSON and event records.

use pyo3::prelude::*;

use crate::world_doc::{WorldDoc, WorldJoint};

use super::common::pyerr;

/// Rapier rigid bodies. Build from a dump, `step`, then `to_json` to write
/// poses back onto the same entities the renderer draws.
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

    fn velocity(&self, id: &str) -> Option<[f32; 3]> {
        self.inner.velocity(id)
    }

    fn angular_velocity(&self, id: &str) -> Option<[f32; 3]> {
        self.inner.angular_velocity(id)
    }

    /// Yaw, pitch, roll. Same convention as `WorldProp`.
    fn rotation_euler(&self, id: &str) -> Option<[f32; 3]> {
        self.inner.rotation_euler(id)
    }

    fn is_dynamic(&self, id: &str) -> bool {
        self.inner.is_dynamic(id)
    }

    fn is_kinematic(&self, id: &str) -> bool {
        self.inner.is_kinematic(id)
    }

    /// `static` / `dynamic` / `kinematic`, or None when the id is not simulated.
    fn body_type(&self, id: &str) -> Option<String> {
        self.inner.body_type(id).map(str::to_string)
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

    fn set_kinematic_target(&mut self, id: &str, p: [f32; 3]) -> bool {
        self.inner.set_kinematic_target(id, p)
    }

    fn apply_impulse(&mut self, id: &str, impulse: [f32; 3]) -> bool {
        self.inner.apply_impulse(id, impulse)
    }

    fn apply_force(&mut self, id: &str, force: [f32; 3]) -> bool {
        self.inner.apply_force(id, force)
    }

    fn apply_torque_impulse(&mut self, id: &str, torque: [f32; 3]) -> bool {
        self.inner.apply_torque_impulse(id, torque)
    }

    /// `fixed` / `revolute` / `spherical`. Anchors are body-local.
    /// A joint that sticks is also appended to the dump so `to_json` keeps it.
    #[allow(clippy::too_many_arguments)]
    fn add_joint(
        &mut self,
        kind: &str,
        body_a: &str,
        body_b: &str,
        anchor_a: [f32; 3],
        anchor_b: [f32; 3],
        axis: [f32; 3],
    ) -> bool {
        if !self
            .inner
            .add_joint(kind, body_a, body_b, anchor_a, anchor_b, axis)
        {
            return false;
        }
        self.doc.joints.push(WorldJoint {
            id: format!("joint:{}", self.doc.joints.len() + 1),
            kind: "joint".into(),
            name: kind.into(),
            body_a: body_a.into(),
            body_b: body_b.into(),
            anchor_a,
            anchor_b,
            axis,
        });
        true
    }

    /// Drain `collision` or `contact` events as JSON strings.
    fn take_events(&mut self, name: &str) -> Vec<String> {
        self.inner
            .take_events(name)
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect()
    }

    /// Closest hit. `direction` is normalized internally. `exclude` skips one body.
    /// Call after at least one `step` so the broad phase is current.
    #[pyo3(signature = (origin, direction, max_toi=100.0, exclude=None))]
    fn raycast(
        &self,
        origin: [f32; 3],
        direction: [f32; 3],
        max_toi: f32,
        exclude: Option<&str>,
    ) -> Option<(String, f32, [f32; 3], [f32; 3])> {
        let exclude = exclude.filter(|s| !s.is_empty());
        self.inner
            .raycast(origin, direction, max_toi, exclude)
            .map(|hit| (hit.id, hit.toi, hit.point, hit.normal))
    }
}
