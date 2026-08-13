//! 配送ミッション。判定は経路弧長 `path_s` 基準。

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionPhase {
    /// ピックアップ地点へ向かう。
    ReachPickup,
    /// ドロップオフ地点へ向かう。
    ReachDropoff,
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Mission {
    pub phase: MissionPhase,
    pub pickup_s: f32,
    pub dropoff_s: f32,
    pub radius: f32,
}

impl Default for Mission {
    fn default() -> Self {
        Self {
            phase: MissionPhase::ReachPickup,
            // デモルート上のわかりやすい地点。
            pickup_s: 280.0,
            dropoff_s: 720.0,
            radius: 18.0,
        }
    }
}

impl Mission {
    /// ルート長に合わせたピック／ドロップ位置。
    pub fn for_route_length(len: f32) -> Self {
        let len = len.max(40.0);
        Self {
            phase: MissionPhase::ReachPickup,
            pickup_s: (len * 0.35).clamp(20.0, len - 20.0),
            dropoff_s: (len * 0.88).clamp(30.0, len - 5.0),
            radius: 18.0,
        }
    }

    pub fn update(&mut self, path_s: f32) {
        match self.phase {
            MissionPhase::ReachPickup => {
                if (path_s - self.pickup_s).abs() <= self.radius {
                    self.phase = MissionPhase::ReachDropoff;
                }
            }
            MissionPhase::ReachDropoff => {
                if (path_s - self.dropoff_s).abs() <= self.radius {
                    self.phase = MissionPhase::Complete;
                }
            }
            MissionPhase::Complete => {}
        }
    }

    /// いま目指している目標の弧長。完了後は dropoff。
    pub fn target_s(&self) -> f32 {
        match self.phase {
            MissionPhase::ReachPickup => self.pickup_s,
            MissionPhase::ReachDropoff | MissionPhase::Complete => self.dropoff_s,
        }
    }

    pub fn progress_along_route(&self, path_s: f32) -> f32 {
        match self.phase {
            MissionPhase::ReachPickup => (path_s / self.pickup_s.max(1.0)).clamp(0.0, 1.0) * 0.5,
            MissionPhase::ReachDropoff => {
                let span = (self.dropoff_s - self.pickup_s).max(1.0);
                0.5 + ((path_s - self.pickup_s) / span).clamp(0.0, 1.0) * 0.5
            }
            MissionPhase::Complete => 1.0,
        }
    }

    pub fn label(&self) -> &'static str {
        match self.phase {
            MissionPhase::ReachPickup => "pickup",
            MissionPhase::ReachDropoff => "dropoff",
            MissionPhase::Complete => "complete",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advances_through_phases() {
        let mut m = Mission::default();
        assert_eq!(m.phase, MissionPhase::ReachPickup);
        m.update(100.0);
        assert_eq!(m.phase, MissionPhase::ReachPickup);
        m.update(280.0);
        assert_eq!(m.phase, MissionPhase::ReachDropoff);
        m.update(500.0);
        assert_eq!(m.phase, MissionPhase::ReachDropoff);
        m.update(720.0);
        assert_eq!(m.phase, MissionPhase::Complete);
        m.update(900.0);
        assert_eq!(m.phase, MissionPhase::Complete);
    }

    #[test]
    fn progress_is_monotonic() {
        let m = Mission {
            phase: MissionPhase::ReachDropoff,
            ..Mission::default()
        };
        let a = m.progress_along_route(300.0);
        let b = m.progress_along_route(500.0);
        assert!(b > a);
        assert!(a >= 0.5);
    }
}
