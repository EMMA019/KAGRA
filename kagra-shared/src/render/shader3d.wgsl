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
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var albedo_tex: texture_2d<f32>;
@group(1) @binding(1) var albedo_samp: sampler;

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
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world: vec3<f32>,
    @location(3) material: f32,
    @location(4) uv: vec2<f32>,
    @location(5) mtoon: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let world = model * vec4<f32>(in.pos, 1.0);

    var out: VsOut;
    out.clip = g.view_proj * world;
    out.color = in.color;
    out.normal = mat3x3<f32>(in.m0.xyz, in.m1.xyz, in.m2.xyz) * in.normal;
    out.world = world.xyz;
    out.material = in.material;
    out.uv = in.uv;
    out.mtoon = in.mtoon;
    return out;
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

fn local_lit(n: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    return light_one(n, world, 0)
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

fn local_metal(n: vec3<f32>, world: vec3<f32>, v: vec3<f32>, f0: vec3<f32>, rough: f32) -> vec3<f32> {
    return light_one_metal(n, world, v, f0, rough, 0)
        + light_one_metal(n, world, v, f0, rough, 1)
        + light_one_metal(n, world, v, f0, rough, 2)
        + light_one_metal(n, world, v, f0, rough, 3);
}


fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    // Narkowicz ACES, same as kagra-core V2. Swapchain is sRGB: no extra gamma.
    return saturate((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14));
}

// Tiny SH L1 / hemisphere. Studio zenith vs warm ground (V2 studio_equirect idea).
// No 4K HDR, no cubemap texture, no storage buffer.
fn env_irradiance(n: vec3<f32>) -> vec3<f32> {
    if (g.env.x < 1e-4) {
        return vec3<f32>(0.0);
    }
    let sky = vec3<f32>(0.48, 0.56, 0.78);
    let ground = vec3<f32>(0.42, 0.30, 0.18);
    let sh0 = mix(ground, sky, 0.55);
    let shy = (sky - ground) * 0.5;
    let shx = vec3<f32>(0.06, 0.04, 0.02);
    return (sh0 + shy * n.y + shx * n.x) * g.env.x;
}

fn tone_map(rgb: vec3<f32>) -> vec3<f32> {
    var c = rgb * max(g.env.y, 0.0);
    if (g.env.w > 0.5) {
        c = aces_tonemap(c);
    }
    return c;
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
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let mat_id = i32(in.material + 0.5);
    var alpha = in.color.a;

    // スカイ: 天頂〜地平のグラデーション。ライティングもフォグも掛けない。
    if (mat_id == 3) {
        let dir = normalize(in.world - g.camera_pos.xyz);
        let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
        let zenith = vec3<f32>(0.35, 0.55, 0.95);
        let horizon = vec3<f32>(0.78, 0.86, 0.95);
        let col = mix(horizon, zenith, t * t);
        return vec4<f32>(tone_map(col), 1.0);
    }

    var albedo = in.color.rgb * textureSample(albedo_tex, albedo_samp, in.uv).rgb;
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

    let n = normalize(in.normal);
    let ldir = normalize(g.light.xyz);
    let ndl = max(dot(n, ldir), 0.0);
    let ambient = g.light.w;
    var lit: vec3<f32>;
    if (mat_id == 5) {
        // Thin MToon: half-Lambert mix(shadeColor * albedo, albedo, t).
        // Hair rimLift / matcap / outline stay leftover V2.
        let half_l = dot(n, ldir) * 0.5 + 0.5;
        let toony = in.mtoon.a;
        let shade_t = clamp((half_l - 0.5) / max(1e-3, 1.0 - toony) + 0.5, 0.0, 1.0);
        let shade_rgb = in.mtoon.rgb * albedo;
        lit = mix(shade_rgb, albedo, shade_t);
        let view_dir = normalize(g.camera_pos.xyz - in.world);
        let fresnel = pow(max(1.0 - clamp(dot(n, view_dir), 0.0, 1.0), 0.0), 3.0);
        lit = lit + fresnel * 0.16 * vec3<f32>(1.0, 0.90, 0.78);
        // V2 toon IBL is irr * albedo * 0.35. Keep face from white-masking.
        lit += albedo * env_irradiance(n) * 0.35;
    } else if (mat_id == 4) {
        // 金属コイン: 既存 GGX（RendererV2 と同じ式）。第二レンダラではない。
        let v = normalize(g.camera_pos.xyz - in.world);
        let h = normalize(v + ldir);
        let ndotl = ndl;
        let ndotv = max(dot(n, v), 0.0);
        let ndoth = max(dot(n, h), 0.0);
        let vdoth = max(dot(v, h), 0.0);
        let metallic = 1.0;
        let rough = 0.12;
        let f0 = mix(vec3<f32>(0.04), albedo, metallic);
        let f = f0 + (1.0 - f0) * pow(1.0 - vdoth, 5.0);
        let spec = ggx_d(ndoth, rough) * smith_g(ndotv, ndotl, rough) * f
            / max(4.0 * ndotv * ndotl, 1e-4);
        let kd = (vec3<f32>(1.0) - f) * (1.0 - metallic);
        lit = (kd * albedo + spec) * ndotl + albedo * ambient * 0.22;
        lit += local_metal(n, in.world, v, f0, rough);
        // Metals have no Lambert; SH * f0 is the thin diffuse IBL so coins
        // pick up sky/ground in shadow. GGX locals stay.
        lit += f0 * env_irradiance(n);
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
        lit = body * (ambient * 0.85 + (1.0 - ambient) * max(dot(nw, ldir), 0.0) * 0.35);
        lit += body * local_lit(nw, in.world) * 0.25;
        lit += env_irradiance(r) * mix(0.22, 1.05, fresnel);
        lit += env_irradiance(nw) * body * 0.20;
        lit += vec3<f32>(0.95, 0.97, 1.0) * spec_sun * 0.55;
        // Cheap depth fade: looking down is more transmissive (no scene-depth prepass).
        alpha = mix(0.92, 0.34, ndotv) * in.color.a;
    } else {
        lit = albedo * (ambient + (1.0 - ambient) * ndl);
        lit += albedo * local_lit(n, in.world);
        lit += albedo * env_irradiance(n);
    }

    let dist = length(in.world - g.camera_pos.xyz);
    let span = max(g.fog_range.y - g.fog_range.x, 1e-3);
    let fog = clamp((dist - g.fog_range.x) / span, 0.0, 1.0);

    let rgb = mix(lit, g.fog_color.rgb, fog);
    return vec4<f32>(tone_map(rgb), alpha);
}
