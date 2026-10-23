//! D / +lx moves toward the right edge of the frame, not toward world +X.
//!
//! The default camera looks toward -Z, so +X is screen-right. A chase camera
//! looking along +Z has screen-right = -X. These checks do not need a GPU.

use kagra_shared::neon_run::NeonRun;
use kagra_shared::scene3d::screen_right_xyz;
use kagra_shared::world_doc::{WorldDoc, COORDINATE_NOTE};
use kagra_shared::world_play::WorldPlay;
use kagra_shared::WalkInput;

fn player_xz(doc: &WorldDoc) -> [f32; 2] {
    let p = doc.player.as_ref().or(doc.walkers.first()).expect("player");
    [p.position[0], p.position[2]]
}

fn right_xz(doc: &WorldDoc) -> [f32; 2] {
    let cam = doc.cameras.first().expect("camera");
    let right = screen_right_xyz(cam.position, cam.target);
    [right[0], right[2]]
}

fn strafe(json: &str, frames: i32) -> (f32, String) {
    let mut play = WorldPlay::from_json(json).expect("world");
    play.start();
    let before = player_xz(&play.doc);
    play.input = WalkInput {
        lx: 1.0,
        lz: 0.0,
        jump: false,
        attack: false,
        dodge: false,
    };
    for _ in 0..frames {
        play.tick(1.0 / 60.0);
    }
    let after = player_xz(&play.doc);
    let right = right_xz(&play.doc);
    let dot = (after[0] - before[0]) * right[0] + (after[1] - before[1]) * right[1];
    let detail = format!("from {before:?} to {after:?}, screen_right.xz={right:?}, dot={dot}");
    (dot, detail)
}

fn assert_strafe(label: &str, json: &str) {
    let (dot, detail) = strafe(json, 40);
    assert!(
        dot > 0.25,
        "{label}: Right/D (+lx) must move screen-right. {detail}"
    );
}

#[test]
fn dump_names_screen_left_and_screen_right() {
    let doc = WorldDoc::from_json(
        r#"{"version":1,"cameras":[{"id":"c","type":"camera","position":[0.0,2.0,6.0],"target":[0.0,0.0,0.0],"fov":60.0}]}"#,
    )
    .unwrap();
    let json = doc.to_json().unwrap();
    assert!(json.contains("\"screen_right\""));
    assert!(json.contains("\"screen_left\""));
    assert!(json.contains(COORDINATE_NOTE));
    let again = WorldDoc::from_json(&json).unwrap();
    assert_eq!(again.cameras[0].position, doc.cameras[0].position);
    assert_eq!(again.cameras[0].target, doc.cameras[0].target);
    let right = screen_right_xyz([0.0, 2.0, 6.0], [0.0, 0.0, 0.0]);
    assert!(
        right[0] > 0.99,
        "default camera +X is screen-right, got {right:?}"
    );
    let chase = screen_right_xyz([0.0, 2.0, -6.0], [0.0, 0.0, 0.0]);
    assert!(
        chase[0] < -0.99,
        "looking +Z, screen-right is -X, got {chase:?}"
    );
}

#[test]
fn examples_move_toward_screen_right() {
    assert_strafe("platformer", include_str!("fixtures/platformer_world.json"));
    assert_strafe("fps", include_str!("fixtures/fps_world.json"));
    assert_strafe("action", include_str!("fixtures/action_arena_world.json"));
    assert_strafe("action2d", include_str!("fixtures/action_side_world.json"));
    assert_strafe("fight", include_str!("fixtures/fight_hitstun_world.json"));
    assert_strafe("td", include_str!("fixtures/td_lane_world.json"));
    assert_strafe("sports", include_str!("fixtures/sports_world.json"));
    assert_strafe("stealth", include_str!("fixtures/stealth_world.json"));
    assert_strafe("survival", include_str!("fixtures/survival_world.json"));
}

#[test]
fn neon_right_moves_toward_screen_right() {
    let mut run = NeonRun::from_json(r#"{"version":1,"genre":"neon_run"}"#).unwrap();
    run.confirm();
    let before = ship_x(run.document());
    run.set_steer(1.0, 0.0, false);
    for _ in 0..20 {
        run.tick(1.0 / 60.0);
    }
    let after = ship_x(run.document());
    let right = right_xz(run.document());
    let dot = (after - before) * right[0];
    assert!(
        dot > 0.15,
        "neon D/+steer must move screen-right, x {before} -> {after}, right.xz={right:?}"
    );
    let state = run.state_json();
    assert!(state.contains("screen_right"), "{state}");
    assert!(state.contains("screen_left"), "{state}");
}

fn ship_x(doc: &WorldDoc) -> f32 {
    doc.props
        .iter()
        .find(|p| p.name == "ship-core")
        .expect("ship")
        .position[0]
}
