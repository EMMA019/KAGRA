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


def test_follow_clips_eye_in_front_of_wall():
    """Player → camera segment vs a static box pulls the eye in."""
    from tests.conftest import load_kagra_submodule as load

    w3 = load("world3d")
    world = w3.World3D(half=6.0)
    world.add_box(0.0, 0.0, -2.0, 8.0, 4.0, 0.4)
    world.add_player(0.0, 0.0)
    m = _cam()
    cam = m.Camera3D(800, 600)
    cam.follow(0.0, 0.0, 0.0, distance=4.8, height=2.4, look_y=1.0, lerp=1.0, yaw=0.0)
    raw_z = cam.position[2]
    cam.follow(
        0.0, 0.0, 0.0,
        distance=4.8, height=2.4, look_y=1.0, lerp=1.0, yaw=0.0, world=world,
    )
    assert raw_z < -4.0
    assert cam.position[2] > raw_z
    assert cam.position[2] > -2.0


def test_follow_boxed_room_corner_stays_inside():
    """Switch-style walls: default distance would sit past the +Z wall."""
    import sys
    from pathlib import Path

    from tests.conftest import load_kagra_submodule as load

    examples = Path(__file__).resolve().parents[1] / "examples"
    if str(examples) not in sys.path:
        sys.path.insert(0, str(examples))
    from switch_room_rules import ARENA_HALF, START_XZ, walls

    w3 = load("world3d")
    world = w3.World3D(half=6.0)
    for spec in walls():
        world.add_box(*spec)
    world.add_player(*START_XZ)
    m = _cam()
    cam = m.Camera3D(960, 540)
    import math

    cam.follow(
        START_XZ[0], 0.0, START_XZ[1],
        distance=4.8, height=1.9, look_y=1.0, lerp=1.0, yaw=math.pi,
        world=world,
    )
    assert abs(cam.position[0]) < ARENA_HALF
    assert abs(cam.position[2]) < ARENA_HALF
    assert cam.position[2] > START_XZ[1]
    # closer than the unclipped 3.4+4.8=8.2
    assert cam.position[2] < START_XZ[1] + 4.0


def test_clip_eye_no_hit_keeps_dest():
    m = _cam()
    w3 = load_kagra_submodule("world3d")
    world = w3.World3D(half=6.0)
    dest = m.clip_eye((0.0, 1.0, 0.0), (0.0, 2.4, -4.8), world)
    assert dest == (0.0, 2.4, -4.8)


def test_clamp_eye_rejects_skull_and_tiny_speck():
    m = _cam()
    origin = (0.0, 1.25, 0.0)
    inside = m.clamp_eye(origin, (0.0, 1.3, -0.2), min_distance=6.0, max_distance=12.2)
    d_in = ((inside[0] - origin[0]) ** 2 + (inside[1] - origin[1]) ** 2 + (inside[2] - origin[2]) ** 2) ** 0.5
    assert abs(d_in - 6.0) < 1e-4
    far = m.clamp_eye(origin, (0.0, 20.0, 80.0), min_distance=6.0, max_distance=12.2)
    d_far = ((far[0] - origin[0]) ** 2 + (far[1] - origin[1]) ** 2 + (far[2] - origin[2]) ** 2) ** 0.5
    assert abs(d_far - 12.2) < 1e-4


def test_follow_lerp_from_far_stays_in_chase_band():
    """Hitch left the eye hundreds of metres back (tiny speck)."""
    m = _cam()
    cam = m.Camera3D(960, 540)
    cam.position = (0.0, 80.0, -200.0)
    cam.target = (0.0, 1.0, 0.0)
    cam.follow(
        0.0, 0.0, 0.0,
        distance=12.2, height=4.4, look_y=1.25, lerp=0.22, yaw=0.0,
        min_distance=6.0, max_distance=12.2,
    )
    ox, oy, oz = cam.target
    px, py, pz = cam.position
    dist = ((px - ox) ** 2 + (py - oy) ** 2 + (pz - oz) ** 2) ** 0.5
    assert 6.0 - 1e-4 <= dist <= 12.2 + 1e-4


def test_clip_eye_ignores_collider_inside_min_hit():
    """A tree AABB overlapping the avatar must not slam the cam into the skull."""
    w3 = load_kagra_submodule("world3d")
    world = w3.World3D(half=12.0)
    world.add_box(0.0, 0.0, -0.4, 1.2, 2.4, 1.2)
    world.add_player(0.0, 0.0)
    m = _cam()
    dest = (0.0, 4.4, -12.2)
    kept = m.clip_eye((0.0, 1.25, 0.0), dest, world, min_hit=6.0)
    assert kept == dest


def test_follow_inside_head_stays_outside_vrm_skull():
    """Wall-clip dest 5cm from look-at must not leave the eye in the hair."""
    m = _cam()
    cam = m.Camera3D(960, 540)
    cam.position = (0.0, 1.3, -0.15)
    cam.target = (0.0, 1.25, 0.0)
    cam.follow(
        0.0, 0.0, 0.0,
        distance=12.2, height=4.4, look_y=1.25, lerp=1.0, yaw=0.0,
        min_distance=6.0, max_distance=12.6,
    )
    ox, oy, oz = cam.target
    px, py, pz = cam.position
    dist = ((px - ox) ** 2 + (py - oy) ** 2 + (pz - oz) ** 2) ** 0.5
    assert dist >= 6.0 - 1e-4
    assert pz < oz  # still behind the look-at


def test_follow_zoom_does_not_explode_distance():
    """Mouse-wheel orbit zoom must not push a chase cam into fog-white."""
    m = _cam()
    cam = m.Camera3D(960, 540)
    cam.follow(
        0.0, 0.0, 0.0,
        distance=12.2, height=4.4, look_y=1.25, lerp=1.0, yaw=0.0,
        min_distance=6.0, max_distance=12.6,
    )
    before = cam.position
    cam.zoom(80.0)
    assert cam.position == before
    cam.position = (0.0, 80.0, -200.0)
    class _Eng:
        def update_camera_3d(self, *_a, **_k):
            return None
    cam.update(_Eng())
    ox, oy, oz = cam.target
    px, py, pz = cam.position
    dist = ((px - ox) ** 2 + (py - oy) ** 2 + (pz - oz) ** 2) ** 0.5
    assert 6.0 - 1e-4 <= dist <= 12.6 + 1e-4
