// src/input.rs
use std::collections::{HashMap, HashSet};
use winit::keyboard::{Key, KeyCode, NamedKey, NativeKeyCode, PhysicalKey};

/// Auto-repeat typically starts after ~250ms. Holds longer than this many
/// `begin_frame`s get a longer post-up block (Issue B, Crest Isle).
const LONG_HOLD_FRAMES: u32 = 8;
/// After a long hold, ignore leftover non-repeat KEYDOWN until this many
/// quiet `begin_frame`s with no down. A leftover down refreshes the window.
/// 15 frames (~250ms) ate Emma's fast re-tap; 3 frames (~50ms) still blocks
/// same/next-frame leftovers while a real press after ~3 quiet frames walks.
const REHOLD_QUIET_FRAMES: u8 = 3;

pub struct InputState {
    held: HashSet<u32>,
    pressed: HashSet<u32>,
    released: HashSet<u32>,
    /// Codes that went up last frame. Windows WM_KEYUP then a queued KEYDOWN has
    /// KF_REPEAT clear (`repeat=false`), so #71's repeat filter misses it.
    rehold_block: HashSet<u32>,
    /// Quiet frames remaining after a long-hold key-up. Leftover KEYDOWN refreshes.
    rehold_quiet: HashMap<u32, u8>,
    /// `begin_frame`s seen while the key was held.
    hold_frames: HashMap<u32, u32>,
    /// Auto-repeat seen while held. A hitch can starve `begin_frame` counts.
    saw_repeat: HashSet<u32>,
    /// Physical token → KeyCode while down. IME `Process` / JIS Unidentified
    /// release still clears the same walk key.
    native_held: HashMap<u64, KeyCode>,
    mouse_x: f32,
    mouse_y: f32,
    mouse_held: HashSet<u32>,
    mouse_pressed: HashSet<u32>,
    mouse_released: HashSet<u32>,
    wheel_x: f32,
    wheel_y: f32,
    mouse_dx: f32,
    mouse_dy: f32,
    pub char_buffer: Vec<char>,
    pub preedit_text: String,
    pub preedit_cursor: Option<(usize, usize)>,
    pub ime_x: f32,
    pub ime_y: f32,
    pub backspace_pressed: bool,
    pub enter_pressed: bool,
    pub escape_pressed: bool,
    pub focused: bool,
}

impl InputState {
    pub fn new() -> Self {
        InputState {
            held: HashSet::new(),
            pressed: HashSet::new(),
            released: HashSet::new(),
            rehold_block: HashSet::new(),
            rehold_quiet: HashMap::new(),
            hold_frames: HashMap::new(),
            saw_repeat: HashSet::new(),
            native_held: HashMap::new(),
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_held: HashSet::new(),
            mouse_pressed: HashSet::new(),
            mouse_released: HashSet::new(),
            wheel_x: 0.0,
            wheel_y: 0.0,
            mouse_dx: 0.0,
            mouse_dy: 0.0,
            char_buffer: Vec::new(),
            preedit_text: String::new(),
            preedit_cursor: None,
            ime_x: 100.0,
            ime_y: 600.0,
            backspace_pressed: false,
            enter_pressed: false,
            escape_pressed: false,
            focused: false,
        }
    }

    pub fn begin_frame(&mut self) {
        for code in self.held.iter() {
            *self.hold_frames.entry(*code).or_insert(0) += 1;
        }
        self.rehold_quiet.retain(|_, n| {
            *n = n.saturating_sub(1);
            *n > 0
        });
        // Last frame's key-ups block a non-repeat re-down for one more frame.
        self.rehold_block = std::mem::take(&mut self.released);
        self.pressed.clear();
        self.mouse_pressed.clear();
        self.mouse_released.clear();
        self.wheel_x = 0.0;
        self.wheel_y = 0.0;
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        self.char_buffer.clear();
        self.backspace_pressed = false;
        self.enter_pressed = false;
        self.escape_pressed = false;
    }

    pub fn on_preedit(&mut self, text: &str, cursor: Option<(usize, usize)>) {
        self.preedit_text = text.to_string();
        self.preedit_cursor = cursor;
    }

    pub fn on_commit(&mut self, text: &str) {
        self.preedit_text.clear();
        self.preedit_cursor = None;
        for c in text.chars() {
            self.char_buffer.push(c);
        }
    }

    pub fn on_char(&mut self, c: char) {
        if c >= ' ' || c == '\n' || c == '\t' {
            self.char_buffer.push(c);
        }
    }

    pub fn on_key_down(&mut self, code: u32) {
        if !self.held.contains(&code) {
            self.pressed.insert(code);
        }
        self.held.insert(code);
        self.hold_frames.entry(code).or_insert(0);
    }

    fn arm_rehold(&mut self, code: u32) {
        let held = self.hold_frames.remove(&code).unwrap_or(0);
        let repeated = self.saw_repeat.remove(&code);
        if held >= LONG_HOLD_FRAMES || repeated {
            self.rehold_quiet.insert(code, REHOLD_QUIET_FRAMES);
        }
    }

    pub fn on_key_up(&mut self, code: u32) {
        self.held.remove(&code);
        self.released.insert(code);
        self.arm_rehold(code);
    }

    /// OS auto-repeat must not re-press a key after release (sticky walk).
    ///
    /// Windows: after WM_KEYUP, a late WM_KEYDOWN has bit 30 clear so winit
    /// reports `repeat=false`. Ignore that re-down for the rest of this frame
    /// and the next (`rehold_block`). After a long hold, also ignore leftover
    /// non-repeat KEYDOWN until `REHOLD_QUIET_FRAMES` quiet frames (`rehold_quiet`).
    pub fn apply_key(&mut self, code: u32, down: bool, repeat: bool) {
        if down {
            if repeat {
                if self.held.contains(&code) {
                    self.saw_repeat.insert(code);
                }
                return;
            }
            if self.released.contains(&code) || self.rehold_block.contains(&code) {
                if self.rehold_quiet.contains_key(&code) {
                    self.rehold_quiet.insert(code, REHOLD_QUIET_FRAMES);
                }
                return;
            }
            if self.rehold_quiet.contains_key(&code) {
                self.rehold_quiet.insert(code, REHOLD_QUIET_FRAMES);
                return;
            }
            self.on_key_down(code);
        } else {
            self.on_key_up(code);
        }
    }

    /// WindowEvent / focused DeviceEvent. Pairs Unidentified IME releases to the press.
    pub fn ingest_key(
        &mut self,
        physical: PhysicalKey,
        logical: &Key,
        down: bool,
        repeat: bool,
    ) -> Option<KeyCode> {
        let token = native_token(physical);
        let resolved =
            resolve_keycode(physical, logical).or_else(|| windows_unidentified_walk_key(physical));
        if down {
            let code = resolved?;
            if let Some(t) = token {
                self.native_held.insert(t, code);
            }
            if let Some(scan) = keycode_to_windows_scan(code) {
                self.native_held.insert(windows_scan_token(scan), code);
            }
            self.apply_key(code as u32, true, repeat);
            Some(code)
        } else {
            let from_token = token.and_then(|t| self.native_held.remove(&t));
            let code = resolved.or(from_token)?;
            if let Some(scan) = keycode_to_windows_scan(code) {
                self.native_held.remove(&windows_scan_token(scan));
            }
            if let Some(t) = token {
                self.native_held.remove(&t);
            }
            self.apply_key(code as u32, false, repeat);
            Some(code)
        }
    }

    /// Focus loss / IME: key-up often never arrives, so treat every key as released.
    pub fn release_all(&mut self) {
        let held: Vec<u32> = self.held.drain().collect();
        for code in held {
            self.released.insert(code);
            self.arm_rehold(code);
        }
        self.native_held.clear();
        for btn in self.mouse_held.drain() {
            self.mouse_released.insert(btn);
        }
    }

    pub fn set_backspace_pressed(&mut self) {
        self.backspace_pressed = true;
    }
    pub fn set_enter_pressed(&mut self) {
        self.enter_pressed = true;
    }
    pub fn set_escape_pressed(&mut self) {
        self.escape_pressed = true;
    }

    pub fn is_key_down(&self, code: u32) -> bool {
        self.held.contains(&code)
    }
    pub fn is_key_pressed(&self, code: u32) -> bool {
        self.pressed.contains(&code)
    }
    pub fn is_key_released(&self, code: u32) -> bool {
        self.released.contains(&code)
    }

    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        self.mouse_x = x;
        self.mouse_y = y;
    }

    pub fn on_mouse_down(&mut self, button: u32) {
        if !self.mouse_held.contains(&button) {
            self.mouse_pressed.insert(button);
        }
        self.mouse_held.insert(button);
    }

    pub fn on_mouse_up(&mut self, button: u32) {
        self.mouse_held.remove(&button);
        self.mouse_released.insert(button);
    }

    pub fn on_mouse_delta(&mut self, dx: f32, dy: f32) {
        self.mouse_dx += dx;
        self.mouse_dy += dy;
    }

    pub fn on_mouse_wheel(&mut self, dx: f32, dy: f32) {
        self.wheel_x += dx;
        self.wheel_y += dy;
    }

    pub fn mouse_pos(&self) -> (f32, f32) {
        (self.mouse_x, self.mouse_y)
    }
    pub fn mouse_delta(&self) -> (f32, f32) {
        (self.mouse_dx, self.mouse_dy)
    }
    pub fn is_mouse_down(&self, button: u32) -> bool {
        self.mouse_held.contains(&button)
    }
    pub fn is_mouse_pressed(&self, button: u32) -> bool {
        self.mouse_pressed.contains(&button)
    }
    pub fn is_mouse_released(&self, button: u32) -> bool {
        self.mouse_released.contains(&button)
    }
    pub fn mouse_wheel(&self) -> (f32, f32) {
        (self.wheel_x, self.wheel_y)
    }
    pub fn wheel_y(&self) -> f32 {
        self.wheel_y
    }
}

const TAG_CODE: u64 = 1 << 32;
const TAG_WIN: u64 = 2 << 32;
const TAG_MAC: u64 = 3 << 32;
const TAG_XKB: u64 = 4 << 32;
const TAG_AND: u64 = 5 << 32;

fn native_token(physical: PhysicalKey) -> Option<u64> {
    match physical {
        PhysicalKey::Code(code) => Some(TAG_CODE | (code as u32 as u64)),
        PhysicalKey::Unidentified(native) => match native {
            NativeKeyCode::Windows(v) => Some(windows_scan_token(v)),
            NativeKeyCode::MacOS(v) => Some(TAG_MAC | u64::from(v)),
            NativeKeyCode::Xkb(v) => Some(TAG_XKB | u64::from(v)),
            NativeKeyCode::Android(v) => Some(TAG_AND | u64::from(v)),
            NativeKeyCode::Unidentified => None,
        },
    }
}

fn windows_scan_token(scan: u16) -> u64 {
    TAG_WIN | u64::from(scan)
}

/// Physical code, or logical arrow/letter when the OS reports ``Unidentified``.
/// ``NamedKey::Process`` (VK_PROCESSKEY) is not a walk key.
pub fn resolve_keycode(physical: PhysicalKey, logical: &Key) -> Option<KeyCode> {
    if let PhysicalKey::Code(code) = physical {
        return Some(code);
    }
    match logical {
        Key::Named(NamedKey::Process) => None,
        Key::Named(named) => named_to_keycode(*named),
        Key::Character(s) => character_to_keycode(s.as_str()),
        _ => None,
    }
}

fn windows_unidentified_walk_key(physical: PhysicalKey) -> Option<KeyCode> {
    match physical {
        PhysicalKey::Unidentified(NativeKeyCode::Windows(scan)) => windows_scan_to_keycode(scan),
        _ => None,
    }
}

fn windows_scan_to_keycode(scan: u16) -> Option<KeyCode> {
    // HIWORD scancode; extended arrows often arrive as 0xE0xx.
    let low = scan & 0x00FF;
    Some(match low {
        0x11 => KeyCode::KeyW,
        0x1E => KeyCode::KeyA,
        0x1F => KeyCode::KeyS,
        0x20 => KeyCode::KeyD,
        0x39 => KeyCode::Space,
        0x48 => KeyCode::ArrowUp,
        0x4B => KeyCode::ArrowLeft,
        0x4D => KeyCode::ArrowRight,
        0x50 => KeyCode::ArrowDown,
        _ => return None,
    })
}

fn keycode_to_windows_scan(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::KeyW => 0x11,
        KeyCode::KeyA => 0x1E,
        KeyCode::KeyS => 0x1F,
        KeyCode::KeyD => 0x20,
        KeyCode::Space => 0x39,
        KeyCode::ArrowUp => 0x48,
        KeyCode::ArrowLeft => 0x4B,
        KeyCode::ArrowRight => 0x4D,
        KeyCode::ArrowDown => 0x50,
        _ => return None,
    })
}

fn named_to_keycode(named: NamedKey) -> Option<KeyCode> {
    Some(match named {
        NamedKey::ArrowDown => KeyCode::ArrowDown,
        NamedKey::ArrowUp => KeyCode::ArrowUp,
        NamedKey::ArrowLeft => KeyCode::ArrowLeft,
        NamedKey::ArrowRight => KeyCode::ArrowRight,
        NamedKey::Space => KeyCode::Space,
        NamedKey::Enter => KeyCode::Enter,
        NamedKey::Escape => KeyCode::Escape,
        NamedKey::Backspace => KeyCode::Backspace,
        NamedKey::Tab => KeyCode::Tab,
        _ => return None,
    })
}

fn character_to_keycode(s: &str) -> Option<KeyCode> {
    let mut chars = s.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(match ch.to_ascii_uppercase() {
        'A' => KeyCode::KeyA,
        'B' => KeyCode::KeyB,
        'C' => KeyCode::KeyC,
        'D' => KeyCode::KeyD,
        'E' => KeyCode::KeyE,
        'F' => KeyCode::KeyF,
        'G' => KeyCode::KeyG,
        'H' => KeyCode::KeyH,
        'I' => KeyCode::KeyI,
        'J' => KeyCode::KeyJ,
        'K' => KeyCode::KeyK,
        'L' => KeyCode::KeyL,
        'M' => KeyCode::KeyM,
        'N' => KeyCode::KeyN,
        'O' => KeyCode::KeyO,
        'P' => KeyCode::KeyP,
        'Q' => KeyCode::KeyQ,
        'R' => KeyCode::KeyR,
        'S' => KeyCode::KeyS,
        'T' => KeyCode::KeyT,
        'U' => KeyCode::KeyU,
        'V' => KeyCode::KeyV,
        'W' => KeyCode::KeyW,
        'X' => KeyCode::KeyX,
        'Y' => KeyCode::KeyY,
        'Z' => KeyCode::KeyZ,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::NativeKeyCode;

    const DOWN: u32 = KeyCode::ArrowDown as u32;
    const KEY_S: u32 = KeyCode::KeyS as u32;

    #[test]
    fn key_up_clears_held() {
        let mut inp = InputState::new();
        inp.on_key_down(DOWN);
        assert!(inp.is_key_down(DOWN));
        inp.on_key_up(DOWN);
        assert!(!inp.is_key_down(DOWN));
        assert!(inp.is_key_released(DOWN));
    }

    #[test]
    fn late_repeat_after_up_does_not_rehold() {
        let mut inp = InputState::new();
        inp.apply_key(DOWN, true, false);
        inp.begin_frame();
        inp.apply_key(DOWN, false, false);
        inp.begin_frame();
        inp.apply_key(DOWN, true, true);
        assert!(!inp.is_key_down(DOWN), "repeat after key-up must not stick");
        assert!(!inp.is_key_pressed(DOWN));
    }

    #[test]
    fn windows_keyup_then_nonrepeat_down_same_frame_does_not_rehold() {
        // Actual Win32 path: after WM_KEYUP, KF_REPEAT is 0 so winit says repeat=false.
        let mut inp = InputState::new();
        inp.apply_key(DOWN, true, false);
        inp.begin_frame();
        inp.apply_key(DOWN, false, false);
        inp.apply_key(DOWN, true, false);
        assert!(
            !inp.is_key_down(DOWN),
            "Windows post-up KEYDOWN must not stick"
        );
    }

    #[test]
    fn windows_keyup_then_nonrepeat_down_next_frame_does_not_rehold() {
        let mut inp = InputState::new();
        inp.apply_key(DOWN, true, false);
        inp.begin_frame();
        inp.apply_key(DOWN, false, false);
        inp.begin_frame();
        inp.apply_key(DOWN, true, false);
        assert!(
            !inp.is_key_down(DOWN),
            "queued post-up KEYDOWN next frame must not stick"
        );
        inp.begin_frame();
        inp.apply_key(DOWN, true, false);
        assert!(
            inp.is_key_down(DOWN),
            "a real re-press after the block window must work"
        );
    }

    #[test]
    fn long_hold_leftover_down_for_30_frames_then_quiet_then_real_press() {
        // Hitch-stalled leftover: down → many auto-repeat → up → 30 frames of
        // leftover `repeat=false` KEYDOWN must not re-hold. Each leftover
        // refreshes a 3-frame quiet window. After 3 silent frames a real
        // re-press must hold.
        let mut inp = InputState::new();
        inp.apply_key(DOWN, true, false);
        for _ in 0..80 {
            inp.begin_frame();
            inp.apply_key(DOWN, true, true);
        }
        inp.begin_frame();
        inp.apply_key(DOWN, false, false);
        assert!(!inp.is_key_down(DOWN));
        for i in 0..30 {
            inp.begin_frame();
            inp.apply_key(DOWN, true, false);
            assert!(
                !inp.is_key_down(DOWN),
                "leftover KEYDOWN frame {i} after long hold must not re-hold"
            );
        }
        for _ in 0..REHOLD_QUIET_FRAMES {
            inp.begin_frame();
        }
        inp.apply_key(DOWN, true, false);
        assert!(
            inp.is_key_down(DOWN),
            "a real re-press after 3 quiet frames must work"
        );
    }

    #[test]
    fn long_hold_three_quiet_frames_then_repress_holds() {
        // Emma: long hold → release → same key after ~3 quiet frames must walk.
        let mut inp = InputState::new();
        inp.apply_key(DOWN, true, false);
        for _ in 0..LONG_HOLD_FRAMES {
            inp.begin_frame();
            inp.apply_key(DOWN, true, true);
        }
        inp.apply_key(DOWN, false, false);
        for _ in 0..3 {
            inp.begin_frame();
        }
        inp.apply_key(DOWN, true, false);
        assert!(
            inp.is_key_down(DOWN),
            "a real re-press after 3 quiet frames must hold"
        );
    }

    #[test]
    fn leftover_down_during_quiet_refreshes_window() {
        let mut inp = InputState::new();
        inp.apply_key(DOWN, true, false);
        for _ in 0..20 {
            inp.begin_frame();
            inp.apply_key(DOWN, true, true);
        }
        inp.begin_frame();
        inp.apply_key(DOWN, false, false);
        inp.begin_frame();
        inp.apply_key(DOWN, true, false);
        assert!(!inp.is_key_down(DOWN));
        for _ in 0..(REHOLD_QUIET_FRAMES as usize - 2) {
            inp.begin_frame();
        }
        inp.apply_key(DOWN, true, false);
        assert!(
            !inp.is_key_down(DOWN),
            "leftover KEYDOWN must refresh the quiet window, not re-hold"
        );
        for _ in 0..REHOLD_QUIET_FRAMES {
            inp.begin_frame();
        }
        inp.apply_key(DOWN, true, false);
        assert!(inp.is_key_down(DOWN));
    }

    #[test]
    fn hitch_repeat_without_many_begin_frames_still_blocks() {
        // Load hitch can starve begin_frame while auto-repeat still queues.
        let mut inp = InputState::new();
        inp.apply_key(DOWN, true, false);
        inp.begin_frame();
        for _ in 0..40 {
            inp.apply_key(DOWN, true, true);
        }
        inp.apply_key(DOWN, false, false);
        for i in 0..30 {
            inp.begin_frame();
            inp.apply_key(DOWN, true, false);
            assert!(
                !inp.is_key_down(DOWN),
                "saw_repeat must arm quiet even with few begin_frames (i={i})"
            );
        }
        for _ in 0..REHOLD_QUIET_FRAMES {
            inp.begin_frame();
        }
        inp.apply_key(DOWN, true, false);
        assert!(inp.is_key_down(DOWN));
    }

    #[test]
    fn short_tap_still_reholds_after_one_frame_block() {
        // Taps must stay snappy: only the #80 1–2 frame window, not 16 frames.
        let mut inp = InputState::new();
        inp.apply_key(KEY_S, true, false);
        inp.begin_frame();
        inp.apply_key(KEY_S, false, false);
        inp.begin_frame();
        inp.apply_key(KEY_S, true, false);
        assert!(
            !inp.is_key_down(KEY_S),
            "one-frame rehold_block still applies to taps"
        );
        inp.begin_frame();
        inp.apply_key(KEY_S, true, false);
        assert!(
            inp.is_key_down(KEY_S),
            "a tap re-press after the short block window must work"
        );
    }

    #[test]
    fn repeat_while_held_keeps_held_without_repress() {
        let mut inp = InputState::new();
        inp.apply_key(DOWN, true, false);
        inp.begin_frame();
        inp.apply_key(DOWN, true, true);
        assert!(inp.is_key_down(DOWN));
        assert!(!inp.is_key_pressed(DOWN));
    }

    #[test]
    fn release_all_clears_held_keys() {
        let mut inp = InputState::new();
        inp.on_key_down(DOWN);
        inp.on_mouse_down(1);
        inp.release_all();
        assert!(!inp.is_key_down(DOWN));
        assert!(inp.is_key_released(DOWN));
        assert!(!inp.is_mouse_down(1));
        assert!(inp.is_mouse_released(1));
    }

    #[test]
    fn unidentified_physical_uses_logical_arrow_down() {
        let code = resolve_keycode(
            PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
            &Key::Named(NamedKey::ArrowDown),
        );
        assert_eq!(code, Some(KeyCode::ArrowDown));
    }

    #[test]
    fn unidentified_physical_uses_logical_letter() {
        let code = resolve_keycode(
            PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
            &Key::Character("s".into()),
        );
        assert_eq!(code, Some(KeyCode::KeyS));
    }

    #[test]
    fn process_logical_is_not_a_keycode() {
        let code = resolve_keycode(
            PhysicalKey::Unidentified(NativeKeyCode::Windows(0x50)),
            &Key::Named(NamedKey::Process),
        );
        assert_eq!(code, None);
    }

    #[test]
    fn ime_process_release_clears_arrow_via_windows_scan() {
        let mut inp = InputState::new();
        inp.ingest_key(
            PhysicalKey::Code(KeyCode::ArrowDown),
            &Key::Named(NamedKey::ArrowDown),
            true,
            false,
        );
        assert!(inp.is_key_down(DOWN));
        inp.ingest_key(
            PhysicalKey::Unidentified(NativeKeyCode::Windows(0x50)),
            &Key::Named(NamedKey::Process),
            false,
            false,
        );
        assert!(!inp.is_key_down(DOWN), "IME Process release must clear ↓");
    }

    #[test]
    fn ime_process_release_clears_s_via_windows_scan() {
        let mut inp = InputState::new();
        inp.ingest_key(
            PhysicalKey::Code(KeyCode::KeyS),
            &Key::Character("s".into()),
            true,
            false,
        );
        assert!(inp.is_key_down(KEY_S));
        inp.ingest_key(
            PhysicalKey::Unidentified(NativeKeyCode::Windows(0x1F)),
            &Key::Named(NamedKey::Process),
            false,
            false,
        );
        assert!(!inp.is_key_down(KEY_S), "IME Process release must clear S");
    }

    #[test]
    fn extended_windows_scan_0xe050_is_arrow_down() {
        let mut inp = InputState::new();
        inp.ingest_key(
            PhysicalKey::Unidentified(NativeKeyCode::Windows(0xE050)),
            &Key::Named(NamedKey::Process),
            true,
            false,
        );
        assert!(inp.is_key_down(DOWN));
        inp.ingest_key(
            PhysicalKey::Unidentified(NativeKeyCode::Windows(0xE050)),
            &Key::Named(NamedKey::Process),
            false,
            false,
        );
        assert!(!inp.is_key_down(DOWN));
    }
}
