"""Equirect → cube faces. GPU 不要。``set_hdri`` の変換と同じ式。

PMREM はまだ無い。法線でキューブを直接サンプルする。
"""
from __future__ import annotations

import math
from typing import Sequence

FACE_NAMES = ("+x", "-x", "+y", "-y", "+z", "-z")


def face_dir(face: int, u: float, v: float) -> tuple[float, float, float]:
    """キューブ面の方向。``u,v`` は [-1, 1]。wgpu / GL 面順。"""
    if face == 0:
        return (1.0, -v, -u)
    if face == 1:
        return (-1.0, -v, u)
    if face == 2:
        return (u, 1.0, v)
    if face == 3:
        return (u, -1.0, -v)
    if face == 4:
        return (u, -v, 1.0)
    return (-u, -v, -1.0)


def dir_to_equirect_uv(dx: float, dy: float, dz: float) -> tuple[float, float]:
    leng = math.sqrt(dx * dx + dy * dy + dz * dz) or 1.0
    dx, dy, dz = dx / leng, dy / leng, dz / leng
    u = 0.5 + math.atan2(dx, -dz) / (2.0 * math.pi)
    v = 0.5 - math.asin(max(-1.0, min(1.0, dy))) / math.pi
    return u % 1.0, max(0.0, min(1.0, v))


def sample_equirect_rgb(
    pixels: Sequence[Sequence[float]],
    width: int,
    height: int,
    u: float,
    v: float,
) -> tuple[float, float, float]:
    """最近傍。``pixels`` は行優先 RGB 0..1。"""
    x = int((u % 1.0) * width) % width
    y = int(max(0.0, min(0.999999, v)) * height)
    return tuple(pixels[y * width + x][i] for i in range(3))  # type: ignore[return-value]


def equirect_to_face(
    pixels: Sequence[Sequence[float]],
    width: int,
    height: int,
    face: int,
    face_size: int = 8,
) -> list[tuple[float, float, float]]:
    out: list[tuple[float, float, float]] = []
    for y in range(face_size):
        for x in range(face_size):
            u = 2.0 * (x + 0.5) / face_size - 1.0
            v = 2.0 * (y + 0.5) / face_size - 1.0
            dx, dy, dz = face_dir(face, u, v)
            eu, ev = dir_to_equirect_uv(dx, dy, dz)
            out.append(sample_equirect_rgb(pixels, width, height, eu, ev))
    return out


def studio_equirect(width: int = 32, height: int = 16) -> list[tuple[float, float, float]]:
    """上は冷たい空、下は暖かい地面。``set_hdri('studio')`` と同じ。"""
    pix: list[tuple[float, float, float]] = []
    for y in range(height):
        t = y / max(1, height - 1)
        for x in range(width):
            az = x / max(1, width - 1)
            sky = (0.35 + 0.25 * az, 0.45 + 0.15 * az, 0.70)
            ground = (0.55, 0.38, 0.22)
            s = 1.0 / (1.0 + math.exp((t - 0.55) * 12.0))
            pix.append((
                sky[0] * s + ground[0] * (1.0 - s),
                sky[1] * s + ground[1] * (1.0 - s),
                sky[2] * s + ground[2] * (1.0 - s),
            ))
    return pix


def pbr_enabled(metallic: float, roughness: float) -> bool:
    """金属/粗さが既定（0 / 1）なら旧 Lambert のまま。"""
    return float(metallic) > 0.001 or float(roughness) < 0.999
