"""API 索引の鮮度チェック（生成物のドリフト防止）。"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "tools" / "gen_api_index.py"


def test_api_index_up_to_date():
    """コミット済みの索引が現在の API と一致すること。

    生成せずに検証する。先に生成すると常に一致してドリフトを検出できない。
    """
    check = subprocess.run(
        [sys.executable, str(SCRIPT), "--check"],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
    )
    assert check.returncode == 0, (
        (check.stderr or check.stdout)
        + "\n索引が古いです。`python tools/gen_api_index.py` を実行してコミットしてください。"
    )


def test_api_index_walk_is_not_2d_ecs():
    """Front World is the 3D world. Entity / tilemap stay off the public table."""
    text = (ROOT / "docs" / "API_INDEX.md").read_text(encoding="utf-8")
    assert "| `World` |" in text
    assert "from kagra.world" in text
    assert "| `Entity` |" not in text
    assert "| `EntityScene` |" not in text
    assert "| `TileMap` |" not in text
    assert "| `TileSet` |" not in text
    assert "from kagra.entity)" not in text
    assert "from kagra.tilemap)" not in text
