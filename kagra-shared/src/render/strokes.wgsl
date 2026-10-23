// Camera-facing points and screen-space fat lines.
//
// The point quad is the idea behind three.js Points (MIT,
// https://github.com/mrdoob/three.js). WebGPU point-list is one pixel and
// this path has no gl_PointSize, so the vertex shader builds a billboard.
// The line quad is the idea behind three.js Line2: project the segment,
// then offset in pixels. This WGSL is original. No three.js source is copied.
// No storage buffers. Instances start at 0 (no base instance).

struct View {
    view_proj: mat4x4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    viewport: vec4<f32>,
};

@group(0) @binding(0) var<uniform> v: View;

struct FragOut {
    @location(0) color: vec4<f32>,
    @location(1) emit: vec4<f32>,
};

struct PointIn {
    @location(0) corner: vec2<f32>,
    @location(1) pos: vec4<f32>,
    @location(2) color: vec4<f32>,
};

struct PointOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) corner: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_point(input: PointIn) -> PointOut {
    let world = input.pos.xyz
        + v.cam_right.xyz * input.corner.x * input.pos.w
        + v.cam_up.xyz * input.corner.y * input.pos.w;
    var o: PointOut;
    o.clip = v.view_proj * vec4<f32>(world, 1.0);
    o.corner = input.corner;
    o.color = input.color;
    return o;
}

@fragment
fn fs_point(input: PointOut) -> FragOut {
    let r = length(input.corner);
    if (r > 1.0) {
        discard;
    }
    let edge = 1.0 - smoothstep(0.55, 1.0, r);
    let a = input.color.a * edge;
    let rgb = input.color.rgb * a;
    var o: FragOut;
    o.color = vec4<f32>(rgb, a);
    o.emit = vec4<f32>(a, 0.0, 0.0, 1.0);
    return o;
}

struct LineIn {
    @location(0) corner: vec2<f32>,
    @location(1) a: vec4<f32>,
    @location(2) b: vec4<f32>,
    @location(3) color: vec4<f32>,
};

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_line(input: LineIn) -> LineOut {
    let clip_a = v.view_proj * vec4<f32>(input.a.xyz, 1.0);
    let clip_b = v.view_proj * vec4<f32>(input.b.xyz, 1.0);
    let ndc_a = clip_a.xy / clip_a.w;
    let ndc_b = clip_b.xy / clip_b.w;
    var dir = (ndc_b - ndc_a) * v.viewport.xy * 0.5;
    let len = length(dir);
    let side = select(vec2<f32>(0.0, 1.0), vec2<f32>(-dir.y, dir.x) / len, len > 1e-3);
    let offset_px = side * input.corner.y * input.a.w * 0.5;
    let ndc = mix(ndc_a, ndc_b, input.corner.x) + offset_px / v.viewport.xy * 2.0;
    let clip_w = mix(clip_a.w, clip_b.w, input.corner.x);
    var o: LineOut;
    o.clip = vec4<f32>(ndc * clip_w, mix(clip_a.z, clip_b.z, input.corner.x), clip_w);
    o.color = input.color;
    return o;
}

@fragment
fn fs_line(input: LineOut) -> FragOut {
    let a = input.color.a;
    let rgb = input.color.rgb * a;
    var o: FragOut;
    o.color = vec4<f32>(rgb, a);
    o.emit = vec4<f32>(a, 0.0, 0.0, 1.0);
    return o;
}
