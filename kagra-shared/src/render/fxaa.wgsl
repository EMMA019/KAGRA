// FXAA (simplified 3.11-style): luma edge detection + directional 2-tap blend.
// Runs on the composite output (sRGB frame) before the HUD is drawn.
// Cheap: 9 texture samples per pixel, no history, no motion vectors.

struct VO {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>; // xy = 1 / texel size

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VO {
    // uv (0,0) → clip 左上。テクスチャ原点も左上に合わせる
    var uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    var out: VO;
    out.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    return out;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

@fragment
fn fs_main(in: VO) -> @location(0) vec4<f32> {
    let rcp = params.xy;
    let center = textureSample(t_src, s_src, in.uv).rgb;
    let l_center = luma(center);

    // 4 近傍の輝度でエッジを検出。
    let l_lt = luma(textureSample(t_src, s_src, in.uv + vec2<f32>(-rcp.x, -rcp.y)).rgb);
    let l_rt = luma(textureSample(t_src, s_src, in.uv + vec2<f32>(rcp.x, -rcp.y)).rgb);
    let l_lb = luma(textureSample(t_src, s_src, in.uv + vec2<f32>(-rcp.x, rcp.y)).rgb);
    let l_rb = luma(textureSample(t_src, s_src, in.uv + vec2<f32>(rcp.x, rcp.y)).rgb);

    let l_min = min(min(l_lt, l_rt), min(l_lb, l_rb));
    let l_max = max(max(l_lt, l_rt), max(l_lb, l_rb));
    let l_range = l_max - l_min;
    // コントラスト閾値（FXAA 3.11 と同じ相対/絶対閾値）。
    if (l_range < max(0.0312, l_max * 0.125)) {
        return vec4<f32>(center, 1.0);
    }

    // エッジ方向: 勾配ベクトル（FXAA 3.11 方式）。エッジの法線を求め、
    // 強い成分（水平勾配 → 縦エッジ）へ 1px のタップ方向を決める。
    let l_up = luma(textureSample(t_src, s_src, in.uv + vec2<f32>(0.0, -rcp.y * 2.0)).rgb);
    let l_down = luma(textureSample(t_src, s_src, in.uv + vec2<f32>(0.0, rcp.y * 2.0)).rgb);
    let l_left = luma(textureSample(t_src, s_src, in.uv + vec2<f32>(-rcp.x * 2.0, 0.0)).rgb);
    let l_right = luma(textureSample(t_src, s_src, in.uv + vec2<f32>(rcp.x * 2.0, 0.0)).rgb);

    let dir_x = -((l_left + l_right) - 2.0 * l_center);
    let dir_y = -((l_up + l_down) - 2.0 * l_center);
    var dir = vec2<f32>(0.0, 0.0);
    if (abs(dir_x) >= abs(dir_y)) {
        dir = vec2<f32>(sign(dir_x), 0.0);
    } else {
        dir = vec2<f32>(0.0, sign(dir_y));
    }

    // エッジを横切る ±1px のタップ。明るい側へ寄せ、コントラスト比で
    // ブレンド量を決める。
    let a = textureSample(t_src, s_src, in.uv + dir * rcp).rgb;
    let b = textureSample(t_src, s_src, in.uv - dir * rcp).rgb;
    let l_a = luma(a);
    let l_b = luma(b);
    let toward_a = (l_a - l_center) > (l_b - l_center);
    let far = select(b, a, toward_a);
    let grad = clamp(l_range * 2.5, 0.0, 1.0);
    let col = mix(center, far, grad * 0.5);
    return vec4<f32>(col, 1.0);
}
