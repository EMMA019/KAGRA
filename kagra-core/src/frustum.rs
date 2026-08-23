//! 視錐台カリング。three.js の `Frustum.intersectsBox` と同じ種類の仕事。
//! GPU 不要。view / proj は列優先 16 要素（wgpu / nalgebra と同じ）。

use nalgebra::Matrix4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    pub fn from_mesh3d_verts(verts: &[[f32; 8]]) -> Option<Self> {
        if verts.is_empty() {
            return None;
        }
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for v in verts {
            for i in 0..3 {
                min[i] = min[i].min(v[i]);
                max[i] = max[i].max(v[i]);
            }
        }
        Some(Self { min, max }.expand(1e-3))
    }

    pub fn expand(self, pad: f32) -> Self {
        Self {
            min: [self.min[0] - pad, self.min[1] - pad, self.min[2] - pad],
            max: [self.max[0] + pad, self.max[1] + pad, self.max[2] + pad],
        }
    }
}

/// 正規化した平面 ax+by+cz+d >= 0 が内側。
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [[f32; 4]; 6],
}

impl Frustum {
    pub fn from_view_proj(vp: &Matrix4<f32>) -> Self {
        let normalize = |mut n: [f32; 4]| {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-8);
            n[0] /= len;
            n[1] /= len;
            n[2] /= len;
            n[3] /= len;
            n
        };
        // 左右上下はクリップ ±w。近クリップは wgpu の z∈[0, w]（OpenGL の z≥-w ではない）。
        let plane = |sign: f32, axis: usize| {
            normalize([
                vp[(3, 0)] + sign * vp[(axis, 0)],
                vp[(3, 1)] + sign * vp[(axis, 1)],
                vp[(3, 2)] + sign * vp[(axis, 2)],
                vp[(3, 3)] + sign * vp[(axis, 3)],
            ])
        };
        let near = normalize([vp[(2, 0)], vp[(2, 1)], vp[(2, 2)], vp[(2, 3)]]);
        Self {
            planes: [
                plane(1.0, 0),
                plane(-1.0, 0),
                plane(1.0, 1),
                plane(-1.0, 1),
                near,
                plane(-1.0, 2),
            ],
        }
    }

    pub fn from_view_proj_col(view: &[f32; 16], proj: &[f32; 16]) -> Self {
        let view = Matrix4::from_column_slice(view);
        let proj = Matrix4::from_column_slice(proj);
        Self::from_view_proj(&(proj * view))
    }

    /// 完全に外側なら false（境界上は残す）。
    pub fn contains_aabb(&self, aabb: &Aabb) -> bool {
        for plane in &self.planes {
            let px = if plane[0] >= 0.0 { aabb.max[0] } else { aabb.min[0] };
            let py = if plane[1] >= 0.0 { aabb.max[1] } else { aabb.min[1] };
            let pz = if plane[2] >= 0.0 { aabb.max[2] } else { aabb.min[2] };
            if plane[0] * px + plane[1] * py + plane[2] * pz + plane[3] < 0.0 {
                return false;
            }
        }
        true
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStats {
    pub draw_calls: u32,
    pub triangles: u32,
    pub culled: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Camera3D;

    fn default_frustum() -> Frustum {
        let mut cam = Camera3D::new(1280, 720);
        cam.update_matrices();
        Frustum::from_view_proj(&cam.view_proj())
    }

    #[test]
    fn target_box_is_visible() {
        let fr = default_frustum();
        let box_at_look = Aabb {
            min: [-0.3, 0.7, -0.3],
            max: [0.3, 1.3, 0.3],
        };
        assert!(fr.contains_aabb(&box_at_look));
    }

    #[test]
    fn far_side_box_is_culled() {
        let fr = default_frustum();
        let far = Aabb {
            min: [80.0, -1.0, -1.0],
            max: [82.0, 1.0, 1.0],
        };
        assert!(!fr.contains_aabb(&far));
    }

    #[test]
    fn behind_camera_is_culled() {
        let fr = default_frustum();
        // default camera is at z=3.5 looking at z=0; box behind the eye
        let behind = Aabb {
            min: [-0.2, 0.8, 8.0],
            max: [0.2, 1.2, 9.0],
        };
        assert!(!fr.contains_aabb(&behind));
    }

    #[test]
    fn from_col_matches_matrix() {
        let mut cam = Camera3D::new(800, 600);
        cam.update_matrices();
        let view: [f32; 16] = cam.view_matrix().as_slice().try_into().unwrap();
        let proj: [f32; 16] = cam.proj_matrix().as_slice().try_into().unwrap();
        let a = Frustum::from_view_proj(&cam.view_proj());
        let b = Frustum::from_view_proj_col(&view, &proj);
        let sample = Aabb {
            min: [-0.5, 0.5, -0.5],
            max: [0.5, 1.5, 0.5],
        };
        assert_eq!(a.contains_aabb(&sample), b.contains_aabb(&sample));
    }

    #[test]
    fn mesh_verts_aabb() {
        let verts = [
            [1.0, 2.0, 3.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            [-1.0, 0.0, 4.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        ];
        let aabb = Aabb::from_mesh3d_verts(&verts).unwrap();
        assert!(aabb.min[0] <= -1.0);
        assert!(aabb.max[2] >= 4.0);
    }
}
