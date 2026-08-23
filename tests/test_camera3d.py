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


def test_use_orbit_clears_showcase():
    m = _cam()
    cam = m.Camera3D(800, 600)
    cam.use_showcase()
    cam.use_orbit(radius=3.0, target=(0, 0.9, 0))
    assert cam._showcase is False
    cam.showcase_tick(10.0)
    assert cam.orbit_r == 3.0
