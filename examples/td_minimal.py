"""td_lane tower defense. `TdSession` (Rust owns waves, gold, lives, placement).

Five waves of raccoons and wolves. Three towers: ARROW, CANNON, MAGE.
Tap or click a type button, then tap or click a green pad to build.
Space / Enter starts and retries. WASD walks the knight; J builds if the
knight is standing on a pad.

    python examples/td_minimal.py
    python examples/td_minimal.py --headless scratch/td_minimal.png

HUD strings are ASCII (hyphen, not an em dash) and come from Rust.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.gameloop import Scene, draw_world, mouse_clicked, mouse_pos, pressed, run, was_pressed  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/td_world.json"
WIDTH = 960
HEIGHT = 540


def _load_session():
    try:
        from kagra.kagra_shared import TdSession
    except ImportError:
        from kagra_shared import TdSession  # type: ignore[no-redef]
    return TdSession.from_json(DUMP.read_text(encoding="utf-8"))


def _kind_center(kind: int, width: int, height: int) -> tuple[float, float]:
    """Same rects as Rust `kind_buttons`."""
    w = float(width)
    h = float(height)
    scale = min(w, h) / 720.0
    scale = 0.5 if scale < 0.5 else (2.0 if scale > 2.0 else scale)
    bw = 150.0 * scale
    bh = 44.0 * scale
    gap = 12.0 * scale
    y = h - bh - 16.0 * scale
    total = bw * 3.0 + gap * 2.0
    x0 = (w - total) * 0.5
    x = x0 + kind * (bw + gap)
    return x + bw * 0.5, y + bh * 0.5


class TdPlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.width = WIDTH
        self.height = HEIGHT
        self.session = _load_session()
        self.world = json.loads(self.session.dump())

    def update(self, dt: float) -> None:
        if mouse_clicked(1):
            x, y = mouse_pos()
            self.session.pointer(float(x), float(y), self.width, self.height)
        elif self.session.phase != "playing" and (
            was_pressed("space") or was_pressed("enter") or was_pressed("return")
        ):
            self.session.confirm()
        for key, kind in (("1", 0), ("2", 1), ("3", 2)):
            if was_pressed(key):
                self.session.select_tower(kind)
        lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
            1.0 if pressed("a") or pressed("left") else 0.0
        )
        lz = (1.0 if pressed("s") or pressed("down") else 0.0) - (
            1.0 if pressed("w") or pressed("up") else 0.0
        )
        attack = was_pressed("j")
        self.session.set_input(lx, lz, False, attack, False)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())

    def draw(self) -> None:
        hud = json.loads(self.session.hud_json(self.width, self.height))
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=hud)


def _place(scene: TdPlay, slot_id: str, kind: int) -> None:
    cx, cy = _kind_center(kind, scene.width, scene.height)
    scene.session.pointer(cx, cy, scene.width, scene.height)
    assert scene.session.selected == kind, scene.session.selected
    world = json.loads(scene.session.dump())
    slot = next(p for p in world["props"] if p.get("id") == slot_id)
    x, _, z = slot["position"]
    screen = scene.session.world_to_screen(float(x), 0.0, float(z), scene.width, scene.height)
    assert screen is not None, slot_id
    scene.session.pointer(float(screen[0]), float(screen[1]), scene.width, scene.height)


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/td_minimal.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = TdPlay()
    if headless:
        scene.session.pointer(12.0, 12.0, scene.width, scene.height)
        assert scene.session.phase == "playing"
        _place(scene, "prop:slot-a", 0)
        _place(scene, "prop:slot-b", 1)
        _place(scene, "prop:slot-c", 2)
        assert scene.session.gold < 80
        for _ in range(240):
            scene.session.set_input(0.0, 0.0, False, False, False)
            scene.session.tick(1.0 / 60.0)
        scene.world = json.loads(scene.session.dump())
        scene.draw()
        Path(out).write_bytes(scene._canvas_png or b"")
        hud = scene.session.hud_json(scene.width, scene.height)
        assert hud.isascii(), hud
        assert "wave" in hud and "gold" in hud and "lives" in hud
        assert "\u2014" not in hud
        for _ in range(9000):
            if scene.session.won:
                break
            scene.session.set_input(0.0, 0.0, False, False, False)
            scene.session.tick(1.0 / 60.0)
        scene.world = json.loads(scene.session.dump())
        print(
            f"wrote {out} won={scene.session.won} lives={scene.session.lives} "
            f"wave={scene.session.wave}/{scene.session.waves} "
            f"kills={scene.session.kills} leaks={scene.session.leaks} "
            f"gold={scene.session.gold} phase={scene.session.phase}"
        )
        assert scene.session.won, "five waves should clear with three towers"
        scene.session.pointer(12.0, 12.0, scene.width, scene.height)
        assert scene.session.phase == "playing"
        assert scene.session.lives == 15
        assert scene.session.gold == 80
        return
    run(scene, width=WIDTH, height=HEIGHT, title="KAGRA - td lane")


if __name__ == "__main__":
    main()
