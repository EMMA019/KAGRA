"""スタジオ品質 3D チェス（PolyHaven 彫刻駒 + PBR 大理石/黒檀/真鍮） — ランナー。

    python examples/chess_minimal.py
    python examples/chess_minimal.py --headless scratch/chess.png
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.chess_game import (  # noqa: E402
    H,
    W,
    StudioChess,
    default_state,
    make_world,
    scripted_opening,
)
from kagra.gameloop import rgba_to_png, run  # noqa: E402


def main() -> int:
    headless = "--headless" in sys.argv or os.environ.get("KAGRA_CHESS_HEADLESS", "").strip() in {
        "1",
        "true",
        "yes",
    }
    if not headless:
        run(StudioChess(), width=W, height=H, title="KAGRA — Studio 3D Chess")
        return 0
    out = "scratch/chess.png"
    if "--headless" in sys.argv:
        idx = sys.argv.index("--headless")
        if len(sys.argv) > idx + 1 and not sys.argv[idx + 1].startswith("--"):
            out = sys.argv[idx + 1]
    state = scripted_opening(default_state())
    scene = StudioChess()
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
    world_path = path.parent / "chess_world.json"
    world_path.write_text(json.dumps(scene.world), encoding="utf-8")
    summary = {
        "move_count": state["move_count"],
        "legal_moves": len(state["legal"]),
        "props": len(scene.world["props"]),
    }
    print(json.dumps(summary, ensure_ascii=False))
    print(f"wrote {path} ({len(png)} bytes) and {world_path}")
    return 0 if state["move_count"] >= 6 else 1


if __name__ == "__main__":
    raise SystemExit(main())
