"""puzzle_pad 最小例 — `GenreSession` / `PuzzlePadSession` リファレンス。

WASD で歩き、crate（bundled `crate.glb`）を pad へ押し込む。
タイトル → プレイ → 結果は Rust（`puzzle.rs`）。Python は入力と HUD だけ。

    python examples/puzzle_pad_minimal.py
    python examples/puzzle_pad_minimal.py --headless scratch/puzzle_pad_minimal.png
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import kagra  # noqa: E402
from kagra.gameloop import Scene, draw_world, pressed, run, was_pressed  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/puzzle_pad_world.json"


def _load_session():
    try:
        from kagra.kagra_shared import PuzzlePadSession
    except ImportError:
        from kagra_shared import PuzzlePadSession  # type: ignore[no-redef]
    return PuzzlePadSession.from_json(DUMP.read_text(encoding="utf-8"))


class PuzzlePlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.session.confirm()
        self.world = json.loads(self.session.dump())

    def update(self, dt: float) -> None:
        lx = (1.0 if pressed("d") else 0.0) - (1.0 if pressed("a") else 0.0)
        lz = (1.0 if pressed("w") else 0.0) - (1.0 if pressed("s") else 0.0)
        attack = was_pressed("j") or was_pressed("z")
        self.session.set_input(lx, lz, False, attack, False)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())

    def draw(self) -> None:
        status = self.session.phase
        if self.session.solved:
            status = "solved!"
        elif self.session.pushing:
            status = "pushing..."
        hud = {
            "quads": [{"x": 8, "y": 8, "w": 280, "h": 22, "color": [24, 28, 32, 210]}],
            "texts": [
                {
                    "text": f"puzzle_pad - {status}  (WASD / J look)",
                    "x": 148,
                    "y": 13,
                    "size": 14,
                    "color": [230, 240, 250, 255],
                    "align": "center",
                },
            ],
        }
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=hud)


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/puzzle_pad_minimal.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = PuzzlePlay()
    if headless:
        for _ in range(240):
            scene.update(1.0 / 60.0)
        scene.draw()
        Path(out).write_bytes(scene._canvas_png)
        print(f"wrote {out} solved={scene.session.solved}")
        return
    run(scene)


if __name__ == "__main__":
    main()
