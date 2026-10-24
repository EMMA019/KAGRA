//! Per-frame render cost of Neon Cat with and without the night atmosphere.
//!
//! `cargo run --release -p kagra-shared --features render --example fx_cost -- WORLD.json W H [FRAMES]`
//!
//! Plays the scripted run, renders every frame offscreen (the phone path) and
//! prints the p50/p95/max of `render()` in ms, plus the draw and triangle
//! counts the renderer reports. On a machine with no GPU this is lavapipe
//! (CPU raster): the numbers compare variants, they are not phone numbers.
use std::time::Instant;

use kagra_shared::render::Renderer;
use kagra_shared::SharedSession;

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let world = std::fs::read_to_string(&a[0]).map_err(|e| e.to_string())?;
    let w: u32 = a[1].parse().map_err(|_| "W")?;
    let h: u32 = a[2].parse().map_err(|_| "H")?;
    let frames: u32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(240);
    let mut session = SharedSession::default();
    session.load_game_json(&world)?;
    session.start_game();
    let renderer = pollster::block_on(Renderer::new_phone_offscreen(w, h))?;
    session.attach_renderer(renderer);
    session.render()?;
    let (mut render_ms, mut sim_ms) = (Vec::new(), Vec::new());
    for _ in 0..frames {
        let steer = session.cat.as_ref().map(|run| run.script_steer());
        if let Some((lx, jump)) = steer {
            session.set_play_input(lx, 0.0, jump, false, false);
        }
        let t0 = Instant::now();
        session.request_frame();
        sim_ms.push(t0.elapsed().as_secs_f32() * 1000.0);
        let t1 = Instant::now();
        session.render()?;
        // Wait for the GPU so the time is the whole frame.
        let _ = session.render_readback()?;
        render_ms.push(t1.elapsed().as_secs_f32() * 1000.0);
    }
    if let Some(cat) = session.cat.as_ref() {
        let t = Instant::now();
        let mut bytes = 0;
        for _ in 0..1000 {
            bytes = cat.state_json().len();
        }
        println!(
            "state_json {bytes} bytes, {:.1} us per call",
            t.elapsed().as_secs_f32() * 1000.0
        );
    }
    let t = Instant::now();
    let mut bytes = 0;
    for _ in 0..1000 {
        bytes = session.stats_json().len();
    }
    println!(
        "stats_json {bytes} bytes, {:.1} us per call",
        t.elapsed().as_secs_f32() * 1000.0
    );
    let pct = |v: &mut Vec<f32>, q: f32| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[((v.len() as f32 - 1.0) * q).round() as usize]
    };
    println!(
        "{w}x{h} frames={frames} sim p50={:.2} render p50={:.2} p95={:.2} max={:.2} ms",
        pct(&mut sim_ms, 0.5),
        pct(&mut render_ms.clone(), 0.5),
        pct(&mut render_ms.clone(), 0.95),
        pct(&mut render_ms, 1.0),
    );
    Ok(())
}
