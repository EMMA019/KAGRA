"""Screen axes. GPU-free. +X is right only when the camera looks toward -Z."""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

axes = load_kagra_submodule("axes")
flight = load_kagra_submodule("flight")
brawl = load_kagra_submodule("backyard_brawl")


def test_default_camera_plus_x_is_screen_right():
    right = axes.screen_right((0.0, 2.0, 6.0), (0.0, 0.0, 0.0))
    assert right[0] > 0.99
    assert abs(right[1]) < 1e-6
    assert abs(right[2]) < 1e-6
    left = axes.screen_left((0.0, 2.0, 6.0), (0.0, 0.0, 0.0))
    assert left[0] < -0.99


def test_looking_plus_z_plus_x_is_screen_left():
    right = axes.screen_right((0.0, 2.0, -6.0), (0.0, 0.0, 0.0))
    assert right[0] < -0.99
    wish = axes.wish_world(1.0, 0.0, (0.0, 2.0, -6.0), (0.0, 0.0, 0.0))
    assert wish[0] * right[0] + wish[1] * right[2] > 0.9


def test_annotate_dump_names_both_sides():
    world = {
        "version": 1,
        "cameras": [
            {"id": "c", "type": "camera", "position": [0, 2, 6], "target": [0, 0, 0]}
        ],
    }
    axes.annotate_dump(world)
    note = world["coordinates"]["note"]
    assert "screen_right = -X" in note
    assert world["coordinates"]["handedness"] == "right"
    cam = world["cameras"][0]
    assert cam["screen_right"][0] > 0.9
    assert cam["screen_left"][0] < -0.9


def test_flight_right_yaws_toward_screen_right():
    assert flight.roll_from_keys(True, False) == 1.0
    assert flight.roll_from_keys(False, True) == -1.0
    start = flight.default_state()
    world = flight.make_world(start)
    cam = world["cameras"][0]
    right = axes.screen_right(cam["position"], cam["target"])
    # Chase looks mostly along +Z, so the right edge is world -X.
    assert right[0] < -0.5

    straight = flight.default_state()
    turned = flight.default_state()
    for _ in range(60):
        flight.step_flight(straight, 1.0 / 60.0, 0.0, 0.0, False)
        flight.step_flight(turned, 1.0 / 60.0, 0.0, flight.roll_from_keys(True, False), False)
    assert turned["yaw"] < straight["yaw"]
    dx = turned["x"] - straight["x"]
    dz = turned["z"] - straight["z"]
    assert dx * right[0] + dz * right[2] > 0.5


def test_backyard_right_moves_screen_right():
    state = brawl.default_state()
    state["phase"] = "play"
    x0 = state["player"]["x"]
    z0 = state["player"]["z"]
    brawl.step_brawl(state, 0.4, 1.0, 0.0, False, False)
    world = brawl.make_world(state)
    cam = world["cameras"][0]
    right = axes.screen_right(cam["position"], cam["target"])
    dx = state["player"]["x"] - x0
    dz = state["player"]["z"] - z0
    assert dx * right[0] + dz * right[2] > 0.3


def test_side_view_plus_x_is_screen_right():
    # Platformer / torneko / fight sit the camera on +Z and look toward -Z.
    right = axes.screen_right((0.0, 4.8, 20.0), (0.0, 1.8, 0.0))
    assert right[0] > 0.99
    # Top-down torneko: eye is +2 in Z from the target.
    right = axes.screen_right((0.0, 10.5, 2.0), (0.0, 0.0, 0.0))
    assert right[0] > 0.9
