"""Gamepad inject / axis — GPU 不要。"""
from __future__ import annotations

import math

import pytest

from tests.conftest import load_kagra_submodule

pad = load_kagra_submodule("pad")
play = load_kagra_submodule("play")
touch = load_kagra_submodule("touch")


@pytest.fixture(autouse=True)
def _reset_pad():
    pad.inject_pad(clear=True)
    yield
    pad.inject_pad(clear=True)


def test_inject_axis_and_buttons():
    pad.inject_pad(lx=0.8, ly=-1.0, buttons={"A": True})
    assert pad.axis("left") == pytest.approx((0.8, -1.0))
    assert pad.pad("a")
    assert pad.pad("south")
    assert pad.pad_pressed("a")
    assert not pad.pad("b")
    pad.inject_pad(buttons={"a": False})
    assert not pad.pad("a")
    assert pad.pad_released("a")


def test_inject_clamps_and_right_stick():
    pad.inject_pad(lx=4.0, ry=-0.5)
    assert pad.axis("left")[0] == pytest.approx(1.0)
    assert pad.axis("right") == pytest.approx((0.0, -0.5))


def test_stick_move_up_is_forward():
    fwd, right = pad.stick_move(0.0, -1.0)
    assert fwd == pytest.approx(1.0)
    assert right == pytest.approx(0.0)
    assert pad.stick_move(0.05, 0.05) == (0.0, 0.0)
    assert pad.stick_move(0.0, 0.0) == (0.0, 0.0)
    assert pad.stick_move(float("nan"), 0.5) == (0.0, 0.0)


def test_walk_wish_from_stick():
    fwd, right = pad.stick_move(1.0, 0.0)
    fx, fz = play.walk_wish(fwd, right, 0.0, speed=2.0)
    assert fx == pytest.approx(-2.0)
    assert abs(fz) < 1e-9


def test_virtual_pad_stick_deadzone():
    vp = touch.VirtualPad(deadzone=0.25)
    vp.set_stick(0.1, -0.9)
    assert vp.stick() == pytest.approx((0.0, -0.9))


def test_normalize_aliases():
    assert pad.normalize_button("CROSS") == "a"
    assert pad.normalize_button("nope") == ""


def test_apply_hardware_fills_axes_until_inject():
    pad._STATE.apply_hardware(0.4, -0.8, 0.1, 0.2, ["a", "start"])
    assert pad.axis("left") == pytest.approx((0.4, -0.8))
    assert pad.axis("right") == pytest.approx((0.1, 0.2))
    assert pad.pad("a") and pad.pad("start")
    pad.inject_pad(lx=1.0)
    pad._STATE.apply_hardware(0.0, 0.0, 0.0, 0.0, [])
    assert pad.axis("left")[0] == pytest.approx(1.0)
