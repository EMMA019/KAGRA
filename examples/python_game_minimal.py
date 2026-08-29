"""最小の Python ゲームマスター見本（shared wgpu 30）。

WASD で歩き、水辺（shore）の近くで J を押すと cast → 3 秒後 → bite。
**ゲームロジックは全部 Python。** Rust（kagra_shared）は tick と描画だけ。

実行（デスクトップ窓）:
    python examples/python_game_minimal.py

実行（ヘッドレスで PNG を保存。CI / verify 用）:
    python examples/python_game_minimal.py --headless scratch/hello.png

「こんなゲーム作りたい」の最初のコピー元にしてください。
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import kagra  # noqa: E402
from kagra.audio import se  # noqa: E402
from kagra.gameloop import Scene, draw_world, pressed, run, was_pressed  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/interact_fish_world.json"


class FishPlay(Scene):
    """水辺で J → cast → 3 秒タイマー → bite。接着 API の実演。"""

    def __init__(self) -> None:
        super().__init__()
        self.play = kagra.WorldPlay.from_json(DUMP.read_text(encoding="utf-8"))
        self.play.confirm()  # タイトル → プレイ（プレイ中は無視）
        self.world = json.loads(self.play.dump())
        self.casting = False

    def update(self, dt: float) -> None:
        # WASD → wish（カメラ基準）
        lx = (1.0 if pressed("d") else 0.0) - (1.0 if pressed("a") else 0.0)
        lz = (1.0 if pressed("w") else 0.0) - (1.0 if pressed("s") else 0.0)
        attack = was_pressed("j") or was_pressed("z")
        self.play.set_input(lx, lz, False, attack, False)
        # tick が世界を進める（内部で攻撃入力 → 近くの interact を発火、
        # タイマーもここでカウントダウン）。
        self.play.tick(dt)
        self.world = json.loads(self.play.dump())
        # 出来事を消費してゲームロジックを進める（Python の仕事）
        if not self.casting and self.play.take_events("cast"):
            se("cast")
            self.play.start_timer("cast", 3.0, "bite")
            self.casting = True
        if self.play.take_events("bite"):
            print("🎣 釣れた！")
            se("bite")
            self.casting = False

    def draw(self) -> None:
        hint = "WASD: 歩く    J: 釣る"
        if self.casting:
            hint = "🎣 待て…"
        hud = {
            "quads": [{"x": 8, "y": 8, "w": 304, "h": 22, "color": [20, 28, 20, 200]}],
            "texts": [
                {"text": hint, "x": 160, "y": 13, "size": 14, "color": [240, 230, 180, 255], "align": "center"},
            ],
        }
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=hud)


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/py_game.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = FishPlay()
    if headless:
        # 数フレーム進めて（前進 + J）PNG を保存。
        scene.play.set_input(0.0, 1.0, False, False, False)
        for _ in range(20):
            scene.play.tick(1 / 60)
        scene.play.set_input(0.0, 0.0, False, True, False)  # J → cast プロンプト
        scene.play.tick(1 / 60)
        scene.world = json.loads(scene.play.dump())
        hud = {
            "texts": [
                {"text": "KAGRA python game master", "x": 160, "y": 12, "size": 14, "color": [255, 255, 255, 255], "align": "center"},
            ],
        }
        png = draw_world(scene.world, 320, 180, hud=hud)
        Path(out).write_bytes(png)
        print(f"wrote {out} ({len(png)} bytes)")
        return
    run(scene, width=320, height=180)


if __name__ == "__main__":
    main()
