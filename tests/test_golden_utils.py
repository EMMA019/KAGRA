"""golden_utils の PNG 読み書き（GPU 不要）。"""
from __future__ import annotations

from pathlib import Path

from tests.golden_utils import _read_png_rgba, _write_png_rgba, compare_png


def test_png_roundtrip(tmp_path: Path):
    w, h = 4, 3
    rgba = bytes(
        [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
            10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255,
            0, 0, 0, 255, 127, 127, 127, 255, 255, 255, 255, 255, 1, 2, 3, 255,
        ]
    )
    path = tmp_path / "round.png"
    _write_png_rgba(path, w, h, rgba)
    gw, gh, got = _read_png_rgba(path)
    assert (gw, gh) == (w, h)
    assert got == rgba


def test_compare_png_identical(tmp_path: Path, monkeypatch):
    import tests.golden_utils as gu

    goldens = tmp_path / "goldens"
    goldens.mkdir()
    monkeypatch.setattr(gu, "GOLDENS_DIR", goldens)
    rgba = bytes([40, 50, 70, 255] * (8 * 8))
    actual = tmp_path / "actual.png"
    _write_png_rgba(actual, 8, 8, rgba)
    monkeypatch.setenv("KAGRA_UPDATE_GOLDENS", "1")
    compare_png(actual, "box.png")
    monkeypatch.delenv("KAGRA_UPDATE_GOLDENS")
    compare_png(actual, "box.png")


def test_png_mean_abs_identical(tmp_path: Path):
    from tests.golden_utils import png_mean_abs

    rgba = bytes([40, 50, 70, 255] * (4 * 4))
    a = tmp_path / "a.png"
    b = tmp_path / "b.png"
    _write_png_rgba(a, 4, 4, rgba)
    _write_png_rgba(b, 4, 4, rgba)
    assert png_mean_abs(a, b) == 0.0


def test_assert_pngs_differ_requires_gap(tmp_path: Path, monkeypatch):
    import tests.golden_utils as gu
    from tests.golden_utils import assert_pngs_differ

    monkeypatch.setattr(gu, "DIFFS_DIR", tmp_path / "diffs")
    dark = bytes([10, 10, 10, 255] * (8 * 8))
    bright = bytes([200, 200, 200, 255] * (8 * 8))
    a = tmp_path / "dark.png"
    b = tmp_path / "bright.png"
    _write_png_rgba(a, 8, 8, dark)
    _write_png_rgba(b, 8, 8, bright)
    mean = assert_pngs_differ(a, b, min_mean_abs=20.0, name="gap")
    assert mean > 100.0

    same = tmp_path / "same.png"
    _write_png_rgba(same, 8, 8, dark)
    try:
        assert_pngs_differ(a, same, min_mean_abs=4.0, name="too_close")
        raise AssertionError("expected pair too similar")
    except AssertionError as exc:
        assert "mean_abs" in str(exc)
