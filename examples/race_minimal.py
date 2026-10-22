"""race_drive 3-lap oval. `RaceSession` (Rust owns steer, laps, clock, ghost).

The course is a small loop of KayKit road tiles and a sedan. W accelerates,
S brakes, A/D steer. On-screen L / R / ACCEL pads do the same for touch.

A ghost taxi replays the best lap once the first finish is in. The lap clock
is Rust state (`lap_time` / `best_lap`); this file only draws it.

    python examples/race_minimal.py
    python examples/race_minimal.py --headless scratch/race.png

HUD text is ASCII (hyphen, not an em dash).
"""
from __future__ import annotations

import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import kagra  # noqa: E402
from kagra.gameloop import Scene, draw_world, mouse_down, mouse_pos, pressed, run, was_pressed  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/race_world.json"


def _load_session():
    session_cls = kagra.RaceSession
    if session_cls is None:
        from kagra_shared import RaceSession as session_cls  # type: ignore[no-redef]
    return session_cls.from_json(DUMP.read_text(encoding="utf-8"))


def _wrap(a: float) -> float:
    return (a + math.pi) % (2.0 * math.pi) - math.pi


def _waypoints(world: dict) -> list[tuple[float, float]]:
    rows = []
    for prop in world.get("props") or []:
        if prop.get("name") != "waypoint":
            continue
        pos = prop.get("position") or [0.0, 0.0, 0.0]
        rows.append((str(prop.get("id") or ""), float(pos[0]), float(pos[2])))
    rows.sort()
    return [(x, z) for _, x, z in rows]


def _steer(x: float, z: float, yaw: float, wps: list[tuple[float, float]]) -> tuple[float, float]:
    nearest = 0
    best = 1.0e9
    for i, (wx, wz) in enumerate(wps):
        d = (wx - x) * (wx - x) + (wz - z) * (wz - z)
        if d < best:
            best = d
            nearest = i
    ahead = 2 if best < 2.2 * 2.2 else 1
    tx, tz = wps[(nearest + ahead) % len(wps)]
    desired = math.atan2(tx - x, tz - z)
    err = _wrap(desired - yaw)
    steer = max(-1.0, min(1.0, -err * 2.4))
    throttle = 0.62 if abs(steer) > 0.55 else 1.0
    return steer, throttle


def _clock(seconds: float) -> str:
    seconds = max(0.0, float(seconds))
    mins = int(seconds // 60.0)
    sec = seconds - 60.0 * mins
    return f"{mins}:{sec:04.1f}"


def _touch_layout(width: int, height: int) -> tuple[tuple[int, int, int, int], ...]:
    """On-screen L / R / ACCEL. Origin is the top-left, y grows down."""
    y = height - 58
    left = (14, y, 62, 46)
    right = (84, y, 62, 46)
    accel = (width - 96, y, 82, 46)
    return left, right, accel


def _hit(rect: tuple[int, int, int, int], x: float, y: float) -> bool:
    rx, ry, rw, rh = rect
    return rx <= x <= rx + rw and ry <= y <= ry + rh


class RacePlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.session.confirm()
        self.world = json.loads(self.session.dump())
        self._wps = _waypoints(self.world)
        self._scripted = False
        self._touch = (False, False, False)

    def _scripted_wish(self) -> tuple[float, float]:
        if self.session.won or not self._wps:
            return 0.0, 0.0
        player = self.world.get("player") or {}
        pos = player.get("position") or [0.0, 0.0, 0.0]
        yaw = float(player.get("yaw") or 0.0)
        if self.session.laps >= 1 and self.session.lap_time < 0.45:
            return 0.0, 0.15
        return _steer(float(pos[0]), float(pos[2]), yaw, self._wps)

    def update(self, dt: float) -> None:
        if self._scripted:
            lx, lz = self._scripted_wish()
            self._touch = (False, False, False)
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
            left, right, accel = _touch_layout(self.width, self.height)
            mx, my = mouse_pos()
            held = mouse_down()
            touch_l = held and _hit(left, mx, my)
            touch_r = held and _hit(right, mx, my)
            touch_a = held and _hit(accel, mx, my)
            if touch_l:
                lx -= 1.0
            if touch_r:
                lx += 1.0
            if touch_a:
                lz = max(lz, 1.0)
            lx = max(-1.0, min(1.0, lx))
            lz = max(-1.0, min(1.0, lz))
            self._touch = (touch_l, touch_r, touch_a)
        self.session.set_input(lx, lz, False, False, False)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())

    def draw(self) -> None:
        laps = int(self.session.laps)
        need = int(self.session.laps_need)
        best = float(self.session.best_lap)
        best_txt = _clock(best) if best > 0.0 else "--"
        if self.session.won:
            status = f"race - finish  best {best_txt}"
        else:
            shown = min(laps + 1, need)
            status = f"race - lap {shown}/{need}  {_clock(self.session.lap_time)}  best {best_txt}"
        ghost = "ghost on" if self.session.ghost else "ghost --"
        left, right, accel = _touch_layout(self.width, self.height)
        touch_l, touch_r, touch_a = self._touch

        def pad_color(active: bool, base: list[int]) -> list[int]:
            if active:
                return [base[0] + 40, base[1] + 40, base[2] + 30, 235]
            return base

        hud = {
            "quads": [
                {"x": 8, "y": 8, "w": 360, "h": 40, "color": [20, 28, 40, 210]},
                {
                    "x": left[0],
                    "y": left[1],
                    "w": left[2],
                    "h": left[3],
                    "color": pad_color(touch_l, [36, 44, 58, 220]),
                },
                {
                    "x": right[0],
                    "y": right[1],
                    "w": right[2],
                    "h": right[3],
                    "color": pad_color(touch_r, [36, 44, 58, 220]),
                },
                {
                    "x": accel[0],
                    "y": accel[1],
                    "w": accel[2],
                    "h": accel[3],
                    "color": pad_color(touch_a, [48, 140, 72, 230]),
                },
            ],
            "texts": [
                {
                    "text": status,
                    "x": 16,
                    "y": 14,
                    "size": 15,
                    "color": [236, 242, 248, 255],
                    "align": "left",
                },
                {
                    "text": f"A/D steer   W accel   {ghost}",
                    "x": 16,
                    "y": 32,
                    "size": 11,
                    "color": [200, 214, 226, 255],
                    "align": "left",
                },
                {
                    "text": "L",
                    "x": left[0] + left[2] * 0.5,
                    "y": left[1] + 14,
                    "size": 16,
                    "color": [240, 244, 248, 255],
                    "align": "center",
                },
                {
                    "text": "R",
                    "x": right[0] + right[2] * 0.5,
                    "y": right[1] + 14,
                    "size": 16,
                    "color": [240, 244, 248, 255],
                    "align": "center",
                },
                {
                    "text": "ACCEL",
                    "x": accel[0] + accel[2] * 0.5,
                    "y": accel[1] + 14,
                    "size": 13,
                    "color": [248, 252, 255, 255],
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
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/race.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = RacePlay()
    if headless:
        scene.width = 640
        scene.height = 360
        scene._scripted = True
        for _ in range(4200):
            scene.update(1.0 / 60.0)
            if scene.session.won:
                break
        scene.draw()
        Path(out).write_bytes(scene._canvas_png or b"")
        player = (scene.world.get("player") or {}).get("position")
        print(
            f"wrote {out} won={scene.session.won} laps={scene.session.laps}/"
            f"{scene.session.laps_need} best={scene.session.best_lap:.2f} "
            f"ghost={scene.session.ghost} phase={scene.session.phase} pos={player}"
        )
        return
    run(scene, width=640, height=360, title="KAGRA - race")


if __name__ == "__main__":
    main()
