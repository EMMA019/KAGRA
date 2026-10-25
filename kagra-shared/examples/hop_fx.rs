//! HOP-BEAT's finishing pass, one frame cost per variant, and a PNG of each.
//!
//! `cargo run --release -p kagra-shared --features render --example hop_fx -- WORLD.json W H [FRAMES] [OUT_DIR] [QUALITY]`
//!
//! Variants: plain (`fx_post` and `fx_atmo` off), post only, atmosphere only, both.
//! Every frame is rendered offscreen (the phone path) and read back, and the
//! p50/p95 of that time is printed. With no GPU this is lavapipe (CPU raster):
//! the numbers compare effects with each other, they are not phone numbers.
use std::io::BufWriter;
use std::time::Instant;

use kagra_shared::render::Renderer;
use kagra_shared::SharedSession;

fn write_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(rgba).map_err(|e| e.to_string())
}

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&a[0]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let w: u32 = a[1].parse().map_err(|_| "W")?;
    let h: u32 = a[2].parse().map_err(|_| "H")?;
    let frames: u32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(240);
    let out = a.get(4).map(std::path::PathBuf::from);
    let quality = a.get(5).cloned();
    if let Some(dir) = &out {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    println!(
        "{w}x{h} frames={frames} quality={}",
        quality.as_deref().unwrap_or("-")
    );
    for (name, post, atmo) in [
        ("plain", false, false),
        ("post", true, false),
        ("atmo", false, true),
        ("both", true, true),
    ] {
        let mut v = world.clone();
        v["fx_post"] = post.into();
        v["fx_atmo"] = atmo.into();
        if let Some(q) = &quality {
            v["quality"] = q.clone().into();
        }
        let mut session = SharedSession::default();
        session.load_game_json(&v.to_string())?;
        session.start_game();
        session.create_surface(w, h);
        let renderer = pollster::block_on(Renderer::new_phone_offscreen(w, h))?;
        session.attach_renderer(renderer);
        session.render()?;
        if let Some(g) = session.hop_mut() {
            g.confirm();
        }
        let mut ms = Vec::new();
        let mut last = Vec::new();
        for _ in 0..frames {
            session.request_frame();
            let t = Instant::now();
            session.render()?;
            last = session.render_readback()?;
            ms.push(t.elapsed().as_secs_f32() * 1000.0);
        }
        if let Some(dir) = &out {
            write_png(&dir.join(format!("{name}.png")), w, h, &last)?;
        }
        let props = session.hop().map(|g| g.document().props.len()).unwrap_or(0);
        ms.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let q = |f: f32| ms[((ms.len() as f32 - 1.0) * f).round() as usize];
        println!(
            "{name:<6} props={props:<4} render p50={:.2} p95={:.2} ms",
            q(0.5),
            q(0.95)
        );
    }
    Ok(())
}
