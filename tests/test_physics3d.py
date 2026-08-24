"""Physics3D — GPU 不要。カプセル / OBB / レイヤー / トリガー。"""
from __future__ import annotations

import math

import pytest

from tests.conftest import load_kagra_submodule


def _phys():
    return load_kagra_submodule("physics3d")


def test_aabb_still_blocks():
    m = _phys()
    p = m.Physics3D(gravity=0)
    p.set_ground_y(-10.0)
    player = p.add_body(0, 1.0, 0, 0.4, 1.8, 0.4)
    player.use_gravity = False
    p.add_body(1.0, 0, 0, 0.5, 2.0, 2.0, is_static=True)
    player.vx = 4.0
    for _ in range(40):
        p.update(0.016)
    assert player.x < 0.75


def test_capsule_stopped_by_static_aabb():
    m = _phys()
    p = m.Physics3D(gravity=0)
    p.set_ground_y(-10.0)
    player = p.add_capsule(0, 0, 0, radius=0.3, height=1.8)
    player.use_gravity = False
    p.add_body(1.05, 0, 0, 0.4, 2.0, 2.0, is_static=True)
    player.vx = 5.0
    for _ in range(40):
        p.update(0.016)
    assert player.x < 0.85


def test_capsule_lands_on_box():
    m = _phys()
    p = m.Physics3D(gravity=20.0)
    p.set_ground_y(-10.0)
    player = p.add_capsule(0, 2.4, 0, radius=0.25, height=1.6)
    p.add_body(0, 0, 0, 2.0, 1.0, 2.0, is_static=True)
    for _ in range(80):
        p.update(0.016)
    assert player.y >= 0.95
    assert player.on_ground


def test_obb_blocks_at_yaw():
    m = _phys()
    p = m.Physics3D(gravity=0)
    p.set_ground_y(-10.0)
    player = p.add_capsule(0, 0, 0, radius=0.25, height=1.6)
    player.use_gravity = False
    # yaw=0 の OBB は AABB と同じく正面で止める
    p.add_obb(1.1, 0, 0, 0.4, 2.0, 2.0, yaw=0.0, is_static=True)
    player.vx = 4.0
    for _ in range(40):
        p.update(0.016)
    assert player.x < 0.95


def test_yaw_obb_pushes_capsule_out():
    m = _phys()
    p = m.Physics3D(gravity=0)
    p.set_ground_y(-10.0)
    player = p.add_capsule(1.2, 0, 0, radius=0.25, height=1.6)
    player.use_gravity = False
    wall = p.add_obb(1.2, 0, 0, 0.4, 2.0, 2.0, yaw=math.pi / 4, is_static=True)
    p.update(0.016)
    # 中心に埋め込んだら外へ出る
    dist = math.hypot(player.x - wall.x, player.z - wall.z)
    assert dist > 0.15


def test_trigger_does_not_resolve():
    m = _phys()
    p = m.Physics3D(gravity=0)
    p.set_ground_y(-10.0)
    player = p.add_body(0, 1.0, 0, 0.4, 1.0, 0.4)
    player.use_gravity = False
    hits = []
    zone = p.add_body(1.0, 0, 0, 1.0, 2.0, 1.0, is_static=True, trigger=True)
    zone.on_collide = lambda other, kind: hits.append(kind)
    player.vx = 3.0
    for _ in range(40):
        p.update(0.016)
    assert player.x > 1.2
    assert "trigger" in hits
    assert "hit" not in hits


def test_layer_mask_skips():
    m = _phys()
    p = m.Physics3D(gravity=0)
    p.set_ground_y(-10.0)
    player = p.add_body(0, 1.0, 0, 0.4, 1.0, 0.4, layer=1, mask=1)
    player.use_gravity = False
    p.add_body(0.6, 1.0, 0, 0.4, 1.0, 0.4, is_static=True, layer=2, mask=2)
    player.vx = 3.0
    for _ in range(30):
        p.update(0.016)
    assert player.x > 1.0


def test_layer_mask_hits_when_both_match():
    m = _phys()
    p = m.Physics3D(gravity=0)
    p.set_ground_y(-10.0)
    player = p.add_body(0, 1.0, 0, 0.4, 1.0, 0.4, layer=1, mask=2)
    player.use_gravity = False
    p.add_body(0.7, 1.0, 0, 0.4, 1.0, 0.4, is_static=True, layer=2, mask=1)
    player.vx = 3.0
    for _ in range(30):
        p.update(0.016)
    assert player.x < 0.6


def test_raycast_capsule():
    m = _phys()
    p = m.Physics3D(gravity=0)
    cap = p.add_capsule(0, 0, 2, radius=0.3, height=1.6)
    hit = p.raycast(0, 0.8, 0, 0, 0, 1, max_dist=10)
    assert hit is not None
    body, dist, *_ = hit
    assert body is cap
    assert 1.4 < dist < 2.0


def test_raycast_obb():
    m = _phys()
    p = m.Physics3D(gravity=0)
    wall = p.add_obb(0, 0, 3, 2.0, 2.0, 0.3, yaw=0.3, is_static=True)
    hit = p.raycast(0, 1.0, 0, 0, 0, 1, max_dist=20)
    assert hit is not None
    assert hit[0] is wall


def test_sync_vrm_reads_avatar_id():
    m = _phys()
    p = m.Physics3D()
    body = p.add_body(1.5, 0.2, -3.0, 0.4, 1.8, 0.4)

    class Av:
        vrm_id = 7

    # エンジンなしでも落ちない
    p.sync_vrm(body, Av())
    p.sync_vrm(body, 3)


def test_existing_ground_and_bounce():
    m = _phys()
    p = m.Physics3D(gravity=9.8)
    p.set_ground_y(0.0)
    b = p.add_body(0, 1.0, 0, 0.4, 1.0, 0.4, restitution=0.0)
    for _ in range(80):
        p.update(0.016)
    assert abs(b.y) < 0.05
    assert b.on_ground


def test_capsule_squeezes_past_sphere_aabb_corner():
    m = _phys()
    p = m.Physics3D(gravity=0)
    p.set_ground_y(-10.0)
    player = p.add_capsule(2.0, 0.0, 2.0, radius=0.28, height=1.6)
    player.use_gravity = False
    player.friction = 0.0
    p.add_sphere(0.0, 0.0, 0.0, 0.5)
    player.vx = player.vz = -3.0
    for _ in range(80):
        p.update(0.016)
    dist = math.hypot(player.x, player.z)
    assert dist < 0.92
    assert dist > 0.70


def test_capsule_blocked_by_sphere_head_on():
    m = _phys()
    p = m.Physics3D(gravity=0)
    p.set_ground_y(-10.0)
    player = p.add_capsule(2.0, 0.0, 0.0, radius=0.28, height=1.6)
    player.use_gravity = False
    p.add_sphere(0.0, 0.0, 0.0, 0.5)
    player.vx = -4.0
    for _ in range(50):
        p.update(0.016)
    assert player.x > 0.70


def test_capsule_squeezes_past_cylinder_aabb_corner():
    m = _phys()
    p = m.Physics3D(gravity=0)
    p.set_ground_y(-10.0)
    player = p.add_capsule(2.0, 0.0, 2.0, radius=0.28, height=1.6)
    player.use_gravity = False
    player.friction = 0.0
    p.add_cylinder(0.0, 0.0, 0.0, 0.5, 2.0)
    player.vx = player.vz = -3.0
    for _ in range(80):
        p.update(0.016)
    dist = math.hypot(player.x, player.z)
    assert dist < 0.92
    assert dist > 0.70


def test_ray_hits_sphere_and_cylinder_cap():
    m = _phys()
    p = m.Physics3D()
    ball = p.add_sphere(0.0, 0.0, 0.0, 0.5)
    hit = p.raycast(0.0, 0.5, 3.0, 0.0, 0.0, -1.0)
    assert hit is not None and hit[0] is ball
    cyl = p.add_cylinder(4.0, 0.0, 0.0, 0.4, 1.2)
    cap = p.raycast(4.0, 3.0, 0.0, 0.0, -1.0, 0.0)
    assert cap is not None and cap[0] is cyl


def test_height_fn_lands():
    m = _phys()
    p = m.Physics3D(gravity=20.0)
    p.set_height_fn(lambda _x, _z: 2.0)
    player = p.add_capsule(0, 5.0, 0, 0.25, 1.6)
    for _ in range(80):
        p.update(0.016)
    assert player.y == pytest.approx(2.0, abs=0.05)
    assert player.on_ground


def test_height_fn_cliff_blocks():
    m = _phys()
    p = m.Physics3D(gravity=0.0)
    p.set_height_fn(lambda x, _z: 5.0 if x > 0.4 else 0.0)
    player = p.add_capsule(0.0, 0.0, 0.0, 0.25, 1.6)
    player.use_gravity = False
    player.friction = 0.0
    player.vx = 4.0
    for _ in range(40):
        p.update(0.016)
    assert player.x < 0.55


def test_height_normal_on_ramp():
    m = _phys()
    nx, ny, nz = m.height_normal(lambda x, _z: 0.5 * x, 0.0, 0.0)
    assert nx < -0.3
    assert ny > 0.8
    assert abs(nz) < 0.05


def test_walkable_ramp_follows_slope():
    m = _phys()
    p = m.Physics3D(gravity=20.0)
    p.set_height_fn(lambda x, _z: 0.4 * x)
    player = p.add_capsule(1.0, 0.4, 0.0, 0.25, 1.6)
    player.friction = 0.0
    for _ in range(20):
        p.update(0.016)
    assert player.on_ground
    player.vx = 3.0
    p.update(0.016)
    assert player.vy > 0.2
    assert player.x > 1.02


def test_steep_ramp_slides_down():
    m = _phys()
    p = m.Physics3D(gravity=20.0)
    p.set_height_fn(lambda x, _z: 2.6 * x)
    player = p.add_capsule(2.0, 5.3, 0.0, 0.25, 1.6)
    player.friction = 0.0
    for _ in range(20):
        p.update(0.016)
    x0 = player.x
    for _ in range(50):
        player.vx = 0.0
        player.vz = 0.0
        p.update(0.016)
    assert player.x < x0 - 0.25
    assert player.y < 2.6 * x0 - 0.35


def test_steep_ramp_blocks_walk_up():
    m = _phys()
    p = m.Physics3D(gravity=20.0)
    p.set_height_fn(lambda x, _z: 2.6 * x)
    player = p.add_capsule(0.6, 1.6, 0.0, 0.25, 1.6)
    player.friction = 0.0
    for _ in range(20):
        p.update(0.016)
    for _ in range(40):
        player.vx = 4.0
        p.update(0.016)
    assert player.x < 1.15


def test_stairs_are_climbable():
    land = load_kagra_submodule("land")

    def fn(x, z):
        s = land.stair_y(x, z, x0=-1, x1=1, z0=0, z1=3.2, y0=0.0, y1=1.8, steps=6)
        return 0.0 if s is None else s

    m = _phys()
    p = m.Physics3D(gravity=20.0)
    p.set_height_fn(fn)
    player = p.add_capsule(0.0, 0.0, 0.1, 0.25, 1.6)
    player.friction = 0.0
    for _ in range(80):
        player.vx = 0.0
        player.vz = 3.0
        p.update(0.016)
    assert player.z > 2.2
    assert player.y > 1.2


def test_jump_not_killed_by_slope():
    m = _phys()
    p = m.Physics3D(gravity=20.0)
    p.set_height_fn(lambda x, _z: 0.2 * x)
    player = p.add_capsule(1.0, 0.2, 0.0, 0.25, 1.6)
    for _ in range(20):
        p.update(0.016)
    gy = 0.2 * player.x
    player.vy = 6.0
    p.update(0.016)
    assert player.y > gy + 0.04
    assert player.vy > 4.5


def test_water_buoyancy_lifts():
    m = _phys()
    p = m.Physics3D(gravity=12.0)
    p.set_ground_y(-8.0)
    p.set_water_y(1.0)
    player = p.add_capsule(0.0, -1.5, 0.0, 0.25, 1.6)
    for _ in range(90):
        p.update(0.016)
    assert player.y > -0.4
    assert player.y < 3.0
