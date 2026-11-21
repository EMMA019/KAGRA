//! A floating thumb stick any kit can use: where its base lands, how the base follows a thumb
//! past the rim, the analog push with a dead zone, and its look (base, rim, a jelly neck and a
//! soft knob). Discs and rings are rounded quads (the GPU draws the edge). The jelly neck is
//! the hull of the base and the knob, filled one pixel row at a time with a part-covered quad
//! at each row end, so nothing in it is drawn twice. First used by Moonlit Swarm.

use crate::scene::{DrawList, Quad};

/// Where the base lands for a thumb at (`x`, `y`): there, but kept `rad + margin` off every edge.
pub fn land(x: f32, y: f32, rad: f32, margin: f32, w: f32, h: f32) -> (f32, f32) {
    let m = rad + margin;
    (x.clamp(m, (w - m).max(m)), y.clamp(m, (h - m).max(m)))
}

/// The base after the thumb moved to `finger`: past the rim it is dragged along so the thumb
/// sits on the rim (turning back is instant), inside it stays put.
pub fn follow(base: (f32, f32), finger: (f32, f32), rad: f32) -> (f32, f32) {
    let (dx, dy) = (finger.0 - base.0, finger.1 - base.1);
    let d = dx.hypot(dy);
    if d <= rad {
        return base;
    }
    let k = (d - rad) / d;
    (base.0 + dx * k, base.1 + dy * k)
}

/// Where the knob is drawn: the thumb, clamped to the rim.
pub fn knob(base: (f32, f32), finger: (f32, f32), rad: f32) -> (f32, f32) {
    let (dx, dy) = (finger.0 - base.0, finger.1 - base.1);
    let d = dx.hypot(dy).max(1e-3);
    let k = d.min(rad) / d;
    (base.0 + dx * k, base.1 + dy * k)
}

/// The push, -1..=1 on each axis (screen x right, y down): nothing inside `dead` (a fraction
/// of the radius), then a smooth ramp to full at the rim.
pub fn analog(base: (f32, f32), finger: (f32, f32), rad: f32, dead: f32) -> (f32, f32) {
    let (dx, dy) = ((finger.0 - base.0) / rad, (finger.1 - base.1) / rad);
    let len = dx.hypot(dy);
    if len <= dead {
        return (0.0, 0.0);
    }
    let m = ((len.min(1.0) - dead) / (1.0 - dead)).clamp(0.0, 1.0);
    (dx / len * m, dy / len * m)
}

/// Colours of the stick (RGBA). [`StickLook::default`] is Moonlit Swarm's dark violet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickLook {
    pub base: [u8; 4],
    pub rim: [u8; 4],
    pub neck: [u8; 4],
    pub knob: [u8; 4],
    pub glint: [u8; 4],
    /// The resting hint: base, rim, inner dot.
    pub rest: [[u8; 4]; 3],
}

impl Default for StickLook {
    fn default() -> Self {
        Self {
            base: [10, 6, 22, 120],
            rim: [160, 125, 255, 150],
            neck: [110, 80, 210, 120],
            knob: [205, 185, 255, 215],
            glint: [255, 255, 255, 110],
            rest: [[10, 6, 22, 60], [150, 120, 230, 70], [150, 120, 230, 55]],
        }
    }
}

/// One filled row from `l` to `r` at `y`, its two ends faded by how much of the end pixel is
/// inside.
fn span(list: &mut DrawList, y: f32, l: f32, r: f32, c: [u8; 4]) {
    if r - l < 0.5 {
        return;
    }
    let (li, ri) = (l.ceil(), r.floor());
    if ri > li {
        list.quads.push(Quad::new(li, y, ri - li, 1.0, c));
    }
    let edge = |cov: f32| [c[0], c[1], c[2], (c[3] as f32 * cov.clamp(0.0, 1.0)) as u8];
    if li - l > 0.02 {
        list.quads
            .push(Quad::new(li - 1.0, y, 1.0, 1.0, edge(li - l)));
    }
    if r - ri > 0.02 {
        list.quads.push(Quad::new(ri, y, 1.0, 1.0, edge(r - ri)));
    }
}

/// The left and right edge of a circle on the row through `y`, if the row crosses it.
fn chord(cx: f32, cy: f32, rad: f32, y: f32) -> Option<(f32, f32)> {
    let dy = y - cy;
    (dy.abs() < rad).then(|| {
        let half = (rad * rad - dy * dy).sqrt();
        (cx - half, cx + half)
    })
}

/// A filled circle (the GPU rounds it: `Quad::circle`).
pub fn smooth_disc(list: &mut DrawList, cx: f32, cy: f32, rad: f32, c: [u8; 4]) {
    list.quads.push(Quad::circle(cx, cy, rad, c));
}

/// A ring from `rad - width` to `rad`.
pub fn smooth_ring(list: &mut DrawList, cx: f32, cy: f32, rad: f32, width: f32, c: [u8; 4]) {
    list.quads.push(Quad::ring(cx, cy, rad, width, c));
}

/// The convex hull of a chain of circles, one span per row (left-most to right-most edge of
/// any circle the row crosses). Exact for a chain that tapers evenly between two circles.
fn blob(list: &mut DrawList, circles: &[(f32, f32, f32)], c: [u8; 4]) {
    let top = circles
        .iter()
        .map(|&(_, y, r)| y - r)
        .fold(f32::MAX, f32::min);
    let bottom = circles
        .iter()
        .map(|&(_, y, r)| y + r)
        .fold(f32::MIN, f32::max);
    let mut y = top.floor();
    while y < bottom {
        let mid = y + 0.5;
        let (mut l, mut r) = (f32::MAX, f32::MIN);
        for &(cx, cy, rad) in circles {
            if let Some((a, b)) = chord(cx, cy, rad, mid) {
                l = l.min(a);
                r = r.max(b);
            }
        }
        if l < r {
            span(list, y, l, r, c);
        }
        y += 1.0;
    }
}

/// Draw the stick: base at `b` with radius `rad`, knob at `k` (use [`knob`]); `s` is the HUD
/// scale. `live` false draws the resting hint where a thumb would land.
pub fn draw(
    list: &mut DrawList,
    b: (f32, f32),
    k: (f32, f32),
    rad: f32,
    s: f32,
    live: bool,
    look: &StickLook,
) {
    let (bx, by) = b;
    if !live {
        smooth_disc(list, bx, by, rad, look.rest[0]);
        smooth_ring(list, bx, by, rad, 2.0 * s, look.rest[1]);
        smooth_disc(list, bx, by, rad * 0.4, look.rest[2]);
        return;
    }
    let (kx, ky) = k;
    smooth_disc(list, bx, by, rad, look.base);
    smooth_ring(list, bx, by, rad, 2.5 * s, look.rim);
    // The jelly: thick at the base, thinner as it stretches, as wide as the knob at the end.
    let stretch = ((kx - bx).hypot(ky - by) / rad).min(1.0);
    let (r0, r1) = (rad * (0.44 - 0.16 * stretch), rad * 0.42);
    let n = 8;
    let chain: Vec<(f32, f32, f32)> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            (bx + (kx - bx) * t, by + (ky - by) * t, r0 + (r1 - r0) * t)
        })
        .collect();
    blob(list, &chain, look.neck);
    smooth_disc(list, kx, ky, rad * 0.42, look.knob);
    // A glint up and to the left, so the knob reads as a soft ball.
    smooth_disc(
        list,
        kx - rad * 0.13,
        ky - rad * 0.14,
        rad * 0.13,
        look.glint,
    );
}

// --------------------------------------------------------------- buttons ----

/// An action button a kit draws itself (the kit's drawn buttons are the source
/// of truth on a phone: some host channels never deliver the second button).
/// The ultimate is a button of its own: a kit gates the press on its gauge,
/// so a tap below a full meter is spent, not saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionId {
    Melee,
    Ranged,
    Dodge,
    Ult,
    /// A second melee button beside [`ActionId::Melee`]: the
    /// diagonal cut to Melee's wide one (city_action's motion
    /// round). Same latch, same buffer — a kit with one melee
    /// button simply never wires it.
    MeleeDiag,
}

/// How long a press is remembered when it cannot fire at once (the input
/// buffer). About 11 fixed steps at 60 Hz: long enough to feel responsive,
/// short enough that a stale press never fires late.
pub const BUFFER_SECS: f32 = 0.18;
/// The melee combo's reservation window: a touch longer, matched to the
/// swing lengths, so an early next-stage press still chains.
pub const COMBO_BUFFER_SECS: f32 = 0.24;

/// One button's state. The tap **latch** and the input **buffer** are the same
/// record seen two ways: a press is latched the moment it lands (a tap shorter
/// than one fixed step still counts), and it stays buffered until the kit can
/// fire it or its window runs out. Releasing the finger never clears it; only
/// [`ActionButtons::clear`] (pause, cards, title) does.
#[derive(Clone, Debug, Default, PartialEq)]
struct ButtonSlot {
    /// The touch finger holding it, if any.
    touch_finger: Option<u32>,
    /// The host channel (keys / gamepad / page button) holding it.
    host_held: bool,
    /// A latched press: the step it landed on and its window in steps.
    press: Option<(u64, u64)>,
    /// The step the most recent press landed on, whether it is still
    /// waiting or already fired: a kit can pair a fresh press against
    /// one it consumed a step ago (a charge being held, say).
    last_latch: Option<u64>,
    /// QA counters: presses landed, presses fired, presses that expired.
    presses: u32,
    consumed: u32,
    expired: u32,
}

/// The kit's action buttons: held state per finger, a latch per press, and a
/// short buffer per action. The kit calls [`ActionButtons::advance`] once per
/// fixed step, then asks [`ActionButtons::take_press`] for each action it can
/// start. Because the step counter freezes during hitstop, a tap landed and
/// released inside a hitstop is still read on the first step after it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionButtons {
    slots: [ButtonSlot; 5],
    step: u64,
}

impl ActionButtons {
    pub fn new() -> Self {
        Self::default()
    }

    fn slot(&mut self, id: ActionId) -> &mut ButtonSlot {
        &mut self.slots[id as usize]
    }

    fn get(&self, id: ActionId) -> &ButtonSlot {
        &self.slots[id as usize]
    }

    fn latch(&mut self, id: ActionId, window_secs: f32) {
        let steps = (window_secs.max(0.0) * 60.0).round() as u64;
        let now = self.step;
        let slot = self.slot(id);
        slot.press = Some((now, steps));
        slot.last_latch = Some(now);
        slot.presses += 1;
    }

    /// A touch landed on the button: hold it with this finger and latch the
    /// press. A second finger on an already-held button re-latches the press
    /// but does not steal the hold.
    pub fn press_touch(&mut self, id: ActionId, finger: u32, window_secs: f32) {
        if self.get(id).touch_finger.is_none() {
            self.slot(id).touch_finger = Some(finger);
        }
        self.latch(id, window_secs);
    }

    /// A touch left the button. The latched press survives; only the hold ends.
    pub fn release_touch(&mut self, id: ActionId, finger: u32) {
        if self.get(id).touch_finger == Some(finger) {
            self.slot(id).touch_finger = None;
        }
    }

    /// The host channel's level for this button. A rising edge latches a press
    /// (with the standard buffer window); the level itself is the hold.
    pub fn set_host(&mut self, id: ActionId, down: bool) {
        if down && !self.get(id).host_held {
            self.latch(id, BUFFER_SECS);
        }
        self.slot(id).host_held = down;
    }

    /// Is the button held right now, by a finger or by the host channel?
    pub fn held(&self, id: ActionId) -> bool {
        let s = self.get(id);
        s.touch_finger.is_some() || s.host_held
    }

    /// The touch finger holding this button, if any.
    pub fn touch_finger(&self, id: ActionId) -> Option<u32> {
        self.get(id).touch_finger
    }

    /// Is a latched press waiting to be fired?
    pub fn pending(&self, id: ActionId) -> bool {
        self.get(id).press.is_some()
    }

    /// The step the waiting press landed on, if one is waiting. A
    /// kit that pairs two landings (a follow-up press against one
    /// already consumed) reads the pair from here and
    /// [`ActionButtons::last_press_step`].
    pub fn press_step(&self, id: ActionId) -> Option<u64> {
        self.get(id).press.map(|(landed, _)| landed)
    }

    /// The step the most recent press landed on, waiting or already
    /// fired: a follow-up press may pair against a landing the kit
    /// already consumed (the press that started a held charge, say),
    /// and its landing is still the one to pair against.
    pub fn last_press_step(&self, id: ActionId) -> Option<u64> {
        self.get(id).last_latch
    }

    /// The current fixed step (the counter [`ActionButtons::advance`]
    /// climbs, frozen while the sim is frozen).
    pub fn step_now(&self) -> u64 {
        self.step
    }

    /// Fire the latched press, if one is waiting. Each press fires once.
    pub fn take_press(&mut self, id: ActionId) -> bool {
        let slot = self.slot(id);
        if slot.press.take().is_some() {
            slot.consumed += 1;
            true
        } else {
            false
        }
    }

    /// Drop the latched press without firing it: the kit saw the press
    /// and chose not to use it (a press that landed before the window it
    /// could have fed, swept as the window opens). It counts as expired,
    /// so the QA counters always add up — every press fired, expired or
    /// is still waiting.
    pub fn drop_press(&mut self, id: ActionId) -> bool {
        let slot = self.slot(id);
        if slot.press.take().is_some() {
            slot.expired += 1;
            true
        } else {
            false
        }
    }

    /// Advance one fixed step, expiring presses whose window has run out.
    pub fn advance(&mut self) {
        self.step += 1;
        for slot in self.slots.iter_mut() {
            if let Some((landed, window)) = slot.press {
                if self.step - landed > window {
                    slot.press = None;
                    slot.expired += 1;
                }
            }
        }
    }

    /// Drop every hold, latch and buffer (pause, level-up cards, title,
    /// game over). The QA counters keep their totals.
    pub fn clear(&mut self) {
        for slot in self.slots.iter_mut() {
            slot.touch_finger = None;
            slot.host_held = false;
            slot.press = None;
        }
    }

    /// QA counters for one button: (presses, fired, expired).
    pub fn counts(&self, id: ActionId) -> (u32, u32, u32) {
        let s = self.get(id);
        (s.presses, s.consumed, s.expired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(list: &DrawList, x: f32, y: f32) -> u32 {
        list.quads
            .iter()
            .filter(|q| q.contains(x, y))
            .map(|q| q.color[3] as u32)
            .sum()
    }

    #[test]
    fn the_base_lands_on_screen_follows_past_the_rim_and_pushes_analog() {
        assert_eq!(land(5.0, 900.0, 60.0, 4.0, 390.0, 844.0), (64.0, 780.0));
        let base = (100.0, 500.0);
        assert_eq!(
            follow(base, (130.0, 500.0), 60.0),
            base,
            "inside the rim it stays"
        );
        assert_eq!(
            follow(base, (280.0, 500.0), 60.0),
            (220.0, 500.0),
            "past it, dragged along"
        );
        assert_eq!(knob(base, (280.0, 500.0), 60.0), (160.0, 500.0));
        assert_eq!(
            analog(base, (103.0, 500.0), 60.0, 0.12),
            (0.0, 0.0),
            "dead zone"
        );
        let (x, y) = analog(base, (160.0, 500.0), 60.0, 0.12);
        assert!((x - 1.0).abs() < 1e-5 && y == 0.0, "full at the rim");
        let (half, _) = analog(base, (130.0, 500.0), 60.0, 0.12);
        assert!(half > 0.3 && half < 0.6, "a ramp: {half}");
    }

    #[test]
    fn discs_and_rings_are_single_rounded_quads() {
        let mut l = DrawList::default();
        smooth_disc(&mut l, 100.0, 100.0, 40.0, [255, 255, 255, 200]);
        smooth_ring(&mut l, 100.0, 100.0, 40.0, 3.0, [255, 255, 255, 200]);
        assert_eq!(l.quads.len(), 2);
        let (d, r) = (l.quads[0], l.quads[1]);
        assert_eq!(
            (d.x, d.y, d.w, d.radius, d.stroke),
            (60.0, 60.0, 80.0, 40.0, 0.0)
        );
        assert_eq!((r.radius, r.stroke), (40.0, 3.0));
    }

    #[test]
    fn a_tap_shorter_than_a_step_still_fires_exactly_once() {
        let mut b = ActionButtons::new();
        // Down and up again before any step runs (a sub-frame tap).
        b.press_touch(ActionId::Melee, 7, BUFFER_SECS);
        b.release_touch(ActionId::Melee, 7);
        assert!(!b.held(ActionId::Melee));
        b.advance();
        assert!(b.take_press(ActionId::Melee), "the latch kept the tap");
        assert!(!b.take_press(ActionId::Melee), "and only once");
        assert_eq!(b.counts(ActionId::Melee), (1, 1, 0));
    }

    #[test]
    fn an_unfired_press_expires_after_its_window() {
        let mut b = ActionButtons::new();
        b.press_touch(ActionId::Ranged, 3, BUFFER_SECS);
        for _ in 0..10 {
            b.advance();
        }
        assert!(b.pending(ActionId::Ranged), "0.18 s is 11 steps");
        b.advance();
        b.advance();
        assert!(!b.pending(ActionId::Ranged), "then it is gone");
        assert!(!b.take_press(ActionId::Ranged), "a stale press never fires");
        assert_eq!(b.counts(ActionId::Ranged), (1, 0, 1));
    }

    #[test]
    fn a_dropped_press_counts_as_expired_and_never_fires() {
        let mut b = ActionButtons::new();
        b.press_touch(ActionId::Ranged, 3, BUFFER_SECS);
        assert!(b.drop_press(ActionId::Ranged), "a waiting press drops");
        assert!(!b.pending(ActionId::Ranged));
        assert!(
            !b.take_press(ActionId::Ranged),
            "a dropped press never fires"
        );
        assert!(!b.drop_press(ActionId::Ranged), "nothing left to drop");
        assert_eq!(b.counts(ActionId::Ranged), (1, 0, 1));
        // A press after the drop is a fresh press and fires as usual.
        b.press_touch(ActionId::Ranged, 4, BUFFER_SECS);
        assert!(b.take_press(ActionId::Ranged));
        assert_eq!(b.counts(ActionId::Ranged), (2, 1, 1));
    }

    #[test]
    fn the_combo_window_outlives_the_plain_buffer() {
        let mut b = ActionButtons::new();
        b.press_touch(ActionId::Melee, 1, COMBO_BUFFER_SECS);
        for _ in 0..12 {
            b.advance();
        }
        assert!(
            b.take_press(ActionId::Melee),
            "0.24 s of reservation covers an early next-stage press"
        );
    }

    #[test]
    fn press_landings_stay_readable_for_chord_judgments() {
        let mut b = ActionButtons::new();
        assert_eq!(b.press_step(ActionId::Melee), None);
        assert_eq!(b.last_press_step(ActionId::Melee), None);
        b.press_touch(ActionId::Melee, 1, BUFFER_SECS);
        assert_eq!(b.press_step(ActionId::Melee), Some(0));
        assert_eq!(b.last_press_step(ActionId::Melee), Some(0));
        b.advance();
        b.advance();
        b.press_touch(ActionId::Ranged, 2, BUFFER_SECS);
        assert_eq!(b.press_step(ActionId::Ranged), Some(2));
        assert_eq!(b.step_now(), 2);
        // A fired press leaves `press_step` but keeps its landing:
        // the chord judge can still pair against it.
        assert!(b.take_press(ActionId::Melee));
        assert_eq!(b.press_step(ActionId::Melee), None);
        assert_eq!(b.last_press_step(ActionId::Melee), Some(0));
    }

    #[test]
    fn the_host_channel_latches_on_the_rising_edge_only() {
        let mut b = ActionButtons::new();
        b.set_host(ActionId::Melee, true);
        b.set_host(ActionId::Melee, true);
        assert!(b.held(ActionId::Melee));
        assert_eq!(b.counts(ActionId::Melee).0, 1, "one edge, one press");
        b.set_host(ActionId::Melee, false);
        assert!(!b.held(ActionId::Melee));
        assert!(b.take_press(ActionId::Melee), "the press outlives the hold");
    }

    #[test]
    fn only_the_holding_finger_releases_and_clear_drops_everything() {
        let mut b = ActionButtons::new();
        b.press_touch(ActionId::Melee, 5, BUFFER_SECS);
        b.release_touch(ActionId::Melee, 9);
        assert!(b.held(ActionId::Melee), "another finger's end is ignored");
        b.press_touch(ActionId::Ranged, 6, BUFFER_SECS);
        b.clear();
        assert!(!b.held(ActionId::Melee));
        assert!(!b.pending(ActionId::Ranged), "clear drops the buffer too");
        assert_eq!(b.counts(ActionId::Melee).0, 1, "counters survive a clear");
    }

    #[test]
    fn the_jelly_is_drawn_once_per_row() {
        let mut l = DrawList::default();
        blob(
            &mut l,
            &[
                (100.0, 100.0, 20.0),
                (130.0, 100.0, 20.0),
                (160.0, 100.0, 20.0),
            ],
            [255, 255, 255, 100],
        );
        // Overlapping circles do not stack alpha.
        assert_eq!(alpha_at(&l, 130.5, 100.5), 100);
        assert_eq!(alpha_at(&l, 115.5, 100.5), 100);
    }
}
