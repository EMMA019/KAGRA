//! ????????????????
//!
//! GPU ??? CI ?????????????????????????????
//! ????????????????????????????????????????

#![cfg(feature = "render")]

use std::sync::{Mutex, OnceLock};

use kagra_shared::render::Renderer;
use kagra_shared::scene::DrawList;
use kagra_shared::scene3d::{Batch, Camera, Instance, Material, MeshId, Scene3D};
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
        // ステア符号の検証では交通・建物が道路画素を汚す／弾くので外す。
        session.driving.traffic = kagra_shared::TrafficSystem::disabled();
        session.driving.collide_scenery = false;
        session.driving.show_buildings = false;
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

fn unique_rgb(rgba: &[u8]) -> usize {
    let mut set = std::collections::HashSet::new();
    for chunk in rgba.chunks(4) {
        set.insert([chunk[0], chunk[1], chunk[2]]);
    }
    set.len()
}

fn render_world_fixture(json: &str) -> Option<Vec<u8>> {
    let _guard = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let doc = kagra_shared::WorldDoc::from_json(json).expect("parse dump");
    kagra_shared::render_world_doc(&doc, W, H).ok()
}

/// ポスト（threshold bloom）が明るい 3D オブジェクトの周囲に光をにじませる。
/// 3D フレームは線形 HDR（トーン前）なので、白 box（リニア 1.0）は閾値 0.85
/// を超えて抽出される。intensity 0 なら外側は黒のまま。
#[test]
fn bloom_spills_light_around_bright_quad() {
    let _guard = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(mut renderer) = pollster::block_on(Renderer::new_offscreen(W, H)) else {
        eprintln!("no GPU adapter; skipping bloom test");
        return;
    };
    // compile_meshes は heightfield 無しだと id が飛ぶ（dense チェックに
    // 引っかかる）ので、box だけを個別に登録する。
    let all = kagra_shared::world_doc::compile_meshes();
    for (id, mesh) in all {
        if id.0 == 0 {
            let got = renderer.upload_mesh(&mesh);
            assert_eq!(got, MeshId(0), "box must be the first mesh");
        }
    }
    // 白 box（Solid、ambient=1 → リニア 1.0）を画面中央に。ibl 0 で IBL を外し、
    // クリアは黒。
    let scene = Scene3D {
        camera: Camera {
            eye: glam::Vec3::new(0.0, 0.0, 6.0),
            target: glam::Vec3::ZERO,
            up: glam::Vec3::Y,
            fov_y: 60f32.to_radians(),
            near: 0.1,
            far: 100.0,
        },
        clear: [0, 0, 0, 255],
        ambient: 1.0,
        ibl: 0.0,
        batches: vec![Batch {
            mesh: MeshId(0),
            instances: vec![Instance {
                model: glam::Mat4::from_translation(glam::Vec3::ZERO),
                color: [255, 255, 255, 255],
                material: Material::Solid,
            }],
        }],
        ..Default::default()
    };
    let list = DrawList::default();
    renderer
        .render_frame(Some(&scene), &list)
        .expect("off frame");
    let off = renderer.read_rgba().expect("off readback");
    renderer.set_bloom(0.85, 0.6);
    renderer
        .render_frame(Some(&scene), &list)
        .expect("bloom frame");
    let on = renderer.read_rgba().expect("bloom readback");
    // box の縁から数 px 外で、bloom が off より明るいピクセルが存在する。
    let mut spilled = false;
    for dx in 8..28u32 {
        let x = W / 2 + dx;
        let po = pixel(&off, W, x, H / 2);
        let pn = pixel(&on, W, x, H / 2);
        if pn[0] as i32 > po[0] as i32 + 6 {
            spilled = true;
            break;
        }
    }
    assert!(spilled, "bloom must brighten pixels outside the box");
    // box の中心は白いまま（にじみで濁らない）。
    let inside = pixel(&on, W, W / 2, H / 2);
    assert!(inside[0] > 200, "box core stays bright, got {inside:?}");
}

/// FXAA が白 box の縁に中間色を作る（ジャギー除去）。無効なら縁は白か黒のみ。
#[test]
fn fxaa_smooths_edges_with_intermediate_colors() {
    let _guard = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(mut renderer) = pollster::block_on(Renderer::new_offscreen(W, H)) else {
        eprintln!("no GPU adapter; skipping fxaa test");
        return;
    };
    let all = kagra_shared::world_doc::compile_meshes();
    for (id, mesh) in all {
        if id.0 == 0 {
            let got = renderer.upload_mesh(&mesh);
            assert_eq!(got, MeshId(0));
        }
    }
    let scene = Scene3D {
        camera: Camera {
            eye: glam::Vec3::new(0.0, 0.0, 6.0),
            target: glam::Vec3::ZERO,
            up: glam::Vec3::Y,
            fov_y: 60f32.to_radians(),
            near: 0.1,
            far: 100.0,
        },
        clear: [0, 0, 0, 255],
        ambient: 1.0,
        ibl: 0.0,
        batches: vec![Batch {
            mesh: MeshId(0),
            instances: vec![Instance {
                model: glam::Mat4::from_translation(glam::Vec3::ZERO),
                color: [255, 255, 255, 255],
                material: Material::Solid,
            }],
        }],
        ..Default::default()
    };
    let list = DrawList::default();
    renderer.set_bloom(0.85, 0.0); // bloom なし。FXAA のみを検証。
    renderer.set_fxaa(true);
    renderer
        .render_frame(Some(&scene), &list)
        .expect("fxaa on frame");
    let on = renderer.read_rgba().expect("fxaa on readback");
    renderer.set_fxaa(false);
    renderer
        .render_frame(Some(&scene), &list)
        .expect("fxaa off frame");
    let off = renderer.read_rgba().expect("fxaa off readback");

    // box の縁（1x1x1 が距離 6 で中心 ±~7px）を横断する中間色ピクセルを数える。
    // ACES 後の白は ~232 なので、真の「エッジ中間色」は 50..220 で判定。
    let count_mid = |rgba: &[u8]| {
        (50..80u32)
            .filter(|&x| {
                let p = pixel(rgba, W, x, H / 2);
                (50..220).contains(&p[0])
            })
            .count()
    };
    let mid_on = count_mid(&on);
    let mid_off = count_mid(&off);
    assert!(
        mid_on > mid_off,
        "FXAA must create intermediate edge pixels, on={mid_on} off={mid_off}"
    );
    // box の中心は白のまま（エッジだけ滑らかにする）。
    let center = pixel(&on, W, W / 2, H / 2);
    assert!(center[0] > 200, "box core stays white, got {center:?}");
}

/// Compiled WorldDoc through wgpu 30 offscreen (no kagra-core window).
/// No adapter → skip. GPU-free roundtrip tests live in world_doc.rs.
#[test]
fn world_doc_offscreen_crest_isle_is_not_flat() {
    let json = include_str!("fixtures/crest_isle_world.json");
    let Some(rgba) = render_world_fixture(json) else {
        eprintln!("no GPU adapter; skipping WorldDoc offscreen test");
        return;
    };
    assert_eq!(rgba.len() as u32, W * H * 4);
    let sky = pixel(&rgba, W, W / 2, 2);
    assert!(
        sky[2] > sky[0],
        "expected sky-ish clear at the top, got {sky:?}"
    );
    let colors = unique_rgb(&rgba);
    assert!(
        colors > 4,
        "compiled WorldDoc should shade more than a clear color (got {colors} unique)"
    );
}

#[test]
fn world_doc_offscreen_orb_rush_draws_batches() {
    let json = include_str!("fixtures/orb_rush_world.json");
    let Some(rgba) = render_world_fixture(json) else {
        eprintln!("no GPU adapter; skipping WorldDoc orb offscreen test");
        return;
    };
    assert_eq!(rgba.len() as u32, W * H * 4);
    let first = pixel(&rgba, W, 0, 0);
    assert!(
        rgba.chunks(4).any(|p| p != first.as_slice()),
        "frame is a flat color; compile_scene batches were not drawn"
    );
    let colors = unique_rgb(&rgba);
    assert!(
        colors > 4,
        "orb dump should draw walker/props, got {colors} unique"
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
