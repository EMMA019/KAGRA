"""Juice: shake, hitstop, squash, ease. No GPU."""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

juice = load_kagra_submodule("juice")


class _Box:
    def __init__(self) -> None:
        self.y = 0.0


def test_ease_endpoints_and_out_cubic():
    assert juice.ease("out_cubic", 0.0) == 0.0
    assert juice.ease("out_cubic", 1.0) == 1.0
    assert juice.ease("out_cubic", 0.5) > 0.5
    assert juice.ease("linear", 0.25) == 0.25
    assert juice.ease("nope", 0.0) == 0.0
    assert juice.ease("out_bounce", 1.0) == 1.0


def test_tween_reaches_end():
    box = _Box()
    feel = juice.Juice()
    feel.tween(box, "y", 2.0, duration=0.2, ease="linear")
    for _ in range(10):
        feel.tick(0.02)
    assert box.y == 2.0


def test_hitstop_eats_dt_then_returns_the_rest():
    feel = juice.Juice()
    feel.hitstop(0.05)
    assert feel.tick(0.02) == 0.0
    rest = feel.tick(0.04)
    assert abs(rest - 0.01) < 1e-6


def test_squash_peaks_then_settles():
    feel = juice.Juice()
    feel.squash(0.3, duration=0.2)
    feel.tick(0.07)
    _sx, sy, _sz = feel.scale()
    assert sy < 0.75
    assert feel.scale()[0] > 1.0
    feel.tick(1.0)
    assert feel.scale() == (1.0, 1.0, 1.0)


def test_stretch_grows_y():
    feel = juice.Juice()
    feel.stretch(0.2, duration=0.2)
    feel.tick(0.07)
    assert feel.scale()[1] > 1.1


def test_shake_offset_is_deterministic_then_ends():
    a = juice.Juice()
    b = juice.Juice()
    a.shake(0.2, duration=0.3)
    b.shake(0.2, duration=0.3)
    a.tick(0.04)
    b.tick(0.04)
    assert a.camera_offset() == b.camera_offset()
    assert a.camera_offset() != (0.0, 0.0, 0.0)
    a.tick(1.0)
    assert a.camera_offset() == (0.0, 0.0, 0.0)


def test_disabled_is_a_passthrough():
    feel = juice.Juice()
    feel.enabled = False
    feel.hitstop(1.0)
    feel.squash(0.4, 0.2)
    assert feel.tick(0.1) == 0.1
    assert feel.scale() == (1.0, 1.0, 1.0)
    assert feel.camera_offset() == (0.0, 0.0, 0.0)


def test_apply_world_writes_scale_and_shifts_camera_once():
    feel = juice.Juice()
    feel.squash(0.3, duration=0.2)
    feel.shake(0.2, duration=0.4)
    feel.tick(0.07)
    world = {
        "player": {"id": "walker:player", "name": "player", "position": [0, 1, 0]},
        "walkers": [{"id": "walker:player", "name": "player"}],
        "cameras": [{"position": [0.0, 4.0, 10.0], "target": [0.0, 1.0, 0.0]}],
    }
    feel.apply_world(world)
    assert world["player"]["scale"][1] < 1.0
    assert world["walkers"][0]["scale"] == world["player"]["scale"]
    assert world["cameras"][0]["position"] != [0.0, 4.0, 10.0]
    ox = world["cameras"][0]["position"][0]
    feel.apply_world(world)
    # A second call on the same dict stacks. Games pass a fresh dump.
    assert world["cameras"][0]["position"][0] != ox


def test_apply_post_presets():
    world = {}
    juice.apply_post(world, "legacy")
    assert world["bloom"] == 0.0
    assert world["vignette"] == 1.0
    assert world["lut"] == "off"
    assert world["fxaa"] is True
    juice.apply_post(world, "mobile")
    assert world["bloom"] > 0.0
    assert world["lut"] == "punch"
    assert world["vignette"] > 1.0
    juice.apply_post(world, "off")
    assert world["vignette"] == 0.0
    assert world["fxaa"] is False


def test_apply_look_night_and_pastel():
    world = {"lights": [{"id": "light:0", "intensity": 1.0, "color": [1.0, 1.0, 1.0]}]}
    juice.apply_look(world, "night")
    assert world["sky"][2] < 80
    assert world["lut"] == "cool"
    assert world["hemi_sky"][2] > world["hemi_ground"][2]
    assert world["lights"][0]["intensity"] < 1.0
    assert world["lights"][0]["color"][2] > world["lights"][0]["color"][0]
    day = {}
    juice.apply_look(day, "pastel")
    assert day["lut"] == "warm"
    assert day["sky"][0] > 150
