//! Entity hitboxes built from a spec row: mesh AABB × scale, floor-seated,
//! centred on an anchor, inset so a graze survives.

use crate::game_spec::{EntityKind, GameSpec};

#[derive(Clone, Debug)]
pub struct Hitbox {
    pub kind: String,
    pub path: String,
    /// Prop origin. Scale is about this point, then the mesh is translated.
    pub origin: [f32; 3],
    pub scale: [f32; 3],
    /// World AABB of the baked mesh.
    pub skin_min: [f32; 3],
    pub skin_max: [f32; 3],
    pub hit_cx: f32,
    pub hit_cz: f32,
    pub hit_hx: f32,
    pub hit_hz: f32,
    /// Feet at or above this clear the entity.
    pub hit_top: f32,
}

impl Hitbox {
    /// Floor-seated `kind` centred on `(anchor_x, anchor_z)`.
    pub fn place(s: &GameSpec, kind: &EntityKind, anchor_x: f32, anchor_z: f32) -> Self {
        let scale = kind.scale;
        let local_min = kind.mesh_min;
        let local_max = kind.mesh_max;
        let origin = [
            anchor_x - scale[0] * (local_min[0] + local_max[0]) * 0.5,
            -scale[1] * local_min[1],
            anchor_z - scale[2] * (local_min[2] + local_max[2]) * 0.5,
        ];
        let skin_min = [
            origin[0] + scale[0] * local_min[0],
            origin[1] + scale[1] * local_min[1],
            origin[2] + scale[2] * local_min[2],
        ];
        let skin_max = [
            origin[0] + scale[0] * local_max[0],
            origin[1] + scale[1] * local_max[1],
            origin[2] + scale[2] * local_max[2],
        ];
        let hx = ((skin_max[0] - skin_min[0]) * 0.5 - s.hit_inset).max(0.05);
        let hz = ((skin_max[2] - skin_min[2]) * 0.5 - s.hit_inset).max(0.05);
        Self {
            kind: kind.name.clone(),
            path: kind.glb.clone(),
            origin,
            scale,
            skin_min,
            skin_max,
            hit_cx: anchor_x,
            hit_cz: anchor_z,
            hit_hx: hx,
            hit_hz: hz,
            hit_top: (skin_max[1] - skin_min[1]) * s.hit_top,
        }
    }

    /// Feet at `(x, y, z)` inside the kill box.
    pub fn feet_hit(&self, x: f32, y: f32, z: f32) -> bool {
        (x - self.hit_cx).abs() <= self.hit_hx
            && (z - self.hit_cz).abs() <= self.hit_hz
            && y < self.hit_top
    }

    /// The feet moved from (x0, z0) to (x1, z1) this substep: does that
    /// segment touch the box in x AND z (slab test)? Catches a side entry,
    /// not only a front one.
    /// `half_w` is the runner's own body half-width in x.
    pub fn sweep_hit(&self, x0: f32, z0: f32, x1: f32, z1: f32, y: f32, half_w: f32) -> bool {
        if y >= self.hit_top {
            return false;
        }
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for (a, b, c, h) in [
            (x0, x1, self.hit_cx, self.hit_hx + half_w),
            (z0, z1, self.hit_cz, self.hit_hz),
        ] {
            let d = b - a;
            if d.abs() < 1e-6 {
                if (a - c).abs() > h {
                    return false;
                }
            } else {
                let (t0, t1) = ((c - h - a) / d, (c + h - a) / d);
                lo = lo.max(t0.min(t1));
                hi = hi.min(t0.max(t1));
                if lo > hi {
                    return false;
                }
            }
        }
        true
    }

    /// Point inside the box grown by `pad` (sparks, pickups).
    pub fn contains_padded(&self, x: f32, y: f32, z: f32, pad: f32) -> bool {
        let hx = (self.hit_hx + pad).max(0.1);
        let hz = (self.hit_hz + pad).max(0.1);
        (x - self.hit_cx).abs() <= hx && (z - self.hit_cz).abs() <= hz && y <= self.hit_top + 0.05
    }
}
