// Threshold bloom, ported from kagra-core (wgpu 0.19) bloom.rs / BLOOM_SHADER.
// Extract bright pixels at half res, blur H+V, add back to the sharp frame.
// Full-screen triangle (draw 0..3). Works on wgpu 30 / WebGL2.

struct VO {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;

// Two vec4s so extract/blur and the grade share one uniform block.
// extra.x is the LUT mix. Extract and blur only read `v`.
struct Params {
    v: vec4<f32>,
    extra: vec4<f32>,
}
@group(1) @binding(0) var<uniform> params: Params;

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
    let threshold = params.v.x;
    let knee = max(params.v.y, 1e-4);
    let soft = lum - threshold + knee;
    let soft_c = clamp(soft, 0.0, 2.0 * knee);
    let soft_t = (soft_c * soft_c) / (4.0 * knee);
    let contrib = max(lum - threshold, soft_t);
    let w = contrib / max(lum, 1e-5);
    return vec4<f32>(c.rgb * w, 1.0);
}

@fragment
fn fs_blur(in: VO) -> @location(0) vec4<f32> {
    let texel = params.v.xy;
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
@group(2) @binding(2) var t_lut: texture_2d<f32>;
@group(2) @binding(3) var s_lut: sampler;

// Narkowicz ACES, same as kagra-core V2 / shader3d.wgsl. Applied here because
// the frame is now a linear HDR target (bloom must extract pre-tonemap).
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    return saturate((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14));
}

// 16³ strip, 16 slices across. Manual slice lerp so linear filtering stays
// inside one slice (WebGL2 texture_2d, no texture_3d).
fn sample_lut(color: vec3<f32>) -> vec3<f32> {
    let n = 16.0;
    let c = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    let z = c.b * (n - 1.0);
    let z0 = floor(z);
    let z1 = min(z0 + 1.0, n - 1.0);
    let fz = z - z0;
    let y = (c.g * (n - 1.0) + 0.5) / n;
    let x0 = (z0 * n + c.r * (n - 1.0) + 0.5) / (n * n);
    let x1 = (z1 * n + c.r * (n - 1.0) + 0.5) / (n * n);
    let a = textureSample(t_lut, s_lut, vec2<f32>(x0, y)).rgb;
    let b = textureSample(t_lut, s_lut, vec2<f32>(x1, y)).rgb;
    return mix(a, b, fz);
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let cut = c <= vec3<f32>(0.0031308);
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, cut);
}

// params.v.x = bloom intensity, v.y = exposure, v.z = ACES on (>0.5),
// v.w = vignette (1 = the historical curve, 0 = off).
// params.extra.x = LUT mix after ACES. 0 skips the sample.
// params.extra.y > 0.5 encodes sRGB in the shader. WebGPU canvases are unorm
// (no sRGB format), so the hardware will not apply the OETF. sRGB targets
// leave this at 0 and encode on write.
@fragment
fn fs_composite(in: VO) -> @location(0) vec4<f32> {
    let sharp = textureSample(t_src, s_src, in.uv);
    let bloom = textureSample(t_bloom, s_bloom, in.uv);
    var c = sharp.rgb + bloom.rgb * params.v.x;
    c = c * max(params.v.y, 0.0);
    if (params.v.z > 0.5) {
        c = aces_tonemap(c);
    }
    let lut_mix = clamp(params.extra.x, 0.0, 1.0);
    if (lut_mix > 1e-4) {
        c = mix(c, sample_lut(c), lut_mix);
    }
    let d = (in.uv - vec2<f32>(0.5, 0.5)) * vec2<f32>(1.28, 1.08);
    let r2 = dot(d, d);
    let vig = clamp(1.0 - r2 * 1.05, 0.28, 1.0);
    let v = clamp(params.v.w, 0.0, 2.0);
    if (v <= 1.0) {
        c = mix(c, c * vig, v);
    } else {
        c = c * vig * mix(1.0, vig, v - 1.0);
    }
    if (params.extra.y > 0.5) {
        c = linear_to_srgb(max(c, vec3<f32>(0.0)));
    }
    return vec4<f32>(c, sharp.a);
}
