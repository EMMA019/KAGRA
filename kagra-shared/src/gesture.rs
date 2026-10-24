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
    /// No lateral swipe from this touch before this time.
    cool_until: f32,
}

#[derive(Clone, Debug)]
pub struct SwipeRecognizer {
    /// Distance threshold as a fraction of the short screen side.
    pub min_dist: f32,
    /// Speed threshold in short-sides per second.
    pub min_speed: f32,
    /// After a lateral swipe, the same touch cannot swipe sideways again
    /// for this long (one swipe, one lane).
    pub cooldown: f32,
    tracks: HashMap<u32, Track>,
}

impl Default for SwipeRecognizer {
    fn default() -> Self {
        Self {
            min_dist: 0.06,
            min_speed: 1.2,
            cooldown: 0.0,
            tracks: HashMap::new(),
        }
    }
}

impl SwipeRecognizer {
    /// Forget every finger (retry, result screen, lost pointer capture).
    pub fn reset(&mut self) {
        self.tracks.clear();
    }

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
                        cool_until: f32::MIN,
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
                let lateral = dx.abs() > dy.abs();
                let cool = tr.cool_until;
                if lateral && t < cool {
                    // Travel during the cooldown does not bank toward the next lane.
                    *tr = Track {
                        x: ev.x,
                        y: ev.y,
                        t,
                        swiped: true,
                        cool_until: cool,
                    };
                    return None;
                }
                let dir = if lateral {
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
                let cool_until = if lateral { t + self.cooldown } else { cool };
                *tr = Track {
                    x: ev.x,
                    y: ev.y,
                    t,
                    swiped: true,
                    cool_until,
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

/// What the drag scheme reports for one pointer event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragEvent {
    /// First finger down.
    Start,
    /// Horizontal finger travel since `Start`, in short sides (+ is right).
    Drag(f32),
    /// A mostly vertical flick: `Up` or `Down`.
    Flick(Swipe),
    /// The dragging finger lifted.
    Release,
}

#[derive(Clone, Copy, Debug)]
struct DragTrack {
    x0: f32,
    // Flick anchor and last sample.
    ax: f32,
    ay: f32,
    lx: f32,
    ly: f32,
    lt: f32,
    armed: bool,
    up: bool,
    // Furthest point of the last flick, for re-arming on a reversal.
    ey: f32,
}

/// Hold-and-drag steering plus vertical flicks.
///
/// The first finger down steers: [`DragEvent::Drag`] carries its
/// horizontal travel, so the game maps it to lanes with its own
/// sensitivity. A flick fires once the finger has moved `flick_dist`
/// vertically within `flick_angle_deg` of vertical. It fires once per
/// stroke and re-arms only after the finger turns back or slows down.
#[derive(Clone, Debug)]
pub struct DragFlick {
    pub flick_dist: f32,
    pub flick_angle_deg: f32,
    /// Below this speed (short sides per second) a spent flick re-arms.
    pub rearm_speed: f32,
    primary: Option<u32>,
    tracks: HashMap<u32, DragTrack>,
}

impl Default for DragFlick {
    fn default() -> Self {
        Self {
            flick_dist: 0.07,
            flick_angle_deg: 30.0,
            rearm_speed: 0.25,
            primary: None,
            tracks: HashMap::new(),
        }
    }
}

/// A finger with no events for this long is treated as lifted.
pub const STALE_SECS: f32 = 1.5;

impl DragFlick {
    /// Forget every finger and the primary. A host that loses a pointerup
    /// (overlay on top, capture lost) must not leave a stale primary that
    /// blocks every later drag.
    pub fn reset(&mut self) {
        self.tracks.clear();
        self.primary = None;
    }

    pub fn active(&self) -> bool {
        !self.tracks.is_empty()
    }

    pub fn push(&mut self, ev: PointerEvent, w: u32, h: u32, t: f32) -> Vec<DragEvent> {
        let side = (w.min(h).max(1)) as f32;
        let mut out = Vec::new();
        match ev.phase {
            PointerPhase::Begin => {
                // Tracks silent for STALE_SECS lost their End somewhere.
                self.tracks.retain(|_, tr| t - tr.lt < STALE_SECS);
                self.tracks.insert(
                    ev.id,
                    DragTrack {
                        x0: ev.x,
                        ax: ev.x,
                        ay: ev.y,
                        lx: ev.x,
                        ly: ev.y,
                        lt: t,
                        armed: true,
                        up: false,
                        ey: ev.y,
                    },
                );
                // A primary whose track is gone, or a Begin reusing the
                // primary's id, means its End was lost: take over.
                let stale = match self.primary {
                    Some(p) => p == ev.id || !self.tracks.contains_key(&p),
                    None => true,
                };
                if stale {
                    self.primary = Some(ev.id);
                    out.push(DragEvent::Start);
                }
            }
            PointerPhase::Move => {
                let Some(tr) = self.tracks.get_mut(&ev.id) else {
                    return out;
                };
                let seg = ((ev.x - tr.lx).powi(2) + (ev.y - tr.ly).powi(2)).sqrt() / side;
                let speed = seg / (t - tr.lt).max(1.0 / 120.0);
                let (fdx, fdy) = ((ev.x - tr.ax) / side, (ev.y - tr.ay) / side);
                let tan = self.flick_angle_deg.to_radians().tan();
                if tr.armed {
                    if fdy.abs() >= self.flick_dist && fdx.abs() <= fdy.abs() * tan {
                        tr.armed = false;
                        tr.up = fdy < 0.0;
                        tr.ey = ev.y;
                        out.push(DragEvent::Flick(if tr.up {
                            Swipe::Up
                        } else {
                            Swipe::Down
                        }));
                    } else if fdx.abs() > self.flick_dist * 0.5 || speed < self.rearm_speed * 0.5 {
                        // Sideways drag or a resting finger: the next flick starts here.
                        tr.ax = ev.x;
                        tr.ay = ev.y;
                    }
                } else {
                    tr.ey = if tr.up {
                        tr.ey.min(ev.y)
                    } else {
                        tr.ey.max(ev.y)
                    };
                    let back = if tr.up { ev.y - tr.ey } else { tr.ey - ev.y } / side;
                    if back >= self.flick_dist * 0.3 || speed < self.rearm_speed {
                        tr.armed = true;
                        tr.ax = ev.x;
                        tr.ay = ev.y;
                    }
                }
                tr.lx = ev.x;
                tr.ly = ev.y;
                tr.lt = t;
                if self.primary == Some(ev.id) {
                    out.push(DragEvent::Drag((ev.x - tr.x0) / side));
                }
            }
            PointerPhase::End | PointerPhase::Cancel => {
                self.tracks.remove(&ev.id);
                if self.primary == Some(ev.id) {
                    self.primary = None;
                    out.push(DragEvent::Release);
                }
            }
        }
        out
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
    fn swipe_cooldown_gives_one_lane_per_swipe() {
        let mut r = SwipeRecognizer {
            cooldown: 0.3,
            ..Default::default()
        };
        r.push(ev(PointerPhase::Begin, 500.0, 500.0), 1000, 1000, 0.0);
        let mut n = 0;
        for i in 1..=10 {
            let x = 500.0 + i as f32 * 30.0;
            let g = r.push(
                ev(PointerPhase::Move, x, 500.0),
                1000,
                1000,
                i as f32 / 60.0,
            );
            n += matches!(g, Some(Gesture::Swipe(Swipe::Right))) as u32;
        }
        assert_eq!(n, 1);
    }

    #[test]
    fn flick_is_vertical_and_fires_once() {
        let mut r = DragFlick::default();
        r.push(ev(PointerPhase::Begin, 500.0, 500.0), 1000, 1000, 0.0);
        let mut flicks = 0;
        for i in 1..=8 {
            let y = 500.0 - i as f32 * 30.0;
            let out = r.push(
                ev(PointerPhase::Move, 500.0, y),
                1000,
                1000,
                i as f32 / 60.0,
            );
            flicks += out
                .iter()
                .filter(|e| matches!(e, DragEvent::Flick(Swipe::Up)))
                .count();
        }
        assert_eq!(flicks, 1, "one flick per stroke");
        // 45 degrees is not a flick.
        let mut r = DragFlick::default();
        r.push(ev(PointerPhase::Begin, 500.0, 500.0), 1000, 1000, 0.0);
        for i in 1..=8 {
            let d = i as f32 * 20.0;
            let out = r.push(
                ev(PointerPhase::Move, 500.0 + d, 500.0 - d),
                1000,
                1000,
                i as f32 / 60.0,
            );
            assert!(!out.iter().any(|e| matches!(e, DragEvent::Flick(_))));
        }
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
