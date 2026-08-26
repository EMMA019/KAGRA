"""CharacterController — GPU 不要。accel / wish / jump / land. Not Rapier."""
from __future__ import annotations

import math

import pytest

from tests.conftest import load_kagra_submodule


def _ctrl():
    return load_kagra_submodule("controller")


def _phys():
    return load_kagra_submodule("physics3d")


def _play():
    return load_kagra_submodule("play")


def _cam():
    return load_kagra_submodule("camera3d").Camera3D(640, 360)


def test_accelerate_xz_ramps_then_stops():
    m = _ctrl()
    vx = vz = 0.0
    for _ in range(8):
        vx, vz = m.accelerate_xz(vx, vz, 3.2, 0.0, 0.016, accel=14.0, decel=22.0)
    assert 0.5 < vx < 2.6
    assert abs(vz) < 1e-9
    for _ in range(80):
        vx, vz = m.accelerate_xz(vx, vz, 3.2, 0.0, 0.016, accel=14.0, decel=22.0)
    assert vx == pytest.approx(3.2, abs=0.05)
    for _ in range(6):
        vx, vz = m.accelerate_xz(vx, vz, 0.0, 0.0, 0.016, accel=14.0, decel=22.0)
    assert 0.5 < vx < 3.1
    for _ in range(40):
        vx, vz = m.accelerate_xz(vx, vz, 0.0, 0.0, 0.016, accel=14.0, decel=22.0)
    assert abs(vx) < 0.05 and abs(vz) < 0.05


def test_air_control_is_weaker_than_ground():
    m = _ctrl()
    gx, gz = m.accelerate_xz(0.0, 0.0, 3.2, 0.0, 0.1, accel=14.0, air=False)
    ax, az = m.accelerate_xz(0.0, 0.0, 3.2, 0.0, 0.1, accel=14.0, air=True, air_control=0.38)
    assert gx > ax * 1.5
    assert abs(az) < 1e-9 and abs(gz) < 1e-9


def test_controller_wish_move_try_jump_names():
    m = _ctrl()
    c = m.CharacterController(speed=3.2, jump=6.0, accel=14.0, decel=22.0)
    c.wish(3.2, 0.0)
    assert c.wish_x == pytest.approx(3.2)
    c.move(0.0, 2.0)
    assert c.wish_z == pytest.approx(2.0)
    c.try_jump()
    assert c._jump_queued is True


def test_controller_jump_and_land():
    ctrl_mod = _ctrl()
    phys = _phys()
    world = phys.Physics3D(gravity=20.0)
    world.set_ground_y(0.0)
    body = world.add_capsule(0.0, 0.2, 0.0, 0.28, 1.7)
    for _ in range(12):
        world.update(0.016)
    assert body.on_ground
    c = ctrl_mod.CharacterController(speed=3.2, jump=6.2, accel=14.0, decel=22.0)
    c.try_jump()
    c.apply(body, 0.016)
    assert body.vy > 5.0
    world.update(0.016)
    airborne = False
    saw_land = False
    for _ in range(90):
        c.wish(0.0, 0.0)
        c.apply(body, 0.016)
        world.update(0.016)
        if not body.on_ground:
            airborne = True
        if c.landed:
            saw_land = True
    assert airborne
    assert saw_land
    assert body.on_ground
    assert body.y == pytest.approx(0.0, abs=0.08)


def test_walk_wish_accelerates_on_flat():
    play = _play()
    w = play.World3D(gravity=9.8)
    p = w.add_player(0.0, 0.0, radius=0.28, height=1.7)
    for _ in range(8):
        w.update(0.016)
    walk = play.Walk(w, _cam(), speed=3.2, accel=14.0, decel=22.0)
    speeds = []
    for _ in range(12):
        walk.wish(1.0, 0.0)
        walk.update(0.016)
        speeds.append(math.hypot(p.vx, p.vz))
    assert speeds[0] < speeds[8]
    assert speeds[0] < 2.0
    assert speeds[-1] > 1.4


def test_walk_move_and_try_jump_on_world():
    play = _play()
    w = play.World3D(gravity=20.0)
    p = w.add_player(0.0, 0.0, radius=0.28, height=1.7)
    for _ in range(10):
        w.update(0.016)
    walk = play.Walk(w, _cam(), speed=3.2, jump=6.0, accel=14.0, decel=22.0)
    walk.move(0.0, 0.0)
    walk.try_jump()
    walk.update(0.016)
    assert p.vy > 4.5
    assert walk.landed is False
    for _ in range(80):
        walk.move(0.0, 0.0)
        walk.update(0.016)
    assert p.on_ground
    assert p.y == pytest.approx(0.0, abs=0.1)


def test_walk_release_decelerates_not_held_key():
    """Idle wish decelerates; must not keep walk-speed. Instant snap was cheap."""
    play = _play()
    w = play.World3D(gravity=9.8)
    p = w.add_player(0.0, 0.0, radius=0.28, height=1.7)
    walk = play.Walk(w, _cam(), speed=3.2, accel=14.0, decel=22.0)
    for _ in range(40):
        walk.wish(1.0, 0.0)
        walk.update(0.016)
    x0, z0 = p.x, p.z
    for _ in range(30):
        walk.wish(0.0, 0.0)
        walk.update(0.016)
    dist = math.hypot(p.x - x0, p.z - z0)
    held = 3.2 * 0.016 * 30
    assert dist < held * 0.45, f"idle still moved {dist:.3f}m (held would be {held:.2f})"
    assert abs(p.vx) < 0.12 and abs(p.vz) < 0.12
