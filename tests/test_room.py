"""閉じた部屋の配置 — GPU 不要。"""
from __future__ import annotations

import pytest

from tests.conftest import load_kagra_submodule

play = load_kagra_submodule("play")


@pytest.fixture(autouse=True)
def _clear_props():
    play.Prop.clear()
    yield
    play.Prop.clear()


def test_room_layout_has_floor_ceiling_four_walls():
    parts = play.room_layout(half=4.0, height=3.0, thick=0.2)
    kinds = [p["kind"] for p in parts]
    assert kinds.count("floor") == 1
    assert kinds.count("ceiling") == 1
    assert kinds.count("wall") == 4
    floor = next(p for p in parts if p["kind"] == "floor")
    assert floor["model"] == "plane"
    assert floor["y"] == 0.0
    assert floor["sx"] == pytest.approx(8.0)
    assert floor["sz"] == pytest.approx(8.0)
    ceil = next(p for p in parts if p["kind"] == "ceiling")
    assert ceil["model"] == "box"
    assert ceil["y"] == pytest.approx(3.0)
    zs = sorted(p["z"] for p in parts if p["kind"] == "wall" and abs(p["x"]) < 1e-9)
    assert zs == pytest.approx([-4.0, 4.0])
    xs = sorted(p["x"] for p in parts if p["kind"] == "wall" and abs(p["z"]) < 1e-9)
    assert xs == pytest.approx([-4.0, 4.0])


def test_center_is_inside_room():
    assert play.point_in_room(0.0, 1.2, 0.0, half=4.0, height=3.0, thick=0.2)
    assert not play.point_in_room(0.0, -0.1, 0.0, half=4.0, height=3.0)
    assert not play.point_in_room(3.95, 1.0, 0.0, half=4.0, height=3.0, thick=0.2)


def test_room_builds_six_props_without_gpu():
    props = play.room(half=5.0, height=2.8, look=False, textured=False)
    assert len(props) == 6
    models = [p.model for p in props]
    assert models.count("plane") == 1
    assert models.count("box") == 5
    assert all(p.enabled for p in props)


def test_room_rejects_tiny():
    with pytest.raises(ValueError):
        play.room_layout(half=0.1)
    with pytest.raises(ValueError):
        play.room_layout(height=0.2)
