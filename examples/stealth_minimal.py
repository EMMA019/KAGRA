"""stealth_hide yard. `StealthSession` (Rust owns patrols, cones, noise, alert).

Sneak past two KayKit guards, pick up the dagger, and leave through the exit.
Vision cones and the alert meter are Rust state; this file only draws them.
Caught means the run is over until Space / Enter retries.

WASD (or the on-screen stick) moves. C / Shift / Ctrl, or the CROUCH pad,
quiets your step and shortens the cones. The headless route is `demo_input`.

    python examples/stealth_minimal.py
    python examples/stealth_minimal.py --headless scratch/stealth.png

HUD text is ASCII (hyphen, not an em dash).
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import kagra  # noqa: E402
from kagra.gameloop import Scene, draw_world, mouse_down, mouse_pos, pressed, run, was_pressed  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/stealth_world.json"


def _load_session():
    session_cls = kagra.StealthSession
    if session_cls is None:
        from kagra_shared import StealthSession as session_cls  # type: ignore[no-redef]
    return session_cls.from_json(DUMP.read_text(encoding="utf-8"))


def _touch_layout(width: int, height: int) -> tuple[tuple[int, int, int], tuple[int, int, int, int]]:
    """Stick center (cx, cy, r) and the CROUCH pad. Origin is top-left."""
    stick = (92, height - 108, 56)
    crouch = (width - 118, height - 72, 100, 52)
    return stick, crouch


def _hit(rect: tuple[int, int, int, int], x: float, y: float) -> bool:
    rx, ry, rw, rh = rect
    return rx <= x <= rx + rw and ry <= y <= ry + rh


def _stick_wish(mx: float, my: float, stick: tuple[int, int, int]) -> tuple[float, float, bool]:
    cx, cy, radius = stick
    dx = (mx - cx) / float(radius)
    dy = (my - cy) / float(radius)
    mag = (dx * dx + dy * dy) ** 0.5
    if mag < 0.18:
        return 0.0, 0.0, False
    if mag > 1.0:
        dx /= mag
        dy /= mag
    # Screen-up is forward (camera looks toward -Z).
    return max(-1.0, min(1.0, dx)), max(-1.0, min(1.0, -dy)), True


class StealthPlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.session.confirm()
        self.world = json.loads(self.session.dump())
        self._scripted = False
        self._touch = (0.0, 0.0, False)

    def update(self, dt: float) -> None:
        if self._scripted:
            lx, lz, crouch = self.session.demo_input()
            self._touch = (0.0, 0.0, False)
        else:
            if self.session.phase != "playing" and (
                was_pressed("space") or was_pressed("enter") or was_pressed("return")
            ):
                self.session.confirm()
            lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
                1.0 if pressed("a") or pressed("left") else 0.0
            )
            lz = (1.0 if pressed("w") or pressed("up") else 0.0) - (
                1.0 if pressed("s") or pressed("down") else 0.0
            )
            crouch = pressed("c") or pressed("shift") or pressed("control") or pressed("z")
            stick, pad = _touch_layout(self.width, self.height)
            mx, my = mouse_pos()
            held = mouse_down()
            touch_c = held and _hit(pad, mx, my)
            sx, sy, stick_on = (0.0, 0.0, False)
            if held:
                cx, cy, radius = stick
                if (mx - cx) ** 2 + (my - cy) ** 2 <= (radius * 1.35) ** 2:
                    sx, sy, stick_on = _stick_wish(mx, my, stick)
            if stick_on:
                lx = max(-1.0, min(1.0, lx + sx))
                lz = max(-1.0, min(1.0, lz + sy))
            if touch_c:
                crouch = True
            self._touch = (sx if stick_on else 0.0, sy if stick_on else 0.0, touch_c or crouch)
        self.session.set_input(lx, lz, False, False, crouch)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())

    def draw(self) -> None:
        alert = max(0.0, min(1.0, float(self.session.alert)))
        state = str(self.session.alert_state)
        key = "key yes" if self.session.has_key else "key --"
        if self.session.clear:
            status = f"stealth - clear  {key}"
        elif self.session.seen:
            status = "stealth - caught  retry"
        else:
            status = f"stealth - {state}  {key}"
        stick, pad = _touch_layout(self.width, self.height)
        cx, cy, radius = stick
        knob_x, knob_y, crouch_on = self._touch
        kx = cx + knob_x * radius * 0.72
        ky = cy - knob_y * radius * 0.72
        meter_w = int(160 * alert)
        meter_col = [232, 186, 48, 255]
        if alert >= 1.0 or self.session.seen:
            meter_col = [210, 48, 48, 255]
        elif alert >= 0.18:
            meter_col = [232, 120, 36, 255]

        def pad_color(active: bool, base: list[int]) -> list[int]:
            if active:
                return [base[0] + 36, base[1] + 36, base[2] + 24, 235]
            return base

        hud = {
            "quads": [
                {"x": 8, "y": 8, "w": 340, "h": 48, "color": [16, 20, 26, 214]},
                {"x": 16, "y": 40, "w": 160, "h": 8, "color": [28, 32, 38, 220]},
                {"x": 16, "y": 40, "w": max(meter_w, 2), "h": 8, "color": meter_col},
                {
                    "x": cx - radius,
                    "y": cy - radius,
                    "w": radius * 2,
                    "h": radius * 2,
                    "color": [32, 40, 48, 200],
                },
                {
                    "x": int(kx) - 16,
                    "y": int(ky) - 16,
                    "w": 32,
                    "h": 32,
                    "color": [220, 210, 170, 230],
                },
                {
                    "x": pad[0],
                    "y": pad[1],
                    "w": pad[2],
                    "h": pad[3],
                    "color": pad_color(crouch_on, [48, 72, 96, 225]),
                },
            ],
            "texts": [
                {
                    "text": status,
                    "x": 16,
                    "y": 12,
                    "size": 15,
                    "color": [236, 240, 232, 255],
                    "align": "left",
                },
                {
                    "text": f"WASD move   C crouch   alert {int(alert * 100):d}%",
                    "x": 16,
                    "y": 28,
                    "size": 11,
                    "color": [190, 198, 186, 255],
                    "align": "left",
                },
                {
                    "text": "CROUCH",
                    "x": pad[0] + pad[2] * 0.5,
                    "y": pad[1] + 16,
                    "size": 13,
                    "color": [244, 248, 252, 255],
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
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/stealth.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = StealthPlay()
    if headless:
        scene.width = 640
        scene.height = 360
        scene._scripted = True
        for _ in range(1800):
            scene.update(1.0 / 60.0)
            if scene.session.clear:
                break
        scene.draw()
        Path(out).write_bytes(scene._canvas_png or b"")
        player = (scene.world.get("player") or {}).get("position")
        print(
            f"wrote {out} clear={scene.session.clear} key={scene.session.has_key} "
            f"alert={scene.session.alert:.2f} state={scene.session.alert_state} "
            f"phase={scene.session.phase} pos={player}"
        )
        return
    run(scene, width=640, height=360, title="KAGRA - stealth")


if __name__ == "__main__":
    main()
