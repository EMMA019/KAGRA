//! ????????????????
//!
//! GPU ??? CI ?????????????????????????????
//! ????????????????????????????????????????

#![cfg(feature = "render")]

use std::sync::{Mutex, OnceLock};

use kagra_shared::render::Renderer;
use kagra_shared::session::SceneKind;
use kagra_shared::SharedSession;

const W: u32 = 128;
const H: u32 = 96;

/// ????????????????????????????????? wgpu
/// ????????????????????????
static GPU: Mutex<()> = Mutex::new(());

fn pixel(rgba: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

/// ????????? None???????????????????
fn with_session<T>(w: u32, h: u32, f: impl FnOnce(&mut SharedSession) -> T) -> Option<T> {
    let _guard = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let renderer = pollster::block_on(Renderer::new_offscreen(w, h)).ok()?;
    let mut session = SharedSession::default();
    session.create_surface(w, h);
    session.attach_renderer(renderer);
    Some(f(&mut session))
}

fn render_2d(frames: u64) -> Option<Vec<u8>> {
    with_session(W, H, |session| {
        session.set_scene_kind(SceneKind::Demo2D);
        for _ in 0..frames {
            session.request_frame();
        }
        session.render().expect("render failed");
        session.render_readback().expect("readback failed")
    })
}

fn render_driving(straight: f32, turn: f32, steer: f32) -> Option<Vec<u8>> {
    with_session(W, H, |session| {
        session.set_drive(0.0, 1.0, 0.0);
        for _ in 0..(straight * 60.0) as u64 {
            session.request_frame();
        }
        session.set_drive(steer, 1.0, 0.0);
        for _ in 0..(turn * 60.0) as u64 {
            session.request_frame();
        }
        session.render().expect("render failed");
        session.render_readback().expect("readback failed")
    })
}

/// ??????? 1 ???????????????????????????
fn straight_frame() -> Option<&'static [u8]> {
    static FRAME: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    FRAME
        .get_or_init(|| render_driving(4.0, 0.0, 0.0))
        .as_deref()
}

#[test]
fn driving_scene_has_sky_road_and_truck() {
    let Some(rgba) = straight_frame() else {
        eprintln!("no GPU adapter; skipping driving render test");
        return;
    };
    assert_eq!(rgba.len() as u32, W * H * 4);

    // ????????????
    let sky = pixel(rgba, W, W / 2, 3);
    assert!(
        sky[2] > sky[0] && sky[2] > sky[1],
        "expected sky at the top, got {sky:?}"
    );

    // ?????????????????
    // A single sample can land on a roadside pole, so measure area instead.
    let grass = count_pixels(rgba, is_grass);
    assert!(
        grass > 200,
        "expected grass beside the road, got {grass} px"
    );

    let road = count_pixels(rgba, is_road);
    assert!(road > 200, "expected a road surface, got {road} px");

    // ?????????????????????
    let (count, _) = truck_pixels(rgba);
    assert!(count > 20, "could not find the red truck body ({count} px)");
}

/// ??????????????????????????????????
#[test]
fn depth_buffer_keeps_the_truck_in_front() {
    let Some(rgba) = straight_frame() else {
        return;
    };
    let (count, _) = truck_pixels(rgba);
    assert!(
        count > 20,
        "the truck was overdrawn by the ground; depth test is not working"
    );
}

/// ???????????????????????????????
#[test]
fn distance_fog_fades_the_road_toward_the_sky() {
    let Some(rgba) = straight_frame() else {
        return;
    };
    let sky = pixel(rgba, W, W / 2, 2);
    // ??????????????????????????
    let far = pixel(rgba, W, W / 2, H / 2 - 4);
    let near = pixel(rgba, W, W / 2, H - 2);
    let dist = |p: [u8; 4]| {
        (0..3)
            .map(|i| (p[i] as i32 - sky[i] as i32).abs())
            .sum::<i32>()
    };
    assert!(
        dist(far) < dist(near),
        "distant road should blend into the sky; far={far:?} near={near:?} sky={sky:?}"
    );
}

/// ???????????????????????????
#[test]
fn lighting_shades_faces_differently() {
    let Some(rgba) = straight_frame() else {
        return;
    };
    let mut reds: Vec<u8> = Vec::new();
    for y in 0..H {
        for x in 0..W {
            let p = pixel(rgba, W, x, y);
            if is_truck(p) {
                reds.push(p[0]);
            }
        }
    }
    assert!(reds.len() > 20, "could not find the truck body");
    let min = *reds.iter().min().unwrap();
    let max = *reds.iter().max().unwrap();
    assert!(
        max - min > 8,
        "the truck is flat-shaded; lighting is not applied (min={min} max={max})"
    );
}

/// ??????????????????????
/// The chase camera keeps the truck centred, so steering shows up as the world
/// swinging the other way: turn right and the road slides to the left.
/// This is the one check that the sign of the steering survives the whole
/// pipeline (input -> heading -> camera -> projection).
#[test]
fn steering_right_swings_the_road_left() {
    let turned = render_driving(4.0, 0.6, 0.6);
    let (Some(straight), Some(turned)) = (straight_frame(), turned.as_deref()) else {
        return;
    };
    let before = center_of(straight, is_road);
    let after = center_of(turned, is_road);
    assert!(
        after < before - 2.0,
        "steering right should sweep the road to the left: {before} -> {after}"
    );
}

/// However far it turns, the camera keeps the truck roughly centred.
#[test]
fn chase_camera_keeps_the_truck_centered() {
    let Some(rgba) = render_driving(4.0, 0.6, 0.6) else {
        return;
    };
    let (count, center) = truck_pixels(&rgba);
    assert!(count > 20, "could not find the truck body");
    let offset = (center - W as f32 * 0.5).abs();
    assert!(
        offset < W as f32 * 0.25,
        "the camera lost the truck: it sits at x={center} of {W}"
    );
}

#[test]
fn offscreen_frame_shows_2d_scene() {
    let Some(rgba) = render_2d(30) else {
        eprintln!("no GPU adapter; skipping 2D render test");
        return;
    };

    let first = pixel(&rgba, W, 0, 0);
    assert!(
        rgba.chunks(4).any(|p| p != first),
        "frame is a flat color; nothing was drawn"
    );

    // ?????????????
    let center = pixel(&rgba, W, W / 2, H / 2);
    assert!(
        center[0] > center[2],
        "expected a warm player quad at the center, got {center:?}"
    );

    // ??????????????frame=30 ???????????
    let bar = pixel(&rgba, W, 2, H - 3);
    assert!(
        bar[1] > bar[0] && bar[1] > bar[2],
        "expected the green frame bar at the bottom, got {bar:?}"
    );
}

#[test]
fn resize_keeps_rendering() {
    let Some(rgba) = with_session(W, H, |session| {
        session.create_surface(128, 48);
        session.request_frame();
        session.render().expect("render after resize failed");
        session.render_readback().expect("readback failed")
    }) else {
        eprintln!("no GPU adapter; skipping resize test");
        return;
    };
    assert_eq!(rgba.len(), 128 * 48 * 4);

    // ?????????????????????????????????
    let sky = pixel(&rgba, 128, 64, 1);
    assert!(sky[2] > sky[0], "sky is missing after resize: {sky:?}");
}

/// ?????????????????????????
fn is_truck(p: [u8; 4]) -> bool {
    p[0] > 60 && p[0] as i32 > p[1] as i32 + 35 && p[0] as i32 > p[2] as i32 + 35
}

/// ??????????????
fn is_grass(p: [u8; 4]) -> bool {
    (p[1] as i32) > p[0] as i32 + 10 && (p[1] as i32) > p[2] as i32 + 20
}

/// ???????????????????????????
fn is_road(p: [u8; 4]) -> bool {
    let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
    r < 110 && (r - g).abs() < 12 && (b - g).abs() < 24 && b >= g
}

fn count_pixels(rgba: &[u8], pred: fn([u8; 4]) -> bool) -> usize {
    let mut n = 0;
    for y in 0..H {
        for x in 0..W {
            if pred(pixel(rgba, W, x, y)) {
                n += 1;
            }
        }
    }
    n
}

/// ????????????????HUD ??????????
fn center_of(rgba: &[u8], pred: fn([u8; 4]) -> bool) -> f32 {
    let mut sum = 0f64;
    let mut count = 0usize;
    for y in (H / 2)..(H * 3 / 4) {
        for x in 0..W {
            if pred(pixel(rgba, W, x, y)) {
                sum += x as f64;
                count += 1;
            }
        }
    }
    if count == 0 {
        return f32::NAN;
    }
    (sum / count as f64) as f32
}

/// ??????????????????
fn truck_pixels(rgba: &[u8]) -> (usize, f32) {
    let mut count = 0usize;
    let mut sum = 0f64;
    for y in 0..H {
        for x in 0..W {
            if is_truck(pixel(rgba, W, x, y)) {
                count += 1;
                sum += x as f64;
            }
        }
    }
    let center = if count == 0 {
        0.0
    } else {
        (sum / count as f64) as f32
    };
    (count, center)
}
