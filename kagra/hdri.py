"""Equirect → cube faces. GPU 不要。``set_hdri`` の変換と同じ式。

拡散は小さな irradiance キューブ（PMREM-lite）。スペキュラは鋭いキューブ。
"""
from __future__ import annotations

import math
from typing import Sequence

IRRADIANCE_FACE_SIZE = 8
IRRADIANCE_SAMPLES = 16

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


def _norm3(x: float, y: float, z: float) -> tuple[float, float, float]:
    leng = math.sqrt(x * x + y * y + z * z) or 1.0
    return x / leng, y / leng, z / leng


def _cross(a: tuple[float, float, float], b: tuple[float, float, float]) -> tuple[float, float, float]:
    return (
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )


def _onb(n: tuple[float, float, float]) -> tuple[tuple[float, float, float], tuple[float, float, float], tuple[float, float, float]]:
    if abs(n[1]) < 0.999:
        t = _norm3(*_cross((0.0, 1.0, 0.0), n))
    else:
        t = _norm3(*_cross((1.0, 0.0, 0.0), n))
    return t, _cross(n, t), n


def _radical_inverse_vdc(bits: int) -> float:
    inv = 0.0
    base = 0.5
    i = int(bits)
    while i > 0:
        if i & 1:
            inv += base
        i >>= 1
        base *= 0.5
    return inv


def cosine_hemisphere(
    n: tuple[float, float, float],
    index: int,
    samples: int,
) -> tuple[float, float, float]:
    """余弦重みの半球方向。``index`` は 0..samples-1。"""
    samples = max(1, int(samples))
    u = (float(index) + 0.5) / float(samples)
    v = _radical_inverse_vdc(int(index))
    r = math.sqrt(max(0.0, u))
    phi = 2.0 * math.pi * v
    lx = r * math.cos(phi)
    ly = r * math.sin(phi)
    lz = math.sqrt(max(0.0, 1.0 - u))
    t, b, nn = _onb(_norm3(*n))
    return (
        t[0] * lx + b[0] * ly + nn[0] * lz,
        t[1] * lx + b[1] * ly + nn[1] * lz,
        t[2] * lx + b[2] * ly + nn[2] * lz,
    )


def irradiance_face(
    pixels: Sequence[Sequence[float]],
    width: int,
    height: int,
    face: int,
    face_size: int = IRRADIANCE_FACE_SIZE,
    samples: int = IRRADIANCE_SAMPLES,
) -> list[tuple[float, float, float]]:
    """1 面の拡散 irradiance（余弦半球の平均）。"""
    face_size = max(1, int(face_size))
    samples = max(1, int(samples))
    out: list[tuple[float, float, float]] = []
    for y in range(face_size):
        for x in range(face_size):
            u = 2.0 * (x + 0.5) / face_size - 1.0
            v = 2.0 * (y + 0.5) / face_size - 1.0
            n = _norm3(*face_dir(face, u, v))
            acc = [0.0, 0.0, 0.0]
            for s in range(samples):
                dx, dy, dz = cosine_hemisphere(n, s, samples)
                eu, ev = dir_to_equirect_uv(dx, dy, dz)
                rgb = sample_equirect_rgb(pixels, width, height, eu, ev)
                acc[0] += rgb[0]
                acc[1] += rgb[1]
                acc[2] += rgb[2]
            inv = 1.0 / float(samples)
            out.append((acc[0] * inv, acc[1] * inv, acc[2] * inv))
    return out


def irradiance_cube(
    pixels: Sequence[Sequence[float]],
    width: int,
    height: int,
    face_size: int = IRRADIANCE_FACE_SIZE,
    samples: int = IRRADIANCE_SAMPLES,
) -> list[list[tuple[float, float, float]]]:
    """6 面。法線でサンプルする拡散キューブ。"""
    return [
        irradiance_face(pixels, width, height, face, face_size, samples)
        for face in range(6)
    ]


def _smoothstep(edge0: float, edge1: float, x: float) -> float:
    if abs(edge1 - edge0) < 1e-8:
        return 1.0 if x >= edge0 else 0.0
    t = max(0.0, min(1.0, (x - edge0) / (edge1 - edge0)))
    return t * t * (3.0 - 2.0 * t)


def spot_cone_params(angle: float, penumbra: float = 0.25) -> tuple[float, float]:
    """``(cos_outer, cos_inner)``。``angle`` は外角（ラジアン）。"""
    angle = max(0.02, min(math.pi - 0.02, float(angle)))
    penumbra = max(0.0, min(0.99, float(penumbra)))
    inner_angle = angle * (1.0 - penumbra)
    cos_outer = math.cos(angle)
    cos_inner = math.cos(inner_angle)
    if cos_inner <= cos_outer:
        cos_inner = min(1.0, cos_outer + 1e-4)
    return cos_outer, cos_inner


def spot_cone_factor(
    from_light: tuple[float, float, float],
    axis: tuple[float, float, float],
    cos_outer: float,
    cos_inner: float,
) -> float:
    """光源から面へ向かう方向とスポット軸のコーン（0..1）。"""
    a = _norm3(*from_light)
    b = _norm3(*axis)
    c = a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    return _smoothstep(float(cos_outer), float(cos_inner), c)
