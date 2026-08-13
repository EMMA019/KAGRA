//! 回廊型オープンワールド。チャンク index から決定的に路肩建物を置く。

use crate::collide::Obb2;
use crate::road::{LodLevel, RoadPath, RoadStreamer};
use crate::scene3d::{Material, MeshId, SceneBuilder};
use glam::{Mat4, Vec3};

/// 建物の見た目寸法（幅・高さ・奥行き）。
#[derive(Clone, Copy, Debug)]
pub struct BuildingSpec {
    pub size: Vec3,
    pub color: [u8; 4],
    pub lateral: f32,
    pub along_s: f32,
    pub heading: f32,
}

/// `chunk` の路肩に置く建物。Far では空。
pub fn buildings_for_chunk(
    path: &RoadPath,
    start_s: f32,
    end_s: f32,
    chunk_index: i32,
    lod: LodLevel,
) -> Vec<BuildingSpec> {
    if matches!(lod, LodLevel::Far) {
        return Vec::new();
    }
    let slots = match lod {
        LodLevel::Near => 4,
        LodLevel::Mid => 2,
        LodLevel::Far => 0,
    };
    let mut out = Vec::with_capacity(slots * 2);
    let span = (end_s - start_s).max(1.0);
    for i in 0..slots {
        let t = (i as f32 + 0.5) / slots as f32;
        let s = start_s + span * t;
        let seed = (chunk_index as u32)
            .wrapping_mul(1009)
            .wrapping_add(i as u32 * 17);
        if hash01(seed) < 0.2 {
            continue;
        }
        let frame = path.sample(s);
        let h = 6.0 + hash01(seed.wrapping_add(1)) * 18.0;
        let w = 8.0 + hash01(seed.wrapping_add(2)) * 10.0;
        let d = 8.0 + hash01(seed.wrapping_add(3)) * 12.0;
        // Mid は大きな箱だけ。
        let size = if matches!(lod, LodLevel::Mid) {
            Vec3::new(w * 1.2, h * 0.85, d * 1.2)
        } else {
            Vec3::new(w, h, d)
        };
        let shoulder = 16.0 * 0.5 + 6.0 + size.x * 0.5;
        // 暖色にして道路画素判定（暗い寒色グレー）と被らないようにする。
        let warm = hash01(seed.wrapping_add(4));
        let color = [
            (120.0 + warm * 50.0) as u8,
            (95.0 + warm * 35.0) as u8,
            (70.0 + warm * 25.0) as u8,
            255,
        ];
        for side in [-1.0, 1.0] {
            out.push(BuildingSpec {
                size,
                color,
                lateral: side * shoulder,
                along_s: s,
                heading: frame.heading(),
            });
        }
    }
    out
}

pub fn emit_buildings(b: &mut SceneBuilder, mesh: MeshId, path: &RoadPath, specs: &[BuildingSpec]) {
    for spec in specs {
        let frame = path.sample(spec.along_s);
        let pos = frame.pos + frame.right * spec.lateral + Vec3::Y * (spec.size.y * 0.5);
        let model = Mat4::from_scale_rotation_translation(
            spec.size,
            glam::Quat::from_rotation_y(spec.heading),
            pos,
        );
        b.push_material(mesh, model, spec.color, Material::Solid);
    }
}

/// アクティブチャンクの建物コライダ。
pub fn building_colliders(streamer: &RoadStreamer, path_s: f32) -> Vec<Obb2> {
    let mut out = Vec::new();
    for chunk in streamer.active_chunks(path_s) {
        let specs = buildings_for_chunk(
            &streamer.path,
            chunk.start_s,
            chunk.end_s,
            chunk.index,
            chunk.lod,
        );
        for spec in specs {
            let frame = streamer.path.sample(spec.along_s);
            let pos = frame.pos + frame.right * spec.lateral;
            out.push(Obb2::from_box(pos, spec.size, spec.heading));
        }
    }
    out
}

/// アクティブチャンクのポールコライダ（路肩）。
pub fn pole_colliders(streamer: &RoadStreamer, path_s: f32, road_width: f32) -> Vec<Obb2> {
    let mut out = Vec::new();
    for chunk in streamer.active_chunks(path_s) {
        let Some(step) = streamer.pole_step(chunk.lod) else {
            continue;
        };
        for frame in streamer.path.walk(chunk.start_s, chunk.end_s, step) {
            let shoulder = road_width * 0.5 + 1.5;
            for side in [-1.0, 1.0] {
                let pos = frame.pos + frame.right * (side * shoulder);
                out.push(Obb2::from_box(
                    pos,
                    Vec3::new(0.4, 3.0, 0.4),
                    frame.heading(),
                ));
            }
        }
    }
    out
}

fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(747796405).wrapping_add(2891336453);
    x = ((x >> ((x >> 28) + 4)) ^ x).wrapping_mul(277803737);
    x = (x >> 22) ^ x;
    (x as f32) / (u32::MAX as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::road::RoadStreamer;

    #[test]
    fn buildings_are_deterministic() {
        let streamer = RoadStreamer::default();
        let a = buildings_for_chunk(&streamer.path, 0.0, 80.0, 0, LodLevel::Near);
        let b = buildings_for_chunk(&streamer.path, 0.0, 80.0, 0, LodLevel::Near);
        assert_eq!(a.len(), b.len());
        assert!((a[0].along_s - b[0].along_s).abs() < 1e-4);
    }

    #[test]
    fn far_chunks_have_no_buildings() {
        let streamer = RoadStreamer::default();
        let far = buildings_for_chunk(&streamer.path, 0.0, 80.0, 0, LodLevel::Far);
        assert!(far.is_empty());
    }

    #[test]
    fn colliders_exist_near_the_truck() {
        let streamer = RoadStreamer::default();
        let cols = building_colliders(&streamer, 40.0);
        assert!(!cols.is_empty());
    }
}
