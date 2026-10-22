"""sports_goal 1v1 kickabout. `SportsSession` (Rust owns the ball, the score,
the opponent, and the match clock).

Knight vs rogue on a small pitch. First to 2, or the clock. W/S/A/D move,
J or K shoots. The on-screen stick and KICK button do the same for touch.
SPACE on the result starts a rematch.

    python examples/sports_minimal.py
    python examples/sports_minimal.py --headless scratch/sports.png

HUD text is ASCII (hyphen, not an em dash).
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import kagra  # noqa: E402
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
from kagra.touch import VirtualPad  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/sports_world.json"


def _load_session():
    session_cls = kagra.SportsSession
    if session_cls is None:
        from kagra_shared import SportsSession as session_cls  # type: ignore[no-redef]
    return session_cls.from_json(DUMP.read_text(encoding="utf-8"))


def _clock(seconds: float) -> str:
    whole = max(0, int(float(seconds) + 1e-3))
    return f"{whole // 60}:{whole % 60:02d}"


def _touch_layout(width: int, height: int) -> tuple[tuple[int, int, int, int], tuple[int, int, int, int]]:
    """Stick base and KICK button. Origin is the top-left, y grows down."""
    base = (24, height - 150, 118, 118)
    kick = (width - 112, height - 96, 88, 68)
    return base, kick


def _hit(rect: tuple[int, int, int, int], x: float, y: float) -> bool:
    rx, ry, rw, rh = rect
    return rx <= x <= rx + rw and ry <= y <= ry + rh


def _stick_zone(base: tuple[int, int, int, int]) -> tuple[int, int, int, int]:
    return (base[0] - 8, base[1] - 16, base[2] + 40, base[3] + 28)


class SportsPlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.session.confirm()
        self.world = json.loads(self.session.dump())
        self._scripted = False
        self._stick = (0.0, 0.0)
        self._kick = False

    def _scripted_wish(self) -> tuple[float, float, bool]:
        if self.session.phase != "playing" or self.session.won:
            return 0.0, 0.0, False
        return 0.0, 1.0, True

    def update(self, dt: float) -> None:
        if self._scripted:
            lx, lz, attack = self._scripted_wish()
            self._stick = (0.0, 0.0)
            self._kick = False
        else:
            if self.session.phase != "playing" and (
                was_pressed("space")
                or was_pressed("enter")
                or was_pressed("return")
                or mouse_clicked()
            ):
                self.session.confirm()
            lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
                1.0 if pressed("a") or pressed("left") else 0.0
            )
            lz = (1.0 if pressed("w") or pressed("up") else 0.0) - (
                1.0 if pressed("s") or pressed("down") else 0.0
            )
            attack = pressed("j") or pressed("k") or (
                self.session.phase == "playing" and pressed("space")
            )
            base, kick = _touch_layout(self.width, self.height)
            mx, my = mouse_pos()
            held = mouse_down()
            stick_x, stick_y = 0.0, 0.0
            if held and _hit(_stick_zone(base), mx, my):
                cx = base[0] + base[2] * 0.5
                cy = base[1] + base[3] * 0.5
                radius = base[2] * 0.46
                pad = VirtualPad(deadzone=0.18)
                pad.set_stick((mx - cx) / radius, (my - cy) / radius)
                stick_x, stick_y = pad.stick()
                lx += stick_x
                lz += -stick_y
            touch_kick = held and _hit(kick, mx, my)
            if touch_kick:
                attack = True
            lx = max(-1.0, min(1.0, lx))
            lz = max(-1.0, min(1.0, lz))
            self._stick = (stick_x, stick_y)
            self._kick = touch_kick
        self.session.set_input(lx, lz, False, bool(attack), False)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())

    def draw(self) -> None:
        ps = int(self.session.player_score)
        opp = int(self.session.opp_score)
        need = int(self.session.score_to_win)
        left = _clock(self.session.match_left)
        if self.session.phase != "playing":
            if self.session.won:
                status = f"YOU WIN  {ps}-{opp}"
            elif ps == opp:
                status = f"DRAW  {ps}-{opp}"
            else:
                status = f"YOU LOSE  {ps}-{opp}"
            hint = "SPACE rematch"
        else:
            status = f"soccer  {ps}-{opp}  first to {need}  {left}"
            hint = "WASD move   J kick"
        base, kick = _touch_layout(self.width, self.height)
        sx, sy = self._stick
        cx = base[0] + base[2] * 0.5
        cy = base[1] + base[3] * 0.5
        radius = base[2] * 0.46
        knob = 34
        kx = cx + sx * radius * 0.62 - knob * 0.5
        ky = cy + sy * radius * 0.62 - knob * 0.5

        def pad_color(active: bool, color: list[int]) -> list[int]:
            if active:
                return [color[0] + 36, color[1] + 36, color[2] + 24, 235]
            return color

        hud = {
            "quads": [
                {"x": 8, "y": 8, "w": 420, "h": 40, "color": [16, 36, 24, 214]},
                {
                    "x": base[0],
                    "y": base[1],
                    "w": base[2],
                    "h": base[3],
                    "color": [28, 48, 36, 210],
                },
                {
                    "x": kx,
                    "y": ky,
                    "w": knob,
                    "h": knob,
                    "color": pad_color(abs(sx) + abs(sy) > 0.05, [220, 196, 96, 235]),
                },
                {
                    "x": kick[0],
                    "y": kick[1],
                    "w": kick[2],
                    "h": kick[3],
                    "color": pad_color(self._kick, [48, 150, 78, 230]),
                },
            ],
            "texts": [
                {
                    "text": status,
                    "x": 16,
                    "y": 14,
                    "size": 15,
                    "color": [236, 244, 236, 255],
                    "align": "left",
                },
                {
                    "text": hint,
                    "x": 16,
                    "y": 32,
                    "size": 11,
                    "color": [198, 220, 204, 255],
                    "align": "left",
                },
                {
                    "text": "KICK",
                    "x": kick[0] + kick[2] * 0.5,
                    "y": kick[1] + 24,
                    "size": 16,
                    "color": [248, 252, 248, 255],
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
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/sports.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = SportsPlay()
    if headless:
        scene.width = 640
        scene.height = 360
        scene._scripted = True
        for _ in range(900):
            scene.update(1.0 / 60.0)
            if scene.session.won:
                break
        scene.draw()
        png = scene._canvas_png or b""
        won = bool(scene.session.won)
        score = f"{scene.session.player_score}-{scene.session.opp_score}"
        scene.session.confirm()
        scene.update(1.0 / 60.0)
        Path(out).write_bytes(png)
        print(
            f"wrote {out} won={won} score={score} "
            f"rematch_phase={scene.session.phase} "
            f"rematch_score={scene.session.player_score}"
        )
        return
    run(scene, width=640, height=360, title="KAGRA - soccer")


if __name__ == "__main__":
    main()
