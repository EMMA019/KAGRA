//! Score, pickups, death reason, and the in-play score panel.

use crate::scene::{DrawList, Quad, TextAlign, TextQuad};

#[derive(Debug)]
pub struct RunStats {
    pub score: u32,
    pub score_acc: f32,
    /// Pickups collected (rings / coins).
    pub rings: u32,
    pub alive: bool,
    /// Entity name after a hit. Empty while alive.
    pub death: String,
    /// Per-frame events for audio / feel.
    pub just_rings: u32,
    pub just_crash: bool,
    pub just_jump: bool,
}

impl Default for RunStats {
    fn default() -> Self {
        Self {
            score: 0,
            score_acc: 0.0,
            rings: 0,
            alive: true,
            death: String::new(),
            just_rings: 0,
            just_crash: false,
            just_jump: false,
        }
    }
}

impl RunStats {
    pub fn clear_events(&mut self) {
        self.just_rings = 0;
        self.just_crash = false;
        self.just_jump = false;
    }

    /// Fractional points (distance) roll into whole score.
    pub fn add_points(&mut self, points: f32) {
        self.score_acc += points;
        let whole = self.score_acc.floor();
        if whole >= 1.0 {
            self.score += whole as u32;
            self.score_acc -= whole;
        }
    }

    pub fn pickup(&mut self, score: u32) {
        self.rings += 1;
        self.just_rings += 1;
        self.score += score;
    }

    /// Returns false when already dead.
    pub fn die(&mut self, reason: &str) -> bool {
        if !self.alive {
            return false;
        }
        self.alive = false;
        self.death = reason.to_string();
        self.just_crash = true;
        true
    }

    /// The in-play `SCORE n` panel at `top`.
    pub fn paint_score(&self, list: &mut DrawList, top: f32) {
        list.quads
            .push(Quad::new(12.0, top, 168.0, 36.0, [8, 4, 20, 160]));
        list.texts.push(
            TextQuad::new(
                &format!("SCORE {}", self.score),
                22.0,
                top + 6.0,
                16.0,
                [180, 245, 255, 255],
            )
            .aligned(TextAlign::Left),
        );
    }
}
