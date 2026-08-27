//! Real desktop window: WorldDoc → live tick → Scene3D → kagra-shared wgpu 30.
//!
//! Collectathon loop on the official path: title → play → result.
//! WASD walks, mouse / arrows look, Space starts (title) or jumps (play).
//! Shared `WorldPlay` advances the dump each frame (wish → sit on heightfield
//! → pick up coins/stars). Capsule player; VRM is not this path.
//! Esc / Q / close quit. `--seconds` starts, injects forward walk, then exits.
//!
//! winit 0.29 is the workspace line. Separate process + wgpu 30. Do not load
//! RendererV2 in this process.
//!
//! ```bash
//! cargo run -p kagra-shared --features render --example window
//! cargo run -p kagra-shared --features render --example window -- path/to/dump.json
//! cargo run -p kagra-shared --features render --example window -- dump.json --width 960 --height 540 --seconds 8
//! ```
//!
//! `python -m kagra.play_world dump.json` shells to this example.
//!
//! Desktop-only. Wasm canvas uses `Renderer::new_for_surface`.

#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use kagra_shared::collectathon::WalkInput;
use kagra_shared::render::Renderer;
use kagra_shared::WorldPlay;
#[cfg(not(target_arch = "wasm32"))]
use winit::event::{DeviceEvent, ElementState, Event, KeyEvent, MouseButton, WindowEvent};
#[cfg(not(target_arch = "wasm32"))]
use winit::event_loop::{ControlFlow, EventLoop};
#[cfg(not(target_arch = "wasm32"))]
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
#[cfg(not(target_arch = "wasm32"))]
use winit::window::{CursorGrabMode, WindowBuilder};

struct Args {
    dump: PathBuf,
    width: u32,
    height: u32,
    seconds: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Keys {
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    jump: bool,
    attack: bool,
    dodge: bool,
}

impl Keys {
    fn walk_input(self) -> WalkInput {
        let lx = (self.d as i32 - self.a as i32) as f32;
        let lz = (self.w as i32 - self.s as i32) as f32;
        WalkInput {
            lx,
            lz,
            jump: self.jump,
            attack: self.attack,
            dodge: self.dodge,
        }
        .clamped()
    }

    fn look_delta(self, dt: f32) -> (f32, f32) {
        const SPEED: f32 = 1.6;
        let yaw = (self.right as i32 - self.left as i32) as f32 * SPEED * dt;
        let pitch = (self.up as i32 - self.down as i32) as f32 * SPEED * dt;
        (yaw, pitch)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_key(keys: &mut Keys, key: &Key, physical: &PhysicalKey, down: bool) {
    if let PhysicalKey::Code(code) = physical {
        match code {
            KeyCode::KeyW => keys.w = down,
            KeyCode::KeyA => keys.a = down,
            KeyCode::KeyS => keys.s = down,
            KeyCode::KeyD => keys.d = down,
            KeyCode::ArrowLeft => keys.left = down,
            KeyCode::ArrowRight => keys.right = down,
            KeyCode::ArrowUp => keys.up = down,
            KeyCode::ArrowDown => keys.down = down,
            KeyCode::Space => keys.jump = down,
            KeyCode::KeyJ | KeyCode::KeyZ | KeyCode::KeyF => keys.attack = down,
            KeyCode::ShiftLeft | KeyCode::ShiftRight | KeyCode::KeyC | KeyCode::ControlLeft => {
                keys.dodge = down
            }
            _ => {}
        }
    }
    match key {
        Key::Named(NamedKey::ArrowLeft) => keys.left = down,
        Key::Named(NamedKey::ArrowRight) => keys.right = down,
        Key::Named(NamedKey::ArrowUp) => keys.up = down,
        Key::Named(NamedKey::ArrowDown) => keys.down = down,
        Key::Named(NamedKey::Space) => keys.jump = down,
        Key::Character(c) => {
            if c.eq_ignore_ascii_case("w") {
                keys.w = down;
            } else if c.eq_ignore_ascii_case("a") {
                keys.a = down;
            } else if c.eq_ignore_ascii_case("s") {
                keys.s = down;
            } else if c.eq_ignore_ascii_case("d") {
                keys.d = down;
            } else if c.eq_ignore_ascii_case("j")
                || c.eq_ignore_ascii_case("z")
                || c.eq_ignore_ascii_case("f")
            {
                keys.attack = down;
            } else if c.eq_ignore_ascii_case("c") {
                keys.dodge = down;
            }
        }
        _ => {}
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn grab_cursor(window: &winit::window::Window) {
    let _ = window
        .set_cursor_grab(CursorGrabMode::Locked)
        .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
    window.set_cursor_visible(false);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // Example `window` is desktop-only. Wasm stays on `new_for_surface`.
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    let args = parse_args()?;
    let json = std::fs::read_to_string(&args.dump)
        .map_err(|e| format!("read world dump {}: {e}", args.dump.display()))?;
    let mut play = WorldPlay::from_json(&json)?;

    let event_loop = EventLoop::new().map_err(|e| format!("no display: {e}"))?;
    let window = WindowBuilder::new()
        .with_title(if play.is_action() {
            "KAGRA Action Arena (shared wgpu 30)"
        } else if play.is_platformer() {
            "KAGRA Box Hop (shared wgpu 30)"
        } else {
            "KAGRA Crest Isle (shared wgpu 30)"
        })
        .with_inner_size(winit::dpi::LogicalSize::new(args.width, args.height))
        .build(&event_loop)
        .map_err(|e| format!("no display: {e}"))?;
    let window = Arc::new(window);
    let size = window.inner_size();
    let width = size.width.max(1);
    let height = size.height.max(1);
    let mut renderer = pollster::block_on(Renderer::new_for_window(window.clone(), width, height))?;
    renderer.upload_world_meshes(&play.doc)?;
    println!(
        "WorldDoc window {} ({}x{})\n  Space / Enter start (title or result)\n  WASD walk | mouse or arrows look | Space jump | Esc quit\n  click the window if the mouse look is not captured",
        args.dump.display(),
        width,
        height
    );
    grab_cursor(&window);

    let start = Instant::now();
    let mut last = Instant::now();
    let seconds = args.seconds;
    let mut keys = Keys::default();
    let mut mouse_look = (0.0f32, 0.0f32);
    let mut view_w = width;
    let mut view_h = height;
    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => elwt.exit(),
                Event::WindowEvent {
                    event:
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    logical_key,
                                    physical_key,
                                    state,
                                    repeat: false,
                                    ..
                                },
                            ..
                        },
                    ..
                } => {
                    let down = state == ElementState::Pressed;
                    if down && is_quit_key(&logical_key) {
                        elwt.exit();
                        return;
                    }
                    if down && is_start_key(&logical_key) && !play.game.is_playing() {
                        play.confirm();
                        return;
                    }
                    apply_key(&mut keys, &logical_key, &physical_key, down);
                }
                Event::WindowEvent {
                    event:
                        WindowEvent::MouseInput {
                            state: ElementState::Pressed,
                            button: MouseButton::Left,
                            ..
                        },
                    ..
                } => {
                    if !play.game.is_playing() {
                        play.confirm();
                    } else {
                        keys.attack = true;
                    }
                    grab_cursor(&window);
                }
                Event::DeviceEvent {
                    event: DeviceEvent::MouseMotion { delta },
                    ..
                } => {
                    const SENS: f32 = 0.005;
                    mouse_look.0 += delta.0 as f32 * SENS;
                    mouse_look.1 += delta.1 as f32 * SENS;
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    if size.width > 0 && size.height > 0 {
                        view_w = size.width;
                        view_h = size.height;
                        renderer.resize(size.width, size.height);
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    let t = start.elapsed().as_secs_f32();
                    if let Some(max) = seconds {
                        if t >= max {
                            elwt.exit();
                            return;
                        }
                        if !play.game.is_playing() {
                            play.start();
                        }
                    }
                    let dt = last.elapsed().as_secs_f32();
                    last = Instant::now();
                    let (arrow_yaw, arrow_pitch) = keys.look_delta(dt);
                    play.add_look(arrow_yaw + mouse_look.0, arrow_pitch + mouse_look.1);
                    mouse_look = (0.0, 0.0);
                    let mut input = keys.walk_input();
                    // Headless-ish smoke: --seconds walks forward so a live
                    // tick is visible without a human holding W.
                    if seconds.is_some()
                        && play.game.is_playing()
                        && input.lz.abs() < 1e-4
                        && input.lx.abs() < 1e-4
                    {
                        input.lz = 1.0;
                    }
                    if seconds.is_some() && play.is_action() && play.game.is_playing() {
                        input.attack = true;
                    }
                    if seconds.is_some() && play.is_platformer() && play.game.is_playing() {
                        // hop so landing is visible without a human holding Space
                        input.jump = true;
                    }
                    if !play.game.is_playing() {
                        input = WalkInput::default();
                    }
                    play.input = input;
                    play.tick(dt);
                    keys.attack = false;
                    let scene = play.doc.compile_scene(renderer.aspect());
                    let hud = play.build_hud(view_w, view_h);
                    if let Err(e) = renderer.render_frame(Some(&scene), &hud) {
                        eprintln!("draw: {e}");
                        elwt.exit();
                    }
                }
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .map_err(|e| format!("window event loop: {e}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn is_quit_key(key: &Key) -> bool {
    match key {
        Key::Named(NamedKey::Escape) => true,
        Key::Character(c) => c.eq_ignore_ascii_case("q"),
        _ => false,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn is_start_key(key: &Key) -> bool {
    match key {
        Key::Named(NamedKey::Enter | NamedKey::Space) => true,
        Key::Character(c) => c.eq_ignore_ascii_case(" ") || c.eq_ignore_ascii_case("enter"),
        _ => false,
    }
}

fn parse_args() -> Result<Args, String> {
    let mut dump = None;
    let mut width = 960u32;
    let mut height = 540u32;
    let mut seconds = None::<f32>;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--width" => {
                width = parse_next(&mut args, "--width")?;
            }
            "--height" => {
                height = parse_next(&mut args, "--height")?;
            }
            "--seconds" => {
                seconds = Some(parse_next(&mut args, "--seconds")?);
            }
            s if s.starts_with('-') => {
                return Err(format!("unknown flag {s} (see --help)"));
            }
            s => dump = Some(PathBuf::from(s)),
        }
    }
    let dump = dump.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/crest_isle_world.json")
    });
    Ok(Args {
        dump,
        width,
        height,
        seconds,
    })
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    let raw = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
    raw.parse().map_err(|e| format!("{flag} {raw:?}: {e}"))
}

fn print_help() {
    eprintln!(
        "kagra-shared WorldDoc window (wgpu 30, not RendererV2)\n\
         \n\
         cargo run -p kagra-shared --features render --example window -- [dump.json] \\\n\
             [--width 960] [--height 540] [--seconds N]\n\
         \n\
         WASD walk, mouse / arrows look, click/J attack, Shift/C dodge, Space jump, Esc / Q / close.\n\
         Space / Enter / click starts from the title or result.\n\
         Default dump is the Crest Isle collectathon. --seconds starts, walks, then exits.\n\
         No display → error containing \"no display\" (Python skips)."
    );
}
