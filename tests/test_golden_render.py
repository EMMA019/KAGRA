"""GPU ヘッドレス描画のゴールデン画像回帰テスト。

基準更新:
    KAGRA_UPDATE_GOLDENS=1 pytest tests -m golden

アセット（VRM/フォント）に依存しない。winit はプロセスあたり 1 回しか
EventLoop を作れないため、各シーンは子プロセスで描画する。
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

pytestmark = pytest.mark.golden

ROOT = Path(__file__).resolve().parents[1]
RENDER = ROOT / "tests" / "render_golden_scene.py"
OUT_DIR = ROOT / "scratch" / "golden_actual"


def _prefer_installed_kagra():
    """クローン直下の ``kagra/`` は wheel の ``kagra_core`` を隠す。"""
    root = ROOT.resolve()
    kept: list[str] = []
    for p in sys.path:
        if p in ("", "."):
            continue
        try:
            if Path(p).resolve() == root:
                continue
        except OSError:
            pass
        kept.append(p)
    sys.path[:] = kept
    for name in list(sys.modules):
        if name == "kagra" or name.startswith("kagra."):
            del sys.modules[name]


def _ensure_kagra():
    _prefer_installed_kagra()
    pytest.importorskip("kagra")


def _render(scene: str, out_name: str) -> Path:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / out_name
    if out.exists():
        out.unlink()

    env = os.environ.copy()
    env.pop("PYTHONPATH", None)
    proc = subprocess.run(
        [sys.executable, str(RENDER), scene, str(out)],
        cwd=tempfile.gettempdir(),
        env=env,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise AssertionError(
            f"render failed for {scene} ({proc.returncode})\n"
            f"stdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
    assert out.exists(), f"screenshot missing: {out}"
    return out


def test_golden_shapes2d():
    _ensure_kagra()
    from tests.golden_utils import compare_png

    actual = _render("shapes2d", "shapes2d.png")
    compare_png(actual, "shapes2d.png")


def test_golden_mesh3d():
    _ensure_kagra()
    from tests.golden_utils import compare_png

    actual = _render("mesh3d", "mesh3d.png")
    compare_png(actual, "mesh3d.png")


def test_pairwise_indoor_spot_shadow():
    """スポットがマップを所有しているとき、影のオン/オフが画素で違う。"""
    _ensure_kagra()
    from tests.golden_utils import assert_pngs_differ

    on = _render("indoor_spot", "indoor_spot.png")
    off = _render("indoor_spot_off", "indoor_spot_off.png")
    assert_pngs_differ(on, off, min_mean_abs=4.0, name="indoor_spot")


def test_pairwise_tonemap_aces():
    """ACES オン/オフがハイライトを変える（高露出クロム）。"""
    _ensure_kagra()
    from tests.golden_utils import assert_pngs_differ

    on = _render("tonemap_on", "tonemap_on.png")
    off = _render("tonemap_off", "tonemap_off.png")
    assert_pngs_differ(on, off, min_mean_abs=3.0, name="tonemap_aces")


def test_pairwise_ibl_metal():
    """金属とプラスチックが同じ HDRI で違う（スペキュラ mip）。"""
    _ensure_kagra()
    from tests.golden_utils import assert_pngs_differ

    metal = _render("ibl_metal", "ibl_metal.png")
    plastic = _render("ibl_plastic", "ibl_plastic.png")
    assert_pngs_differ(metal, plastic, min_mean_abs=3.0, name="ibl_metal")


def test_pairwise_normal_map():
    """接空間法線の有無がサイドライトで画素差になる。"""
    _ensure_kagra()
    from tests.golden_utils import assert_pngs_differ

    bump = _render("normal_bump", "normal_bump.png")
    flat = _render("normal_flat", "normal_flat.png")
    assert_pngs_differ(bump, flat, min_mean_abs=3.0, name="normal_bump")
