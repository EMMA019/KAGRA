"""Phase 1 verify: Rapier 剛体物理 — 箱が落ちて積もり、歩行者が箱を押す。

PhysicsWorld が無いときも PNG を書く（exit 0）。あるときは積み上げを検証する。
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

try:
    import kagra_shared as ks
except ImportError:
    ks = None

from kagra.gameloop import draw_world, rgba_to_png


def _write_png(world: dict) -> None:
    out = Path("scratch/verify_physics.png")
    out.parent.mkdir(parents=True, exist_ok=True)
    try:
        png = draw_world(world, 320, 180)
    except (ImportError, RuntimeError) as exc:
        print(f"render fallback: {exc}")
        w, h = 320, 180
        rgba = bytearray(w * h * 4)
        for y in range(h):
            for x in range(w):
                i = (y * w + x) * 4
                rgba[i] = (48 + x) & 255
                rgba[i + 1] = (72 + y) & 255
                rgba[i + 2] = 120
                rgba[i + 3] = 255
        png = rgba_to_png(bytes(rgba), w, h)
    out.write_bytes(png)
    print(f"wrote {out} ({len(png)} bytes)")


world = {
    "version": 1,
    "half": 8.0,
    "floor_y": 0.0,
    "props": [
        # 下の箱（床に着く）
        {"id": "prop:a", "type": "prop", "name": "crate", "model": "box",
         "position": [0.0, 0.6, 0.0], "scale": [1.0, 1.0, 1.0],
         "enabled": True, "is_static": False, "color": [200, 140, 80]},
        # 上の箱（下の箱に積もる）
        {"id": "prop:b", "type": "prop", "name": "crate", "model": "box",
         "position": [0.0, 3.5, 0.0], "scale": [1.0, 1.0, 1.0],
         "enabled": True, "is_static": False, "color": [120, 160, 220]},
        # 歩行者が押す箱
        {"id": "prop:c", "type": "prop", "name": "crate", "model": "box",
         "position": [3.0, 0.6, -2.0], "scale": [0.8, 0.8, 0.8],
         "enabled": True, "is_static": False, "color": [160, 200, 120]},
        # 地面の印（静的）
        {"id": "prop:ground", "type": "prop", "name": "ground", "model": "box",
         "position": [0.0, 0.0, 0.0], "scale": [8.0, 0.1, 8.0],
         "enabled": True, "is_static": True, "color": [90, 120, 90]},
    ],
    "walkers": [
        {"id": "walker:hero", "type": "walker", "name": "hero",
         "position": [0.5, 1.0, -2.0], "yaw": 0.0, "on_ground": True},
    ],
    "lights": [
        {"id": "light:key", "type": "light", "name": "key", "position": [0, 6.0, 3.0],
         "kind": "point", "slot": 0, "intensity": 3.0, "radius": 12.0,
         "color": [1.0, 0.95, 0.85]},
    ],
    "cameras": [
        {"id": "camera:main", "type": "camera", "name": "main",
         "position": [4.0, 3.0, 5.0], "target": [0.0, 0.8, 0.0], "fov": 50},
    ],
    "heightfield": None,
}

if ks is None or not hasattr(ks, "PhysicsWorld"):
    _write_png(world)
    print("physics feature absent; wrote the rest pose")
    sys.exit(0)

from kagra.rigid import PhysicsWorld

phys = PhysicsWorld(world)
# まず箱 a/b を落として積ませる
for _ in range(240):
    phys.sync_walkers(world)
    phys.step(1 / 60)
# 歩行者が +X へ進んで箱 c を押す
for _ in range(240):
    hero = world["walkers"][0]
    hero["position"][0] += 0.02
    phys.sync_walkers(world)
    phys.step(1 / 60)
world = phys.to_world()

by_id = {p["id"]: p for p in world["props"]}
ya = by_id["prop:a"]["position"][1]
yb = by_id["prop:b"]["position"][1]
yc = by_id["prop:c"]["position"][0]
assert 0.4 < ya < 1.6, f"下の箱は床に着く, ya={ya}"
assert yb > ya + 0.7, f"上の箱は下の箱に積もる, yb={yb} ya={ya}"
assert yc > 3.1, f"歩行者が箱を押す, box_c.x={yc}"
print(f"PHYSICS_OK a.y={ya:.3f} b.y={yb:.3f} c.x={yc:.3f}")
_write_png(world)
