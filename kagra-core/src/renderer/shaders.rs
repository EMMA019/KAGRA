// WGSL shader sources
// ---------- シェーダ定数（元のまま）----------
pub(super) const RECT_SHADER: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) color: vec4<f32> }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) color: vec4<f32> }
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.color); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> { return in.color; }
"#;

pub(super) const SPRITE_SHADER: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.uv, in.alpha); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    c.a *= in.alpha;
    return c;
}
"#;

pub(super) const SHADER_GRAYSCALE: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>;
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.uv, in.alpha); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    let gray = dot(c.rgb, vec3<f32>(0.299, 0.587, 0.114));
    c = vec4<f32>(mix(c.rgb, vec3<f32>(gray), params.x), c.a * in.alpha);
    return c;
}
"#;

pub(super) const SHADER_FLASH: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>;
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.uv, in.alpha); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    let flash = vec3<f32>(select(1.0, params.y, params.y > 0.0), select(1.0, params.z, params.z > 0.0), select(1.0, params.w, params.w > 0.0));
    c = vec4<f32>(mix(c.rgb, flash, params.x), c.a * in.alpha);
    return c;
}
"#;

pub(super) const SHADER_SPOTLIGHT: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32, @location(2) screen_pos: vec2<f32> }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>;
@vertex fn vs_main(in: VI) -> VO {
    var out: VO;
    out.pos = vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv; out.alpha = in.alpha;
    out.screen_pos = in.position * 0.5 + 0.5;
    return out;
}
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    let spot = vec2<f32>(params.x, 1.0 - params.y);
    let dist = distance(in.screen_pos, spot);
    let falloff = 1.0 - smoothstep(params.z * 0.5, params.z, dist);
    let light = 0.15 + falloff * params.w;
    c = vec4<f32>(c.rgb * clamp(light, 0.0, 1.5), c.a * in.alpha);
    return c;
}
"#;

pub(super) const SHADER_GLOW: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>;
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.uv, in.alpha); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    let edge = min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y));
    let rim = (1.0 - smoothstep(0.0, 0.12, edge)) * params.w;
    c = vec4<f32>(c.rgb + vec3<f32>(params.x, params.y, params.z) * rim, c.a * in.alpha);
    return c;
}
"#;

pub(super) const SHADER_TINT: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>;
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.uv, in.alpha); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    c = vec4<f32>(mix(c.rgb, c.rgb * vec3<f32>(params.x, params.y, params.z), params.w), c.a * in.alpha);
    return c;
}
"#;

pub(super) const SKINNING_SHADER: &str = r#"
struct Uniforms {
    bone_matrices: array<mat4x4<f32>, 256>,
    screen_size: vec4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(1) @binding(0) var texture: texture_2d<f32>;
@group(1) @binding(1) var tex_sampler: sampler;
struct VI { @location(0) position: vec3<f32>, @location(1) uv: vec2<f32>,
            @location(2) joints: vec4<u32>, @location(3) weights: vec4<f32> }
struct VO { @builtin(position) clip_position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs_main(in: VI) -> VO {
    var m = mat4x4<f32>(vec4(0.0),vec4(0.0),vec4(0.0),vec4(0.0));
    m += uniforms.bone_matrices[in.joints[0]] * in.weights[0];
    m += uniforms.bone_matrices[in.joints[1]] * in.weights[1];
    m += uniforms.bone_matrices[in.joints[2]] * in.weights[2];
    m += uniforms.bone_matrices[in.joints[3]] * in.weights[3];
    let wp = m * vec4<f32>(in.position, 1.0);
    let half_w = uniforms.screen_size.x * 0.5;
    let half_h = uniforms.screen_size.y * 0.5;
    var out: VO;
    out.clip_position = vec4<f32>((wp.x/half_w)-1.0, -((wp.y/half_h)-1.0), wp.z/1000.0, 1.0);
    out.uv = in.uv;
    return out;
}
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    return textureSample(texture, tex_sampler, in.uv);
}
"#;

pub(super) const SHADER_3D: &str = r#"
struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    toon: vec4<f32>,
    eye: vec4<f32>,
    fog_params: vec4<f32>,
    fog_color: vec4<f32>,
};
@group(0) @binding(0) var<uniform> cam: Camera;
@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;
@group(2) @binding(0) var<uniform> light_vp: mat4x4<f32>;
@group(2) @binding(1) var shadow_map: texture_depth_2d;
@group(2) @binding(2) var shadow_sampler: sampler_comparison;
struct VI { @location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32> }
struct VO {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) light: f32,
    @location(2) world_pos: vec3<f32>,
}
fn shadow_factor(world_pos: vec3<f32>) -> f32 {
    let sc = light_vp * vec4<f32>(world_pos, 1.0);
    let ndc = sc.xyz / sc.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, ndc.y * -0.5 + 0.5);
    let depth = ndc.z;
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || depth < 0.0 || depth > 1.0 {
        return 1.0;
    }
    let texel = 1.0 / 1024.0;
    let d = depth - 0.002;
    var s = 0.0;
    s += textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(-0.5, -0.5) * texel, d);
    s += textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(0.5, -0.5) * texel, d);
    s += textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(-0.5, 0.5) * texel, d);
    s += textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(0.5, 0.5) * texel, d);
    return mix(0.55, 1.0, s * 0.25);
}
fn apply_fog(color: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    if cam.fog_params.z < 0.5 { return color; }
    let dist = length(world_pos - cam.eye.xyz);
    let t = saturate((dist - cam.fog_params.x) / max(1e-3, cam.fog_params.y - cam.fog_params.x));
    return mix(color, cam.fog_color.rgb, t);
}
@vertex fn vs_main(in: VI) -> VO {
    var out: VO;
    let pos4 = vec4<f32>(in.position, 1.0);
    let view_pos = cam.view * pos4;
    out.clip_pos = cam.proj * view_pos;
    out.uv = in.uv;
    let light_dir = normalize(cam.light_dir.xyz);
    out.light = clamp(dot(normalize(in.normal), light_dir), 0.2, 1.0);
    out.world_pos = in.position;
    return out;
}
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    if c.a < 0.01 { discard; }
    let lit = in.light * shadow_factor(in.world_pos);
    let rgb = apply_fog(c.rgb * lit, in.world_pos);
    return vec4<f32>(rgb, c.a);
}
"#;

pub(super) const SHADER_SKINNING_3D_BLEND: &str = r#"
struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    // x=threshold, y=softness, z=shade, w=lit
    toon: vec4<f32>,
    eye: vec4<f32>,
    fog_params: vec4<f32>,
    fog_color: vec4<f32>,
};
struct Mtoon {
    shade_color: vec4<f32>,
    rim_color: vec4<f32>,
    params: vec4<f32>,
    outline_color: vec4<f32>,
};
@group(0) @binding(0) var<uniform> cam: Camera;
struct SkinUniforms { bone_matrices: array<mat4x4<f32>, 256>, screen_size: vec4<f32> };
@group(1) @binding(0) var<uniform> skin: SkinUniforms;
@group(2) @binding(0) var t_diffuse: texture_2d<f32>;
@group(2) @binding(1) var s_diffuse: sampler;
@group(2) @binding(2) var<storage, read> morph_deltas: array<f32>;
@group(2) @binding(3) var<uniform> blend_weights: array<vec4<f32>, 64>;
@group(2) @binding(4) var<uniform> num_morph_targets: u32;
@group(2) @binding(5) var<uniform> mtoon: Mtoon;
@group(2) @binding(6) var t_shade: texture_2d<f32>;
@group(3) @binding(0) var<uniform> light_vp: mat4x4<f32>;
@group(3) @binding(1) var shadow_map: texture_depth_2d;
@group(3) @binding(2) var shadow_sampler: sampler_comparison;
struct VI { @location(0) position: vec3<f32>, @location(1) uv: vec2<f32>,
            @location(2) joints: vec4<u32>, @location(3) weights: vec4<f32>,
            @location(4) normal: vec3<f32>,
            @builtin(vertex_index) vertex_idx: u32 }
struct VO {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_nrm: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
}

fn apply_morph(in: VI, pos: ptr<function, vec3<f32>>) {
    let total_f32 = arrayLength(&morph_deltas);
    let num_targets = num_morph_targets;
    if num_targets > 0u && total_f32 >= num_targets * 3u {
        let vertex_count = total_f32 / 3u / num_targets;
        if vertex_count > 0u && in.vertex_idx < vertex_count {
            let capped = min(num_targets, 256u);
            for (var i = 0u; i < capped; i = i + 1u) {
                let w = blend_weights[i / 4u][i % 4u];
                if w > 0.001 {
                    let base = (i * vertex_count + in.vertex_idx) * 3u;
                    let dx = morph_deltas[base];
                    let dy = morph_deltas[base + 1u];
                    let dz = morph_deltas[base + 2u];
                    *pos = *pos + w * vec3<f32>(dx, dy, dz);
                }
            }
        }
    }
}

fn skin_matrix(in: VI) -> mat4x4<f32> {
    var m = mat4x4<f32>(vec4(0.0),vec4(0.0),vec4(0.0),vec4(0.0));
    m = m + skin.bone_matrices[in.joints[0]] * in.weights[0];
    m = m + skin.bone_matrices[in.joints[1]] * in.weights[1];
    m = m + skin.bone_matrices[in.joints[2]] * in.weights[2];
    m = m + skin.bone_matrices[in.joints[3]] * in.weights[3];
    return m;
}

fn shadow_factor(world_pos: vec3<f32>) -> f32 {
    let sc = light_vp * vec4<f32>(world_pos, 1.0);
    let ndc = sc.xyz / sc.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, ndc.y * -0.5 + 0.5);
    let depth = ndc.z;
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || depth < 0.0 || depth > 1.0 {
        return 1.0;
    }
    let texel = 1.0 / 1024.0;
    let d = depth - 0.002;
    var s = 0.0;
    s += textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(-0.5, -0.5) * texel, d);
    s += textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(0.5, -0.5) * texel, d);
    s += textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(-0.5, 0.5) * texel, d);
    s += textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(0.5, 0.5) * texel, d);
    return mix(0.55, 1.0, s * 0.25);
}

fn apply_fog(color: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    if cam.fog_params.z < 0.5 { return color; }
    let dist = length(world_pos - cam.eye.xyz);
    let t = saturate((dist - cam.fog_params.x) / max(1e-3, cam.fog_params.y - cam.fog_params.x));
    return mix(color, cam.fog_color.rgb, t);
}

@vertex fn vs_main(in: VI) -> VO {
    var pos = in.position;
    apply_morph(in, &pos);
    let m = skin_matrix(in);
    let world_pos = m * vec4<f32>(pos, 1.0);
    let view_pos = cam.view * world_pos;
    let clip_pos = cam.proj * view_pos;
    let skinned_nrm = (m * vec4<f32>(in.normal, 0.0)).xyz;
    var world_nrm = vec3<f32>(0.0, 1.0, 0.0);
    if dot(skinned_nrm, skinned_nrm) > 1e-12 {
        world_nrm = normalize(skinned_nrm);
    }
    var out: VO;
    out.clip_pos = clip_pos;
    out.uv = in.uv;
    out.world_nrm = world_nrm;
    out.world_pos = world_pos.xyz;
    return out;
}

@vertex fn vs_shadow(in: VI) -> @builtin(position) vec4<f32> {
    var pos = in.position;
    apply_morph(in, &pos);
    let m = skin_matrix(in);
    let world_pos = m * vec4<f32>(pos, 1.0);
    return light_vp * world_pos;
}

@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var base = textureSample(t_diffuse, s_diffuse, in.uv);
    if base.a < 0.05 { discard; }
    let n = normalize(in.world_nrm);
    let light_dir = normalize(cam.light_dir.xyz);
    let half_lambert = dot(n, light_dir) * 0.5 + 0.5;
    let toony = mtoon.shade_color.a;
    let shift = mtoon.params.x;
    var t: f32;
    // set_toon_params 互換: softness < 0.999 ならグローバル toon の smoothstep 経路
    if cam.toon.y < 0.999 {
        let threshold = cam.toon.x;
        let softness = cam.toon.y;
        let edge0 = threshold - softness;
        let edge1 = threshold + max(softness, 1e-4);
        t = smoothstep(edge0, edge1, half_lambert);
    } else {
        t = saturate((half_lambert - 0.5 + shift) / max(1e-3, 1.0 - toony) + 0.5);
    }
    var shade_rgb: vec3<f32>;
    if mtoon.params.w > 0.5 {
        let shade_tex = textureSample(t_shade, s_diffuse, in.uv);
        shade_rgb = mtoon.shade_color.rgb * shade_tex.rgb;
    } else {
        shade_rgb = mtoon.shade_color.rgb * base.rgb;
    }
    var col = mix(shade_rgb, base.rgb, t);
    let V = normalize(cam.eye.xyz - in.world_pos);
    let rim_power = mtoon.rim_color.a;
    let rim_lift = mtoon.params.y;
    let rim = pow(max(1.0 - saturate(dot(n, V)), 0.0), max(rim_power, 1e-3)) * rim_lift;
    col = col + rim * mtoon.rim_color.rgb;
    col = col * shadow_factor(in.world_pos);
    col = apply_fog(col, in.world_pos);
    return vec4<f32>(col, base.a);
}

@vertex fn vs_outline(in: VI) -> VO {
    var pos = in.position;
    apply_morph(in, &pos);
    let m = skin_matrix(in);
    var world_pos = m * vec4<f32>(pos, 1.0);
    let skinned_nrm = (m * vec4<f32>(in.normal, 0.0)).xyz;
    var world_nrm = vec3<f32>(0.0, 1.0, 0.0);
    if dot(skinned_nrm, skinned_nrm) > 1e-12 {
        world_nrm = normalize(skinned_nrm);
    }
    world_pos = vec4<f32>(world_pos.xyz + world_nrm * mtoon.params.z, 1.0);
    let view_pos = cam.view * world_pos;
    let clip_pos = cam.proj * view_pos;
    var out: VO;
    out.clip_pos = clip_pos;
    out.uv = in.uv;
    out.world_nrm = world_nrm;
    out.world_pos = world_pos.xyz;
    return out;
}

@fragment fn fs_outline(_in: VO) -> @location(0) vec4<f32> {
    return vec4<f32>(mtoon.outline_color.rgb, 1.0);
}
"#;
