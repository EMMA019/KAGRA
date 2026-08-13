// 3D: インスタンス化したメッシュ + 方向光 + 距離フォグ + 手続きテクスチャ。
//
// マテリアルはインスタンス属性の material（0=solid, 1=road, 2=grass, 3=sky）。
// テクスチャファイルは持たず、ワールド XZ のノイズで路面と草を出す。

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz = ライトへ向かう方向、w = 環境光の強さ
    light: vec4<f32>,
    // rgb = フォグ色（リニア）
    fog_color: vec4<f32>,
    // x = 効き始める距離、y = 覆いきる距離
    fog_range: vec4<f32>,
    camera_pos: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) m0: vec4<f32>,
    @location(3) m1: vec4<f32>,
    @location(4) m2: vec4<f32>,
    @location(5) m3: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) material: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world: vec3<f32>,
    @location(3) material: f32,
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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let mat_id = i32(in.material + 0.5);

    // スカイ: 天頂〜地平のグラデーション。ライティングもフォグも掛けない。
    if (mat_id == 3) {
        let dir = normalize(in.world - g.camera_pos.xyz);
        let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
        let zenith = vec3<f32>(0.35, 0.55, 0.95);
        let horizon = vec3<f32>(0.78, 0.86, 0.95);
        let col = mix(horizon, zenith, t * t);
        return vec4<f32>(col, 1.0);
    }

    var albedo = in.color.rgb;
    if (mat_id == 1) {
        // アスファルト: 細かいノイズ + 薄い轍。
        let n = fbm(in.world.xz * 0.35);
        let grooves = smoothstep(0.45, 0.55, abs(fract(in.world.x * 0.12) - 0.5));
        albedo *= (0.78 + 0.28 * n) * (1.0 - 0.08 * grooves);
    } else if (mat_id == 2) {
        // 草地: まだらな緑。
        let n = fbm(in.world.xz * 0.18);
        albedo *= (0.75 + 0.4 * n);
        albedo = mix(albedo, albedo * vec3<f32>(0.85, 1.05, 0.8), n);
    }

    let n = normalize(in.normal);
    let ndl = max(dot(n, normalize(g.light.xyz)), 0.0);
    let ambient = g.light.w;
    let lit = albedo * (ambient + (1.0 - ambient) * ndl);

    let dist = length(in.world - g.camera_pos.xyz);
    let span = max(g.fog_range.y - g.fog_range.x, 1e-3);
    let fog = clamp((dist - g.fog_range.x) / span, 0.0, 1.0);

    return vec4<f32>(mix(lit, g.fog_color.rgb, fog), in.color.a);
}
