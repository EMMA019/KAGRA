"""トルネコライク・ミニマルローグライク — ランナー。

ゲーム本体（ロジック + ダンジョン + 世界）は `kagra.torneko.Torneko`。
ここは窓 / ヘッドレス verify の入り口だけ。

実行（窓）:
    python examples/torneko_minimal.py [--seed 12345]

実行（ヘッドレス verify。スクリプト方針で 200 ターン遊んで PNG + 状態）:
    python examples/torneko_minimal.py --headless scratch/torneko.png --seed 12345 --turns 200

セーブ: デフォルト ~/.kagra/torneko.json（`--save パス` で変更可）
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.gameloop import run  # noqa: E402
from kagra.torneko import W, H, Torneko, scripted_policy  # noqa: E402


def main() -> None:
    headless = "--headless" in sys.argv
    seed = 12345
    if "--seed" in sys.argv:
        seed = int(sys.argv[sys.argv.index("--seed") + 1])
    save_path: Path | None = None
    if "--save" in sys.argv:
        save_path = Path(sys.argv[sys.argv.index("--save") + 1])

    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/torneko.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
        turns = int(sys.argv[sys.argv.index("--turns") + 1]) if "--turns" in sys.argv else 200

        game = Torneko(seed=seed, save_path=save_path)
        steps = scripted_policy(game, turns)
        game._save()
        game.draw()
        png = game._canvas_png or b""
        Path(out).write_bytes(png)
        state = {
            "seed": game.seed,
            "floor": game.floor,
            "hp": game.player["hp"],
            "atk": game.player["atk"],
            "inventory": game.inventory,
            "enemies_left": sum(1 for e in game.enemies if e["hp"] > 0),
            "items_left": len(game.items),
            "state": game.state,
            "steps": len(steps),
        }
        print(json.dumps(state, ensure_ascii=False))
        print(f"wrote {out} ({len(png)} bytes)")
        return

    run(Torneko(seed=seed), width=W, height=H, title="KAGRA — トルネコ (ミニ)")


if __name__ == "__main__":
    main()
