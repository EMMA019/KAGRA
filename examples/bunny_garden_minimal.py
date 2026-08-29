"""バニーガーデン系ミニマルゲーム — ランナー。

ゲーム本体（ロジック + 世界 + HUD）は `kagra.bunny_garden.BunnyGarden`。
ここは窓 / ヘッドレス verify の入り口だけ。

実行（窓）:
    python examples/bunny_garden_minimal.py

実行（ヘッドレス verify。3 日回して PNG + 最終状態 JSON）:
    python examples/bunny_garden_minimal.py --headless scratch/bunny.png --days 3

セーブ: デフォルト ~/.kagra/bunny_garden.json（`--save パス` で変更可）
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.bunny_garden import W, H, BunnyGarden  # noqa: E402
from kagra.gameloop import run  # noqa: E402


def _headless_policy(scene: BunnyGarden) -> None:
    """1 日分: 話す → ほめる → モヒート → 閉店。"""
    scene._drain()
    scene._do_choice(0)                    # 話す
    scene._drain()
    scene._do_choice(2)                    # ほめる
    scene._drain()
    scene._do_choice(1)                    # 飲み物
    scene._do_drink(0)                     # モヒート
    scene._drain()
    scene._do_choice(3)                    # 閉店


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    days = 1
    save_path: Path | None = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/bunny.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
        if "--days" in sys.argv:
            days = int(sys.argv[sys.argv.index("--days") + 1])
    if "--save" in sys.argv:
        save_path = Path(sys.argv[sys.argv.index("--save") + 1])

    scene = BunnyGarden(save_path=save_path)
    if headless:
        if save_path is None:
            scene.save_path = Path(out).with_suffix(".save.json")
        for _ in range(days):
            _headless_policy(scene)
            scene._next_day()   # 閉店確定 → 翌日（対話プレイと同じ流れ）
        scene._save()
        scene.draw()
        png = scene._canvas_png or b""
        Path(out).write_bytes(png)
        print(json.dumps(scene.game, ensure_ascii=False))
        print(f"wrote {out} ({len(png)} bytes)")
        return
    run(scene, width=W, height=H, title="KAGRA — バニーガーデン (ミミ)")


if __name__ == "__main__":
    main()
