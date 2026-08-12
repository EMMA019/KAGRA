// src/input.rs
use std::collections::HashSet;

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

    pub fn on_mouse_wheel(&mut self, dx: f32, dy: f32) {
        self.wheel_x += dx;
        self.wheel_y += dy;
    }

    pub fn mouse_pos(&self) -> (f32, f32) { (self.mouse_x, self.mouse_y) }
    pub fn is_mouse_down(&self, button: u32) -> bool { self.mouse_held.contains(&button) }
    pub fn is_mouse_pressed(&self, button: u32) -> bool { self.mouse_pressed.contains(&button) }
    pub fn is_mouse_released(&self, button: u32) -> bool { self.mouse_released.contains(&button) }
    pub fn mouse_wheel(&self) -> (f32, f32) { (self.wheel_x, self.wheel_y) }
    pub fn wheel_y(&self) -> f32 { self.wheel_y }
}