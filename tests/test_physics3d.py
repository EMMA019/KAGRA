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


def test_raycast_ignore_skips_body():
    m = _phys()
    p = m.Physics3D(gravity=0)
    cap = p.add_capsule(0, 0, 1, radius=0.3, height=1.6)
    wall = p.add_body(0, 0, 4, 2.0, 2.0, 0.4, is_static=True)
    hit = p.raycast(0, 0.8, 0, 0, 0, 1, max_dist=20)
    assert hit is not None and hit[0] is cap
    hit = p.raycast(0, 0.8, 0, 0, 0, 1, max_dist=20, ignore=cap)
    assert hit is not None and hit[0] is wall


def test_raycast_static_only_skips_dynamic():
    m = _phys()
    p = m.Physics3D(gravity=0)
    p.add_body(0, 0, 2, 1.0, 1.0, 1.0, is_static=False)
    wall = p.add_body(0, 0, 5, 2.0, 2.0, 0.4, is_static=True)
    hit = p.raycast(0, 0.5, 0, 0, 0, 1, max_dist=20, static_only=True)
    assert hit is not None and hit[0] is wall
    hit = p.raycast(0, 0.5, 0, 0, 0, 1, max_dist=20)
    assert hit is not None and hit[0].is_static is False


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
    peak_on_stair = -999.0
    for _ in range(80):
        player.vx = 0.0
        player.vz = 3.0
        p.update(0.016)
        if 0.0 <= player.z <= 3.2:
            peak_on_stair = max(peak_on_stair, player.y)
    assert player.z > 2.2
    assert peak_on_stair > 1.2
    assert player.y > -0.1


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


def test_capsule_stands_on_trimesh():
    m = _phys()
    p = m.Physics3D(gravity=20.0)
    p.set_ground_y(-8.0)
    verts = [
        [-2.0, 0.7, -2.0],
        [2.0, 0.7, -2.0],
        [2.0, 0.7, 2.0],
        [-2.0, 0.7, 2.0],
    ]
    p.add_trimesh(verts, [0, 1, 2, 0, 2, 3])
    player = p.add_capsule(0.0, 2.2, 0.0, 0.25, 1.6)
    for _ in range(80):
        p.update(0.016)
    assert player.y == pytest.approx(0.7, abs=0.15)
    assert player.on_ground
    assert player.y > -1.0


def test_capsule_does_not_fall_through_ramp():
    m = _phys()
    p = m.Physics3D(gravity=20.0)
    p.set_ground_y(-8.0)
    verts = [
        [0.0, 0.0, -1.5],
        [4.0, 1.6, -1.5],
        [4.0, 1.6, 1.5],
        [0.0, 0.0, 1.5],
    ]
    p.add_trimesh(verts, [0, 1, 2, 0, 2, 3])
    player = p.add_capsule(2.0, 1.1, 0.0, 0.25, 1.6)
    for _ in range(50):
        p.update(0.016)
    assert player.y > -1.0
    assert player.y > 0.15


def test_ray_hits_trimesh():
    m = _phys()
    p = m.Physics3D()
    mesh = p.add_trimesh(
        [[0, 0, 2], [1, 0, 2], [0, 1, 2]],
        [0, 1, 2],
    )
    hit = p.raycast(0.2, 0.2, 0.0, 0.0, 0.0, 1.0, max_dist=10)
    assert hit is not None
    assert hit[0] is mesh
    assert 1.5 < hit[1] < 2.2


def test_capsule_stands_on_dynamic_box():
    """積んで寝た箱の上にカプセルが乗る。箱は沈まない。"""
    m = _phys()
    p = m.Physics3D(gravity=22.0)
    p.set_ground_y(0.0)
    crate = p.add_body(0.0, 1.2, 0.0, 1.2, 0.5, 1.2)
    for _ in range(160):
        p.update(0.016)
    assert crate.y == pytest.approx(0.0, abs=0.12)
    assert crate.sleeping or abs(crate.vy) < 0.2
    top = crate.y + crate.h
    player = p.add_capsule(0.0, top + 1.4, 0.0, radius=0.25, height=1.6)
    for _ in range(160):
        p.update(0.016)
    assert player.on_ground
    assert player.y == pytest.approx(top, abs=0.12)
    assert crate.y == pytest.approx(0.0, abs=0.12)


def test_boxes_stack_and_sleep():
    m = _phys()
    p = m.Physics3D(gravity=22.0)
    p.set_ground_y(0.0)
    low = p.add_body(0.0, 0.5, 0.0, 0.8, 0.4, 0.8)
    high = p.add_body(0.0, 1.2, 0.0, 0.8, 0.4, 0.8)
    for _ in range(180):
        p.update(0.016)
    assert low.y == pytest.approx(0.0, abs=0.1)
    assert high.y > low.y + 0.25
    assert high.y < 1.05
    assert abs(high.vy) < 0.55


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


def test_height_support_plane_does_not_fat_lift():
    """Linear slope: plane snap ≈ center, fat max-Y lifts by grade * radius."""
    m = _phys()
    fn = lambda x, _z: 0.4 * x
    h_center = fn(1.0, 0.0)
    plane, _n = m.height_support(fn, 1.0, 0.0, foot_radius=0.08, snap_to_plane=True)
    fat, _n = m.height_support(
        fn, 1.0, 0.0, foot_radius=0.28, snap_to_plane=False,
    )
    assert abs(plane - h_center) < m.GROUNDED_FLOAT
    assert fat - h_center == pytest.approx(0.4 * 0.28, abs=0.001)


def test_cliff_beside_foot_is_ignored():
    m = _phys()
    fn = lambda x, _z: 5.0 if x > 0.05 else 0.0
    y, _n = m.height_support(fn, 0.0, 0.0, foot_radius=0.08, snap_to_plane=True)
    assert y == pytest.approx(0.0, abs=0.01)


def test_walk_known_slope_under_grounded_float():
    """GPU-free: Walk-radius capsule on grade 0.4 stays within |foot − terrain|.

    Documented budget: ``GROUNDED_FLOAT`` = 0.05 (``debug_trace`` default).
    Still no Rapier.
    """
    m = _phys()
    fn = lambda x, _z: 0.4 * x
    p = m.Physics3D(gravity=20.0)
    p.set_height_fn(fn)
    player = p.add_capsule(1.0, 0.5, 0.0, 0.28, 1.7)
    player.friction = 0.0
    budget = m.GROUNDED_FLOAT
    grounded = 0
    peak = 0.0
    for _ in range(160):
        player.vx = 3.2
        player.vz = 0.0
        p.update(0.016)
        gy = fn(player.x, player.z)
        if player.on_ground:
            grounded += 1
            peak = max(peak, abs(player.y - gy))
            assert abs(player.y - gy) <= budget
    assert grounded > 100
    assert peak <= budget


def test_fat_aabb_float_exceeds_budget():
    """Hypothesis check: loosen foot to the wall capsule, skip plane snap."""
    m = _phys()
    fn = lambda x, _z: 0.4 * x
    p = m.Physics3D(gravity=20.0)
    p.set_height_fn(fn)
    p.foot_radius = 0.28
    p.snap_to_plane = False
    player = p.add_capsule(1.0, 0.5, 0.0, 0.28, 1.7)
    player.friction = 0.0
    peak = 0.0
    for _ in range(80):
        player.vx = 3.0
        p.update(0.016)
        if player.on_ground:
            peak = max(peak, abs(player.y - fn(player.x, player.z)))
    assert peak > m.GROUNDED_FLOAT
    assert peak == pytest.approx(0.4 * 0.28, abs=0.02)


def test_overworld_height_walk_under_grounded_float():
    land = load_kagra_submodule("land")
    m = _phys()
    fn = land.overworld_height
    p = m.Physics3D(gravity=20.0)
    p.set_height_fn(fn)
    player = p.add_capsule(4.0, fn(4.0, 4.0) + 0.3, 4.0, 0.28, 1.7)
    player.friction = 0.0
    budget = m.GROUNDED_FLOAT
    grounded = 0
    walkable = 0
    for _ in range(200):
        player.vx = 3.2
        player.vz = 1.1
        p.update(0.016)
        if not player.on_ground:
            continue
        grounded += 1
        gy = fn(player.x, player.z)
        _nx, ny, _nz = m.height_normal(fn, player.x, player.z)
        if ny < p._walkable_ny():
            continue
        walkable += 1
        assert abs(player.y - gy) <= budget
    assert grounded > 120
    assert walkable > 80


def test_wish_zero_stops_on_walkable_slope():
    """Physics leftover after #79: wish 0 must not keep walking on grade 0.4.

    Tiny settle is OK; continued walk-speed is not. Steep slide is a
    different path (``test_steep_ramp_slides_down``).
    """
    m = _phys()
    fn = lambda x, _z: 0.4 * x
    p = m.Physics3D(gravity=20.0)
    p.set_height_fn(fn)
    player = p.add_capsule(1.0, 0.5, 0.0, 0.28, 1.7)
    player.friction = 0.0
    for _ in range(20):
        player.vx = 3.2
        player.vz = 0.0
        p.update(0.016)
    x0, z0 = player.x, player.z
    for _ in range(30):
        player.vx = 0.0
        player.vz = 0.0
        p.update(0.016)
    dist = math.hypot(player.x - x0, player.z - z0)
    walked = 3.2 * 0.016 * 30
    assert dist < 0.35, f"walkable slope idle moved {dist:.3f}m (held would be ~{walked:.2f})"
    assert abs(player.vx) < 1.0
    assert dist < walked * 0.25


def test_wish_zero_stops_on_flat():
    m = _phys()
    p = m.Physics3D(gravity=20.0)
    p.set_ground_y(0.0)
    player = p.add_capsule(0.0, 0.2, 0.0, 0.28, 1.7)
    for _ in range(10):
        p.update(0.016)
    for _ in range(15):
        player.vx = 3.2
        player.vz = 0.0
        p.update(0.016)
    x0, z0 = player.x, player.z
    for _ in range(30):
        player.vx = 0.0
        player.vz = 0.0
        p.update(0.016)
    dist = math.hypot(player.x - x0, player.z - z0)
    assert dist < 0.12
    assert abs(player.vx) < 0.05 and abs(player.vz) < 0.05


def test_foot_offsets_include_diagonals():
    """4 cardinals miss a diagonal ridge — that was the one-sided sample."""
    m = _phys()
    offs = m.foot_offsets(0.08)
    assert len(offs) == 9
    diags = [o for o in offs if abs(o[0]) > 0.01 and abs(o[1]) > 0.01]
    assert len(diags) == 4


def test_one_sided_terrace_does_not_fat_lift():
    """Hypothesis: a 6 cm shelf 8 cm uphill used to raise sit by raw max-Y.

    Center-plane + BUMP_PLANE_ERR keeps |sit − center| under GROUNDED_FLOAT.
    Not missing Rapier.
    """
    m = _phys()

    def fn(x, _z):
        base = 0.4 * x
        return base + 0.06 if x > 1.04 else base

    h_center = fn(1.0, 0.0)
    y, _n = m.height_support(fn, 1.0, 0.0, foot_radius=0.08, snap_to_plane=True)
    assert abs(y - h_center) <= m.GROUNDED_FLOAT


def test_diagonal_pebble_is_sampled():
    """8-point ring sees a pebble on the diagonal; 4 cardinals would miss it."""
    m = _phys()

    def fn(x, z):
        if abs(x - 0.056) < 0.012 and abs(z - 0.056) < 0.012:
            return 0.10
        return 0.0

    y, _n = m.height_support(fn, 0.0, 0.0, foot_radius=0.08, snap_to_plane=True)
    assert y > 0.07


def test_ground_stick_snaps_down_on_slope():
    m = _phys()
    p = m.Physics3D(gravity=20.0)
    fn = lambda x, _z: 0.4 * x
    p.set_height_fn(fn)
    player = p.add_capsule(1.0, 0.44, 0.0, 0.28, 1.7)
    player.vy = 0.0
    p.update(0.016)
    gy = fn(player.x, player.z)
    assert player.on_ground
    assert abs(player.y - gy) <= m.GROUNDED_FLOAT


def test_capsule_steps_up_low_crate():
    """Static prop lip (~30 cm) is a step, not a wall."""
    m = _phys()
    p = m.Physics3D(gravity=20.0)
    p.set_ground_y(0.0)
    player = p.add_capsule(0.0, 0.0, 0.0, 0.28, 1.7)
    p.add_body(0.75, 0.0, 0.0, 0.8, 0.30, 0.8, is_static=True)
    player.friction = 0.0
    for _ in range(10):
        p.update(0.016)
    for _ in range(55):
        player.vx = 2.4
        p.update(0.016)
    assert player.x > 0.45, f"stuck at x={player.x:.3f} y={player.y:.3f}"
    assert player.y > 0.22
    assert player.on_ground


def test_capsule_still_blocked_by_tall_wall():
    m = _phys()
    p = m.Physics3D(gravity=0.0)
    p.set_ground_y(-10.0)
    player = p.add_capsule(0.0, 0.0, 0.0, 0.28, 1.7)
    player.use_gravity = False
    p.add_body(0.8, 0.0, 0.0, 0.5, 2.0, 1.2, is_static=True)
    player.vx = 3.0
    for _ in range(40):
        p.update(0.016)
    assert player.x < 0.7
    assert player.y < 0.2
