"""島の上空を飛ぶリングくぐり。ロジックは全部 Python。

``WorldProp.pitch`` / ``roll`` と ``local: true`` の親子で機体を組む。
ジャンルは付けない（自由な WorldDoc）。``WorldPlay`` には足さない。
"""
from __future__ import annotations

import math
from typing import Any

from kagra.audio import se
from kagra.gameloop import Scene, pressed, render_rgba, was_pressed
from kagra.ui2d import merge, message, panel

W, H = 480, 270
ALT = 14.0
SPEED = 14.0
FAST = 22.0
RING_R = 3.8
RINGS = ((0.0, ALT, -6.0), (0.0, ALT, 16.0), (0.0, ALT, 38.0))


def _mat_y(yaw: float) -> list[list[float]]:
    c, s = math.cos(yaw), math.sin(yaw)
    return [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]


def _mat_x(angle: float) -> list[list[float]]:
    c, s = math.cos(angle), math.sin(angle)
    return [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]


def _mat_z(angle: float) -> list[list[float]]:
    c, s = math.cos(angle), math.sin(angle)
    return [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]


def _mul(a: list[list[float]], b: list[list[float]]) -> list[list[float]]:
    return [
        [sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)]
        for i in range(3)
    ]


def orientation(yaw: float, pitch: float, roll: float) -> list[list[float]]:
    """Rust ``prop_rotation`` と同じ。列はローカル X, Y, Z（Z が機首）。"""
    return _mul(_mat_y(yaw), _mul(_mat_x(-pitch), _mat_z(roll)))


def forward_vector(yaw: float, pitch: float, roll: float) -> tuple[float, float, float]:
    m = orientation(yaw, pitch, roll)
    return (m[0][2], m[1][2], m[2][2])


def step_flight(state: dict[str, Any], dt: float, stick_pitch: float, stick_roll: float, fast: bool) -> bool:
    """1 フレーム。リングを新たにくぐったら True。"""
    dt = max(0.0, min(float(dt), 0.1))
    state["pitch"] = max(-0.7, min(0.7, state["pitch"] + stick_pitch * 0.9 * dt))
    state["pitch"] *= max(0.0, 1.0 - 0.55 * dt)
    state["roll"] = max(-1.1, min(1.1, state["roll"] + stick_roll * 1.4 * dt))
    state["roll"] *= max(0.0, 1.0 - 1.6 * dt)
    state["yaw"] += -state["roll"] * 0.85 * dt
    fx, fy, fz = forward_vector(state["yaw"], state["pitch"], state["roll"])
    speed = FAST if fast else SPEED
    prev_z = state["z"]
    state["x"] += fx * speed * dt
    state["y"] = max(3.0, state["y"] + fy * speed * dt)
    state["z"] += fz * speed * dt
    state["spin"] = state.get("spin", 0.0) + 14.0 * dt
    hit = False
    passed: set[int] = state["passed"]
    for i, (rx, ry, rz) in enumerate(RINGS):
        if i in passed:
            continue
        crossed = (prev_z < rz <= state["z"]) or (state["z"] < rz <= prev_z)
        if crossed and math.hypot(state["x"] - rx, state["y"] - ry) <= RING_R:
            passed.add(i)
            state["score"] = int(state["score"]) + 1
            hit = True
    return hit


def default_state() -> dict[str, Any]:
    return {
        "x": 0.0,
        "y": ALT,
        "z": -30.0,
        "yaw": 0.0,
        "pitch": 0.0,
        "roll": 0.0,
        "spin": 0.0,
        "score": 0,
        "passed": set(),
    }


def _prop(
    pid: str,
    name: str,
    model: str,
    position: list[float],
    scale: list[float],
    color: list[int],
    *,
    parent: str | None = None,
    local: bool = False,
    yaw: float = 0.0,
    pitch: float = 0.0,
    roll: float = 0.0,
    enabled: bool = True,
) -> dict[str, Any]:
    prop: dict[str, Any] = {
        "id": pid,
        "type": "prop",
        "name": name,
        "position": [float(v) for v in position],
        "yaw": float(yaw),
        "pitch": float(pitch),
        "roll": float(roll),
        "model": model,
        "scale": [float(v) for v in scale],
        "enabled": enabled,
        "color": color,
    }
    if parent:
        prop["parent"] = parent
    if local:
        prop["local"] = True
    return prop


def make_world(state: dict[str, Any]) -> dict[str, Any]:
    """自由世界。機体は無効な親ヒンジ + local な部品。"""
    x, y, z = state["x"], state["y"], state["z"]
    yaw, pitch, roll = state["yaw"], state["pitch"], state["roll"]
    m = orientation(yaw, pitch, roll)
    fx, fy, fz = m[0][2], m[1][2], m[2][2]
    rx, ry, rz = m[0][0], m[1][0], m[2][0]
    ux, uy, uz = m[0][1], m[1][1], m[2][1]
    # Behind, above, and off to the side so the airframe reads bigger than the ring.
    cam = [
        x - fx * 7.5 + ux * 2.6 + rx * 3.4,
        y - fy * 7.5 + uy * 2.6 + ry * 3.4,
        z - fz * 7.5 + uz * 2.6 + rz * 3.4,
    ]
    look = [x + fx * 2.0, y + fy * 2.0, z + fz * 2.0]
    white = [236, 240, 244]
    wing = [210, 224, 236]
    gold = [255, 186, 48]
    nose_c = [220, 64, 52]
    cloud = [245, 248, 252]
    props = [
        _prop("prop:plane", "plane", "box", [x, y, z], [1, 1, 1], white, yaw=yaw, pitch=pitch, roll=roll, enabled=False),
        _prop("prop:fuselage", "fuselage", "box", [0, 0, 0.1], [0.42, 0.28, 1.7], white, parent="prop:plane", local=True),
        _prop("prop:wing", "wing", "box", [0, 0.05, 0.15], [3.4, 0.06, 0.55], wing, parent="prop:plane", local=True),
        _prop("prop:tail", "tail", "box", [0, 0.12, -0.72], [1.15, 0.05, 0.32], wing, parent="prop:plane", local=True),
        _prop("prop:fin", "fin", "box", [0, 0.32, -0.78], [0.06, 0.42, 0.34], white, parent="prop:plane", local=True),
        _prop("prop:nose", "nose", "box", [0, 0, 0.98], [0.18, 0.16, 0.22], nose_c, parent="prop:plane", local=True),
        _prop(
            "prop:propeller",
            "propeller",
            "box",
            [0, 0, 1.12],
            [1.15, 0.08, 0.05],
            [40, 44, 52],
            parent="prop:plane",
            local=True,
            roll=state.get("spin", 0.0),
        ),
    ]
    for i, (rx, ry, rz) in enumerate(RINGS):
        props.append(
            _prop(f"prop:ring{i}", "ring", "ring", [rx, ry, rz], [8, 8, 8], gold)
        )
    for i, (cx, cy, cz, s) in enumerate(
        ((-18, 22, -10, 4.0), (14, 26, 8, 5.5), (-8, 30, 36, 3.5), (22, 24, 48, 4.5))
    ):
        props.append(_prop(f"prop:cloud{i}", "cloud", "box", [cx, cy, cz], [s, s * 0.45, s * 0.7], cloud))
    return {
        "version": 1,
        "half": 64.0,
        "floor_y": 0.0,
        "water_y": 0.0,
        "sky": [128, 176, 214, 255],
        "heightfield": {"fn": "island_height", "tile": 16.0},
        "props": props,
        "lights": [
            {
                "id": "light:0",
                "type": "light",
                "name": "sun",
                "position": [30, 50, 10],
                "kind": "point",
                "slot": 0,
                "intensity": 2.4,
                "radius": 180,
                "color": [1.0, 0.95, 0.82],
            }
        ],
        "cameras": [
            {
                "id": "camera:main",
                "type": "camera",
                "name": "chase",
                "position": cam,
                "target": look,
                "fov": 52,
            }
        ],
    }


class FlightGame(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.width = W
        self.height = H
        self.state = default_state()
        self.world = make_world(self.state)

    def update(self, dt: float) -> None:
        if was_pressed("escape"):
            self.quit()
            return
        stick_pitch = (1.0 if pressed("w") or pressed("up") else 0.0) - (
            1.0 if pressed("s") or pressed("down") else 0.0
        )
        stick_roll = (1.0 if pressed("a") or pressed("left") else 0.0) - (
            1.0 if pressed("d") or pressed("right") else 0.0
        )
        if step_flight(self.state, dt, stick_pitch, stick_roll, pressed("shift")):
            se("coin")
        self.world = make_world(self.state)

    def draw(self) -> None:
        score = self.state["score"]
        total = len(RINGS)
        done = score >= total
        title = "CLEAR" if done else f"RINGS  {score} / {total}"
        hud = merge(
            panel(8, 8, 200, 28, color=(12, 22, 36, 180), border=(255, 196, 80, 255)),
            message(title, 16, 12, 180, size=16, color=[255, 236, 200, 255]),
            message("W/S pitch   A/D bank   Shift fast", 8, H - 22, 320, size=12, color=[230, 236, 240, 230]),
        )
        self._frame_rgba = render_rgba(self.world, self.width, self.height, hud=hud)


def scripted_flight(state: dict[str, Any] | None = None, seconds: float = 5.0) -> dict[str, Any]:
    """直進。3 つのリングを順にくぐる。"""
    state = default_state() if state is None else state
    steps = int(seconds / (1.0 / 60.0))
    for _ in range(steps):
        step_flight(state, 1.0 / 60.0, 0.0, 0.0, False)
    return state
