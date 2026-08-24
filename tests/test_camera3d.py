"""Camera3D.ray_from_screen — GPU 不要。"""
from __future__ import annotations

from tests.conftest import load_kagra_submodule


def _cam():
    return load_kagra_submodule("camera3d")


def test_center_ray_points_at_target():
    m = _cam()
    cam = m.Camera3D(800, 600, fov_deg=45.0)
    cam.position = (0.0, 1.0, 3.0)
    cam.target = (0.0, 1.0, 0.0)
    cam.up = (0.0, 1.0, 0.0)
    ray = cam.ray_from_screen(400, 300)
    assert ray is not None
    origin, direction = ray
    # ニア面上の点。カメラ後方には出ない
    assert origin[2] < 3.0
    assert direction[2] < -0.85
    assert abs(direction[0]) < 0.08
    assert abs(direction[1]) < 0.08


def test_right_of_center_has_positive_x():
    m = _cam()
    cam = m.Camera3D(800, 600, fov_deg=45.0)
    cam.position = (0.0, 1.0, 3.0)
    cam.target = (0.0, 1.0, 0.0)
    left = cam.ray_from_screen(100, 300)
    right = cam.ray_from_screen(700, 300)
    assert left is not None and right is not None
    assert left[1][0] < 0.0
    assert right[1][0] > 0.0


def test_mat4_inv_roundtrip():
    m = _cam()
    ident = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    inv = m._mat4_inv(ident)
    assert inv is not None
    assert all(abs(a - b) < 1e-6 for a, b in zip(inv, ident))


def test_showcase_moves_from_body_to_face():
    m = _cam()
    cam = m.Camera3D(1280, 720, fov_deg=32.0)
    cam.use_showcase(cut_period=4.0, blend_sec=0.5, orbit_speed=0.0)
    body_r = cam.orbit_r
    body_y = cam.orbit_tgt[1]
    cam.showcase_tick(4.6)
    assert cam.orbit_r < body_r
    assert cam.orbit_tgt[1] > body_y
    assert cam.fov_deg < 32.0


def test_world_to_screen_puts_target_near_center():
    m = _cam()
    cam = m.Camera3D(800, 600, fov_deg=45.0)
    cam.position = (0.0, 1.0, 3.0)
    cam.target = (0.0, 1.0, 0.0)
    cam.up = (0.0, 1.0, 0.0)
    hit = cam.world_to_screen(0.0, 1.0, 0.0)
    assert hit is not None
    sx, sy = hit
    assert abs(sx - 400) < 8
    assert abs(sy - 300) < 8


def test_world_to_screen_behind_camera_is_none():
    m = _cam()
    cam = m.Camera3D(800, 600, fov_deg=45.0)
    cam.position = (0.0, 1.0, 3.0)
    cam.target = (0.0, 1.0, 0.0)
    assert cam.world_to_screen(0.0, 1.0, 8.0) is None


def test_use_orbit_clears_showcase():
    m = _cam()
    cam = m.Camera3D(800, 600)
    cam.use_showcase()
    cam.use_orbit(radius=3.0, target=(0, 0.9, 0))
    assert cam._showcase is False
    cam.showcase_tick(10.0)
    assert cam.orbit_r == 3.0


def test_follow_snaps_behind_target():
    m = _cam()
    cam = m.Camera3D(800, 600)
    cam.follow(0.0, 0.0, 0.0, distance=4.0, height=2.0, look_y=1.0, lerp=1.0, yaw=0.0)
    assert cam._orbit is False
    assert cam._follow is True
    assert abs(cam.position[2] - (-4.0)) < 1e-6
    assert abs(cam.position[1] - 2.0) < 1e-6
    assert cam.target[1] == 1.0


def test_look_clears_orbit_and_sets_eye():
    m = _cam()
    cam = m.Camera3D(800, 600)
    cam.use_orbit(radius=3.0)
    cam.look(0.0, 1.55, 0.0, 0.0, 1.55, 1.0)
    assert cam._orbit is False
    assert cam._follow is False
    assert cam.position == (0.0, 1.55, 0.0)
    assert cam.target == (0.0, 1.55, 1.0)


def test_follow_behind_when_facing_neg_z():
    """Pretty Room yaw=π: camera sits +Z of the player, not inside the mesh."""
    import math

    m = _cam()
    cam = m.Camera3D(960, 540)
    cam.follow(
        0.0, 0.0, 2.4,
        distance=3.2, height=1.7, look_y=1.15, lerp=1.0, yaw=math.pi,
    )
    assert abs(cam.position[0]) < 1e-6
    assert abs(cam.position[1] - 1.7) < 1e-6
    assert abs(cam.position[2] - 5.6) < 1e-6
    assert abs(cam.position[2] - 2.4) > 2.5
    assert cam.target[2] == 2.4


def test_follow_lerps_toward_target():
    m = _cam()
    cam = m.Camera3D(800, 600)
    cam.position = (0.0, 1.0, 3.0)
    cam.target = (0.0, 1.0, 0.0)
    before = cam.position
    cam.follow(2.0, 0.0, -1.0, lerp=0.5, yaw=0.0)
    assert cam.position != before
    assert cam.position[0] != 2.0


def test_follow_bounds_half_keeps_eye_inside_room():
    """Switch Room spawn: default distance would put the eye past half=5.6."""
    import math

    m = _cam()
    cam = m.Camera3D(960, 540)
    cam.follow(
        0.0, 0.0, 3.4,
        distance=4.8, height=1.9, look_y=1.0, lerp=1.0, yaw=math.pi,
        bounds_half=5.6,
    )
    assert abs(cam.position[2]) <= 5.6 - 0.14
    assert cam.position[2] > 3.4
