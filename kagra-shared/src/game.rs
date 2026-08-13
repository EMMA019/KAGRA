//! デモゲーム「Corridor Haul」。タイトル → 配送 → 完了のループ。
//!
//! 運転そのものは `DrivingScene`、目標判定は `Mission`。ここはゲームとしての
//! 進行・タイマー・スコアだけを持つ。文言表示はシェル（HTML 等）側。

use crate::mission::{Mission, MissionPhase};
use crate::scene::FIXED_DT;
use serde::{Deserialize, Serialize};

pub const GAME_ID: &str = "corridor_haul";
pub const GAME_TITLE: &str = "Corridor Haul";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePhase {
    #[default]
    Title,
    Playing,
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DemoGame {
    pub phase: GamePhase,
    /// プレイ中の経過秒。
    pub time_s: f32,
    /// 完了時に確定したタイム。未完了は null。
    pub finish_time_s: Option<f32>,
    /// ローカル最良（セーブ経由で残す）。
    pub best_time_s: Option<f32>,
    /// 荷物を積んだか（pickup 到達後）。
    pub has_cargo: bool,
    pub score: u32,
}

impl Default for DemoGame {
    fn default() -> Self {
        Self {
            phase: GamePhase::Title,
            time_s: 0.0,
            finish_time_s: None,
            best_time_s: None,
            has_cargo: false,
            score: 0,
        }
    }
}

impl DemoGame {
    /// タイトルから配送を開始する。運転シーンは呼び出し側で `restart` する。
    pub fn start(&mut self) {
        let best = self.best_time_s;
        *self = Self {
            phase: GamePhase::Playing,
            best_time_s: best,
            ..Self::default()
        };
        self.phase = GamePhase::Playing;
    }

    pub fn is_driving(&self) -> bool {
        matches!(self.phase, GamePhase::Playing)
    }

    /// ミッション状態を見てタイマー／完了を進める。
    pub fn tick(&mut self, mission: &Mission) {
        match self.phase {
            GamePhase::Playing => {
                self.has_cargo = matches!(
                    mission.phase,
                    MissionPhase::ReachDropoff | MissionPhase::Complete
                );
                if mission.phase == MissionPhase::Complete {
                    self.finish(mission);
                } else {
                    self.time_s += FIXED_DT;
                }
            }
            GamePhase::Title | GamePhase::Complete => {}
        }
    }

    fn finish(&mut self, _mission: &Mission) {
        self.phase = GamePhase::Complete;
        self.finish_time_s = Some(self.time_s);
        self.score = score_for_time(self.time_s);
        match self.best_time_s {
            Some(b) if self.time_s < b => self.best_time_s = Some(self.time_s),
            None => self.best_time_s = Some(self.time_s),
            _ => {}
        }
    }

    pub fn objective_key(&self, mission: &Mission) -> &'static str {
        match self.phase {
            GamePhase::Title => "title",
            GamePhase::Complete => "complete",
            GamePhase::Playing => match mission.phase {
                MissionPhase::ReachPickup => "pickup",
                MissionPhase::ReachDropoff => "dropoff",
                MissionPhase::Complete => "complete",
            },
        }
    }
}

/// 速いほど高い。おおよそ 3 分以内でプラスが残る想定。
pub fn score_for_time(time_s: f32) -> u32 {
    let base = 12_000.0 - time_s * 35.0;
    base.max(500.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_enters_playing() {
        let mut g = DemoGame::default();
        assert_eq!(g.phase, GamePhase::Title);
        g.start();
        assert_eq!(g.phase, GamePhase::Playing);
        assert_eq!(g.time_s, 0.0);
    }

    #[test]
    fn completing_mission_finishes_run() {
        let mut g = DemoGame::default();
        g.start();
        let mut m = Mission::default();
        for _ in 0..60 {
            g.tick(&m);
        }
        assert!((g.time_s - 1.0).abs() < 1e-3);
        m.phase = MissionPhase::Complete;
        g.tick(&m);
        assert_eq!(g.phase, GamePhase::Complete);
        assert!(g.finish_time_s.is_some());
        assert!(g.score >= 500);
        assert_eq!(g.best_time_s, g.finish_time_s);
    }

    #[test]
    fn best_time_keeps_faster_run() {
        let mut g = DemoGame {
            best_time_s: Some(100.0),
            ..DemoGame::default()
        };
        g.start();
        g.time_s = 80.0;
        let m = Mission {
            phase: MissionPhase::Complete,
            ..Mission::default()
        };
        g.tick(&m);
        assert_eq!(g.best_time_s, Some(80.0));
    }
}
