//! VRM MToon マテリアル（VRM 1.0 `VRMC_materials_mtoon` / VRM 0.x 互換）

use std::collections::HashMap;
use std::sync::Arc;

use wgpu::util::DeviceExt;

/// GPU に送る MToon パラメータ（16 バイト整列）
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MtoonGpu {
    /// rgb = shadeColor, a = shadingToonyFactor (0..1)
    pub shade_color: [f32; 4],
    /// rgb = rimColor, a = rimFresnelPower
    pub rim_color: [f32; 4],
    /// x=shadingShift, y=rimLift, z=outlineWidthFactor, w=hasShadeTex (0/1)
    pub params: [f32; 4],
    /// rgb = outlineColor, a = outlineLightingMix (unused for now)
    pub outline_color: [f32; 4],
}

impl Default for MtoonGpu {
    fn default() -> Self {
        Self {
            // やや暗めの影色（ベース色に乗算される）
            shade_color: [0.55, 0.50, 0.52, 0.85],
            rim_color: [0.0, 0.0, 0.0, 5.0],
            params: [0.0, 0.0, 0.0, 0.0],
            outline_color: [0.05, 0.05, 0.08, 1.0],
        }
    }
}

#[derive(Clone)]
pub struct MtoonMaterial {
    pub gpu: MtoonGpu,
    pub shade_texture_id: Option<u32>,
    pub buffer: Arc<wgpu::Buffer>,
}

impl MtoonMaterial {
    pub fn create(device: &wgpu::Device, gpu: MtoonGpu, shade_texture_id: Option<u32>) -> Self {
        let mut g = gpu;
        g.params[3] = if shade_texture_id.is_some() { 1.0 } else { 0.0 };
        let buffer = Arc::new(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MToon UBO"),
            contents: bytemuck::bytes_of(&g),
            usage: wgpu::BufferUsages::UNIFORM,
        }));
        Self {
            gpu: g,
            shade_texture_id,
            buffer,
        }
    }

    pub fn default_mat(device: &wgpu::Device) -> Self {
        Self::create(device, MtoonGpu::default(), None)
    }
}

fn f32_arr3(v: Option<&serde_json::Value>, def: [f32; 3]) -> [f32; 3] {
    v.and_then(|a| a.as_array())
        .map(|a| {
            [
                a.first().and_then(|x| x.as_f64()).unwrap_or(def[0] as f64) as f32,
                a.get(1).and_then(|x| x.as_f64()).unwrap_or(def[1] as f64) as f32,
                a.get(2).and_then(|x| x.as_f64()).unwrap_or(def[2] as f64) as f32,
            ]
        })
        .unwrap_or(def)
}

fn f32_val(v: Option<&serde_json::Value>, def: f32) -> f32 {
    v.and_then(|x| x.as_f64()).map(|x| x as f32).unwrap_or(def)
}

fn tex_index(ext: &serde_json::Value, key: &str) -> Option<usize> {
    ext.get(key)
        .and_then(|t| t.get("index"))
        .and_then(|i| i.as_u64())
        .map(|i| i as usize)
}

/// glTF material + 任意の MToon 拡張からマテリアルを構築する。
pub fn parse_mtoon(
    device: &wgpu::Device,
    mat: &serde_json::Value,
    tex_id_map: &HashMap<usize, u32>,
) -> MtoonMaterial {
    let mut gpu = MtoonGpu::default();
    let mut shade_tex: Option<u32> = None;

    // VRM 1.0
    if let Some(mtoon) = mat
        .pointer("/extensions/VRMC_materials_mtoon")
        .or_else(|| mat.pointer("/extensions/VRM/materials_mtoon"))
    {
        let shade = f32_arr3(mtoon.get("shadeColorFactor"), [0.9, 0.85, 0.85]);
        let toony = f32_val(mtoon.get("shadingToonyFactor"), 0.9);
        gpu.shade_color = [shade[0], shade[1], shade[2], toony.clamp(0.0, 0.999)];

        let shift = f32_val(mtoon.get("shadingShiftFactor"), 0.0);
        let rim = f32_arr3(mtoon.get("parametricRimColorFactor"), [0.0, 0.0, 0.0]);
        let rim_pow = f32_val(mtoon.get("parametricRimFresnelPowerFactor"), 5.0);
        let rim_lift = f32_val(mtoon.get("parametricRimLiftFactor"), 0.0);
        gpu.rim_color = [rim[0], rim[1], rim[2], rim_pow.max(0.1)];

        let outline_w = f32_val(mtoon.get("outlineWidthFactor"), 0.0);
        // worldCoordinates の場合はメートル、screen の場合は別扱い → ここでは world 想定で縮小
        let outline_mode = mtoon
            .get("outlineWidthMode")
            .and_then(|m| m.as_str())
            .unwrap_or("none");
        let width = match outline_mode {
            "worldCoordinates" => outline_w,
            "screenCoordinates" => outline_w * 0.01,
            _ => 0.0,
        };
        let outline = f32_arr3(mtoon.get("outlineColorFactor"), [0.0, 0.0, 0.0]);
        gpu.outline_color = [outline[0], outline[1], outline[2], 1.0];
        gpu.params = [shift, rim_lift, width, 0.0];

        if let Some(ti) = tex_index(mtoon, "shadeMultiplyTexture") {
            shade_tex = tex_id_map.get(&ti).copied();
        }
    } else if let Some(props) = mat.get("extensions").and_then(|e| e.get("VRM")) {
        // 一部の VRM0 は material 直下ではなく別経路。最低限 base から推定。
        let _ = props;
    }

    // VRM 0.x: materials[].extras / または呼び出し側で materialProperties を渡す場合
    if let Some(vrm0) = mat.get("extras").and_then(|e| e.get("VRM_MToon")) {
        let shade = f32_arr3(vrm0.get("shadeColor"), gpu.shade_color[..3].try_into().unwrap());
        gpu.shade_color[0] = shade[0];
        gpu.shade_color[1] = shade[1];
        gpu.shade_color[2] = shade[2];
    }

    MtoonMaterial::create(device, gpu, shade_tex)
}

/// VRM 0.x `extensions.VRM.materialProperties` 配列を material index で引けるようにする。
pub fn parse_vrm0_material_properties(
    device: &wgpu::Device,
    gltf: &serde_json::Value,
    materials: &[serde_json::Value],
    tex_id_map: &HashMap<usize, u32>,
) -> Vec<MtoonMaterial> {
    let mut out: Vec<MtoonMaterial> = materials
        .iter()
        .map(|m| parse_mtoon(device, m, tex_id_map))
        .collect();

    let Some(props) = gltf
        .pointer("/extensions/VRM/materialProperties")
        .and_then(|p| p.as_array())
    else {
        return out;
    };

    for (i, prop) in props.iter().enumerate() {
        if i >= out.len() {
            break;
        }
        let shader = prop
            .get("shader")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        if !(shader.contains("MToon") || shader.contains("VRM")) {
            // 名前が空でも floatProperties があれば読む
            if prop.get("floatProperties").is_none() && prop.get("vectorProperties").is_none() {
                continue;
            }
        }

        let mut gpu = out[i].gpu;
        let vectors = prop.get("vectorProperties");
        let floats = prop.get("floatProperties");

        if let Some(shade) = vectors
            .and_then(|v| v.get("_ShadeColor").or_else(|| v.get("ShadeColor")))
            .and_then(|a| a.as_array())
        {
            gpu.shade_color[0] = shade.first().and_then(|x| x.as_f64()).unwrap_or(0.9) as f32;
            gpu.shade_color[1] = shade.get(1).and_then(|x| x.as_f64()).unwrap_or(0.85) as f32;
            gpu.shade_color[2] = shade.get(2).and_then(|x| x.as_f64()).unwrap_or(0.85) as f32;
        }
        if let Some(outline) = vectors
            .and_then(|v| v.get("_OutlineColor").or_else(|| v.get("OutlineColor")))
            .and_then(|a| a.as_array())
        {
            gpu.outline_color[0] = outline.first().and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            gpu.outline_color[1] = outline.get(1).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            gpu.outline_color[2] = outline.get(2).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
        }
        if let Some(rim) = vectors
            .and_then(|v| v.get("_RimColor").or_else(|| v.get("RimColor")))
            .and_then(|a| a.as_array())
        {
            gpu.rim_color[0] = rim.first().and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            gpu.rim_color[1] = rim.get(1).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            gpu.rim_color[2] = rim.get(2).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
        }

        if let Some(toony) = floats
            .and_then(|f| f.get("_ShadeToony").or_else(|| f.get("_ShadingToony")).or_else(|| f.get("ShadeToony")))
            .and_then(|x| x.as_f64())
        {
            gpu.shade_color[3] = (toony as f32).clamp(0.0, 0.999);
        }
        if let Some(shift) = floats
            .and_then(|f| f.get("_ShadeShift").or_else(|| f.get("ShadeShift")))
            .and_then(|x| x.as_f64())
        {
            gpu.params[0] = shift as f32;
        }
        if let Some(ow) = floats
            .and_then(|f| f.get("_OutlineWidth").or_else(|| f.get("OutlineWidth")))
            .and_then(|x| x.as_f64())
        {
            // UniVRM OutlineWidth → ワールド押し出し量
            let mode = floats
                .and_then(|f| f.get("_OutlineWidthMode").or_else(|| f.get("OutlineWidthMode")))
                .and_then(|x| x.as_f64())
                .unwrap_or(1.0);
            if mode > 0.5 {
                gpu.params[2] = ((ow as f32) * 0.01).clamp(0.002, 0.04);
            }
        }
        if let Some(rp) = floats
            .and_then(|f| f.get("_RimFresnelPower").or_else(|| f.get("RimFresnelPower")))
            .and_then(|x| x.as_f64())
        {
            gpu.rim_color[3] = (rp as f32).max(0.1);
        }
        if let Some(rl) = floats
            .and_then(|f| f.get("_RimLift").or_else(|| f.get("RimLift")))
            .and_then(|x| x.as_f64())
        {
            gpu.params[1] = rl as f32;
        }

        let mut shade_tex = out[i].shade_texture_id;
        if let Some(ti) = prop
            .pointer("/textureProperties/_ShadeTexture")
            .or_else(|| prop.pointer("/textureProperties/ShadeTexture"))
            .and_then(|x| x.as_u64())
        {
            shade_tex = tex_id_map.get(&(ti as usize)).copied();
        }

        out[i] = MtoonMaterial::create(device, gpu, shade_tex);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_gpu_size_is_64() {
        assert_eq!(std::mem::size_of::<MtoonGpu>(), 64);
    }

    #[test]
    fn parse_vrm1_mtoon_json() {
        let mat: serde_json::Value = serde_json::json!({
            "extensions": {
                "VRMC_materials_mtoon": {
                    "shadeColorFactor": [0.2, 0.3, 0.4],
                    "shadingToonyFactor": 0.8,
                    "shadingShiftFactor": 0.1,
                    "outlineWidthMode": "worldCoordinates",
                    "outlineWidthFactor": 0.02,
                    "outlineColorFactor": [0.1, 0.0, 0.0],
                    "parametricRimColorFactor": [1.0, 0.5, 0.2],
                    "parametricRimFresnelPowerFactor": 4.0,
                    "parametricRimLiftFactor": 0.2
                }
            }
        });
        // device なしのパース部分だけ検証するため GPU 作成はスキップしフィールドを直接読む
        let mtoon = mat.pointer("/extensions/VRMC_materials_mtoon").unwrap();
        let shade = f32_arr3(mtoon.get("shadeColorFactor"), [0.0; 3]);
        assert!((shade[0] - 0.2).abs() < 1e-5);
        assert!((f32_val(mtoon.get("shadingToonyFactor"), 0.0) - 0.8).abs() < 1e-5);
    }
}
