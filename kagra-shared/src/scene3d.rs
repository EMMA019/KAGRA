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

use std::sync::Arc;

use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};

/// 位置 + 法線 + UV + 頂点色 + 接線。
///
/// `color` はリニア（glTF COLOR_0）。手続きメッシュは白。
/// `tangent` は xyz + 向き（glTF TANGENT の w）。長さ 0 は接線なし。
/// box / plane / quad / cylinder の UV は 0..1。未指定は [0,0]（1x1 白）。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex3 {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub tangent: [f32; 4],
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
            color: [1.0, 1.0, 1.0, 1.0],
            tangent: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

/// glTF サンプラのラップ。手続きメッシュは `Clamp`（従来の見た目）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TexWrap {
    #[default]
    Clamp,
    Repeat,
    Mirror,
}

/// GPU に依存しないテクスチャサンプラ。baseColor のサンプラがメッシュ全体に勝つ。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexSampler {
    pub wrap_u: TexWrap,
    pub wrap_v: TexWrap,
    pub mag_nearest: bool,
    pub min_nearest: bool,
    pub mip_nearest: bool,
    /// 1 は異方性なし。アップロード時にデバイス上限で切る。
    pub anisotropy: u8,
}

impl Default for TexSampler {
    fn default() -> Self {
        Self {
            wrap_u: TexWrap::Clamp,
            wrap_v: TexWrap::Clamp,
            mag_nearest: false,
            min_nearest: false,
            mip_nearest: false,
            anisotropy: 4,
        }
    }
}

impl TexSampler {
    /// glTF がサンプラを省略したときの規定（REPEAT + リニアミップ）。
    pub fn gltf_default() -> Self {
        Self {
            wrap_u: TexWrap::Repeat,
            wrap_v: TexWrap::Repeat,
            ..Self::default()
        }
    }
}

/// RGBA8 baseColor（glTF / VRM）。無しならレンダラが 1x1 白を貼る。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbedoRgba {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

/// MToon shade (VRM 0 materialProperties / VRM 1 VRMC_materials_mtoon).
/// Port of kagra-core mtoon.rs minus GPU: shade + rim + outline + matcap/normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MtoonShade {
    pub shade_color: [f32; 3],
    pub shading_toony: f32,
    pub shading_shift: f32,
    pub rim_color: [f32; 3],
    pub rim_power: f32,
    pub rim_lift: f32,
    pub outline_color: [f32; 3],
    pub outline_width: f32,
    /// matcap / normal textures present (flags ride gpu_shift_lift z/w).
    pub has_matcap: bool,
    pub has_normal: bool,
}

impl Default for MtoonShade {
    fn default() -> Self {
        Self {
            shade_color: [0.55, 0.50, 0.52],
            shading_toony: 0.85,
            shading_shift: 0.0,
            rim_color: [0.0, 0.0, 0.0],
            rim_power: 5.0,
            rim_lift: 0.0,
            outline_color: [0.05, 0.05, 0.08],
            outline_width: 0.0,
            has_matcap: false,
            has_normal: false,
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

    /// GPU instance location 10: rgb = rimColor, a = rimFresnelPower.
    pub fn gpu_rim(self) -> [f32; 4] {
        [
            self.rim_color[0],
            self.rim_color[1],
            self.rim_color[2],
            self.rim_power.max(0.1),
        ]
    }

    /// GPU instance location 11: rgb = outlineColor, a = outlineWidth.
    pub fn gpu_outline(self) -> [f32; 4] {
        [
            self.outline_color[0],
            self.outline_color[1],
            self.outline_color[2],
            self.outline_width.max(0.0),
        ]
    }

    /// Shift/toony/lift packed for shader uniforms use.
    /// z = hasMatcap, w = hasNormal (ride along so no extra instance slot).
    pub fn gpu_shift_lift(self) -> [f32; 4] {
        [
            self.shading_shift,
            self.rim_lift,
            if self.has_matcap { 1.0 } else { 0.0 },
            if self.has_normal { 1.0 } else { 0.0 },
        ]
    }
}

/// CPU 側のメッシュ。`Renderer::upload_mesh` で GPU に載せて `MeshId` を得る。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    pub vertices: Vec<Vertex3>,
    pub indices: Vec<u32>,
    pub albedo: Option<AlbedoRgba>,
    /// Pre-encoded KTX2 base color. Used when the device has the matching
    /// compression feature. An RGBA `albedo` is the fallback.
    pub albedo_ktx: Option<crate::ktx_image::KtxImage>,
    /// MToon matcap (SphereAdd). Present for hair with a matcap texture.
    pub matcap: Option<AlbedoRgba>,
    /// MToon / glTF normal map (RGBA, blue channel is the bump).
    pub normal: Option<AlbedoRgba>,
    /// Present when the glTF/VRM primitive authored MToon shade.
    pub mtoon: Option<MtoonShade>,
    /// Linear ORM image. G = roughness, B = metallic. Flags say which channel wins.
    pub orm: Option<AlbedoRgba>,
    pub orm_rough: bool,
    pub orm_metal: bool,
    /// Emissive color map (sRGB bytes). Strength stays on the instance.
    pub emissive_map: Option<AlbedoRgba>,
    /// Shader alpha sentinel.
    /// 0 = discard when `tex.a < 0.5` (procedural meshes).
    /// `> 0` = MASK cutoff. `-1` = OPAQUE (no discard). `<= -1.5` = BLEND.
    pub alpha_cut: f32,
    pub double_sided: bool,
    pub sampler: TexSampler,
    /// glTF emissiveStrength. The dump wins when it sets `emissive`.
    pub emissive: f32,
    /// Linear. White when the factor is already baked into `emissive_map`.
    pub emissive_color: [f32; 3],
    /// `KHR_materials_clearcoat` factor. 0 leaves the base lobe. A dump
    /// `clearcoat` above 0 replaces this at draw time.
    pub clearcoat: f32,
    /// Clearcoat roughness from the same extension. 0 in the shader means 0.1.
    pub clearcoat_roughness: f32,
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
    /// Water: procedural waves + Fresnel + IBL reflection. Same shader family, not a second renderer.
    Water = 6,
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
    pub metallic: f32,
    pub roughness: f32,
    /// Unlit HDR add, multiplied by `emissive_color` (or albedo when that is black).
    /// 0 keeps the old picture. Bloom extracts whatever this pushes over the threshold.
    pub emissive: f32,
    /// Linear 0..1. All zero means "use the shaded albedo".
    pub emissive_color: [f32; 3],
    /// Dump set `emissive` or `emissive_color`. Otherwise the mesh glTF value is used.
    pub emissive_authored: bool,
    /// 0..1 clearcoat lobe. 0 leaves the base lighting.
    pub clearcoat: f32,
    /// Clearcoat roughness. 0 in the shader means 0.1.
    pub clearcoat_roughness: f32,
}

impl Instance {
    pub fn new(model: Mat4, color: [u8; 4]) -> Self {
        Self {
            model,
            color,
            material: Material::Solid,
            metallic: 0.0,
            roughness: 0.42,
            emissive: 0.0,
            emissive_color: [0.0; 3],
            emissive_authored: false,
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
        }
    }

    pub fn with_material(model: Mat4, color: [u8; 4], material: Material) -> Self {
        let (metallic, roughness) = match material {
            Material::Metal => (1.0, 0.12),
            Material::Grass | Material::Road => (0.0, 0.85),
            _ => (0.0, 0.42),
        };
        Self {
            model,
            color,
            material,
            metallic,
            roughness,
            emissive: 0.0,
            emissive_color: [0.0; 3],
            emissive_authored: false,
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
        }
    }

    pub fn with_pbr(
        model: Mat4,
        color: [u8; 4],
        material: Material,
        metallic: f32,
        roughness: f32,
    ) -> Self {
        Self {
            model,
            color,
            material,
            metallic: metallic.clamp(0.0, 1.0),
            roughness: roughness.clamp(0.04, 1.0),
            emissive: 0.0,
            emissive_color: [0.0; 3],
            emissive_authored: false,
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
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

    /// wgpu NDC: +x is right, +y is up, depth 0..1. Use glam's DirectX RH
    /// projection (OpenGL's depth is -1..1). +x in view space is screen-right;
    /// the readback does not mirror X.
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

/// Right-handed view basis. `screen_right` is `forward × up` (the same side
/// `look_at_rh` puts on the right of the frame).
///
/// World +X is that direction only when the camera looks toward -Z. The
/// default camera (`eye` (0, 2, 6), target origin) does. A chase camera
/// looking along +Z has `screen_right = -X`.
pub fn screen_basis(eye: Vec3, target: Vec3, up: Vec3) -> (Vec3, Vec3, Vec3) {
    let mut forward = target - eye;
    if forward.length_squared() < 1e-12 {
        forward = Vec3::NEG_Z;
    } else {
        forward = forward.normalize();
    }
    let mut up = if up.length_squared() < 1e-12 {
        Vec3::Y
    } else {
        up.normalize()
    };
    let mut right = forward.cross(up);
    if right.length_squared() < 1e-8 {
        let fallback = if forward.y.abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        up = fallback;
        right = forward.cross(up);
    }
    right = right.normalize();
    let up = right.cross(forward);
    (forward, right, up)
}

/// World-space unit vector toward the right edge of the frame.
pub fn screen_right(eye: Vec3, target: Vec3) -> Vec3 {
    screen_basis(eye, target, Vec3::Y).1
}

/// World-space unit vector toward the left edge of the frame.
pub fn screen_left(eye: Vec3, target: Vec3) -> Vec3 {
    -screen_right(eye, target)
}

/// [`screen_right`] for dump arrays. `+X` is right only when `target.z < eye.z`
/// (the camera looks toward -Z).
pub fn screen_right_xyz(eye: [f32; 3], target: [f32; 3]) -> [f32; 3] {
    screen_right(Vec3::from_array(eye), Vec3::from_array(target)).to_array()
}

/// [`screen_left`] for dump arrays.
pub fn screen_left_xyz(eye: [f32; 3], target: [f32; 3]) -> [f32; 3] {
    let right = screen_right_xyz(eye, target);
    [-right[0], -right[1], -right[2]]
}

/// Stick into a world XZ wish. `+lx` is screen-right, `+lz` is into the frame
/// (the camera's forward, flattened). This is not "D means world +X".
pub fn wish_world_xz(lx: f32, lz: f32, eye: Vec3, target: Vec3) -> Vec3 {
    let (forward, right, _) = screen_basis(eye, target, Vec3::Y);
    let flat_fwd = Vec3::new(forward.x, 0.0, forward.z);
    let flat_right = Vec3::new(right.x, 0.0, right.z);
    let fwd = if flat_fwd.length_squared() < 1e-8 {
        Vec3::NEG_Z
    } else {
        flat_fwd.normalize()
    };
    let right = if flat_right.length_squared() < 1e-8 {
        Vec3::X
    } else {
        flat_right.normalize()
    };
    right * lx + fwd * lz
}

/// One directional map. 2048 matches `shader3d.wgsl` PCF texel (WebGL2 max).
pub const SHADOW_MAP_SIZE: u32 = 2048;
/// Ortho half-extent around the look target. Covers Crest contact + nearby casters.
pub const SHADOW_ORTHO_HALF: f32 = 24.0;

/// Snap XZ to shadow texels so the umbra does not crawl when the eye moves.
pub fn snap_shadow_center_xz(center: Vec3, half: f32, map_size: f32) -> Vec3 {
    let texel = (2.0 * half.max(0.5)) / map_size.max(1.0);
    if texel < 1e-8 {
        return center;
    }
    Vec3::new(
        (center.x / texel).round() * texel,
        center.y,
        (center.z / texel).round() * texel,
    )
}

/// Directional sun ortho, DirectX/WebGPU Z 0..1. `light_dir` is toward the light.
pub fn directional_shadow_view_proj(light_dir: Vec3, focus: Vec3, half: f32) -> Mat4 {
    let dir = if light_dir.length_squared() < 1e-12 {
        Vec3::Y
    } else {
        light_dir.normalize()
    };
    let half = half.max(0.5);
    let light_dist = half + 8.0;
    let far = light_dist + half + 8.0;
    let eye = focus + dir * light_dist;
    let up = if dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
    let view = glam::camera::rh::view::look_at_mat4(eye, focus, up);
    let proj = glam::camera::rh::proj::directx::orthographic(-half, half, -half, half, 0.1, far);
    proj * view
}

/// Ortho half-extents for up to three cascades. Count 1 repeats the historical
/// 24 m map so an omitted dump does not change the shadow.
pub fn cascade_halves(count: u32) -> [f32; 3] {
    let n = count.clamp(1, 3);
    if n == 1 {
        return [SHADOW_ORTHO_HALF; 3];
    }
    // Tight contact ring, then a wider ring. Count 2 used to stop at the
    // historical 24 m map, which left a chase camera's next gate (about 40 m
    // ahead) fully lit. The second tile now matches the 80 m far ring so the
    // ship, the rocks, and the rings in frame can receive the sun. Count 3
    // keeps the 24 m middle tile. Same 2048 map, snapped per ring.
    if n == 2 {
        return [8.0, 80.0, 80.0];
    }
    [8.0, SHADOW_ORTHO_HALF, 80.0]
}

/// One view-projection per cascade. Slot 0 matches [`scene_shadow_view_proj`]
/// when `count` is 1.
pub fn scene_cascade_view_projs(scene: &Scene3D, count: u32) -> [Mat4; 3] {
    let n = count.clamp(1, 3);
    let halves = cascade_halves(n);
    let t = scene.camera.target;
    // Count 1 snaps to the full 2048 map. Extra rings render into 1024 tiles,
    // so the snap grid matches the texel those tiles actually draw.
    let map = if n <= 1 {
        SHADOW_MAP_SIZE as f32
    } else {
        (SHADOW_MAP_SIZE / 2) as f32
    };
    let mut out = [Mat4::IDENTITY; 3];
    for i in 0..3 {
        let focus = snap_shadow_center_xz(Vec3::new(t.x, t.y.max(1.0), t.z), halves[i], map);
        out[i] = directional_shadow_view_proj(scene.light_dir, focus, halves[i]);
    }
    out
}

/// Pixel rect of cascade `index` inside the 2048 map. Count 1 uses the full map.
pub fn cascade_viewport(index: u32, count: u32) -> (f32, f32, f32, f32) {
    let size = SHADOW_MAP_SIZE as f32;
    if count <= 1 {
        return (0.0, 0.0, size, size);
    }
    let tile = size * 0.5;
    let i = index.min(2);
    let col = (i % 2) as f32;
    let row = (i / 2) as f32;
    (col * tile, row * tile, tile, tile)
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
    /// Diffuse IBL strength. 0 = off. Default 0.35 (studio outdoor, V2 set_hdri).
    pub ibl: f32,
    /// Linear exposure. Default 1.0 (V2 set_exposure).
    pub exposure: f32,
    /// ACES tonemap. Default on so outdoor dumps are not key+fill-flat.
    pub tonemap: bool,
    /// 0 ACES, 1 AgX, 2 Neutral. Ignored while `tonemap` is false.
    pub tonemap_code: f32,
    /// Procedural atmosphere on the sky sphere. False keeps the studio gradient.
    pub sky_atmosphere: bool,
    /// Directional shadow radius. 1 = the historical 3×3 PCF. Larger = a wider tent.
    pub shadow_softness: f32,
    /// 1 = the historical single map. 2 or 3 pack extra rings into the same map.
    pub shadow_cascades: u32,
    /// Contact-hardening (blocker search). Off unless a dump or the high tier asks.
    pub shadow_pcss: bool,
    /// Hemisphere sky color. Default is the studio blue that the SH used to hardcode.
    pub hemi_sky: [f32; 3],
    /// Hemisphere ground color. Default is the studio warm ground.
    pub hemi_ground: [f32; 3],
    /// 64×64 linear RGBA prefiltered lat-long (4 roughness rows). None = SH specular.
    pub env_pixels: Option<Vec<u8>>,
    /// Camera-facing point sprites. The renderer draws them after the meshes.
    pub points: Vec<crate::strokes::PointCloud>,
    /// Screen-space fat lines. The renderer draws them after the meshes.
    pub lines: Vec<crate::strokes::FatLine>,
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
            ibl: 0.35,
            exposure: 1.0,
            tonemap: true,
            tonemap_code: 0.0,
            sky_atmosphere: false,
            shadow_softness: 1.0,
            shadow_cascades: 1,
            shadow_pcss: false,
            hemi_sky: [0.48, 0.56, 0.78],
            hemi_ground: [0.42, 0.30, 0.18],
            env_pixels: None,
            points: Vec::new(),
            lines: Vec::new(),
            batches: Vec::new(),
        }
    }
}

/// Draw-list stats for one compiled frame. Not dumped (GPU ids are not game objects).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    pub batches: u32,
    pub instances: u32,
}

impl Scene3D {
    pub fn instance_count(&self) -> usize {
        self.batches.iter().map(|b| b.instances.len()).sum()
    }

    /// Batch + instance counts after GPU instancing (same mesh+material = one draw).
    pub fn render_stats(&self) -> RenderStats {
        RenderStats {
            batches: self.batches.len() as u32,
            instances: self.instance_count() as u32,
        }
    }
}

/// Camera-centered map for a compiled scene (play_world / offscreen).
pub fn scene_shadow_view_proj(scene: &Scene3D) -> Mat4 {
    let t = scene.camera.target;
    let focus = snap_shadow_center_xz(
        Vec3::new(t.x, t.y.max(1.0), t.z),
        SHADOW_ORTHO_HALF,
        SHADOW_MAP_SIZE as f32,
    );
    directional_shadow_view_proj(scene.light_dir, focus, SHADOW_ORTHO_HALF)
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
        let inst = Instance::with_material(model, color, material);
        self.push_instance(mesh, inst);
    }

    pub fn push_pbr(
        &mut self,
        mesh: MeshId,
        model: Mat4,
        color: [u8; 4],
        material: Material,
        metallic: f32,
        roughness: f32,
    ) {
        let inst = Instance::with_pbr(model, color, material, metallic, roughness);
        self.push_instance(mesh, inst);
    }

    pub fn push_instance(&mut self, mesh: MeshId, inst: Instance) {
        if let Some((_, local)) = self.bounds.iter().find(|(m, _)| *m == mesh) {
            if !self.frustum.intersects(&local.transformed(inst.model)) {
                self.culled += 1;
                return;
            }
        }
        let material = inst.material;
        match self
            .batches
            .iter_mut()
            .find(|b| b.mesh == mesh && b.instances.first().is_some_and(|i| i.material == material))
        {
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

        let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        let mut mesh = MeshData::default();
        for (normal, corners) in faces {
            let base = mesh.vertices.len() as u32;
            for (c, uv) in corners.into_iter().zip(uvs) {
                mesh.vertices.push(Vertex3::with_uv(c, normal, uv));
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
                Vertex3::with_uv(Vec3::new(-w, 0.0, d), n, [0.0, 0.0]),
                Vertex3::with_uv(Vec3::new(w, 0.0, d), n, [1.0, 0.0]),
                Vertex3::with_uv(Vec3::new(w, 0.0, -d), n, [1.0, 1.0]),
                Vertex3::with_uv(Vec3::new(-w, 0.0, -d), n, [0.0, 1.0]),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            ..MeshData::default()
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
                Vertex3::with_uv(Vec3::new(-hx, -hy, 0.0), n, [0.0, 1.0]),
                Vertex3::with_uv(Vec3::new(hx, -hy, 0.0), n, [1.0, 1.0]),
                Vertex3::with_uv(Vec3::new(hx, hy, 0.0), n, [1.0, 0.0]),
                Vertex3::with_uv(Vec3::new(-hx, hy, 0.0), n, [0.0, 0.0]),
                Vertex3::with_uv(Vec3::new(-hx, -hy, 0.0), bn, [1.0, 1.0]),
                Vertex3::with_uv(Vec3::new(hx, -hy, 0.0), bn, [0.0, 1.0]),
                Vertex3::with_uv(Vec3::new(hx, hy, 0.0), bn, [0.0, 0.0]),
                Vertex3::with_uv(Vec3::new(-hx, hy, 0.0), bn, [1.0, 0.0]),
            ],
            indices: vec![0, 1, 2, 0, 2, 3, 4, 6, 5, 4, 7, 6],
            ..MeshData::default()
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
            let u0 = i as f32 / segments as f32;
            let u1 = (i + 1) as f32 / segments as f32;
            let base = mesh.vertices.len() as u32;
            mesh.vertices.push(Vertex3::with_uv(b0, n, [u0, 1.0]));
            mesh.vertices.push(Vertex3::with_uv(b1, n, [u1, 1.0]));
            mesh.vertices.push(Vertex3::with_uv(t1, n, [u1, 0.0]));
            mesh.vertices.push(Vertex3::with_uv(t0, n, [u0, 0.0]));
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

    /// Ring in the XY plane (hole along +Z) so a `yaw` of 0 faces a +Z flight path.
    /// Major radius 0.5, minor radius given. Origin is the ring center.
    pub fn torus_mesh(major: f32, minor: f32, major_seg: u32, minor_seg: u32) -> MeshData {
        let major_seg = major_seg.max(8);
        let minor_seg = minor_seg.max(6);
        let mut mesh = MeshData::default();
        for i in 0..major_seg {
            let u0 = i as f32 / major_seg as f32 * std::f32::consts::TAU;
            let u1 = (i + 1) as f32 / major_seg as f32 * std::f32::consts::TAU;
            for j in 0..minor_seg {
                let v0 = j as f32 / minor_seg as f32 * std::f32::consts::TAU;
                let v1 = (j + 1) as f32 / minor_seg as f32 * std::f32::consts::TAU;
                let ring = |u: f32, v: f32| -> (Vec3, Vec3) {
                    let (su, cu) = u.sin_cos();
                    let (sv, cv) = v.sin_cos();
                    let n = Vec3::new(cu * cv, su * cv, sv);
                    let pos = Vec3::new(major * cu, major * su, 0.0) + n * minor;
                    (pos, n)
                };
                let (p00, n00) = ring(u0, v0);
                let (p10, n10) = ring(u1, v0);
                let (p11, n11) = ring(u1, v1);
                let (p01, n01) = ring(u0, v1);
                let base = mesh.vertices.len() as u32;
                mesh.vertices.push(Vertex3::new(p00, n00));
                mesh.vertices.push(Vertex3::new(p10, n10));
                mesh.vertices.push(Vertex3::new(p11, n11));
                mesh.vertices.push(Vertex3::new(p01, n01));
                mesh.indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 2,
                    base,
                    base + 2,
                    base + 3,
                ]);
            }
        }
        mesh
    }

    /// 外側から見る球体メッシュ（反時計回りカリング対応）。
    pub fn sphere_mesh(radius: f32, segments: u32) -> MeshData {
        let segments = segments.max(8);
        let rings = segments / 2;
        let mut mesh = MeshData::default();
        for y in 0..=rings {
            let v = y as f32 / rings as f32;
            let pitch = std::f32::consts::PI * (v - 0.5);
            let (sp, cp) = pitch.sin_cos();
            for x in 0..=segments {
                let u = x as f32 / segments as f32;
                let yaw = u * std::f32::consts::TAU;
                let (sy, cy) = yaw.sin_cos();
                let dir = Vec3::new(cy * cp, sp, sy * cp);
                mesh.vertices
                    .push(Vertex3::with_uv(dir * radius, dir, [u, 1.0 - v]));
            }
        }
        let stride = segments + 1;
        for y in 0..rings {
            for x in 0..segments {
                let i = y * stride + x;
                mesh.indices.extend_from_slice(&[
                    i,
                    i + 1,
                    i + stride,
                    i + 1,
                    i + stride + 1,
                    i + stride,
                ]);
            }
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
    fn directional_shadow_maps_focus_and_caster() {
        let light = Vec3::new(-0.4, 1.0, 0.3).normalize();
        let focus = Vec3::new(0.0, 1.0, 0.0);
        let vp = directional_shadow_view_proj(light, focus, SHADOW_ORTHO_HALF);
        let clip = vp * focus.extend(1.0);
        let ndc = clip.xyz() / clip.w;
        assert!(
            ndc.x.abs() < 1e-4 && ndc.y.abs() < 1e-4,
            "focus off-center {ndc}"
        );
        assert!(
            ndc.z > 0.0 && ndc.z < 1.0,
            "focus depth should sit inside the ortho, got {}",
            ndc.z
        );
        // A capsule standing on the focus casts along -light onto the ground.
        let ground = Vec3::new(0.0, 0.0, 0.0);
        let gclip = vp * ground.extend(1.0);
        let gndc = gclip.xyz() / gclip.w;
        assert!(
            gndc.x.abs() < 1.0 && gndc.y.abs() < 1.0 && gndc.z > 0.0 && gndc.z < 1.0,
            "ground at focus must be on the map {gndc}"
        );
    }

    #[test]
    fn shadow_center_snap_is_stable_for_sub_texel_eye_move() {
        let half = SHADOW_ORTHO_HALF;
        let map = SHADOW_MAP_SIZE as f32;
        let texel = (2.0 * half) / map;
        let a = snap_shadow_center_xz(Vec3::new(4.0, 1.5, -2.0), half, map);
        let b = snap_shadow_center_xz(
            Vec3::new(a.x + texel * 0.2, a.y, a.z + texel * 0.2),
            half,
            map,
        );
        assert!((a.x - b.x).abs() < 1e-5 && (a.z - b.z).abs() < 1e-5);
    }

    #[test]
    fn one_cascade_matches_the_historical_map() {
        let scene = Scene3D::default();
        let one = scene_cascade_view_projs(&scene, 1);
        let historical = scene_shadow_view_proj(&scene);
        assert_eq!(one[0].to_cols_array(), historical.to_cols_array());
        assert_eq!(one[1].to_cols_array(), one[0].to_cols_array());
        let (x, y, w, h) = cascade_viewport(0, 1);
        assert_eq!(
            (x, y, w, h),
            (0.0, 0.0, SHADOW_MAP_SIZE as f32, SHADOW_MAP_SIZE as f32)
        );
        let halves = cascade_halves(3);
        assert!(halves[0] < halves[1] && halves[1] < halves[2]);
        let two = cascade_halves(2);
        assert!(
            two[0] < 16.0 && two[1] >= 64.0 && (two[1] - two[2]).abs() < 1e-4,
            "count 2 must reach the next gate, got {two:?}"
        );
        let (x2, _, w2, _) = cascade_viewport(1, 3);
        assert!(x2 > 0.0 && w2 < SHADOW_MAP_SIZE as f32);
        let three = scene_cascade_view_projs(&scene, 3);
        assert_ne!(three[0].to_cols_array(), three[2].to_cols_array());
    }

    #[test]
    fn default_camera_puts_plus_x_on_screen_right() {
        let c = Camera::default();
        let right = screen_right(c.eye, c.target);
        assert!(
            right.x > 0.99 && right.y.abs() < 1e-4 && right.z.abs() < 1e-4,
            "default camera (looking -Z) screen_right={right}"
        );
        let vp = c.view_projection(16.0 / 9.0);
        let plus = vp * Vec3::new(1.5, 0.0, 0.0).extend(1.0);
        let minus = vp * Vec3::new(-1.5, 0.0, 0.0).extend(1.0);
        let plus_x = plus.x / plus.w;
        let minus_x = minus.x / minus.w;
        assert!(
            plus_x > 0.05 && minus_x < -0.05 && plus_x > minus_x,
            "+X must be NDC-right of -X, ndc {plus_x} vs {minus_x}"
        );
    }

    #[test]
    fn looking_plus_z_puts_plus_x_on_screen_left() {
        let eye = Vec3::new(0.0, 2.0, -6.0);
        let target = Vec3::ZERO;
        let right = screen_right(eye, target);
        assert!(right.x < -0.99, "chase along +Z screen_right={right}");
        let wish = wish_world_xz(1.0, 0.0, eye, target);
        assert!(
            wish.dot(right) > 0.9,
            "+lx must follow screen_right, wish={wish} right={right}"
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
    fn builder_instances_n_trees_as_one_batch() {
        let c = cam();
        let mut b = SceneBuilder::new(&c, 1.0);
        let mesh = MeshId(0);
        b.register(mesh, Aabb::from_center_size(Vec3::ZERO, Vec3::ONE));
        for i in 0..32 {
            b.push(
                mesh,
                Mat4::from_translation(Vec3::new(i as f32 * 0.15, 0.0, 0.0)),
                [46, 120, 52, 255],
            );
        }
        let batches = b.finish();
        assert_eq!(batches.len(), 1, "N trees of the same mesh = 1 GPU batch");
        assert_eq!(batches[0].instances.len(), 32);
        let stats = Scene3D {
            batches: batches.clone(),
            ..Scene3D::default()
        }
        .render_stats();
        assert_eq!(stats.batches, 1);
        assert_eq!(stats.instances, 32);
    }

    #[test]
    fn builder_splits_batches_by_material() {
        let mut b = SceneBuilder::new(&cam(), 1.0);
        b.push_material(MeshId(0), Mat4::IDENTITY, [255; 4], Material::Solid);
        b.push_material(
            MeshId(0),
            Mat4::IDENTITY,
            [20, 80, 110, 255],
            Material::Water,
        );
        let batches = b.finish();
        assert_eq!(batches.len(), 2, "water and solid must not share a draw");
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
        assert!(
            m.vertices.iter().any(|v| v.uv == [0.0, 0.0])
                && m.vertices.iter().any(|v| v.uv == [1.0, 1.0]),
            "quad UVs cover the PNG"
        );
        assert!(m.albedo.is_none());
    }

    #[test]
    fn vertex3_uv_keeps_compatible_stride() {
        assert_eq!(std::mem::size_of::<Vertex3>(), 64);
        let cap = primitives::cylinder_mesh(0.5, 1.0, 12);
        assert!(
            cap.vertices.iter().any(|v| v.uv[0] > 0.0 && v.uv[1] > 0.0),
            "cylinder sides unwrap so a bottle PNG shows"
        );
        assert!(cap.albedo.is_none());
        let plane = primitives::plane_mesh(1.0, 1.0);
        assert!(
            plane.vertices.iter().any(|v| v.uv == [0.0, 0.0])
                && plane.vertices.iter().any(|v| v.uv == [1.0, 1.0]),
            "plane UVs cover the PNG"
        );
    }
    #[test]
    fn toon_material_id_is_five() {
        assert_eq!(Material::Toon as u8, 5);
        assert_eq!(Material::Metal as u8, 4);
        assert_eq!(Material::Water as u8, 6);
        let plane = primitives::plane_mesh(1.0, 1.0);
        assert!(plane.mtoon.is_none());
    }
}
