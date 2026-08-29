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

    pub fn from_points(pts: &[[f32; 3]]) -> Option<Self> {
        if pts.is_empty() {
            return None;
        }
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in pts {
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        }
        Some(Self { min, max }.expand(1e-4))
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            min: [
                self.min[0].min(other.min[0]),
                self.min[1].min(other.min[1]),
                self.min[2].min(other.min[2]),
            ],
            max: [
                self.max[0].max(other.max[0]),
                self.max[1].max(other.max[1]),
                self.max[2].max(other.max[2]),
            ],
        }
    }

    /// 8 隅を行列で飛ばす（スキン行列 / TRS）。
    pub fn transform(self, m: &Matrix4<f32>) -> Self {
        let corners = [
            [self.min[0], self.min[1], self.min[2]],
            [self.max[0], self.min[1], self.min[2]],
            [self.min[0], self.max[1], self.min[2]],
            [self.max[0], self.max[1], self.min[2]],
            [self.min[0], self.min[1], self.max[2]],
            [self.max[0], self.min[1], self.max[2]],
            [self.min[0], self.max[1], self.max[2]],
            [self.max[0], self.max[1], self.max[2]],
        ];
        let mut pts = [[0.0f32; 3]; 8];
        for (i, c) in corners.iter().enumerate() {
            let p = m.transform_point(&nalgebra::Point3::new(c[0], c[1], c[2]));
            pts[i] = [p.x, p.y, p.z];
        }
        Self::from_points(&pts).unwrap_or(self)
    }

    /// 位置 + 軸スケール + Y 回転（箱インスタンス用）。
    pub fn transform_trs(self, pos: [f32; 3], scale: [f32; 3], yaw: f32) -> Self {
        let (s, c) = yaw.sin_cos();
        let m = Matrix4::new(
            c * scale[0], 0.0, s * scale[2], pos[0],
            0.0, scale[1], 0.0, pos[1],
            -s * scale[0], 0.0, c * scale[2], pos[2],
            0.0, 0.0, 0.0, 1.0,
        );
        self.transform(&m)
    }

    pub fn padded(self, abs_pad: f32, rel: f32) -> Self {
        let ext = [
            (self.max[0] - self.min[0]).max(0.0),
            (self.max[1] - self.min[1]).max(0.0),
            (self.max[2] - self.min[2]).max(0.0),
        ];
        let mean = (ext[0] + ext[1] + ext[2]) / 3.0;
        self.expand(abs_pad + rel * mean)
    }

    pub fn center(self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    pub fn max_extent(self) -> f32 {
        (self.max[0] - self.min[0])
            .max(self.max[1] - self.min[1])
            .max(self.max[2] - self.min[2])
    }
}

/// バインド姿勢の頂点を、重み 0.05 以上のジョイントへ振り分ける。
pub fn bone_bind_aabbs(
    pos: &[[f32; 3]],
    joints: &[[u32; 4]],
    weights: &[[f32; 4]],
) -> Vec<(u16, Aabb)> {
    use std::collections::HashMap;
    let mut acc: HashMap<u16, ( [f32; 3], [f32; 3] )> = HashMap::new();
    for i in 0..pos.len() {
        let p = pos[i];
        let j = joints.get(i).copied().unwrap_or([0; 4]);
        let w = weights.get(i).copied().unwrap_or([1.0, 0.0, 0.0, 0.0]);
        let mut any = false;
        for k in 0..4 {
            if w[k] > 0.05 {
                any = true;
                let key = j[k] as u16;
                let e = acc.entry(key).or_insert((p, p));
                for a in 0..3 {
                    e.0[a] = e.0[a].min(p[a]);
                    e.1[a] = e.1[a].max(p[a]);
                }
            }
        }
        if !any {
            let key = j[0] as u16;
            let e = acc.entry(key).or_insert((p, p));
            for a in 0..3 {
                e.0[a] = e.0[a].min(p[a]);
                e.1[a] = e.1[a].max(p[a]);
            }
        }
    }
    let mut out: Vec<(u16, Aabb)> = acc
        .into_iter()
        .map(|(k, (min, max))| (k, Aabb { min, max }.expand(1e-4)))
        .collect();
    out.sort_by_key(|(k, _)| *k);
    out
}

/// スキン行列でボーン AABB を飛ばして和を取る。パッドは Spring / morph 用。
pub const VRM_CULL_ABS_PAD: f32 = 0.12;
pub const VRM_CULL_REL_PAD: f32 = 0.20;

pub fn skinned_aabb(
    bone_aabbs: &[(u16, Aabb)],
    matrices: &[Matrix4<f32>],
    extra_pad: f32,
) -> Option<Aabb> {
    let mut acc: Option<Aabb> = None;
    for &(ji, aabb) in bone_aabbs {
        let m = matrices.get(ji as usize).copied().unwrap_or_else(Matrix4::identity);
        let w = aabb.transform(&m);
        acc = Some(match acc {
            Some(u) => u.union(w),
            None => w,
        });
    }
    acc.map(|a| a.padded(VRM_CULL_ABS_PAD + extra_pad.max(0.0), VRM_CULL_REL_PAD))
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

    #[test]
    fn transform_identity_keeps_box() {
        let a = Aabb {
            min: [-1.0, 0.0, -1.0],
            max: [1.0, 2.0, 1.0],
        };
        let t = a.transform(&Matrix4::identity());
        // from_points が 1e-4 パッドするので、完全一致はしない
        assert!((t.min[1] - a.min[1]).abs() < 2e-4);
        assert!((t.max[0] - a.max[0]).abs() < 2e-4);
    }

    #[test]
    fn transform_translation_moves_box() {
        let a = Aabb {
            min: [-0.2, 0.0, -0.2],
            max: [0.2, 0.4, 0.2],
        };
        let m = Matrix4::new_translation(&nalgebra::Vector3::new(10.0, 0.0, 0.0));
        let t = a.transform(&m);
        assert!(t.min[0] > 9.0);
        assert!(t.max[0] < 11.0);
    }

    #[test]
    fn skinned_bones_follow_palette() {
        let bone = Aabb {
            min: [-0.1, 0.0, -0.1],
            max: [0.1, 0.2, 0.1],
        };
        let m = Matrix4::new_translation(&nalgebra::Vector3::new(0.0, 0.0, 40.0));
        let world = skinned_aabb(&[(0, bone)], &[m], 0.0).unwrap();
        let fr = default_frustum();
        assert!(!fr.contains_aabb(&world));
    }

    #[test]
    fn bone_bind_groups_by_joint() {
        let pos = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        let joints = [[0, 0, 0, 0], [1, 0, 0, 0]];
        let weights = [[1.0, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]];
        let aabbs = bone_bind_aabbs(&pos, &joints, &weights);
        assert_eq!(aabbs.len(), 2);
        assert_eq!(aabbs[0].0, 0);
        assert_eq!(aabbs[1].0, 1);
    }

    #[test]
    fn trs_yaw_moves_unit_box() {
        let unit = Aabb {
            min: [-0.5, -0.5, -0.5],
            max: [0.5, 0.5, 0.5],
        };
        let t = unit.transform_trs([3.0, 1.0, 0.0], [2.0, 1.0, 2.0], 0.0);
        assert!((t.center()[0] - 3.0).abs() < 1e-4);
        assert!((t.max_extent() - 2.0).abs() < 1e-3);
    }
}
