//! Entity hitboxes built from a spec row: mesh AABB × scale, floor-seated,
//! centred on an anchor, inset so a graze survives.

use crate::game_spec::{EntityKind, GameSpec, Part, PartRole};

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
    /// Underside of the solid region. 0 = floor-seated; above 0 the body
    /// passes beneath when it is short enough (a beam, a lintel).
    pub hit_y0: f32,
    pub role: PartRole,
    /// Ramp only: incline length from `skin_min[2]`.
    pub ramp_run: f32,
}

impl Hitbox {
    /// Floor-seated `kind` centred on `(anchor_x, anchor_z)`.
    pub fn place(s: &GameSpec, kind: &EntityKind, anchor_x: f32, anchor_z: f32) -> Self {
        let role = if kind.behavior == crate::game_spec::Behavior::Rideable {
            PartRole::Ride
        } else {
            PartRole::Solid
        };
        Self::place_mesh(
            s,
            &kind.name,
            &kind.glb,
            kind.scale,
            kind.mesh_min,
            kind.mesh_max,
            [0.0; 3],
            kind.hit_y,
            role,
            0.0,
            (anchor_x, anchor_z),
        )
    }

    /// One extra mesh of `kind`, placed relative to the anchor.
    pub fn place_part(
        s: &GameSpec,
        kind: &EntityKind,
        part: &Part,
        anchor_x: f32,
        anchor_z: f32,
    ) -> Self {
        Self::place_mesh(
            s,
            &kind.name,
            &part.glb,
            part.scale,
            part.mesh_min,
            part.mesh_max,
            part.offset,
            part.hit_y,
            part.role,
            part.run,
            (anchor_x, anchor_z),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn place_mesh(
        s: &GameSpec,
        name: &str,
        glb: &str,
        scale: [f32; 3],
        local_min: [f32; 3],
        local_max: [f32; 3],
        offset: [f32; 3],
        hit_y: Option<[f32; 2]>,
        role: PartRole,
        run: f32,
        anchor: (f32, f32),
    ) -> Self {
        let (anchor_x, anchor_z) = (anchor.0 + offset[0], anchor.1 + offset[2]);
        let origin = [
            anchor_x - scale[0] * (local_min[0] + local_max[0]) * 0.5,
            offset[1] - scale[1] * local_min[1],
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
        let (y0, y1) = match hit_y {
            Some([lo, hi]) => (origin[1] + scale[1] * lo, origin[1] + scale[1] * hi),
            None => (skin_min[1], skin_max[1]),
        };
        let floor_seated = y0 <= 0.001;
        let hit_top = match role {
            // A ride top is a wall below the step tolerance, a floor above it.
            PartRole::Ride => skin_max[1] - s.runner.body.step_tol,
            _ => y0 + (y1 - y0) * s.hit_top,
        };
        Self {
            kind: name.to_string(),
            path: glb.to_string(),
            origin,
            scale,
            skin_min,
            skin_max,
            hit_cx: anchor_x,
            hit_cz: anchor_z,
            hit_hx: hx,
            hit_hz: hz,
            hit_top,
            hit_y0: if floor_seated { 0.0 } else { y0 },
            role,
            ramp_run: run,
        }
    }

    /// Does this part end the run? Ramps and decor never do.
    pub fn kills(&self) -> bool {
        matches!(self.role, PartRole::Solid | PartRole::Ride)
    }

    /// Body of height `h` standing on `y`, against this part's vertical extent.
    pub fn y_overlaps(&self, y: f32, h: f32) -> bool {
        y < self.hit_top && (self.hit_y0 <= 0.0 || y + h > self.hit_y0)
    }

    /// Ground height this part gives the feet at `(x, z)`, or 0.
    pub fn support(&self, x: f32, z: f32, half_w: f32) -> f32 {
        let in_x = (x - self.hit_cx).abs() <= self.hit_hx + half_w;
        match self.role {
            PartRole::Ride => {
                if in_x && (z - self.hit_cz).abs() <= self.hit_hz {
                    self.skin_max[1]
                } else {
                    0.0
                }
            }
            PartRole::Ramp => {
                let t = (z - self.skin_min[2]) / self.ramp_run.max(1e-3);
                let in_x = (x - self.hit_cx).abs() <= (self.skin_max[0] - self.skin_min[0]) * 0.5;
                if in_x && (0.0..=1.0).contains(&t) {
                    self.skin_max[1] * t
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }

    /// Feet at `(x, y, z)` inside the kill box.
    pub fn feet_hit(&self, x: f32, y: f32, z: f32) -> bool {
        self.kills()
            && (x - self.hit_cx).abs() <= self.hit_hx
            && (z - self.hit_cz).abs() <= self.hit_hz
            && y < self.hit_top
    }

    /// The feet moved from (x0, z0) to (x1, z1) this substep: does that
    /// segment touch the box in x AND z (slab test)? Catches a side entry,
    /// not only a front one.
    /// `half_w` is the runner's own body half-width in x.
    pub fn sweep_hit(&self, x0: f32, z0: f32, x1: f32, z1: f32, y: f32, half_w: f32) -> bool {
        self.sweep_hit_body(x0, z0, x1, z1, y, 0.0, half_w)
    }

    /// `sweep_hit` for a body `h` tall: a part with an underside is cleared
    /// when the body fits beneath it.
    #[allow(clippy::too_many_arguments)]
    pub fn sweep_hit_body(
        &self,
        x0: f32,
        z0: f32,
        x1: f32,
        z1: f32,
        y: f32,
        h: f32,
        half_w: f32,
    ) -> bool {
        if !self.kills() || !self.y_overlaps(y, h) {
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
        (x - self.hit_cx).abs() <= hx
            && (z - self.hit_cz).abs() <= hz
            && y <= self.hit_top + 0.05
            && y >= self.hit_y0 - 0.05
    }
}

/// x positions (cat centres) where a body `h` tall standing on `y` would
/// hit one of `parts`, merged.
pub fn blocked_spans(parts: &[Hitbox], y: f32, h: f32, pad: f32) -> Vec<(f32, f32)> {
    let mut v: Vec<(f32, f32)> = parts
        .iter()
        .filter(|p| p.kills() && p.y_overlaps(y, h))
        .map(|p| (p.hit_cx - p.hit_hx - pad, p.hit_cx + p.hit_hx + pad))
        .collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut out: Vec<(f32, f32)> = Vec::new();
    for (lo, hi) in v {
        match out.last_mut() {
            Some(last) if lo <= last.1 => last.1 = last.1.max(hi),
            _ => out.push((lo, hi)),
        }
    }
    out
}

/// Free cat-centre intervals inside the rails, after removing `blocked`.
pub fn free_between(blocked: &[(f32, f32)], limit: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let mut at = -limit;
    for &(lo, hi) in blocked {
        if lo > at {
            out.push((at, lo.min(limit)));
        }
        at = at.max(hi);
    }
    if at < limit {
        out.push((at, limit));
    }
    out.retain(|(a, b)| b - a > 0.02);
    out
}
