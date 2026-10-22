"""裏庭 3v3 旗取りゲーム（Backyard Rascals 風） — ランナー。

    python examples/backyard_brawl_minimal.py
    python examples/backyard_brawl_minimal.py --headless scratch/backyard.png
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.backyard_brawl import (  # noqa: E402
    H,
    W,
    BackyardRascals,
    default_state,
    make_world,
    scripted_match,
)
from kagra.gameloop import rgba_to_png, run  # noqa: E402


def main() -> int:
    headless = "--headless" in sys.argv or os.environ.get("KAGRA_BRAWL_HEADLESS", "").strip() in {
        "1",
        "true",
        "yes",
    }
    if not headless:
        run(BackyardRascals(), width=W, height=H, title="KAGRA — Backyard Rascals 3v3 CTF")
        return 0
    out = "scratch/backyard.png"
    if "--headless" in sys.argv:
        idx = sys.argv.index("--headless")
        if len(sys.argv) > idx + 1 and not sys.argv[idx + 1].startswith("--"):
            out = sys.argv[idx + 1]
    state = scripted_match(default_state(), rounds=1)
    # アクション中の絵（プレイヤーがバットを振り、旗を持って庭の中央を走る瞬間）を保存
    state["player"]["x"] = -0.6
    state["player"]["z"] = 1.8
    state["player"]["yaw"] = 0.25
    state["player"]["swing_t"] = 0.20
    state["red_flag"]["carrier"] = "walker:player"
    state["red_flag"]["x"] = -0.6
    state["red_flag"]["z"] = 1.8
    state["banner"] = "BONK! FLAG STOLEN!"
    state["banner_t"] = 2.0
    scene = BackyardRascals()
    scene.state = state
    scene.world = make_world(state)
    try:
        scene.draw()
    except (ImportError, RuntimeError) as exc:
        print(f"render fallback: {exc}")
        scene._frame_rgba = bytes([32, 24, 28, 255]) * (scene.width * scene.height)
    png = rgba_to_png(scene._frame_rgba or b"", scene.width, scene.height)
    path = Path(out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png)
    world_path = path.parent / "backyard_world.json"
    world_path.write_text(json.dumps(scene.world), encoding="utf-8")
    summary = {
        "blue_score": state["blue_score"],
        "carrier": state["red_flag"]["carrier"],
        "walkers": len(scene.world["walkers"]),
        "props": len(scene.world["props"]),
    }
    print(json.dumps(summary, ensure_ascii=False))
    print(f"wrote {path} ({len(png)} bytes) and {world_path}")
    return 0 if state["blue_score"] >= 1 else 1


if __name__ == "__main__":
    raise SystemExit(main())
