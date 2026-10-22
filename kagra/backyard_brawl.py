"""裏庭 3v3 旗取りゲーム（Backyard Rascals 風） — ゲームロジックは全部 Python。

同梱の ``.glb`` / ``.gltf`` アセット（騎士・魔法使い・ローグ・バーバリアン・
アライグマ・オオカミ・中世の家・宿屋・鐘楼・風車・井戸・東屋・木・松・岩）を
そのまま使って賑やかな庭を組み、ボット 5 体（味方 2 + 敵 3）と旗を奪い合う。

- WASD / 矢印: 走る
- Space / J / クリック: スイング（近くの敵を BONK! で吹き飛ばし、旗を落とさせる）
- Shift: ダッシュ
- 敵陣（奥・赤拠点）の旗を取って自陣（手前・青拠点）に 3 回持ち帰れば勝利！
"""
from __future__ import annotations

import math
from typing import Any

from kagra.audio import se
from kagra.gameloop import Scene, mouse_clicked, pressed, render_rgba, was_pressed
from kagra.ui2d import bar, list_lines, merge, panel

W, H = 800, 450
WIN_CAPTURES = 3
BONK_REACH = 2.6
BONK_COOLDOWN = 0.55
STUN_SEC = 1.4
HOME_POS = (0.0, 7.5)
ENEMY_POS = (0.0, -7.5)

# 庭の建物・木・岩（alias, x, z, yaw, scale）
YARD_SCENERY: tuple[tuple[str, float, float, float, float], ...] = (
    ("inn", -8.2, -9.0, 0.35, 1.45),
    ("house", 8.2, -9.0, -0.35, 1.45),
    ("tower", 0.0, -12.2, 0.0, 1.35),
    ("house", -8.6, 8.2, 2.65, 1.45),
    ("mill", 8.6, 8.2, -2.55, 1.35),
    ("gazebo", -9.5, 0.0, 1.57, 1.45),
    ("well", 8.8, 0.0, -1.57, 1.45),
    ("stylized_tree", -11.5, -4.5, 0.2, 0.80),
    ("stylized_pine", 11.5, -4.5, 1.1, 0.70),
    ("stylized_tree", -11.5, 4.5, 2.1, 0.80),
    ("stylized_pine", 11.5, 4.5, 0.5, 0.70),
    ("stylized_pine", -4.8, -11.5, 0.8, 0.65),
    ("stylized_tree", 4.8, -11.5, 1.9, 0.75),
    ("rock", -5.2, -2.5, 0.4, 0.85),
    ("rock", 5.2, 2.5, 2.2, 0.85),
    ("chest", -2.8, -7.8, 0.2, 0.85),
    ("chest", 2.8, 7.8, 3.14, 0.85),
)


def _bot(
    bid: str,
    team: str,
    gltf: str,
    x: float,
    z: float,
    yaw: float,
    role: str,
) -> dict[str, Any]:
    return {
        "id": bid,
        "team": team,
        "gltf": gltf,
        "x": float(x),
        "z": float(z),
        "yaw": float(yaw),
        "role": role,
        "stun": 0.0,
        "kx": 0.0,
        "kz": 0.0,
        "clip": 0.1,
        "cooldown": 0.0,
    }


def default_state() -> dict[str, Any]:
    return {
        "time": 0.0,
        "blue_score": 0,
        "red_score": 0,
        "won": False,
        "banner": "STEAL THE RED FLAG!",
        "banner_t": 2.5,
        "player": {
            "id": "walker:player",
            "team": "blue",
            "gltf": "knight",
            "x": 0.0,
            "z": 5.5,
            "yaw": math.pi,
            "stun": 0.0,
            "kx": 0.0,
            "kz": 0.0,
            "clip": 0.15,
            "swing_t": 0.0,
            "cooldown": 0.0,
        },
        "bots": [
            _bot("walker:blue_raccoon", "blue", "raccoon", -2.6, 5.8, math.pi, "attacker"),
            _bot("walker:blue_mage", "blue", "mage", 2.4, 6.2, math.pi, "defender"),
            _bot("walker:red_rogue", "red", "rogue", -2.2, -5.5, 0.0, "defender"),
            _bot("walker:red_barbarian", "red", "barbarian", 0.0, -4.2, 0.0, "chaser"),
            _bot("walker:red_wolf", "red", "wolf", 2.4, -5.5, 0.0, "attacker"),
        ],
        "red_flag": {
            "x": ENEMY_POS[0],
            "z": ENEMY_POS[1],
            "carrier": None,
        },
    }


def _all_actors(state: dict[str, Any]) -> list[dict[str, Any]]:
    return [state["player"], *state["bots"]]


def _clamp_yard(x: float, z: float) -> tuple[float, float]:
    return (max(-7.5, min(7.5, x)), max(-8.5, min(8.5, z)))


def _bonk_target(attacker: dict[str, Any], target: dict[str, Any], state: dict[str, Any]) -> None:
    dx = target["x"] - attacker["x"]
    dz = target["z"] - attacker["z"]
    dist = max(0.01, math.hypot(dx, dz))
    target["stun"] = STUN_SEC
    target["kx"] = (dx / dist) * 9.5
    target["kz"] = (dz / dist) * 9.5
    if state["red_flag"]["carrier"] == target["id"]:
        state["red_flag"]["carrier"] = None
        state["red_flag"]["x"] = target["x"]
        state["red_flag"]["z"] = target["z"]
        state["banner"] = "BONK! FLAG DROPPED!"
        state["banner_t"] = 1.6


def step_brawl(
    state: dict[str, Any],
    dt: float,
    move_x: float,
    move_z: float,
    swing: bool,
    dash: bool,
) -> dict[str, bool]:
    """1 フレーム進める。イベント辞書（bonk / pickup / score）を返す。"""
    dt = max(0.0, min(float(dt), 0.1))
    events = {"bonk": False, "pickup": False, "score": False}
    if state.get("won"):
        return events

    state["time"] += dt
    if state["banner_t"] > 0.0:
        state["banner_t"] = max(0.0, state["banner_t"] - dt)

    p = state["player"]
    p["cooldown"] = max(0.0, p["cooldown"] - dt)
    p["swing_t"] = max(0.0, p["swing_t"] - dt)

    # プレイヤー移動 / ノックバック
    if p["stun"] > 0.0:
        p["stun"] = max(0.0, p["stun"] - dt)
        p["x"], p["z"] = _clamp_yard(p["x"] + p["kx"] * dt, p["z"] + p["kz"] * dt)
        p["kx"] *= max(0.0, 1.0 - 5.0 * dt)
        p["kz"] *= max(0.0, 1.0 - 5.0 * dt)
    else:
        mag = math.hypot(move_x, move_z)
        if mag > 1e-4:
            nx, nz = move_x / mag, move_z / mag
            spd = 7.4 if dash else 5.2
            p["x"], p["z"] = _clamp_yard(p["x"] + nx * spd * dt, p["z"] + nz * spd * dt)
            p["yaw"] = math.atan2(nx, nz)
            p["clip"] = (p["clip"] + dt * (1.8 if dash else 1.3)) % 1.2
        else:
            p["clip"] = 0.12

    # プレイヤースイング（BONK!）
    if swing and p["cooldown"] <= 0.0 and p["stun"] <= 0.0:
        p["cooldown"] = BONK_COOLDOWN
        p["swing_t"] = 0.28
        for b in state["bots"]:
            if b["team"] == "red" and math.hypot(b["x"] - p["x"], b["z"] - p["z"]) <= BONK_REACH:
                _bonk_target(p, b, state)
                state["banner"] = "BONK! SENT FLYING!"
                state["banner_t"] = 1.4
                events["bonk"] = True

    # ボット AI
    flag = state["red_flag"]
    for b in state["bots"]:
        b["cooldown"] = max(0.0, b["cooldown"] - dt)
        if b["stun"] > 0.0:
            b["stun"] = max(0.0, b["stun"] - dt)
            b["x"], b["z"] = _clamp_yard(b["x"] + b["kx"] * dt, b["z"] + b["kz"] * dt)
            b["kx"] *= max(0.0, 1.0 - 5.0 * dt)
            b["kz"] *= max(0.0, 1.0 - 5.0 * dt)
            continue

        # 目標地点
        if b["team"] == "blue":
            if flag["carrier"] == b["id"]:
                tx, tz = HOME_POS
            elif flag["carrier"] is None:
                tx, tz = flag["x"], flag["z"]
            else:
                # 旗持ちを護衛
                tx, tz = p["x"] - 1.5, p["z"] - 1.2
            spd = 4.2
        else:
            # 赤チーム: 旗持ち（またはプレイヤー）を追いかける
            carrier = next(
                (a for a in _all_actors(state) if a["id"] == flag["carrier"]),
                p if b["role"] == "chaser" else None,
            )
            if carrier is not None:
                tx, tz = carrier["x"], carrier["z"]
            else:
                tx = ENEMY_POS[0] + math.sin(state["time"] * 1.3 + len(b["id"])) * 3.0
                tz = ENEMY_POS[1] + 1.8
            spd = 3.6

        dx, dz = tx - b["x"], tz - b["z"]
        d = math.hypot(dx, dz)
        if d > 0.25:
            b["x"], b["z"] = _clamp_yard(b["x"] + (dx / d) * spd * dt, b["z"] + (dz / d) * spd * dt)
            b["yaw"] = math.atan2(dx, dz)
            b["clip"] = (b["clip"] + dt * 1.15) % 1.2

    # 旗の取得・運搬・キャプチャ判定
    if flag["carrier"] is None:
        for a in _all_actors(state):
            if a["team"] == "blue" and a["stun"] <= 0.0:
                if math.hypot(a["x"] - flag["x"], a["z"] - flag["z"]) <= 1.65:
                    flag["carrier"] = a["id"]
                    state["banner"] = "FLAG STOLEN! RUN HOME!"
                    state["banner_t"] = 1.8
                    events["pickup"] = True
                    break

    if flag["carrier"] is not None:
        carrier = next((a for a in _all_actors(state) if a["id"] == flag["carrier"]), None)
        if carrier is None:
            flag["carrier"] = None
        else:
            flag["x"] = carrier["x"]
            flag["z"] = carrier["z"]
            if math.hypot(carrier["x"] - HOME_POS[0], carrier["z"] - HOME_POS[1]) <= 2.1:
                state["blue_score"] += 1
                flag["carrier"] = None
                flag["x"], flag["z"] = ENEMY_POS
                events["score"] = True
                if state["blue_score"] >= WIN_CAPTURES:
                    state["won"] = True
                    state["banner"] = "BLUE TEAM WINS 3 - 0!"
                    state["banner_t"] = 99.0
                else:
                    state["banner"] = f"FLAG CAPTURED! ({state['blue_score']} / {WIN_CAPTURES})"
                    state["banner_t"] = 2.2

    return events


def make_world(state: dict[str, Any]) -> dict[str, Any]:
    """裏庭キャプチャー・ザ・フラッグの WorldDoc を組み立てる。"""
    p = state["player"]
    px, pz = p["x"], p["z"]

    props: list[dict[str, Any]] = [
        # 中央の小道と芝生のアクセント
        {
            "id": "prop:lane",
            "type": "prop",
            "name": "lane",
            "model": "box",
            "position": [0.0, 0.02, 0.0],
            "scale": [3.2, 0.04, 15.5],
            "color": [210, 178, 126],
            "enabled": True,
        },
        # 青拠点（手前）と赤拠点（奥）のリング
        {
            "id": "prop:blue_base",
            "type": "prop",
            "name": "blue_base",
            "model": "ring",
            "position": [HOME_POS[0], 0.12, HOME_POS[1]],
            "pitch": 1.5708,
            "scale": [3.4, 3.4, 3.4],
            "color": [64, 168, 255],
            "metallic": 0.65,
            "roughness": 0.2,
            "enabled": True,
        },
        {
            "id": "prop:red_base",
            "type": "prop",
            "name": "red_base",
            "model": "ring",
            "position": [ENEMY_POS[0], 0.12, ENEMY_POS[1]],
            "pitch": 1.5708,
            "scale": [3.4, 3.4, 3.4],
            "color": [255, 82, 72],
            "metallic": 0.65,
            "roughness": 0.2,
            "enabled": True,
        },
    ]

    # 青チームの旗（自陣）
    props.extend(
        [
            {
                "id": "prop:blue_pole",
                "type": "prop",
                "name": "pole",
                "model": "cylinder",
                "position": [HOME_POS[0] - 1.3, 0.0, HOME_POS[1]],
                "scale": [0.10, 2.2, 0.10],
                "color": [230, 220, 195],
                "enabled": True,
            },
            {
                "id": "prop:blue_banner",
                "type": "prop",
                "name": "flag",
                "model": "box",
                "parent": "prop:blue_pole",
                "local": True,
                "position": [0.42, 1.85, 0.0],
                "scale": [0.80, 0.46, 0.06],
                "color": [52, 148, 255],
                "enabled": True,
            },
        ]
    )

    # 奪い合う赤チームの旗（持ち運び中はキャリアの頭上に追従）
    flag = state["red_flag"]
    fy = 1.45 if flag["carrier"] else 0.0
    props.extend(
        [
            {
                "id": "prop:red_pole",
                "type": "prop",
                "name": "red_flag",
                "model": "cylinder",
                "position": [flag["x"], fy, flag["z"]],
                "yaw": state["time"] * 2.0 if flag["carrier"] else 0.0,
                "scale": [0.11, 2.2, 0.11],
                "color": [245, 225, 160],
                "enabled": True,
            },
            {
                "id": "prop:red_banner",
                "type": "prop",
                "name": "red_banner",
                "model": "box",
                "parent": "prop:red_pole",
                "local": True,
                "position": [0.45, 1.85, 0.0],
                "scale": [0.85, 0.50, 0.07],
                "color": [255, 64, 56],
                "enabled": True,
            },
        ]
    )

    # プレイヤーの手に持つ木製バット（親子階層でスイング）
    swing_angle = -1.35 if p["swing_t"] > 0.0 else 0.25
    props.extend(
        [
            {
                "id": "prop:player_hand",
                "type": "prop",
                "name": "hand",
                "model": "box",
                "position": [px, 0.95, pz],
                "yaw": p["yaw"] + (0.9 if p["swing_t"] > 0.0 else 0.0),
                "scale": [1.0, 1.0, 1.0],
                "enabled": False,
            },
            {
                "id": "prop:bat",
                "type": "prop",
                "name": "bat",
                "model": "capsule",
                "parent": "prop:player_hand",
                "local": True,
                "position": [0.55, 0.15, 0.45],
                "pitch": swing_angle,
                "roll": -0.35,
                "scale": [0.14, 1.05, 0.14],
                "color": [214, 148, 76],
                "enabled": True,
            },
        ]
    )

    # 庭の建物・木・岩（同梱の glTF アセット）
    for i, (alias, sx, sz, syaw, sscale) in enumerate(YARD_SCENERY):
        props.append(
            {
                "id": f"prop:yard_{i}",
                "type": "prop",
                "name": alias,
                "model": "box",
                "gltf": alias,
                "position": [sx, 0.0, sz],
                "yaw": syaw,
                "scale": [sscale, sscale, sscale],
                "enabled": True,
            }
        )

    # 空に浮かぶ立体的な雲
    for i, (cx, cy, cz, cs) in enumerate(((-12, 13, -16, 3.2), (11, 14, -15, 3.8), (0, 15, -20, 4.2))):
        props.append(
            {
                "id": f"prop:cloud_{i}",
                "type": "prop",
                "name": "cloud",
                "model": "sphere",
                "position": [cx, cy, cz],
                "scale": [cs * 1.5, cs * 0.6, cs],
                "color": [248, 250, 255],
                "enabled": True,
            }
        )

    walkers: list[dict[str, Any]] = []
    for a in _all_actors(state):
        walkers.append(
            {
                "id": a["id"],
                "type": "walker",
                "name": "hurt" if a["stun"] > 0.0 else a["team"],
                "position": [a["x"], 0.0, a["z"]],
                "yaw": a["yaw"],
                "gltf": a["gltf"],
                "clip": a["clip"],
                "anim_blend": 1.0,
            }
        )

    # 庭全体と奥の家並み・キャラクターが見渡せる明るい追従カメラ
    cam_pos = [px * 0.55, 5.2, pz + 7.2]
    cam_target = [px * 0.35, 1.1, pz - 3.5]

    return {
        "version": 1,
        "half": 28.0,
        "floor_y": 0.0,
        "sky": [128, 188, 240, 255],
        "props": props,
        "walkers": walkers,
        "lights": [
            {
                "id": "light:0",
                "type": "light",
                "name": "sun",
                "position": [14, 26, 16],
                "kind": "point",
                "slot": 0,
                "intensity": 2.6,
                "radius": 120,
                "color": [1.0, 0.96, 0.86],
            },
            {
                "id": "light:1",
                "type": "light",
                "name": "fill",
                "position": [-14, 18, -10],
                "kind": "point",
                "slot": 1,
                "intensity": 0.9,
                "radius": 90,
                "color": [0.68, 0.84, 1.0],
            },
        ],
        "cameras": [
            {
                "id": "camera:main",
                "type": "camera",
                "name": "main",
                "position": cam_pos,
                "target": cam_target,
                "fov": 46,
            }
        ],
    }


class BackyardRascals(Scene):
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
        mx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
            1.0 if pressed("a") or pressed("left") else 0.0
        )
        mz = (1.0 if pressed("s") or pressed("down") else 0.0) - (
            1.0 if pressed("w") or pressed("up") else 0.0
        )
        swing = any(was_pressed(k) for k in ("space", "j", "z")) or mouse_clicked(1)
        ev = step_brawl(self.state, dt, mx, mz, swing, pressed("shift"))
        if ev["bonk"]:
            se("hit")
        if ev["pickup"] or ev["score"]:
            se("coin")
        self.world = make_world(self.state)

    def draw(self) -> None:
        blue = self.state["blue_score"]
        red = self.state["red_score"]
        parts = [
            panel(12, 12, 260, 52, color=(16, 26, 44, 215), border=(88, 176, 255, 255)),
            list_lines(
                [f"BLUE CRITTERS  {blue} / {WIN_CAPTURES}    RED  {red}"],
                x=22,
                y=18,
                size=14,
                color=[245, 248, 255, 255],
            ),
            bar(22, 42, 236, 10, ratio=blue / WIN_CAPTURES, color=[72, 180, 255, 255]),
            panel(12, self.height - 34, 420, 24, color=(16, 26, 44, 190), border=(255, 200, 80, 220)),
            list_lines(
                ["WASD Move   Space/Click BONK!   Shift Dash"],
                x=22,
                y=self.height - 29,
                size=13,
                color=[245, 244, 220, 245],
            ),
        ]
        if self.state["banner_t"] > 0.0 and self.state["banner"]:
            parts.append(
                panel(self.width // 2 - 150, 14, 300, 30, color=(28, 20, 18, 215), border=(255, 204, 72, 255))
            )
            parts.append(
                list_lines(
                    [self.state["banner"]],
                    x=self.width // 2 - 136,
                    y=21,
                    size=14,
                    color=[255, 232, 130, 255],
                )
            )
        self._frame_rgba = render_rgba(self.world, self.width, self.height, hud=merge(*parts))


def scripted_match(state: dict[str, Any] | None = None, rounds: int = 3) -> dict[str, Any]:
    """自動で敵の旗へ走り、邪魔する敵を BONK! で吹き飛ばして 3 回キャプチャする。"""
    state = default_state() if state is None else state
    dt = 1.0 / 60.0
    for _ in range(int(60 * 28)):
        if state["blue_score"] >= rounds:
            break
        p = state["player"]
        flag = state["red_flag"]
        if flag["carrier"] == p["id"]:
            tx, tz = HOME_POS
        else:
            tx, tz = flag["x"], flag["z"]
        dx, dz = tx - p["x"], tz - p["z"]
        near_foe = any(
            b["team"] == "red" and b["stun"] <= 0.0 and math.hypot(b["x"] - p["x"], b["z"] - p["z"]) <= 2.2
            for b in state["bots"]
        )
        step_brawl(state, dt, dx, dz, near_foe, True)
    return state
