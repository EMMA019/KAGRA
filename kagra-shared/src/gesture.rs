//! Touch gestures for action games: 4-way swipes recognised while the
//! finger is still moving, plus a short input buffer.
//!
//! A swipe fires on `Move` as soon as the finger has travelled
//! `min_dist` (a fraction of the short screen side) or is moving faster
//! than `min_speed` over at least half that distance. The anchor then
//! resets to the current point, so one touch can chain several swipes.
//! A touch that never travels far enough is a tap, which games may
//! ignore. Times are in seconds from any monotonic clock.

use crate::input::{PointerEvent, PointerPhase};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Swipe {
    Left,
    Right,
    Up,
    Down,
}

impl Swipe {
    pub fn name(self) -> &'static str {
        match self {
            Swipe::Left => "left",
            Swipe::Right => "right",
            Swipe::Up => "up",
            Swipe::Down => "down",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gesture {
    Swipe(Swipe),
    Tap,
}

#[derive(Clone, Copy, Debug)]
struct Track {
    x: f32,
    y: f32,
    t: f32,
    swiped: bool,
}

#[derive(Clone, Debug)]
pub struct SwipeRecognizer {
    /// Distance threshold as a fraction of the short screen side.
    pub min_dist: f32,
    /// Speed threshold in short-sides per second.
    pub min_speed: f32,
    tracks: HashMap<u32, Track>,
}

impl Default for SwipeRecognizer {
    fn default() -> Self {
        Self {
            min_dist: 0.06,
            min_speed: 1.2,
            tracks: HashMap::new(),
        }
    }
}

impl SwipeRecognizer {
    /// Any finger currently down.
    pub fn active(&self) -> bool {
        !self.tracks.is_empty()
    }

    /// Feed one pointer event. `w`/`h` are the surface size in the same
    /// units as the event coordinates (y grows downward).
    pub fn push(&mut self, ev: PointerEvent, w: u32, h: u32, t: f32) -> Option<Gesture> {
        let side = (w.min(h).max(1)) as f32;
        match ev.phase {
            PointerPhase::Begin => {
                self.tracks.insert(
                    ev.id,
                    Track {
                        x: ev.x,
                        y: ev.y,
                        t,
                        swiped: false,
                    },
                );
                None
            }
            PointerPhase::Move => {
                let tr = self.tracks.get_mut(&ev.id)?;
                let (dx, dy) = ((ev.x - tr.x) / side, (ev.y - tr.y) / side);
                let d = (dx * dx + dy * dy).sqrt();
                let dt = (t - tr.t).max(1e-3);
                let fast = d / dt >= self.min_speed && d >= self.min_dist * 0.5;
                if d < self.min_dist && !fast {
                    return None;
                }
                let dir = if dx.abs() > dy.abs() {
                    if dx > 0.0 {
                        Swipe::Right
                    } else {
                        Swipe::Left
                    }
                } else if dy > 0.0 {
                    Swipe::Down
                } else {
                    Swipe::Up
                };
                *tr = Track {
                    x: ev.x,
                    y: ev.y,
                    t,
                    swiped: true,
                };
                Some(Gesture::Swipe(dir))
            }
            PointerPhase::End => {
                let tr = self.tracks.remove(&ev.id)?;
                (!tr.swiped).then_some(Gesture::Tap)
            }
            PointerPhase::Cancel => {
                self.tracks.remove(&ev.id);
                None
            }
        }
    }
}

/// Remembers a press for `window` seconds so it still counts when the
/// game can act on it a little later (jump pressed just before landing).
#[derive(Clone, Debug)]
pub struct InputBuffer<A: Copy + PartialEq> {
    pub window: f32,
    queued: Vec<(A, f32)>,
}

impl<A: Copy + PartialEq> InputBuffer<A> {
    pub fn new(window: f32) -> Self {
        Self {
            window,
            queued: Vec::new(),
        }
    }

    pub fn press(&mut self, a: A, now: f32) {
        self.queued.retain(|(q, _)| *q != a);
        self.queued.push((a, now));
    }

    /// True (and consumed) if `a` was pressed within the window.
    pub fn take(&mut self, a: A, now: f32) -> bool {
        let w = self.window;
        self.queued.retain(|(_, t)| now - *t <= w);
        if let Some(i) = self.queued.iter().position(|(q, _)| *q == a) {
            self.queued.remove(i);
            return true;
        }
        false
    }

    pub fn clear(&mut self) {
        self.queued.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(phase: PointerPhase, x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            id: 1,
            x,
            y,
            phase,
            pressure: 0.5,
        }
    }

    #[test]
    fn swipes_fire_mid_move_and_chain() {
        let mut r = SwipeRecognizer::default();
        assert_eq!(
            r.push(ev(PointerPhase::Begin, 500.0, 500.0), 1000, 2000, 0.0),
            None
        );
        // 30 px of 1000 is under the distance threshold and slow.
        assert_eq!(
            r.push(ev(PointerPhase::Move, 530.0, 500.0), 1000, 2000, 0.2),
            None
        );
        assert_eq!(
            r.push(ev(PointerPhase::Move, 580.0, 500.0), 1000, 2000, 0.25),
            Some(Gesture::Swipe(Swipe::Right))
        );
        assert_eq!(
            r.push(ev(PointerPhase::Move, 580.0, 420.0), 1000, 2000, 0.3),
            Some(Gesture::Swipe(Swipe::Up))
        );
        assert_eq!(
            r.push(ev(PointerPhase::End, 580.0, 420.0), 1000, 2000, 0.35),
            None
        );
    }

    #[test]
    fn fast_flick_and_tap() {
        let mut r = SwipeRecognizer::default();
        r.push(ev(PointerPhase::Begin, 500.0, 500.0), 1000, 1000, 0.0);
        assert_eq!(
            r.push(ev(PointerPhase::Move, 500.0, 535.0), 1000, 1000, 0.016),
            Some(Gesture::Swipe(Swipe::Down))
        );
        r.push(ev(PointerPhase::Begin, 100.0, 100.0), 1000, 1000, 1.0);
        assert_eq!(
            r.push(ev(PointerPhase::End, 102.0, 101.0), 1000, 1000, 1.1),
            Some(Gesture::Tap)
        );
    }

    #[test]
    fn buffer_expires() {
        let mut b = InputBuffer::new(0.15);
        b.press(Swipe::Up, 1.0);
        assert!(b.take(Swipe::Up, 1.1));
        assert!(!b.take(Swipe::Up, 1.1));
        b.press(Swipe::Up, 2.0);
        assert!(!b.take(Swipe::Up, 2.2));
    }
}
