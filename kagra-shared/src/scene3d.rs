//! 3D シーンの記述。GPU に依存しないので `render` feature 無しでもテストできる。
//!
//! 2D の `scene::DrawList` と同じ考え方で、「状態から描画内容を作る」ところまでを
//! 純粋な計算に閉じ込める。レンダラはここで組み立てた `Scene3D` を GPU に流すだけ。
//! おかげでカメラ行列・視錐台カリング・ワールド生成を GPU の無い CI で検証できる。
//!
//! 座標系は右手系で y が上。奥行きは wgpu に合わせて 0..1 に写す。
//!
//! 永続ワールドは `WorldDoc`（`docs/schemas/world.json`）。ここは 1 フレームの
//! 描画内容だけ。dump JSON を `Scene3D` に詰め込まない（モバイルの collectathon /
//! driving がこの型を組み立てる）。

use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};

/// 位置 + 法線 + UV。カプセル / プロップ / ハイトフィールドは uv = 0（1x1 白）。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex3 {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex3 {
    pub fn new(pos: Vec3, normal: Vec3) -> Self {
        Self::with_uv(pos, normal, [0.0, 0.0])
    }

    pub fn with_uv(pos: Vec3, normal: Vec3, uv: [f32; 2]) -> Self {
        Self {
            pos: pos.to_array(),
            normal: normal.to_array(),
            uv,
        }
    }
}

/// RGBA8 baseColor（glTF / VRM）。無しならレンダラが 1x1 白を貼る。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbedoRgba {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Thin MToon shade (VRM 0 materialProperties / VRM 1 VRMC_materials_mtoon).
/// Not RendererV2: shadeColor + shadingToony/shift only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MtoonShade {
    pub shade_color: [f32; 3],
    pub shading_toony: f32,
    pub shading_shift: f32,
}

impl Default for MtoonShade {
    fn default() -> Self {
        Self {
            shade_color: [0.55, 0.50, 0.52],
            shading_toony: 0.85,
            shading_shift: 0.0,
        }
    }
}

impl MtoonShade {
    /// GPU instance location 9: rgb = shadeColor, a = shadingToony.
    pub fn gpu(self) -> [f32; 4] {
        [
            self.shade_color[0],
            self.shade_color[1],
            self.shade_color[2],
            self.shading_toony.clamp(0.0, 0.999),
        ]
    }
}

/// CPU 側のメッシュ。`Renderer::upload_mesh` で GPU に載せて `MeshId` を得る。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    pub vertices: Vec<Vertex3>,
    pub indices: Vec<u32>,
    pub albedo: Option<AlbedoRgba>,
    /// Present when the glTF/VRM primitive authored MToon shade.
    pub mtoon: Option<MtoonShade>,
}

impl MeshData {
    /// ローカル空間の境界箱。視錐台カリングに使う。
    pub fn bounds(&self) -> Aabb {
        let mut b = Aabb::empty();
        for v in &self.vertices {
            b.expand(Vec3::from_array(v.pos));
        }
        b
    }
}

/// アップロード済みメッシュの参照。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MeshId(pub u32);

/// フラグメント側の見た目。テクスチャファイルは持たず、ワールド座標の
/// 手続きノイズでアスファルトや草を出す（WebGL2 でもそのまま動く）。
/// `Metal` は既存 shared GGX（コイン）。`Toon` は VRM MToon の shade 段。第二レンダラではない。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Material {
    #[default]
    Solid = 0,
    Road = 1,
    Grass = 2,
    /// カメラ追従の天球。ライティング無しで空のグラデーションを塗る。
    Sky = 3,
    /// 金属（GGX。コイン。metallic=1 / roughness≈0.12）。
    Metal = 4,
    /// VRM MToon shade step (shadeColor + shadingToony). Not a second renderer.
    Toon = 5,
}

/// One of four local lights (`slot=0..3`). Intensity 0 = unused (no slot leak).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
    pub radius: f32,
    pub spot: bool,
}

impl LocalLight {
    pub const OFF: Self = Self {
        position: Vec3::ZERO,
        direction: Vec3::ZERO,
        color: [0.0, 0.0, 0.0],
        intensity: 0.0,
        radius: 0.0,
        spot: false,
    };
}

/// 1 個の描画実体。行列と色だけなので、同じメッシュを大量に並べられる。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    pub model: Mat4,
    /// sRGB の RGBA（0..255）。
    pub color: [u8; 4],
    pub material: Material,
}

impl Instance {
    pub fn new(model: Mat4, color: [u8; 4]) -> Self {
        Self {
            model,
            color,
            material: Material::Solid,
        }
    }

    pub fn with_material(model: Mat4, color: [u8; 4], material: Material) -> Self {
        Self {
            model,
            color,
            material,
        }
    }
}

/// 同一メッシュの実体をまとめたもの。1 バッチが 1 ドローコールになる。
#[derive(Clone, Debug, PartialEq)]
pub struct Batch {
    pub mesh: MeshId,
    pub instances: Vec<Instance>,
}

/// 視点。`aspect` は描画時の画面比なので、ここには持たせない。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    /// 垂直画角（ラジアン）。
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 2.0, 6.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov_y: 60f32.to_radians(),
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl Camera {
    pub fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_at_mat4(self.eye, self.target, self.up)
    }

    /// wgpu の NDC は y が上で奥行きが 0..1 なので、glam の `directx` 系を使う。
    /// `opengl` 系は奥行きが -1..1 になるので合わない。
    pub fn projection(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::directx::perspective(
            self.fov_y,
            aspect.max(1e-3),
            self.near,
            self.far,
        )
    }

    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }
}

/// 軸並行境界箱。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    pub fn from_center_size(center: Vec3, size: Vec3) -> Self {
        let h = size * 0.5;
        Self {
            min: center - h,
            max: center + h,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }

    pub fn expand(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// 行列で変換した後の境界箱。8 頂点を写して囲み直す。
    pub fn transformed(&self, m: Mat4) -> Self {
        if self.is_empty() {
            return *self;
        }
        let mut out = Aabb::empty();
        for i in 0..8 {
            let corner = Vec3::new(
                if i & 1 == 0 { self.min.x } else { self.max.x },
                if i & 2 == 0 { self.min.y } else { self.max.y },
                if i & 4 == 0 { self.min.z } else { self.max.z },
            );
            out.expand(m.transform_point3(corner));
        }
        out
    }
}

/// view-projection から取り出した 6 枚のクリップ平面。
///
/// 各平面は `ax + by + cz + d = 0` の形で、内側が正。
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    pub fn from_view_projection(vp: Mat4) -> Self {
        // 行ベクトルを取り出して和と差を作る、Gribb/Hartmann の方法。
        let r0 = vp.row(0);
        let r1 = vp.row(1);
        let r2 = vp.row(2);
        let r3 = vp.row(3);
        // 奥行きが 0..1 なので near は r2 単体、far は r3 - r2。
        let planes = [
            r3 + r0, // left
            r3 - r0, // right
            r3 + r1, // bottom
            r3 - r1, // top
            r2,      // near
            r3 - r2, // far
        ];
        Self {
            planes: planes.map(normalize_plane),
        }
    }

    /// 箱が視錐台と交差するか。偽陽性は許すが、見えるものを落とさない。
    pub fn intersects(&self, aabb: &Aabb) -> bool {
        if aabb.is_empty() {
            return false;
        }
        for p in &self.planes {
            // 平面から最も遠い角が内側に無ければ、箱全体が外側。
            let n = p.xyz();
            let positive = Vec3::new(
                if n.x >= 0.0 { aabb.max.x } else { aabb.min.x },
                if n.y >= 0.0 { aabb.max.y } else { aabb.min.y },
                if n.z >= 0.0 { aabb.max.z } else { aabb.min.z },
            );
            if n.dot(positive) + p.w < 0.0 {
                return false;
            }
        }
        true
    }
}

fn normalize_plane(p: Vec4) -> Vec4 {
    let len = p.xyz().length();
    if len > 0.0 {
        p / len
    } else {
        p
    }
}

/// 1 フレームぶんの 3D 描画内容。
#[derive(Clone, Debug)]
pub struct Scene3D {
    pub camera: Camera,
    /// 空の色。フォグの色と合わせると地平線が自然につながる。
    pub clear: [u8; 4],
    /// ライトへ向かう方向（正規化済み）。
    pub light_dir: Vec3,
    /// 影側の明るさ（0..1）。
    pub ambient: f32,
    /// Local lights by slot (0 = key). Unused slots are `LocalLight::OFF`.
    pub local_lights: [LocalLight; 4],
    pub fog_color: [u8; 4],
    /// フォグが効き始める距離と、完全に覆う距離。
    pub fog_start: f32,
    pub fog_end: f32,
    pub batches: Vec<Batch>,
}

impl Default for Scene3D {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            clear: [130, 165, 205, 255],
            light_dir: Vec3::new(-0.4, 1.0, 0.3).normalize(),
            ambient: 0.35,
            local_lights: [LocalLight::OFF; 4],
            fog_color: [130, 165, 205, 255],
            fog_start: 120.0,
            fog_end: 420.0,
            batches: Vec::new(),
        }
    }
}

impl Scene3D {
    pub fn instance_count(&self) -> usize {
        self.batches.iter().map(|b| b.instances.len()).sum()
    }
}

/// バッチを組み立てながら視錐台の外を落とすヘルパ。
///
/// メッシュごとの境界箱を覚えておき、`push` のたびに判定する。ワールド生成側は
/// 「置きたいものを全部 push する」だけでよく、カリングの正しさはここで一括して
/// テストできる。
pub struct SceneBuilder {
    frustum: Frustum,
    bounds: Vec<(MeshId, Aabb)>,
    batches: Vec<Batch>,
    culled: usize,
}

impl SceneBuilder {
    pub fn new(camera: &Camera, aspect: f32) -> Self {
        Self {
            frustum: Frustum::from_view_projection(camera.view_projection(aspect)),
            bounds: Vec::new(),
            batches: Vec::new(),
            culled: 0,
        }
    }

    /// メッシュのローカル境界箱を登録する。未登録のメッシュはカリングされない。
    pub fn register(&mut self, mesh: MeshId, bounds: Aabb) {
        match self.bounds.iter_mut().find(|(m, _)| *m == mesh) {
            Some(slot) => slot.1 = bounds,
            None => self.bounds.push((mesh, bounds)),
        }
    }

    pub fn push(&mut self, mesh: MeshId, model: Mat4, color: [u8; 4]) {
        self.push_material(mesh, model, color, Material::Solid);
    }

    pub fn push_material(&mut self, mesh: MeshId, model: Mat4, color: [u8; 4], material: Material) {
        if let Some((_, local)) = self.bounds.iter().find(|(m, _)| *m == mesh) {
            if !self.frustum.intersects(&local.transformed(model)) {
                self.culled += 1;
                return;
            }
        }
        let inst = Instance::with_material(model, color, material);
        match self.batches.iter_mut().find(|b| b.mesh == mesh) {
            Some(b) => b.instances.push(inst),
            None => self.batches.push(Batch {
                mesh,
                instances: vec![inst],
            }),
        }
    }

    /// 視錐台の外だったので捨てた数。デバッグ表示とテスト用。
    pub fn culled(&self) -> usize {
        self.culled
    }

    pub fn finish(self) -> Vec<Batch> {
        self.batches
    }
}

/// 手続き生成のプリミティブ。S5 でローダを入れるまではこれで足りる。
pub mod primitives {
    use super::{MeshData, Vertex3};
    use glam::Vec3;

    /// 原点中心の直方体。面ごとに法線を分けるので陰影がはっきり出る。
    pub fn box_mesh(size: Vec3) -> MeshData {
        let h = size * 0.5;
        let faces: [(Vec3, [Vec3; 4]); 6] = [
            // +X
            (
                Vec3::X,
                [
                    Vec3::new(h.x, -h.y, h.z),
                    Vec3::new(h.x, -h.y, -h.z),
                    Vec3::new(h.x, h.y, -h.z),
                    Vec3::new(h.x, h.y, h.z),
                ],
            ),
            // -X
            (
                Vec3::NEG_X,
                [
                    Vec3::new(-h.x, -h.y, -h.z),
                    Vec3::new(-h.x, -h.y, h.z),
                    Vec3::new(-h.x, h.y, h.z),
                    Vec3::new(-h.x, h.y, -h.z),
                ],
            ),
            // +Y
            (
                Vec3::Y,
                [
                    Vec3::new(-h.x, h.y, h.z),
                    Vec3::new(h.x, h.y, h.z),
                    Vec3::new(h.x, h.y, -h.z),
                    Vec3::new(-h.x, h.y, -h.z),
                ],
            ),
            // -Y
            (
                Vec3::NEG_Y,
                [
                    Vec3::new(-h.x, -h.y, -h.z),
                    Vec3::new(h.x, -h.y, -h.z),
                    Vec3::new(h.x, -h.y, h.z),
                    Vec3::new(-h.x, -h.y, h.z),
                ],
            ),
            // +Z
            (
                Vec3::Z,
                [
                    Vec3::new(-h.x, -h.y, h.z),
                    Vec3::new(h.x, -h.y, h.z),
                    Vec3::new(h.x, h.y, h.z),
                    Vec3::new(-h.x, h.y, h.z),
                ],
            ),
            // -Z
            (
                Vec3::NEG_Z,
                [
                    Vec3::new(h.x, -h.y, -h.z),
                    Vec3::new(-h.x, -h.y, -h.z),
                    Vec3::new(-h.x, h.y, -h.z),
                    Vec3::new(h.x, h.y, -h.z),
                ],
            ),
        ];

        let mut mesh = MeshData::default();
        for (normal, corners) in faces {
            let base = mesh.vertices.len() as u32;
            for c in corners {
                mesh.vertices.push(Vertex3::new(c, normal));
            }
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        mesh
    }

    /// xz 平面に広がる板。原点中心で法線は +Y。
    pub fn plane_mesh(width: f32, depth: f32) -> MeshData {
        let (w, d) = (width * 0.5, depth * 0.5);
        let n = Vec3::Y;
        MeshData {
            vertices: vec![
                Vertex3::new(Vec3::new(-w, 0.0, d), n),
                Vertex3::new(Vec3::new(w, 0.0, d), n),
                Vertex3::new(Vec3::new(w, 0.0, -d), n),
                Vertex3::new(Vec3::new(-w, 0.0, -d), n),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            albedo: None,
            mtoon: None,
        }
    }

    /// Standing XY card (2D sprite in the same WorldDoc as 3D). Origin center,
    /// faces +Z, two-sided so walking around still sees it. Scale is width/height.
    pub fn quad_mesh(width: f32, height: f32) -> MeshData {
        let (hx, hy) = (width * 0.5, height * 0.5);
        let n = Vec3::Z;
        let bn = Vec3::NEG_Z;
        MeshData {
            vertices: vec![
                Vertex3::new(Vec3::new(-hx, -hy, 0.0), n),
                Vertex3::new(Vec3::new(hx, -hy, 0.0), n),
                Vertex3::new(Vec3::new(hx, hy, 0.0), n),
                Vertex3::new(Vec3::new(-hx, hy, 0.0), n),
                Vertex3::new(Vec3::new(-hx, -hy, 0.0), bn),
                Vertex3::new(Vec3::new(hx, -hy, 0.0), bn),
                Vertex3::new(Vec3::new(hx, hy, 0.0), bn),
                Vertex3::new(Vec3::new(-hx, hy, 0.0), bn),
            ],
            indices: vec![0, 1, 2, 0, 2, 3, 4, 6, 5, 4, 7, 6],
            albedo: None,
            mtoon: None,
        }
    }

    /// カメラを包む天球。内側から見るので、スカイ用パイプラインはカリング無し。
    pub fn sky_dome(radius: f32, segments: u32) -> MeshData {
        let segments = segments.max(8);
        let rings = segments / 2;
        let mut mesh = MeshData::default();
        for y in 0..=rings {
            let v = y as f32 / rings as f32;
            let pitch = std::f32::consts::PI * (v - 0.5); // -PI/2 .. +PI/2
            let (sp, cp) = pitch.sin_cos();
            for x in 0..=segments {
                let u = x as f32 / segments as f32;
                let yaw = u * std::f32::consts::TAU;
                let (sy, cy) = yaw.sin_cos();
                let dir = Vec3::new(cy * cp, sp, sy * cp);
                // 法線は外向き。ライティングはスカイでは使わない。
                mesh.vertices.push(Vertex3::new(dir * radius, dir));
            }
        }
        let stride = segments + 1;
        for y in 0..rings {
            for x in 0..segments {
                let i = y * stride + x;
                mesh.indices.extend_from_slice(&[
                    i,
                    i + stride,
                    i + 1,
                    i + 1,
                    i + stride,
                    i + stride + 1,
                ]);
            }
        }
        mesh
    }

    /// 底が xz の円、頂点が +Y の円錐。高さ `height`、底半径 `radius`。原点は底面中心。
    pub fn cone_mesh(radius: f32, height: f32, segments: u32) -> MeshData {
        let segments = segments.max(6);
        let mut mesh = MeshData::default();
        let apex = Vec3::new(0.0, height, 0.0);
        let nrm_up = Vec3::Y;
        // 底面（下向き）
        let base_center = mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex3::new(Vec3::ZERO, Vec3::NEG_Y));
        for i in 0..segments {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            mesh.vertices.push(Vertex3::new(
                Vec3::new(c * radius, 0.0, s * radius),
                Vec3::NEG_Y,
            ));
        }
        for i in 0..segments {
            let a = base_center + 1 + i;
            let b = base_center + 1 + (i + 1) % segments;
            mesh.indices.extend_from_slice(&[base_center, b, a]);
        }
        // 側面
        for i in 0..segments {
            let a0 = i as f32 / segments as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();
            let p0 = Vec3::new(c0 * radius, 0.0, s0 * radius);
            let p1 = Vec3::new(c1 * radius, 0.0, s1 * radius);
            let n = (p0 + p1 + apex).normalize_or(nrm_up);
            let base = mesh.vertices.len() as u32;
            mesh.vertices.push(Vertex3::new(p0, n));
            mesh.vertices.push(Vertex3::new(p1, n));
            mesh.vertices.push(Vertex3::new(apex, n));
            mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
        mesh
    }

    /// Y 軸の円柱。原点は底面中心、高さ `height`、半径 `radius`。
    pub fn cylinder_mesh(radius: f32, height: f32, segments: u32) -> MeshData {
        let segments = segments.max(6);
        let mut mesh = MeshData::default();
        // 側面
        for i in 0..segments {
            let a0 = i as f32 / segments as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();
            let b0 = Vec3::new(c0 * radius, 0.0, s0 * radius);
            let b1 = Vec3::new(c1 * radius, 0.0, s1 * radius);
            let t0 = Vec3::new(c0 * radius, height, s0 * radius);
            let t1 = Vec3::new(c1 * radius, height, s1 * radius);
            let n = Vec3::new(c0 + c1, 0.0, s0 + s1).normalize_or(Vec3::X);
            let base = mesh.vertices.len() as u32;
            mesh.vertices.push(Vertex3::new(b0, n));
            mesh.vertices.push(Vertex3::new(b1, n));
            mesh.vertices.push(Vertex3::new(t1, n));
            mesh.vertices.push(Vertex3::new(t0, n));
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        // 上面・底面
        let top_c = mesh.vertices.len() as u32;
        mesh.vertices
            .push(Vertex3::new(Vec3::new(0.0, height, 0.0), Vec3::Y));
        let bot_c = mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex3::new(Vec3::ZERO, Vec3::NEG_Y));
        for i in 0..segments {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            mesh.vertices.push(Vertex3::new(
                Vec3::new(c * radius, height, s * radius),
                Vec3::Y,
            ));
            mesh.vertices.push(Vertex3::new(
                Vec3::new(c * radius, 0.0, s * radius),
                Vec3::NEG_Y,
            ));
        }
        for i in 0..segments {
            let t0 = top_c + 2 + i * 2;
            let t1 = top_c + 2 + ((i + 1) % segments) * 2;
            let b0 = top_c + 3 + i * 2;
            let b1 = top_c + 3 + ((i + 1) % segments) * 2;
            let _ = bot_c;
            mesh.indices.extend_from_slice(&[top_c, t0, t1]);
            mesh.indices.extend_from_slice(&[bot_c, b1, b0]);
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn cam() -> Camera {
        Camera {
            eye: Vec3::new(0.0, 0.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov_y: FRAC_PI_2,
            near: 0.1,
            far: 100.0,
        }
    }

    #[test]
    fn projection_maps_depth_to_zero_one() {
        let c = cam();
        let vp = c.view_projection(1.0);
        // 手前の点は z=0 付近、遠くの点は z=1 付近に写る。
        let near_pt = vp * Vec3::new(0.0, 0.0, 10.0 - c.near).extend(1.0);
        let far_pt = vp * Vec3::new(0.0, 0.0, 10.0 - c.far).extend(1.0);
        assert!((near_pt.z / near_pt.w).abs() < 1e-3, "near should map to 0");
        assert!(
            (far_pt.z / far_pt.w - 1.0).abs() < 1e-3,
            "far should map to 1"
        );
    }

    #[test]
    fn camera_looks_at_target() {
        let c = cam();
        let vp = c.view_projection(1.0);
        let clip = vp * Vec3::ZERO.extend(1.0);
        let ndc = clip.xyz() / clip.w;
        assert!(
            ndc.x.abs() < 1e-5 && ndc.y.abs() < 1e-5,
            "target off-center"
        );
    }

    #[test]
    fn frustum_keeps_what_is_in_front_and_drops_what_is_behind() {
        let f = Frustum::from_view_projection(cam().view_projection(1.0));
        let front = Aabb::from_center_size(Vec3::ZERO, Vec3::splat(1.0));
        let behind = Aabb::from_center_size(Vec3::new(0.0, 0.0, 60.0), Vec3::splat(1.0));
        let far_away = Aabb::from_center_size(Vec3::new(0.0, 0.0, -200.0), Vec3::splat(1.0));
        let sideways = Aabb::from_center_size(Vec3::new(500.0, 0.0, 0.0), Vec3::splat(1.0));
        assert!(f.intersects(&front));
        assert!(!f.intersects(&behind), "behind the camera must be culled");
        assert!(!f.intersects(&far_away), "beyond far plane must be culled");
        assert!(!f.intersects(&sideways), "off to the side must be culled");
    }

    #[test]
    fn huge_box_straddling_the_camera_is_kept() {
        // 視錐台より大きい床板は、角が全部外でも見えている。
        let f = Frustum::from_view_projection(cam().view_projection(1.0));
        let ground =
            Aabb::from_center_size(Vec3::new(0.0, -1.0, 0.0), Vec3::new(1000.0, 0.1, 1000.0));
        assert!(f.intersects(&ground));
    }

    #[test]
    fn transformed_bounds_follow_translation() {
        let b = Aabb::from_center_size(Vec3::ZERO, Vec3::splat(2.0));
        let moved = b.transformed(Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)));
        assert!((moved.center().x - 10.0).abs() < 1e-5);
    }

    #[test]
    fn rotated_bounds_grow_to_contain() {
        let b = Aabb::from_center_size(Vec3::ZERO, Vec3::new(2.0, 0.1, 0.1));
        let rotated = b.transformed(Mat4::from_rotation_y(std::f32::consts::FRAC_PI_4));
        // 45 度回すと x も z も伸びる。
        assert!(rotated.max.x > 0.6 && rotated.max.z > 0.6);
    }

    #[test]
    fn builder_groups_instances_and_culls() {
        let c = cam();
        let mut b = SceneBuilder::new(&c, 1.0);
        let mesh = MeshId(0);
        b.register(mesh, Aabb::from_center_size(Vec3::ZERO, Vec3::ONE));
        b.push(mesh, Mat4::IDENTITY, [255, 0, 0, 255]);
        b.push(
            mesh,
            Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)),
            [0, 255, 0, 255],
        );
        b.push(
            mesh,
            Mat4::from_translation(Vec3::new(0.0, 0.0, 900.0)),
            [0, 0, 255, 255],
        );
        assert_eq!(b.culled(), 1);
        let batches = b.finish();
        assert_eq!(batches.len(), 1, "same mesh should share one batch");
        assert_eq!(batches[0].instances.len(), 2);
    }

    #[test]
    fn unregistered_mesh_is_never_culled() {
        let mut b = SceneBuilder::new(&cam(), 1.0);
        b.push(
            MeshId(7),
            Mat4::from_translation(Vec3::new(0.0, 0.0, 5000.0)),
            [255; 4],
        );
        assert_eq!(b.culled(), 0);
        assert_eq!(b.finish()[0].instances.len(), 1);
    }

    #[test]
    fn box_mesh_has_six_quads_and_outward_normals() {
        let m = primitives::box_mesh(Vec3::splat(2.0));
        assert_eq!(m.vertices.len(), 24);
        assert_eq!(m.indices.len(), 36);
        for v in &m.vertices {
            let p = Vec3::from_array(v.pos);
            let n = Vec3::from_array(v.normal);
            assert!(p.dot(n) > 0.0, "normal should point away from the center");
        }
        let b = m.bounds();
        assert!((b.min + Vec3::ONE).length() < 1e-5);
        assert!((b.max - Vec3::ONE).length() < 1e-5);
    }

    #[test]
    fn plane_mesh_is_flat_and_faces_up() {
        let m = primitives::plane_mesh(10.0, 4.0);
        assert!(m.vertices.iter().all(|v| v.pos[1] == 0.0));
        assert!(m.vertices.iter().all(|v| v.normal == [0.0, 1.0, 0.0]));
        let b = m.bounds();
        assert_eq!(b.max.x, 5.0);
        assert_eq!(b.max.z, 2.0);
    }

    #[test]
    fn quad_mesh_is_vertical_and_faces_plus_z() {
        let m = primitives::quad_mesh(2.0, 3.0);
        assert!(m.vertices.iter().all(|v| v.pos[2].abs() < 1e-6));
        assert_eq!(m.vertices.len(), 8);
        assert!(m
            .vertices
            .iter()
            .take(4)
            .all(|v| v.normal == [0.0, 0.0, 1.0]));
        assert!(m
            .vertices
            .iter()
            .skip(4)
            .all(|v| v.normal == [0.0, 0.0, -1.0]));
        let b = m.bounds();
        assert!((b.max.x - 1.0).abs() < 1e-5);
        assert!((b.max.y - 1.5).abs() < 1e-5);
        assert!(b.max.z.abs() < 1e-5);
        assert_eq!(m.indices.len(), 12);
        assert!(m.vertices.iter().all(|v| v.uv == [0.0, 0.0]));
        assert!(m.albedo.is_none());
    }

    #[test]
    fn vertex3_uv_keeps_compatible_stride() {
        assert_eq!(std::mem::size_of::<Vertex3>(), 32);
        let cap = primitives::cylinder_mesh(0.5, 1.0, 12);
        assert!(cap.vertices.iter().all(|v| v.uv == [0.0, 0.0]));
        assert!(cap.albedo.is_none());
        let plane = primitives::plane_mesh(1.0, 1.0);
        assert!(plane.vertices.iter().all(|v| v.uv == [0.0, 0.0]));
    }
    #[test]
    fn toon_material_id_is_five() {
        assert_eq!(Material::Toon as u8, 5);
        assert_eq!(Material::Metal as u8, 4);
        let plane = primitives::plane_mesh(1.0, 1.0);
        assert!(plane.mtoon.is_none());
    }
}
