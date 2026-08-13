//! 車両運動。GPU にも描画にも依存しない純粋な計算。
//!
//! 車輪ごとの荷重やサスペンションは扱わず、前輪 1 本・後輪 1 本に潰した
//! いわゆる自転車モデルで進める。曲がる感触と速度感を出すにはこれで足り、
//! 全部が純関数なので GPU の無い CI で挙動を固定できる。

use glam::{Mat4, Quat, Vec3};

/// ドライバの入力。いずれも連続値で、シェル側が仮想ハンドルでもスライダーでも
/// 傾きセンサでも、コアはこの 3 軸しか見ない。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DriveInput {
    /// -1（左いっぱい）〜 +1（右いっぱい）。
    pub steer: f32,
    /// 0〜1。
    pub throttle: f32,
    /// 0〜1。
    pub brake: f32,
}

impl DriveInput {
    pub fn clamped(self) -> Self {
        Self {
            steer: self.steer.clamp(-1.0, 1.0),
            throttle: self.throttle.clamp(0.0, 1.0),
            brake: self.brake.clamp(0.0, 1.0),
        }
    }
}

/// 車体の諸元。トラックらしく重くて曲がりにくい値を既定にしてある。
#[derive(Clone, Copy, Debug)]
pub struct TruckSpec {
    /// 前後車軸の距離（m）。大きいほど曲がらない。
    pub wheelbase: f32,
    /// 実際の最大切れ角（ラジアン）。
    pub max_steer: f32,
    /// ハンドルが最大まで切れるのにかかる時間（秒）。急ハンドルを防ぐ。
    pub steer_rate: f32,
    /// 駆動加速度（m/s^2）。
    pub accel: f32,
    /// 制動減速度（m/s^2）。
    pub brake: f32,
    /// 速度の二乗に比例する空気抵抗。実質の最高速は accel と釣り合う点で決まり、
    /// `max_speed` はその上に置く安全弁でしかない。
    pub drag: f32,
    /// 何も踏まないときの減速（m/s^2）。エンジンブレーキ相当。
    pub rolling: f32,
    pub max_speed: f32,
    /// 見た目の寸法（幅・高さ・長さ）。描画と境界箱に使う。
    pub size: Vec3,
}

impl Default for TruckSpec {
    fn default() -> Self {
        Self {
            wheelbase: 6.0,
            max_steer: 32f32.to_radians(),
            steer_rate: 0.35,
            accel: 4.5,
            brake: 9.0,
            // accel と釣り合うのが 25m/s ≒ 90km/h になるよう選んだ。
            drag: 0.0072,
            rolling: 1.2,
            max_speed: 33.0, // ≒120km/h（下り坂などの上限）
            size: Vec3::new(2.5, 3.4, 12.0),
        }
    }
}

/// 平面上を走る車体。位置は xz、向きは y 軸まわりの角度だけ持つ。
///
/// 向きの符号は右手系の y 軸まわりの回転そのままなので、増えると上から見て
/// 反時計まわり（+Z から +X へ）。一方で `steer_angle` は運転席から見た向きで
/// 定義し、**正が右**。カメラは車体の後ろから +Z 方向を見るので、右手系では
/// 画面の右が -X になり、右へ切ると `heading` は減る。
#[derive(Clone, Copy, Debug)]
pub struct Truck {
    pub spec: TruckSpec,
    pub pos: Vec3,
    /// +Z を 0 とした y 軸まわりの向き（ラジアン、反時計まわりが正）。
    pub heading: f32,
    /// 前進速度（m/s）。後退は扱わないので常に 0 以上。
    pub speed: f32,
    /// いま実際に切れている前輪の角度（ラジアン）。正が右。入力から遅れて追従する。
    pub steer_angle: f32,
}

impl Default for Truck {
    fn default() -> Self {
        Self {
            spec: TruckSpec::default(),
            pos: Vec3::ZERO,
            heading: 0.0,
            speed: 0.0,
            steer_angle: 0.0,
        }
    }
}

impl Truck {
    /// 車体の前方向。
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.heading.sin(), 0.0, self.heading.cos())
    }

    pub fn speed_kmh(&self) -> f32 {
        self.speed * 3.6
    }

    /// 描画用の姿勢行列。
    pub fn model_matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(Quat::from_rotation_y(self.heading), self.pos)
    }

    pub fn update(&mut self, input: DriveInput, dt: f32) {
        let input = input.clamped();
        let s = self.spec;

        // ハンドルは瞬間には切れない。目標角へ一定の角速度で寄せる。
        let target = input.steer * s.max_steer;
        let max_delta = if s.steer_rate > 0.0 {
            s.max_steer * dt / s.steer_rate
        } else {
            f32::INFINITY
        };
        self.steer_angle += (target - self.steer_angle).clamp(-max_delta, max_delta);

        // 縦方向。前進のみを扱うので、抵抗は常に減速側に効かせればよく、
        // 0 で下限を切るだけでブレーキが後退に化けない。
        let mut accel = input.throttle * s.accel;
        if self.speed > 1e-4 {
            accel -= s.drag * self.speed * self.speed;
            accel -= input.brake * s.brake;
            if input.throttle < 1e-3 {
                accel -= s.rolling;
            }
        }
        self.speed = (self.speed + accel * dt).clamp(0.0, s.max_speed);

        // 自転車モデル: 進んだぶんだけ向きが変わる。止まっていれば回らない。
        // 右へ切る（steer_angle > 0）と上から見て時計まわりなので heading は減る。
        if self.speed > 1e-4 {
            let yaw_rate = -self.speed * self.steer_angle.tan() / s.wheelbase.max(1e-3);
            self.heading = wrap_angle(self.heading + yaw_rate * dt);
        }

        self.pos += self.forward() * self.speed * dt;
    }
}

/// -PI..PI に畳む。長時間走っても角度が発散しないように。
pub fn wrap_angle(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut x = (a + PI) % TAU;
    if x < 0.0 {
        x += TAU;
    }
    x - PI
}

/// 車体を追いかけるカメラ。距離と高さを保ちつつ、急な向きの変化を均す。
#[derive(Clone, Copy, Debug)]
pub struct ChaseCamera {
    pub distance: f32,
    pub height: f32,
    /// 車体のどれだけ前を見るか（m）。
    pub look_ahead: f32,
    /// 1 秒あたりどれだけ目標に寄るか（0..1 に近いほど機敏）。
    pub stiffness: f32,
    eye: Vec3,
    target: Vec3,
    initialized: bool,
}

impl Default for ChaseCamera {
    fn default() -> Self {
        Self {
            distance: 18.0,
            height: 7.5,
            look_ahead: 12.0,
            stiffness: 6.0,
            eye: Vec3::ZERO,
            target: Vec3::ZERO,
            initialized: false,
        }
    }
}

impl ChaseCamera {
    pub fn update(&mut self, truck: &Truck, dt: f32) {
        let fwd = truck.forward();
        let desired_eye = truck.pos - fwd * self.distance + Vec3::Y * self.height;
        let desired_target = truck.pos + fwd * self.look_ahead + Vec3::Y * 1.5;

        if !self.initialized {
            self.eye = desired_eye;
            self.target = desired_target;
            self.initialized = true;
            return;
        }

        // フレームレートに依らない指数追従。
        let t = 1.0 - (-self.stiffness * dt).exp();
        self.eye = self.eye.lerp(desired_eye, t);
        self.target = self.target.lerp(desired_target, t);
    }

    pub fn eye(&self) -> Vec3 {
        self.eye
    }

    pub fn target(&self) -> Vec3 {
        self.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn drive(truck: &mut Truck, input: DriveInput, seconds: f32) {
        for _ in 0..(seconds / DT) as usize {
            truck.update(input, DT);
        }
    }

    #[test]
    fn throttle_accelerates_forward() {
        let mut t = Truck::default();
        drive(
            &mut t,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            3.0,
        );
        assert!(t.speed > 5.0, "speed was {}", t.speed);
        assert!(t.pos.z > 5.0, "should have moved along +Z, got {}", t.pos.z);
    }

    #[test]
    fn drag_caps_the_top_speed() {
        let mut t = Truck::default();
        drive(
            &mut t,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            600.0,
        );
        assert!(t.speed <= t.spec.max_speed + 1e-3);
        // 抵抗が効いて最高速に張り付く前に頭打ちになる。
        assert!(t.speed > 10.0);
    }

    #[test]
    fn brake_stops_without_reversing() {
        let mut t = Truck::default();
        drive(
            &mut t,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            5.0,
        );
        assert!(t.speed > 1.0);
        drive(
            &mut t,
            DriveInput {
                brake: 1.0,
                ..Default::default()
            },
            10.0,
        );
        assert_eq!(t.speed, 0.0, "brake must settle at a stop");
    }

    #[test]
    fn coasting_slows_down() {
        let mut t = Truck::default();
        drive(
            &mut t,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            5.0,
        );
        let fast = t.speed;
        drive(&mut t, DriveInput::default(), 2.0);
        assert!(t.speed < fast);
    }

    #[test]
    fn steady_steering_drives_a_circle() {
        let mut t = Truck::default();
        drive(
            &mut t,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            6.0,
        );
        let start = t.pos;
        let input = DriveInput {
            throttle: 1.0,
            steer: 1.0,
            ..Default::default()
        };
        // 一周して出発点の近くへ戻る。
        let mut closest_after_half = f32::INFINITY;
        let mut turned = 0.0;
        let mut prev = t.heading;
        for i in 0..4000 {
            t.update(input, DT);
            let d = wrap_angle(t.heading - prev);
            turned += d;
            prev = t.heading;
            if turned.abs() > std::f32::consts::PI && i > 100 {
                closest_after_half = closest_after_half.min(t.pos.distance(start));
            }
            if turned.abs() > std::f32::consts::TAU {
                break;
            }
        }
        assert!(
            turned.abs() >= std::f32::consts::TAU - 0.2,
            "should complete a full turn, got {turned}"
        );
        assert!(
            closest_after_half < 40.0,
            "circle should come back near the start, closest was {closest_after_half}"
        );
    }

    /// 追従カメラは車体の後ろから前を見るので、右手系では画面の右が -X になる。
    /// 「右へ切ったら画面の右へ行く」ことをここで固定する。ここを取り違えると
    /// 操作が左右反転するが、絵を見ないと気づけない。
    #[test]
    fn steering_right_moves_toward_screen_right() {
        let mut t = Truck::default();
        drive(
            &mut t,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            4.0,
        );
        drive(
            &mut t,
            DriveInput {
                throttle: 1.0,
                steer: 1.0,
                ..Default::default()
            },
            2.0,
        );

        // 走り出しの向き（+Z）を見ているカメラにとっての画面右。
        let screen_right = Vec3::Z.cross(Vec3::Y).normalize();
        assert_eq!(screen_right, Vec3::NEG_X);
        assert!(
            t.pos.dot(screen_right) > 0.0,
            "steering right must move the truck to the right of the screen, got {}",
            t.pos
        );
    }

    #[test]
    fn stationary_truck_does_not_rotate() {
        let mut t = Truck::default();
        drive(
            &mut t,
            DriveInput {
                steer: 1.0,
                ..Default::default()
            },
            2.0,
        );
        assert_eq!(t.heading, 0.0, "a parked truck must not turn in place");
    }

    #[test]
    fn steering_is_rate_limited() {
        let mut t = Truck::default();
        t.update(
            DriveInput {
                steer: 1.0,
                ..Default::default()
            },
            DT,
        );
        assert!(
            t.steer_angle < t.spec.max_steer,
            "wheels should not snap to full lock in one frame"
        );
    }

    #[test]
    fn left_and_right_are_mirrored() {
        let go = DriveInput {
            throttle: 1.0,
            ..Default::default()
        };
        let mut left = Truck::default();
        let mut right = Truck::default();
        drive(&mut left, go, 4.0);
        drive(&mut right, go, 4.0);
        let l = DriveInput {
            throttle: 1.0,
            steer: -1.0,
            ..Default::default()
        };
        let r = DriveInput {
            throttle: 1.0,
            steer: 1.0,
            ..Default::default()
        };
        drive(&mut left, l, 3.0);
        drive(&mut right, r, 3.0);
        assert!((left.heading + right.heading).abs() < 1e-3);
        assert!((left.pos.x + right.pos.x).abs() < 1e-2);
    }

    #[test]
    fn heading_stays_wrapped() {
        let mut t = Truck::default();
        drive(
            &mut t,
            DriveInput {
                throttle: 1.0,
                steer: 1.0,
                ..Default::default()
            },
            300.0,
        );
        assert!(t.heading.abs() <= std::f32::consts::PI + 1e-4);
    }

    #[test]
    fn chase_camera_sits_behind_and_above() {
        let mut t = Truck::default();
        drive(
            &mut t,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            3.0,
        );
        let mut cam = ChaseCamera::default();
        cam.update(&t, DT);
        let behind = (cam.eye() - t.pos).dot(t.forward());
        assert!(behind < 0.0, "camera must be behind the truck");
        assert!(cam.eye().y > t.pos.y);
        assert!((cam.target() - t.pos).dot(t.forward()) > 0.0);
    }

    #[test]
    fn chase_camera_catches_up_and_is_framerate_independent() {
        let mut t = Truck::default();
        drive(
            &mut t,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            3.0,
        );

        let mut slow = ChaseCamera::default();
        let mut fast = ChaseCamera::default();
        slow.update(&t, DT);
        fast.update(&t, DT);
        // 同じ 1 秒を 30fps と 120fps で追う。
        for _ in 0..30 {
            slow.update(&t, 1.0 / 30.0);
        }
        for _ in 0..120 {
            fast.update(&t, 1.0 / 120.0);
        }
        assert!(
            slow.eye().distance(fast.eye()) < 0.5,
            "smoothing must not depend on frame rate"
        );
    }
}
