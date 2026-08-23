//! VRM humanoid ボーン球 vs レイ（クリックピック）。
//! ジェスチャ認識はエンジンの外。

use nalgebra::Vector3;

/// humanoid 標準名のピック球半径（メートル）。
pub fn bone_pick_radius(name: &str) -> f32 {
    match name {
        "head" => 0.14,
        "hips" | "spine" | "chest" | "upperChest" => 0.11,
        "neck" => 0.07,
        "leftHand" | "rightHand" => 0.08,
        "leftFoot" | "rightFoot" | "leftToes" | "rightToes" => 0.07,
        n if n.contains("Distal")
            || n.contains("Intermediate")
            || n.contains("Proximal")
            || n.contains("Metacarpal") =>
        {
            0.025
        }
        n if n.contains("Arm") || n.contains("Leg") || n.contains("Shoulder") => 0.07,
        _ => 0.06,
    }
}

/// 正規化済み方向。ヒット距離 t（origin + t*dir）を返す。
pub fn ray_sphere(
    origin: Vector3<f32>,
    dir: Vector3<f32>,
    center: Vector3<f32>,
    radius: f32,
) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(&dir);
    let c = oc.dot(&oc) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let s = disc.sqrt();
    let t0 = -b - s;
    if t0 >= 0.0 {
        return Some(t0);
    }
    let t1 = -b + s;
    if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_hit_in_front() {
        let t = ray_sphere(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(0.0, 0.0, 5.0),
            0.5,
        );
        assert!(t.is_some());
        let t = t.unwrap();
        assert!((t - 4.5).abs() < 1e-4);
    }

    #[test]
    fn sphere_miss() {
        let t = ray_sphere(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(2.0, 0.0, 5.0),
            0.5,
        );
        assert!(t.is_none());
    }

    #[test]
    fn head_radius_larger_than_finger() {
        assert!(bone_pick_radius("head") > bone_pick_radius("leftIndexDistal") * 3.0);
    }
}
