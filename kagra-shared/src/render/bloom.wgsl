// Threshold bloom, ported from kagra-core (wgpu 0.19) bloom.rs / BLOOM_SHADER.
// Extract bright pixels at half res, blur H+V, add back to the sharp frame.
// Full-screen triangle (draw 0..3). Works on wgpu 30 / WebGL2.

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

// Narkowicz ACES, same as kagra-core V2 / shader3d.wgsl. Applied here because
// the frame is now a linear HDR target (bloom must extract pre-tonemap).
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    return saturate((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14));
}

// params.x = bloom intensity, params.y = exposure, params.z = ACES on (>0.5).
@fragment
fn fs_composite(in: VO) -> @location(0) vec4<f32> {
    let sharp = textureSample(t_src, s_src, in.uv);
    let bloom = textureSample(t_bloom, s_bloom, in.uv);
    var c = sharp.rgb + bloom.rgb * params.x;
    c = c * max(params.y, 0.0);
    if (params.z > 0.5) {
        c = aces_tonemap(c);
    }
    return vec4<f32>(c, sharp.a);
}
