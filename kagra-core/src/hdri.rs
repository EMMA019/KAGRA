//! Equirect → cube. GPU 不要な変換。PMREM はまだ無い。

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
}
