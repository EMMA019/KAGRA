"""Bar 経営シム — ランナー。

    python examples/bar_sim_minimal.py
    python examples/bar_sim_minimal.py --headless scratch/bar.png --days 7 --seed 1
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.bar_sim import W, H, BarSim  # noqa: E402
from kagra.gameloop import run  # noqa: E402


def _headless_night(scene: BarSim) -> None:
    """仕入れ（可能なら）→ 営業。客には好きな酒を出す。在庫切れは会話。"""
    scene._drain()
    scene._do_menu(0)
    scene._do_prep(0)
    scene._drain()
    scene._do_prep(3)
    scene._open_night()
    while scene.guest is not None or scene.queue:
        if scene.queue:
            scene._drain()
        if scene.guest is None:
            break
        likes = scene.guest["likes"]
        from kagra.bar_sim import DRINKS

        stock = DRINKS[likes]["stock"]
        if scene.game["stock"].get(stock, 0) > 0:
            scene._do_serve(0)
        else:
            scene._do_serve(1)
        scene._drain()


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    days = 1
    seed = 1
    save_path: Path | None = None
    if "--seed" in sys.argv:
        seed = int(sys.argv[sys.argv.index("--seed") + 1])
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/bar.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
        if "--days" in sys.argv:
            days = int(sys.argv[sys.argv.index("--days") + 1])
        save_path = Path(out).with_suffix(".save.json")
    if "--save" in sys.argv:
        save_path = Path(sys.argv[sys.argv.index("--save") + 1])

    scene = BarSim(save_path=save_path, seed=seed)
    if headless:
        for _ in range(days):
            if scene.game["lost"] or scene.game["won"]:
                break
            _headless_night(scene)
        scene._save()
        scene.draw()
        png = scene._canvas_png or b""
        Path(out).write_bytes(png)
        print(json.dumps(scene.game, ensure_ascii=False))
        print(f"wrote {out} ({len(png)} bytes)")
        return
    run(scene, width=W, height=H, title="KAGRA — Lumen Bar")


if __name__ == "__main__":
    main()
