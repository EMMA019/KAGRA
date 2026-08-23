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
