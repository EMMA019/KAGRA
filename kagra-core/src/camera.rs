// src/camera.rs
// 簡易3Dカメラ制御（オービット、パン、ズーム）

use nalgebra::{Matrix4, Vector3, Point3};

/// 行優先で並べた 4x4 行列を列優先に並べ替える。
///
/// Python API は行を上から並べた読みやすい順で行列を受け取るが、
/// WGSL の mat4x4 は列優先で読まれるため境界で変換する必要がある。
/// 転置しないと view/proj が壊れて何も描画されない。
pub fn row_major_to_column_major(m: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for r in 0..4 {
        for c in 0..4 {
            out[c * 4 + r] = m[r * 4 + c];
        }
    }
    out
}

pub struct Camera3D {
    pub position: Vector3<f32>,
    pub target: Vector3<f32>,
    pub up: Vector3<f32>,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub aspect: f32,
    // 内部状態
    view_matrix: Matrix4<f32>,
    proj_matrix: Matrix4<f32>,
    view_proj: Matrix4<f32>,
    dirty: bool,
}

impl Camera3D {
    pub fn new(width: u32, height: u32) -> Self {
        let aspect = width as f32 / height as f32;
        let mut cam = Self {
            // 初期カメラ位置をやや正面の離れた位置に変更
            position: Vector3::new(0.0, 1.2, 3.5),
            // 注視点を足元(0,0,0)から、モデルの顔〜胸付近の高さ(Y=1.0)に変更
            target: Vector3::new(0.0, 1.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            fov: 45.0_f32.to_radians(),
            near: 0.1,
            far: 1000.0,
            aspect,
            view_matrix: Matrix4::identity(),
            proj_matrix: Matrix4::identity(),
            view_proj: Matrix4::identity(),
            dirty: true,
        };
        cam.update_matrices();
        cam
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        // 最小化などで height=0 になっても aspect を壊さない
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        if (aspect - self.aspect).abs() > 1e-6 {
            self.aspect = aspect;
            self.dirty = true;
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.position = Vector3::new(x, y, z);
        self.dirty = true;
    }

    pub fn set_target(&mut self, x: f32, y: f32, z: f32) {
        self.target = Vector3::new(x, y, z);
        self.dirty = true;
    }

    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        // 現在の方位角・仰角を計算
        let direction = self.target - self.position;
        let radius = direction.magnitude();
        if radius < 1e-6 { return; }
        let mut yaw = direction.x.atan2(direction.z);
        let mut pitch = (direction.y / radius).asin();
        yaw += delta_x * 0.01;
        pitch = (pitch + delta_y * 0.01).clamp(-1.4, 1.4);
        let new_dir = Vector3::new(
            radius * yaw.sin() * pitch.cos(),
            radius * pitch.sin(),
            radius * yaw.cos() * pitch.cos(),
        );
        self.position = self.target - new_dir;
        self.dirty = true;
    }

    pub fn zoom(&mut self, delta: f32) {
        let direction = self.target - self.position;
        let radius = direction.magnitude();
        if radius < 1e-6 {
            return; // ゼロ除算を防ぐ
        }
        let new_radius = (radius - delta * 0.5).max(0.5);
        let new_pos = self.target - direction.normalize() * new_radius;
        self.position = new_pos;
        self.dirty = true;
    }

    pub fn update_matrices(&mut self) {
        if !self.dirty { return; }
        self.view_matrix = Matrix4::look_at_rh(
            &Point3::from(self.position),
            &Point3::from(self.target),
            &self.up,
        );
        
        let proj = Matrix4::new_perspective(self.aspect, self.fov, self.near, self.far);
        
        // 【重要】OpenGL形式のZ座標(-1.0〜1.0)をWebGPU形式(0.0〜1.0)に変換する補正行列
        // これがないと手前にあるポリゴンがクリップされてちょん切れて表示されます。
        let wgpu_correction = Matrix4::new(
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 0.5, 0.5,
            0.0, 0.0, 0.0, 1.0,
        );
        
        self.proj_matrix = wgpu_correction * proj;
        self.view_proj = self.proj_matrix * self.view_matrix;
        self.dirty = false;
    }

    pub fn view_matrix(&self) -> Matrix4<f32> { self.view_matrix }
    pub fn proj_matrix(&self) -> Matrix4<f32> { self.proj_matrix }
    pub fn view_proj(&self) -> Matrix4<f32> { self.view_proj }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_builds_view_proj() {
        let cam = Camera3D::new(1280, 720);
        assert!(!cam.dirty);
        let expected = cam.proj_matrix() * cam.view_matrix();
        let vp = cam.view_proj();
        for r in 0..4 {
            for c in 0..4 {
                assert!((vp[(r, c)] - expected[(r, c)]).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn proj_applies_wgpu_z_correction() {
        let cam = Camera3D::new(800, 600);
        let raw = Matrix4::new_perspective(cam.aspect, cam.fov, cam.near, cam.far);
        assert!((cam.proj_matrix() - raw).norm() > 1e-3);
    }

    #[test]
    fn zoom_respects_min_radius() {
        let mut cam = Camera3D::new(800, 600);
        for _ in 0..100 {
            cam.zoom(100.0);
        }
        cam.update_matrices();
        let radius = (cam.target - cam.position).magnitude();
        assert!(radius >= 0.5 - 1e-4);
    }

    #[test]
    fn row_major_to_column_major_transposes() {
        let m: [f32; 16] = [
             0.0,  1.0,  2.0,  3.0,
             4.0,  5.0,  6.0,  7.0,
             8.0,  9.0, 10.0, 11.0,
            12.0, 13.0, 14.0, 15.0,
        ];
        let got = row_major_to_column_major(&m);
        // 1 列目は元の 1 行目
        assert_eq!(&got[0..4], &[0.0, 4.0, 8.0, 12.0]);
        // 対角は不変
        for i in 0..4 {
            assert_eq!(got[i * 4 + i], m[i * 4 + i]);
        }
        // 2 回かけると元に戻る
        assert_eq!(row_major_to_column_major(&got), m);
    }

    #[test]
    fn resize_updates_aspect_and_marks_dirty() {
        let mut cam = Camera3D::new(800, 600);
        assert!(!cam.dirty);
        cam.resize(1600, 400);
        assert!(cam.dirty);
        assert!((cam.aspect - 4.0).abs() < 1e-5);
    }

    #[test]
    fn resize_to_same_aspect_is_noop() {
        let mut cam = Camera3D::new(800, 600);
        cam.resize(1600, 1200);
        assert!(!cam.dirty, "同じアスペクト比で再計算を走らせない");
    }

    #[test]
    fn resize_survives_zero_height() {
        let mut cam = Camera3D::new(800, 600);
        cam.resize(800, 0);
        assert!(cam.aspect.is_finite());
        assert!(cam.aspect > 0.0);
    }

    #[test]
    fn orbit_keeps_radius() {
        let mut cam = Camera3D::new(800, 600);
        let before = (cam.target - cam.position).magnitude();
        cam.orbit(12.0, -4.0);
        let after = (cam.target - cam.position).magnitude();
        assert!((before - after).abs() < 1e-3);
    }
}