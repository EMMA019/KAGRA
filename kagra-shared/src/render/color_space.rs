//! Author colors are sRGB. Lighting and the HDR frame are linear.
//!
//! Base color and emissive textures are `Rgba8UnormSrgb`, so WebGPU and WebGL2
//! both decode on sample. Normal and ORM stay `Rgba8Unorm` (data, not color).
//! Color mips are averaged in linear light and stored back as sRGB.
//!
//! Grade order, once, in `bloom.wgsl`: bloom extract on the HDR frame, then
//! exposure, ACES, the LUT, vignette, then one sRGB OETF. An sRGB target
//! encodes on store. An unorm target encodes in the shader. The direct unorm
//! fallback (the frame cannot be sampled) runs that same grade in `present_rgb`
//! so it does not skip ACES or encode twice.

/// IEC 61966-2-1. Same split as `linear_to_srgb` in the shaders.
pub(super) fn srgb_channel_to_linear(encoded: f32) -> f32 {
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

pub(super) fn linear_channel_to_srgb(linear: f32) -> f32 {
    let c = if linear.is_finite() {
        linear.max(0.0)
    } else {
        0.0
    };
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Prop / fog / clear bytes are sRGB. The HDR frame always wants linear.
pub(super) fn srgb_u8_to_linear(color: [u8; 4]) -> [f32; 4] {
    [
        srgb_channel_to_linear(color[0] as f32 / 255.0),
        srgb_channel_to_linear(color[1] as f32 / 255.0),
        srgb_channel_to_linear(color[2] as f32 / 255.0),
        color[3] as f32 / 255.0,
    ]
}

/// HUD bytes, drawn after the grade. An sRGB target encodes on store, so the
/// vertex color is linear. An unorm target stores the code unchanged.
pub(super) fn display_u8(color: [u8; 4], srgb_target: bool) -> [f32; 4] {
    if srgb_target {
        srgb_u8_to_linear(color)
    } else {
        [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            color[3] as f32 / 255.0,
        ]
    }
}

/// Narkowicz ACES fit. Same constants as `aces_tonemap` in the shaders.
pub(super) fn aces_narkowicz(x: f32) -> f32 {
    let n = x * (2.51 * x + 0.03);
    let d = x * (2.43 * x + 0.59) + 0.14;
    (n / d).clamp(0.0, 1.0)
}

fn mul3(m: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

/// AgX-style curve: published inset, log2, a smoothstep, published outset.
/// The matrices are Troy Sobotka's AgX base. The curve between them is original
/// and matches `agx_tonemap` in the shaders. Not a copy of three.js.
pub(super) fn agx_rgb(rgb: [f32; 3]) -> [f32; 3] {
    const INSET: [[f32; 3]; 3] = [
        [0.85662715, 0.09512124, 0.048251606],
        [0.13731897, 0.761242, 0.10143904],
        [0.11189821, 0.07679942, 0.81130236],
    ];
    const OUTSET: [[f32; 3]; 3] = [
        [1.1271006, -0.11060664, -0.016493939],
        [-0.14132977, 1.1578238, -0.016493939],
        [-0.14132977, -0.11060664, 1.2519364],
    ];
    let c = mul3(INSET, rgb.map(|x| x.max(0.0)));
    let mapped = c.map(|x| {
        let y = (x.max(1e-10).log2() + 12.47393) / (4.026069 + 12.47393);
        let t = y.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    });
    mul3(OUTSET, mapped).map(|x| x.clamp(0.0, 1.0))
}

/// Khronos PBR Neutral. Same constants as `neutral_tonemap` in the shaders.
pub(super) fn neutral_rgb(mut color: [f32; 3]) -> [f32; 3] {
    const START: f32 = 0.76;
    const DESAT: f32 = 0.15;
    let x = color[0].min(color[1]).min(color[2]);
    let offset = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
    color = color.map(|c| c - offset);
    let peak = color[0].max(color[1]).max(color[2]);
    if peak < START {
        return color.map(|c| c.max(0.0));
    }
    let d = 1.0 - START;
    let new_peak = 1.0 - d * d / (peak + d - START);
    let scale = new_peak / peak.max(1e-6);
    color = color.map(|c| c * scale);
    let g = 1.0 - 1.0 / (DESAT * (peak - new_peak) + 1.0);
    color
        .map(|c| c + (new_peak - c) * g)
        .map(|c| c.clamp(0.0, 1.0))
}

/// Exposure, optional curve, then the sRGB OETF. `curve` is 0 ACES, 1 AgX, 2 Neutral.
/// Matches `present_rgb` and the composite shader for a flat clear (no bloom, no LUT, vignette 0).
pub(super) fn grade_curve(color: [u8; 4], exposure: f32, tonemap: bool, curve: f32) -> [f32; 4] {
    let lin = srgb_u8_to_linear(color);
    let e = exposure.max(0.0);
    let mut rgb = [lin[0] * e, lin[1] * e, lin[2] * e];
    if tonemap {
        rgb = if curve > 1.5 {
            neutral_rgb(rgb)
        } else if curve > 0.5 {
            agx_rgb(rgb)
        } else {
            rgb.map(aces_narkowicz)
        };
    }
    [
        linear_channel_to_srgb(rgb[0]),
        linear_channel_to_srgb(rgb[1]),
        linear_channel_to_srgb(rgb[2]),
        lin[3],
    ]
}

/// Exposure, optional ACES, then the sRGB OETF.
#[allow(dead_code)]
pub(super) fn grade_authored(color: [u8; 4], exposure: f32, tonemap: bool) -> [f32; 4] {
    grade_curve(color, exposure, tonemap, 0.0)
}

#[cfg(test)]
pub(super) fn reference_curve(color: [u8; 4], exposure: f32, tonemap: bool, curve: f32) -> [u8; 4] {
    let g = grade_curve(color, exposure, tonemap, curve);
    [
        (g[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (g[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (g[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        color[3],
    ]
}

#[cfg(test)]
pub(super) fn reference_byte(color: [u8; 4], exposure: f32, tonemap: bool) -> [u8; 4] {
    reference_curve(color, exposure, tonemap, 0.0)
}

/// True when the 3D pass writes the swapchain and the hardware will not encode.
pub(super) fn present_in_fragment(direct_to_target: bool, target_is_srgb: bool) -> bool {
    direct_to_target && !target_is_srgb
}

/// 2×2 box. Color maps (`srgb`) average in linear and store the OETF so the
/// sampler's decode stays correct at every mip. Data maps average the unorm.
pub(super) fn box_downsample(prev: &[u8], pw: u32, ph: u32, srgb: bool) -> Vec<u8> {
    let nw = (pw / 2).max(1);
    let nh = (ph / 2).max(1);
    let mut next = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        for x in 0..nw {
            let mut acc = [0.0f32; 4];
            for dy in 0..2 {
                for dx in 0..2 {
                    let sx = (2 * x + dx).min(pw - 1);
                    let sy = (2 * y + dy).min(ph - 1);
                    let i = ((sy * pw + sx) * 4) as usize;
                    for c in 0..4 {
                        let b = prev[i + c] as f32 / 255.0;
                        acc[c] += if srgb && c < 3 {
                            srgb_channel_to_linear(b)
                        } else {
                            b
                        };
                    }
                }
            }
            let di = ((y * nw + x) * 4) as usize;
            for c in 0..4 {
                let v = acc[c] * 0.25;
                let encoded = if srgb && c < 3 {
                    linear_channel_to_srgb(v)
                } else {
                    v
                };
                next[di + c] = (encoded.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use palette::{LinSrgb, Srgb};

    fn palette_to_linear(encoded: f32) -> f32 {
        Srgb::new(encoded, 0.0, 0.0).into_linear().red
    }

    fn palette_to_srgb(linear: f32) -> f32 {
        Srgb::from_linear(LinSrgb::new(linear, 0.0, 0.0)).red
    }

    #[test]
    fn transfer_matches_palette() {
        for byte in [0u8, 1, 16, 32, 64, 128, 186, 200, 255] {
            let e = byte as f32 / 255.0;
            let lin = srgb_channel_to_linear(e);
            assert!(
                (lin - palette_to_linear(e)).abs() < 1e-6,
                "decode {byte}: {lin} vs palette"
            );
            let back = linear_channel_to_srgb(lin);
            assert!(
                (back - palette_to_srgb(lin)).abs() < 1e-6,
                "encode {byte}: {back} vs palette"
            );
        }
    }

    #[test]
    fn mid_gray_is_not_half_linear() {
        let lin = srgb_u8_to_linear([128, 128, 128, 255]);
        assert!(lin[0] < 0.25 && lin[0] > 0.20, "got {}", lin[0]);
        assert!((lin[0] - palette_to_linear(128.0 / 255.0)).abs() < 1e-6);
        // One decode + one encode lands on the same code. Skipping the decode
        // and then encoding treats 128/255 as linear and stores ~188.
        let round = reference_byte([128, 64, 32, 255], 1.0, false);
        for (i, byte) in [128u8, 64, 32].iter().enumerate() {
            let d = (round[i] as i32 - *byte as i32).abs();
            assert!(d <= 1, "channel {i} roundtrip {round:?}");
        }
        let skipped = (linear_channel_to_srgb(128.0 / 255.0) * 255.0).round() as u8;
        assert!(skipped > 180, "missing decode encodes to {skipped}");
    }

    #[test]
    fn aces_runs_on_linear_before_the_oetf() {
        let raw = reference_byte([128, 64, 32, 255], 1.0, false);
        let graded = reference_byte([128, 64, 32, 255], 1.0, true);
        assert_ne!(raw, graded, "ACES must move a mid color");
        // Locked Narkowicz result for author (128, 64, 32): the toe lifts the
        // decoded red, so the stored code is 154, not the 188 of a missing decode.
        assert_eq!(graded, [154, 61, 19, 255]);
    }

    #[test]
    fn direct_unorm_is_the_only_fragment_present() {
        assert!(present_in_fragment(true, false));
        assert!(!present_in_fragment(true, true));
        assert!(!present_in_fragment(false, false));
        assert!(!present_in_fragment(false, true));
    }

    #[test]
    fn color_mip_averages_linear_data_mip_does_not() {
        let src = vec![
            0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255,
        ];
        let color = box_downsample(&src, 2, 2, true);
        let encoded_half = (linear_channel_to_srgb(0.5) * 255.0).round() as u8;
        assert_eq!(color[0], encoded_half);
        assert!(
            color[0] > 170,
            "gamma-space average would be 128, got {}",
            color[0]
        );
        assert_eq!(color[3], 255);
        let data = box_downsample(&[128, 128, 255, 255, 128, 128, 255, 255], 2, 1, false);
        assert_eq!(&data[..4], &[128, 128, 255, 255]);
    }

    #[test]
    fn agx_and_neutral_stay_in_range_and_leave_aces() {
        let hot = [4.0, 0.15, 0.05];
        let aces = hot.map(aces_narkowicz);
        let agx = agx_rgb(hot);
        let neutral = neutral_rgb(hot);
        for c in agx.into_iter().chain(neutral) {
            assert!((0.0..=1.0).contains(&c), "{c}");
        }
        assert!((agx[0] - aces[0]).abs() > 0.02, "agx {agx:?} aces {aces:?}");
        assert!(neutral[0] <= 1.0);
        let dim = neutral_rgb([0.2, 0.1, 0.05]);
        assert!(
            (dim[0] - 0.2).abs() < 0.08,
            "neutral should barely move a dark color {dim:?}"
        );
        assert!(agx_rgb([0.0, 0.0, 0.0]).iter().all(|c| *c < 0.05));
    }
}
