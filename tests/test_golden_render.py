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
from pathlib import Path

import pytest

pytestmark = pytest.mark.golden

ROOT = Path(__file__).resolve().parents[1]
RENDER = ROOT / "tests" / "render_golden_scene.py"
OUT_DIR = ROOT / "scratch" / "golden_actual"


def _ensure_kagra():
    pytest.importorskip("kagra")


def _render(scene: str, out_name: str) -> Path:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / out_name
    if out.exists():
        out.unlink()

    env = os.environ.copy()
    env["PYTHONPATH"] = str(ROOT) + os.pathsep + env.get("PYTHONPATH", "")
    proc = subprocess.run(
        [sys.executable, str(RENDER), scene, str(out)],
        cwd=str(ROOT),
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
