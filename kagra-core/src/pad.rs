// USB / XInput via gilrs. Polled on the winit EventLoop thread (Windows: one loop).
use gilrs::{Axis, Button, Gamepad, Gilrs};

const BTN_A: u32 = 1 << 0;
const BTN_B: u32 = 1 << 1;
const BTN_X: u32 = 1 << 2;
const BTN_Y: u32 = 1 << 3;
const BTN_LB: u32 = 1 << 4;
const BTN_RB: u32 = 1 << 5;
const BTN_LT: u32 = 1 << 6;
const BTN_RT: u32 = 1 << 7;
const BTN_SELECT: u32 = 1 << 8;
const BTN_START: u32 = 1 << 9;
const BTN_UP: u32 = 1 << 10;
const BTN_DOWN: u32 = 1 << 11;
const BTN_LEFT: u32 = 1 << 12;
const BTN_RIGHT: u32 = 1 << 13;
const BTN_LS: u32 = 1 << 14;
const BTN_RS: u32 = 1 << 15;

#[derive(Clone, Debug, Default)]
pub struct PadHw {
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
    held: u32,
}

impl PadHw {
    pub fn axis(&self, stick: u32) -> (f32, f32) {
        if stick == 1 {
            (self.rx, self.ry)
        } else {
            (self.lx, self.ly)
        }
    }

    pub fn down(&self, name: &str) -> bool {
        button_bit(name).is_some_and(|bit| self.held & bit != 0)
    }
}

pub fn button_bit(name: &str) -> Option<u32> {
    match name.trim().to_ascii_lowercase().as_str() {
        "a" => Some(BTN_A),
        "b" => Some(BTN_B),
        "x" => Some(BTN_X),
        "y" => Some(BTN_Y),
        "lb" => Some(BTN_LB),
        "rb" => Some(BTN_RB),
        "lt" => Some(BTN_LT),
        "rt" => Some(BTN_RT),
        "select" => Some(BTN_SELECT),
        "start" => Some(BTN_START),
        "up" => Some(BTN_UP),
        "down" => Some(BTN_DOWN),
        "left" => Some(BTN_LEFT),
        "right" => Some(BTN_RIGHT),
        "ls" => Some(BTN_LS),
        "rs" => Some(BTN_RS),
        _ => None,
    }
}

pub fn map_button(b: Button) -> Option<u32> {
    match b {
        Button::South => Some(BTN_A),
        Button::East => Some(BTN_B),
        Button::West => Some(BTN_X),
        Button::North => Some(BTN_Y),
        // gilrs: Trigger = L2/R2, Trigger2 = L1/R1
        Button::LeftTrigger => Some(BTN_LT),
        Button::LeftTrigger2 => Some(BTN_LB),
        Button::RightTrigger => Some(BTN_RT),
        Button::RightTrigger2 => Some(BTN_RB),
        Button::Select => Some(BTN_SELECT),
        Button::Start => Some(BTN_START),
        Button::DPadUp => Some(BTN_UP),
        Button::DPadDown => Some(BTN_DOWN),
        Button::DPadLeft => Some(BTN_LEFT),
        Button::DPadRight => Some(BTN_RIGHT),
        Button::LeftThumb => Some(BTN_LS),
        Button::RightThumb => Some(BTN_RS),
        _ => None,
    }
}

const WATCH: [Button; 16] = [
    Button::South,
    Button::East,
    Button::West,
    Button::North,
    Button::LeftTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger,
    Button::RightTrigger2,
    Button::Select,
    Button::Start,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
    Button::LeftThumb,
    Button::RightThumb,
];

pub fn fill_from_gamepad(gp: &Gamepad, state: &mut PadHw) {
    // gilrs stick Y is usually negative-up — same as Walk / VirtualPad (down = +Y).
    state.lx = gp.value(Axis::LeftStickX).clamp(-1.0, 1.0);
    state.ly = gp.value(Axis::LeftStickY).clamp(-1.0, 1.0);
    state.rx = gp.value(Axis::RightStickX).clamp(-1.0, 1.0);
    state.ry = gp.value(Axis::RightStickY).clamp(-1.0, 1.0);
    state.held = 0;
    for btn in WATCH {
        if gp.is_pressed(btn) {
            if let Some(bit) = map_button(btn) {
                state.held |= bit;
            }
        }
    }
}

pub fn pump(gilrs: &mut Gilrs, state: &mut PadHw) {
    while gilrs.next_event().is_some() {}
    *state = PadHw::default();
    if let Some((_, gp)) = gilrs.gamepads().find(|(_, g)| g.is_connected()) {
        fill_from_gamepad(&gp, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn south_is_a_and_start_is_start() {
        assert_eq!(map_button(Button::South), Some(BTN_A));
        assert_eq!(map_button(Button::Start), Some(BTN_START));
        assert_eq!(button_bit("A"), Some(BTN_A));
        assert_eq!(button_bit("start"), Some(BTN_START));
        let mut hw = PadHw::default();
        hw.held = BTN_A | BTN_START;
        assert!(hw.down("a"));
        assert!(hw.down("start"));
        assert!(!hw.down("b"));
        assert_eq!(hw.axis(0), (0.0, 0.0));
        hw.rx = 0.5;
        hw.ry = -1.0;
        assert_eq!(hw.axis(1), (0.5, -1.0));
    }
}
