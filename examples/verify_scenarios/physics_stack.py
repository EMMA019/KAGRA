"""Phase 1 verify: Rapier 剛体物理 — 箱が落ちて積もるのを offscreen で描画。

kagra_shared（physics feature）が無い環境ではスキップせず「スキップ」扱いを
望むが、verify は exit コードで判定するため、無い場合は 0 で抜けて
expect ファイル無し（verify 側でスキップ相当になるよう JSON 側で調整）。
"""
import json
import sys

try:
    import kagra_shared as ks
except ImportError:
    sys.exit(0)

if not hasattr(ks, "PhysicsWorld"):
    sys.exit(0)

from kagra.gameloop import draw_world
from kagra.rigid import PhysicsWorld

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
        # 地面の印（静的）
        {"id": "prop:ground", "type": "prop", "name": "ground", "model": "box",
         "position": [0.0, 0.0, 0.0], "scale": [8.0, 0.1, 8.0],
         "enabled": True, "is_static": True, "color": [90, 120, 90]},
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

phys = PhysicsWorld(world)
for _ in range(300):
    phys.step(1 / 60)
world = phys.to_world()

by_id = {p["id"]: p for p in world["props"]}
ya = by_id["prop:a"]["position"][1]
yb = by_id["prop:b"]["position"][1]
assert 0.4 < ya < 1.6, f"下の箱は床に着く, ya={ya}"
assert yb > ya + 0.7, f"上の箱は下の箱に積もる, yb={yb} ya={ya}"
print(f"PHYSICS_OK a.y={ya:.3f} b.y={yb:.3f}")

png = draw_world(world, 320, 180)
out = "scratch/verify_physics.png"
with open(out, "wb") as f:
    f.write(png)
print(f"wrote {out} ({len(png)} bytes)")
