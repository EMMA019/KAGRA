// Switchable post passes. The graph matches three.js EffectComposer order
// (MIT, https://github.com/mrdoob/three.js): SSAO, DOF, camera motion,
// then the existing bloom grade, then TAA or FXAA, then vignette.
// Algorithms follow three.js SSAOShader, BokehPass, and the Karis
// neighborhood clamp used by TAA examples. No three.js source is copied.
// Full-screen triangle (draw 0..3). No storage buffers. Every textureSample
// stays in uniform control flow (WebGPU).

struct VO {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;

// v / extra are the pass knobs. m* is one mat4 (inverse view-projection for
// motion). p* is the previous view-projection. Column-major, same as glam.
struct Params {
    v: vec4<f32>,
    extra: vec4<f32>,
    m0: vec4<f32>,
    m1: vec4<f32>,
    m2: vec4<f32>,
    m3: vec4<f32>,
    p0: vec4<f32>,
    p1: vec4<f32>,
    p2: vec4<f32>,
    p3: vec4<f32>,
}
@group(1) @binding(0) var<uniform> params: Params;

@group(2) @binding(0) var t_extra: texture_2d<f32>;
@group(2) @binding(1) var s_extra: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VO {
    var uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    var out: VO;
    out.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    return out;
}

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn keep_alpha(color_a: f32, keep: f32) -> f32 {
    return select(1.0, color_a, keep > 0.5);
}

fn device_z(eye: f32, near: f32, far: f32) -> f32 {
    return clamp(far * (eye - near) / max(eye * (far - near), 1e-4), 0.0, 1.0);
}

// Half-resolution SSAO. t_src.r is linear eye depth. HDR stores meters.
// An unorm depth stores eye/span in 0..1 (params.m0.x > 1) because meters
// above 1 would clamp. Samples are taken first, then scaled, so the sample
// stays in uniform control flow. Sky on the packed path is a stored value
// near 1. Alpha keeps meters on HDR and the packed value on unorm.
@fragment
fn fs_ssao(in: VO) -> @location(0) vec4<f32> {
    let texel = params.extra.zw;
    let stored = textureSample(t_src, s_src, in.uv).r;
    let zx_s = textureSample(t_src, s_src, in.uv + vec2<f32>(texel.x, 0.0)).r;
    let zy_s = textureSample(t_src, s_src, in.uv + vec2<f32>(0.0, texel.y)).r;
    let span = params.m0.x;
    let z = select(stored, stored * span, span > 1.0);
    let zx = select(zx_s, zx_s * span, span > 1.0);
    let zy = select(zy_s, zy_s * span, span > 1.0);
    let radius = max(params.v.x, 0.0);
    let bias = max(params.v.y, 1e-4);
    let intensity = params.v.z;
    let range = max(params.v.w, bias);
    let far = max(params.extra.y, 1.0);
    let gx = zx - z;
    let gy = zy - z;
    let rot = hash21(in.uv) * 6.2831853;
    var occ = 0.0;
    for (var i = 0u; i < 8u; i = i + 1u) {
        let a = rot + f32(i) * 2.3999632;
        let t = (f32(i) + 0.5) / 8.0;
        let off = vec2<f32>(cos(a), sin(a)) * radius * mix(0.35, 1.0, t);
        let seye_s = textureSample(t_src, s_src, in.uv + off * texel).r;
        let seye = select(seye_s, seye_s * span, span > 1.0);
        let predicted = z + dot(vec2<f32>(gx, gy), off);
        let closer = predicted - seye;
        let near_w = smoothstep(bias, bias + range * 0.35, closer);
        let far_w = 1.0 - smoothstep(range, range * 1.75, closer);
        occ = occ + near_w * far_w;
    }
    let sky_hdr = select(0.0, 1.0, z > far * 0.98);
    let sky_norm = select(0.0, 1.0, stored > 0.98);
    let sky = select(sky_hdr, sky_norm, span > 1.0);
    let ao = clamp(1.0 - occ / 8.0 * intensity, 0.2, 1.0);
    let ao_out = mix(ao, 1.0, sky);
    let alpha_out = select(z, stored, span > 1.0);
    return vec4<f32>(vec3<f32>(ao_out), alpha_out);
}

@fragment
fn fs_ao_blur(in: VO) -> @location(0) vec4<f32> {
    let src = textureSample(t_src, s_src, in.uv);
    let dir = params.v.xy;
    var sum = src.rgb;
    var wsum = 1.0;
    for (var i = 1u; i <= 3u; i = i + 1u) {
        let o = dir * f32(i);
        let a = textureSample(t_src, s_src, in.uv + o);
        let b = textureSample(t_src, s_src, in.uv - o);
        // v.z is the depth span. 0 stores meters in alpha (scale 1). A packed
        // unorm alpha is eye/span, so the same meter weight needs *span.
        let depth_scale = max(params.v.z, 1.0);
        let wa = exp(-abs(a.a - src.a) * depth_scale * 1.5) * exp(-f32(i) * 0.4);
        let wb = exp(-abs(b.a - src.a) * depth_scale * 1.5) * exp(-f32(i) * 0.4);
        sum = sum + a.rgb * wa + b.rgb * wb;
        wsum = wsum + wa + wb;
    }
    return vec4<f32>(sum / max(wsum, 1e-4), src.a);
}

@fragment
fn fs_ao_apply(in: VO) -> @location(0) vec4<f32> {
    let src = textureSample(t_src, s_src, in.uv);
    let ao = textureSample(t_extra, s_extra, in.uv).r;
    return vec4<f32>(src.rgb * vec3<f32>(ao), keep_alpha(src.a, params.v.x));
}

// Disk gather. t_extra.r is linear eye depth in meters. Aperture 0 keeps
// every tap on the source pixel. Color alpha stays the source alpha.
@fragment
fn fs_dof(in: VO) -> @location(0) vec4<f32> {
    let src = textureSample(t_src, s_src, in.uv);
    let focus = params.v.x;
    let aperture = max(params.v.y, 0.0);
    let max_r = max(params.v.z, 0.0);
    let texel = params.extra.zw;
    let eye = textureSample(t_extra, s_extra, in.uv).r;
    let coc = clamp(abs(eye - focus) * aperture, 0.0, max_r);
    let rot = hash21(in.uv) * 6.2831853;
    var sum = src.rgb;
    var wsum = 1.0;
    for (var i = 0u; i < 8u; i = i + 1u) {
        let a = rot + f32(i) * 2.3999632;
        let t = (f32(i) + 0.5) / 8.0;
        let off = vec2<f32>(cos(a), sin(a)) * coc * t;
        let uv = in.uv + off * texel;
        let s = textureSample(t_src, s_src, uv);
        let seye = textureSample(t_extra, s_extra, uv).r;
        let scoc = clamp(abs(seye - focus) * aperture, 0.0, max_r);
        let w = 0.55 + 0.45 * smoothstep(0.0, max(coc, 1.0), scoc);
        sum = sum + s.rgb * w;
        wsum = wsum + w;
    }
    return vec4<f32>(sum / wsum, keep_alpha(src.a, params.v.w));
}

// Camera-only velocity. t_extra.r is linear eye depth; it is converted back
// to device z with the same projection. A still camera has zero extent.
@fragment
fn fs_motion(in: VO) -> @location(0) vec4<f32> {
    let src = textureSample(t_src, s_src, in.uv);
    let eye = textureSample(t_extra, s_extra, in.uv).r;
    let depth = device_z(eye, params.extra.x, params.extra.y);
    let ndc = vec4<f32>(in.uv.x * 2.0 - 1.0, (1.0 - in.uv.y) * 2.0 - 1.0, depth, 1.0);
    let inv_vp = mat4x4<f32>(params.m0, params.m1, params.m2, params.m3);
    let prev_vp = mat4x4<f32>(params.p0, params.p1, params.p2, params.p3);
    let world_h = inv_vp * ndc;
    let world = world_h.xyz / max(world_h.w, 1e-6);
    let prev_c = prev_vp * vec4<f32>(world, 1.0);
    let prev_w = select(prev_c.w, 1e-4, abs(prev_c.w) < 1e-4);
    let prev_uv = vec2<f32>(prev_c.x / prev_w * 0.5 + 0.5, 0.5 - prev_c.y / prev_w * 0.5);
    let vel = (in.uv - prev_uv) * params.v.x;
    var sum = src.rgb;
    for (var i = 1u; i < 8u; i = i + 1u) {
        let t = f32(i) / 7.0 - 0.5;
        sum = sum + textureSample(t_src, s_src, in.uv + vel * t).rgb;
    }
    return vec4<f32>(sum / 8.0, keep_alpha(src.a, params.v.y));
}

// Neighborhood clamp (Karis). blend 0 is the first frame: history is ignored.
@fragment
fn fs_taa(in: VO) -> @location(0) vec4<f32> {
    let curr = textureSample(t_src, s_src, in.uv).rgb;
    let texel = params.extra.xy;
    var cmin = curr;
    var cmax = curr;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let n = textureSample(t_src, s_src, in.uv + vec2<f32>(f32(x), f32(y)) * texel).rgb;
            cmin = min(cmin, n);
            cmax = max(cmax, n);
        }
    }
    let hist = textureSample(t_extra, s_extra, in.uv).rgb;
    let clamped = clamp(hist, cmin, cmax);
    let blend = clamp(params.v.x, 0.0, 0.95);
    return vec4<f32>(mix(curr, clamped, blend), 1.0);
}

// Same curve as bloom.wgsl fs_composite. Runs after TAA, in the space the
// composite texture already stored (linear on sample when the target is sRGB).
@fragment
fn fs_vignette(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_src, s_src, in.uv).rgb;
    let d = (in.uv - vec2<f32>(0.5, 0.5)) * vec2<f32>(1.28, 1.08);
    let r2 = dot(d, d);
    let vig = clamp(1.0 - r2 * 1.05, 0.28, 1.0);
    let v = clamp(params.v.w, 0.0, 2.0);
    if (v <= 1.0) {
        c = mix(c, c * vig, v);
    } else {
        c = c * vig * mix(1.0, vig, v - 1.0);
    }
    return vec4<f32>(c, 1.0);
}

@fragment
fn fs_copy(in: VO) -> @location(0) vec4<f32> {
    return textureSample(t_src, s_src, in.uv) * params.v.x;
}
