use std::collections::HashSet;

pub struct InputState {
    // keyboard
    held:     HashSet<u32>,
    pressed:  HashSet<u32>,
    released: HashSet<u32>,

    // mouse
    mouse_x: f32,
    mouse_y: f32,
    mouse_held: HashSet<u32>,
    mouse_pressed: HashSet<u32>,
    mouse_released: HashSet<u32>,
    wheel_x: f32,
    wheel_y: f32,
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
        }
    }

    pub fn begin_frame(&mut self) {
        self.pressed.clear();
        self.released.clear();
        self.mouse_pressed.clear();
        self.mouse_released.clear();
        self.wheel_x = 0.0;
        self.wheel_y = 0.0;
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

    pub fn is_key_down(&self, code: u32) -> bool { self.held.contains(&code) }
    pub fn is_key_pressed(&self, code: u32) -> bool { self.pressed.contains(&code) }
    pub fn is_key_released(&self, code: u32) -> bool { self.released.contains(&code) }

    // mouse events (called from window.rs)
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

    // --- Getters (used by lib.rs and window.rs) ---
    #[allow(dead_code)]
    pub fn mouse_x(&self) -> f32 { self.mouse_x }
    #[allow(dead_code)]
    pub fn mouse_y(&self) -> f32 { self.mouse_y }
    pub fn mouse_pos(&self) -> (f32, f32) { (self.mouse_x, self.mouse_y) }
    
    pub fn is_mouse_down(&self, button: u32) -> bool { self.mouse_held.contains(&button) }
    pub fn is_mouse_pressed(&self, button: u32) -> bool { self.mouse_pressed.contains(&button) }
    pub fn is_mouse_released(&self, button: u32) -> bool { self.mouse_released.contains(&button) }
    
    #[allow(dead_code)]
    pub fn wheel_x(&self) -> f32 { self.wheel_x }
    pub fn wheel_y(&self) -> f32 { self.wheel_y }
    pub fn mouse_wheel(&self) -> (f32, f32) { (self.wheel_x, self.wheel_y) }
}