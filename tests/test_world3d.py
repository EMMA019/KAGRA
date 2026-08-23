"""World3D — 床と箱の衝突。GPU 不要。"""
from __future__ import annotations

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
