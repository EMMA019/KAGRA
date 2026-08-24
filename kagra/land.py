"""島の高さ関数。GPU 不要。海・草原・山を 1 枚の高さ場で表す。

Rapier / ボクセルは使わない。``Physics3D.set_height_fn`` と同じ ``(x, z) → y``。
"""
from __future__ import annotations

import math


WATER_Y = 0.0


def island_height(x: float, z: float) -> float:
    """中央が草原、北東が山、西が入り江。"""
    x = float(x)
    z = float(z)
    r = math.hypot(x, z)
    shelf = 0.38 - 0.052 * r
    hill = 4.3 * math.exp(-((x - 9.0) ** 2 + (z - 6.0) ** 2) / 28.0)
    bay = -2.7 * math.exp(-((x + 11.0) ** 2 + z * z) / 36.0)
    return shelf + hill + bay


def biome_at(x: float, z: float, *, water_y: float = WATER_Y, fn=island_height) -> str:
    """``sea`` / ``grass`` / ``mountain``。"""
    h = float(fn(float(x), float(z)))
    if h < float(water_y) - 0.04:
        return "sea"
    if h > 2.2:
        return "mountain"
    return "grass"


def terrain_rgba(
    u: float,
    v: float,
    *,
    half: float = 24.0,
    water_y: float = WATER_Y,
    fn=island_height,
) -> tuple[int, int, int, int]:
    """高さ場テクスチャ。u,v は 0..1。"""
    x = (float(u) * 2.0 - 1.0) * float(half)
    z = (float(v) * 2.0 - 1.0) * float(half)
    kind = biome_at(x, z, water_y=water_y, fn=fn)
    if kind == "sea":
        return 42, 92, 110, 255
    if kind == "mountain":
        return 128, 118, 108, 255
    return 76, 140, 62, 255
