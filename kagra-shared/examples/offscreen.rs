//! 共有コアの描画をオフスクリーンで 1 枚焼いて PNG に落とす。
//!
//! モバイル実機や WebGPU が無い環境でも、シーンとシェーダが正しいかを確認できる。
//!
//! ```bash
//! cargo run -p kagra-shared --features render --example offscreen
//! cargo run -p kagra-shared --features render --example offscreen -- 960 540 out.png
//! ```

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use kagra_shared::input::{PointerEvent, PointerPhase};
use kagra_shared::render::Renderer;
use kagra_shared::SharedSession;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let width: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(640);
    let height: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(360);
    let out = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "scratch/shared_offscreen.png".to_string()),
    );

    let mut session = SharedSession::default();
    session.create_surface(width, height);

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
