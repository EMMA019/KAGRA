"""fps_range arena. `FpsSession` (Rust owns look, hitscan, ammo, waves, HP).

Three waves of KayKit raiders. WASD moves, the mouse looks, J fires, R reloads.
Touch: left stick to move, drag the right side to look, FIRE and RELOAD buttons.

    python examples/fps_minimal.py
    python examples/fps_minimal.py --headless scratch/fps.png

HUD text is ASCII (hyphen, not an em dash).
"""
from __future__ import annotations

import json
import math
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

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/fps_world.json"
LOOK_SENS = 0.0045


def _load_session():
    session_cls = kagra.FpsSession
    if session_cls is None:
        from kagra_shared import FpsSession as session_cls  # type: ignore[no-redef]
    return session_cls.from_json(DUMP.read_text(encoding="utf-8"))


def _wrap(a: float) -> float:
    return (a + math.pi) % (2.0 * math.pi) - math.pi


def _touch_layout(width: int, height: int):
    """Left stick, FIRE, RELOAD. Origin is the top-left, y grows down."""
    stick = (86, height - 92, 58)
    fire = (width - 98, height - 124, 82, 46)
    reload = (width - 98, height - 68, 82, 46)
    return stick, fire, reload


def _hit(rect: tuple[int, int, int, int], x: float, y: float) -> bool:
    rx, ry, rw, rh = rect
    return rx <= x <= rx + rw and ry <= y <= ry + rh


def _in_stick(stick: tuple[int, int, int], x: float, y: float) -> bool:
    cx, cy, r = stick
    return (x - cx) * (x - cx) + (y - cy) * (y - cy) <= (r + 16) * (r + 16)


def _enemies(world: dict) -> list[tuple[float, float, float]]:
    out = []
    for walker in world.get("walkers") or []:
        if walker.get("name") != "enemy":
            continue
        pos = walker.get("position") or [0.0, 0.0, 0.0]
        out.append((float(pos[0]), float(pos[1]), float(pos[2])))
    return out


class FpsPlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.session.confirm()
        self.world = json.loads(self.session.dump())
        self._scripted = False
        self._prev_mouse: tuple[float, float] | None = None
        self._drag: str | None = None
        self._press: tuple[float, float] | None = None
        self._was_held = False
        self._stick_axis = (0.0, 0.0)
        self._tap_fire = False
        self._buttons = (False, False)

    def _scripted_wish(self) -> tuple[float, float, bool, bool]:
        if self.session.won or self.session.lost:
            return 0.0, 0.0, False, False
        player = self.world.get("player") or {}
        pos = player.get("position") or [0.0, 0.0, 0.0]
        px, pz = float(pos[0]), float(pos[2])
        best = None
        for x, _y, z in _enemies(self.world):
            d = (x - px) * (x - px) + (z - pz) * (z - pz)
            if best is None or d < best[0]:
                best = (d, x, z)
        if best is None:
            return 0.0, 0.0, False, False
        d, x, z = best
        desired = math.atan2(x - px, z - pz)
        err = _wrap(desired - float(self.session.look_yaw))
        self.session.add_look(err, -float(self.session.look_pitch))
        lz = -1.0 if math.sqrt(d) < 3.4 else 0.0
        dodge = int(self.session.ammo) == 0
        attack = int(self.session.ammo) > 0 and not self.session.reloading
        return 0.0, lz, attack, dodge

    def _pointer(self) -> tuple[float, float, bool, bool]:
        """Returns lx, lz, fire, reload from keys plus the on-screen pads."""
        width, height = self.width, self.height
        stick, fire_r, reload_r = _touch_layout(width, height)
        mx, my = mouse_pos()
        held = mouse_down()
        just = mouse_clicked()
        if just:
            if _in_stick(stick, mx, my):
                self._drag = "stick"
            elif _hit(fire_r, mx, my):
                self._drag = "fire"
            elif _hit(reload_r, mx, my):
                self._drag = "reload"
            else:
                self._drag = "world"
            self._press = (mx, my)
        if self._drag == "world" and held and self._press is not None:
            moved = math.hypot(mx - self._press[0], my - self._press[1])
            if moved > 8.0 and mx > width * 0.42:
                self._drag = "look"
        released = self._was_held and not held
        if not held:
            self._drag = None
            self._stick_axis = (0.0, 0.0)
        look_blocked = self._drag in ("stick", "fire", "reload")
        if self._prev_mouse is not None and not look_blocked:
            dx = mx - self._prev_mouse[0]
            dy = my - self._prev_mouse[1]
            if dx or dy:
                self.session.add_look(dx * LOOK_SENS, -dy * LOOK_SENS)
        self._prev_mouse = (mx, my)
        self._was_held = held
        self._tap_fire = False
        if released and self._press is not None:
            px, py = self._press
            started_on_pad = (
                _in_stick(stick, px, py) or _hit(fire_r, px, py) or _hit(reload_r, px, py)
            )
            moved = math.hypot(mx - px, my - py)
            self._tap_fire = moved < 8.0 and not started_on_pad
        if self._drag == "stick":
            cx, cy, radius = stick
            lx = (mx - cx) / float(radius)
            ly = (my - cy) / float(radius)
            mag = math.hypot(lx, ly)
            if mag > 1.0:
                lx /= mag
                ly /= mag
            # Screen y grows down; forward is stick-up.
            self._stick_axis = (max(-1.0, min(1.0, lx)), max(-1.0, min(1.0, -ly)))
        lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
            1.0 if pressed("a") or pressed("left") else 0.0
        )
        lz = (1.0 if pressed("w") or pressed("up") else 0.0) - (
            1.0 if pressed("s") or pressed("down") else 0.0
        )
        if self._drag == "stick":
            lx, lz = self._stick_axis
        else:
            lx = max(-1.0, min(1.0, lx))
            lz = max(-1.0, min(1.0, lz))
        fire = pressed("j") or pressed("f") or self._drag == "fire" or self._tap_fire
        reload = pressed("r") or self._drag == "reload"
        self._buttons = (self._drag == "fire", self._drag == "reload")
        return lx, lz, fire, reload

    def update(self, dt: float) -> None:
        phase = self.session.phase
        if not self._scripted and phase != "playing" and (
            was_pressed("space") or was_pressed("enter") or was_pressed("return")
        ):
            self.session.confirm()
        if self._scripted:
            lx, lz, attack, dodge = self._scripted_wish()
            self._buttons = (False, False)
        else:
            lx, lz, attack, dodge = self._pointer()
            if phase != "playing":
                attack = False
                dodge = False
                stick, fire_r, _reload_r = _touch_layout(self.width, self.height)
                mx, my = mouse_pos()
                if mouse_clicked() and _hit(fire_r, mx, my):
                    self.session.confirm()
        self.session.set_input(lx, lz, False, attack, dodge)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())

    def draw(self) -> None:
        hp = int(self.session.hp)
        hp_max = int(self.session.hp_max)
        ammo = int(self.session.ammo)
        mag = int(self.session.mag)
        wave = int(self.session.wave)
        waves = int(self.session.waves)
        if self.session.won:
            status = "clear - retry"
        elif self.session.lost:
            status = "down - retry"
        elif self.session.reloading:
            status = f"HP {hp}/{hp_max}   AMMO reloading   WAVE {wave}/{waves}"
        else:
            status = f"HP {hp}/{hp_max}   AMMO {ammo}/{mag}   WAVE {wave}/{waves}"
        stick, fire_r, reload_r = _touch_layout(self.width, self.height)
        fire_on, reload_on = self._buttons
        cx, cy = self.width * 0.5, self.height * 0.5
        sx, sy, sr = stick
        knob_x = sx + self._stick_axis[0] * (sr - 14) - 10
        knob_y = sy - self._stick_axis[1] * (sr - 14) - 10

        def pad_color(active: bool, base: list[int]) -> list[int]:
            if active:
                return [min(255, base[0] + 36), min(255, base[1] + 36), min(255, base[2] + 24), 235]
            return base

        quads = [
            {"x": 8, "y": 8, "w": 430, "h": 36, "color": [16, 18, 22, 210]},
            {"x": cx - 12, "y": cy - 2, "w": 24, "h": 4, "color": [12, 14, 16, 230]},
            {"x": cx - 2, "y": cy - 12, "w": 4, "h": 24, "color": [12, 14, 16, 230]},
            {"x": cx - 10, "y": cy - 1, "w": 20, "h": 2, "color": [240, 240, 240, 230]},
            {"x": cx - 1, "y": cy - 10, "w": 2, "h": 20, "color": [240, 240, 240, 230]},
            {"x": sx - sr, "y": sy - sr, "w": sr * 2, "h": sr * 2, "color": [28, 32, 40, 160]},
            {"x": knob_x, "y": knob_y, "w": 20, "h": 20, "color": [220, 224, 230, 220]},
            {
                "x": fire_r[0],
                "y": fire_r[1],
                "w": fire_r[2],
                "h": fire_r[3],
                "color": pad_color(fire_on, [168, 64, 48, 230]),
            },
            {
                "x": reload_r[0],
                "y": reload_r[1],
                "w": reload_r[2],
                "h": reload_r[3],
                "color": pad_color(reload_on, [48, 72, 112, 230]),
            },
        ]
        fire_label = "RETRY" if self.session.phase != "playing" else "FIRE"
        texts = [
            {
                "text": status,
                "x": 16,
                "y": 16,
                "size": 15,
                "color": [236, 240, 244, 255],
                "align": "left",
            },
            {
                "text": "WASD move   mouse look   J fire   R reload",
                "x": 16,
                "y": self.height - 22,
                "size": 11,
                "color": [210, 216, 222, 230],
                "align": "left",
            },
            {
                "text": fire_label,
                "x": fire_r[0] + fire_r[2] * 0.5,
                "y": fire_r[1] + 14,
                "size": 14,
                "color": [248, 248, 248, 255],
                "align": "center",
            },
            {
                "text": "RELOAD",
                "x": reload_r[0] + reload_r[2] * 0.5,
                "y": reload_r[1] + 14,
                "size": 13,
                "color": [236, 240, 248, 255],
                "align": "center",
            },
        ]
        self._canvas_png = draw_world(
            self.world, self.width, self.height, hud={"quads": quads, "texts": texts}
        )


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/fps.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = FpsPlay()
    if headless:
        scene.width = 640
        scene.height = 360
        scene._scripted = True
        combat = None
        for _ in range(2400):
            scene.update(1.0 / 60.0)
            if combat is None and int(scene.session.hits) >= 1 and not scene.session.won:
                scene.draw()
                combat = scene._canvas_png
            if scene.session.won or scene.session.lost:
                break
        scene.draw()
        Path(out).write_bytes(scene._canvas_png or b"")
        if combat:
            combat_path = Path(out).with_name("fps_combat.png")
            combat_path.write_bytes(combat)
        print(
            f"wrote {out} won={scene.session.won} lost={scene.session.lost} "
            f"hp={scene.session.hp}/{scene.session.hp_max} "
            f"kills={scene.session.kills} wave={scene.session.wave}/{scene.session.waves} "
            f"phase={scene.session.phase}"
        )
        return
    run(scene, width=960, height=540, title="KAGRA - fps")


if __name__ == "__main__":
    main()
