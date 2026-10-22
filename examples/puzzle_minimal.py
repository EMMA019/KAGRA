"""puzzle 最小例 - スライド / グリッド / マッチ。ルールは Rust (`PuzzleSession`).

4 面: 3x3 スライド、ライツアウト、マッチ、4x4 スライド。
クリアで次の面、R でその面をやり直し、全部で win。

操作: 矢印 / WASD、画面スワイプ、タップ。J / Z は決定。
H は一手ヒント。タイトルとクリアは Space / Enter / タップ。

    python examples/puzzle_minimal.py
    python examples/puzzle_minimal.py --headless scratch/puzzle_win.png
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.gameloop import Scene, draw_world, mouse_clicked, mouse_down, mouse_pos, pressed, run, was_pressed  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/puzzle_world.json"


def _load_session():
    text = DUMP.read_text(encoding="utf-8")
    try:
        from kagra.kagra_shared import PuzzleSession
    except ImportError:
        from kagra_shared import PuzzleSession  # type: ignore[no-redef]
    return PuzzleSession.from_json(text)


class PuzzlePlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.world = json.loads(self.session.dump())
        self._drag: tuple[int, int] | None = None

    def update(self, dt: float) -> None:
        if was_pressed("escape"):
            self.quit()
            return
        session = self.session
        if session.phase != "playing" or session.solved:
            self._drag = None
            if was_pressed("enter") or was_pressed("space") or mouse_clicked():
                session.confirm()
            elif was_pressed("r"):
                session.retry()
            session.set_input(0.0, 0.0, False, False, False)
        else:
            if was_pressed("r"):
                session.retry()
            if was_pressed("h"):
                session.play_demo_move()
            lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
                1.0 if pressed("a") or pressed("left") else 0.0
            )
            lz = (1.0 if pressed("w") or pressed("up") else 0.0) - (
                1.0 if pressed("s") or pressed("down") else 0.0
            )
            attack = was_pressed("j") or was_pressed("z")
            session.set_input(lx, lz, False, attack, False)
            self._pointer()
        session.tick(dt)
        self.world = json.loads(session.dump())

    def _pointer(self) -> None:
        x, y = mouse_pos()
        if mouse_clicked():
            self._drag = (x, y)
            return
        if self._drag is None or mouse_down():
            return
        x0, y0 = self._drag
        self._drag = None
        hit = self.session.cell_at(float(x0), float(y0), float(self.width), float(self.height))
        if hit is None:
            return
        col, row = hit
        dc = x - x0
        dr = y - y0
        if abs(dc) < 14 and abs(dr) < 14:
            self.session.tap(col, row)
            return
        if abs(dc) >= abs(dr):
            self.session.swipe_at(col, row, 1 if dc > 0 else -1, 0)
        else:
            self.session.swipe_at(col, row, 0, 1 if dr > 0 else -1)

    def _status(self) -> str:
        session = self.session
        n = session.level + 1
        total = session.level_count
        kind = session.kind
        if session.phase == "title":
            return "puzzle - title   Space or tap to start"
        if session.phase == "complete":
            return "puzzle - you win   R retry"
        if session.solved:
            return f"puzzle - L{n} clear   Space or tap for next"
        if kind == "slide":
            return f"L{n}/{total} slide  moves {session.moves}   swipe / arrows, R retry"
        if kind == "grid":
            return f"L{n}/{total} grid  moves {session.moves}   tap a light, R retry"
        return f"L{n}/{total} match  moves {session.moves}   swipe gems, R retry"

    def draw(self) -> None:
        hud = {
            "quads": [
                {"x": 8, "y": 8, "w": self.width - 16, "h": 36, "color": [16, 20, 24, 220]},
            ],
            "texts": [
                {
                    "text": self._status(),
                    "x": self.width * 0.5,
                    "y": 26,
                    "size": 13,
                    "color": [232, 236, 240, 255],
                    "align": "center",
                },
            ],
        }
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=hud)


def _solve(scene: PuzzlePlay) -> None:
    scene.session.confirm()
    guard = 0
    while scene.session.phase != "complete" and guard < 32:
        guard += 1
        if scene.session.solved:
            scene.session.confirm()
        elif not scene.session.play_demo_move():
            break
        scene.session.tick(1.0 / 60.0)
    scene.world = json.loads(scene.session.dump())


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/puzzle_win.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = PuzzlePlay()
    if headless:
        scene.width = 640
        scene.height = 360
        _solve(scene)
        scene.draw()
        Path(out).write_bytes(scene._canvas_png or b"")
        print(
            f"wrote {out} won={scene.session.won} phase={scene.session.phase} "
            f"level={scene.session.level} moves={scene.session.moves}"
        )
        return
    run(scene, width=960, height=540, title="puzzle")


if __name__ == "__main__":
    main()
