//! 平行光シャドウの ortho 合わせ。GPU 不要。
//!
//! P4 は VRM AABB だけ。P5 は床・箱・Prop も和に入れる。
//! 空のような巨大メッシュは半辺を壊すので除外する。
//! 2 段カスケードは近（視点）と遠（和）。既定は 1 段。

use crate::frustum::Aabb;

/// これより大きい AABB は空扱い（`World3D(half=7)` の床は 14）。
pub(super) const SHADOW_SKIP_EXTENT: f32 = 24.0;
pub(super) const SHADOW_HALF_MIN: f32 = 2.0;
/// `World3D(half=7)` の 14×14 床が収まるように P4 の 14 から上げた。
pub(super) const SHADOW_HALF_MAX: f32 = 28.0;
/// 近段の ortho 半辺。屋外 2 段のときだけ使う。
pub(super) const SHADOW_NEAR_HALF: f32 = 12.0;
const SHADOW_HALF_PAD: f32 = 1.25;

pub(super) fn aabb_is_shadow_volume(aabb: &Aabb) -> bool {
    aabb.max_extent() <= SHADOW_SKIP_EXTENT
}

pub(super) fn fold_shadow_aabb(acc: Option<Aabb>, aabb: Aabb) -> Option<Aabb> {
    if !aabb_is_shadow_volume(&aabb) {
        return acc;
    }
    Some(match acc {
        Some(prev) => prev.union(aabb),
        None => aabb,
    })
}

/// 和 AABB からライト空間の中心と半辺。無しなら P4 と同じ既定。
pub(super) fn shadow_fit_center_half(union: Option<Aabb>) -> ([f32; 3], f32) {
    match union {
        Some(a) => {
            let half = (a.max_extent() * 0.5 * SHADOW_HALF_PAD)
                .clamp(SHADOW_HALF_MIN, SHADOW_HALF_MAX);
            (a.center(), half)
        }
        None => ([0.0, 1.0, 0.0], 6.0),
    }
}

/// 1 段なら今まで通りの fit を 2 回返す。2 段なら近（視点）と遠（和）。
pub(super) fn cascade_center_half(
    union: Option<Aabb>,
    focus: [f32; 3],
    cascades: u32,
) -> [([f32; 3], f32); 2] {
    let far = shadow_fit_center_half(union);
    if cascades < 2 {
        return [far, far];
    }
    let near_h = SHADOW_NEAR_HALF.min(far.1);
    [(focus, near_h), far]
}

/// ワールド XZ をテクセルにスナップして、2 段影が這うのを抑える。
pub(super) fn snap_center_xz(center: [f32; 3], half: f32, map_size: f32) -> [f32; 3] {
    let texel = (2.0 * half.max(0.5)) / map_size.max(1.0);
    if texel < 1e-8 {
        return center;
    }
    [
        (center[0] / texel).round() * texel,
        center[1],
        (center[2] / texel).round() * texel,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_aabb(min: [f32; 3], max: [f32; 3]) -> Aabb {
        Aabb { min, max }
    }

    #[test]
    fn vrm_only_clamps_to_legacy_min() {
        let vrm = box_aabb([-0.4, 0.0, -0.4], [0.4, 1.6, 0.4]);
        let acc = fold_shadow_aabb(None, vrm);
        let (center, half) = shadow_fit_center_half(acc);
        assert!((center[1] - 0.8).abs() < 1e-4);
        assert!((half - SHADOW_HALF_MIN).abs() < 1e-5);
    }

    #[test]
    fn world_floor_widens_ortho() {
        let floor = box_aabb([-7.0, -0.02, -7.0], [7.0, 0.02, 7.0]);
        let (_, half) = shadow_fit_center_half(fold_shadow_aabb(None, floor));
        let expected = (14.0 * 0.5 * SHADOW_HALF_PAD).clamp(SHADOW_HALF_MIN, SHADOW_HALF_MAX);
        assert!((half - expected).abs() < 1e-4);
        assert!(half > SHADOW_HALF_MIN);
        assert!(half < SHADOW_HALF_MAX);
    }

    #[test]
    fn union_vrm_and_floor_follows_floor() {
        let vrm = box_aabb([-0.4, 0.0, -0.4], [0.4, 1.6, 0.4]);
        let floor = box_aabb([-7.0, -0.02, -7.0], [7.0, 0.02, 7.0]);
        let acc = fold_shadow_aabb(fold_shadow_aabb(None, vrm), floor);
        let (_, half) = shadow_fit_center_half(acc);
        let floor_only = shadow_fit_center_half(fold_shadow_aabb(None, floor)).1;
        assert!((half - floor_only).abs() < 1e-4);
    }

    #[test]
    fn skips_sky_sized_aabb() {
        let floor = box_aabb([-7.0, -0.02, -7.0], [7.0, 0.02, 7.0]);
        let sky = box_aabb([-40.0, -40.0, -40.0], [40.0, 40.0, 40.0]);
        assert!(!aabb_is_shadow_volume(&sky));
        let with_sky = fold_shadow_aabb(fold_shadow_aabb(None, floor), sky);
        let floor_only = fold_shadow_aabb(None, floor);
        assert_eq!(with_sky, floor_only);
    }

    #[test]
    fn world_only_no_vrm() {
        let crate_box = box_aabb([3.0, 0.0, -1.0], [4.0, 1.0, 0.0]);
        let (center, half) = shadow_fit_center_half(fold_shadow_aabb(None, crate_box));
        assert!((center[0] - 3.5).abs() < 1e-4);
        assert!((half - SHADOW_HALF_MIN).abs() < 1e-5);
    }

    #[test]
    fn empty_matches_legacy_default() {
        let (center, half) = shadow_fit_center_half(None);
        assert_eq!(center, [0.0, 1.0, 0.0]);
        assert!((half - 6.0).abs() < 1e-6);
    }

    #[test]
    fn huge_union_clamps_half() {
        let wide = box_aabb([-30.0, 0.0, -30.0], [30.0, 1.0, 30.0]);
        let (_, half) = shadow_fit_center_half(Some(wide));
        assert!((half - SHADOW_HALF_MAX).abs() < 1e-5);
    }

    #[test]
    fn one_cascade_copies_the_union_fit() {
        let floor = box_aabb([-7.0, -0.02, -7.0], [7.0, 0.02, 7.0]);
        let acc = fold_shadow_aabb(None, floor);
        let one = cascade_center_half(acc, [9.0, 1.0, 3.0], 1);
        let far = shadow_fit_center_half(acc);
        assert_eq!(one[0], far);
        assert_eq!(one[1], far);
    }

    #[test]
    fn two_cascades_near_follows_focus() {
        let floor = box_aabb([-7.0, -0.02, -7.0], [7.0, 0.02, 7.0]);
        let acc = fold_shadow_aabb(None, floor);
        let two = cascade_center_half(acc, [4.0, 1.5, -2.0], 2);
        assert!((two[0].0[0] - 4.0).abs() < 1e-5);
        assert!((two[0].1 - SHADOW_NEAR_HALF.min(two[1].1)).abs() < 1e-5);
        assert_eq!(two[1], shadow_fit_center_half(acc));
    }

    #[test]
    fn snap_xz_quantizes_and_ignores_sub_texel() {
        let half = 12.0;
        let map = 2048.0;
        let texel = 24.0 / map;
        let a = snap_center_xz([4.0, 1.5, -2.0], half, map);
        let b = snap_center_xz([4.0 + texel * 0.2, 1.5, -2.0], half, map);
        assert_eq!(a, b);
        assert!((a[1] - 1.5).abs() < 1e-6);
        assert!((a[0] / texel - (a[0] / texel).round()).abs() < 1e-4);
    }
}
