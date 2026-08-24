//! Equirect → cube. GPU 不要な変換。拡散は小さな irradiance キューブ。

const PI: f32 = std::f32::consts::PI;

pub fn face_dir(face: usize, u: f32, v: f32) -> [f32; 3] {
    match face {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        _ => [-u, -v, -1.0],
    }
}

pub fn dir_to_equirect_uv(dx: f32, dy: f32, dz: f32) -> (f32, f32) {
    let len = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-8);
    let dx = dx / len;
    let dy = dy / len;
    let dz = dz / len;
    let mut u = 0.5 + dx.atan2(-dz) / (2.0 * PI);
    if u < 0.0 {
        u += 1.0;
    }
    if u >= 1.0 {
        u -= 1.0;
    }
    let v = (0.5 - dy.clamp(-1.0, 1.0).asin() / PI).clamp(0.0, 1.0);
    (u, v)
}

pub fn studio_equirect_rgba(width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let mut pix = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let t = y as f32 / (height - 1).max(1) as f32;
        let s = 1.0 / (1.0 + ((t - 0.55) * 12.0).exp());
        for x in 0..width {
            let az = x as f32 / (width - 1).max(1) as f32;
            let sky = [0.35 + 0.25 * az, 0.45 + 0.15 * az, 0.70];
            let ground = [0.55, 0.38, 0.22];
            let r = sky[0] * s + ground[0] * (1.0 - s);
            let g = sky[1] * s + ground[1] * (1.0 - s);
            let b = sky[2] * s + ground[2] * (1.0 - s);
            pix.extend_from_slice(&[
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8,
                255,
            ]);
        }
    }
    (pix, width, height)
}

fn sample_rgba(pix: &[u8], w: u32, h: u32, u: f32, v: f32) -> [u8; 4] {
    let x = ((u.fract() + 1.0).fract() * w as f32) as u32 % w;
    let y = (v.clamp(0.0, 0.999999) * h as f32) as u32;
    let i = ((y * w + x) * 4) as usize;
    [pix[i], pix[i + 1], pix[i + 2], pix[i + 3]]
}

pub fn equirect_to_cube_rgba(pix: &[u8], w: u32, h: u32, face_size: u32) -> Vec<[u8; 4]> {
    let mut out = Vec::with_capacity((face_size * face_size * 6) as usize);
    for face in 0..6 {
        for y in 0..face_size {
            for x in 0..face_size {
                let u = 2.0 * (x as f32 + 0.5) / face_size as f32 - 1.0;
                let v = 2.0 * (y as f32 + 0.5) / face_size as f32 - 1.0;
                let d = face_dir(face, u, v);
                let (eu, ev) = dir_to_equirect_uv(d[0], d[1], d[2]);
                out.push(sample_rgba(pix, w, h, eu, ev));
            }
        }
    }
    out
}

pub fn pbr_enabled(metallic: f32, roughness: f32) -> bool {
    metallic > 0.001 || roughness < 0.999
}

fn norm3(x: f32, y: f32, z: f32) -> [f32; 3] {
    let len = (x * x + y * y + z * z).sqrt().max(1e-8);
    [x / len, y / len, z / len]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn onb(n: [f32; 3]) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let t = if n[1].abs() < 0.999 {
        let c = cross([0.0, 1.0, 0.0], n);
        norm3(c[0], c[1], c[2])
    } else {
        let c = cross([1.0, 0.0, 0.0], n);
        norm3(c[0], c[1], c[2])
    };
    (t, cross(n, t), n)
}

fn radical_inverse_vdc(mut bits: u32) -> f32 {
    let mut inv = 0.0f32;
    let mut base = 0.5f32;
    while bits > 0 {
        if bits & 1 == 1 {
            inv += base;
        }
        bits >>= 1;
        base *= 0.5;
    }
    inv
}

fn cosine_hemisphere(n: [f32; 3], index: u32, samples: u32) -> [f32; 3] {
    let samples = samples.max(1);
    let u = (index as f32 + 0.5) / samples as f32;
    let v = radical_inverse_vdc(index);
    let r = u.max(0.0).sqrt();
    let phi = 2.0 * PI * v;
    let lx = r * phi.cos();
    let ly = r * phi.sin();
    let lz = (1.0 - u).max(0.0).sqrt();
    let (t, b, nn) = onb(norm3(n[0], n[1], n[2]));
    [
        t[0] * lx + b[0] * ly + nn[0] * lz,
        t[1] * lx + b[1] * ly + nn[1] * lz,
        t[2] * lx + b[2] * ly + nn[2] * lz,
    ]
}

pub const IRRADIANCE_FACE_SIZE: u32 = 8;
pub const IRRADIANCE_SAMPLES: u32 = 16;
pub const SPECULAR_MIPS: u32 = 4;


pub fn downsample_cube_rgba(rgba: &[[u8; 4]], face_size: u32) -> Vec<[u8; 4]> {
    let face_size = face_size.max(2);
    let half = face_size / 2;
    let mut out = Vec::with_capacity((half * half * 6) as usize);
    let fsz = face_size as usize;
    let hs = half as usize;
    for face in 0..6 {
        let base = face * fsz * fsz;
        for y in 0..hs {
            for x in 0..hs {
                let mut acc = [0u32; 3];
                for oy in 0..2 {
                    for ox in 0..2 {
                        let px = rgba[base + (y * 2 + oy) * fsz + (x * 2 + ox)];
                        acc[0] += px[0] as u32;
                        acc[1] += px[1] as u32;
                        acc[2] += px[2] as u32;
                    }
                }
                out.push([
                    (acc[0] / 4) as u8,
                    (acc[1] / 4) as u8,
                    (acc[2] / 4) as u8,
                    255,
                ]);
            }
        }
    }
    out
}


pub fn cube_mip_chain(rgba: &[[u8; 4]], face_size: u32, mips: u32) -> Vec<(u32, Vec<[u8; 4]>)> {
    let mut size = face_size.max(1);
    let mut cur = rgba.to_vec();
    let mut out = vec![(size, cur.clone())];
    let n = mips.max(1).min(8);
    for _ in 1..n {
        if size < 2 {
            break;
        }
        cur = downsample_cube_rgba(&cur, size);
        size /= 2;
        out.push((size, cur.clone()));
    }
    out
}

pub fn irradiance_cube_rgba(
    pix: &[u8],
    w: u32,
    h: u32,
    face_size: u32,
    samples: u32,
) -> Vec<[u8; 4]> {
    let face_size = face_size.max(1);
    let samples = samples.max(1);
    let mut out = Vec::with_capacity((face_size * face_size * 6) as usize);
    for face in 0..6 {
        for y in 0..face_size {
            for x in 0..face_size {
                let u = 2.0 * (x as f32 + 0.5) / face_size as f32 - 1.0;
                let v = 2.0 * (y as f32 + 0.5) / face_size as f32 - 1.0;
                let d = face_dir(face, u, v);
                let n = norm3(d[0], d[1], d[2]);
                let mut acc = [0.0f32; 3];
                for s in 0..samples {
                    let dir = cosine_hemisphere(n, s, samples);
                    let (eu, ev) = dir_to_equirect_uv(dir[0], dir[1], dir[2]);
                    let rgba = sample_rgba(pix, w, h, eu, ev);
                    acc[0] += rgba[0] as f32;
                    acc[1] += rgba[1] as f32;
                    acc[2] += rgba[2] as f32;
                }
                let inv = 1.0 / samples as f32;
                out.push([
                    (acc[0] * inv).clamp(0.0, 255.0) as u8,
                    (acc[1] * inv).clamp(0.0, 255.0) as u8,
                    (acc[2] * inv).clamp(0.0, 255.0) as u8,
                    255,
                ]);
            }
        }
    }
    out
}

pub fn spot_cone_params(angle: f32, penumbra: f32) -> (f32, f32) {
    let angle = angle.clamp(0.02, PI - 0.02);
    let penumbra = penumbra.clamp(0.0, 0.99);
    let inner_angle = angle * (1.0 - penumbra);
    let cos_outer = angle.cos();
    let mut cos_inner = inner_angle.cos();
    if cos_inner <= cos_outer {
        cos_inner = (cos_outer + 1e-4).min(1.0);
    }
    (cos_outer, cos_inner)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < 1e-8 {
        return if x >= edge0 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn spot_cone_factor(from_light: [f32; 3], axis: [f32; 3], cos_outer: f32, cos_inner: f32) -> f32 {
    let a = norm3(from_light[0], from_light[1], from_light[2]);
    let b = norm3(axis[0], axis[1], axis[2]);
    let c = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    smoothstep(cos_outer, cos_inner, c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plus_y_is_equirect_top() {
        let (_u, v) = dir_to_equirect_uv(0.0, 1.0, 0.0);
        assert!(v < 0.05);
    }

    #[test]
    fn studio_has_pixels() {
        let (pix, w, h) = studio_equirect_rgba(8, 4);
        assert_eq!(pix.len(), (w * h * 4) as usize);
        let cube = equirect_to_cube_rgba(&pix, w, h, 2);
        assert_eq!(cube.len(), 2 * 2 * 6);
    }

    #[test]
    fn pbr_flag_defaults_off() {
        assert!(!pbr_enabled(0.0, 1.0));
        assert!(pbr_enabled(0.8, 1.0));
    }

    #[test]
    fn irradiance_constant_is_flat() {
        let pix = vec![80u8, 100, 120, 255].repeat(8 * 4);
        let cube = irradiance_cube_rgba(&pix, 8, 4, 2, 8);
        assert_eq!(cube.len(), 2 * 2 * 6);
        for px in &cube {
            assert!((px[0] as i32 - 80).abs() < 4);
            assert!((px[1] as i32 - 100).abs() < 4);
            assert!((px[2] as i32 - 120).abs() < 4);
        }
    }

    #[test]
    fn spot_on_axis_is_one() {
        let (outer, inner) = spot_cone_params(0.9, 0.3);
        let t = spot_cone_factor([0.0, -1.0, 0.0], [0.0, -1.0, 0.0], outer, inner);
        assert!((t - 1.0).abs() < 1e-5);
    }

    #[test]
    fn spot_sideways_is_zero() {
        let (outer, inner) = spot_cone_params(0.6, 0.2);
        let t = spot_cone_factor([1.0, 0.0, 0.0], [0.0, -1.0, 0.0], outer, inner);
        assert_eq!(t, 0.0);
    }

    #[test]
    fn cube_mips_halve() {
        let pix = vec![[10u8, 20, 30, 255]; 8 * 8 * 6];
        let chain = cube_mip_chain(&pix, 8, 3);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].0, 8);
        assert_eq!(chain[1].0, 4);
        assert_eq!(chain[2].0, 2);
        assert_eq!(chain[1].1.len(), 4 * 4 * 6);
    }
}
