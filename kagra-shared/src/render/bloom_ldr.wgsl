// 8-bit bloom. The pass follows three.js UnrealBloomPass (MIT,
// https://github.com/mrdoob/three.js): a luma high-pass, then a mip chain
// that downsamples and upsamples. The WGSL is original. No three.js source
// is copied. An emissive mask from the 3D pass blooms saturated neon that
// never exceeds 1 on an 8-bit frame.

struct VO {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;
@group(0) @binding(2) var t_emit: texture_2d<f32>;
@group(0) @binding(3) var s_emit: sampler;

struct Params {
    v: vec4<f32>,
    extra: vec4<f32>,
}
@group(1) @binding(0) var<uniform> params: Params;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VO {
    var uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    var out: VO;
    out.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    return out;
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// v.x = luma threshold in 0..1, v.y = knee. t_emit.r is emissive strength.
@fragment
fn fs_extract(in: VO) -> @location(0) vec4<f32> {
    let c = textureSample(t_src, s_src, in.uv);
    let emit = textureSample(t_emit, s_emit, in.uv).r;
    let lum = luminance(c.rgb);
    let threshold = params.v.x;
    let knee = max(params.v.y, 1e-4);
    let soft = lum - threshold + knee;
    let soft_c = clamp(soft, 0.0, 2.0 * knee);
    let soft_t = (soft_c * soft_c) / (4.0 * knee);
    let contrib = max(lum - threshold, soft_t);
    let luma_w = contrib / max(lum, 1e-5);
    // Colored neon can sit under the luma bar after the 8-bit clamp.
    let emit_w = smoothstep(0.20, 0.55, emit);
    let w = max(luma_w, emit_w);
    return vec4<f32>(c.rgb * w, 1.0);
}

fn tap(uv: vec2<f32>) -> vec3<f32> {
    return textureSample(t_src, s_src, uv).rgb;
}

// v.xy = source texel. v.z > 0.5 is the first hop (Karis weight).
@fragment
fn fs_down(in: VO) -> @location(0) vec4<f32> {
    let t = params.v.xy;
    let o = t * 0.5;
    let c0 = tap(in.uv + vec2<f32>(-o.x, -o.y));
    let c1 = tap(in.uv + vec2<f32>(o.x, -o.y));
    let c2 = tap(in.uv + vec2<f32>(-o.x, o.y));
    let c3 = tap(in.uv + vec2<f32>(o.x, o.y));
    if (params.v.z > 0.5) {
        let w0 = 1.0 / (1.0 + luminance(c0));
        let w1 = 1.0 / (1.0 + luminance(c1));
        let w2 = 1.0 / (1.0 + luminance(c2));
        let w3 = 1.0 / (1.0 + luminance(c3));
        let sum = c0 * w0 + c1 * w1 + c2 * w2 + c3 * w3;
        return vec4<f32>(sum / max(w0 + w1 + w2 + w3, 1e-4), 1.0);
    }
    return vec4<f32>((c0 + c1 + c2 + c3) * 0.25, 1.0);
}

// Tent of the smaller mip. The pipeline adds this onto the larger mip.
@fragment
fn fs_up(in: VO) -> @location(0) vec4<f32> {
    let t = params.v.xy;
    var sum = tap(in.uv) * 0.25;
    sum += tap(in.uv + vec2<f32>(t.x, 0.0)) * 0.125;
    sum += tap(in.uv + vec2<f32>(-t.x, 0.0)) * 0.125;
    sum += tap(in.uv + vec2<f32>(0.0, t.y)) * 0.125;
    sum += tap(in.uv + vec2<f32>(0.0, -t.y)) * 0.125;
    sum += tap(in.uv + t) * 0.0625;
    sum += tap(in.uv + vec2<f32>(-t.x, t.y)) * 0.0625;
    sum += tap(in.uv + vec2<f32>(t.x, -t.y)) * 0.0625;
    sum += tap(in.uv - t) * 0.0625;
    return vec4<f32>(sum * 0.65, 1.0);
}
