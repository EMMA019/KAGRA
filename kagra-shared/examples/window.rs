//! Real desktop window: WorldDoc → Scene3D → kagra-shared wgpu 30 Renderer.
//!
//! Same `Renderer` as collectathon / mobile / the offscreen example. Not
//! kagra-core `RendererV2`. Not the `(-12800,-12800)` fake-headless window.
//! Capsules / boxes / plane. Esc or the close button quit. Optional timed
//! orbit, then exit (`--seconds`). WASD is a later slice.
//!
//! winit 0.29 is the workspace line (kagra-core also uses it with wgpu 0.19).
//! This example is a **separate process** talking to wgpu 30 via
//! raw-window-handle 0.6. Do not load both renderers in one process.
//!
//! ```bash
//! cargo run -p kagra-shared --features render --example window
//! cargo run -p kagra-shared --features render --example window -- path/to/dump.json
//! cargo run -p kagra-shared --features render --example window -- dump.json --width 960 --height 540 --seconds 8
//! ```
//!
//! `python -m kagra.play_world dump.json` shells to this example (or an
//! installed helper). Skip when there is no display.
//!
//! Desktop-only. Wasm canvas uses `Renderer::new_for_surface`.

#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use kagra_shared::render::Renderer;
use kagra_shared::scene::DrawList;
use kagra_shared::{Camera, WorldDoc};
#[cfg(not(target_arch = "wasm32"))]
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
#[cfg(not(target_arch = "wasm32"))]
use winit::event_loop::{ControlFlow, EventLoop};
#[cfg(not(target_arch = "wasm32"))]
use winit::keyboard::{Key, NamedKey};
#[cfg(not(target_arch = "wasm32"))]
use winit::window::WindowBuilder;

struct Args {
    dump: PathBuf,
    width: u32,
    height: u32,
    seconds: Option<f32>,
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
    let doc = WorldDoc::from_json(&json)?;
    let base_cam = doc
        .compile_scene(args.width as f32 / args.height.max(1) as f32)
        .camera;

    let event_loop = EventLoop::new().map_err(|e| format!("no display: {e}"))?;
    let window = WindowBuilder::new()
        .with_title("KAGRA WorldDoc (shared wgpu 30)")
        .with_inner_size(winit::dpi::LogicalSize::new(args.width, args.height))
        .build(&event_loop)
        .map_err(|e| format!("no display: {e}"))?;
    let window = Arc::new(window);
    let size = window.inner_size();
    let width = size.width.max(1);
    let height = size.height.max(1);
    let mut renderer = pollster::block_on(Renderer::new_for_window(window.clone(), width, height))?;
    renderer.upload_compile_meshes()?;
    println!(
        "WorldDoc window {} ({}x{}) compile_scene → shared wgpu 30 (Esc to close)",
        args.dump.display(),
        width,
        height
    );

    let start = Instant::now();
    let seconds = args.seconds;
    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);
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
                                    state: ElementState::Pressed,
                                    repeat: false,
                                    ..
                                },
                            ..
                        },
                    ..
                } if is_quit_key(&logical_key) => elwt.exit(),
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    if size.width > 0 && size.height > 0 {
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
                    }
                    let mut scene = doc.compile_scene(renderer.aspect());
                    scene.camera = orbit_camera(base_cam, t);
                    if let Err(e) = renderer.render_frame(Some(&scene), &DrawList::default()) {
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

/// Slow XZ orbit so a few seconds of present is visible. Height stays.
fn orbit_camera(cam: Camera, t: f32) -> Camera {
    use glam::Vec3;
    let target = cam.target;
    let rel = cam.eye - target;
    let radius = rel.length().max(0.5);
    let height = rel.y;
    let xz = (radius * radius - height * height).max(0.25).sqrt();
    let base = rel.z.atan2(rel.x);
    let ang = base + t * 0.28;
    Camera {
        eye: target + Vec3::new(xz * ang.cos(), height, xz * ang.sin()),
        ..cam
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
         Esc / Q / window close. Default dump is the Crest Isle fixture.\n\
         No display → error containing \"no display\" (Python skips)."
    );
}
