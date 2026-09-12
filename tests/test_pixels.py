"""GPU-free PNG region checks."""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

px = load_kagra_submodule("pixels")
gm = load_kagra_submodule("gameloop")


def _png(tmp_path, w, h, rgb):
    raw = bytes([rgb[0], rgb[1], rgb[2], 255]) * (w * h)
    p = tmp_path / "t.png"
    p.write_bytes(gm.rgba_to_png(raw, w, h))
    return p


def test_solid_region_is_detected(tmp_path):
    p = _png(tmp_path, 8, 8, (10, 20, 30))
    err = px.eval_expect_pixels(p, [{"x": 0, "y": 0, "w": 8, "h": 8, "not_solid": True}])
    assert err and "solid" in err[0]


def test_mean_bounds(tmp_path):
    p = _png(tmp_path, 4, 4, (200, 40, 20))
    assert px.eval_expect_pixels(
        p, [{"x": 0, "y": 0, "w": 4, "h": 4, "mean_min": [180, 10, 0], "mean_max": [220, 60, 40]}]
    ) == []
    err = px.eval_expect_pixels(p, [{"mean_max": [10, 10, 10]}])
    assert err


def test_decode_roundtrip(tmp_path):
    p = _png(tmp_path, 3, 2, (1, 2, 3))
    w, h, rgba = px.decode_png_rgba(p)
    assert (w, h) == (3, 2)
    assert rgba[:4] == bytes([1, 2, 3, 255])
