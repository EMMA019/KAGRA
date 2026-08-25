// Draw commands & vertex formats
use std::sync::Arc;
use crate::color::Color;

// ---------- 描画コマンド ----------
#[derive(Clone)]
pub struct RectCommand {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Color,
}

#[derive(Clone)]
pub struct SpriteCommand {
    pub texture_id: u32,
    pub shader_id: u32,
    pub shader_params: [f32; 4],
    pub dx: f32,
    pub dy: f32,
    pub dw: f32,
    pub dh: f32,
    pub sx: f32,
    pub sy: f32,
    pub sw: f32,
    pub sh: f32,
    pub alpha: f32,
    pub rotation_deg: f32,
    pub pivot_x: f32,
    pub pivot_y: f32,
    pub flip_x: bool,
    pub flip_y: bool,
}

#[derive(Clone)]
pub struct TextCommand {
    pub font_id: u32,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size_px: u32,
    pub color: Color,
}

#[derive(Clone)]
pub struct SkinnedMeshCommand {
    pub texture_id: u32,
    pub vertex_buffer: Arc<wgpu::Buffer>,
    pub index_buffer: Arc<wgpu::Buffer>,
    pub num_indices: u32,
    pub blend_weights_buffer: Arc<wgpu::Buffer>,
    pub morph_delta_buffer: Arc<wgpu::Buffer>,
    pub num_morph_targets: u32,
    /// VRM MToon。None ならレンダラ既定値を使う。
    pub mtoon_buffer: Option<Arc<wgpu::Buffer>>,
    pub shade_texture_id: Option<u32>,
    pub matcap_texture_id: Option<u32>,
    pub normal_texture_id: Option<u32>,
    pub uv_mask_texture_id: Option<u32>,
    pub outline_width: f32,
    /// ドロー個別のスキンパレットスロット。
    /// None は共有 skinning_uniform_buffer（レガシー 2D パス）を使う。
    pub skin_slot: Option<usize>,
    /// パッド付きワールド AABB。None ならカリングしない。
    pub aabb: Option<crate::frustum::Aabb>,
    /// glTF `doubleSided` / VRM0 `_CullMode==Off`。
    pub double_sided: bool,
}

#[derive(Clone)]
pub struct MeshCommand {
    pub texture_id: u32,
    pub verts: Vec<[f32; 5]>,
    pub shader_id: u32,
    pub shader_params: [f32; 4],
}

#[derive(Clone)]
pub struct PolygonCommand {
    pub verts: Vec<[f32; 2]>,
    pub color: Color,
}

#[derive(Clone)]
pub struct Mesh3DCommand {
    pub texture_id: u32,
    pub verts: Vec<[f32; 8]>,
    pub indices: Vec<u32>,
    pub metallic: f32,
    pub roughness: f32,
    pub base_color: [f32; 3],
    /// Sky/backdrop only (`draw_mesh_3d(..., skip_fog=True)`). Not inferred
    /// from fog-off — default fog_params.z is 0 and that unlit every Mesh3D.
    pub skip_fog: bool,
}

/// ワールド 3D インスタンス。pos + yaw(Y) + scale。32 バイト。
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance3D {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub scale: [f32; 3],
    pub _pad: f32,
}

#[derive(Clone)]
pub enum DrawCommand {
    Rect(RectCommand),
    Polygon(PolygonCommand),
    Sprite(SpriteCommand),
    Text(TextCommand),
    SkinnedMesh(SkinnedMeshCommand),
    Mesh(MeshCommand),
}

// ---------- 頂点フォーマット ----------
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct ColorVertex {
    pub(super) position: [f32; 2],
    pub(super) color: [f32; 4],
}

impl ColorVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
    pub(super) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct SpriteVertex {
    pub(super) position: [f32; 2],
    pub(super) uv: [f32; 2],
    pub(super) alpha: f32,
    pub(super) _pad: [f32; 3],
}

impl SpriteVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32];
    pub(super) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinnedVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub joints: [u32; 4],
    pub weights: [f32; 4],
    /// バインド姿勢での法線。2D スキニングでは使わない。
    pub normal: [f32; 3],
}

impl SkinnedVertex {
    // normal は末尾に追加。2D スキニングシェーダは location 4 を宣言しないが、
    // 使わない頂点属性がレイアウトに含まれるのは wgpu では許容される。
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint32x4, 3 => Float32x4, 4 => Float32x3];
    pub(super) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}
