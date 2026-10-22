"""島と海と中世の村の上空を飛ぶリングくぐり。ロジックは全部 Python。

``WorldProp.pitch`` / ``roll`` と ``local: true`` の親子でプロペラ機を組む。
地上には同梱の ``.glb`` / ``.gltf``（風車・宿屋・家・鐘楼・井戸・東屋・木・岩）と
アニメーション付きの住人（騎士・魔法使い・アライグマ）を配置する。
ジャンルは付けない（自由な WorldDoc）。``WorldPlay`` には足さない。
"""
from __future__ import annotations

import math
from typing import Any

from kagra.audio import se
from kagra.gameloop import Scene, pressed, render_rgba, was_pressed
from kagra.ui2d import bar, list_lines, merge, panel

W, H = 800, 450
ALT = 11.5
SPEED = 14.0
FAST = 22.0
RING_R = 4.2
RINGS = ((0.0, ALT, -6.0), (0.0, ALT + 1.0, 16.0), (0.0, ALT, 38.0))

# 島の上の村（x, z, yaw, gltf 別名, scale）
VILLAGE_PROPS: tuple[tuple[str, float, float, float, float], ...] = (
    ("inn", -9.5, 4.0, 0.45, 1.5),
    ("house", 9.0, 2.0, -0.55, 1.5),
    ("mill", 11.5, 22.0, -0.35, 1.4),
    ("tower", -11.0, 26.0, 0.30, 1.4),
    ("gazebo", -7.5, -14.0, 0.20, 1.5),
    ("well", 5.5, -10.0, 0.0, 1.5),
    ("house", -13.5, 15.0, 0.85, 1.4),
    ("stylized_tree", -6.5, -4.0, 0.1, 0.85),
    ("stylized_pine", 7.0, -3.0, 0.9, 0.65),
    ("stylized_tree", 6.5, 12.0, 1.7, 0.85),
    ("stylized_pine", -7.5, 18.0, 2.3, 0.70),
    ("stylized_tree", 8.0, 31.0, 0.4, 0.90),
    ("stylized_pine", -8.0, 34.0, 1.2, 0.65),
    ("rock", -4.8, 8.0, 0.6, 0.9),
    ("rock", 5.2, 25.0, 1.8, 1.0),
)

# 地上を歩く住人（id, gltf 別名, x, z, yaw）
VILLAGE_WALKERS: tuple[tuple[str, str, float, float, float], ...] = (
    ("walker:knight", "knight", -3.2, 5.0, 0.35),
    ("walker:mage", "mage", 3.4, 9.0, -0.45),
    ("walker:raccoon", "raccoon", -2.2, 20.0, 0.20),
)


def _island_y(x: float, z: float) -> float:
    r = math.hypot(x, z)
    shelf = 0.38 - 0.052 * r
    hill = 4.3 * math.exp(-((x - 9.0) ** 2 + (z - 6.0) ** 2) / 28.0)
    bay = -2.7 * math.exp(-((x + 11.0) ** 2 + z * z) / 36.0)
    return max(0.05, shelf + hill + bay)


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


def step_flight(
    state: dict[str, Any], dt: float, stick_pitch: float, stick_roll: float, fast: bool
) -> bool:
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
    state["spin"] = state.get("spin", 0.0) + 18.0 * dt
    state["time"] = state.get("time", 0.0) + dt
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
        "time": 0.0,
        "score": 0,
        "passed": set(),
    }


def _prop(
    pid: str,
    name: str,
    model: str,
    position: list[float],
    scale: list[float],
    color: list[int] | None = None,
    *,
    gltf: str | None = None,
    parent: str | None = None,
    local: bool = False,
    yaw: float = 0.0,
    pitch: float = 0.0,
    roll: float = 0.0,
    metallic: float = 0.0,
    roughness: float = 1.0,
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
    }
    if color is not None:
        prop["color"] = color
    if gltf:
        prop["gltf"] = gltf
    if parent:
        prop["parent"] = parent
    if local:
        prop["local"] = True
    if metallic > 0.0:
        prop["metallic"] = float(metallic)
        prop["roughness"] = float(roughness)
    return prop


def make_world(state: dict[str, Any]) -> dict[str, Any]:
    """自由世界。複葉機（親子階層）+ 黄金のリング + 地上の村と住人。"""
    x, y, z = state["x"], state["y"], state["z"]
    yaw, pitch, roll = state["yaw"], state["pitch"], state["roll"]
    m = orientation(yaw, pitch, roll)
    fx, fy, fz = m[0][2], m[1][2], m[2][2]
    rx, ry, rz = m[0][0], m[1][0], m[2][0]
    ux, uy, uz = m[0][1], m[1][1], m[2][1]
    # 機体と眼下の島・村が両方映る斜め上のチェイス視点
    cam = [
        x - fx * 6.8 + ux * 2.8 + rx * 2.4,
        y - fy * 6.8 + uy * 2.8 + ry * 2.4,
        z - fz * 6.8 + uz * 2.8 + rz * 2.4,
    ]
    look = [x + fx * 3.5, y + fy * 3.5 - 1.2, z + fz * 3.5]

    red_body = [228, 58, 48]
    cream_wing = [248, 240, 218]
    navy_trim = [44, 62, 94]
    canopy_glass = [110, 200, 245]
    gold = [255, 200, 52]
    silver = [220, 228, 238]
    cloud = [248, 250, 255]

    spin = state.get("spin", 0.0)
    t = state.get("time", 0.0)
    passed: set[int] = state.get("passed", set())

    # 鮮やかな赤い複葉機（親ヒンジ + 11 パーツ）
    props = [
        _prop("prop:plane", "plane", "box", [x, y, z], [1, 1, 1], red_body, yaw=yaw, pitch=pitch, roll=roll, enabled=False),
        _prop("prop:fuselage", "fuselage", "box", [0, 0, 0.05], [0.46, 0.38, 1.85], red_body, parent="prop:plane", local=True),
        _prop("prop:cowling", "cowling", "cylinder", [0, -0.02, 1.02], [0.44, 0.34, 0.44], navy_trim, parent="prop:plane", local=True, pitch=1.5708),
        _prop("prop:canopy", "canopy", "sphere", [0, 0.24, -0.05], [0.34, 0.24, 0.68], canopy_glass, parent="prop:plane", local=True, metallic=0.6, roughness=0.15),
        _prop("prop:wing", "wing", "box", [0, -0.04, 0.18], [3.6, 0.07, 0.62], cream_wing, parent="prop:plane", local=True),
        _prop("prop:upper_wing", "upper_wing", "box", [0, 0.48, 0.22], [3.4, 0.07, 0.58], cream_wing, parent="prop:plane", local=True),
        _prop("prop:strut_l", "strut", "box", [-1.15, 0.22, 0.20], [0.06, 0.52, 0.08], navy_trim, parent="prop:plane", local=True),
        _prop("prop:strut_r", "strut", "box", [1.15, 0.22, 0.20], [0.06, 0.52, 0.08], navy_trim, parent="prop:plane", local=True),
        _prop("prop:tail", "tail", "box", [0, 0.08, -0.82], [1.28, 0.06, 0.36], cream_wing, parent="prop:plane", local=True),
        _prop("prop:fin", "fin", "box", [0, 0.30, -0.84], [0.07, 0.48, 0.38], red_body, parent="prop:plane", local=True),
        _prop("prop:nose", "nose", "sphere", [0, -0.02, 1.18], [0.22, 0.22, 0.26], gold, parent="prop:plane", local=True, metallic=0.9, roughness=0.15),
        _prop(
            "prop:propeller",
            "propeller",
            "box",
            [0, -0.02, 1.26],
            [1.35, 0.10, 0.04],
            silver,
            parent="prop:plane",
            local=True,
            roll=spin,
        ),
    ]

    for i, (rx, ry, rz) in enumerate(RINGS):
        ring_col = [90, 235, 145] if i in passed else gold
        props.append(
            _prop(
                f"prop:ring{i}",
                "ring",
                "ring",
                [rx, ry, rz],
                [8.4, 8.4, 8.4],
                ring_col,
                metallic=0.85,
                roughness=0.16,
            )
        )

    # 立体感のある丸い雲クラスター
    for i, (cx, cy, cz, s) in enumerate(
        ((-16, 20, -10, 4.2), (14, 22, 8, 5.0), (-10, 24, 28, 4.5), (16, 21, 44, 4.8))
    ):
        props.append(_prop(f"prop:cloud{i}a", "cloud", "sphere", [cx, cy, cz], [s * 1.4, s * 0.65, s], cloud))
        props.append(_prop(f"prop:cloud{i}b", "cloud", "sphere", [cx + s * 0.55, cy - 0.3, cz + 0.4], [s, s * 0.5, s * 0.8], cloud))

    # 眼下の島に glTF の建物・木・岩を配置
    for i, (alias, vx, vz, vyaw, vscale) in enumerate(VILLAGE_PROPS):
        gy = _island_y(vx, vz)
        props.append(
            _prop(
                f"prop:village_{i}",
                alias,
                "box",
                [vx, gy, vz],
                [vscale, vscale, vscale],
                gltf=alias,
                yaw=vyaw,
            )
        )

    walkers = []
    for wid, alias, wx, wz, wyaw in VILLAGE_WALKERS:
        walkers.append(
            {
                "id": wid,
                "type": "walker",
                "name": alias,
                "position": [wx, _island_y(wx, wz), wz],
                "yaw": wyaw,
                "gltf": alias,
                "clip": 0.2 + (t * 0.9) % 1.2,
                "anim_blend": 1.0,
            }
        )

    return {
        "version": 1,
        "half": 64.0,
        "floor_y": 0.0,
        "water_y": 0.0,
        "sky": [125, 184, 235, 255],
        "heightfield": {"fn": "island_height", "tile": 16.0},
        "props": props,
        "walkers": walkers,
        "lights": [
            {
                "id": "light:0",
                "type": "light",
                "name": "sun",
                "position": [28, 52, 16],
                "kind": "point",
                "slot": 0,
                "intensity": 2.6,
                "radius": 200,
                "color": [1.0, 0.96, 0.86],
            },
            {
                "id": "light:1",
                "type": "light",
                "name": "sky_fill",
                "position": [-24, 30, -12],
                "kind": "point",
                "slot": 1,
                "intensity": 0.85,
                "radius": 160,
                "color": [0.65, 0.82, 1.0],
            },
        ],
        "cameras": [
            {
                "id": "camera:main",
                "type": "camera",
                "name": "chase",
                "position": cam,
                "target": look,
                "fov": 50,
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
        title = "ALL RINGS CLEAR!" if done else f"RINGS  {score} / {total}"
        alt_m = int(round(self.state["y"]))
        hud = merge(
            panel(12, 12, 236, 48, color=(14, 24, 40, 210), border=(255, 200, 72, 255)),
            list_lines([title], x=22, y=18, size=14, color=[255, 242, 200, 255]),
            bar(22, 38, 150, 10, ratio=score / max(1, total), color=[255, 196, 64, 255]),
            list_lines([f"ALT {alt_m}m"], x=180, y=36, size=11, color=[210, 230, 250, 240]),
            panel(12, self.height - 34, 330, 24, color=(14, 24, 40, 185), border=(90, 140, 190, 220)),
            list_lines(
                ["W/S Pitch   A/D Bank   Shift Boost"],
                x=22,
                y=self.height - 29,
                size=13,
                color=[235, 242, 250, 240],
            ),
        )
        self._frame_rgba = render_rgba(self.world, self.width, self.height, hud=hud)


def scripted_flight(state: dict[str, Any] | None = None, seconds: float = 5.0) -> dict[str, Any]:
    """直進。3 つのリングを順にくぐる。"""
    state = default_state() if state is None else state
    steps = int(seconds / (1.0 / 60.0))
    for _ in range(steps):
        step_flight(state, 1.0 / 60.0, 0.0, 0.0, False)
    return state
