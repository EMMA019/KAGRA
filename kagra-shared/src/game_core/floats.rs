//! Words and numbers that rise from a spot in the world and fade: damage, heals, "+100",
//! "MISS". The kit pushes them where something happened; the HUD draws them through the
//! camera ([`View::project`]), so they stay over the thing on any screen.

use super::tween::Ease;
use super::view::View;
use crate::scene::{DrawList, TextAlign, TextFace, TextQuad};

/// One popup: where (world), what, colour, seconds to wait, seconds left.
#[derive(Clone, Debug, PartialEq)]
pub struct Float {
    pub at: [f32; 3],
    pub text: String,
    pub color: [u8; 4],
    pub delay: f32,
    pub life: f32,
}

/// How popups move and look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloatStyle {
    /// Seconds each one lives.
    pub secs: f32,
    /// World units it rises over its life.
    pub rise: f32,
    /// Text size in HUD pixels before the HUD scale.
    pub size: f32,
    /// The pop at the start: the size grows from `1 - pop` with an overshoot.
    pub pop: f32,
}

impl Default for FloatStyle {
    fn default() -> Self {
        Self {
            secs: 1.0,
            rise: 0.9,
            size: 22.0,
            pop: 0.35,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Floats {
    pub items: Vec<Float>,
    pub style: FloatStyle,
}

impl Floats {
    /// At most this many at once (the oldest go first).
    pub const MAX: usize = 32;

    pub fn new(style: FloatStyle) -> Self {
        Self {
            items: Vec::new(),
            style,
        }
    }

    /// A popup at `at` after `delay` seconds (to line up with a strike landing).
    pub fn push(&mut self, at: [f32; 3], text: impl Into<String>, color: [u8; 4], delay: f32) {
        if self.items.len() >= Self::MAX {
            self.items.remove(0);
        }
        self.items.push(Float {
            at,
            text: text.into(),
            color,
            delay: delay.max(0.0),
            life: self.style.secs,
        });
    }

    pub fn tick(&mut self, dt: f32) {
        for f in &mut self.items {
            if f.delay > 0.0 {
                f.delay -= dt;
            } else {
                f.life -= dt;
            }
        }
        self.items.retain(|f| f.life > 0.0);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Draw every shown popup through `view` on a `w` x `h` canvas at HUD scale `s`.
    pub fn draw(&self, list: &mut DrawList, view: &View, w: f32, h: f32, s: f32) {
        let st = self.style;
        for f in &self.items {
            if f.delay > 0.0 {
                continue;
            }
            let age = (1.0 - f.life / st.secs).clamp(0.0, 1.0);
            let rise = Ease::OutCubic.apply(age) * st.rise;
            let Some((sx, sy)) = view.project([f.at[0], f.at[1] + rise, f.at[2]], w, h) else {
                continue;
            };
            let grow = Ease::OutBack.apply((age * 5.0).min(1.0));
            let size = st.size * s * (1.0 - st.pop + st.pop * grow);
            // Full until the last third, then fade.
            let a = (f.life / (st.secs / 3.0)).clamp(0.0, 1.0);
            let c = f.color;
            list.texts.push(
                TextQuad::new(
                    &f.text,
                    sx,
                    sy,
                    size,
                    [c[0], c[1], c[2], (c[3] as f32 * a) as u8],
                )
                .aligned(TextAlign::Center)
                .with_face(TextFace::Display),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popups_wait_rise_fade_and_go() {
        let mut f = Floats::default();
        f.push([0.0, 1.0, 0.0], "12", [255, 80, 80, 255], 0.5);
        f.push([0.0, 1.0, 0.0], "MISS", [255, 255, 255, 255], 0.0);
        let view = View::frame(
            [0.0; 3],
            [0.0, 1.0, 1.0],
            50.0,
            &View::ground_box(-2.0, 2.0, -2.0, 2.0, 0.0),
            0.5,
        );
        let mut list = DrawList::default();
        f.draw(&mut list, &view, 400.0, 800.0, 1.0);
        assert_eq!(list.texts.len(), 1, "the delayed one waits");
        f.tick(0.6);
        f.tick(0.6);
        assert_eq!(f.items.len(), 1, "the first is gone after its second");
        f.tick(1.0);
        assert!(f.is_empty());
        for i in 0..40 {
            f.push([0.0; 3], format!("{i}"), [255; 4], 0.0);
        }
        assert_eq!(f.items.len(), Floats::MAX);
        assert_eq!(f.items[0].text, "8");
    }
}
