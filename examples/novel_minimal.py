"""ノベル最小例 — `NovelSession`（Rust のページ / 選択肢 / 日本語 HUD）。

タップまたはクリックで進む。選択肢はボタンをタップする。
Space / Enter / J でも進む。左右（A/D）で選択してから決定もできる。

    python examples/novel_minimal.py
    python examples/novel_minimal.py --headless scratch/novel_minimal.png
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.gameloop import Scene, draw_world, mouse_clicked, mouse_pos, pressed, run, was_pressed  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/novel_world.json"
WIDTH = 1280
HEIGHT = 720


def _load_session():
    try:
        from kagra.kagra_shared import NovelSession
    except ImportError:
        from kagra_shared import NovelSession  # type: ignore[no-redef]
    return NovelSession.from_json(DUMP.read_text(encoding="utf-8"))


def _choice_tap(which: int, width: int, height: int) -> tuple[float, float]:
    """Rust `choice_buttons` と同じ比率の中心。which=0 残る / 1 歩く。"""
    w = float(width)
    h = float(height)
    scale = min(w, h) / 720.0
    scale = 0.5 if scale < 0.5 else (2.0 if scale > 2.0 else scale)
    x0 = w * (0.16 if which == 0 else 0.54)
    bw = w * 0.30
    y0 = h * 0.68
    bh = 48.0 * scale
    return x0 + bw * 0.5, y0 + bh * 0.5


class NovelPlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.width = WIDTH
        self.height = HEIGHT
        self.session = _load_session()
        self.session.confirm()
        self.world = json.loads(self.session.dump())

    def update(self, dt: float) -> None:
        if mouse_clicked(1):
            x, y = mouse_pos()
            self.session.pointer(float(x), float(y), self.width, self.height)
        lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
            1.0 if pressed("a") or pressed("left") else 0.0
        )
        advance = (
            was_pressed("space")
            or was_pressed("return")
            or was_pressed("j")
            or was_pressed("z")
        )
        self.session.set_input(lx, 0.0, advance, False, False)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())

    def draw(self) -> None:
        hud = json.loads(self.session.hud_json(self.width, self.height))
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=hud)


def _tap(scene: NovelPlay, x: float, y: float) -> None:
    scene.session.pointer(x, y, scene.width, scene.height)
    scene.session.tick(1.0 / 60.0)
    scene.world = json.loads(scene.session.dump())


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/novel_minimal.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = NovelPlay()
    if headless:
        # Page 0 is already on screen after confirm. Two taps reach the choice.
        _tap(scene, 40.0, 40.0)
        _tap(scene, 40.0, 40.0)
        assert scene.session.page == 2, scene.session.page
        assert scene.session.speaker == "ユキ"
        assert "残る" in scene.session.choice_label(0)
        scene.draw()
        Path(out).write_bytes(scene._canvas_png)
        stay_x, stay_y = _choice_tap(0, scene.width, scene.height)
        _tap(scene, stay_x, stay_y)
        print(
            f"wrote {out} phase={scene.session.phase} "
            f"flag={scene.session.flag} speaker={scene.session.speaker}"
        )
        return
    run(scene, width=WIDTH, height=HEIGHT, title="雨の駅")


if __name__ == "__main__":
    main()
