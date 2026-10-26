//! Touch, pointer and key input: the stick thumb, the aiming taps, the cards and pause buttons.

use super::*;

impl SurvivorArena {
    pub(super) fn touch_ev(&mut self, ev: PointerEvent, w: u32, h: u32) {
        self.size.set((w.max(1) as f32, h.max(1) as f32));
        let live = self.flow.phase == FlowPhase::Play && self.cards.is_empty();
        match ev.phase {
            PointerPhase::Begin if live && self.stick_mode() => {
                let (w, h) = self.size.get();
                if self.finger_id.is_none() && self.in_stick_zone(ev.x, ev.y, w, h) {
                    // The stick thumb: the base appears where it lands (kept on screen), never a tap.
                    let r = self.stick_radius(w, h);
                    self.finger_id = Some(ev.id);
                    self.finger = Some((ev.x, ev.y));
                    self.origin = stick::land(ev.x, ev.y, r, 4.0 * hud_scale(w, h), w, h);
                } else if self.finger_id != Some(ev.id) {
                    // The other thumb: a tap picks the target.
                    self.tap_select(ev.x, ev.y);
                }
            }
            PointerPhase::Begin => {
                if live && self.finger_id.is_none() {
                    self.finger_id = Some(ev.id);
                    self.finger = Some((ev.x, ev.y));
                    self.origin = (ev.x, ev.y);
                    self.tap = Some((ev.x, ev.y, 0.0));
                } else if live && self.finger_id != Some(ev.id) {
                    // The other thumb: a tap picks the target.
                    self.tap_select(ev.x, ev.y);
                }
            }
            PointerPhase::Move => {
                if self.finger_id == Some(ev.id) && !self.mute_finger {
                    self.finger = Some((ev.x, ev.y));
                    if self.stick_mode() {
                        // Past the rim the base follows the thumb, so turning back is instant.
                        let (w, h) = self.size.get();
                        let r = self.stick_radius(w, h);
                        self.origin = stick::follow(self.origin, (ev.x, ev.y), r);
                    }
                }
            }
            PointerPhase::End | PointerPhase::Cancel if self.finger_id == Some(ev.id) => {
                // A quick, short touch of the lone finger is a tap too.
                if let (PointerPhase::End, Some((tx, ty, age))) = (ev.phase, self.tap.take()) {
                    let (w, h) = self.size.get();
                    if live && age < 0.25 && (ev.x - tx).hypot(ev.y - ty) < 12.0 * hud_scale(w, h) {
                        self.tap_select(ev.x, ev.y);
                    }
                }
                self.tap = None;
                self.finger_id = None;
                self.finger = None;
                self.mute_finger = false;
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }

    pub(super) fn stick_mode(&self) -> bool {
        self.spec.input.move_scheme == "stick"
    }

    /// Stick radius in pixels.
    pub(super) fn stick_radius(&self, w: f32, h: f32) -> f32 {
        self.spec.input.stick_px * hud_scale(w, h) * 1.25
    }

    /// Where the resting stick hint sits: bottom-left, clear of the screen edge and the home bar.
    /// A thumb anywhere in the zone gets its own base.
    pub(super) fn stick_center(&self, w: f32, h: f32) -> (f32, f32) {
        let r = self.stick_radius(w, h);
        (
            (r * 1.35).max(w * 0.2),
            h - r * 1.45 - 12.0 * hud_scale(w, h),
        )
    }

    /// The thumb zone that grabs the stick: the whole lower-left quadrant-ish, so a thumb
    /// need not land on the base.
    pub(super) fn in_stick_zone(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        x < w * 0.55 && y > h * 0.4
    }

    /// A tap on a card, a pause button or a pause-card button. True when used.
    pub fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) -> bool {
        let (w, h) = (width.max(1) as f32, height.max(1) as f32);
        let s = hud_scale(w, h);
        match self.flow.phase {
            FlowPhase::Pause => {
                for (b, r) in pause_buttons(w, h, s) {
                    if r.contains(x, y) {
                        match b {
                            PauseBtn::Resume => self.flow.resume(),
                            PauseBtn::Aim => {
                                let next = (self.aim_mode + 1) % 3;
                                self.set_aim(["auto", "manual", "hybrid"][next as usize]);
                            }
                            PauseBtn::Retry => self.start_run(),
                            PauseBtn::Title => self.show_title(),
                        }
                        self.mute_after_tap();
                        return true;
                    }
                }
                true
            }
            FlowPhase::Play if !self.cards.is_empty() => {
                for (i, r) in card_rects(w, h, s, self.cards.len()).iter().enumerate() {
                    if r.contains(x, y) {
                        self.pick_card(i);
                        self.mute_after_tap();
                        break;
                    }
                }
                true
            }
            FlowPhase::Play => {
                if pause_rect(w, s).contains(x, y) {
                    self.pause();
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    /// The finger that made this tap must lift before it steers again.
    pub(super) fn mute_after_tap(&mut self) {
        self.finger = None;
        self.tap = None;
        if self.finger_id.is_some() {
            self.mute_finger = true;
        }
    }

    /// Keyboard or pad steering: `lz` is up the screen.
    pub fn walk(&mut self, lx: f32, lz: f32) {
        self.key_dir = (lx, -lz);
    }
}
