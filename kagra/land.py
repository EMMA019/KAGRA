"""島の高さ関数。GPU 不要。海・草原・山を高さ場で表す。

Rapier / ボクセルは使わない。``Physics3D.set_height_fn`` と同じ ``(x, z) → y``。
タイルキーは歩きながらの load / unload 用。街ファイル形式ではない。
"""
from __future__ import annotations

import math
from typing import Callable, Optional


WATER_Y = 0.0
TILE = 10.0


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


def tile_index(x: float, z: float, tile: float = TILE) -> tuple[int, int]:
    """``(x, z)`` が入っているタイルの整数キー。"""
    t = float(tile)
    return (math.floor(float(x) / t), math.floor(float(z) / t))


def tile_origin(ix: int, iz: int, tile: float = TILE) -> tuple[float, float]:
    """タイル南西角（最小 XZ）。"""
    t = float(tile)
    return (int(ix) * t, int(iz) * t)


def tile_keys(
    x: float,
    z: float,
    *,
    tile: float = TILE,
    radius: float = 28.0,
    half: float | None = None,
) -> list[tuple[int, int]]:
    """``(x, z)`` から ``radius`` 以内のタイル。``half`` なら世界の外は捨てる。"""
    t = float(tile)
    r = float(radius)
    x, z = float(x), float(z)
    i0 = math.floor((x - r) / t)
    i1 = math.floor((x + r) / t)
    j0 = math.floor((z - r) / t)
    j1 = math.floor((z + r) / t)
    pad = t * 0.71
    out: list[tuple[int, int]] = []
    for iz in range(j0, j1 + 1):
        for ix in range(i0, i1 + 1):
            cx = (ix + 0.5) * t
            cz = (iz + 0.5) * t
            if math.hypot(cx - x, cz - z) > r + pad:
                continue
            if half is not None:
                h = float(half)
                ox, oz = ix * t, iz * t
                if ox > h or oz > h or ox + t < -h or oz + t < -h:
                    continue
            out.append((ix, iz))
    return out


def stair_y(
    x: float,
    z: float,
    *,
    x0: float,
    x1: float,
    z0: float,
    z1: float,
    y0: float,
    y1: float,
    steps: int = 6,
    axis: str = "z",
) -> float | None:
    """軸に沿った段。範囲外は ``None``（下地と max 合成する）。"""
    x, z = float(x), float(z)
    if not (float(x0) <= x <= float(x1) and float(z0) <= z <= float(z1)):
        return None
    n = max(1, int(steps))
    if axis == "x":
        span = float(x1) - float(x0)
        t = 0.0 if span <= 1e-9 else (x - float(x0)) / span
    else:
        span = float(z1) - float(z0)
        t = 0.0 if span <= 1e-9 else (z - float(z0)) / span
    t = min(max(t, 0.0), 0.999999)
    step = math.floor(t * n)
    return float(y0) + (float(y1) - float(y0)) * (step + 1) / float(n)


def ramp_y(
    x: float,
    z: float,
    *,
    x0: float,
    x1: float,
    z0: float,
    z1: float,
    y0: float,
    y1: float,
    axis: str = "x",
) -> float | None:
    """連続な坂。範囲外は ``None``。"""
    x, z = float(x), float(z)
    if not (float(x0) <= x <= float(x1) and float(z0) <= z <= float(z1)):
        return None
    if axis == "z":
        span = float(z1) - float(z0)
        t = 0.0 if span <= 1e-9 else (z - float(z0)) / span
    else:
        span = float(x1) - float(x0)
        t = 0.0 if span <= 1e-9 else (x - float(x0)) / span
    t = min(max(t, 0.0), 1.0)
    return float(y0) + (float(y1) - float(y0)) * t


def compose_height(base, *layers: Callable[..., Optional[float]]):
    """下地と層（範囲外 ``None``）を max で重ねる。"""

    def fn(x: float, z: float) -> float:
        y = float(base(x, z))
        for layer in layers:
            extra = layer(x, z)
            if extra is not None:
                y = max(y, float(extra))
        return y

    return fn


def _plaza_stair(x: float, z: float) -> float | None:
    return stair_y(
        x, z, x0=-5.2, x1=-3.2, z0=1.5, z1=5.8, y0=0.42, y1=1.85, steps=6,
    )


def _plaza_ramp(x: float, z: float) -> float | None:
    return ramp_y(
        x, z, x0=2.5, x1=7.0, z0=-7.0, z1=-4.5, y0=0.35, y1=1.7, axis="x",
    )


def overworld_height(x: float, z: float) -> float:
    """島 + 広場の階段 + ゆるい坂。"""
    return compose_height(island_height, _plaza_stair, _plaza_ramp)(x, z)


def _smoothstep(t: float) -> float:
    t = max(0.0, min(1.0, float(t)))
    return t * t * (3.0 - 2.0 * t)


def _open_world_base(x: float, z: float) -> float:
    """大きい半島の下地。手前が草原、西が海、北が山。"""
    x = float(x)
    z = float(z)
    meadow = 1.22 + 0.30 * math.sin(x * 0.15) * math.cos(z * 0.13)
    meadow += 0.18 * math.sin(x * 0.33 + 1.1) * math.sin(z * 0.27)
    # 西の海岸線はスポーンから見て画面左（-X）に入る
    west = -9.5 * _smoothstep((-x - 15.0) / 13.0)
    sw = -4.8 * math.exp(-((x + 24.0) ** 2 + (z + 6.0) ** 2) / 160.0)
    south = -7.2 * _smoothstep((-z - 36.0) / 14.0)
    # なだらかな丘は草原のまま（2.2 未満）。山は遠く北。
    h1 = 0.85 * math.exp(-((x - 18.0) ** 2 + (z - 18.0) ** 2) / 95.0)
    h2 = 0.70 * math.exp(-((x + 5.0) ** 2 + (z - 22.0) ** 2) / 80.0)
    peak = 11.2 * math.exp(-((x - 8.0) ** 2 + (z - 52.0) ** 2) / 190.0)
    ridge = 9.4 * math.exp(-((x - 24.0) ** 2 + (z - 48.0) ** 2) / 220.0)
    far = 13.5 * math.exp(-((x - 1.0) ** 2 + (z - 66.0) ** 2) / 200.0)
    east = 0.90 * math.exp(-((x - 30.0) ** 2 + (z - 12.0) ** 2) / 110.0)
    y = meadow + west + sw + south + h1 + h2 + peak + ridge + far + east
    # スポーン草原を平らに（歩きやすく、最初のショットが草地）
    w = math.exp(-(x * x + (z + 7.0) ** 2) / 58.0)
    return y * (1.0 - 0.58 * w) + 1.18 * w


def _crest_stair(x: float, z: float) -> float | None:
    """峰へ続く段。ジャンプで上がれる。"""
    return stair_y(
        x, z,
        x0=5.0, x1=11.0, z0=26.0, z1=53.5,
        y0=1.85, y1=12.8, steps=16, axis="z",
    )


def open_world_height(x: float, z: float) -> float:
    """大きい半島。手前が草原、西〜南西が海、北が山脈。

    Relic / Overworld の島（半辺 24）より広い。``World3D(half=80)`` 向け。
    """
    return compose_height(_open_world_base, _crest_stair)(x, z)


def city_boxes(
    ix: int,
    iz: int,
    *,
    tile: float = TILE,
    fn=island_height,
    water_y: float = WATER_Y,
) -> list[tuple[float, float, float, float, float, float]]:
    """タイル 1 枚の箱街区。海・山・スポーン付近は空。OSM ではない。"""
    t = float(tile)
    cx = (int(ix) + 0.5) * t
    cz = (int(iz) + 0.5) * t
    if abs(int(ix)) <= 1 and abs(int(iz)) <= 1:
        return []
    if biome_at(cx, cz, water_y=water_y, fn=fn) != "grass":
        return []
    seed = abs(int(ix) * 17 + int(iz) * 31)
    if seed % 3 == 0:
        return []
    h = 1.8 + (seed % 5) * 0.45
    w = 2.2 + (seed % 3) * 0.3
    d = 2.0 + ((seed // 3) % 3) * 0.3
    bx = cx + ((seed % 5) - 2) * 0.35
    bz = cz + (((seed // 7) % 5) - 2) * 0.35
    gy = float(fn(bx, bz))
    return [(bx, gy, bz, w, h, d)]
