// 2D クアッド。画面ピクセル座標（左上原点）を NDC に変換するだけ。

struct Screen {
    // xy = 画面サイズ(px), zw = 予約
    size: vec4<f32>,
};

@group(0) @binding(0) var<uniform> screen: Screen;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_samp: sampler;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
    // rounded quad: offset from its centre (px); glyph: atlas uv
    @location(2) local: vec2<f32>,
    // half width, half height, corner radius, kind (0 flat, 1 rounded, 2 glyph)
    @location(3) shape: vec4<f32>,
    // rounded quad: outline width, 0 = filled
    @location(4) stroke: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local: vec2<f32>,
    @location(2) shape: vec4<f32>,
    @location(3) stroke: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let ndc = vec2<f32>(
        in.pos.x / max(screen.size.x, 1.0) * 2.0 - 1.0,
        1.0 - in.pos.y / max(screen.size.y, 1.0) * 2.0,
    );
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.color = in.color;
    out.local = in.local;
    out.shape = in.shape;
    out.stroke = in.stroke;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let kind = in.shape.w;
    if (kind < 0.5) {
        return in.color;
    }
    if (kind < 1.5) {
        // Rounded box SDF in pixels; one-pixel soft edge.
        let r = in.shape.z;
        let q = abs(in.local) - in.shape.xy + vec2<f32>(r, r);
        let d = length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
        var a = clamp(0.5 - d, 0.0, 1.0);
        if (in.stroke > 0.0) {
            // Only the band `stroke` px wide inside the edge.
            a = a * clamp(0.5 + d + in.stroke, 0.0, 1.0);
        }
        return vec4<f32>(in.color.rgb, in.color.a * a);
    }
    // Glyph coverage. Level 0 explicitly: this branch is not uniform control flow.
    let c = textureSampleLevel(atlas, atlas_samp, in.local, 0.0).r;
    return vec4<f32>(in.color.rgb, in.color.a * c);
}
