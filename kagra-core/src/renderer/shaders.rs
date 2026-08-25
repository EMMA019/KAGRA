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
    ambient: vec4<f32>,
    point_pos: vec4<f32>,
    point_col: vec4<f32>,
    env: vec4<f32>,
    spot_dir: vec4<f32>,
    extra_pos: array<vec4<f32>, 3>,
    extra_col: array<vec4<f32>, 3>,
    extra_spot: array<vec4<f32>, 3>,
    extra_inner: vec4<f32>,
};
@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var env_cube: texture_cube<f32>;
@group(0) @binding(2) var env_samp: sampler;
@group(0) @binding(3) var env_irr: texture_cube<f32>;
@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;
@group(1) @binding(2) var t_normal: texture_2d<f32>;
// params.x = cascade count. params.y = 1 when the map is a spot perspective
// (multiply the local light, not the directional sun).
struct ShadowU { vp0: mat4x4<f32>, vp1: mat4x4<f32>, params: vec4<f32> }
@group(2) @binding(0) var<uniform> shadow_u: ShadowU;
@group(2) @binding(1) var shadow_map: texture_depth_2d_array;
@group(2) @binding(2) var shadow_sampler: sampler_comparison;
struct MeshMat { base: vec4<f32>, params: vec4<f32> }
@group(3) @binding(0) var<uniform> mesh_mat: MeshMat;
struct VI { @location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32> }
struct VO {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) light: f32,
    @location(2) world_pos: vec3<f32>,
    @location(3) hemi: vec3<f32>,
    @location(4) world_n: vec3<f32>,
}
fn sample_cascade(world_pos: vec3<f32>, vp: mat4x4<f32>, layer: i32) -> f32 {
    let sc = vp * vec4<f32>(world_pos, 1.0);
    let ndc = sc.xyz / sc.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, ndc.y * -0.5 + 0.5);
    let depth = ndc.z;
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || depth < 0.0 || depth > 1.0 {
        return 1.0;
    }
    let texel = 1.0 / 2048.0;
    let bias = select(0.0015, 0.0035, shadow_u.params.y > 0.5);
    let d = depth - bias;
    var s = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            s += textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(f32(x), f32(y)) * texel, layer, d);
        }
    }
    let dark = select(0.50, 0.16, shadow_u.params.y > 0.5);
    return mix(dark, 1.0, s / 9.0);
}
fn shadow_factor(world_pos: vec3<f32>) -> f32 {
    let n = sample_cascade(world_pos, shadow_u.vp0, 0);
    if shadow_u.params.x < 1.5 {
        return n;
    }
    let sc = shadow_u.vp0 * vec4<f32>(world_pos, 1.0);
    let ndc = sc.xyz / sc.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, ndc.y * -0.5 + 0.5);
    if uv.x >= 0.02 && uv.x <= 0.98 && uv.y >= 0.02 && uv.y <= 0.98 && ndc.z >= 0.0 && ndc.z <= 1.0 {
        return n;
    }
    return sample_cascade(world_pos, shadow_u.vp1, 1);
}
fn apply_fog(color: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    if cam.fog_params.z < 0.5 { return color; }
    let dist = length(world_pos - cam.eye.xyz);
    let t = saturate((dist - cam.fog_params.x) / max(1e-3, cam.fog_params.y - cam.fog_params.x));
    return mix(color, cam.fog_color.rgb, t);
}
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    return saturate((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14));
}
fn hemi_ambient(n: vec3<f32>) -> vec3<f32> {
    if cam.ambient.w < 1e-4 { return vec3<f32>(0.0); }
    let h = 0.45 + 0.55 * saturate(n.y * 0.5 + 0.5);
    return cam.ambient.rgb * cam.ambient.w * h;
}
fn env_light(n: vec3<f32>) -> vec3<f32> {
    if cam.env.x < 1e-4 { return vec3<f32>(0.0); }
    return textureSample(env_irr, env_samp, n).rgb * cam.env.x;
}
fn light_one(n: vec3<f32>, world_pos: vec3<f32>, pos: vec4<f32>, col: vec4<f32>, spot: vec4<f32>, inner: f32) -> vec3<f32> {
    if col.w < 1e-4 { return vec3<f32>(0.0); }
    let to_l = pos.xyz - world_pos;
    let dist = length(to_l);
    let radius = max(pos.w, 0.05);
    let atten = saturate(1.0 - dist / radius);
    let ndotl = saturate(dot(n, normalize(to_l)));
    var cone = 1.0;
    if spot.w > 1e-4 {
        let from_l = normalize(-to_l);
        let axis = normalize(spot.xyz);
        let c = dot(from_l, axis);
        let outer = spot.w;
        if inner - outer < 1e-4 {
            cone = select(0.0, 1.0, c >= outer);
        } else {
            cone = smoothstep(outer, inner, c);
        }
    }
    return col.rgb * col.w * ndotl * atten * atten * cone;
}
fn key_diffuse(n: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    return light_one(n, world_pos, cam.point_pos, cam.point_col, cam.spot_dir, cam.env.z);
}
fn fill_diffuse(n: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    var s = light_one(n, world_pos, cam.extra_pos[0], cam.extra_col[0], cam.extra_spot[0], cam.extra_inner.x);
    s = s + light_one(n, world_pos, cam.extra_pos[1], cam.extra_col[1], cam.extra_spot[1], cam.extra_inner.y);
    s = s + light_one(n, world_pos, cam.extra_pos[2], cam.extra_col[2], cam.extra_spot[2], cam.extra_inner.z);
    return s;
}
fn local_lit(n: vec3<f32>, world_pos: vec3<f32>, loc_sh: f32) -> vec3<f32> {
    return key_diffuse(n, world_pos) * loc_sh + fill_diffuse(n, world_pos);
}
fn ggx_d(ndoth: f32, rough: f32) -> f32 {
    let a = max(rough * rough, 0.002);
    let a2 = a * a;
    let d = ndoth * ndoth * (a2 - 1.0) + 1.0;
    return a2 / max(3.14159265 * d * d, 1e-6);
}
fn smith_g(ndotv: f32, ndotl: f32, rough: f32) -> f32 {
    let k = max(rough * rough, 0.002) * 0.5;
    let gv = ndotv / max(ndotv * (1.0 - k) + k, 1e-5);
    let gl = ndotl / max(ndotl * (1.0 - k) + k, 1e-5);
    return gv * gl;
}
fn cotangent_frame(n: vec3<f32>, p: vec3<f32>, uv: vec2<f32>) -> mat3x3<f32> {
    let dp1 = dpdx(p);
    let dp2 = dpdy(p);
    let duv1 = dpdx(uv);
    let duv2 = dpdy(uv);
    let dp2perp = cross(dp2, n);
    let dp1perp = cross(n, dp1);
    var t = dp2perp * duv1.x + dp1perp * duv2.x;
    var b = dp2perp * duv1.y + dp1perp * duv2.y;
    let invmax = inverseSqrt(max(dot(t, t), dot(b, b)));
    t = t * invmax;
    b = b * invmax;
    return mat3x3<f32>(t, b, n);
}
@vertex fn vs_main(in: VI) -> VO {
    var out: VO;
    let pos4 = vec4<f32>(in.position, 1.0);
    let view_pos = cam.view * pos4;
    out.clip_pos = cam.proj * view_pos;
    out.uv = in.uv;
    let n = normalize(in.normal);
    let light_dir = normalize(cam.light_dir.xyz);
    out.light = clamp(dot(n, light_dir), 0.2, 1.0);
    out.world_pos = in.position;
    out.hemi = hemi_ambient(n);
    out.world_n = n;
    return out;
}
struct II { @location(3) pos_yaw: vec4<f32>, @location(4) scale: vec4<f32> }
@vertex fn vs_instanced(in: VI, inst: II) -> VO {
    var out: VO;
    let c = cos(inst.pos_yaw.w);
    let s = sin(inst.pos_yaw.w);
    let p = in.position * inst.scale.xyz;
    let world = vec3<f32>(
        c * p.x + s * p.z + inst.pos_yaw.x,
        p.y + inst.pos_yaw.y,
        -s * p.x + c * p.z + inst.pos_yaw.z,
    );
    let n = vec3<f32>(
        c * in.normal.x + s * in.normal.z,
        in.normal.y,
        -s * in.normal.x + c * in.normal.z,
    );
    let nn = normalize(n);
    let pos4 = vec4<f32>(world, 1.0);
    out.clip_pos = cam.proj * (cam.view * pos4);
    out.uv = in.uv;
    let light_dir = normalize(cam.light_dir.xyz);
    out.light = clamp(dot(nn, light_dir), 0.2, 1.0);
    out.world_pos = world;
    out.hemi = hemi_ambient(nn);
    out.world_n = nn;
    return out;
}
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    if c.a < 0.01 { discard; }
    var n = normalize(in.world_n);
    var hemi = in.hemi;
    if mesh_mat.params.z > 0.5 {
        let nts = textureSample(t_normal, s_diffuse, in.uv).xyz * 2.0 - 1.0;
        n = normalize(cotangent_frame(n, in.world_pos, in.uv) * nts);
        hemi = hemi_ambient(n);
    }
    let albedo = c.rgb * mesh_mat.base.rgb;
    let env = env_light(n);
    var rgb: vec3<f32>;
    if mesh_mat.params.w > 0.5 {
        let metallic = saturate(mesh_mat.params.x);
        let rough = clamp(mesh_mat.params.y, 0.04, 1.0);
        let v = normalize(cam.eye.xyz - in.world_pos);
        let ldir = normalize(cam.light_dir.xyz);
        let h = normalize(v + ldir);
        let ndotl = saturate(dot(n, ldir));
        let ndotv = saturate(dot(n, v));
        let ndoth = saturate(dot(n, h));
        let vdoth = saturate(dot(v, h));
        let f0 = mix(vec3<f32>(0.04), albedo, metallic);
        let f = f0 + (1.0 - f0) * pow(1.0 - vdoth, 5.0);
        let spec = ggx_d(ndoth, rough) * smith_g(ndotv, ndotl, rough) * f
            / max(4.0 * ndotv * ndotl, 1e-4);
        let kd = (vec3<f32>(1.0) - f) * (1.0 - metallic);
        let sh = shadow_factor(in.world_pos);
        let sun_sh = select(sh, 1.0, shadow_u.params.y > 0.5);
        let loc_sh = select(1.0, sh, shadow_u.params.y > 0.5);
        let sun = (kd * albedo + spec) * ndotl * sun_sh;
        let spec_env = textureSampleLevel(env_cube, env_samp, reflect(-v, n), rough * 3.0).rgb
            * cam.env.x * (1.0 - rough) * f;
        rgb = sun + albedo * local_lit(n, in.world_pos, loc_sh) + hemi + env * (1.0 - metallic) * 0.65 + spec_env;
    } else {
        let light_dir = normalize(cam.light_dir.xyz);
        var ndotl: f32;
        // set_toon_params: same cam.toon stepped lighting as VRM. softness>=0.999 keeps Lambert.
        if cam.toon.y < 0.999 {
            let half_lambert = dot(n, light_dir) * 0.5 + 0.5;
            let threshold = cam.toon.x;
            let softness = cam.toon.y;
            let edge0 = threshold - softness;
            let edge1 = threshold + max(softness, 1e-4);
            let t = smoothstep(edge0, edge1, half_lambert);
            ndotl = mix(cam.toon.z, cam.toon.w, t);
        } else {
            ndotl = in.light;
            if mesh_mat.params.z > 0.5 {
                ndotl = clamp(dot(n, light_dir), 0.2, 1.0);
            }
        }
        let sh = shadow_factor(in.world_pos);
        let sun_sh = select(sh, 1.0, shadow_u.params.y > 0.5);
        let loc_sh = select(1.0, sh, shadow_u.params.y > 0.5);
        let lit = ndotl * sun_sh;
        // Diffuse IBL must scale by albedo (same 0.35 as VRM). Additive env * strength
        // blew mid-green grass / Kenney colormap to white while vertex colors survived.
        rgb = albedo * lit + albedo * local_lit(n, in.world_pos, loc_sh) + hemi + env * albedo * 0.35;
    }
    rgb = apply_fog(rgb, in.world_pos);
    rgb = rgb * max(cam.env.y, 0.0);
    if cam.env.w > 0.5 {
        rgb = aces_tonemap(rgb);
    }
    return vec4<f32>(rgb, c.a);
}
"#;

/// Depth-only world casters. `light_vp` only — the color `SHADER_3D` group 2
/// also samples the map, which cannot be bound while writing it.
pub(super) const SHADER_3D_SHADOW: &str = r#"
@group(0) @binding(0) var<uniform> light_vp: mat4x4<f32>;
struct VI { @location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32> }
struct II { @location(3) pos_yaw: vec4<f32>, @location(4) scale: vec4<f32> }
@vertex fn vs_shadow(in: VI) -> @builtin(position) vec4<f32> {
    return light_vp * vec4<f32>(in.position, 1.0);
}
@vertex fn vs_shadow_instanced(in: VI, inst: II) -> @builtin(position) vec4<f32> {
    let c = cos(inst.pos_yaw.w);
    let s = sin(inst.pos_yaw.w);
    let p = in.position * inst.scale.xyz;
    let world = vec3<f32>(
        c * p.x + s * p.z + inst.pos_yaw.x,
        p.y + inst.pos_yaw.y,
        -s * p.x + c * p.z + inst.pos_yaw.z,
    );
    return light_vp * vec4<f32>(world, 1.0);
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
    ambient: vec4<f32>,
    point_pos: vec4<f32>,
    point_col: vec4<f32>,
    env: vec4<f32>,
    spot_dir: vec4<f32>,
    extra_pos: array<vec4<f32>, 3>,
    extra_col: array<vec4<f32>, 3>,
    extra_spot: array<vec4<f32>, 3>,
    extra_inner: vec4<f32>,
};
struct Mtoon {
    shade_color: vec4<f32>,
    rim_color: vec4<f32>,
    params: vec4<f32>,
    outline_color: vec4<f32>,
    matcap_color: vec4<f32>,
    uv_anim: vec4<f32>,
};
@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var env_cube: texture_cube<f32>;
@group(0) @binding(2) var env_samp: sampler;
@group(0) @binding(3) var env_irr: texture_cube<f32>;
struct SkinUniforms { bone_matrices: array<mat4x4<f32>, 256>, screen_size: vec4<f32> };
@group(1) @binding(0) var<uniform> skin: SkinUniforms;
@group(2) @binding(0) var t_diffuse: texture_2d<f32>;
@group(2) @binding(1) var s_diffuse: sampler;
@group(2) @binding(2) var<storage, read> morph_deltas: array<f32>;
@group(2) @binding(3) var<uniform> blend_weights: array<vec4<f32>, 64>;
@group(2) @binding(4) var<uniform> num_morph_targets: u32;
@group(2) @binding(5) var<uniform> mtoon: Mtoon;
@group(2) @binding(6) var t_shade: texture_2d<f32>;
@group(2) @binding(7) var t_matcap: texture_2d<f32>;
@group(2) @binding(8) var t_normal: texture_2d<f32>;
@group(2) @binding(9) var t_uvmask: texture_2d<f32>;
// params.x = cascade count. params.y = 1 when the map is a spot perspective.
struct ShadowU { vp0: mat4x4<f32>, vp1: mat4x4<f32>, params: vec4<f32> }
@group(3) @binding(0) var<uniform> shadow_u: ShadowU;
@group(3) @binding(1) var shadow_map: texture_depth_2d_array;
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

fn sample_cascade(world_pos: vec3<f32>, vp: mat4x4<f32>, layer: i32) -> f32 {
    let sc = vp * vec4<f32>(world_pos, 1.0);
    let ndc = sc.xyz / sc.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, ndc.y * -0.5 + 0.5);
    let depth = ndc.z;
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || depth < 0.0 || depth > 1.0 {
        return 1.0;
    }
    let texel = 1.0 / 2048.0;
    let bias = select(0.0015, 0.0035, shadow_u.params.y > 0.5);
    let d = depth - bias;
    var s = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            s += textureSampleCompare(shadow_map, shadow_sampler, uv + vec2<f32>(f32(x), f32(y)) * texel, layer, d);
        }
    }
    let dark = select(0.50, 0.16, shadow_u.params.y > 0.5);
    return mix(dark, 1.0, s / 9.0);
}
fn shadow_factor(world_pos: vec3<f32>) -> f32 {
    let n = sample_cascade(world_pos, shadow_u.vp0, 0);
    if shadow_u.params.x < 1.5 {
        return n;
    }
    let sc = shadow_u.vp0 * vec4<f32>(world_pos, 1.0);
    let ndc = sc.xyz / sc.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, ndc.y * -0.5 + 0.5);
    if uv.x >= 0.02 && uv.x <= 0.98 && uv.y >= 0.02 && uv.y <= 0.98 && ndc.z >= 0.0 && ndc.z <= 1.0 {
        return n;
    }
    return sample_cascade(world_pos, shadow_u.vp1, 1);
}

fn apply_fog(color: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    if cam.fog_params.z < 0.5 { return color; }
    let dist = length(world_pos - cam.eye.xyz);
    let t = saturate((dist - cam.fog_params.x) / max(1e-3, cam.fog_params.y - cam.fog_params.x));
    return mix(color, cam.fog_color.rgb, t);
}

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    return saturate((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14));
}

fn light_one(n: vec3<f32>, world_pos: vec3<f32>, pos: vec4<f32>, col: vec4<f32>, spot: vec4<f32>, inner: f32) -> vec3<f32> {
    if col.w < 1e-4 { return vec3<f32>(0.0); }
    let to_l = pos.xyz - world_pos;
    let dist = length(to_l);
    let radius = max(pos.w, 0.05);
    let atten = saturate(1.0 - dist / radius);
    let ndotl = saturate(dot(n, normalize(to_l)));
    var cone = 1.0;
    if spot.w > 1e-4 {
        let from_l = normalize(-to_l);
        let axis = normalize(spot.xyz);
        let c = dot(from_l, axis);
        let outer = spot.w;
        if inner - outer < 1e-4 {
            cone = select(0.0, 1.0, c >= outer);
        } else {
            cone = smoothstep(outer, inner, c);
        }
    }
    return col.rgb * col.w * ndotl * atten * atten * cone;
}
fn local_lit(n: vec3<f32>, world_pos: vec3<f32>, loc_sh: f32) -> vec3<f32> {
    var s = light_one(n, world_pos, cam.point_pos, cam.point_col, cam.spot_dir, cam.env.z) * loc_sh;
    s = s + light_one(n, world_pos, cam.extra_pos[0], cam.extra_col[0], cam.extra_spot[0], cam.extra_inner.x);
    s = s + light_one(n, world_pos, cam.extra_pos[1], cam.extra_col[1], cam.extra_spot[1], cam.extra_inner.y);
    s = s + light_one(n, world_pos, cam.extra_pos[2], cam.extra_col[2], cam.extra_spot[2], cam.extra_inner.z);
    return s;
}

fn animated_uv(uv: vec2<f32>) -> vec2<f32> {
    let speeds = mtoon.uv_anim.xyz;
    if length(speeds) < 1e-6 {
        return uv;
    }
    let t = cam.eye.w;
    let mask = textureSample(t_uvmask, s_diffuse, uv).r;
    let scrolled = uv + speeds.xy * t;
    let ang = speeds.z * t;
    let c = cos(ang);
    let s = sin(ang);
    let p = scrolled - vec2<f32>(0.5, 0.5);
    let rotated = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c) + vec2<f32>(0.5, 0.5);
    return mix(uv, rotated, mask);
}

fn cotangent_frame(n: vec3<f32>, p: vec3<f32>, uv: vec2<f32>) -> mat3x3<f32> {
    let dp1 = dpdx(p);
    let dp2 = dpdy(p);
    let duv1 = dpdx(uv);
    let duv2 = dpdy(uv);
    let dp2perp = cross(dp2, n);
    let dp1perp = cross(n, dp1);
    var t = dp2perp * duv1.x + dp1perp * duv2.x;
    var b = dp2perp * duv1.y + dp1perp * duv2.y;
    let invmax = inverseSqrt(max(dot(t, t), dot(b, b)));
    t = t * invmax;
    b = b * invmax;
    return mat3x3<f32>(t, b, n);
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
    // Write pass binds a per-layer 256-byte buffer; vp0 is that layer's matrix.
    return shadow_u.vp0 * world_pos;
}

@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    let uv = animated_uv(in.uv);
    var base = textureSample(t_diffuse, s_diffuse, uv);
    if base.a < 0.05 { discard; }
    var n = normalize(in.world_nrm);
    if mtoon.uv_anim.w > 0.5 {
        let nts = textureSample(t_normal, s_diffuse, uv).xyz * 2.0 - 1.0;
        n = normalize(cotangent_frame(n, in.world_pos, uv) * nts);
    }
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
        let shade_tex = textureSample(t_shade, s_diffuse, uv);
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
    // light_dir.w = グローバルリム。0 なら従来どおり（ゴールデン互換）。
    let global_rim = cam.light_dir.w;
    if global_rim > 0.001 {
        let fresnel = pow(max(1.0 - saturate(dot(n, V)), 0.0), 3.0);
        let back = saturate(-dot(n, light_dir));
        col = col + (fresnel * 0.55 + back * 0.45) * global_rim * vec3<f32>(1.0, 0.90, 0.72);
        let xz = length(in.world_pos.xz);
        let up = saturate(dot(n, vec3<f32>(0.0, 1.0, 0.0)));
        let spot = exp(-xz * xz * 0.55) * up;
        col = col + spot * global_rim * 0.32 * vec3<f32>(1.0, 0.86, 0.58);
    }
    if mtoon.matcap_color.a > 0.5 {
        let vn = normalize((cam.view * vec4<f32>(n, 0.0)).xyz);
        let muv = vn.xy * 0.5 + vec2<f32>(0.5, 0.5);
        let mc = textureSample(t_matcap, s_diffuse, muv).rgb * mtoon.matcap_color.rgb;
        col = col + mc;
    }
    let sh = shadow_factor(in.world_pos);
    let sun_sh = select(sh, 1.0, shadow_u.params.y > 0.5);
    let loc_sh = select(1.0, sh, shadow_u.params.y > 0.5);
    col = col * sun_sh;
    if cam.ambient.w > 1e-4 {
        let hemi = 0.45 + 0.55 * saturate(n.y * 0.5 + 0.5);
        col = col + cam.ambient.rgb * cam.ambient.w * hemi;
    }
    col = col + local_lit(n, in.world_pos, loc_sh) * base.rgb;
    if cam.env.x > 1e-4 {
        col = col + textureSample(env_irr, env_samp, n).rgb * cam.env.x * base.rgb * 0.35;
    }
    col = apply_fog(col, in.world_pos);
    col = col * max(cam.env.y, 0.0);
    if cam.env.w > 0.5 {
        col = aces_tonemap(col);
    }
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

/// 閾値抽出 → 分離ガウス → シャープなフレームへ加算。
/// 画面全体はぼかさない（トゥーン輪郭を濁さない）。
pub(super) const BLOOM_SHADER: &str = r#"
struct VO {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VO {
    // uv (0,0) → clip 左上。テクスチャ原点も左上に合わせる
    var uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    var out: VO;
    out.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    return out;
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fs_extract(in: VO) -> @location(0) vec4<f32> {
    let c = textureSample(t_src, s_src, in.uv);
    let lum = luminance(c.rgb);
    let threshold = params.x;
    let knee = max(params.y, 1e-4);
    let soft = lum - threshold + knee;
    let soft_c = clamp(soft, 0.0, 2.0 * knee);
    let soft_t = (soft_c * soft_c) / (4.0 * knee);
    let contrib = max(lum - threshold, soft_t);
    let w = contrib / max(lum, 1e-5);
    return vec4<f32>(c.rgb * w, 1.0);
}

@fragment
fn fs_blur(in: VO) -> @location(0) vec4<f32> {
    let texel = params.xy;
    let w0 = 0.227027;
    let w1 = 0.1945946;
    let w2 = 0.1216216;
    let w3 = 0.054054;
    let w4 = 0.016216;
    var sum = textureSample(t_src, s_src, in.uv).rgb * w0;
    sum += textureSample(t_src, s_src, in.uv + texel * 1.0).rgb * w1;
    sum += textureSample(t_src, s_src, in.uv - texel * 1.0).rgb * w1;
    sum += textureSample(t_src, s_src, in.uv + texel * 2.0).rgb * w2;
    sum += textureSample(t_src, s_src, in.uv - texel * 2.0).rgb * w2;
    sum += textureSample(t_src, s_src, in.uv + texel * 3.0).rgb * w3;
    sum += textureSample(t_src, s_src, in.uv - texel * 3.0).rgb * w3;
    sum += textureSample(t_src, s_src, in.uv + texel * 4.0).rgb * w4;
    sum += textureSample(t_src, s_src, in.uv - texel * 4.0).rgb * w4;
    return vec4<f32>(sum, 1.0);
}

@group(2) @binding(0) var t_bloom: texture_2d<f32>;
@group(2) @binding(1) var s_bloom: sampler;

@fragment
fn fs_composite(in: VO) -> @location(0) vec4<f32> {
    let sharp = textureSample(t_src, s_src, in.uv);
    let bloom = textureSample(t_bloom, s_bloom, in.uv);
    return vec4<f32>(sharp.rgb + bloom.rgb * params.x, sharp.a);
}
"#;
