"""ライブ見た目の純ロジック（空・ビネット・カット割り・接地）。GPU 不要。"""
from __future__ import annotations

import struct
import zlib

from tests.conftest import load_kagra_submodule

import pytest

look = load_kagra_submodule("look")


def test_wood_plank_groove_is_darker():
    mid = look.wood_plank_rgba(0.07, 0.4)
    groove = look.wood_plank_rgba(0.005, 0.4)
    assert sum(mid[:3]) > sum(groove[:3])


def test_plaster_ceiling_is_lighter_than_wainscot():
    wall = look.plaster_rgba(0.5, 0.6, ceiling=False)
    dado = look.plaster_rgba(0.5, 0.05, ceiling=False)
    ceil = look.plaster_rgba(0.5, 0.5, ceiling=True)
    assert sum(ceil[:3]) > sum(wall[:3])
    assert sum(wall[:3]) > sum(dado[:3])


def test_sky_zenith_is_darker_than_horizon():
    zenith = look.sky_rgba(0.5, 1.0)
    horizon = look.sky_rgba(0.5, 0.5)
    assert sum(zenith[:3]) < sum(horizon[:3])


def test_sky_is_opaque():
    r, g, b, a = look.sky_rgba(0.2, 0.8)
    assert a == 255
    assert 0 <= r <= 255 and 0 <= g <= 255 and 0 <= b <= 255


def test_vignette_center_is_clear_corners_dark():
    assert look.vignette_alpha(0.5, 0.5) < 20
    assert look.vignette_alpha(0.0, 0.0) > 180


def test_gradient_sky_png_is_valid_png():
    data = look.gradient_sky_png(8, 4)
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    assert b"IHDR" in data
    assert b"IEND" in data
    # IHDR 幅高さ
    ihdr = data.find(b"IHDR")
    w, h = struct.unpack(">II", data[ihdr + 4 : ihdr + 12])
    assert (w, h) == (8, 4)
    # 展開できる
    idat = data.find(b"IDAT")
    size = struct.unpack(">I", data[idat - 4 : idat])[0]
    raw = zlib.decompress(data[idat + 4 : idat + 4 + size])
    assert len(raw) == 4 * (1 + 8 * 4)


def test_showcase_blend_starts_on_body():
    assert look.showcase_blend(0.0) == 0.0
    assert look.showcase_blend(1.0, period=6.5, blend=1.35) == 0.0


def test_showcase_blend_reaches_face():
    face = look.showcase_blend(7.0, period=6.5, blend=1.35)
    assert face > 0.95


def test_showcase_params_lerp():
    body = look.showcase_params(0.0)
    face = look.showcase_params(1.0)
    mid = look.showcase_params(0.5)
    assert body["radius"] > face["radius"]
    assert face["target_y"] > body["target_y"]
    assert abs(mid["radius"] - (body["radius"] + face["radius"]) / 2) < 1e-6


def test_grounding_lift_raises_when_foot_clips():
    lift = look.grounding_lift([-0.08], floor_y=0.0, sole=0.03, current=0.0, follow=1.0)
    assert abs(lift - 0.11) < 1e-6


def test_grounding_lift_does_not_push_down():
    lift = look.grounding_lift([0.2], floor_y=0.0, sole=0.03, current=0.0, follow=1.0)
    assert lift == 0.0


def test_grounding_lift_smooths():
    a = look.grounding_lift([-0.1], current=0.0, follow=0.5, sole=0.0)
    assert 0.0 < a < 0.1


def test_light_slot_allows_four_rejects_fifth():
    assert look.LOCAL_LIGHT_SLOTS == 4
    assert look.check_light_slot(0) == 0
    assert look.check_light_slot(3) == 3
    with pytest.raises(ValueError, match="4 slots"):
        look.check_light_slot(4)
    with pytest.raises(ValueError, match="4 slots"):
        look.check_light_slot(-1)
