"""fishcook - cast, reel, cook, sell, repeat until the day goal.

Rust `FishCookSession` owns the day. J / Space / the HOLD pad is the attack
button: a tap casts, a hold inside the bar reels, a tap in the heat bar
plates the dish, a tap at the stall sells. WASD (and the pads) walk.

World axes: lx is +X, lz is +Z. W moves toward the water (screen forward
on the shore camera).

    python examples/fishcook_minimal.py
    python examples/fishcook_minimal.py --headless scratch/fishcook.png

HUD text is ASCII (hyphen, not an em dash). The status lines come from Rust.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.gameloop import (  # noqa: E402
    Scene,
    draw_world,
    mouse_clicked,
    mouse_down,
    mouse_pos,
    pressed,
    run,
    was_pressed,
)

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/fishcook_world.json"


def _load_session():
    text = DUMP.read_text(encoding="utf-8")
    try:
        from kagra.kagra_shared import FishCookSession
    except ImportError:
        from kagra_shared import FishCookSession  # type: ignore[no-redef]
    return FishCookSession.from_json(text)


def _hold_pad(width: int, height: int) -> tuple[int, int, int, int]:
    """Same rect as `FishCookSession.hold_pad` (top-left origin)."""
    return (width - 108, height - 64, 96, 52)


def _move_pads(width: int, height: int) -> tuple[tuple[int, int, int, int], ...]:
    y = height - 64
    left = (14, y, 52, 52)
    right = (72, y, 52, 52)
    back = (width - 280, y, 52, 52)
    fwd = (width - 222, y, 52, 52)
    return left, right, back, fwd


def _hit(rect: tuple[int, int, int, int], x: float, y: float) -> bool:
    rx, ry, rw, rh = rect
    return rx <= x <= rx + rw and ry <= y <= ry + rh


def _prop_xz(world: dict, name: str) -> tuple[float, float]:
    for prop in world.get("props") or []:
        if prop.get("name") != name:
            continue
        pos = prop.get("position") or [0.0, 0.0, 0.0]
        return float(pos[0]), float(pos[2])
    return 0.0, 0.0


def _player_xz(world: dict) -> tuple[float, float]:
    pos = (world.get("player") or {}).get("position") or [0.0, 0.0, 0.0]
    return float(pos[0]), float(pos[2])


def _steer(world: dict, tx: float, tz: float) -> tuple[float, float]:
    x, z = _player_xz(world)
    dx, dz = tx - x, tz - z
    dist = (dx * dx + dz * dz) ** 0.5
    if dist < 0.2:
        return 0.0, 0.0
    return dx / dist, dz / dist


class FishCookPlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.world = json.loads(self.session.dump())
        self._scripted = False
        self._hold = False

    def _scripted_input(self) -> tuple[float, float, bool]:
        session = self.session
        beat = session.beat
        if beat == "reel":
            marker = float(session.reel_marker)
            return 0.0, 0.0, session.reel_lo <= marker <= session.reel_hi
        if beat == "cook":
            heat = float(session.heat)
            return 0.0, 0.0, session.heat_lo <= heat <= session.heat_hi
        if beat == "catch":
            lx, lz = _steer(self.world, *_prop_xz(self.world, "stove"))
            return lx, lz, False
        if beat == "cooked":
            if session.at_stall:
                return 0.0, 0.0, True
            lx, lz = _steer(self.world, *_prop_xz(self.world, "stall"))
            return lx, lz, False
        if beat == "player":
            if session.on_dock:
                return 0.0, 0.0, True
            lx, lz = _steer(self.world, *_prop_xz(self.world, "dock"))
            return lx, lz, False
        return 0.0, 0.0, False

    def _live_input(self) -> tuple[float, float, bool]:
        # W is screen-forward on the shore cam, which looks toward -Z.
        lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
            1.0 if pressed("a") or pressed("left") else 0.0
        )
        lz = (1.0 if pressed("s") or pressed("down") else 0.0) - (
            1.0 if pressed("w") or pressed("up") else 0.0
        )
        attack = pressed("j") or pressed("space")
        left, right, back, fwd = _move_pads(self.width, self.height)
        hold = _hold_pad(self.width, self.height)
        mx, my = mouse_pos()
        down = mouse_down()
        if down and _hit(left, mx, my):
            lx -= 1.0
        if down and _hit(right, mx, my):
            lx += 1.0
        if down and _hit(back, mx, my):
            lz += 1.0
        if down and _hit(fwd, mx, my):
            lz -= 1.0
        if down and _hit(hold, mx, my):
            attack = True
        lx = max(-1.0, min(1.0, lx))
        lz = max(-1.0, min(1.0, lz))
        return lx, lz, attack

    def update(self, dt: float) -> None:
        session = self.session
        if session.phase != "playing":
            # Scripted headless starts the day once. A later confirm would
            # wipe a finished round before the result frame is drawn.
            if self._scripted and session.phase == "title":
                session.confirm()
            elif (
                was_pressed("space")
                or was_pressed("enter")
                or was_pressed("return")
                or was_pressed("j")
                or mouse_clicked()
            ):
                session.confirm()
            session.set_input(0.0, 0.0, False, False, False)
            self._hold = False
        else:
            if self._scripted:
                lx, lz, attack = self._scripted_input()
                self._hold = attack
            else:
                lx, lz, attack = self._live_input()
                self._hold = attack
            session.set_input(lx, lz, False, attack, False)
        session.tick(dt)
        self.world = json.loads(session.dump())

    def draw(self) -> None:
        hud = json.loads(self.session.hud_json(self.width, self.height))
        left, right, back, fwd = _move_pads(self.width, self.height)
        hold = _hold_pad(self.width, self.height)
        pads = (
            (left, "A", [36, 48, 58, 220]),
            (right, "D", [36, 48, 58, 220]),
            (back, "S", [36, 48, 58, 220]),
            (fwd, "W", [36, 48, 58, 220]),
            (hold, "HOLD", [48, 140, 96, 230] if self._hold else [28, 88, 64, 230]),
        )
        quads = list(hud.get("quads") or [])
        texts = list(hud.get("texts") or [])
        for rect, _label, color in pads:
            quads.append(
                {"x": rect[0], "y": rect[1], "w": rect[2], "h": rect[3], "color": color}
            )
        for rect, label, _color in pads:
            texts.append(
                {
                    "text": label,
                    "x": rect[0] + rect[2] * 0.5,
                    "y": rect[1] + 16,
                    "size": 14,
                    "color": [244, 248, 244, 255],
                    "align": "center",
                }
            )
        self._canvas_png = draw_world(
            self.world, self.width, self.height, hud={"quads": quads, "texts": texts}
        )


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/fishcook.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = FishCookPlay()
    if headless:
        scene.width = 640
        scene.height = 360
        scene._scripted = True
        for _ in range(6000):
            scene.update(1.0 / 60.0)
            if scene.session.won:
                break
        scene.draw()
        Path(out).write_bytes(scene._canvas_png or b"")
        print(
            f"wrote {out} won={scene.session.won} coins={scene.session.coins}/"
            f"{scene.session.goal} sold={scene.session.sold} "
            f"phase={scene.session.phase} beat={scene.session.beat}"
        )
        return
    run(scene, width=960, height=540, title="KAGRA - fish and cook")


if __name__ == "__main__":
    main()
