//! Display-space color grade baked into a 16³ strip LUT.
//!
//! The strip is a 256×16 RGBA8 image (16 slices of 16×16). WebGL2 samples it
//! as `texture_2d` — no `texture_3d`, no storage buffer. Presets are tiny on
//! purpose: a phone can upload one of them and mix it off.

pub const LUT_SIZE: u32 = 16;

/// `off` / unknown presets are identity. `warm`, `punch`, and `cool` are the
/// mobile looks Box Hop turns on.
pub fn bake_lut_strip(preset: &str) -> Vec<u8> {
    let n = LUT_SIZE;
    let width = n * n;
    let mut rgba = vec![0u8; (width * n * 4) as usize];
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let rf = r as f32 / (n - 1) as f32;
                let gf = g as f32 / (n - 1) as f32;
                let bf = b as f32 / (n - 1) as f32;
                let (ro, go, bo) = grade_rgb(preset, rf, gf, bf);
                let x = b * n + r;
                let y = g;
                let i = ((y * width + x) * 4) as usize;
                rgba[i] = to_u8(ro);
                rgba[i + 1] = to_u8(go);
                rgba[i + 2] = to_u8(bo);
                rgba[i + 3] = 255;
            }
        }
    }
    rgba
}

pub fn grade_rgb(preset: &str, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    match preset {
        "warm" => {
            let (r, g, b) = contrast(r, g, b, 1.06);
            let (r, g, b) = saturate(r, g, b, 1.12);
            (
                (r * 1.08 + 0.015).clamp(0.0, 1.0),
                (g * 1.01).clamp(0.0, 1.0),
                (b * 0.88).clamp(0.0, 1.0),
            )
        }
        "punch" => {
            let (r, g, b) = contrast(r, g, b, 1.18);
            saturate(r, g, b, 1.28)
        }
        "cool" => {
            let (r, g, b) = contrast(r, g, b, 1.05);
            (
                (r * 0.90).clamp(0.0, 1.0),
                (g * 0.98).clamp(0.0, 1.0),
                (b * 1.08 + 0.02).clamp(0.0, 1.0),
            )
        }
        _ => (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)),
    }
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn contrast(r: f32, g: f32, b: f32, c: f32) -> (f32, f32, f32) {
    (
        ((r - 0.5) * c + 0.5).clamp(0.0, 1.0),
        ((g - 0.5) * c + 0.5).clamp(0.0, 1.0),
        ((b - 0.5) * c + 0.5).clamp(0.0, 1.0),
    )
}

fn saturate(r: f32, g: f32, b: f32, s: f32) -> (f32, f32, f32) {
    let l = r * 0.2126 + g * 0.7152 + b * 0.0722;
    (
        (l + (r - l) * s).clamp(0.0, 1.0),
        (l + (g - l) * s).clamp(0.0, 1.0),
        (l + (b - l) * s).clamp(0.0, 1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(preset: &str, r: u32, g: u32, b: u32) -> [u8; 3] {
        let rgba = bake_lut_strip(preset);
        let n = LUT_SIZE;
        let x = b * n + r;
        let y = g;
        let i = ((y * n * n + x) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2]]
    }

    #[test]
    fn identity_lut_matches_the_cell() {
        // 8/15 * 255 = 136 exactly.
        let c = cell("off", 8, 8, 8);
        assert_eq!(c, [136, 136, 136]);
        assert_eq!(cell("identity", 0, 0, 15), [0, 0, 255]);
        assert_eq!(bake_lut_strip("warm").len(), (256 * 16 * 4) as usize);
    }

    #[test]
    fn warm_lifts_red_and_drops_blue() {
        let c = cell("warm", 8, 8, 8);
        assert!(
            c[0] > c[2],
            "warm gray should be redder than blue, got {c:?}"
        );
        assert!(c[0] > 136, "warm should lift a mid gray, got {c:?}");
    }

    #[test]
    fn punch_increases_saturation_of_a_green() {
        let ident = cell("off", 4, 12, 4);
        let punch = cell("punch", 4, 12, 4);
        let spread = |c: [u8; 3]| c[1] as i32 - c[0] as i32;
        assert!(
            spread(punch) > spread(ident),
            "punch should push green away from red: {ident:?} -> {punch:?}"
        );
    }

    #[test]
    fn cool_drops_red_on_gray() {
        let c = cell("cool", 8, 8, 8);
        assert!(c[2] > c[0], "cool gray should be bluer than red, got {c:?}");
    }
}
