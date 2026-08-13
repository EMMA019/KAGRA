//! 参照シーン: スプライン道路をトラックで走る。
//!
//! 道は `road::RoadStreamer` がチャンク単位で切り出し、距離に応じて LOD を落とす。
//! ワールド生成もカリングも GPU に触らないので、絵を出さずに CI で検証できる。

use crate::road::{LodLevel, RoadStreamer};
use crate::scene::{DrawList, Quad, FIXED_DT};
use crate::scene3d::{primitives, Aabb, Camera, Material, MeshData, MeshId, Scene3D, SceneBuilder};
use crate::vehicle::{ChaseCamera, DriveInput, Truck};
use glam::{Mat4, Vec3};

/// 道幅（m）。
pub const ROAD_WIDTH: f32 = 16.0;
/// フォグの切れ目。ストリーマの `keep_ahead` と揃える。
const VIEW_AHEAD: f32 = 420.0;

/// シーンが使うメッシュ。`Renderer` が起動時に一度アップロードする。
pub struct MeshSet {
    pub ground: MeshData,
    pub road: MeshData,
    pub dash: MeshData,
    pub pole: MeshData,
    pub truck: MeshData,
    pub cab: MeshData,
    pub sky: MeshData,
    pub shadow: MeshData,
}

/// アップロード後のハンドル。並びは `MeshSet` と同じ。
#[derive(Clone, Copy, Debug)]
pub struct MeshIds {
    pub ground: MeshId,
    pub road: MeshId,
    pub dash: MeshId,
    pub pole: MeshId,
    pub truck: MeshId,
    pub cab: MeshId,
    pub sky: MeshId,
    pub shadow: MeshId,
}

impl MeshSet {
    pub fn build(truck_size: Vec3) -> Self {
        Self {
            // 地面はトラックに追従させる大きな板。フォグで端が見えない。
            ground: primitives::plane_mesh(4000.0, 4000.0),
            // 道セグメント。ローカルで幅 X・長さ Z。経路フレームで回す。
            road: primitives::plane_mesh(ROAD_WIDTH, 8.0),
            dash: primitives::plane_mesh(0.5, 4.0),
            pole: primitives::box_mesh(Vec3::new(0.25, 3.0, 0.25)),
            // glTF が無ければ箱。あれば `MeshSet::build` の前に差し替え可能。
            truck: primitives::box_mesh(truck_size),
            cab: primitives::box_mesh(Vec3::new(
                truck_size.x * 0.95,
                truck_size.y * 0.55,
                truck_size.z * 0.28,
            )),
            sky: primitives::sky_dome(800.0, 24),
            // 車体の下に敷く半透明の影。実シャドウマップは WebGL2 では重いのでこれで足す。
            shadow: primitives::plane_mesh(truck_size.x * 1.4, truck_size.z * 1.2),
        }
    }

    pub fn as_slice(&self) -> [&MeshData; 8] {
        [
            &self.ground,
            &self.road,
            &self.dash,
            &self.pole,
            &self.truck,
            &self.cab,
            &self.sky,
            &self.shadow,
        ]
    }
}

/// 運転デモの状態。
#[derive(Clone, Debug)]
pub struct DrivingScene {
    pub truck: Truck,
    pub camera: ChaseCamera,
    pub input: DriveInput,
    pub streamer: RoadStreamer,
    /// 経路上の弧長位置（m）。チャンク選択の基準。
    pub path_s: f32,
    /// 走行距離（m）。HUD とスコア用。
    pub odometer: f32,
    elapsed: f32,
}

impl Default for DrivingScene {
    fn default() -> Self {
        let streamer = RoadStreamer::default();
        // スタートは経路の少し先。カメラが後ろから追えるようにする。
        let start = streamer.path.sample(8.0);
        let truck = Truck {
            pos: start.pos,
            heading: start.heading(),
            ..Truck::default()
        };

        let mut camera = ChaseCamera::default();
        camera.update(&truck, FIXED_DT);
        Self {
            truck,
            camera,
            input: DriveInput::default(),
            streamer,
            path_s: start.distance,
            odometer: 0.0,
            elapsed: 0.0,
        }
    }
}

impl DrivingScene {
    pub fn set_input(&mut self, input: DriveInput) {
        self.input = input.clamped();
    }

    /// 固定ステップで 1 フレーム進める。
    pub fn update(&mut self) {
        let before = self.truck.pos;
        self.truck.update(self.input, FIXED_DT);
        self.odometer += self.truck.pos.distance(before);
        // 経路位置は「いまの座標に一番近い点」。道を外れてもチャンクは追従する。
        self.path_s = self.streamer.path.nearest(self.truck.pos).distance;
        self.camera.update(&self.truck, FIXED_DT);
        self.elapsed += FIXED_DT;
    }

    pub fn camera3d(&self) -> Camera {
        Camera {
            eye: self.camera.eye(),
            target: self.camera.target(),
            up: Vec3::Y,
            fov_y: 62f32.to_radians(),
            near: 0.5,
            far: 900.0,
        }
    }

    /// いま生きているチャンク数。テストとデバッグ用。
    pub fn active_chunk_count(&self) -> usize {
        self.streamer.active_chunks(self.path_s).len()
    }

    /// 3D の描画内容を組み立てる。`ids` はアップロード済みメッシュのハンドル。
    pub fn build_scene(&self, ids: &MeshIds, aspect: f32) -> Scene3D {
        let camera = self.camera3d();
        let mut b = SceneBuilder::new(&camera, aspect);

        let truck_size = self.truck.spec.size;
        // 道セグメントは長さ 8m。LOD でステップが変わっても境界箱は同じでよい。
        b.register(ids.road, plane_bounds(ROAD_WIDTH, 8.0));
        b.register(ids.dash, plane_bounds(0.5, 4.0));
        b.register(
            ids.pole,
            Aabb::from_center_size(Vec3::ZERO, Vec3::new(0.25, 3.0, 0.25)),
        );
        b.register(ids.truck, Aabb::from_center_size(Vec3::ZERO, truck_size));
        b.register(
            ids.shadow,
            plane_bounds(truck_size.x * 1.4, truck_size.z * 1.2),
        );
        // スカイは常に見えるのでカリング登録しない。

        // 天球はカメラ中心。クリア色の代わりにグラデーションを出す。
        b.push_material(
            ids.sky,
            Mat4::from_translation(camera.eye),
            [138, 172, 214, 255],
            Material::Sky,
        );

        b.push_material(
            ids.ground,
            Mat4::from_translation(Vec3::new(self.truck.pos.x, -0.02, self.truck.pos.z)),
            [86, 112, 72, 255],
            Material::Grass,
        );

        for chunk in self.streamer.active_chunks(self.path_s) {
            self.emit_chunk(&mut b, ids, chunk.start_s, chunk.end_s, chunk.lod);
        }

        let model = self.truck.model_matrix();
        // 接地影。実シャドウマップの代わり。
        b.push_material(
            ids.shadow,
            model * Mat4::from_translation(Vec3::new(0.0, 0.03, 0.0)),
            [0, 0, 0, 90],
            Material::Solid,
        );
        b.push(
            ids.truck,
            model * Mat4::from_translation(Vec3::new(0.0, truck_size.y * 0.5, 0.0)),
            [200, 72, 56, 255],
        );
        b.push(
            ids.cab,
            model
                * Mat4::from_translation(Vec3::new(0.0, truck_size.y * 1.05, truck_size.z * 0.30)),
            [235, 235, 240, 255],
        );

        let sky = [138, 172, 214, 255];
        Scene3D {
            camera,
            clear: sky,
            light_dir: Vec3::new(-0.35, 0.9, 0.25).normalize(),
            ambient: 0.42,
            fog_color: sky,
            fog_start: 140.0,
            fog_end: VIEW_AHEAD,
            batches: b.finish(),
        }
    }

    fn emit_chunk(
        &self,
        b: &mut SceneBuilder,
        ids: &MeshIds,
        start_s: f32,
        end_s: f32,
        lod: LodLevel,
    ) {
        let step = self.streamer.segment_step(lod);
        let frames = self.streamer.path.walk(start_s, end_s, step);
        // 隣り合うサンプルの中点に 8m の板を置く。刻みは 8m 以下なので少し重なり、
        // カーブでも隙間が出ない。非均一スケールはシェーダが法線を壊すので使わない。
        for pair in frames.windows(2) {
            let a = &pair[0];
            let c = &pair[1];
            let mid = self.streamer.path.sample((a.distance + c.distance) * 0.5);
            b.push_material(
                ids.road,
                mid.model(0.0, 0.0, 0.0),
                [58, 58, 64, 255],
                Material::Road,
            );

            if self.streamer.wants_dashes(lod) {
                b.push(ids.dash, mid.model(0.0, 0.01, 0.0), [226, 220, 180, 255]);
            }
        }

        if let Some(pole_step) = self.streamer.pole_step(lod) {
            for frame in self.streamer.path.walk(start_s, end_s, pole_step) {
                let shoulder = ROAD_WIDTH * 0.5 + 1.5;
                for side in [-1.0, 1.0] {
                    b.push(
                        ids.pole,
                        frame.model(side * shoulder, 1.5, 0.0),
                        [216, 216, 220, 255],
                    );
                }
            }
        }
    }

    /// 速度計などの 2D オーバーレイ。既存のクアッド描画をそのまま使う。
    pub fn build_hud(&self, width: u32, height: u32, paused: bool) -> DrawList {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let mut quads = Vec::with_capacity(24);

        let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
        let pad = 18.0 * scale;
        let bar_w = 260.0 * scale;
        let bar_h = 14.0 * scale;

        let top = practical_top_speed(&self.truck.spec);
        let ratio = (self.truck.speed / top).clamp(0.0, 1.0);
        quads.push(Quad::new(
            pad,
            h - pad - bar_h,
            bar_w,
            bar_h,
            [0, 0, 0, 120],
        ));
        quads.push(Quad::new(
            pad,
            h - pad - bar_h,
            bar_w * ratio,
            bar_h,
            speed_color(ratio),
        ));

        for i in 1..4 {
            let x = pad + bar_w * i as f32 / 4.0;
            quads.push(Quad::new(
                x,
                h - pad - bar_h,
                2.0 * scale,
                bar_h,
                [0, 0, 0, 90],
            ));
        }

        let steer_w = 160.0 * scale;
        let steer_x = w - pad - steer_w;
        let steer_y = h - pad - bar_h;
        quads.push(Quad::new(steer_x, steer_y, steer_w, bar_h, [0, 0, 0, 120]));
        let t = (self.truck.steer_angle / self.truck.spec.max_steer).clamp(-1.0, 1.0);
        let cx = steer_x + steer_w * 0.5;
        let half = steer_w * 0.5 * t;
        quads.push(Quad::new(
            cx.min(cx + half),
            steer_y,
            half.abs().max(2.0 * scale),
            bar_h,
            [120, 200, 255, 255],
        ));

        // ルート進捗。経路長に対する path_s の割合。
        let progress = if self.streamer.path.length() > 1.0 {
            (self.path_s / self.streamer.path.length()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let route_w = 200.0 * scale;
        quads.push(Quad::new(pad, pad, route_w, 8.0 * scale, [0, 0, 0, 120]));
        quads.push(Quad::new(
            pad,
            pad,
            route_w * progress,
            8.0 * scale,
            [255, 210, 120, 255],
        ));

        if paused {
            quads.push(Quad::new(0.0, 0.0, w, h, [10, 12, 20, 140]));
        }

        DrawList {
            clear: [0, 0, 0, 0],
            quads,
        }
    }
}

fn plane_bounds(width: f32, depth: f32) -> Aabb {
    Aabb::from_center_size(Vec3::ZERO, Vec3::new(width, 0.02, depth))
}

fn speed_color(ratio: f32) -> [u8; 4] {
    let r = (90.0 + 165.0 * ratio) as u8;
    let g = (230.0 - 120.0 * ratio) as u8;
    [r, g, 120, 255]
}

/// 空気抵抗と駆動力が釣り合う速度。HUD の満目盛りに使う。
pub fn practical_top_speed(spec: &crate::vehicle::TruckSpec) -> f32 {
    if spec.drag <= 0.0 {
        return spec.max_speed;
    }
    (spec.accel / spec.drag).sqrt().min(spec.max_speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> MeshIds {
        MeshIds {
            ground: MeshId(0),
            road: MeshId(1),
            dash: MeshId(2),
            pole: MeshId(3),
            truck: MeshId(4),
            cab: MeshId(5),
            sky: MeshId(6),
            shadow: MeshId(7),
        }
    }

    fn drive(sc: &mut DrivingScene, input: DriveInput, seconds: f32) {
        sc.set_input(input);
        for _ in 0..(seconds / FIXED_DT) as usize {
            sc.update();
        }
    }

    #[test]
    fn scene_has_road_and_truck() {
        let sc = DrivingScene::default();
        let s = sc.build_scene(&ids(), 16.0 / 9.0);
        assert!(s.instance_count() > 10, "world should not be empty");
        let meshes: Vec<_> = s.batches.iter().map(|b| b.mesh).collect();
        assert!(meshes.contains(&ids().road));
        assert!(meshes.contains(&ids().truck));
        assert!(meshes.contains(&ids().ground));
    }

    #[test]
    fn driving_forward_advances_along_the_path() {
        let mut sc = DrivingScene::default();
        let start_s = sc.path_s;
        drive(
            &mut sc,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            15.0,
        );
        assert!(
            sc.path_s > start_s + 20.0,
            "path_s {} -> {}",
            start_s,
            sc.path_s
        );
        assert!(sc.odometer > 20.0);
    }

    #[test]
    fn instance_count_stays_bounded_while_driving() {
        let mut sc = DrivingScene::default();
        let start = sc.build_scene(&ids(), 1.6).instance_count();
        drive(
            &mut sc,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            120.0,
        );
        let later = sc.build_scene(&ids(), 1.6).instance_count();
        assert!(
            later < start * 2,
            "instances grew from {start} to {later}; chunks are not being unloaded"
        );
        assert!(sc.active_chunk_count() <= 10);
    }

    #[test]
    fn far_chunks_drop_dashes_and_poles() {
        let sc = DrivingScene::default();
        // スタート直後は前方に Far チャンクがある。道インスタンスは多く、
        // ポールは Near/Mid だけなので道より少ない。
        let s = sc.build_scene(&ids(), 1.6);
        let count = |id: MeshId| {
            s.batches
                .iter()
                .find(|b| b.mesh == id)
                .map(|b| b.instances.len())
                .unwrap_or(0)
        };
        assert!(count(ids().road) > count(ids().pole));
        assert!(count(ids().road) > count(ids().dash));
    }

    #[test]
    fn road_bends_away_from_the_z_axis() {
        let path = crate::road::RoadPath::demo_route();
        let on_curve = path.sample(320.0);
        assert!(
            on_curve.pos.x.abs() > 5.0,
            "demo route should leave the Z axis, pos={}",
            on_curve.pos
        );
    }

    #[test]
    fn curved_section_still_emits_road_near_the_truck() {
        let mut sc = DrivingScene::default();
        let pose = sc.streamer.path.sample(320.0);
        sc.truck.pos = pose.pos;
        sc.truck.heading = pose.heading();
        sc.path_s = pose.distance;
        sc.camera = ChaseCamera::default();
        sc.camera.update(&sc.truck, FIXED_DT);

        let s = sc.build_scene(&ids(), 1.6);
        let road = s
            .batches
            .iter()
            .find(|b| b.mesh == ids().road)
            .expect("road batch");
        assert!(road.instances.len() > 20);
        let eye = sc.camera.eye();
        let near = road
            .instances
            .iter()
            .map(|i| i.model.w_axis.truncate().distance(eye))
            .fold(f32::INFINITY, f32::min);
        assert!(
            near < 40.0,
            "road should sit in front of the chase camera, nearest={near}"
        );
    }

    #[test]
    fn culling_drops_scenery_behind_the_camera() {
        let sc = DrivingScene::default();
        let camera = sc.camera3d();
        let mut b = SceneBuilder::new(&camera, 1.6);
        b.register(
            ids().pole,
            Aabb::from_center_size(Vec3::ZERO, Vec3::new(0.25, 3.0, 0.25)),
        );
        let behind = sc.camera.eye() - sc.truck.forward() * 50.0;
        b.push(
            ids().pole,
            Mat4::from_translation(behind + Vec3::Y * 1.5),
            [255; 4],
        );
        assert_eq!(b.culled(), 1);
    }

    #[test]
    fn odometer_tracks_distance() {
        let mut sc = DrivingScene::default();
        let start = sc.truck.pos;
        drive(
            &mut sc,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            20.0,
        );
        assert!((sc.odometer - sc.truck.pos.distance(start)).abs() < 2.0);
    }

    #[test]
    fn hud_speed_bar_grows_with_speed() {
        let mut sc = DrivingScene::default();
        let idle = hud_bar_width(&sc);
        drive(
            &mut sc,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            10.0,
        );
        assert!(hud_bar_width(&sc) > idle);
    }

    #[test]
    fn hud_stays_on_screen() {
        let mut sc = DrivingScene::default();
        drive(
            &mut sc,
            DriveInput {
                throttle: 1.0,
                steer: 1.0,
                ..Default::default()
            },
            30.0,
        );
        let (w, h) = (720u32, 1280u32);
        for q in sc.build_hud(w, h, false).quads {
            assert!(q.x >= -1.0 && q.y >= -1.0, "quad off the top-left: {q:?}");
            assert!(
                q.x + q.w <= w as f32 + 1.0 && q.y + q.h <= h as f32 + 1.0,
                "quad off the bottom-right: {q:?}"
            );
        }
    }

    #[test]
    fn practical_top_speed_is_reachable_and_sane() {
        let spec = crate::vehicle::TruckSpec::default();
        let top = practical_top_speed(&spec);
        assert!(
            (60.0..=110.0).contains(&(top * 3.6)),
            "top speed {top} m/s is not truck-like"
        );

        let mut sc = DrivingScene::default();
        drive(
            &mut sc,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            300.0,
        );
        assert!(sc.truck.speed <= top + 0.5);
        assert!(sc.truck.speed > top * 0.9, "should approach the top speed");
    }

    fn hud_bar_width(sc: &DrivingScene) -> f32 {
        sc.build_hud(1280, 720, false).quads[1].w
    }
}
