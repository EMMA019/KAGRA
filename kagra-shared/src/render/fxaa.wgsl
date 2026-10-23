// FXAA (simplified 3.11-style): luma edge detection + directional 2-tap blend.
// Runs on the composite output (sRGB frame) before the HUD is drawn.
// Samples are taken before any return so WebGPU's uniform-control-flow
// rule accepts the shader. No history, no motion vectors.

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

// Copy the composite onto a swapchain that cannot be a copy destination
// (WebGL2 canvases are RENDER_ATTACHMENT only).
@fragment
fn fs_blit(in: VO) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(t_src, s_src, in.uv).rgb, 1.0);
}

@fragment
fn fs_main(in: VO) -> @location(0) vec4<f32> {
    let rcp = params.xy;
    // Every textureSample stays above the early return. WebGPU treats a
    // return that depends on sampled luma as non-uniform, so a sample after
    // it is rejected and the pass draws nothing.
    let center = textureSample(t_src, s_src, in.uv).rgb;
    let l_center = luma(center);

    let rgb_lt = textureSample(t_src, s_src, in.uv + vec2<f32>(-rcp.x, -rcp.y)).rgb;
    let rgb_rt = textureSample(t_src, s_src, in.uv + vec2<f32>(rcp.x, -rcp.y)).rgb;
    let rgb_lb = textureSample(t_src, s_src, in.uv + vec2<f32>(-rcp.x, rcp.y)).rgb;
    let rgb_rb = textureSample(t_src, s_src, in.uv + vec2<f32>(rcp.x, rcp.y)).rgb;
    let rgb_up = textureSample(t_src, s_src, in.uv + vec2<f32>(0.0, -rcp.y)).rgb;
    let rgb_down = textureSample(t_src, s_src, in.uv + vec2<f32>(0.0, rcp.y)).rgb;
    let rgb_left = textureSample(t_src, s_src, in.uv + vec2<f32>(-rcp.x, 0.0)).rgb;
    let rgb_right = textureSample(t_src, s_src, in.uv + vec2<f32>(rcp.x, 0.0)).rgb;
    let rgb_up2 = textureSample(t_src, s_src, in.uv + vec2<f32>(0.0, -rcp.y * 2.0)).rgb;
    let rgb_down2 = textureSample(t_src, s_src, in.uv + vec2<f32>(0.0, rcp.y * 2.0)).rgb;
    let rgb_left2 = textureSample(t_src, s_src, in.uv + vec2<f32>(-rcp.x * 2.0, 0.0)).rgb;
    let rgb_right2 = textureSample(t_src, s_src, in.uv + vec2<f32>(rcp.x * 2.0, 0.0)).rgb;

    let l_lt = luma(rgb_lt);
    let l_rt = luma(rgb_rt);
    let l_lb = luma(rgb_lb);
    let l_rb = luma(rgb_rb);
    let l_min = min(min(l_lt, l_rt), min(l_lb, l_rb));
    let l_max = max(max(l_lt, l_rt), max(l_lb, l_rb));
    let l_range = l_max - l_min;
    // コントラスト閾値（FXAA 3.11 と同じ相対/絶対閾値）。
    if (l_range < max(0.0312, l_max * 0.125)) {
        return vec4<f32>(center, 1.0);
    }

    // エッジ方向: 勾配ベクトル（FXAA 3.11 方式）。エッジの法線を求め、
    // 強い成分（水平勾配 → 縦エッジ）へ 1px のタップ方向を決める。
    let l_up = luma(rgb_up2);
    let l_down = luma(rgb_down2);
    let l_left = luma(rgb_left2);
    let l_right = luma(rgb_right2);

    let dir_x = -((l_left + l_right) - 2.0 * l_center);
    let dir_y = -((l_up + l_down) - 2.0 * l_center);
    var a: vec3<f32>;
    var b: vec3<f32>;
    if (abs(dir_x) >= abs(dir_y)) {
        if (dir_x >= 0.0) {
            a = rgb_right;
            b = rgb_left;
        } else {
            a = rgb_left;
            b = rgb_right;
        }
    } else if (dir_y >= 0.0) {
        a = rgb_down;
        b = rgb_up;
    } else {
        a = rgb_up;
        b = rgb_down;
    }

    // エッジを横切る ±1px のタップ。明るい側へ寄せ、コントラスト比で
    // ブレンド量を決める。
    let l_a = luma(a);
    let l_b = luma(b);
    let toward_a = (l_a - l_center) > (l_b - l_center);
    let far = select(b, a, toward_a);
    let grad = clamp(l_range * 2.5, 0.0, 1.0);
    let col = mix(center, far, grad * 0.5);
    return vec4<f32>(col, 1.0);
}
