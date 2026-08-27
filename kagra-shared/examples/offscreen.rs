//! 共有コアの描画をオフスクリーンで 1 枚焼いて PNG に落とす。
//!
//! モバイル実機や WebGPU が無い環境でも、シーンとシェーダが正しいかを確認できる。
//!
//! ```bash
//! cargo run -p kagra-shared --features render --example offscreen
//! cargo run -p kagra-shared --features render --example offscreen -- 960 540 out.png
//! # 2D のタッチデモを見る
//! cargo run -p kagra-shared --features render --example offscreen -- 640 360 demo.png 2d
//! cargo run -p kagra-shared --features render --example offscreen -- 960 540 isle.png isle
//! cargo run -p kagra-shared --features render --example offscreen -- 640 360 world.png world
//! cargo run -p kagra-shared --features render --example offscreen -- 640 360 world.png world path/to/dump.json
//! ```
//!
//! `python -m kagra.render_world dump.json out.png` shells to this example (or an
//! installed helper). Dump JSON is `World.dump()` / `docs/schemas/world.json`.
//! Not kagra-core `RendererV2`. Not the `(-12800,-12800)` fake-headless window.

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use kagra_shared::input::{PointerEvent, PointerPhase};
use kagra_shared::render::Renderer;
use kagra_shared::session::SceneKind;
use kagra_shared::SharedSession;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let width: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(960);
    let height: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(540);
    let out = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "scratch/shared_offscreen.png".to_string()),
    );
    let mode = args.next().unwrap_or_default();
    let two_d = mode == "2d";
    let isle = mode == "isle";
    let world = mode == "world";

    if world {
        let dump_arg = args.next();
        let (json, source) = match dump_arg {
            Some(path) => {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| format!("read world dump {path}: {e}"))?;
                (text, path)
            }
            None => (
                include_str!("../tests/fixtures/crest_isle_world.json").to_string(),
                "kagra-shared/tests/fixtures/crest_isle_world.json".to_string(),
            ),
        };
        let doc = kagra_shared::WorldDoc::from_json(&json)?;
        let mut renderer = pollster::block_on(Renderer::new_offscreen(width, height))?;
        let pixels = renderer.render_world_doc(&doc)?;
        write_png(&out, width, height, &pixels)?;
        println!(
            "wrote {} ({}x{}, {} bytes) from WorldDoc {} compile_scene",
            out.display(),
            width,
            height,
            pixels.len(),
            source
        );
        return Ok(());
    }

    let mut session = SharedSession::default();
    session.create_surface(width, height);

    if isle {
        session.boot_collectathon();
        session.start_game();
        session.set_walk(0.15, 0.85, false);
        for _ in 0..90 {
            session.request_frame();
        }
        let stats = session.request_frame();
        println!(
            "crest isle at ({:.1},{:.1},{:.1}) stars={} coins={}",
            session.isle.walker.x,
            session.isle.walker.y,
            session.isle.walker.z,
            stats.stars,
            stats.coins
        );
    } else if two_d {
        session.set_scene_kind(SceneKind::Demo2D);
        // パッドで右下へ動かし、途中でタップして波紋を出す。
        session.set_pad(0.8, 0.5);
        for frame in 0..40 {
            if frame == 10 {
                session.push_pointer(PointerEvent {
                    id: 0,
                    x: width as f32 * 0.25,
                    y: height as f32 * 0.35,
                    phase: PointerPhase::Begin,
                    pressure: 1.0,
                });
            }
            session.request_frame();
        }
    } else {
        // S 字の途中に置いてから少し走る。曲がった道と LOD の切替が見える位置。
        let pose = session.driving.streamer.path.sample(320.0);
        session.driving.truck.pos = pose.pos;
        session.driving.truck.heading = pose.heading();
        session.driving.truck.speed = 18.0;
        session.driving.path_s = pose.distance;
        session.driving.camera = kagra_shared::ChaseCamera::default();
        for _ in 0..30 {
            session
                .driving
                .camera
                .update(&session.driving.truck, 1.0 / 60.0);
        }
        session.set_drive(0.25, 0.7, 0.0);
        for _ in 0..90 {
            session.request_frame();
        }
        let stats = session.request_frame();
        let on_path = session.driving.streamer.path.sample(session.driving.path_s);
        println!(
            "driving at {:.1} km/h on curve (path x={:.1}, z={:.1}, chunks={})",
            stats.speed_kmh,
            on_path.pos.x,
            on_path.pos.z,
            session.driving.active_chunk_count()
        );
    }

    let renderer = pollster::block_on(Renderer::new_offscreen(width, height))?;
    session.attach_renderer(renderer);
    session.render()?;

    let pixels = session.render_readback()?;
    write_png(&out, width, height, &pixels)?;
    println!(
        "wrote {} ({}x{}, {} bytes)",
        out.display(),
        width,
        height,
        pixels.len()
    );
    Ok(())
}

fn write_png(path: &PathBuf, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let file = File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())
}
