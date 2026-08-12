"""API 索引の鮮度チェック（生成物のドリフト防止）。"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "tools" / "gen_api_index.py"


def test_api_index_up_to_date():
    # まず生成（ローカルで docs が無くても通す）。CI では --check のみでも可。
    gen = subprocess.run(
        [sys.executable, str(SCRIPT)],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
    )
    assert gen.returncode == 0, gen.stderr or gen.stdout

    check = subprocess.run(
        [sys.executable, str(SCRIPT), "--check"],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
    )
    assert check.returncode == 0, check.stderr or check.stdout
    assert (ROOT / "docs" / "API_INDEX.md").exists()
