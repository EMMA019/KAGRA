"""島の上を飛ぶリングくぐり — ランナー。

    python examples/flight_minimal.py
    python examples/flight_minimal.py --headless scratch/flight.png

W/S で機首、A/D でバンク（バンクすると旋回）、Shift で加速。
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.flight import H, W, FlightGame, make_world, scripted_flight  # noqa: E402
from kagra.gameloop import rgba_to_png, run  # noqa: E402


def _finish(scene: FlightGame, out: str) -> int:
    try:
        scene.draw()
    except (ImportError, RuntimeError) as exc:
        print(f"render fallback: {exc}")
        scene._frame_rgba = bytes([32, 24, 28, 255]) * (scene.width * scene.height)
    png = rgba_to_png(scene._frame_rgba or b"", scene.width, scene.height) if scene._frame_rgba else b""
    if not png:
        png = rgba_to_png(bytes([32, 24, 28, 255]) * (scene.width * scene.height), scene.width, scene.height)
    path = Path(out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png)
    world_path = path.parent / "flight_world.json"
    world_path.write_text(json.dumps(scene.world), encoding="utf-8")
    state = {k: v for k, v in scene.state.items() if k != "passed"}
    state["passed"] = sorted(scene.state["passed"])
    print(json.dumps(state, ensure_ascii=False))
    print(f"wrote {path} ({len(png)} bytes) and {world_path}")
    if state["score"] < 3:
        print("flight did not clear 3 rings")
        return 1
    return 0


def main() -> int:
    headless = "--headless" in sys.argv or os.environ.get("KAGRA_FLIGHT_HEADLESS", "").strip() in {
        "1",
        "true",
        "yes",
    }
    if not headless:
        run(FlightGame(), width=W, height=H, title="KAGRA — Flight")
        return 0
    out = "scratch/flight.png"
    if "--headless" in sys.argv:
        idx = sys.argv.index("--headless")
        if len(sys.argv) > idx + 1 and not sys.argv[idx + 1].startswith("--"):
            out = sys.argv[idx + 1]
    seconds = 5.0
    if "--seconds" in sys.argv:
        seconds = float(sys.argv[sys.argv.index("--seconds") + 1])
    state = scripted_flight(seconds=seconds)
    scene = FlightGame()
    scene.state = state
    scene.world = make_world(state)
    return _finish(scene, out)


if __name__ == "__main__":
    raise SystemExit(main())
