"""2D UI パネル（メッセージ / 選択肢 / バー）の見本。Python のみ。

shared wgpu 30 の上に、トルネコ風のメッセージウィンドウと選択肢メニューを
重ねた 1 枚をヘッドレスで PNG に出す。ゲームロジックも UI も全部 Python
（kagra.ui2d が hud dict を作り、Rust は描くだけ）。

実行:
    python examples/ui_panel_demo.py scratch/ui_panel.png
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.gameloop import draw_world  # noqa: E402
from kagra.ui2d import bar, choice_menu, list_lines, merge, message  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/interact_fish_world.json"


def main() -> None:
    out = sys.argv[1] if len(sys.argv) > 1 else "scratch/ui_panel.png"
    Path(out).parent.mkdir(parents=True, exist_ok=True)

    world = json.loads(DUMP.read_text(encoding="utf-8"))

    hud = merge(
        # タイトルバー
        message(
            "トルネコは 50G を手に入れた！\n「つかってみる？」",
            40,
            108,
            240,
            title="じゅもん",
        ),
        # 選択肢メニュー（0 番目にカーソル）
        choice_menu(["はい", "いいえ"], selected=0, x=40, y=74, w=240),
        # ステータス（HP / 経験値）
        bar(40, 40, 150, 8, ratio=0.7, label="HP  35/50", color=[240, 120, 90, 255]),
        bar(40, 52, 150, 8, ratio=0.35, label="EXP", color=[120, 200, 240, 255]),
        # 装備・在庫リスト（パネルなし）
        list_lines(["つよそうな剣", "やくそう x2", "かぎ"], x=200, y=40, size=11),
    )

    png = draw_world(world, 320, 180, hud=hud)
    Path(out).write_bytes(png)
    print(f"wrote {out} ({len(png)} bytes)")


if __name__ == "__main__":
    main()
