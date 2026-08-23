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


def test_look_pitch_clamps():
    assert play.look_pitch(0.0, -10.0, sens=0.01) == pytest.approx(0.1)
    assert play.look_pitch(1.19, -10.0, sens=0.01) == pytest.approx(1.2)


def test_first_person_eye_looks_along_yaw():
    pos, tgt = play.first_person_eye(0.0, 0.0, 0.0, 0.0, 0.0, eye_height=1.55)
    assert pos == pytest.approx((0.0, 1.55, 0.0))
    assert tgt[2] > pos[2]
    pos_r, tgt_r = play.first_person_eye(0.0, 0.0, 0.0, math.pi / 2, 0.0, eye_height=1.55)
    assert tgt_r[0] > pos_r[0]
    assert abs(tgt_r[2] - pos_r[2]) < 1e-9


def test_hovered_prop_picks_nearest_and_skips_plane():
    play.Prop("plane", x=0, y=0, z=2, scale=20.0, collision=False)
    near = play.Prop("box", x=0, y=0.5, z=2, scale=1.0, collision=False, color="orange")
    far = play.Prop("sphere", x=0, y=0.5, z=6, scale=1.0, collision=False, color="gold")
    hit = play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0)
    assert hit is near
    assert hit is not far
    play.Prop.clear()
    play.Prop("plane", x=0, y=0, z=3, scale=14.0, collision=False)
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is None


def test_color_name_roundtrip():
    assert play.color_name("gold") == "gold"
    assert play.color_name((240, 200, 70)) == "gold"
    assert play.color_name((1, 2, 3)) is None


def test_prop_records_center_xform():
    p = play.Prop("box", x=2.0, y=0.5, z=-1.0, scale=(1.2, 1.0, 1.4), color="orange")
    inst = p.instance()
    assert inst[:3] == pytest.approx([2.0, 0.5, -1.0])
    assert inst[3:6] == pytest.approx([1.2, 1.0, 1.4])
    assert p.color == (240, 140, 50)
    assert p in play.Prop._all
    play.Prop.clear()
    assert play.Prop._all == []


def test_prop_world_verts_match_instance_scale():
    p = play.Prop("box", x=1.0, y=2.0, z=3.0, scale=2.0, collision=False)
    verts, _ = play._unit_mesh("box")
    world = p.world_verts(verts)
    xs = [v[0] for v in world]
    ys = [v[1] for v in world]
    assert min(xs) == pytest.approx(0.0)
    assert max(xs) == pytest.approx(2.0)
    assert min(ys) == pytest.approx(1.0)
    assert max(ys) == pytest.approx(3.0)


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


def test_prop_set_position_moves_collision():
    w = play.World3D(gravity=0.0)
    box = play.Prop("box", x=4.0, y=0.5, z=0.0, scale=(0.8, 1.0, 1.6), world=w)
    p = w.add_player(0.0, 0.0, radius=0.28, height=1.6)
    p.use_gravity = False
    box.set_position(1.2, 0.5, 0.0)
    assert box.body.x == pytest.approx(1.2)
    w.move_player(5.0, 0.0)
    for _ in range(40):
        w.update(0.016)
    assert p.x < 0.95


def test_prop_velocity_update_and_destroy():
    w = play.World3D(gravity=0.0)
    box = play.Prop("box", x=0.0, y=0.5, z=2.0, scale=1.0, world=w)
    box.vz = 2.0
    play.Prop.update_all(0.5)
    assert box.z == pytest.approx(3.0)
    assert box.body.z == pytest.approx(3.0)
    play.destroy(box)
    assert box not in play.Prop._all
    assert box.enabled is False
    assert box.body.active is False
    play.destroy(box)


def test_prop_disabled_skipped_by_hover():
    box = play.Prop("box", x=0, y=0.5, z=2, scale=1.0, collision=False)
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is box
    box.enabled = False
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is None
    box.enabled = True
    box.destroy()
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is None
