//! The end of a run and the words on the way there: the game-over cue that
//! waits for the freeze to finish, the banner a finished goal or pick-up
//! shows for a moment, and the best score on the HUD.
//!
//! The result card itself is the flow's (`GameFlow`, `tick_frame` finishes the
//! run after the hitstop); this holds only what a kit adds around it.

/// Run-time result state every kit carries on its [`Core`](super::Core).
#[derive(Clone, Debug, Default)]
pub struct ResultState {
    /// The crash has happened; the game-over cue waits for the freeze to end.
    pub gameover_pending: bool,
    /// The banner text and the seconds it has left.
    pub banner: String,
    pub banner_t: f32,
}

impl ResultState {
    /// A fresh run: no banner, nothing pending.
    pub fn reset(&mut self) {
        self.banner_t = 0.0;
        self.gameover_pending = false;
    }

    /// Show `text` for `secs`.
    pub fn announce(&mut self, text: String, secs: f32) {
        self.banner = text;
        self.banner_t = secs;
    }

    /// Count the banner down.
    pub fn tick(&mut self, dt: f32) {
        self.banner_t = (self.banner_t - dt).max(0.0);
    }

    /// The crash happened: the cue is owed once the freeze is over.
    pub fn crashed(&mut self) {
        self.gameover_pending = true;
    }

    /// True exactly once, when the cue is owed and the freeze is over.
    pub fn take_gameover(&mut self, hitstop: f32) -> bool {
        if self.gameover_pending && hitstop <= 0.0 {
            self.gameover_pending = false;
            true
        } else {
            false
        }
    }

    /// The BEST chip on the HUD: the saved best, or the live score once it passes it.
    pub fn best_shown(best: u32, score: u32) -> u32 {
        best.max(score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gameover_cue_fires_once_after_the_freeze() {
        let mut r = ResultState::default();
        assert!(!r.take_gameover(0.0));
        r.crashed();
        assert!(!r.take_gameover(0.2), "still frozen");
        assert!(r.take_gameover(0.0));
        assert!(!r.take_gameover(0.0), "once");
    }

    #[test]
    fn the_banner_counts_down_and_resets() {
        let mut r = ResultState::default();
        r.announce("MISSION  COINS".into(), 2.2);
        r.tick(1.0);
        assert!((r.banner_t - 1.2).abs() < 1e-6);
        r.tick(5.0);
        assert_eq!(r.banner_t, 0.0);
        r.announce("x".into(), 1.0);
        r.crashed();
        r.reset();
        assert!(r.banner_t == 0.0 && !r.gameover_pending);
    }

    #[test]
    fn best_follows_the_live_score_once_it_passes() {
        assert_eq!(ResultState::best_shown(500, 120), 500);
        assert_eq!(ResultState::best_shown(500, 900), 900);
    }
}
