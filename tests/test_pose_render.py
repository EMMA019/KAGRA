"""四肢ボーン回転が描画に反映されることの回帰テスト。

Alicia のようなマルチスキン VRM では、スキンパレットを共有バッファ 1 本に
書いていたせいで最後のパレットが全ドローに適用され、腕・脚・指が
バインドポーズで固まっていた。T ポーズと四肢ポーズを同一プロセスで
撮り、画像が実際に変化することを確認する。
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
RENDER = ROOT / "tests" / "render_pose_scene.py"
OUT_DIR = ROOT / "scratch" / "pose_actual"


def _find_vrm() -> Path | None:
    candidates = [
        ROOT / "assets" / "Emma.vrm",
        Path.home() / ".cache" / "kagra" / "samples" / "AliciaSolid.vrm",
    ]
    for p in candidates:
        if p.is_file():
            return p
    return None


def test_limb_rotation_changes_render():
    pytest.importorskip("kagra")
    from tests.golden_utils import _read_png_rgba

    vrm = _find_vrm()
    if vrm is None:
        pytest.skip("no VRM available (assets/Emma.vrm or cached sample)")

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tpose = OUT_DIR / "tpose.png"
    posed = OUT_DIR / "posed.png"
    for p in (tpose, posed):
        if p.exists():
            p.unlink()

    env = os.environ.copy()
    env.pop("PYTHONPATH", None)
    proc = subprocess.run(
        [sys.executable, str(RENDER), str(vrm), str(tpose), str(posed)],
        cwd=tempfile.gettempdir(),
        env=env,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise AssertionError(
            f"render failed ({proc.returncode})\n"
            f"stdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
    assert tpose.exists() and posed.exists()

    aw, ah, a = _read_png_rgba(tpose)
    bw, bh, b = _read_png_rgba(posed)
    assert (aw, ah) == (bw, bh)

    diff_pixels = sum(
        1
        for i in range(0, len(a), 4)
        if abs(a[i] - b[i]) > 8 or abs(a[i + 1] - b[i + 1]) > 8 or abs(a[i + 2] - b[i + 2]) > 8
    )
    ratio = diff_pixels / (aw * ah)
    # 腕 45 度・肘 45 度・脚 45 度で数%以上の画素が動くはず
    assert ratio > 0.01, f"limb rotation did not change the render (diff ratio {ratio:.4f})"
