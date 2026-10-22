//! Rapier rigid bodies (Phase 2).
//!
//! `WorldDoc` props become rigid bodies. `step` integrates them and `sync`
//! writes position + yaw/pitch/roll back so the same dump renders (bundled
//! `.glb` included). Game rules stay in Python. The Rust type is what Android
//! will call later — no `WorldPlay` genre.
//!
//! - `body`: `static` / `dynamic` / `kinematic`. Omitted → `is_static`.
//! - `collider`: `box` / `sphere` / `capsule` / `cylinder` / `cone` / `none`.
//!   Omitted → inferred from `model`. Unknown models are scenery only.
//! - `joints[]`: `fixed` / `revolute` / `spherical` (anchors are body-local).
//! - Walkers stay kinematic capsules. The game owns their positions
//!   (`sync_walkers`). `sync` does not overwrite them.
//! - `take_events("collision")` is start/stop. `take_events("contact")` is the
//!   first frame a contact force crosses the threshold.
//!
//! `enhanced-determinism`: same platform, same input → same result.
//!
//! Python: `kagra_shared.PhysicsWorld` (`py/physics.rs`) and `kagra.rigid`.

use std::collections::HashMap;

use glam::{EulerRot, Vec3};
use rapier3d::prelude::*;

use crate::world_doc::{prop_rotation, WorldDoc, WorldEvent, WorldJoint, WorldProp};

/// Fixed physics step (seconds). Rapier is deterministic at a fixed dt.
pub const PHYSICS_DT: f32 = 1.0 / 120.0;

const MAX_EVENTS: usize = 256;

/// Ray hit. `id` is a prop / walker id, or `"ground"` for the heightfield.
#[derive(Clone, Debug, PartialEq)]
pub struct RayHit {
    pub id: String,
    pub toi: f32,
    pub point: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BodyKind {
    Static,
    Dynamic,
    Kinematic,
}

/// Rapier world + the ids that map back onto a `WorldDoc`.
pub struct PhysicsWorld {
    inner: rapier3d::prelude::PhysicsWorld,
    /// Every simulated prop and walker.
    bodies: HashMap<String, RigidBodyHandle>,
    /// Dynamic props only.
    dynamic: HashMap<String, RigidBodyHandle>,
    /// Walker capsules (game-owned).
    kinematic: HashMap<String, RigidBodyHandle>,
    /// Kinematic props (game-owned, written back by `sync`).
    kinematic_props: HashMap<String, RigidBodyHandle>,
    /// prop id → collider.
    colliders: HashMap<String, ColliderHandle>,
    /// collider → prop id (`"ground"` for the heightfield).
    collider_ids: HashMap<ColliderHandle, String>,
    events: Vec<WorldEvent>,
    event_seq: u32,
}

fn snap(a: f32) -> f32 {
    if a.abs() < 1e-5 {
        0.0
    } else {
        a
    }
}

fn unit_axis(axis: [f32; 3]) -> Vec3 {
    let v = Vec3::from_array(axis);
    if v.length_squared() < 1e-8 {
        Vec3::Y
    } else {
        v.normalize()
    }
}

fn body_kind(p: &WorldProp) -> BodyKind {
    match p
        .body
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "dynamic" => BodyKind::Dynamic,
        "kinematic" => BodyKind::Kinematic,
        "static" | "fixed" => BodyKind::Static,
        _ => {
            if p.is_static {
                BodyKind::Static
            } else {
                BodyKind::Dynamic
            }
        }
    }
}

/// Collider name, or None when the prop is scenery.
fn shape_name(p: &WorldProp) -> Option<String> {
    if let Some(c) = &p.collider {
        let s = c.trim().to_ascii_lowercase();
        if s.is_empty() || s == "none" {
            return None;
        }
        return Some(s);
    }
    let m = p.model.trim().to_ascii_lowercase();
    let stem = std::path::Path::new(&m)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(m.as_str())
        .to_ascii_lowercase();
    match stem.as_str() {
        "" | "box" | "crate" | "cube" | "chest" => Some("box".into()),
        "sphere" | "ball" | "rock" => Some("sphere".into()),
        "capsule" => Some("capsule".into()),
        "cylinder" => Some("cylinder".into()),
        "cone" => Some("cone".into()),
        _ => None,
    }
}

fn collider_for(p: &WorldProp) -> Option<ColliderBuilder> {
    let shape = shape_name(p)?;
    let s = p.scale;
    let hx = (s[0].abs() * 0.5).max(0.02);
    let hy = (s[1].abs() * 0.5).max(0.02);
    let hz = (s[2].abs() * 0.5).max(0.02);
    let mut b = match shape.as_str() {
        "box" | "cuboid" | "cube" => ColliderBuilder::cuboid(hx, hy, hz),
        "sphere" | "ball" => {
            let r = hx.max(hz).max(0.05);
            ColliderBuilder::ball(r)
        }
        "capsule" => {
            let r = hx.min(hz).max(0.05);
            let half = (hy - r).max(0.02);
            ColliderBuilder::capsule_y(half, r)
        }
        "cylinder" => {
            let r = hx.min(hz).max(0.05);
            ColliderBuilder::cylinder(hy.max(0.05), r)
        }
        "cone" => {
            let r = hx.min(hz).max(0.05);
            ColliderBuilder::cone(hy.max(0.05), r)
        }
        _ => return None,
    };
    b = b
        .friction(p.friction)
        .restitution(p.restitution)
        .active_events(ActiveEvents::COLLISION_EVENTS | ActiveEvents::CONTACT_FORCE_EVENTS)
        .contact_force_event_threshold(1.0);
    if p.sensor {
        b = b.sensor(true);
    }
    if p.mass > 0.0 {
        b = b.density(0.0);
    }
    Some(b)
}

fn write_pose(prop: &mut WorldProp, rb: &RigidBody) {
    prop.position = rb.translation().to_array();
    let (yaw, pitch_x, roll) = rb.rotation().to_euler(EulerRot::YXZ);
    prop.yaw = snap(yaw);
    prop.pitch = snap(-pitch_x);
    prop.roll = snap(roll);
}

fn joint_kind_name(j: &WorldJoint) -> String {
    let name = j.name.trim().to_ascii_lowercase();
    if matches!(
        name.as_str(),
        "fixed" | "weld" | "revolute" | "hinge" | "spherical" | "ball"
    ) {
        return name;
    }
    let kind = j.kind.trim().to_ascii_lowercase();
    if matches!(
        kind.as_str(),
        "fixed" | "weld" | "revolute" | "hinge" | "spherical" | "ball"
    ) {
        return kind;
    }
    if !name.is_empty() && name != "joint" {
        return name;
    }
    "fixed".into()
}

impl PhysicsWorld {
    /// Build bodies, the ground heightfield, and joints from a dump.
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let mut this = Self {
            inner: rapier3d::prelude::PhysicsWorld::default(),
            bodies: HashMap::new(),
            dynamic: HashMap::new(),
            kinematic: HashMap::new(),
            kinematic_props: HashMap::new(),
            colliders: HashMap::new(),
            collider_ids: HashMap::new(),
            events: Vec::new(),
            event_seq: 0,
        };
        // A struct literal (Rust Default) can leave gravity at 0.
        // Match serde's default (9.8).
        let g = if doc.gravity > 0.0 { doc.gravity } else { 9.8 };
        this.inner.gravity = Vec3::new(0.0, -g, 0.0);
        this.build_ground(doc);
        for p in &doc.props {
            this.insert_prop(p);
        }
        for w in doc.player.iter().chain(doc.walkers.iter()) {
            this.insert_walker(w.position, &w.id);
        }
        for j in &doc.joints {
            let kind = joint_kind_name(j);
            let _ = this.add_joint(&kind, &j.body_a, &j.body_b, j.anchor_a, j.anchor_b, j.axis);
        }
        this
    }

    fn insert_prop(&mut self, p: &WorldProp) {
        if !p.enabled {
            return;
        }
        let Some(collider) = collider_for(p) else {
            return;
        };
        let kind = body_kind(p);
        let mut builder = match kind {
            BodyKind::Static => RigidBodyBuilder::fixed(),
            BodyKind::Dynamic => RigidBodyBuilder::dynamic().can_sleep(true),
            BodyKind::Kinematic => RigidBodyBuilder::kinematic_position_based(),
        };
        builder = builder.translation(Vec3::from_array(p.position));
        if kind == BodyKind::Dynamic && p.mass > 0.0 {
            builder = builder.additional_mass(p.mass);
        }
        let body = self.inner.insert_body(builder);
        let rot = prop_rotation(p);
        if let Some(rb) = self.inner.bodies.get_mut(body) {
            rb.set_rotation(rot, true);
            if kind == BodyKind::Kinematic {
                rb.set_next_kinematic_translation(Vec3::from_array(p.position));
                rb.set_next_kinematic_rotation(rot);
            }
        }
        let ch = self.inner.insert_collider(collider, Some(body));
        self.colliders.insert(p.id.clone(), ch);
        self.collider_ids.insert(ch, p.id.clone());
        self.bodies.insert(p.id.clone(), body);
        match kind {
            BodyKind::Dynamic => {
                self.dynamic.insert(p.id.clone(), body);
            }
            BodyKind::Kinematic => {
                self.kinematic_props.insert(p.id.clone(), body);
            }
            BodyKind::Static => {}
        }
    }

    fn insert_walker(&mut self, position: [f32; 3], id: &str) {
        let body = self
            .inner
            .insert_body(RigidBodyBuilder::kinematic_position_based().translation(position.into()));
        if let Some(rb) = self.inner.bodies.get_mut(body) {
            rb.set_next_kinematic_translation(Vec3::from_array(position));
        }
        let collider = ColliderBuilder::capsule_y(0.7, 0.28)
            .friction(0.0)
            .density(1.0)
            .active_events(ActiveEvents::COLLISION_EVENTS | ActiveEvents::CONTACT_FORCE_EVENTS)
            .contact_force_event_threshold(1.0);
        let ch = self.inner.insert_collider(collider, Some(body));
        self.collider_ids.insert(ch, id.to_string());
        self.bodies.insert(id.to_string(), body);
        self.kinematic.insert(id.to_string(), body);
    }

    fn build_ground(&mut self, doc: &WorldDoc) {
        let half = doc.half.max(4.0);
        const N: usize = 33;
        let step = (half * 2.0) / (N as f32 - 1.0);
        let mut heights = vec![0.0f32; N * N];
        for iz in 0..N {
            for ix in 0..N {
                let x = -half + ix as f32 * step;
                let z = -half + iz as f32 * step;
                heights[iz * N + ix] = doc.height_at(x, z);
            }
        }
        let collider = ColliderBuilder::heightfield(
            Array2::new(N, N, heights),
            Vec3::new(half * 2.0, 1.0, half * 2.0),
        )
        .friction(0.9)
        .active_events(ActiveEvents::COLLISION_EVENTS);
        let body = self.inner.insert_body(RigidBodyBuilder::fixed());
        let ch = self.inner.insert_collider(collider, Some(body));
        self.collider_ids.insert(ch, "ground".into());
    }

    /// Advance `dt` seconds, split into fixed steps.
    ///
    /// Kinematic bodies move toward the translation set by `sync_walkers` /
    /// `set_position` / `set_kinematic_target`.
    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        let mut remaining = dt;
        while remaining > 0.0 {
            let h = remaining.min(PHYSICS_DT);
            self.inner.integration_parameters.dt = h;
            let (col_tx, col_rx) = std::sync::mpsc::channel();
            let (force_tx, force_rx) = std::sync::mpsc::channel();
            let handler = ChannelEventCollector::new(col_tx, force_tx);
            self.inner.step_with_events(&(), &handler);
            while let Ok(event) = col_rx.try_recv() {
                self.push_collision(event);
            }
            while let Ok(event) = force_rx.try_recv() {
                if event.started {
                    self.push_contact(&event);
                }
            }
            remaining -= h;
        }
    }

    fn collider_name(&self, h: ColliderHandle) -> String {
        self.collider_ids
            .get(&h)
            .cloned()
            .unwrap_or_else(|| "unknown".into())
    }

    fn push_named(&mut self, name: &str, data: serde_json::Value) {
        self.event_seq = self.event_seq.saturating_add(1);
        self.events.push(WorldEvent {
            id: format!("event:{}", self.event_seq),
            kind: "event".into(),
            name: name.into(),
            count: 1,
            data: Some(data),
        });
        if self.events.len() > MAX_EVENTS {
            let drop_n = self.events.len() - MAX_EVENTS;
            self.events.drain(0..drop_n);
        }
    }

    fn push_collision(&mut self, event: CollisionEvent) {
        let (h1, h2, state, sensor) = match event {
            CollisionEvent::Started(h1, h2, flags) => (
                h1,
                h2,
                "started",
                flags.contains(CollisionEventFlags::SENSOR),
            ),
            CollisionEvent::Stopped(h1, h2, flags) => (
                h1,
                h2,
                "stopped",
                flags.contains(CollisionEventFlags::SENSOR),
            ),
        };
        self.push_named(
            "collision",
            serde_json::json!({
                "a": self.collider_name(h1),
                "b": self.collider_name(h2),
                "state": state,
                "sensor": sensor,
            }),
        );
    }

    fn push_contact(&mut self, event: &ContactForceEvent) {
        self.push_named(
            "contact",
            serde_json::json!({
                "a": self.collider_name(event.collider1),
                "b": self.collider_name(event.collider2),
                "force": event.total_force_magnitude,
                "started": event.started,
            }),
        );
    }

    /// Drain events named `collision` or `contact`.
    pub fn take_events(&mut self, name: &str) -> Vec<WorldEvent> {
        let mut out = Vec::new();
        self.events.retain(|e| {
            if e.name == name {
                out.push(e.clone());
                false
            } else {
                true
            }
        });
        out
    }

    /// Push walker positions into kinematic bodies. Call before `step`.
    pub fn sync_walkers(&mut self, doc: &WorldDoc) {
        for w in doc.player.iter().chain(doc.walkers.iter()) {
            let _ = self.set_walker_position(&w.id, w.position);
        }
    }

    /// One walker's game-owned position (next kinematic target).
    pub fn set_walker_position(&mut self, id: &str, p: [f32; 3]) -> bool {
        let Some(&body) = self.kinematic.get(id) else {
            return false;
        };
        if let Some(rb) = self.inner.bodies.get_mut(body) {
            rb.set_next_kinematic_translation(Vec3::from_array(p));
            return true;
        }
        false
    }

    /// Write dynamic and kinematic **props** back. Walkers stay game-owned.
    pub fn sync(&self, doc: &mut WorldDoc) {
        for p in &mut doc.props {
            let handle = self
                .dynamic
                .get(&p.id)
                .or_else(|| self.kinematic_props.get(&p.id));
            if let Some(&body) = handle {
                if let Some(rb) = self.inner.bodies.get(body) {
                    write_pose(p, rb);
                }
            }
        }
        for ev in &self.events {
            if !doc.events.iter().any(|e| e.id == ev.id) {
                doc.events.push(ev.clone());
            }
        }
    }

    /// `fixed` / `revolute` / `spherical` (aliases weld / hinge / ball).
    /// Anchors are in each body's local space. Returns false if either body
    /// is missing or the kind is unknown.
    pub fn add_joint(
        &mut self,
        kind: &str,
        body_a: &str,
        body_b: &str,
        anchor_a: [f32; 3],
        anchor_b: [f32; 3],
        axis: [f32; 3],
    ) -> bool {
        let (Some(&ba), Some(&bb)) = (self.bodies.get(body_a), self.bodies.get(body_b)) else {
            return false;
        };
        let a1 = Vec3::from_array(anchor_a);
        let a2 = Vec3::from_array(anchor_b);
        let axis = unit_axis(axis);
        match kind.trim().to_ascii_lowercase().as_str() {
            "fixed" | "weld" => {
                self.inner.insert_impulse_joint(
                    ba,
                    bb,
                    FixedJointBuilder::new()
                        .local_anchor1(a1)
                        .local_anchor2(a2)
                        .contacts_enabled(false),
                );
            }
            "revolute" | "hinge" => {
                self.inner.insert_impulse_joint(
                    ba,
                    bb,
                    RevoluteJointBuilder::new(axis)
                        .local_anchor1(a1)
                        .local_anchor2(a2)
                        .contacts_enabled(false),
                );
            }
            "spherical" | "ball" => {
                self.inner.insert_impulse_joint(
                    ba,
                    bb,
                    SphericalJointBuilder::new()
                        .local_anchor1(a1)
                        .local_anchor2(a2)
                        .contacts_enabled(false),
                );
            }
            _ => return false,
        }
        true
    }

    pub fn set_velocity(&mut self, id: &str, v: [f32; 3]) -> bool {
        let Some(&body) = self.dynamic.get(id) else {
            return false;
        };
        if let Some(rb) = self.inner.bodies.get_mut(body) {
            rb.set_linvel(Vec3::from_array(v), true);
            return true;
        }
        false
    }

    /// Instant velocity change on a dynamic body (kg·m/s).
    pub fn apply_impulse(&mut self, id: &str, impulse: [f32; 3]) -> bool {
        let Some(&body) = self.dynamic.get(id) else {
            return false;
        };
        if let Some(rb) = self.inner.bodies.get_mut(body) {
            rb.apply_impulse(Vec3::from_array(impulse), true);
            return true;
        }
        false
    }

    /// Force (newtons) accumulated until the next step, then cleared by Rapier.
    pub fn apply_force(&mut self, id: &str, force: [f32; 3]) -> bool {
        let Some(&body) = self.dynamic.get(id) else {
            return false;
        };
        if let Some(rb) = self.inner.bodies.get_mut(body) {
            rb.add_force(Vec3::from_array(force), true);
            return true;
        }
        false
    }

    /// Instant angular impulse on a dynamic body.
    pub fn apply_torque_impulse(&mut self, id: &str, torque: [f32; 3]) -> bool {
        let Some(&body) = self.dynamic.get(id) else {
            return false;
        };
        if let Some(rb) = self.inner.bodies.get_mut(body) {
            rb.apply_torque_impulse(Vec3::from_array(torque), true);
            return true;
        }
        false
    }

    /// Dynamic teleport, or a kinematic prop's next pose. Walkers use
    /// [`set_walker_position`].
    pub fn set_position(&mut self, id: &str, p: [f32; 3]) -> bool {
        if let Some(&body) = self.dynamic.get(id) {
            if let Some(rb) = self.inner.bodies.get_mut(body) {
                rb.set_translation(Vec3::from_array(p), true);
                rb.set_linvel(Vec3::ZERO, true);
                return true;
            }
            return false;
        }
        if let Some(&body) = self.kinematic_props.get(id) {
            if let Some(rb) = self.inner.bodies.get_mut(body) {
                let v = Vec3::from_array(p);
                rb.set_translation(v, true);
                rb.set_next_kinematic_translation(v);
                return true;
            }
        }
        false
    }

    /// Kinematic prop or walker. Sets the next kinematic translation.
    /// Props also snap immediately so `position` matches before the next step.
    pub fn set_kinematic_target(&mut self, id: &str, p: [f32; 3]) -> bool {
        if self.kinematic_props.contains_key(id) {
            return self.set_position(id, p);
        }
        if !self.set_walker_position(id, p) {
            return false;
        }
        if let Some(&body) = self.kinematic.get(id) {
            if let Some(rb) = self.inner.bodies.get_mut(body) {
                rb.set_translation(Vec3::from_array(p), true);
            }
        }
        true
    }

    pub fn position(&self, id: &str) -> Option<[f32; 3]> {
        let body = self
            .dynamic
            .get(id)
            .or_else(|| self.kinematic_props.get(id))
            .or_else(|| self.kinematic.get(id))?;
        let rb = self.inner.bodies.get(*body)?;
        Some(rb.translation().to_array())
    }

    pub fn velocity(&self, id: &str) -> Option<[f32; 3]> {
        let body = self
            .dynamic
            .get(id)
            .or_else(|| self.kinematic_props.get(id))
            .or_else(|| self.kinematic.get(id))?;
        Some(self.inner.bodies.get(*body)?.linvel().to_array())
    }

    pub fn angular_velocity(&self, id: &str) -> Option<[f32; 3]> {
        let body = self
            .dynamic
            .get(id)
            .or_else(|| self.kinematic_props.get(id))
            .or_else(|| self.kinematic.get(id))?;
        Some(self.inner.bodies.get(*body)?.angvel().to_array())
    }

    /// Yaw, pitch, roll in the same convention as `WorldProp` (YXZ, pitch negated).
    pub fn rotation_euler(&self, id: &str) -> Option<[f32; 3]> {
        let &body = self.bodies.get(id)?;
        let rb = self.inner.bodies.get(body)?;
        let (yaw, pitch_x, roll) = rb.rotation().to_euler(EulerRot::YXZ);
        Some([snap(yaw), snap(-pitch_x), snap(roll)])
    }

    pub fn is_dynamic(&self, id: &str) -> bool {
        self.dynamic.contains_key(id)
    }

    pub fn is_kinematic(&self, id: &str) -> bool {
        self.kinematic.contains_key(id) || self.kinematic_props.contains_key(id)
    }

    pub fn is_body(&self, id: &str) -> bool {
        self.is_dynamic(id) || self.is_kinematic(id)
    }

    /// `static` / `dynamic` / `kinematic`, or None if the id is not simulated.
    pub fn body_type(&self, id: &str) -> Option<&'static str> {
        if self.dynamic.contains_key(id) {
            Some("dynamic")
        } else if self.is_kinematic(id) {
            Some("kinematic")
        } else if self.bodies.contains_key(id) {
            Some("static")
        } else {
            None
        }
    }

    /// Closest hit along `dir`. `dir` is normalized internally (toi is meters).
    /// `exclude` skips one body (the caster). The broad phase updates on `step`,
    /// so call this after at least one step.
    pub fn raycast(
        &self,
        origin: [f32; 3],
        dir: [f32; 3],
        max_toi: f32,
        exclude: Option<&str>,
    ) -> Option<RayHit> {
        let mut direction = Vec3::from_array(dir);
        let len = direction.length();
        if len < 1e-8 || max_toi <= 0.0 {
            return None;
        }
        direction /= len;
        let ray = Ray::new(Vec3::from_array(origin), direction);
        let mut filter = QueryFilter::default();
        if let Some(id) = exclude {
            filter.exclude_rigid_body = self.bodies.get(id).copied();
        }
        let (handle, hit) = self
            .inner
            .cast_ray_and_get_normal(&ray, max_toi, true, filter)?;
        let toi = hit.time_of_impact;
        let point = ray.point_at(toi);
        Some(RayHit {
            id: self.collider_name(handle),
            toi,
            point: point.to_array(),
            normal: hit.normal.to_array(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_with_box(y: f32) -> WorldDoc {
        WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![WorldProp {
                id: "prop:box".into(),
                kind: "prop".into(),
                name: "box".into(),
                model: "box".into(),
                position: [0.0, y, 0.0],
                scale: [1.0, 1.0, 1.0],
                enabled: true,
                is_static: false,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn prop(
        id: &str,
        model: &str,
        position: [f32; 3],
        scale: [f32; 3],
        is_static: bool,
    ) -> WorldProp {
        WorldProp {
            id: id.into(),
            kind: "prop".into(),
            name: id.into(),
            model: model.into(),
            position,
            scale,
            enabled: true,
            is_static,
            ..Default::default()
        }
    }

    #[test]
    fn dynamic_box_falls_to_ground() {
        let doc = doc_with_box(5.0);
        let mut world = PhysicsWorld::from_doc(&doc);
        for _ in 0..240 {
            world.step(1.0 / 60.0);
        }
        let y = world.position("prop:box").unwrap()[1];
        assert!(
            y > 0.4 && y < 1.6,
            "箱は床に落ちて止まる（半辺 0.5）, y={y}"
        );
        assert!(!world.is_dynamic("prop:box") || world.position("prop:box").is_some());
    }

    #[test]
    fn static_box_does_not_fall() {
        let mut doc = doc_with_box(3.0);
        doc.props[0].is_static = true;
        let mut world = PhysicsWorld::from_doc(&doc);
        for _ in 0..120 {
            world.step(1.0 / 60.0);
        }
        assert!(
            !world.is_dynamic("prop:box"),
            "静的 prop は動的剛体ではない"
        );
        assert_eq!(world.body_type("prop:box"), Some("static"));
        world.sync(&mut doc);
        assert_eq!(
            doc.props[0].position,
            [0.0, 3.0, 0.0],
            "静的 prop は動かない"
        );
    }

    #[test]
    fn two_boxes_stack() {
        let mut doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![
                WorldProp {
                    id: "prop:a".into(),
                    kind: "prop".into(),
                    name: "box".into(),
                    model: "box".into(),
                    position: [0.0, 0.5, 0.0],
                    scale: [1.0, 1.0, 1.0],
                    enabled: true,
                    is_static: false,
                    ..Default::default()
                },
                WorldProp {
                    id: "prop:b".into(),
                    kind: "prop".into(),
                    name: "box".into(),
                    model: "box".into(),
                    position: [0.0, 3.0, 0.0],
                    scale: [1.0, 1.0, 1.0],
                    enabled: true,
                    is_static: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        for _ in 0..600 {
            world.step(1.0 / 60.0);
        }
        let ya = world.position("prop:a").unwrap()[1];
        let yb = world.position("prop:b").unwrap()[1];
        assert!(ya > 0.4 && ya < 1.0, "下の箱は床に着く, ya={ya}");
        assert!(
            yb > ya + 0.7 && yb < ya + 1.6,
            "上の箱は下の箱に積もる, yb={yb} ya={ya}"
        );
        world.sync(&mut doc);
        assert_eq!(doc.props[0].position[1], ya);
    }

    #[test]
    fn deterministic_same_input_same_result() {
        let doc = doc_with_box(4.0);
        let mut a = PhysicsWorld::from_doc(&doc);
        let mut b = PhysicsWorld::from_doc(&doc);
        for _ in 0..180 {
            a.step(1.0 / 60.0);
            b.step(1.0 / 60.0);
        }
        assert_eq!(a.position("prop:box"), b.position("prop:box"));
    }

    #[test]
    fn set_velocity_throws_box() {
        let doc = doc_with_box(2.0);
        let mut world = PhysicsWorld::from_doc(&doc);
        assert!(world.set_velocity("prop:box", [3.0, 0.0, 0.0]));
        assert!(!world.set_velocity("prop:nope", [0.0; 3]));
        let x0 = world.position("prop:box").unwrap()[0];
        for _ in 0..30 {
            world.step(1.0 / 60.0);
        }
        let x1 = world.position("prop:box").unwrap()[0];
        assert!(x1 > x0 + 0.5, "横に吹き飛ぶ, x0={x0} x1={x1}");
    }

    #[test]
    fn impulse_and_force_move_box() {
        let doc = doc_with_box(1.0);
        let mut world = PhysicsWorld::from_doc(&doc);
        assert!(world.apply_impulse("prop:box", [1.5, 0.0, 0.0]));
        assert!(!world.apply_impulse("prop:nope", [1.0, 0.0, 0.0]));
        let x0 = world.position("prop:box").unwrap()[0];
        world.step(1.0 / 60.0);
        let x1 = world.position("prop:box").unwrap()[0];
        assert!(x1 > x0 + 0.01, "impulse moves, x0={x0} x1={x1}");
        assert!(world.apply_force("prop:box", [40.0, 0.0, 0.0]));
        let x2 = world.position("prop:box").unwrap()[0];
        for _ in 0..20 {
            world.apply_force("prop:box", [40.0, 0.0, 0.0]);
            world.step(1.0 / 60.0);
        }
        let x3 = world.position("prop:box").unwrap()[0];
        assert!(x3 > x2 + 0.05, "force accelerates, x2={x2} x3={x3}");
        assert!(world.apply_torque_impulse("prop:box", [0.0, 0.4, 0.0]));
    }

    #[test]
    fn walker_is_kinematic_and_keeps_game_position() {
        let mut doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            player: Some(crate::world_doc::WorldWalker {
                id: "walker:player".into(),
                kind: "walker".into(),
                name: "player".into(),
                position: [0.0, 3.0, 0.0],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        assert!(
            world.is_kinematic("walker:player"),
            "歩行者はキネマティック"
        );
        for _ in 0..300 {
            world.sync_walkers(&doc);
            world.step(1.0 / 60.0);
        }
        world.sync(&mut doc);
        let p = doc.player.as_ref().unwrap();
        assert_eq!(
            p.position,
            [0.0, 3.0, 0.0],
            "sync は歩行者位置を上書きしない"
        );
    }

    #[test]
    fn kinematic_walker_pushes_dynamic_box() {
        let mut doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![WorldProp {
                id: "prop:box".into(),
                kind: "prop".into(),
                name: "box".into(),
                model: "box".into(),
                position: [2.0, 0.6, 0.0],
                scale: [1.0, 1.0, 1.0],
                enabled: true,
                is_static: false,
                ..Default::default()
            }],
            player: Some(crate::world_doc::WorldWalker {
                id: "walker:player".into(),
                kind: "walker".into(),
                name: "player".into(),
                position: [0.0, 1.0, 0.0],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        let mut px = 0.0f32;
        for _ in 0..240 {
            px += 0.02;
            doc.player.as_mut().unwrap().position[0] = px;
            world.sync_walkers(&doc);
            world.step(1.0 / 60.0);
        }
        let box_x = world.position("prop:box").unwrap()[0];
        assert!(box_x > 2.2, "歩行者が箱を押す, box_x={box_x}");
    }

    #[test]
    fn kinematic_prop_does_not_fall_and_can_be_moved() {
        let mut ram = prop("prop:ram", "box", [0.0, 2.0, 0.0], [0.5, 0.5, 0.5], true);
        ram.body = Some("kinematic".into());
        let doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![ram],
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        assert_eq!(world.body_type("prop:ram"), Some("kinematic"));
        assert!(world.is_kinematic("prop:ram"));
        assert!(!world.is_dynamic("prop:ram"));
        for _ in 0..90 {
            world.step(1.0 / 60.0);
        }
        let y = world.position("prop:ram").unwrap()[1];
        assert!(
            (y - 2.0).abs() < 0.05,
            "kinematic prop ignores gravity, y={y}"
        );
        assert!(world.set_position("prop:ram", [1.5, 0.4, -0.2]));
        let p = world.position("prop:ram").unwrap();
        assert!(
            (p[0] - 1.5).abs() < 1e-3,
            "set_position snaps a kinematic prop, p={p:?}"
        );
    }

    #[test]
    fn sphere_and_capsule_colliders_work() {
        let doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![
                WorldProp {
                    id: "prop:ball".into(),
                    kind: "prop".into(),
                    name: "ball".into(),
                    model: "sphere".into(),
                    position: [0.0, 3.0, 0.0],
                    scale: [1.0, 1.0, 1.0],
                    enabled: true,
                    is_static: false,
                    ..Default::default()
                },
                WorldProp {
                    id: "prop:pill".into(),
                    kind: "prop".into(),
                    name: "pill".into(),
                    model: "capsule".into(),
                    position: [2.0, 4.0, 0.0],
                    scale: [0.6, 1.8, 0.6],
                    enabled: true,
                    is_static: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        for _ in 0..360 {
            world.step(1.0 / 60.0);
        }
        let by = world.position("prop:ball").unwrap()[1];
        assert!(by > 0.4 && by < 0.9, "球は床に落ちる, by={by}");
        let py = world.position("prop:pill").unwrap()[1];
        assert!(py > 0.2 && py < 1.1, "カプセルは床に落ちて止まる, py={py}");
    }

    #[test]
    fn cylinder_and_cone_rest_on_the_floor() {
        let doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![
                prop(
                    "prop:cyl",
                    "cylinder",
                    [-1.5, 3.0, 0.0],
                    [0.8, 1.2, 0.8],
                    false,
                ),
                prop("prop:cone", "cone", [1.5, 3.0, 0.0], [0.8, 1.2, 0.8], false),
            ],
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        assert!(world.is_dynamic("prop:cyl"));
        assert!(world.is_dynamic("prop:cone"));
        for _ in 0..360 {
            world.step(1.0 / 60.0);
        }
        let cy = world.position("prop:cyl").unwrap()[1];
        let oy = world.position("prop:cone").unwrap()[1];
        assert!(cy > 0.2 && cy < 1.2, "cylinder rests, cy={cy}");
        assert!(oy > 0.15 && oy < 1.2, "cone rests, oy={oy}");
    }

    #[test]
    fn collision_and_contact_events_fire_on_landing() {
        let doc = doc_with_box(4.0);
        let mut world = PhysicsWorld::from_doc(&doc);
        for _ in 0..240 {
            world.step(1.0 / 60.0);
        }
        let collisions = world.take_events("collision");
        assert!(
            collisions.iter().any(|e| {
                let data = e.data.as_ref().and_then(|d| d.as_object()).unwrap();
                data.get("state").and_then(|s| s.as_str()) == Some("started")
                    && (data.get("a").and_then(|s| s.as_str()) == Some("prop:box")
                        || data.get("b").and_then(|s| s.as_str()) == Some("prop:box"))
            }),
            "landing emits a collision start, n={}",
            collisions.len()
        );
        assert!(world.take_events("collision").is_empty());
        let contacts = world.take_events("contact");
        assert!(
            contacts.iter().any(|e| {
                let data = e.data.as_ref().and_then(|d| d.as_object()).unwrap();
                data.get("a").and_then(|s| s.as_str()) == Some("prop:box")
                    || data.get("b").and_then(|s| s.as_str()) == Some("prop:box")
            }),
            "landing emits a contact, n={}",
            contacts.len()
        );
    }

    #[test]
    fn sensor_reports_overlap_without_blocking() {
        let mut sensor = prop("prop:zone", "box", [0.0, 2.0, 0.0], [2.0, 0.4, 2.0], true);
        sensor.sensor = true;
        let doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![
                prop("prop:box", "box", [0.0, 4.0, 0.0], [0.6, 0.6, 0.6], false),
                sensor,
            ],
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        let mut saw = false;
        for _ in 0..180 {
            world.step(1.0 / 60.0);
            for e in world.take_events("collision") {
                let data = e.data.as_ref().and_then(|d| d.as_object()).unwrap();
                let a = data.get("a").and_then(|s| s.as_str()).unwrap_or("");
                let b = data.get("b").and_then(|s| s.as_str()).unwrap_or("");
                let sensor_flag = data
                    .get("sensor")
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false);
                if sensor_flag && (a == "prop:zone" || b == "prop:zone") {
                    saw = true;
                }
            }
        }
        let y = world.position("prop:box").unwrap()[1];
        assert!(saw, "sensor emits an overlap");
        assert!(y < 1.2, "sensor does not hold the box up, y={y}");
    }

    #[test]
    fn raycast_hits_the_box_not_the_caster() {
        let doc = doc_with_box(2.0);
        let mut world = PhysicsWorld::from_doc(&doc);
        world.step(1.0 / 60.0);
        let hit = world
            .raycast([0.0, 8.0, 0.0], [0.0, -1.0, 0.0], 20.0, None)
            .expect("ray hits");
        assert_eq!(hit.id, "prop:box");
        assert!(hit.toi > 1.0 && hit.toi < 8.0, "toi={}", hit.toi);
        assert!(hit.normal[1] > 0.5, "top face normal, n={:?}", hit.normal);
        let missed = world.raycast([0.0, 8.0, 0.0], [0.0, -1.0, 0.0], 20.0, Some("prop:box"));
        assert!(missed.is_none() || missed.unwrap().id != "prop:box");
        assert!(world
            .raycast([0.0, 1.0, 0.0], [0.0, 0.0, 0.0], 10.0, None)
            .is_none());
    }

    #[test]
    fn fixed_joint_keeps_two_boxes_together() {
        let doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![
                prop("prop:a", "box", [0.0, 3.0, 0.0], [1.0, 1.0, 1.0], false),
                prop("prop:b", "box", [0.0, 4.1, 0.0], [1.0, 1.0, 1.0], false),
            ],
            joints: vec![WorldJoint {
                id: "joint:weld".into(),
                kind: "joint".into(),
                name: "fixed".into(),
                body_a: "prop:a".into(),
                body_b: "prop:b".into(),
                anchor_a: [0.0, 0.5, 0.0],
                anchor_b: [0.0, -0.55, 0.0],
                axis: [0.0, 1.0, 0.0],
            }],
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        assert!(world.apply_impulse("prop:b", [1.0, 0.0, 0.0]));
        for _ in 0..180 {
            world.step(1.0 / 60.0);
        }
        let a = world.position("prop:a").unwrap();
        let b = world.position("prop:b").unwrap();
        let dy = b[1] - a[1];
        let dx = (b[0] - a[0]).abs();
        assert!(
            dy > 0.7 && dy < 1.4 && dx < 0.35,
            "fixed joint holds the stack, a={a:?} b={b:?}"
        );
    }

    #[test]
    fn revolute_pendulum_holds_and_knocks_the_target() {
        // Same layout as examples/physics_minimal.py: a raised rock swings
        // through a chest sitting on two crates.
        let mut bob = prop(
            "prop:bob",
            "sphere",
            [1.60, 2.57, 0.0],
            [0.7, 0.7, 0.7],
            false,
        );
        bob.mass = 5.0;
        bob.friction = 0.4;
        bob.restitution = 0.05;
        let mut chest = prop(
            "prop:target",
            "box",
            [0.0, 1.45, 0.0],
            [0.5, 0.5, 0.5],
            false,
        );
        chest.friction = 0.45;
        chest.restitution = 0.05;
        let mut c1 = prop("prop:c1", "box", [0.0, 0.3, 0.0], [0.6, 0.6, 0.6], false);
        c1.friction = 0.55;
        let mut c2 = prop("prop:c2", "box", [0.0, 0.9, 0.0], [0.6, 0.6, 0.6], false);
        c2.friction = 0.55;
        let doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 12.0,
            floor_y: 0.0,
            props: vec![
                prop(
                    "prop:pivot",
                    "box",
                    [0.0, 3.15, 0.0],
                    [0.25, 0.25, 0.25],
                    true,
                ),
                bob,
                c1,
                c2,
                chest,
            ],
            joints: vec![WorldJoint {
                id: "joint:swing".into(),
                kind: "joint".into(),
                name: "revolute".into(),
                body_a: "prop:pivot".into(),
                body_b: "prop:bob".into(),
                anchor_a: [0.0, 0.0, 0.0],
                anchor_b: [-1.60, 0.58, 0.0],
                axis: [0.0, 0.0, 1.0],
            }],
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        assert!(!world.add_joint(
            "nope",
            "prop:pivot",
            "prop:bob",
            [0.0; 3],
            [0.0; 3],
            [0.0, 1.0, 0.0]
        ));
        for _ in 0..240 {
            world.step(1.0 / 60.0);
        }
        let bob_p = world.position("prop:bob").unwrap();
        let pivot = [0.0f32, 3.15, 0.0];
        let dist = ((bob_p[0] - pivot[0]).powi(2)
            + (bob_p[1] - pivot[1]).powi(2)
            + (bob_p[2] - pivot[2]).powi(2))
        .sqrt();
        assert!(
            (dist - 1.70).abs() < 0.25,
            "revolute keeps the rope, dist={dist} bob={bob_p:?}"
        );
        assert!(bob_p[1] > 0.9, "bob stays on the joint, y={}", bob_p[1]);
        let target = world.position("prop:target").unwrap();
        assert!(
            target[0].abs() > 0.35 || target[1] < 1.15,
            "swing knocks the chest, target={target:?}"
        );
    }

    #[test]
    fn spherical_joint_keeps_distance() {
        let mut bob = prop(
            "prop:bob",
            "sphere",
            [0.0, 2.0, 0.0],
            [0.5, 0.5, 0.5],
            false,
        );
        bob.mass = 2.0;
        let doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 8.0,
            floor_y: 0.0,
            props: vec![
                prop("prop:pivot", "box", [0.0, 3.2, 0.0], [0.2, 0.2, 0.2], true),
                bob,
            ],
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        assert!(world.add_joint(
            "spherical",
            "prop:pivot",
            "prop:bob",
            [0.0, 0.0, 0.0],
            [0.0, 1.2, 0.0],
            [0.0, 1.0, 0.0],
        ));
        assert!(world.apply_impulse("prop:bob", [2.0, 0.0, 1.0]));
        for _ in 0..180 {
            world.step(1.0 / 60.0);
        }
        let p = world.position("prop:bob").unwrap();
        let dist = (p[0] * p[0] + (p[1] - 3.2).powi(2) + p[2] * p[2]).sqrt();
        assert!(
            (dist - 1.2).abs() < 0.2 && p[1] > 1.0,
            "spherical joint holds, dist={dist} p={p:?}"
        );
    }
}
