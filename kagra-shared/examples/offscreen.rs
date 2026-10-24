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
//! cargo run -p kagra-shared --features render --example offscreen -- 1280 720 overlay.png playtest
//! cargo run -p kagra-shared --features render --example offscreen -- 390 844 lumen.png neon
//! cargo run -p kagra-shared --features render --example offscreen -- 1280 720 cat.png cat play
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
    let flow = mode == "flow";
    let neon = mode == "neon";
    let cat = mode == "cat";
    let mut phone_unorm = false;

    if mode == "playtest" {
        return render_playtest(width, height, &out);
    }

    if world {
        let dump_arg = args.next();
        let rest: Vec<String> = args.collect();
        // Threshold bloom + FXAA on by default (same as the play_world window);
        // --no-bloom / --no-fxaa compare the raw pipeline stages.
        let bloom = !rest.iter().any(|a| a == "--no-bloom");
        let fxaa = !rest.iter().any(|a| a == "--no-fxaa");
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
        renderer.set_bloom(0.85, if bloom { 0.35 } else { 0.0 });
        renderer.set_fxaa(fxaa);
        let pixels = renderer.render_world_doc(&doc)?;
        write_png(&out, width, height, &pixels)?;
        let profile = renderer.frame_profile();
        if let Ok(dump) = doc.to_json() {
            let report = kagra_shared::splice_profile(&dump, &profile);
            let mut report_path = out.clone();
            report_path.set_extension("profile.json");
            if let Some(parent) = report_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&report_path, report) {
                eprintln!("profile dump: {e}");
            } else {
                println!("wrote {}", report_path.display());
            }
        }
        for warning in profile.warnings() {
            eprintln!("profile: {warning}");
        }
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

    if neon {
        let frames: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(100);
        let boost = args.next().as_deref() == Some("boost");
        session.load_game_json(include_str!("../tests/fixtures/neon_run_world.json"))?;
        session.push_key("space", true);
        for _i in 0..frames {
            if boost {
                session.set_play_input(0.0, 0.0, true, false, false);
            }
            session.request_frame();
        }
        println!("lumen frames={frames} boost={boost}");
    } else if cat {
        let which = args.next().unwrap_or_else(|| "play".into());
        let frames: u32 =
            args.next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(match which.as_str() {
                    "title" => 90,
                    "crash" => 220,
                    "flat" => 200,
                    "debug" => 130,
                    _ => 160,
                });
        let mut quality: Option<String> = None;
        let mut classic = false;
        for arg in args.by_ref() {
            match arg.as_str() {
                "low" | "mobile" | "high" => quality = Some(arg),
                "classic" => classic = true,
                _ => {}
            }
        }
        let mut world = include_str!("../tests/fixtures/neon_cat_world.json").to_string();
        if quality.is_some() || classic {
            let mut head = String::from("{");
            if let Some(tier) = quality.as_deref() {
                head.push_str(&format!("\"quality\":\"{tier}\","));
                phone_unorm = true;
            }
            if classic {
                head.push_str("\"cam\":\"classic\",");
            }
            world = world.replacen('{', &head, 1);
        }
        session.load_game_json(&world)?;
        if which == "debug" {
            session.set_debug(true);
        }
        if which != "title" {
            session.start_game();
        }
        for _ in 0..frames {
            match which.as_str() {
                "crash" => session.set_play_input(1.0, 0.0, false, false, false),
                "flat" | "debug" => session.set_play_input(0.0, 0.0, false, false, false),
                "play" => {
                    let steer = session.cat().map(|run| run.script_steer());
                    if let Some((lx, jump)) = steer {
                        session.set_play_input(lx, 0.0, jump, false, false);
                    }
                }
                _ => {}
            }
            session.request_frame();
        }
        let phase = session
            .cat()
            .map(|run| run.phase_name())
            .unwrap_or("missing");
        let score = session.cat().map(|run| run.score()).unwrap_or(0);
        let x = session.cat().map(|run| run.x()).unwrap_or(0.0);
        let death = session
            .cat()
            .map(|run| run.death().to_string())
            .unwrap_or_default();
        println!(
            "neon cat {which} frames={frames} phase={phase} score={score} x={x:.2} death={death} quality={}",
            session
                .cat()
                .and_then(|run| run.document().quality.clone())
                .unwrap_or_else(|| "legacy".into())
        );
    } else if flow {
        let which = args.next().unwrap_or_else(|| "title".into());
        let json = include_str!("../tests/fixtures/platformer_world.json");
        session.load_game_json(json)?;
        if which == "pause" {
            if let Some(play) = session.world.as_mut() {
                play.confirm();
                play.request_pause();
            }
        } else if which == "result" {
            if let Some(play) = session.world.as_mut() {
                play.confirm();
                play.flow.finish(1000, 1, "GOAL");
                play.game.phase = kagra_shared::GamePhase::Complete;
            }
        }
        println!("box hop flow screen: {which}");
    } else if isle {
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

    let renderer = if phone_unorm {
        pollster::block_on(Renderer::new_phone_offscreen(width, height))?
    } else {
        pollster::block_on(Renderer::new_offscreen(width, height))?
    };
    session.attach_renderer(renderer);
    session.render()?;
    if cat {
        println!("picture {}", session.picture_status());
    }

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

fn render_playtest(width: u32, height: u32, out: &PathBuf) -> Result<(), String> {
    let json = include_str!("../tests/fixtures/platformer_world.json");
    let mut session = SharedSession::default();
    session.create_surface(width, height);
    session.set_playtest_enabled(true);
    session.load_game_json(json)?;
    if let Some(play) = session.world.as_mut() {
        play.confirm();
    }
    session.set_play_input(0.0, 1.0, false, false, false);
    for _ in 0..30 {
        session.request_frame();
    }
    let renderer = pollster::block_on(Renderer::new_offscreen(width, height))?;
    session.attach_renderer(renderer);
    session.render()?;
    let game = session.render_readback()?;
    let before = game_shot_path(out);
    write_png(&before, width, height, &game)?;
    session.playtest_set_note("直してここ");
    session.open_playtest();
    session.render()?;
    let overlay = session.render_readback()?;
    write_png(out, width, height, &overlay)?;
    println!(
        "playtest overlay {} and game frame {} ({}x{})",
        out.display(),
        before.display(),
        width,
        height
    );
    Ok(())
}

fn game_shot_path(out: &std::path::Path) -> PathBuf {
    let stem = out
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("playtest");
    let ext = out.extension().and_then(|s| s.to_str()).unwrap_or("png");
    let name = format!("{stem}_game.{ext}");
    match out.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(name),
        _ => PathBuf::from(name),
    }
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
