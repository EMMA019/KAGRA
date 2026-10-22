"""action_arena 3D arena. `ActionSession` (Rust owns hitboxes, HP, combos, AI).

Defeat three KayKit minions, then the barbarian boss. WASD moves, J attacks,
K dodges. The on-screen stick and ATK / DODGE buttons do the same for touch.
Space retries after a win or a loss.

    python examples/action_minimal.py
    python examples/action_minimal.py --headless scratch/action.png

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

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/action_world.json"


def _load_session():
    session_cls = kagra.ActionSession
    if session_cls is None:
        from kagra_shared import ActionSession as session_cls  # type: ignore[no-redef]
    return session_cls.from_json(DUMP.read_text(encoding="utf-8"))


def _wrap(a: float) -> float:
    return (a + math.pi) % (2.0 * math.pi) - math.pi


def _targets(world: dict) -> list[tuple[float, float]]:
    player = world.get("player") or {}
    pid = str(player.get("id") or "")
    rows: list[tuple[float, float]] = []
    for walker in world.get("walkers") or []:
        if str(walker.get("id") or "") == pid:
            continue
        if walker.get("name") not in ("foe", "boss"):
            continue
        pos = walker.get("position") or [0.0, 0.0, 0.0]
        rows.append((float(pos[0]), float(pos[2])))
    for prop in world.get("props") or []:
        if prop.get("enabled") is False:
            continue
        if prop.get("name") not in ("foe", "boss"):
            continue
        pos = prop.get("position") or [0.0, 0.0, 0.0]
        rows.append((float(pos[0]), float(pos[2])))
    return rows


def _wish(world: dict) -> tuple[float, float, bool, bool]:
    """Camera-relative stick plus attack / dodge. Matches the Rust session test."""
    player = world.get("player") or {}
    pos = player.get("position") or [0.0, 0.0, 0.0]
    px, pz = float(pos[0]), float(pos[2])
    facing = float(player.get("yaw") or 0.0)
    cams = world.get("cameras") or [{}]
    cam = cams[0] if cams else {}
    eye = cam.get("position") or [0.0, 0.0, 0.0]
    target = cam.get("target") or [0.0, 0.0, 0.0]
    dx = float(eye[0]) - float(target[0])
    dz = float(eye[2]) - float(target[2])
    yaw = math.atan2(-dx, -dz)
    siny, cosy = math.sin(yaw), math.cos(yaw)
    best = None
    for tx, tz in _targets(world):
        dist = math.hypot(tx - px, tz - pz)
        if best is None or dist < best[0]:
            best = (dist, tx, tz)
    if best is None:
        return 0.0, 0.0, False, False
    dist, tx, tz = best
    length = max(1e-4, math.hypot(tx - px, tz - pz))
    nx = (tx - px) / length
    nz = (tz - pz) / length
    lx = nx * -cosy + nz * siny
    lz = nx * siny + nz * cosy
    err = _wrap(math.atan2(nx, nz) - facing)
    if dist < 0.95:
        approach = -1.0
    elif abs(err) > 0.45 or dist > 1.35:
        approach = 1.0
    else:
        approach = 0.35
    return (
        max(-1.0, min(1.0, lx * approach)),
        max(-1.0, min(1.0, lz * approach)),
        dist < 1.7 and abs(err) < 0.65,
        dist < 0.98,
    )


def _touch_layout(width: int, height: int) -> tuple[tuple[float, ...], ...]:
    """Stick (cx, cy, r) plus ATK / DODGE rects. Origin is top-left, y grows down."""
    stick = (86.0, float(height - 92), 58.0)
    dodge = (width - 168, height - 64, 70, 48)
    atk = (width - 88, height - 64, 70, 48)
    return stick, atk, dodge


def _hit(rect: tuple[float, ...], x: float, y: float) -> bool:
    rx, ry, rw, rh = rect
    return rx <= x <= rx + rw and ry <= y <= ry + rh


def _bar(x: int, y: int, w: int, h: int, frac: float, fill: list[int]) -> list[dict]:
    frac = max(0.0, min(1.0, frac))
    rows = [{"x": x, "y": y, "w": w, "h": h, "color": [24, 16, 16, 200]}]
    if frac > 0.0:
        rows.append({"x": x, "y": y, "w": int(w * frac), "h": h, "color": fill})
    return rows


class ActionPlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.session.confirm()
        self.world = json.loads(self.session.dump())
        self._scripted = False
        self._touch = (0.0, 0.0, False, False)
        self._shot: bytes | None = None

    def update(self, dt: float) -> None:
        if self._scripted:
            if self.session.phase != "playing":
                self.session.confirm()
            lx, lz, attack, dodge = _wish(self.world)
            self._touch = (0.0, 0.0, False, False)
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
            attack = pressed("j") or pressed("z") or pressed("f")
            dodge = pressed("k") or pressed("shift") or pressed("c")
            stick, atk, dodge_pad = _touch_layout(self.width, self.height)
            mx, my = mouse_pos()
            held = mouse_down()
            cx, cy, radius = stick
            touch_lx = 0.0
            touch_lz = 0.0
            if held and math.hypot(mx - cx, my - cy) <= radius + 8.0:
                touch_lx = max(-1.0, min(1.0, (mx - cx) / radius))
                touch_lz = max(-1.0, min(1.0, -(my - cy) / radius))
                lx = max(-1.0, min(1.0, lx + touch_lx))
                lz = max(-1.0, min(1.0, lz + touch_lz))
            touch_atk = held and _hit(atk, mx, my)
            touch_dodge = held and _hit(dodge_pad, mx, my)
            attack = attack or touch_atk
            dodge = dodge or touch_dodge
            self._touch = (touch_lx, touch_lz, touch_atk, touch_dodge)
        self.session.set_input(lx, lz, False, attack, dodge)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())

    def draw(self) -> None:
        hp = int(self.session.hp)
        hp_max = max(1, int(self.session.hp_max))
        foes = int(self.session.foes_left)
        combo = int(self.session.combo)
        if self.session.won:
            status = "arena - WIN  retry"
        elif self.session.dead or self.session.phase != "playing":
            status = "arena - LOSE  retry"
        elif self.session.boss_up:
            status = f"arena - BOSS  foes {foes}"
        else:
            status = f"arena - foes {foes}"
        if combo >= 2:
            status = f"{status}  combo {combo}"
        stick, atk, dodge_pad = _touch_layout(self.width, self.height)
        touch_lx, touch_lz, touch_atk, touch_dodge = self._touch
        cx, cy, radius = stick

        def pad_color(active: bool, base: list[int]) -> list[int]:
            if active:
                return [base[0] + 36, base[1] + 36, base[2] + 24, 235]
            return base

        quads = [
            {"x": 8, "y": 8, "w": 420, "h": 58, "color": [20, 16, 18, 210]},
            {
                "x": int(cx - radius),
                "y": int(cy - radius),
                "w": int(radius * 2),
                "h": int(radius * 2),
                "color": [36, 40, 52, 210],
            },
            {
                "x": int(cx - 8 + touch_lx * radius * 0.55),
                "y": int(cy - 8 - touch_lz * radius * 0.55),
                "w": 16,
                "h": 16,
                "color": [230, 236, 244, 235],
            },
            {
                "x": atk[0],
                "y": atk[1],
                "w": atk[2],
                "h": atk[3],
                "color": pad_color(touch_atk, [176, 56, 48, 230]),
            },
            {
                "x": dodge_pad[0],
                "y": dodge_pad[1],
                "w": dodge_pad[2],
                "h": dodge_pad[3],
                "color": pad_color(touch_dodge, [48, 110, 150, 230]),
            },
        ]
        quads.extend(_bar(16, 36, 140, 12, hp / hp_max, [220, 64, 64, 255]))
        if self.session.boss_hp_max:
            quads.extend(
                _bar(
                    self.width - 168,
                    16,
                    150,
                    12,
                    self.session.boss_hp / max(1, self.session.boss_hp_max),
                    [196, 48, 72, 255],
                )
            )
        texts = [
            {
                "text": status,
                "x": 16,
                "y": 12,
                "size": 15,
                "color": [244, 236, 228, 255],
                "align": "left",
            },
            {
                "text": "HP",
                "x": 16,
                "y": 34,
                "size": 10,
                "color": [244, 220, 210, 255],
                "align": "left",
            },
            {
                "text": "WASD move   J attack   K dodge",
                "x": 168,
                "y": 36,
                "size": 11,
                "color": [210, 206, 198, 255],
                "align": "left",
            },
            {
                "text": "ATK",
                "x": atk[0] + atk[2] * 0.5,
                "y": atk[1] + 14,
                "size": 14,
                "color": [255, 244, 236, 255],
                "align": "center",
            },
            {
                "text": "DODGE",
                "x": dodge_pad[0] + dodge_pad[2] * 0.5,
                "y": dodge_pad[1] + 14,
                "size": 12,
                "color": [236, 246, 255, 255],
                "align": "center",
            },
        ]
        if self.session.boss_hp_max:
            texts.append(
                {
                    "text": "BOSS",
                    "x": self.width - 168,
                    "y": 30,
                    "size": 11,
                    "color": [255, 220, 214, 255],
                    "align": "left",
                }
            )
        hud = {"quads": quads, "texts": texts}
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=hud)
        if (
            self._shot is None
            and self.session.boss_up
            and 0 < int(self.session.boss_hp) < int(self.session.boss_hp_max)
        ):
            self._shot = self._canvas_png


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/action.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = ActionPlay()
    if headless:
        scene.width = 640
        scene.height = 360
        scene._scripted = True
        for _ in range(3600):
            scene.update(1.0 / 60.0)
            if scene.session.won:
                break
        scene.draw()
        png = scene._shot or scene._canvas_png or b""
        Path(out).write_bytes(png)
        player = (scene.world.get("player") or {}).get("position")
        print(
            f"wrote {out} won={scene.session.won} dead={scene.session.dead} "
            f"hp={scene.session.hp}/{scene.session.hp_max} kills={scene.session.kills} "
            f"combo_seen={scene._shot is not None} boss_hp={scene.session.boss_hp} "
            f"phase={scene.session.phase} pos={player}"
        )
        return
    run(scene, width=960, height=540, title="KAGRA - arena")


if __name__ == "__main__":
    main()
