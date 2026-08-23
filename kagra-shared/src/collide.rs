//! xz 平面の OBB 衝突。GPU にも描画にも依存しない。
//!
//! 跳ね返りは小さく、当たったら押し出して速度を削る。ETS 寄りの感触。

use crate::vehicle::Truck;
use glam::Vec3;

/// 水平面の向き付き箱。半サイズはローカル (幅/2, 長さ/2)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obb2 {
    pub center: Vec3,
    pub half_extents: [f32; 2],
    pub heading: f32,
}

impl Obb2 {
    pub fn from_truck(truck: &Truck) -> Self {
        let size = truck.spec.size;
        Self {
            center: Vec3::new(truck.pos.x, 0.0, truck.pos.z),
            half_extents: [size.x * 0.5, size.z * 0.5],
            heading: truck.heading,
        }
    }

    pub fn from_box(center: Vec3, size: Vec3, heading: f32) -> Self {
        Self {
            center: Vec3::new(center.x, 0.0, center.z),
            half_extents: [size.x * 0.5, size.z * 0.5],
            heading,
        }
    }

    fn axes(self) -> (Vec3, Vec3) {
        let fwd = Vec3::new(self.heading.sin(), 0.0, self.heading.cos());
        let right = Vec3::new(fwd.z, 0.0, -fwd.x);
        (right, fwd)
    }

    /// 重なっていれば、自分を相手から離す最短ベクトル（水平）。
    pub fn separation(&self, other: &Obb2) -> Option<Vec3> {
        let (a_r, a_f) = self.axes();
        let (b_r, b_f) = other.axes();
        let d = self.center - other.center;

        let mut best_pen = f32::INFINITY;
        let mut best_axis = Vec3::ZERO;

        for axis in [a_r, a_f, b_r, b_f] {
            let axis = axis.normalize_or(Vec3::X);
            let r_a = project_radius(self, axis);
            let r_b = project_radius(other, axis);
            let dist = d.dot(axis).abs();
            let pen = r_a + r_b - dist;
            if pen <= 0.0 {
                return None;
            }
            if pen < best_pen {
                best_pen = pen;
                // 自分を相手から離す向き。
                best_axis = if d.dot(axis) >= 0.0 { axis } else { -axis };
            }
        }
        Some(best_axis * best_pen)
    }

    pub fn overlaps(&self, other: &Obb2) -> bool {
        self.separation(other).is_some()
    }
}

fn project_radius(obb: &Obb2, axis: Vec3) -> f32 {
    let (right, fwd) = obb.axes();
    obb.half_extents[0] * right.dot(axis).abs() + obb.half_extents[1] * fwd.dot(axis).abs()
}

/// `truck` を `obstacle` から押し出し、法線方向の速度成分を削る。
/// 戻り値は押し出し量（m）。当たっていなければ 0。
pub fn resolve_truck_vs_obb(truck: &mut Truck, obstacle: &Obb2) -> f32 {
    let me = Obb2::from_truck(truck);
    let Some(sep) = me.separation(obstacle) else {
        return 0.0;
    };
    let len = sep.length();
    if len < 1e-5 {
        return 0.0;
    }
    truck.pos += sep;
    let n = sep / len;
    let fwd = truck.forward();
    // 進行方向（符号付き）が壁に向かっている成分。
    let along = truck.speed.signum() * fwd;
    let into = (-along).dot(n).max(0.0);
    // 食い込んだ向きの速度を落とす。正面衝突ほど強く止まる。
    truck.speed *= 1.0 - 0.85 * into;
    if into > 0.4 {
        truck.speed *= 0.35;
    }
    len
}

/// 路外: lateral が道半幅を超えたら減速し、端でソフト壁。
/// 戻り値はいまの横ずれ（符号付き、右が正）。
pub fn apply_road_bounds(
    truck: &mut Truck,
    frame_pos: Vec3,
    frame_right: Vec3,
    half_width: f32,
) -> f32 {
    let to = truck.pos - frame_pos;
    let lateral = to.dot(frame_right);
    let limit = half_width;
    let overhang = lateral.abs() - limit;
    if overhang > 0.0 {
        // 路肩を走ると抵抗が増える（前進・後退どちらも減速）。
        let damp = overhang * 2.5 * crate::scene::FIXED_DT;
        if truck.speed > 0.0 {
            truck.speed = (truck.speed - damp).max(0.0);
        } else if truck.speed < 0.0 {
            truck.speed = (truck.speed + damp).min(0.0);
        }
        truck.speed *= 0.985;
        // ソフト壁: 行き過ぎたぶんを道側へ押し戻す。
        let push = (overhang + 0.15).min(1.5);
        let dir = if lateral > 0.0 {
            -frame_right
        } else {
            frame_right
        };
        truck.pos += dir * push;
    }
    lateral
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separated_boxes_do_not_overlap() {
        let a = Obb2::from_box(Vec3::ZERO, Vec3::new(2.0, 1.0, 4.0), 0.0);
        let b = Obb2::from_box(Vec3::new(10.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 4.0), 0.0);
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn overlapping_boxes_separate() {
        let a = Obb2::from_box(Vec3::ZERO, Vec3::new(2.0, 1.0, 4.0), 0.0);
        let b = Obb2::from_box(Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 4.0), 0.0);
        let sep = a.separation(&b).expect("should overlap");
        assert!(sep.length() > 0.5);
    }

    #[test]
    fn rotated_obb_still_detects_hit() {
        let a = Obb2::from_box(Vec3::ZERO, Vec3::new(2.0, 1.0, 6.0), 0.4);
        let b = Obb2::from_box(Vec3::new(0.5, 0.0, 0.5), Vec3::new(2.0, 1.0, 2.0), -0.3);
        assert!(a.overlaps(&b));
    }

    #[test]
    fn head_on_collision_kills_speed() {
        let mut truck = Truck {
            pos: Vec3::new(0.0, 0.0, 0.0),
            heading: 0.0,
            speed: 20.0,
            ..Truck::default()
        };
        let wall = Obb2::from_box(Vec3::new(0.0, 0.0, 5.0), Vec3::new(8.0, 1.0, 2.0), 0.0);
        // 車体は長さ 12m なので前方が壁に食い込む。
        let pen = resolve_truck_vs_obb(&mut truck, &wall);
        assert!(pen > 0.0, "expected penetration");
        assert!(truck.speed < 10.0, "speed should drop, got {}", truck.speed);
    }

    #[test]
    fn offroad_slows_and_pushes_back() {
        let mut truck = Truck {
            pos: Vec3::new(20.0, 0.0, 0.0),
            speed: 15.0,
            ..Truck::default()
        };
        let before = truck.speed;
        let lat = apply_road_bounds(&mut truck, Vec3::ZERO, Vec3::X, 8.0);
        assert!(lat > 8.0);
        assert!(truck.speed < before);
        assert!(truck.pos.x < 20.0);
    }
}
