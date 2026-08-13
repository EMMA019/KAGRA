//! オフスクリーン描画の回帰テスト。
//!
//! GPU が無い CI ランナーではアダプタが取れないので、その場合は静かに通す。
//! 「絵が出ているか」は色で判定する（画素完全一致はドライバ差で落ちるため）。

#![cfg(feature = "render")]

use kagra_shared::render::Renderer;
use kagra_shared::SharedSession;

const W: u32 = 64;
const H: u32 = 64;

fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

/// アダプタが無ければ None。
fn render_frames(frames: u64) -> Option<Vec<u8>> {
    let renderer = pollster::block_on(Renderer::new_offscreen(W, H)).ok()?;
    let mut session = SharedSession::default();
    session.create_surface(W, H);
    session.attach_renderer(renderer);
    for _ in 0..frames {
        session.request_frame();
    }
    session.render().expect("render failed");
    Some(session.render_readback().expect("readback failed"))
}

#[test]
fn offscreen_frame_shows_scene() {
    let Some(rgba) = render_frames(30) else {
        eprintln!("no GPU adapter; skipping offscreen render test");
        return;
    };
    assert_eq!(rgba.len() as u32, W * H * 4);

    // 単色ではない（何かが描かれている）
    let first = pixel(&rgba, 0, 0);
    assert!(
        rgba.chunks(4).any(|p| p != first),
        "frame is a flat color; nothing was drawn"
    );

    // 画面中央のプレイヤーは暖色
    let center = pixel(&rgba, W / 2, H / 2);
    assert!(
        center[0] > center[2],
        "expected a warm player quad at the center, got {center:?}"
    );

    // 下端のフレームバーは緑寄り（frame=30 なので左半分が埋まる）
    let bar = pixel(&rgba, 2, H - 3);
    assert!(
        bar[1] > bar[0] && bar[1] > bar[2],
        "expected the green frame bar at the bottom, got {bar:?}"
    );

    // 背景は上が暗く下が明るい
    let top = pixel(&rgba, W - 2, 1);
    let bottom = pixel(&rgba, W - 2, H - 12);
    assert!(
        bottom[2] > top[2],
        "expected a vertical gradient, top={top:?} bottom={bottom:?}"
    );
}

#[test]
fn resize_keeps_rendering() {
    let Some(renderer) = pollster::block_on(Renderer::new_offscreen(W, H)).ok() else {
        eprintln!("no GPU adapter; skipping resize test");
        return;
    };
    let mut session = SharedSession::default();
    session.attach_renderer(renderer);
    session.create_surface(128, 48);
    session.request_frame();
    session.render().expect("render after resize failed");
    let rgba = session.render_readback().expect("readback failed");
    assert_eq!(rgba.len(), 128 * 48 * 4);
}
