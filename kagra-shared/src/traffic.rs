//! 経路に沿った交通 AI。チャンク窓と同期してスポーン／デスポーンする。

use crate::collide::{resolve_truck_vs_obb, Obb2};
use crate::road::RoadPath;
use crate::scene::FIXED_DT;
use crate::vehicle::{DriveInput, Truck, TruckSpec};
use glam::Vec3;

/// 同時に生きていてよい AI 車の上限（インスタンス予算）。
pub const MAX_TRAFFIC: usize = 8;
const LANE_OFFSETS: [f32; 2] = [-3.5, 3.5];

#[derive(Clone, Debug)]
pub struct TrafficCar {
    pub truck: Truck,
    pub path_s: f32,
    pub lane: f32,
    pub target_speed: f32,
    pub color: [u8; 4],
}

#[derive(Clone, Debug)]
pub struct TrafficSystem {
    pub cars: Vec<TrafficCar>,
    /// 次にスポーンを試みる弧長。テストでは大きくしてスポーンを止められる。
    pub next_spawn_s: f32,
}

impl Default for TrafficSystem {
    fn default() -> Self {
        Self {
            cars: Vec::new(),
            next_spawn_s: 40.0,
        }
    }
}

impl TrafficSystem {
    /// スポーンしない空の交通。最高速計測など用。
    pub fn disabled() -> Self {
        Self {
            cars: Vec::new(),
            next_spawn_s: 1.0e9,
        }
    }

    /// プレイヤー弧長まわりの窓で AI を更新する。
    pub fn update(
        &mut self,
        path: &RoadPath,
        player_s: f32,
        player: &Truck,
        keep_ahead: f32,
        keep_behind: f32,
    ) {
        self.despawn(player_s, keep_ahead, keep_behind);
        self.spawn(path, player_s, keep_ahead);
        self.drive(path, player_s, player);
    }

    fn despawn(&mut self, player_s: f32, keep_ahead: f32, keep_behind: f32) {
        let lo = player_s - keep_behind - 20.0;
        let hi = player_s + keep_ahead + 40.0;
        self.cars
            .retain(|c| c.path_s >= lo && c.path_s <= hi && c.path_s <= 1.0e6);
    }

    fn spawn(&mut self, path: &RoadPath, player_s: f32, keep_ahead: f32) {
        let end = path.length();
        while self.cars.len() < MAX_TRAFFIC && self.next_spawn_s < player_s + keep_ahead {
            let s = self.next_spawn_s;
            self.next_spawn_s += 55.0 + hash01(s as u32) * 40.0;
            if s <= player_s + 15.0 || s >= end - 10.0 {
                continue;
            }
            // 既に近い車がいればスキップ。
            if self.cars.iter().any(|c| (c.path_s - s).abs() < 25.0) {
                continue;
            }
            let lane = LANE_OFFSETS[(hash01((s * 3.0) as u32) * 2.0) as usize % 2];
            let frame = path.sample(s);
            let size = Vec3::new(2.2, 2.8, 8.0 + hash01((s * 7.0) as u32) * 4.0);
            let mut truck = Truck {
                spec: TruckSpec {
                    size,
                    accel: 3.5,
                    max_speed: 28.0,
                    allow_reverse: false,
                    ..TruckSpec::default()
                },
                pos: frame.pos + frame.right * lane,
                heading: frame.heading(),
                speed: 12.0 + hash01((s * 11.0) as u32) * 10.0,
                steer_angle: 0.0,
            };
            // レーン上に載せ直す。
            place_on_lane(&mut truck, path, s, lane);
            let hue = hash01((s * 13.0) as u32);
            let color = [
                (60.0 + hue * 160.0) as u8,
                (70.0 + (1.0 - hue) * 120.0) as u8,
                90,
                255,
            ];
            self.cars.push(TrafficCar {
                truck,
                path_s: s,
                lane,
                target_speed: 14.0 + hash01((s * 17.0) as u32) * 8.0,
                color,
            });
        }
        // プレイヤーが進んで窓が先に行ったらスポーン位置も進める。
        if self.next_spawn_s < player_s + 30.0 {
            self.next_spawn_s = player_s + 40.0;
        }
    }

    fn drive(&mut self, path: &RoadPath, player_s: f32, player: &Truck) {
        // 前車距離計算用に弧長でソートしたコピー。
        let mut order: Vec<(usize, f32)> = self
            .cars
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.path_s))
            .collect();
        order.sort_by(|a, b| a.1.total_cmp(&b.1));

        for i in 0..self.cars.len() {
            let my_s = self.cars[i].path_s;
            let lane = self.cars[i].lane;
            let target = self.cars[i].target_speed;

            // 同じレーンで前方にいる最も近い車 / プレイヤー。
            let mut gap = 80.0;
            let mut lead_speed = target;
            for &(j, s) in &order {
                if s <= my_s + 0.5 {
                    continue;
                }
                if (self.cars[j].lane - lane).abs() > 2.0 {
                    continue;
                }
                gap = s - my_s;
                lead_speed = self.cars[j].truck.speed;
                break;
            }
            // プレイヤーも障害物。
            if (player_s - my_s) > 0.5
                && (player_s - my_s) < gap
                && lateral_of(player, path).abs() - lane.abs() < 3.0
            {
                gap = player_s - my_s;
                lead_speed = player.speed;
            }

            let input = idm_input(IdmArgs {
                speed: self.cars[i].truck.speed,
                target,
                gap,
                lead_speed,
                path,
                s: my_s,
                lane,
                truck: &self.cars[i].truck,
            });
            self.cars[i].truck.update(input, FIXED_DT);
            // レーンにスナップしつつ path_s を更新。
            let nearest = path.nearest(self.cars[i].truck.pos);
            self.cars[i].path_s = nearest.distance;
            place_on_lane(&mut self.cars[i].truck, path, nearest.distance, lane);
        }
    }

    /// プレイヤーを AI 車から押し出す。
    pub fn resolve_player(&self, player: &mut Truck) -> f32 {
        let mut total = 0.0;
        for car in &self.cars {
            let obb = Obb2::from_truck(&car.truck);
            total += resolve_truck_vs_obb(player, &obb);
        }
        total
    }

    pub fn count(&self) -> usize {
        self.cars.len()
    }
}

fn place_on_lane(truck: &mut Truck, path: &RoadPath, s: f32, lane: f32) {
    let frame = path.sample(s);
    // 縦方向の自由は少し残し、横だけレーンに寄せる。
    let along = (truck.pos - frame.pos).dot(frame.tangent);
    let frame = path.sample((s + along).clamp(0.0, path.length()));
    truck.pos = frame.pos + frame.right * lane;
    // heading は急に回さない。
    let target = frame.heading();
    let mut d = target - truck.heading;
    while d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    while d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    truck.heading += d.clamp(-0.08, 0.08);
}

fn lateral_of(truck: &Truck, path: &RoadPath) -> f32 {
    let f = path.nearest(truck.pos);
    (truck.pos - f.pos).dot(f.right)
}

struct IdmArgs<'a> {
    speed: f32,
    target: f32,
    gap: f32,
    lead_speed: f32,
    path: &'a RoadPath,
    s: f32,
    lane: f32,
    truck: &'a Truck,
}

/// 簡易 IDM: 隙間が狭いとブレーキ、目標より遅ければアクセル、レーン逸脱でステア。
fn idm_input(a: IdmArgs<'_>) -> DriveInput {
    let desired_gap = 12.0 + a.speed * 0.8;
    let closing = (a.speed - a.lead_speed).max(0.0);
    let brake = if a.gap < desired_gap {
        ((desired_gap - a.gap) / desired_gap + closing * 0.05).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let throttle = if brake > 0.2 {
        0.0
    } else if a.speed < a.target {
        ((a.target - a.speed) / a.target).clamp(0.15, 1.0)
    } else {
        0.05
    };

    let frame = a.path.sample(a.s);
    let desired_pos = frame.pos + frame.right * a.lane;
    let err = desired_pos - a.truck.pos;
    let lat_err = err.dot(frame.right);
    // 右が正のステア。横ずれを戻す。
    let heading_err = {
        let mut d = frame.heading() - a.truck.heading;
        while d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        while d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        d
    };
    // heading の誤差: 正なら反時計に足りない → 左へ切る（steer 負）。
    // lat_err 正（右にいる）→ 左へ（steer 負）。
    let steer = (-heading_err * 1.8 - lat_err * 0.12).clamp(-1.0, 1.0);

    DriveInput {
        steer,
        throttle,
        brake,
    }
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
    fn traffic_stays_within_budget() {
        let streamer = RoadStreamer::default();
        let mut traffic = TrafficSystem::default();
        let player = Truck::default();
        for s in (0..500).map(|i| i as f32 * 2.0) {
            traffic.update(
                &streamer.path,
                s,
                &player,
                streamer.keep_ahead,
                streamer.keep_behind,
            );
            assert!(traffic.count() <= MAX_TRAFFIC);
        }
    }

    #[test]
    fn slow_lead_car_makes_follower_brake() {
        let path = RoadStreamer::default().path;
        let mut slow = Truck::default();
        place_on_lane(&mut slow, &path, 80.0, -3.5);
        slow.speed = 5.0;
        let mut fast = Truck::default();
        place_on_lane(&mut fast, &path, 50.0, -3.5);
        fast.speed = 20.0;

        let mut sys = TrafficSystem {
            cars: vec![
                TrafficCar {
                    truck: slow,
                    path_s: 80.0,
                    lane: -3.5,
                    target_speed: 5.0,
                    color: [1, 1, 1, 255],
                },
                TrafficCar {
                    truck: fast,
                    path_s: 50.0,
                    lane: -3.5,
                    target_speed: 22.0,
                    color: [1, 1, 1, 255],
                },
            ],
            next_spawn_s: 9999.0,
        };
        let player = Truck::default();
        for _ in 0..120 {
            sys.drive(&path, 0.0, &player);
        }
        let follower = sys.cars.iter().find(|c| c.path_s < 70.0).unwrap();
        assert!(
            follower.truck.speed < 15.0,
            "follower should slow, got {}",
            follower.truck.speed
        );
    }

    #[test]
    fn cars_despawn_behind_the_window() {
        let streamer = RoadStreamer::default();
        let mut traffic = TrafficSystem::default();
        let player = Truck::default();
        traffic.update(&streamer.path, 100.0, &player, 400.0, 80.0);
        let before = traffic.count();
        assert!(before > 0);
        traffic.update(&streamer.path, 800.0, &player, 400.0, 80.0);
        assert!(traffic.cars.iter().all(|c| c.path_s > 700.0));
    }
}
