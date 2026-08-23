"""Ursina 級の Prop / Walk — GPU 不要。"""
from __future__ import annotations

import math

import pytest

from tests.conftest import load_kagra_submodule

play = load_kagra_submodule("play")


@pytest.fixture(autouse=True)
def _clear_props():
    play.Prop.clear()
    yield
    play.Prop.clear()


def test_resolve_color_name_and_rgb():
    assert play.resolve_color("gold") == (240, 200, 70)
    assert play.resolve_color((10, 20, 30)) == (10, 20, 30)
    with pytest.raises(ValueError):
        play.resolve_color("not-a-color")


def test_walk_wish_yaw_zero_is_plus_z():
    fx, fz = play.walk_wish(1.0, 0.0, 0.0, speed=2.0)
    assert abs(fx) < 1e-9
    assert abs(fz - 2.0) < 1e-9


def test_walk_wish_yaw_half_pi_is_plus_x():
    fx, fz = play.walk_wish(1.0, 0.0, math.pi / 2, speed=3.0)
    assert abs(fx - 3.0) < 1e-9
    assert abs(fz) < 1e-9


def test_look_yaw_subtracts_mouse_x():
    assert play.look_yaw(0.0, 10.0, sens=0.01) == pytest.approx(-0.1)


def test_prop_records_center_xform():
    p = play.Prop("box", x=2.0, y=0.5, z=-1.0, scale=(1.2, 1.0, 1.4), color="orange")
    inst = p.instance()
    assert inst[:3] == pytest.approx([2.0, 0.5, -1.0])
    assert inst[3:6] == pytest.approx([1.2, 1.0, 1.4])
    assert p.color == (240, 140, 50)
    assert p in play.Prop._all
    play.Prop.clear()
    assert play.Prop._all == []


def test_prop_bake_without_engine_is_zero():
    p = play.Prop("sphere", color="gold", collision=False)
    assert p.bake() == 0
    assert p.mesh_id == 0
    assert play.Prop.bake_all() == [0]


def test_prop_blocks_player_via_world3d():
    w = play.World3D(gravity=0.0)
    play.Prop("box", x=1.2, y=0.5, z=0.0, scale=(0.8, 1.0, 1.6), world=w)
    p = w.add_player(0.0, 0.0, radius=0.28, height=1.6)
    p.use_gravity = False
    w.move_player(5.0, 0.0)
    for _ in range(40):
        w.update(0.016)
    assert p.x < 0.95
    assert w.box_xforms == []
