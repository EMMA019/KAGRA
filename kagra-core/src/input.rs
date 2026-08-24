// src/input.rs
use std::collections::HashSet;
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

pub struct InputState {
    held:     HashSet<u32>,
    pressed:  HashSet<u32>,
    released: HashSet<u32>,
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
        self.pressed.clear();
        self.released.clear();
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
    }

    pub fn on_key_up(&mut self, code: u32) {
        self.held.remove(&code);
        self.released.insert(code);
    }

    /// OS auto-repeat must not re-press a key after release (sticky walk).
    pub fn apply_key(&mut self, code: u32, down: bool, repeat: bool) {
        if down {
            if !repeat {
                self.on_key_down(code);
            }
        } else {
            self.on_key_up(code);
        }
    }

    /// Focus loss / IME: key-up often never arrives, so treat every key as released.
    pub fn release_all(&mut self) {
        for code in self.held.drain() {
            self.released.insert(code);
        }
        for btn in self.mouse_held.drain() {
            self.mouse_released.insert(btn);
        }
    }

    pub fn set_backspace_pressed(&mut self) { self.backspace_pressed = true; }
    pub fn set_enter_pressed(&mut self) { self.enter_pressed = true; }
    pub fn set_escape_pressed(&mut self) { self.escape_pressed = true; }

    pub fn is_key_down(&self, code: u32) -> bool { self.held.contains(&code) }
    pub fn is_key_pressed(&self, code: u32) -> bool { self.pressed.contains(&code) }
    pub fn is_key_released(&self, code: u32) -> bool { self.released.contains(&code) }

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

    pub fn mouse_pos(&self) -> (f32, f32) { (self.mouse_x, self.mouse_y) }
    pub fn mouse_delta(&self) -> (f32, f32) { (self.mouse_dx, self.mouse_dy) }
    pub fn is_mouse_down(&self, button: u32) -> bool { self.mouse_held.contains(&button) }
    pub fn is_mouse_pressed(&self, button: u32) -> bool { self.mouse_pressed.contains(&button) }
    pub fn is_mouse_released(&self, button: u32) -> bool { self.mouse_released.contains(&button) }
    pub fn mouse_wheel(&self) -> (f32, f32) { (self.wheel_x, self.wheel_y) }
    pub fn wheel_y(&self) -> f32 { self.wheel_y }
}

/// Physical code, or logical arrow/letter when the OS reports ``Unidentified``.
pub fn resolve_keycode(physical: PhysicalKey, logical: &Key) -> Option<KeyCode> {
    if let PhysicalKey::Code(code) = physical {
        return Some(code);
    }
    match logical {
        Key::Named(named) => named_to_keycode(*named),
        Key::Character(s) => character_to_keycode(s.as_str()),
        _ => None,
    }
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
        inp.apply_key(DOWN, false, false);
        inp.apply_key(DOWN, true, true);
        assert!(!inp.is_key_down(DOWN), "repeat after key-up must not stick");
        assert!(!inp.is_key_pressed(DOWN));
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
}