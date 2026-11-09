//! Camera math a kit needs for touch: fit a view to a phone in either orientation, put a
//! world point on the canvas, and find the ground under a finger. `fov` is the vertical field
//! of view in degrees (what `WorldCamera::fov` means); pixels start at the top-left, y down.
//! First used by Moonlit Swarm (tap to lock a foe, drag to walk).

/// A look-at camera: the eye, the point it looks at, and the vertical field of view (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub fov: f32,
}

impl View {
    /// The narrowest view of `eye -> target` that keeps every point of `keep` on the canvas
    /// (aspect = width / height), clamped to `fov_min..=fov_max` degrees. A portrait phone
    /// needs a wider vertical angle to show the same width; near points spread wider than far
    /// ones, so pass the corners of what must stay in sight. Points behind the eye are skipped.
    pub fn fit(
        eye: [f32; 3],
        target: [f32; 3],
        keep: &[[f32; 3]],
        aspect: f32,
        fov_min: f32,
        fov_max: f32,
    ) -> Self {
        let mut v = Self {
            eye,
            target,
            fov: fov_min,
        };
        let (f, r, u) = v.basis();
        let mut need = 0.0_f32;
        for &p in keep {
            let d = sub3(p, eye);
            let cz = dot(d, f);
            if cz < 0.1 {
                continue;
            }
            need = need
                .max(dot(d, u).abs() / cz)
                .max(dot(d, r).abs() / (cz * aspect.max(0.2)));
        }
        v.fov = (2.0 * need.atan())
            .to_degrees()
            .clamp(fov_min, fov_max.max(fov_min));
        v
    }

    /// A view from `target` back along `toward_eye` at a fixed `fov` (degrees), as close as it
    /// can be while every point of `keep` stays on the canvas. Use it when a fitted angle would
    /// get too wide on a narrow phone: the camera backs off instead of turning fish-eye.
    pub fn frame(
        target: [f32; 3],
        toward_eye: [f32; 3],
        fov: f32,
        keep: &[[f32; 3]],
        aspect: f32,
    ) -> Self {
        let dir = norm3(toward_eye);
        let at = |d: f32| {
            [
                target[0] + dir[0] * d,
                target[1] + dir[1] * d,
                target[2] + dir[2] * d,
            ]
        };
        let need = |d: f32| Self::fit(at(d), target, keep, aspect, 0.0, 179.0).fov;
        let (mut lo, mut hi) = (0.5_f32, 1.0_f32);
        while need(hi) > fov && hi < 4096.0 {
            lo = hi;
            hi *= 2.0;
        }
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if need(mid) > fov {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Self {
            eye: at(hi),
            target,
            fov,
        }
    }

    /// The same picture inside a band of the canvas. `self` is the view wanted for the rows
    /// `y0..y1` (fractions of the canvas height from the top; a scoreboard above and a
    /// panel below cover the rest). The result is the view to give the renderer for the
    /// whole canvas: a wider angle, turned so `target` lands in the middle of the band.
    /// [`View::project`] and [`View::ground_at`] on it use whole-canvas pixels as usual.
    pub fn in_band(&self, y0: f32, y1: f32) -> Self {
        let (y0, y1) = (y0.clamp(0.0, 1.0), y1.clamp(0.0, 1.0));
        let band = (y1 - y0).max(0.05);
        let t = (self.fov.to_radians() * 0.5).tan() / band;
        // The band's middle in NDC (+1 is the top edge), and the angle above the axis
        // that lands there.
        let yc = 1.0 - (y0 + y1);
        let a = (yc * t).atan();
        let (f, _, u) = self.basis();
        let (s, c) = a.sin_cos();
        let nf = norm3([
            f[0] * c - u[0] * s,
            f[1] * c - u[1] * s,
            f[2] * c - u[2] * s,
        ]);
        let d = dot(sub3(self.target, self.eye), f).max(0.1);
        Self {
            eye: self.eye,
            target: [
                self.eye[0] + nf[0] * d,
                self.eye[1] + nf[1] * d,
                self.eye[2] + nf[2] * d,
            ],
            fov: (2.0 * t.atan()).to_degrees(),
        }
    }

    /// Ease from `self` to `to` (`k` 0..1): eye and target move in a straight line, the
    /// angle follows. A broadcast cut that glides instead of jumping.
    pub fn lerp(&self, to: &View, k: f32) -> Self {
        let k = k.clamp(0.0, 1.0);
        let l = |a: [f32; 3], b: [f32; 3]| {
            [
                a[0] + (b[0] - a[0]) * k,
                a[1] + (b[1] - a[1]) * k,
                a[2] + (b[2] - a[2]) * k,
            ]
        };
        Self {
            eye: l(self.eye, to.eye),
            target: l(self.target, to.target),
            fov: self.fov + (to.fov - self.fov) * k,
        }
    }

    /// The corners of a ground rectangle `x0..x1` by `z0..z1` at height `y`, for [`View::fit`].
    pub fn ground_box(x0: f32, x1: f32, z0: f32, z1: f32, y: f32) -> [[f32; 3]; 4] {
        [[x0, y, z0], [x1, y, z0], [x0, y, z1], [x1, y, z1]]
    }

    fn basis(&self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let f = norm3(sub3(self.target, self.eye));
        let r = norm3(cross(f, [0.0, 1.0, 0.0]));
        (f, r, cross(r, f))
    }

    /// A world point on a `w` x `h` canvas (pixels), or `None` behind the camera.
    pub fn project(&self, p: [f32; 3], w: f32, h: f32) -> Option<(f32, f32)> {
        let aspect = w / h.max(1.0);
        let (f, r, u) = self.basis();
        let v = sub3(p, self.eye);
        let cz = dot(v, f);
        if cz < 0.1 {
            return None;
        }
        let t = (self.fov.to_radians() * 0.5).tan();
        let nx = dot(v, r) / (cz * t * aspect.max(0.2));
        let ny = dot(v, u) / (cz * t);
        Some(((nx * 0.5 + 0.5) * w, (0.5 - ny * 0.5) * h))
    }

    /// The nearest of `points` to a canvas pixel, within `reach` pixels: what a thumb meant to
    /// tap. Give each thing's middle (not its feet). `None` when nothing is that close.
    pub fn pick(
        &self,
        points: impl IntoIterator<Item = [f32; 3]>,
        px: f32,
        py: f32,
        w: f32,
        h: f32,
        reach: f32,
    ) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, p) in points.into_iter().enumerate() {
            let Some((sx, sy)) = self.project(p, w, h) else {
                continue;
            };
            let d = (sx - px).hypot(sy - py);
            if d < reach && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(i, _)| i)
    }

    /// The point at height `ground_y` under a canvas pixel, as world `(x, z)`, or `None` for a
    /// ray that does not come down to it.
    pub fn ground_at(&self, px: f32, py: f32, w: f32, h: f32, ground_y: f32) -> Option<(f32, f32)> {
        let aspect = w / h.max(1.0);
        let (f, r, u) = self.basis();
        let ty = (self.fov.to_radians() * 0.5).tan();
        let nx = (2.0 * px / w.max(1.0) - 1.0) * ty * aspect;
        let ny = (1.0 - 2.0 * py / h.max(1.0)) * ty;
        let d = [
            f[0] + r[0] * nx + u[0] * ny,
            f[1] + r[1] * nx + u[1] * ny,
            f[2] + r[2] * nx + u[2] * ny,
        ];
        if d[1] > -0.02 {
            return None;
        }
        let t = (ground_y - self.eye[1]) / d[1];
        if t < 0.0 {
            return None;
        }
        Some((self.eye[0] + d[0] * t, self.eye[2] + d[2] * t))
    }
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn len3(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm3(a: [f32; 3]) -> [f32; 3] {
    let l = len3(a).max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARENA: [[f32; 3]; 4] = [
        [-7.0, 0.0, -7.0],
        [7.0, 0.0, -7.0],
        [-7.0, 0.0, 7.0],
        [7.0, 0.0, 7.0],
    ];

    fn top_down() -> View {
        View::fit(
            [0.0, 14.0, 9.0],
            [0.0; 3],
            &ARENA,
            390.0 / 844.0,
            20.0,
            100.0,
        )
    }

    #[test]
    fn the_target_is_the_centre_and_plus_x_is_screen_right() {
        let v = top_down();
        let (x, y) = v.project([0.0; 3], 390.0, 844.0).unwrap();
        assert!((x - 195.0).abs() < 0.5 && (y - 422.0).abs() < 0.5);
        let (rx, _) = v.project([3.0, 0.0, 0.0], 390.0, 844.0).unwrap();
        assert!(rx > 195.0);
        let (_, dy) = v.project([0.0, 0.0, 3.0], 390.0, 844.0).unwrap();
        assert!(dy > 422.0, "+z (toward the camera) is down the screen");
        assert!(
            v.project([0.0, 20.0, 30.0], 390.0, 844.0).is_none(),
            "behind"
        );
    }

    #[test]
    fn a_portrait_fit_keeps_the_width_on_screen() {
        let v = top_down();
        let mut edge = false;
        for p in ARENA {
            let (sx, sy) = v.project(p, 390.0, 844.0).unwrap();
            assert!(
                (-0.5..=390.5).contains(&sx) && (-0.5..=844.5).contains(&sy),
                "{p:?}"
            );
            edge |= !(1.0..=389.0).contains(&sx) || !(1.0..=843.0).contains(&sy);
        }
        assert!(edge, "the narrowest view touches an edge");
        let land = View::fit(
            [0.0, 14.0, 9.0],
            [0.0; 3],
            &ARENA,
            844.0 / 390.0,
            20.0,
            100.0,
        );
        assert!(land.fov < v.fov, "landscape needs less vertical angle");
        let back = View::frame([0.0; 3], [0.0, 14.0, 9.0], 50.0, &ARENA, 390.0 / 844.0);
        assert!((back.fov - 50.0).abs() < 1e-6);
        let need = View::fit(back.eye, back.target, &ARENA, 390.0 / 844.0, 0.0, 179.0).fov;
        assert!(need <= 50.0 && need > 49.5, "just fits: {need}");
    }

    #[test]
    fn ground_at_undoes_project() {
        let v = top_down();
        for p in [[0.0, 0.0, 0.0], [2.5, 0.0, -3.0], [-4.0, 0.0, 4.0]] {
            let (sx, sy) = v.project(p, 390.0, 844.0).unwrap();
            let (gx, gz) = v.ground_at(sx, sy, 390.0, 844.0, 0.0).unwrap();
            assert!(
                (gx - p[0]).abs() < 1e-3 && (gz - p[2]).abs() < 1e-3,
                "{p:?}"
            );
        }
        assert!(
            v.ground_at(195.0, -4000.0, 390.0, 844.0, 0.0).is_none(),
            "sky"
        );
    }

    #[test]
    fn pick_finds_the_nearest_thing_within_reach() {
        let v = top_down();
        let things = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [-3.0, 0.0, 3.0]];
        let (sx, sy) = v.project(things[1], 390.0, 844.0).unwrap();
        assert_eq!(v.pick(things, sx + 3.0, sy, 390.0, 844.0, 40.0), Some(1));
        assert_eq!(v.pick(things, sx + 300.0, sy, 390.0, 844.0, 40.0), None);
    }

    #[test]
    fn a_view_in_a_band_shows_its_picture_between_the_panels() {
        let want = View {
            eye: [0.0, 3.0, 8.0],
            target: [0.0, 1.0, -18.0],
            fov: 24.0,
        };
        let (w, h) = (390.0, 844.0);
        let (y0, y1) = (0.09, 0.62);
        let v = want.in_band(y0, y1);
        assert!(
            v.fov > want.fov * 1.6,
            "wider for the whole canvas: {}",
            v.fov
        );
        // The target lands in the middle of the band.
        let (_, sy) = v.project(want.target, w, h).unwrap();
        assert!((sy / h - (y0 + y1) * 0.5).abs() < 0.01, "{}", sy / h);
        // The top and bottom of the wanted picture land on the band's edges.
        let d = 26.0_f32 * 1.0;
        let up = (want.fov.to_radians() * 0.5).tan() * d;
        let (f, _, u) = want.basis();
        let at = |s: f32| {
            [
                want.eye[0] + f[0] * d + u[0] * up * s,
                want.eye[1] + f[1] * d + u[1] * up * s,
                want.eye[2] + f[2] * d + u[2] * up * s,
            ]
        };
        let (_, top) = v.project(at(1.0), w, h).unwrap();
        let (_, bottom) = v.project(at(-1.0), w, h).unwrap();
        assert!((top / h - y0).abs() < 0.02, "top {}", top / h);
        assert!((bottom / h - y1).abs() < 0.02, "bottom {}", bottom / h);
        // A glide between two views passes through the middle.
        let mid = want.lerp(&v, 0.5);
        assert!((mid.fov - (want.fov + v.fov) * 0.5).abs() < 1e-4);
    }
}
