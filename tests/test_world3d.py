"""World3D — 床と箱の衝突。GPU 不要。"""
from __future__ import annotations

import pytest

from tests.conftest import load_kagra_submodule


def _world():
    return load_kagra_submodule("world3d")


def test_player_stopped_by_box():
    m = _world()
    w = m.World3D(gravity=0.0)
    w.add_box(1.1, 0.0, 0.0, 0.6, 1.6, 1.6)
    p = w.add_player(0.0, 0.0, radius=0.28, height=1.6)
    p.use_gravity = False
    w.move_player(5.0, 0.0)
    for _ in range(40):
        w.update(0.016)
    assert p.x < 0.85


def test_player_walks_open_floor():
    m = _world()
    w = m.World3D(gravity=0.0)
    p = w.add_player(0.0, 0.0, radius=0.25, height=1.6)
    p.use_gravity = False
    p.friction = 0.0
    w.move_player(0.0, -3.0)
    for _ in range(20):
        w.update(0.016)
    assert p.z < -0.5


def test_bake_without_engine_is_empty():
    m = _world()
    w = m.World3D()
    w.add_floor()
    w.add_box(0, 0, 0, 1, 1, 1)
    assert w.bake(1, 2) == []
    assert w.mesh_ids == []


def test_box_xforms_recorded_without_bake():
    m = _world()
    w = m.World3D()
    w.add_box(2, 0, -1, 1.2, 1.0, 1.2)
    assert len(w.box_xforms) == 1
    assert abs(w.box_xforms[0][0] - 2.0) < 1e-6
    assert abs(w.box_xforms[0][1] - 0.5) < 1e-6


def test_add_box_draw_false_is_physics_only():
    m = _world()
    w = m.World3D()
    body = w.add_box(1, 0, 1, 1, 1, 1, draw=False)
    assert w.box_xforms == []
    assert w._pending == []
    assert body in w.boxes
    w.add_box(2, 0, 1, 1, 1, 1, draw=True)
    assert len(w.box_xforms) == 1


def test_add_sphere_and_cylinder_are_physics_only():
    m = _world()
    w = m.World3D()
    ball = w.add_sphere(0, 0, 0, 0.5)
    cyl = w.add_cylinder(2, 0, 0, 0.4, 1.5)
    assert ball.shape == "sphere"
    assert cyl.shape == "cylinder"
    assert w.box_xforms == []
    assert w._pending == []


def test_height_fn_player_spawns_on_terrain():
    m = _world()
    w = m.World3D()
    w.set_height_fn(lambda _x, _z: 1.25)
    p = w.add_player(0.0, 0.0)
    assert p.y == pytest.approx(1.25)
    assert w.ground_y(1.0, 1.0) == pytest.approx(1.25)


def test_stream_tiles_load_and_unload():
    m = _world()
    w = m.World3D(half=24.0)
    w.set_height_fn(lambda _x, _z: 0.0, tile=10.0, stream_radius=12.0)
    n = w.stream_tiles(0.0, 0.0)
    assert n >= 1
    near = set(w.loaded_tiles())
    assert any(abs(ix) <= 1 and abs(iz) <= 1 for ix, iz in near)
    w.stream_tiles(30.0, 0.0)
    far = set(w.loaded_tiles())
    assert near != far
    assert (0, 0) not in far


def test_load_city_y_zero_snaps_to_ground(tmp_path):
    path = tmp_path / "c0.json"
    path.write_text(
        '{"version":1,"tile":10,"boxes":[{"x":2.0,"z":1.0,"w":1,"h":2,"d":1}]}',
        encoding="utf-8",
    )
    m = _world()
    w = m.World3D(half=24.0)
    w.set_height_fn(lambda _x, _z: 1.5, tile=10.0, stream_radius=16.0)
    w.load_city(str(path))
    w.stream_tiles(0.0, 0.0)
    assert any(abs(b.y - 1.5) < 1e-6 for b in w.boxes)


def test_load_city_places_on_stream(tmp_path):
    city = load_kagra_submodule("city")
    path = tmp_path / "c.json"
    path.write_text(
        '{"version":1,"tile":10,"boxes":[{"x":2.0,"y":0.4,"z":1.0,"w":1,"h":2,"d":1}]}',
        encoding="utf-8",
    )
    m = _world()
    w = m.World3D(half=24.0)
    w.set_height_fn(lambda _x, _z: 0.4, tile=10.0, stream_radius=16.0)
    w.load_city(str(path))
    w.stream_tiles(0.0, 0.0)
    assert any(abs(b.x - 2.0) < 1e-6 for b in w.boxes)


def test_player_stands_on_dynamic_stack():
    """Walk 相当のカプセルが積み木の上に立つ。"""
    m = _world()
    w = m.World3D(gravity=22.0)
    w.add_box(0.0, 0.8, 0.0, 1.0, 0.45, 1.0, is_static=False)
    for _ in range(140):
        w.update(0.016)
    crate = w.boxes[0]
    top = crate.y + crate.h
    p = w.add_player(0.0, 0.0, radius=0.26, height=1.6)
    p.x = crate.x
    p.z = crate.z
    p.y = top + 1.3
    for _ in range(160):
        w.update(0.016)
    assert p.on_ground
    assert p.y == pytest.approx(top, abs=0.14)
    assert crate.y == pytest.approx(0.0, abs=0.12)


def test_dynamic_box_xform_tracks_body():
    m = _world()
    w = m.World3D(gravity=0.0)
    b = w.add_box(0.0, 1.0, 0.0, 0.6, 0.6, 0.6, is_static=False)
    b.use_gravity = False
    b.vx = 2.0
    b.friction = 0.0
    w.update(0.05)
    assert w.box_xforms[0][0] == pytest.approx(b.x)


def test_chunk_fill_once_per_tile():
    m = _world()
    w = m.World3D(half=24.0)
    hits: list[tuple[int, int]] = []
    w.set_height_fn(lambda _x, _z: 0.0, tile=10.0, stream_radius=8.0)
    w.set_chunk_fill(lambda ix, iz: hits.append((ix, iz)))
    w.stream_tiles(0.0, 0.0)
    first = list(hits)
    assert first
    w.stream_tiles(16.0, 0.0)
    w.stream_tiles(0.0, 0.0)
    assert hits[: len(first)] == first
    assert len(hits) == len(set(hits))
