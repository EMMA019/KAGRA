"""Swing a jointed rock into a crate stack. Rules stay in Python.

    python examples/physics_minimal.py
    python examples/physics_minimal.py --headless scratch/physics.png

A/D or Left/Right shove the bob. Tap the left or right half of the screen
to shove. R rebuilds the world. The bob is a revolute joint, so a shove
(or gravity from the raised start) knocks the chest off the crates.

HUD strings are ASCII.
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.gameloop import (  # noqa: E402
    Scene,
    draw_world,
    mouse_clicked,
    mouse_pos,
    pressed,
    rgba_to_png,
    run,
    was_pressed,
)
from kagra.touch import PointerEvent, PointerPhase, VirtualPad  # noqa: E402

W, H = 480, 270


def _prop(
    pid: str,
    name: str,
    model: str,
    position: list[float],
    scale: list[float],
    *,
    gltf: str | None = None,
    static: bool = False,
    body: str | None = None,
    collider: str | None = None,
    mass: float = 0.0,
    friction: float | None = None,
    restitution: float = 0.0,
    color: list[int] | None = None,
) -> dict:
    row: dict = {
        "id": pid,
        "type": "prop",
        "name": name,
        "model": model,
        "position": list(position),
        "scale": list(scale),
        "enabled": True,
        "is_static": static,
        "restitution": restitution,
    }
    if gltf:
        row["gltf"] = gltf
    if body:
        row["body"] = body
    if collider:
        row["collider"] = collider
    if mass > 0.0:
        row["mass"] = mass
    if friction is not None:
        row["friction"] = friction
    if color is not None:
        row["color"] = color
    return row


def make_world() -> dict:
    """Raised revolute bob, two crates, a chest. Same numbers as the Rust test."""
    return {
        "version": 1,
        "half": 12.0,
        "floor_y": 0.0,
        "gravity": 9.8,
        "sky": [130, 165, 205, 255],
        "ground": True,
        "props": [
            _prop(
                "prop:pivot",
                "pivot",
                "box",
                [0.0, 3.15, 0.0],
                [0.25, 0.25, 0.25],
                gltf="crate.glb",
                static=True,
                body="static",
                color=[90, 78, 64],
            ),
            _prop(
                "prop:bob",
                "bob",
                "sphere",
                [1.60, 2.57, 0.0],
                [0.7, 0.7, 0.7],
                gltf="rock",
                body="dynamic",
                mass=5.0,
                friction=0.4,
                restitution=0.05,
                color=[150, 140, 130],
            ),
            _prop(
                "prop:c1",
                "crate",
                "box",
                [0.0, 0.3, 0.0],
                [0.6, 0.6, 0.6],
                gltf="crate.glb",
                body="dynamic",
                friction=0.55,
                color=[186, 132, 72],
            ),
            _prop(
                "prop:c2",
                "crate",
                "box",
                [0.0, 0.9, 0.0],
                [0.6, 0.6, 0.6],
                gltf="crate.glb",
                body="dynamic",
                friction=0.55,
                color=[150, 108, 64],
            ),
            _prop(
                "prop:target",
                "target",
                "box",
                [0.0, 1.45, 0.0],
                [0.5, 0.5, 0.5],
                gltf="chest",
                body="dynamic",
                friction=0.45,
                restitution=0.05,
                color=[196, 164, 72],
            ),
            _prop(
                "prop:ram",
                "ram",
                "box",
                [3.4, 0.45, 2.2],
                [0.55, 0.55, 0.55],
                gltf="crate.glb",
                static=True,
                body="kinematic",
                color=[120, 140, 160],
            ),
            _prop(
                "prop:pine",
                "pine",
                "stylized_pine",
                [-3.4, 0.0, -1.6],
                [0.45, 0.45, 0.45],
                gltf="stylized_pine",
                static=True,
                collider="none",
            ),
        ],
        "joints": [
            {
                "id": "joint:swing",
                "type": "joint",
                "name": "revolute",
                "body_a": "prop:pivot",
                "body_b": "prop:bob",
                "anchor_a": [0.0, 0.0, 0.0],
                "anchor_b": [-1.60, 0.58, 0.0],
                "axis": [0.0, 0.0, 1.0],
            }
        ],
        "lights": [
            {
                "id": "light:key",
                "type": "light",
                "name": "key",
                "position": [2.5, 6.0, 4.0],
                "kind": "point",
                "slot": 0,
                "intensity": 3.2,
                "radius": 16.0,
                "color": [1.0, 0.96, 0.88],
            }
        ],
        "cameras": [
            {
                "id": "camera:main",
                "type": "camera",
                "name": "main",
                "position": [4.4, 2.7, 6.2],
                "target": [0.3, 1.5, 0.0],
                "fov": 46,
            }
        ],
        "heightfield": None,
    }


def _fresh() -> dict:
    return json.loads(json.dumps(make_world()))


def _knocked(pos: list[float] | None) -> bool:
    if pos is None:
        return False
    return abs(pos[0]) > 0.35 or pos[1] < 1.15


def _physics_available() -> bool:
    try:
        import kagra_shared as ks
    except ImportError:
        return False
    return hasattr(ks, "PhysicsWorld")


def _shove_from_x(x: float, width: int) -> float:
    """Left half of the screen shoves -X, right half shoves +X."""
    return -1.0 if x < width * 0.5 else 1.0


class PhysicsPlay(Scene):
    def __init__(self) -> None:
        super().__init__(_fresh())
        from kagra.rigid import PhysicsWorld

        self._template = _fresh()
        self.phys = PhysicsWorld(self.world)
        self.pad = VirtualPad()
        self.won = False
        self.hits = 0
        self._ram_x = 3.4

    def retry(self) -> None:
        from kagra.rigid import PhysicsWorld

        self.world = _fresh()
        self.phys = PhysicsWorld(self.world)
        self.won = False
        self.hits = 0
        self._ram_x = 3.4
        self.pad.clear()

    def update(self, dt: float) -> None:
        if was_pressed("escape"):
            self.quit()
            return
        if was_pressed("r"):
            self.retry()
            return
        shove = 0.0
        if pressed("a") or pressed("left"):
            shove -= 1.0
        if pressed("d") or pressed("right"):
            shove += 1.0
        if mouse_clicked():
            x, y = mouse_pos()
            _ = PointerEvent(0, float(x), float(y), PointerPhase.BEGIN)
            self.pad.set_stick(_shove_from_x(float(x), self.width), 0.0)
            lx, _ly = self.pad.stick()
            shove += lx
            self.pad.clear()
        if shove != 0.0 and not self.won:
            self.phys.apply_force("prop:bob", [shove * 48.0, 0.0, 0.0])
        self.phys.step(dt)
        self.hits += len(self.phys.take_events("collision"))
        self.phys.take_events("contact")
        target = self.phys.position("prop:target")
        if _knocked(target):
            self.won = True
        self.world = self.phys.to_world()

    def _status(self) -> str:
        if self.won:
            return "clear  R retry"
        return "swing  A/D or tap   R retry"

    def draw(self) -> None:
        hud = {
            "quads": [
                {"x": 8, "y": 8, "w": self.width - 16, "h": 36, "color": [16, 22, 28, 220]},
            ],
            "texts": [
                {
                    "text": self._status(),
                    "x": self.width * 0.5,
                    "y": 26,
                    "size": 16,
                    "color": [236, 240, 244, 255],
                    "align": "center",
                }
            ],
        }
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=hud)


def _solve() -> tuple[dict, dict]:
    """Gravity swing, one touch shove, a kinematic nudge, then retry."""
    from kagra.rigid import PhysicsWorld

    template = _fresh()
    probe = PhysicsWorld(template)
    for _ in range(12):
        probe.step(1 / 60)
    moved = probe.position("prop:bob")[0]
    # R retry: a fresh world puts the bob back on the raised start.
    phys = PhysicsWorld(_fresh())
    reset = phys.position("prop:bob")[0]
    assert abs(reset - 1.60) < 0.05, f"retry restores the bob, x={reset} moved={moved}"

    phys.step(1 / 60)
    # Offset in X so the ray misses the pivot and hits the chest or a crate.
    hit = phys.raycast([0.2, 4.0, 0.0], [0.0, -1.0, 0.0], 20.0, exclude="prop:bob")
    assert hit is not None, "raycast hits the stack"
    assert hit["id"] in {"prop:target", "prop:c1", "prop:c2", "ground"}, hit
    assert phys.body_type("prop:ram") == "kinematic"
    assert phys.set_position("prop:ram", [3.1, 0.45, 2.0])

    pad = VirtualPad()
    pad.set_stick(-1.0, 0.0)
    lx, _ly = pad.stick()
    pointer = PointerEvent(0, 40.0, 80.0, PointerPhase.BEGIN)
    if pointer.x < W * 0.5:
        lx = min(lx, _shove_from_x(pointer.x, W))
    phys.apply_impulse("prop:bob", [lx * 2.0, 0.0, 0.0])

    collisions = 0
    won = False
    for _ in range(240):
        phys.step(1 / 60)
        collisions += len(phys.take_events("collision"))
        phys.take_events("contact")
        if _knocked(phys.position("prop:target")):
            won = True
    if not won:
        for _ in range(6):
            phys.apply_impulse("prop:bob", [-10.0, 1.0, 0.0])
            for _ in range(40):
                phys.step(1 / 60)
                collisions += len(phys.take_events("collision"))
                phys.take_events("contact")
                if _knocked(phys.position("prop:target")):
                    won = True
    bob = phys.position("prop:bob")
    target = phys.position("prop:target")
    stats = {
        "won": won,
        "bob": [round(v, 3) for v in bob],
        "target": [round(v, 3) for v in target],
        "collisions": collisions,
        "ray": hit["id"],
        "ram": [round(v, 3) for v in phys.position("prop:ram")],
    }
    assert bob[1] > 0.8, f"bob stays on the joint, y={bob[1]}"
    assert collisions > 0, "collision events fired"
    assert abs(phys.position("prop:ram")[0] - 3.1) < 0.05
    world = phys.to_world()
    return world, stats


def _write(world: dict, out: str, hud_text: str) -> bytes:
    path = Path(out)
    path.parent.mkdir(parents=True, exist_ok=True)
    hud = {
        "quads": [{"x": 8, "y": 8, "w": W - 16, "h": 36, "color": [16, 22, 28, 220]}],
        "texts": [
            {
                "text": hud_text,
                "x": W * 0.5,
                "y": 26,
                "size": 16,
                "color": [236, 240, 244, 255],
                "align": "center",
            }
        ],
    }
    try:
        png = draw_world(world, W, H, hud=hud)
    except (ImportError, RuntimeError) as exc:
        print(f"render fallback: {exc}")
        rgba = bytearray(W * H * 4)
        for y in range(H):
            for x in range(W):
                i = (y * W + x) * 4
                rgba[i] = (40 + (x * 3) // 8) & 255
                rgba[i + 1] = (90 + y // 2) & 255
                rgba[i + 2] = 160
                rgba[i + 3] = 255
        png = rgba_to_png(bytes(rgba), W, H)
    path.write_bytes(png)
    world_path = path.parent / "physics_world.json"
    world_path.write_text(json.dumps(world), encoding="utf-8")
    print(f"wrote {path} ({len(png)} bytes) and {world_path}")
    return png


def main() -> int:
    headless = "--headless" in sys.argv or os.environ.get("KAGRA_PHYSICS_HEADLESS", "").strip() in {
        "1",
        "true",
        "yes",
    }
    if not headless:
        if not _physics_available():
            print("kagra_shared.PhysicsWorld is missing; build with physics")
            return 1
        run(PhysicsPlay(), width=W, height=H, title="KAGRA — Physics")
        return 0
    out = "scratch/physics.png"
    if "--headless" in sys.argv:
        idx = sys.argv.index("--headless")
        if len(sys.argv) > idx + 1 and not sys.argv[idx + 1].startswith("--"):
            out = sys.argv[idx + 1]
    if not _physics_available():
        _write(_fresh(), out, "swing  A/D or tap   R retry")
        print("physics feature absent; wrote the rest pose")
        return 0
    world, stats = _solve()
    label = "clear  R retry" if stats["won"] else "swing  A/D or tap   R retry"
    _write(world, out, label)
    print(json.dumps(stats))
    if not stats["won"]:
        print("swing did not knock the target")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
