//! 音声の「いま鳴らすべき量」。実際の再生はシェル側。
//!
//! 共有コアは rodio/WebAudio に依存しない。代わりに毎フレームのレベルを出し、
//! Android は AudioTrack、iOS は AVAudioEngine、Web は Web Audio で鳴らす。

use crate::vehicle::{DriveInput, Truck};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct AudioLevels {
    /// エンジン。速度とスロットルから。0..1。
    pub engine: f32,
    /// 風切り音。速度の二乗に近い。0..1。
    pub wind: f32,
    /// ブレーキ鳴き。0..1。
    pub brake: f32,
}

impl AudioLevels {
    pub fn from_truck(truck: &Truck, input: DriveInput, muted: bool, master: f32) -> Self {
        if muted || master <= 0.0 {
            return Self::default();
        }
        let input = input.clamped();
        let top = truck.spec.max_speed.max(1.0);
        let speed_n = (truck.speed / top).clamp(0.0, 1.0);
        // アイドリングを残しつつ、スロットルで持ち上げる。
        let engine = (0.12 + 0.55 * speed_n + 0.33 * input.throttle).clamp(0.0, 1.0) * master;
        let wind = (speed_n * speed_n).clamp(0.0, 1.0) * master;
        let brake = if truck.speed > 1.0 {
            input.brake * master
        } else {
            0.0
        };
        Self {
            engine,
            wind,
            brake,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_truck_has_quiet_engine() {
        let t = Truck::default();
        let a = AudioLevels::from_truck(&t, DriveInput::default(), false, 1.0);
        assert!(a.engine > 0.0 && a.engine < 0.3);
        assert_eq!(a.wind, 0.0);
        assert_eq!(a.brake, 0.0);
    }

    #[test]
    fn throttle_and_speed_raise_engine() {
        let mut t = Truck::default();
        t.speed = 20.0;
        let a = AudioLevels::from_truck(
            &t,
            DriveInput {
                throttle: 1.0,
                ..Default::default()
            },
            false,
            1.0,
        );
        assert!(a.engine > 0.7);
        assert!(a.wind > 0.2);
    }

    #[test]
    fn mute_silences_everything() {
        let mut t = Truck::default();
        t.speed = 30.0;
        let a = AudioLevels::from_truck(
            &t,
            DriveInput {
                throttle: 1.0,
                brake: 1.0,
                ..Default::default()
            },
            true,
            1.0,
        );
        assert_eq!(a, AudioLevels::default());
    }
}
