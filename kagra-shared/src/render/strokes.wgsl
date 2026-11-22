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

// Point atlas (group 1 is bound for points only; lines never touch it).
// Untextured batches bind a 1x1 white texture and never sample it.
@group(1) @binding(0) var atlas_tex: texture_2d<f32>;
@group(1) @binding(1) var atlas_smp: sampler;

struct FragOut {
    @location(0) color: vec4<f32>,
    @location(1) emit: vec4<f32>,
};

struct PointIn {
    @location(0) corner: vec2<f32>,
    @location(1) pos: vec4<f32>,
    @location(2) color: vec4<f32>,
    // (u0, v0, u1, v1) of this point's frame; v0 is the sheet's TOP row.
    @location(3) uv_rect: vec4<f32>,
    // (rotation radians, textured flag, emit scale, unused)
    @location(4) misc: vec4<f32>,
};

struct PointOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) corner: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) misc: vec4<f32>,
};

@vertex
fn vs_point(input: PointIn) -> PointOut {
    let c = cos(input.misc.x);
    let s = sin(input.misc.x);
    let rc = vec2<f32>(
        input.corner.x * c - input.corner.y * s,
        input.corner.x * s + input.corner.y * c,
    );
    let world = input.pos.xyz
        + v.cam_right.xyz * rc.x * input.pos.w
        + v.cam_up.xyz * rc.y * input.pos.w;
    var o: PointOut;
    o.clip = v.view_proj * vec4<f32>(world, 1.0);
    o.corner = input.corner;
    o.color = input.color;
    // UVs come from the UNROTATED corner, so the picture turns with the quad.
    // Corner y = +1 is the top of the billboard and samples v0 (sheet top).
    let t = input.corner * 0.5 + 0.5;
    o.uv = vec2<f32>(
        mix(input.uv_rect.x, input.uv_rect.z, t.x),
        mix(input.uv_rect.w, input.uv_rect.y, t.y),
    );
    o.misc = input.misc;
    return o;
}

// The premultiplied colour a point paints into the frame target: the
// atlas texel (or the soft-disc falloff) times the point colour, with the
// alpha folded into rgb the way both blend states expect.
fn point_color(input: PointOut) -> vec4<f32> {
    var a: f32;
    var tex_rgb = vec3<f32>(1.0);
    if (input.misc.y > 0.5) {
        let tex = textureSample(atlas_tex, atlas_smp, input.uv);
        a = input.color.a * tex.a;
        tex_rgb = tex.rgb;
    } else {
        let r = length(input.corner);
        if (r > 1.0) {
            discard;
        }
        let edge = 1.0 - smoothstep(0.55, 1.0, r);
        a = input.color.a * edge;
    }
    return vec4<f32>(input.color.rgb * tex_rgb * a, a);
}

@fragment
fn fs_point(input: PointOut) -> FragOut {
    let col = point_color(input);
    var o: FragOut;
    o.color = col;
    o.emit = vec4<f32>(col.a * input.misc.z, 0.0, 0.0, 1.0);
    return o;
}

// Alpha-blended points (smoke): the same colour, but they must never
// feed the bloom emit target. The pipeline gives both targets one
// shared blend/write state — a per-target write mask (the old way of
// muting the emit write) requires INDEPENDENT_BLEND, which some
// adapters do not expose — so the muting lives in the shader instead:
// emit is exactly zero, and the shared premultiplied blend over the
// emit target is the identity.
@fragment
fn fs_point_alpha(input: PointOut) -> FragOut {
    var o: FragOut;
    o.color = point_color(input);
    o.emit = vec4<f32>(0.0);
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

// viewport.zw = (start, end) view depth over which a line fades out.
// end <= start disables it.
fn line_fade(depth: f32) -> f32 {
    if (v.viewport.w <= v.viewport.z) {
        return 1.0;
    }
    return 1.0 - smoothstep(v.viewport.z, v.viewport.w, depth);
}

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
    let fade = mix(line_fade(clip_a.w), line_fade(clip_b.w), input.corner.x);
    o.color = vec4<f32>(input.color.rgb, input.color.a * fade);
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
