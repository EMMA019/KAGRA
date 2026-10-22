"""Three-day camp. `SurvivalSession` (Rust owns the clock, hunger, gathering).

Gather wood and mushrooms, craft a campfire, build a gazebo shelter, and
live through three nights. Night hurts unless you stand by the fire or
under the shelter. Hunger hurts when the bar hits empty.

WASD moves. J gathers, F eats, B builds. The on-screen stick and the
GATHER / EAT / BUILD buttons do the same for touch.

    python examples/survival_minimal.py
    python examples/survival_minimal.py --headless scratch/survival.png

HUD text is ASCII (hyphen, not an em dash).
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import kagra  # noqa: E402
from kagra.gameloop import Scene, draw_world, mouse_down, mouse_pos, pressed, run, was_pressed  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/survival_world.json"


def _load_session():
    session_cls = kagra.SurvivalSession
    if session_cls is None:
        from kagra_shared import SurvivalSession as session_cls  # type: ignore[no-redef]
    return session_cls.from_json(DUMP.read_text(encoding="utf-8"))


def _stick_layout(height: int) -> tuple[int, int, int]:
    """On-screen stick. Origin is the top-left, y grows down."""
    return 86, height - 92, 52


def _buttons(width: int, height: int) -> tuple[tuple[int, int, int, int], ...]:
    y = height - 58
    gather = (width - 268, y, 82, 46)
    eat = (width - 178, y, 64, 46)
    build = (width - 106, y, 90, 46)
    return gather, eat, build


def _hit(rect: tuple[int, int, int, int], x: float, y: float) -> bool:
    rx, ry, rw, rh = rect
    return rx <= x <= rx + rw and ry <= y <= ry + rh


class SurvivalPlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.session.confirm()
        self.world = json.loads(self.session.dump())
        self._scripted = False
        self._touch = (0.0, 0.0, False, False, False)

    def update(self, dt: float) -> None:
        if self._scripted:
            lx, lz, jump, attack, dodge = self.session.demo_input()
            self._touch = (0.0, 0.0, False, False, False)
        else:
            if self.session.phase != "playing" and (
                was_pressed("enter") or was_pressed("return")
            ):
                self.session.confirm()
            lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
                1.0 if pressed("a") or pressed("left") else 0.0
            )
            lz = (1.0 if pressed("w") or pressed("up") else 0.0) - (
                1.0 if pressed("s") or pressed("down") else 0.0
            )
            attack = pressed("j") or pressed("e")
            jump = pressed("f") or pressed("space")
            dodge = pressed("b") or pressed("r")
            cx, cy, radius = _stick_layout(self.height)
            gather, eat, build = _buttons(self.width, self.height)
            mx, my = mouse_pos()
            held = mouse_down()
            knob_x = 0.0
            knob_z = 0.0
            if held:
                dx = (mx - cx) / float(radius)
                dy = (my - cy) / float(radius)
                if dx * dx + dy * dy <= 1.7 * 1.7 and not (
                    _hit(gather, mx, my) or _hit(eat, mx, my) or _hit(build, mx, my)
                ):
                    knob_x = max(-1.0, min(1.0, dx))
                    knob_z = max(-1.0, min(1.0, -dy))
                    lx += knob_x
                    lz += knob_z
            touch_g = held and _hit(gather, mx, my)
            touch_e = held and _hit(eat, mx, my)
            touch_b = held and _hit(build, mx, my)
            if touch_g:
                attack = True
            if touch_e:
                jump = True
            if touch_b:
                dodge = True
            lx = max(-1.0, min(1.0, lx))
            lz = max(-1.0, min(1.0, lz))
            self._touch = (knob_x, knob_z, touch_g, touch_e, touch_b)
        self.session.set_input(lx, lz, bool(jump), bool(attack), bool(dodge))
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())

    def draw(self) -> None:
        status = str(self.session.status)
        hp = max(0.0, min(1.0, float(self.session.health) / 100.0))
        hun = max(0.0, min(1.0, float(self.session.hunger) / 100.0))
        cx, cy, radius = _stick_layout(self.height)
        gather, eat, build = _buttons(self.width, self.height)
        knob_x, knob_z, touch_g, touch_e, touch_b = self._touch
        night = bool(self.session.night) and not bool(self.session.won)
        exposed = night and not bool(self.session.safe)

        def pad_color(active: bool, base: list[int]) -> list[int]:
            if active:
                return [min(255, base[0] + 36), min(255, base[1] + 36), min(255, base[2] + 24), 235]
            return base

        quads = []
        if exposed:
            quads.append(
                {"x": 0, "y": 0, "w": self.width, "h": self.height, "color": [28, 16, 48, 64]}
            )
        quads.extend(
            [
            {"x": 8, "y": 8, "w": 520, "h": 62, "color": [18, 24, 20, 214]},
            {"x": 16, "y": 36, "w": 160, "h": 12, "color": [48, 28, 28, 220]},
            {"x": 16, "y": 36, "w": max(2, int(160 * hp)), "h": 12, "color": [196, 72, 64, 240]},
            {"x": 190, "y": 36, "w": 160, "h": 12, "color": [48, 40, 24, 220]},
            {"x": 190, "y": 36, "w": max(2, int(160 * hun)), "h": 12, "color": [232, 176, 64, 240]},
            {
                "x": cx - radius,
                "y": cy - radius,
                "w": radius * 2,
                "h": radius * 2,
                "color": [28, 36, 32, 200],
            },
            {
                "x": cx - 16 + int(knob_x * (radius - 18)),
                "y": cy - 16 + int(-knob_z * (radius - 18)),
                "w": 32,
                "h": 32,
                "color": [220, 228, 216, 230],
            },
            {
                "x": gather[0],
                "y": gather[1],
                "w": gather[2],
                "h": gather[3],
                "color": pad_color(touch_g, [52, 96, 58, 228]),
            },
            {
                "x": eat[0],
                "y": eat[1],
                "w": eat[2],
                "h": eat[3],
                "color": pad_color(touch_e, [120, 92, 40, 228]),
            },
            {
                "x": build[0],
                "y": build[1],
                "w": build[2],
                "h": build[3],
                "color": pad_color(touch_b, [64, 78, 120, 228]),
            },
            ]
        )
        hud = {
            "quads": quads,
            "texts": [
                {
                    "text": status,
                    "x": 16,
                    "y": 12,
                    "size": 15,
                    "color": [236, 242, 232, 255],
                    "align": "left",
                },
                {
                    "text": "WASD move   J gather   F eat   B build",
                    "x": 16,
                    "y": 52,
                    "size": 11,
                    "color": [200, 214, 196, 255],
                    "align": "left",
                },
                {
                    "text": "GATHER",
                    "x": gather[0] + gather[2] * 0.5,
                    "y": gather[1] + 14,
                    "size": 13,
                    "color": [244, 252, 244, 255],
                    "align": "center",
                },
                {
                    "text": "EAT",
                    "x": eat[0] + eat[2] * 0.5,
                    "y": eat[1] + 14,
                    "size": 14,
                    "color": [255, 246, 220, 255],
                    "align": "center",
                },
                {
                    "text": "BUILD",
                    "x": build[0] + build[2] * 0.5,
                    "y": build[1] + 14,
                    "size": 13,
                    "color": [232, 238, 255, 255],
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
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/survival.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = SurvivalPlay()
    if headless:
        scene.width = 640
        scene.height = 360
        scene._scripted = True
        for _ in range(7000):
            scene.update(1.0 / 60.0)
            if scene.session.won or scene.session.dead:
                break
        scene.draw()
        Path(out).write_bytes(scene._canvas_png or b"")
        player = (scene.world.get("player") or {}).get("position")
        print(
            f"wrote {out} won={scene.session.won} dead={scene.session.dead} "
            f"day={scene.session.day}/{scene.session.days_need} "
            f"fire={scene.session.fire} shelter={scene.session.shelter} "
            f"phase={scene.session.phase} status={scene.session.status} pos={player}"
        )
        if not scene.session.won:
            raise SystemExit(1)
        return
    run(scene, width=960, height=540, title="KAGRA - survival")


if __name__ == "__main__":
    main()
