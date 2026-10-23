// 3D: インスタンス化したメッシュ + 方向光 + 距離フォグ + 手続きテクスチャ。
//
// マテリアルはインスタンス属性の material（0=solid, 1=road, 2=grass, 3=sky, 4=metal, 5=toon, 6=water）。
// 路面と草はワールド XZ のノイズ。baseColor は group 1（無ければ 1x1 白）。

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz = ライトへ向かう方向、w = 環境光の強さ
    light: vec4<f32>,
    // rgb = フォグ色（リニア）
    fog_color: vec4<f32>,
    // x = 効き始める距離、y = 覆いきる距離
    fog_range: vec4<f32>,
    camera_pos: vec4<f32>,
    // x = IBL strength, y = exposure (1), z = elapsed seconds (water scroll), w = ACES on if >0.5.
    // Thin V2 cam.env port. Procedural SH, no cubemap bind (WebGL2).
    env: vec4<f32>,
    // slot 0..3 の局所光。xyz + w=intensity / rgb + w=radius / xyz dir + w=spot。
    // 強度 0 は未使用（スロット漏れなし）。
    light_pos: array<vec4<f32>, 4>,
    light_col: array<vec4<f32>, 4>,
    light_dir: array<vec4<f32>, 4>,
    // Cascade 0. Count 1 is this matrix alone, over the full 2048 map.
    light_view_proj: mat4x4<f32>,
    // Cascades 1 and 2. Packed into tiles of the same map when cascade.x > 1.5.
    cascade1: mat4x4<f32>,
    cascade2: mat4x4<f32>,
    // x = shadow radius. 1 = the historical 3x3 PCF. Larger = a wider tent.
    // y > 0.5: this pass writes the unorm swapchain. present_rgb applies
    // exposure, ACES, and one sRGB OETF. A separate Rgba8Unorm frame (y = 0)
    // is the WebGPU path when SSAO is on: composite grades that frame once.
    // Sampling Rgba16Float stays off. z = camera near, w = camera far.
    // Shading does not read z or w.
    shadow: vec4<f32>,
    // x = cascade count (1, 2, or 3). y > 0.5 enables the PCSS blocker search.
    // The direct unorm path forces y to 0 so it never samples the Rgba16Float
    // raw target. z > 1 stores eye/z in 0..1 for SSAO on an 8-bit depth color.
    // z <= 1 stores linear eye meters. w is unused.
    cascade: vec4<f32>,
    // Hemisphere colors. Defaults match the old hardcoded studio SH.
    hemi_sky: vec4<f32>,
    // rgb = ground. w > 0.5 samples the prefiltered env strip (group 3).
    hemi_ground: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var albedo_tex: texture_2d<f32>;
@group(1) @binding(1) var albedo_samp: sampler;
@group(1) @binding(2) var matcap_tex: texture_2d<f32>;
@group(1) @binding(3) var matcap_samp: sampler;
@group(1) @binding(4) var normal_tex: texture_2d<f32>;
@group(1) @binding(5) var normal_samp: sampler;
// ORM is linear: G = roughness, B = metallic. Emissive map is a color.
// Defaults are unused until the instance flags in mtoon2 say to sample.
@group(1) @binding(6) var orm_tex: texture_2d<f32>;
@group(1) @binding(7) var orm_samp: sampler;
@group(1) @binding(8) var emissive_tex: texture_2d<f32>;
@group(1) @binding(9) var emissive_samp: sampler;
@group(2) @binding(0) var shadow_tex: texture_depth_2d;
@group(2) @binding(1) var shadow_samp: sampler_comparison;
// Device z of the sun map, for the PCSS blocker search. Unused while cascade.y is off.
@group(2) @binding(2) var shadow_raw: texture_2d<f32>;
@group(2) @binding(3) var shadow_raw_samp: sampler;
// Prefiltered lat-long, 4 roughness rows. Sampled only when hemi_ground.w > 0.5.
@group(3) @binding(0) var env_tex: texture_2d<f32>;
@group(3) @binding(1) var env_samp: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) m0: vec4<f32>,
    @location(3) m1: vec4<f32>,
    @location(4) m2: vec4<f32>,
    @location(5) m3: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) material: f32,
    @location(8) uv: vec2<f32>,
    @location(9) mtoon: vec4<f32>,
    @location(10) mtoon2: vec4<f32>,
    @location(11) mtoon3: vec4<f32>,
    @location(12) mtoon4: vec4<f32>,
    // 13 and 14 are vertex streams. 15 is the alpha sentinel.
    // Together with 0..12 this is 16 attributes, the WebGL2 minimum.
    @location(13) vcolor: vec4<f32>,
    @location(14) tangent: vec4<f32>,
    @location(15) alpha_cut: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world: vec3<f32>,
    @location(3) material: f32,
    @location(4) uv: vec2<f32>,
    @location(5) mtoon: vec4<f32>,
    @location(6) mtoon2: vec4<f32>,
    @location(7) mtoon3: vec4<f32>,
    @location(8) mtoon4: vec4<f32>,
    @location(9) vcolor: vec4<f32>,
    @location(10) tangent: vec4<f32>,
    @location(11) alpha_cut: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let world = model * vec4<f32>(in.pos, 1.0);

    var out: VsOut;
    out.clip = g.view_proj * world;
    // Glow quads (trail ribbon, grid, rails) sit on or just above the floor.
    // They depth-write with alpha blending, so a coplanar compare flickers
    // into black/white bands. Pull them toward the camera in clip space.
    // DirectX/WebGPU z is 0 at the near plane.
    if (in.mtoon4.z > 0.30) {
        out.clip.z = max(out.clip.z - 0.0016 * out.clip.w, 0.0);
    }
    out.color = in.color;
    out.normal = mat3x3<f32>(in.m0.xyz, in.m1.xyz, in.m2.xyz) * in.normal;
    out.world = world.xyz;
    out.material = in.material;
    out.uv = in.uv;
    out.mtoon = in.mtoon;
    out.mtoon2 = in.mtoon2;
    out.mtoon3 = in.mtoon3;
    out.mtoon4 = in.mtoon4;
    out.vcolor = in.vcolor;
    let axes = mat3x3<f32>(in.m0.xyz, in.m1.xyz, in.m2.xyz);
    let flip = select(1.0, -1.0, determinant(axes) < 0.0);
    out.tangent = vec4<f32>(axes * in.tangent.xyz, in.tangent.w * flip);
    out.alpha_cut = in.alpha_cut;
    return out;
}

// アウトライン（MToon backface push-out）。法線方向へ width 押し出し、
// 単色で塗る。カリングは Front（裏面だけ見える）。
@vertex
fn vs_outline(in: VsIn) -> VsOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let world_pos = model * vec4<f32>(in.pos, 1.0);
    let world_n = normalize(mat3x3<f32>(in.m0.xyz, in.m1.xyz, in.m2.xyz) * in.normal);
    let world = world_pos + vec4<f32>(world_n * max(in.mtoon3.w, 0.0), 0.0);

    var out: VsOut;
    out.clip = g.view_proj * world;
    out.color = vec4<f32>(in.mtoon3.rgb, 1.0);
    out.normal = world_n;
    out.world = world.xyz;
    out.material = in.material;
    out.uv = in.uv;
    out.mtoon = in.mtoon;
    out.mtoon2 = in.mtoon2;
    out.mtoon3 = in.mtoon3;
    out.mtoon4 = in.mtoon4;
    out.vcolor = in.vcolor;
    out.tangent = in.tangent;
    out.alpha_cut = in.alpha_cut;
    return out;
}

// Narkowicz ACES, same constants as bloom.wgsl. Only when this pass writes
// the swapchain (g.shadow.y > 0.5). Composite owns the grade otherwise.
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    return saturate((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let cut = c <= vec3<f32>(0.0031308);
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, cut);
}

fn present_rgb(rgb: vec3<f32>) -> vec3<f32> {
    if (g.shadow.y <= 0.5) {
        return rgb;
    }
    var c = rgb * max(g.env.y, 0.0);
    if (g.env.w > 0.5) {
        c = aces_tonemap(c);
    }
    return linear_to_srgb(max(c, vec3<f32>(0.0)));
}

// location 1 is the emissive mask for 8-bit bloom. HDR extract ignores it.
struct FragOut {
    @location(0) color: vec4<f32>,
    @location(1) emit: vec4<f32>,
}

fn frag(color: vec4<f32>, emit: f32) -> FragOut {
    var out: FragOut;
    out.color = color;
    out.emit = vec4<f32>(saturate(emit), 0.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_outline(in: VsOut) -> FragOut {
    return frag(vec4<f32>(present_rgb(in.color.rgb), 1.0), 0.0);
}

// Linear eye depth in meters for SSAO / DOF / motion. The color target stays
// ALPHA_BLENDING, so device depth cannot ride in the color alpha. Sky is the
// far plane. Cutout holes discard after the albedo sample (WebGPU).
@fragment
fn fs_depth(in: VsOut) -> @location(0) vec4<f32> {
    let tex = textureSample(albedo_tex, albedo_samp, in.uv);
    let sky = abs(in.material - 3.0) < 0.5;
    if (!sky && tex.a < 0.5) {
        discard;
    }
    let near = max(g.shadow.z, 1e-3);
    let far = max(g.shadow.w, near + 0.01);
    let device_z = select(in.clip.z, 1.0, sky);
    let eye = (near * far) / max(far - device_z * (far - near), 1e-4);
    // cascade.z is a uniform. span > 1 packs meters into 0..1 for an unorm
    // target. HDR leaves it at 0 and stores meters. Both select arms run;
    // the divisor stays at least 1.
    let span = g.cascade.z;
    let stored = select(eye, clamp(eye / max(span, 1.0), 0.0, 1.0), span > 1.0);
    return vec4<f32>(stored, stored, stored, 1.0);
}

fn shadow_clip(in: VsIn, vp: mat4x4<f32>) -> vec4<f32> {
    // Contact blobs (alpha) and emissive ribbons / rings / rails. A torus
    // or a 70 m rail in the shadow map is what streaks the track.
    if (in.color.a < 0.72 || in.mtoon4.z > 0.30) {
        return vec4<f32>(0.0, 0.0, -2.0, 1.0);
    }
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let world = model * vec4<f32>(in.pos, 1.0);
    return vp * world;
}

@vertex
fn vs_shadow(in: VsIn) -> @builtin(position) vec4<f32> {
    return shadow_clip(in, g.light_view_proj);
}

@vertex
fn vs_shadow_1(in: VsIn) -> @builtin(position) vec4<f32> {
    return shadow_clip(in, g.cascade1);
}

@vertex
fn vs_shadow_2(in: VsIn) -> @builtin(position) vec4<f32> {
    return shadow_clip(in, g.cascade2);
}

@fragment
fn fs_shadow_depth(@builtin(position) clip: vec4<f32>) -> @location(0) vec4<f32> {
    let d = clip.z;
    return vec4<f32>(d, d, d, 1.0);
}

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var x = p;
    for (var i = 0; i < 3; i++) {
        v += a * noise2(x);
        x *= 2.1;
        a *= 0.5;
    }
    return v;
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

fn light_one(n: vec3<f32>, world: vec3<f32>, i: i32) -> vec3<f32> {
    let pos_i = g.light_pos[i];
    if (pos_i.w <= 1e-5) {
        return vec3<f32>(0.0);
    }
    let col_r = g.light_col[i];
    let dir_s = g.light_dir[i];
    let to_l = pos_i.xyz - world;
    let dist = length(to_l);
    let ldir = to_l / max(dist, 1e-4);
    let rad = max(col_r.w, 0.5);
    let atten = pos_i.w / (1.0 + (dist * dist) / (rad * rad));
    var cone = 1.0;
    if (dir_s.w > 0.5) {
        let d = normalize(dir_s.xyz);
        cone = pow(max(dot(ldir, -d), 0.0), 6.0);
    }
    return col_r.rgb * max(dot(n, ldir), 0.0) * atten * cone;
}

fn local_lit(n: vec3<f32>, world: vec3<f32>, sh: f32) -> vec3<f32> {
    return light_one(n, world, 0) * sh
        + light_one(n, world, 1)
        + light_one(n, world, 2)
        + light_one(n, world, 3);
}

// Metal locals: GGX spec only. Lambert local_lit on metal reads as plastic.
fn light_one_metal(n: vec3<f32>, world: vec3<f32>, v: vec3<f32>, f0: vec3<f32>, rough: f32, i: i32) -> vec3<f32> {
    let pos_i = g.light_pos[i];
    if (pos_i.w <= 1e-5) {
        return vec3<f32>(0.0);
    }
    let col_r = g.light_col[i];
    let dir_s = g.light_dir[i];
    let to_l = pos_i.xyz - world;
    let dist = length(to_l);
    let ldir = to_l / max(dist, 1e-4);
    let rad = max(col_r.w, 0.5);
    let atten = pos_i.w / (1.0 + (dist * dist) / (rad * rad));
    var cone = 1.0;
    if (dir_s.w > 0.5) {
        let d = normalize(dir_s.xyz);
        cone = pow(max(dot(ldir, -d), 0.0), 6.0);
    }
    let h = normalize(v + ldir);
    let ndotl = max(dot(n, ldir), 0.0);
    let ndotv = max(dot(n, v), 0.0);
    let ndoth = max(dot(n, h), 0.0);
    let vdoth = max(dot(v, h), 0.0);
    let f = f0 + (1.0 - f0) * pow(1.0 - vdoth, 5.0);
    let spec = ggx_d(ndoth, rough) * smith_g(ndotv, ndotl, rough) * f
        / max(4.0 * ndotv * ndotl, 1e-4);
    return col_r.rgb * spec * ndotl * atten * cone;
}

fn local_metal(n: vec3<f32>, world: vec3<f32>, v: vec3<f32>, f0: vec3<f32>, rough: f32, sh: f32) -> vec3<f32> {
    return light_one_metal(n, world, v, f0, rough, 0) * sh
        + light_one_metal(n, world, v, f0, rough, 1)
        + light_one_metal(n, world, v, f0, rough, 2)
        + light_one_metal(n, world, v, f0, rough, 3);
}


// Tiny SH L1 / hemisphere. Colors come from the dump (the old studio pair when omitted).
// No 4K HDR, no cubemap, no storage buffer.
fn env_irradiance(n: vec3<f32>) -> vec3<f32> {
    if (g.env.x < 1e-4) {
        return vec3<f32>(0.0);
    }
    let sky = g.hemi_sky.rgb;
    let ground = g.hemi_ground.rgb;
    let sh0 = mix(ground, sky, 0.55);
    let shy = (sky - ground) * 0.5;
    let shx = vec3<f32>(0.06, 0.04, 0.02);
    return (sh0 + shy * n.y + shx * n.x) * g.env.x;
}

// Specular IBL. The sample always runs (textureSampleLevel, uniform flow).
// hemi_ground.w <= 0.5 mixes back to the SH, so an old dump is unchanged.
// The strip is 64×64: 4 roughness rows of a lat-long (PMREM-style, texture_2d).
fn env_specular(dir: vec3<f32>, rough: f32) -> vec3<f32> {
    let sh = env_irradiance(dir);
    let safe = select(vec3<f32>(0.0, 1.0, 0.0), dir, dot(dir, dir) > 1e-6);
    let n = normalize(safe);
    let u = atan2(n.z, n.x) * 0.15915494 + 0.5;
    let v = acos(clamp(n.y, -1.0, 1.0)) * 0.31830988;
    let level = clamp(rough, 0.0, 1.0) * 3.0;
    let row = min(floor(level), 3.0);
    let fy = level - row;
    // Stay inside the 16px row. v=1 is the last texel, not the next row.
    let local = (v * 15.0 + 0.5) / 64.0;
    let v0 = row * 0.25 + local;
    let v1 = min(row + 1.0, 3.0) * 0.25 + local;
    let a = textureSampleLevel(env_tex, env_samp, vec2<f32>(u, v0), 0.0).rgb;
    let b = textureSampleLevel(env_tex, env_samp, vec2<f32>(u, v1), 0.0).rgb;
    let sampled = mix(a, b, fy) * g.env.x;
    return mix(sh, sampled, step(0.5, g.hemi_ground.w));
}

// Directional sun. Depth compare (WebGL2 sampler2DShadow / WebGPU
// texture_depth_2d). The sampler is nearest: linear filtering of
// depth32float is the optional `depth32float-filterable` feature, which
// wgpu 30 cannot request. PCF uses `textureSampleCompareLevel` so the taps
// stay valid inside the loop.
// shadow.x <= 1 keeps the historical 3x3 kernel. Larger radii use a 5x5 tent.
// shadow.z <= 1.5 samples the full 2048 map. 2 or 3 cascades use tiles of it.
fn shadow_slope(nrm: vec3<f32>) -> f32 {
    let nlen = length(nrm);
    let nsafe = select(vec3<f32>(0.0, 1.0, 0.0), nrm / max(nlen, 1e-4), nlen > 1e-4);
    return clamp(1.0 - abs(dot(nsafe, normalize(g.light.xyz))), 0.0, 1.0);
}

fn shadow_receiver(world: vec3<f32>, nrm: vec3<f32>) -> vec3<f32> {
    let nlen = length(nrm);
    let nsafe = select(vec3<f32>(0.0, 1.0, 0.0), nrm / max(nlen, 1e-4), nlen > 1e-4);
    let slope = shadow_slope(nrm);
    return world + nsafe * (0.04 + slope * 0.07);
}

fn shadow_bias(depth: f32, nrm: vec3<f32>) -> f32 {
    // depth32float polygon offset barely moves the map. The receiver is also
    // pushed along its normal. This subtract stays small so a flat floor
    // keeps contact.
    let slope = shadow_slope(nrm);
    return depth - (0.0016 + slope * 0.0045);
}

fn pcf_lit(uv: vec2<f32>, d: f32) -> f32 {
    let texel = 1.0 / 2048.0;
    let radius = max(g.shadow.x, 0.0);
    var lit = 1.0;
    if (radius <= 1.01) {
        var s = 0.0;
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                s += textureSampleCompareLevel(
                    shadow_tex,
                    shadow_samp,
                    uv + vec2<f32>(f32(x), f32(y)) * texel,
                    d,
                );
            }
        }
        lit = mix(0.28, 1.0, s / 9.0);
    } else {
        var acc = 0.0;
        var wsum = 0.0;
        let step = texel * radius;
        for (var y = -2; y <= 2; y++) {
            for (var x = -2; x <= 2; x++) {
                let w = 1.0 / (1.0 + abs(f32(x)) + abs(f32(y)));
                acc += w * textureSampleCompareLevel(
                    shadow_tex,
                    shadow_samp,
                    uv + vec2<f32>(f32(x), f32(y)) * step,
                    d,
                );
                wsum += w;
            }
        }
        lit = mix(0.28, 1.0, acc / max(wsum, 1e-4));
    }
    // A compare outside the map (or a zero w) is NaN on some WebGPU devices.
    // NaN * 0 is still NaN, so an out-of-frustum mix would blank the fragment.
    return select(1.0, lit, lit == lit);
}

// Blocker search on the raw depth, then a 3x3 compare. shadow.w selects it.
fn pcss_lit(uv: vec2<f32>, d: f32) -> f32 {
    let texel = 1.0 / 2048.0;
    let search = texel * 8.0;
    var blockers = 0.0;
    var bsum = 0.0;
    for (var i = 0u; i < 8u; i = i + 1u) {
        let a = f32(i) * 2.3999632;
        let o = vec2<f32>(cos(a), sin(a)) * search * mix(0.35, 1.0, f32(i) / 7.0);
        let z = textureSampleLevel(shadow_raw, shadow_raw_samp, uv + o, 0.0).r;
        let hit = select(0.0, 1.0, z < d);
        blockers = blockers + hit;
        bsum = bsum + z * hit;
    }
    let avg = bsum / max(blockers, 1.0);
    let pen = select(1.0, clamp((d - avg) * 80.0, 1.0, 8.0), blockers > 0.5);
    var s = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            s += textureSampleCompareLevel(
                shadow_tex,
                shadow_samp,
                uv + vec2<f32>(f32(x), f32(y)) * texel * pen,
                d,
            );
        }
    }
    let lit = mix(0.28, 1.0, s / 9.0);
    return select(1.0, lit, lit == lit);
}

fn shade_uv(uv: vec2<f32>, d: f32) -> f32 {
    if (g.cascade.y > 0.5) {
        return pcss_lit(uv, d);
    }
    return pcf_lit(uv, d);
}

fn project_shadow(vp: mat4x4<f32>, world: vec3<f32>) -> vec4<f32> {
    let sc = vp * vec4<f32>(world, 1.0);
    let w = select(sc.w, 1e-4, abs(sc.w) < 1e-4);
    let ndc = sc.xyz / w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, ndc.y * -0.5 + 0.5);
    let inside = select(
        0.0,
        1.0,
        (uv.x >= 0.0) & (uv.x <= 1.0) & (uv.y >= 0.0) & (uv.y <= 1.0) & (ndc.z >= 0.0) & (ndc.z <= 1.0),
    );
    return vec4<f32>(uv, ndc.z, inside);
}

fn tile_uv(uv: vec2<f32>, index: i32) -> vec2<f32> {
    let clamped = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
    let origin = vec2<f32>(f32(index % 2), f32(index / 2)) * 0.5;
    return origin + clamped * 0.5;
}

fn shadow_single(world: vec3<f32>, nrm: vec3<f32>) -> f32 {
    let p = project_shadow(g.light_view_proj, shadow_receiver(world, nrm));
    let lit = shade_uv(p.xy, shadow_bias(p.z, nrm));
    return mix(1.0, lit, p.w);
}

fn shadow_cascades(world: vec3<f32>, nrm: vec3<f32>) -> f32 {
    let origin = shadow_receiver(world, nrm);
    let a = project_shadow(g.light_view_proj, origin);
    let b = project_shadow(g.cascade1, origin);
    let c = project_shadow(g.cascade2, origin);
    let la = shade_uv(tile_uv(a.xy, 0), shadow_bias(a.z, nrm));
    let lb = shade_uv(tile_uv(b.xy, 1), shadow_bias(b.z, nrm));
    let lc = shade_uv(tile_uv(c.xy, 2), shadow_bias(c.z, nrm));
    let use2 = select(0.0, 1.0, g.cascade.x >= 2.5);
    let w0 = a.w;
    let w1 = b.w * (1.0 - w0);
    let w2 = c.w * use2 * (1.0 - max(w0, b.w));
    let lit = la * w0 + lb * w1 + lc * w2;
    let covered = max(w0, max(w1, w2));
    return mix(1.0, select(1.0, lit, covered > 0.0), covered);
}

fn shadow_factor(world: vec3<f32>, nrm: vec3<f32>) -> f32 {
    // Count is a uniform. One cascade keeps the full-map kernel.
    if (g.cascade.x <= 1.5) {
        return shadow_single(world, nrm);
    }
    return shadow_cascades(world, nrm);
}

// mtoon2.x/y/z = sample roughness / metal / emissive map. mtoon4.z = emissive strength.
// mtoon3.rgb = emissive color (0 = use albedo). Non-toon only; toon keeps those slots.
fn pbr_metal_rough(in: VsOut, orm: vec4<f32>, metallic_in: f32, rough_in: f32) -> vec2<f32> {
    let use_r = step(0.5, in.mtoon2.x);
    let use_m = step(0.5, in.mtoon2.y);
    let metallic = mix(metallic_in, clamp(orm.b, 0.0, 1.0), use_m);
    let rough = mix(rough_in, clamp(orm.g, 0.04, 1.0), use_r);
    return vec2<f32>(metallic, rough);
}

// glTF normal maps are OpenGL Y-up (do not flip green). A zero tangent keeps
// the geometric normal, with the old half-blend as a last resort.
fn normal_from_map(geo: vec3<f32>, tangent: vec4<f32>, nmap: vec3<f32>) -> vec3<f32> {
    let mapped = nmap * 2.0 - 1.0;
    if (length(tangent.xyz) < 0.1) {
        return normalize(geo + mapped * 0.5);
    }
    let t = normalize(tangent.xyz);
    let b = normalize(cross(geo, t)) * tangent.w;
    return normalize(mat3x3<f32>(t, b, geo) * mapped);
}

fn emissive_rgb(in: VsOut, albedo: vec3<f32>, em_map: vec3<f32>) -> vec3<f32> {
    let strength = max(in.mtoon4.z, 0.0);
    var col = in.mtoon3.rgb;
    let use_alb = step(dot(col, col), 1e-8);
    col = mix(col, albedo, use_alb);
    let use_map = step(0.5, in.mtoon2.z);
    col = mix(col, col * em_map, use_map);
    return col * strength;
}

// Two scrolling procedural normals (fbm gradients). env.z = seconds.
// No normal-map texture, no SSR, no storage buffer.
fn water_normal(world: vec3<f32>, geo_n: vec3<f32>) -> vec3<f32> {
    let t = g.env.z;
    let xz = world.xz;
    let uv1 = xz * 0.18 + vec2<f32>(t * 0.06, t * 0.025);
    let uv2 = xz * 0.47 + vec2<f32>(-t * 0.04, t * 0.055);
    let e = 0.10;
    let n1x = fbm(uv1 + vec2<f32>(e, 0.0)) - fbm(uv1 - vec2<f32>(e, 0.0));
    let n1z = fbm(uv1 + vec2<f32>(0.0, e)) - fbm(uv1 - vec2<f32>(0.0, e));
    let n2x = fbm(uv2 + vec2<f32>(e, 0.0)) - fbm(uv2 - vec2<f32>(e, 0.0));
    let n2z = fbm(uv2 + vec2<f32>(0.0, e)) - fbm(uv2 - vec2<f32>(0.0, e));
    let bump = vec3<f32>((n1x + n2x) * 1.6, 0.0, (n1z + n2z) * 1.6);
    return normalize(normalize(geo_n) + bump);
}

@fragment
fn fs_main(in: VsOut) -> FragOut {
    let mat_id = i32(in.material + 0.5);
    var alpha = in.color.a * in.vcolor.a;

    // WebGPU rejects textureSample after a non-uniform return or inside a
    // fragment `if` (derivatives must be uniform). Sample every map first,
    // then branch. Sky pixels pay for the samples and ignore them.
    let tex = textureSample(albedo_tex, albedo_samp, in.uv);
    let orm = textureSample(orm_tex, orm_samp, in.uv);
    let em_map = textureSample(emissive_tex, emissive_samp, in.uv).rgb;
    let nmap = textureSample(normal_tex, normal_samp, in.uv).rgb;
    let n_geo = normalize(in.normal);
    let n_mapped = normal_from_map(n_geo, in.tangent, nmap);
    let n = select(n_geo, n_mapped, in.mtoon4.w > 0.5);
    let n_toon = n;
    let cap_view = normalize(g.camera_pos.xyz - in.world);
    let cap_uv = reflect(-cap_view, n_toon).xy * 0.5 + vec2<f32>(0.5, 0.5);
    let matcap = textureSample(matcap_tex, matcap_samp, cap_uv).rgb;
    // Called before the sky return so the compare stays uniform on WebGPU.
    let sh = shadow_factor(in.world, in.normal);

    // スカイ: in.color.rgb を基準に天頂〜地平〜足元のスタジオグラデーション。
    // Linear HDR; exposure + ACES are applied by the bloom composite pass.
    if (mat_id == 3) {
        let dir = normalize(in.world - g.camera_pos.xyz);
        let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
        let base = in.color.rgb;
        let day_mix = clamp(dot(base, vec3<f32>(0.333)), 0.0, 1.0) * 0.65;
        let day_horizon = mix(base * 1.45, vec3<f32>(0.78, 0.86, 0.96), day_mix);
        let day_zenith = base * vec3<f32>(0.68, 0.80, 1.05);
        let day_col = mix(day_horizon, day_zenith, smoothstep(0.18, 0.82, t));
        // Dark skies (Lumen). A saturated magenta at dir.y == 0 sat on the
        // phone's 8-bit present as a pink line across the middle of the frame.
        // Keep a faint warm lift and fade it out well above the horizon.
        let night = 1.0 - smoothstep(0.05, 0.22, day_mix);
        let night_col = mix(
            vec3<f32>(0.10, 0.035, 0.08),
            vec3<f32>(0.012, 0.016, 0.05),
            smoothstep(-0.20, 0.55, dir.y),
        );
        let col = mix(day_col, night_col, night);
        return frag(vec4<f32>(present_rgb(col), 1.0), 0.0);
    }

    // 0 = legacy cutout. >0 = MASK. -1 = OPAQUE. <= -1.5 = BLEND (no discard).
    let cut = in.alpha_cut;
    if (cut > 0.0 && tex.a < cut) {
        discard;
    } else if (cut == 0.0 && tex.a < 0.5) {
        discard;
    }
    var albedo = in.color.rgb * tex.rgb * in.vcolor.rgb;
    if (mat_id == 1) {
        // アスファルト: 細かいノイズ + 薄い轍。
        let gn = fbm(in.world.xz * 0.35);
        let grooves = smoothstep(0.45, 0.55, abs(fract(in.world.x * 0.12) - 0.5));
        albedo *= (0.78 + 0.28 * gn) * (1.0 - 0.08 * grooves);
    } else if (mat_id == 2) {
        // 草地: まだらな緑。高さで磯／岩に混ぜ、平面のハゲ緑にしない。
        let gn = fbm(in.world.xz * 0.18);
        albedo *= (0.75 + 0.4 * gn);
        albedo = mix(albedo, albedo * vec3<f32>(0.85, 1.05, 0.8), gn);
        let hy = in.world.y;
        if (hy < 0.12) {
            let shore = vec3<f32>(0.42, 0.40, 0.30);
            albedo = mix(shore, albedo, clamp((hy + 0.25) / 0.40, 0.0, 1.0));
        } else if (hy > 2.2) {
            let rock = vec3<f32>(0.40, 0.36, 0.32);
            albedo = mix(albedo, rock, clamp((hy - 2.2) / 6.0, 0.0, 0.85));
        }
    }

    let ldir = normalize(g.light.xyz);
    let ndl = max(dot(n, ldir), 0.0);
    let ambient = g.light.w;
    var lit: vec3<f32>;
    if (mat_id == 5) {
        // MToon: half-Lambert + shadeShift + toony（影 2 段階）、
        // リム（色 + fresnel power + lift）、matcap（反射 UV）、normal マップ。
        let shift = in.mtoon4.x;
        let n2 = n_toon;
        let half_l = (dot(n2, ldir) * sh) * 0.5 + 0.5;
        let toony = in.mtoon.a;
        let shade_t =
            clamp((half_l - 0.5) / max(1e-3, 1.0 - toony) + 0.5 + shift, 0.0, 1.0);
        let shade_rgb = in.mtoon.rgb * albedo;
        lit = mix(shade_rgb, albedo, shade_t);
        let view_dir = normalize(g.camera_pos.xyz - in.world);
        let fresnel = pow(
            max(1.0 - clamp(dot(n2, view_dir), 0.0, 1.0), 0.0),
            in.mtoon2.w,
        );
        lit += in.mtoon2.rgb * (fresnel + in.mtoon4.y);
        lit += matcap * step(0.5, in.mtoon4.z);
        // V2 toon IBL is irr * albedo * 0.35. Keep face from white-masking.
        lit += albedo * env_irradiance(n2) * 0.35;
    } else if (mat_id == 4) {
        // 金属コイン: 既存 GGX（RendererV2 と同じ式）。第二レンダラではない。
        let v = normalize(g.camera_pos.xyz - in.world);
        let h = normalize(v + ldir);
        let ndotl = ndl;
        let ndotv = max(dot(n, v), 0.0);
        let ndoth = max(dot(n, h), 0.0);
        let vdoth = max(dot(v, h), 0.0);
        let metallic_in = select(clamp(in.mtoon4.x, 0.0, 1.0), 1.0, in.mtoon4.x <= 1e-4 && in.mtoon4.y <= 1e-4);
        let rough_in = select(clamp(in.mtoon4.y, 0.04, 1.0), 0.12, in.mtoon4.y <= 1e-4);
        let mr = pbr_metal_rough(in, orm, metallic_in, rough_in);
        let metallic = mr.x;
        let rough = mr.y;
        let f0 = mix(vec3<f32>(0.04), albedo, metallic);
        let f = f0 + (1.0 - f0) * pow(1.0 - vdoth, 5.0);
        let spec = ggx_d(ndoth, rough) * smith_g(ndotv, ndotl, rough) * f
            / max(4.0 * ndotv * ndotl, 1e-4);
        let kd = (vec3<f32>(1.0) - f) * (1.0 - metallic);
        let fresnel_edge = pow(1.0 - ndotv, 4.0);
        lit = (kd * albedo + spec * 1.45) * ndotl * sh + albedo * ambient * 0.28;
        lit += kd * albedo * local_lit(n, in.world, sh);
        lit += local_metal(n, in.world, v, f0, rough, sh) * 1.35;
        let r = reflect(-v, n);
        let studio_spec = pow(max(r.y * 0.65 + 0.35, 0.0), 6.0) * (1.0 - rough);
        lit += f0 * env_specular(r, rough) * (1.2 - rough * 0.5);
        lit += mix(f0, vec3<f32>(1.0, 0.98, 0.94), 0.55) * (studio_spec * 0.48 + fresnel_edge * 0.36);
    } else if (mat_id == 6) {
        // Water: two scrolling normals + Fresnel + SH reflection.
        // Not a Water Renderer / V2. No SSR, no physics sim, no caustics.
        let nw = water_normal(in.world, n);
        let v = normalize(g.camera_pos.xyz - in.world);
        let ndotv = max(dot(nw, v), 0.0);
        let fresnel = pow(1.0 - ndotv, 5.0);
        let deep = albedo * vec3<f32>(0.45, 0.70, 0.82);
        let body = mix(deep, albedo, ndotv * 0.65);
        let r = reflect(-v, nw);
        let spec_sun = pow(max(dot(nw, normalize(v + ldir)), 0.0), 96.0);
        lit = body * (ambient * 0.85 + (1.0 - ambient) * max(dot(nw, ldir), 0.0) * 0.35 * sh);
        lit += body * local_lit(nw, in.world, sh) * 0.25;
        lit += env_specular(r, 0.08) * mix(0.22, 1.05, fresnel);
        lit += env_irradiance(nw) * body * 0.20;
        lit += vec3<f32>(0.95, 0.97, 1.0) * spec_sun * 0.55 * sh;
        // Cheap depth fade: looking down is more transmissive (no scene-depth prepass).
        alpha = mix(0.92, 0.34, ndotv) * in.color.a;
    } else {
        // Radial contact blob: soft round AO under feet/pieces instead of a hard square.
        if (alpha < 0.72 && mat_id == 0) {
            let r2 = dot(in.uv - vec2<f32>(0.5, 0.5), in.uv - vec2<f32>(0.5, 0.5)) * 4.0;
            if (r2 >= 1.0) {
                discard;
            }
            let soft = (1.0 - r2) * (1.0 - r2);
            let ao = clamp(alpha * soft * 1.75, 0.0, 0.85);
            if (ao < 0.03) {
                discard;
            }
            return frag(vec4<f32>(present_rgb(vec3<f32>(0.008, 0.010, 0.018)), ao), 0.0);
        }
        let rough_in = select(clamp(in.mtoon4.y, 0.04, 1.0), 0.42, in.mtoon4.y <= 1e-4);
        let metallic_in = clamp(in.mtoon4.x, 0.0, 1.0);
        let mr = pbr_metal_rough(in, orm, metallic_in, rough_in);
        let rough = mr.y;
        let metallic = mr.x;
        let v = normalize(g.camera_pos.xyz - in.world);
        let ndotv = max(dot(n, v), 0.0);
        let f0 = mix(vec3<f32>(0.04), albedo, metallic);
        let fresnel = f0 + (1.0 - f0) * pow(1.0 - ndotv, 5.0);
        let gloss = max(0.0, 1.0 - rough);
        let h = normalize(v + ldir);
        let ndoth = max(dot(n, h), 0.0);
        let sun_spec = ggx_d(ndoth, rough) * smith_g(ndotv, ndl, rough) * fresnel
            / max(4.0 * ndotv * max(ndl, 1e-3), 1e-4);
        lit = albedo * (ambient + (1.0 - ambient) * ndl * sh);
        lit += albedo * local_lit(n, in.world, sh);
        lit += albedo * env_irradiance(n);
        let r = reflect(-v, n);
        let env_spec = env_specular(r, rough);
        if (gloss > 0.08) {
            let studio_spec = pow(max(r.y * 0.65 + 0.35, 0.0), 8.0) * gloss;
            lit += sun_spec * ndl * sh * gloss * 1.4;
            lit += local_metal(n, in.world, v, f0, rough, sh) * gloss * 1.3;
            lit += env_spec * fresnel * gloss * gloss * 1.5;
            lit += vec3<f32>(1.0, 0.98, 0.95) * (studio_spec * 0.22 + pow(1.0 - ndotv, 4.0) * gloss * 0.18);
        }
        // Studio tabletop / cyclorama radial gradient (active on studio floor y < -0.12 when ibl < 0.30)
        if (in.world.y < -0.12 && g.env.x < 0.30) {
            let r_dist = length(in.world.xz);
            let spot = smoothstep(8.2, 1.8, r_dist);
            let outer = smoothstep(10.5, 3.2, r_dist);
            lit = lit * mix(0.08, 1.15, outer) + vec3<f32>(0.09, 0.13, 0.22) * spot * spot;
        }
    }

    var emit_strength = 0.0;
    if (mat_id != 5) {
        let em = emissive_rgb(in, albedo, em_map);
        lit += em;
        emit_strength = saturate(max(em.r, max(em.g, em.b)));
    }

    let dist = length(in.world - g.camera_pos.xyz);
    let span = max(g.fog_range.y - g.fog_range.x, 1e-3);
    let fog = clamp((dist - g.fog_range.x) / span, 0.0, 1.0);

    // Linear HDR. exposure + ACES are applied by the bloom composite pass,
    // which also adds the bloom spill before tonemapping.
    let rgb = mix(lit, g.fog_color.rgb, fog);
    if (cut <= -1.5 && mat_id != 6) {
        alpha = in.color.a * tex.a * in.vcolor.a;
    }
    return frag(vec4<f32>(present_rgb(rgb), alpha), emit_strength * (1.0 - fog));
}
