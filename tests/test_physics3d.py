"""Physics3D — GPU 不要。カプセル / OBB / レイヤー / トリガー。"""
from __future__ import annotations

import math

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
