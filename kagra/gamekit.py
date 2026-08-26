"""Indie-game helpers that Orb Rush used to hand-roll.

All of this is GPU-free: PNG / WAV bytes, 3D mesh builders, JSON persist.
``kagra.texture_from_fn`` / ``kagra.draw_billboard`` / ``kagra.se`` wrap
the pieces that need the renderer.
"""
from __future__ import annotations

import json
import math
import os
import struct
import tempfile
import wave
from pathlib import Path
from typing import Callable, Iterable, Optional


PixelFn = Callable[[int, int], tuple[int, ...]]


def data_dir(directory: str | Path | None = None) -> Path:
    """JSON セーブの置き場。``KAGRA_DATA`` があればそれを優先。"""
    if directory is not None:
        root = Path(directory)
    elif env := os.environ.get("KAGRA_DATA"):
        root = Path(env)
    else:
        root = Path.home() / ".kagra" / "saves"
    root.mkdir(parents=True, exist_ok=True)
    return root


def save_json(name: str, data: dict, *, directory: str | Path | None = None) -> Path:
    """小さな dict を JSON で残す。拡張子は付けなくてよい。

    アセット用 ``kagra.load_data`` とは別（ゲーム進行の永続化）。
    """
    slug = Path(name).stem or "save"
    path = data_dir(directory) / f"{slug}.json"
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
    return path


def load_json(name: str, default=None, *, directory: str | Path | None = None):
    """``save_json`` の対。無ければ ``default``。"""
    slug = Path(name).stem or "save"
    path = data_dir(directory) / f"{slug}.json"
    if not path.is_file():
        return default
    try:
        obj = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return default
    return obj


def rgba_png(width: int, height: int, pixel_fn: PixelFn) -> bytes:
    """``pixel_fn(x, y) -> (r,g,b) or (r,g,b,a)`` から PNG バイトを作る。"""
    from kagra.look import encode_png_rgba

    if width < 1 or height < 1:
        raise ValueError("png size must be >= 1")
    pix = bytearray()
    for y in range(height):
        for x in range(width):
            c = pixel_fn(x, y)
            if len(c) == 3:
                pix.extend((c[0], c[1], c[2], 255))
            elif len(c) == 4:
                pix.extend(c)
            else:
                raise ValueError("pixel_fn must return 3 or 4 ints")
    return encode_png_rgba(width, height, bytes(pix))


def write_png(
    width: int,
    height: int,
    pixel_fn: PixelFn,
    *,
    name: str | None = None,
) -> Path:
    """手続きテクスチャを tempfile の PNG にしてパスを返す。"""
    tag = name or f"tex_{width}x{height}"
    path = Path(tempfile.gettempdir()) / f"kagra_{tag}.png"
    path.write_bytes(rgba_png(width, height, pixel_fn))
    return path


def write_tone(
    name: str,
    freqs: Iterable[float],
    duration: float = 0.12,
    volume: float = 0.35,
    decay: bool = True,
) -> Path:
    """単純な合成トーンを WAV に書いてパスを返す。外部アセット不要。"""
    hz = [float(f) for f in freqs]
    if not hz:
        raise ValueError("freqs must not be empty")
    duration = max(0.01, float(duration))
    volume = max(0.0, min(1.0, float(volume)))
    path = Path(tempfile.gettempdir()) / f"kagra_tone_{name}.wav"
    rate = 22050
    n = int(rate * duration)
    frames = bytearray()
    nfreq = len(hz)
    for i in range(n):
        t = i / rate
        env = ((1.0 - t / duration) if decay else 1.0)
        env = max(0.0, env) ** 1.4
        s = sum(math.sin(2 * math.pi * f * t) for f in hz) / nfreq
        sample = max(-1.0, min(1.0, s * volume * env))
        frames += struct.pack("<h", int(sample * 32767))
    with wave.open(str(path), "w") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(bytes(frames))
    return path


def _yaw_of(camera=None, yaw: Optional[float] = None) -> float:
    if yaw is not None:
        return float(yaw)
    return float(getattr(camera, "orbit_th", 0.0))


def billboard_mesh(
    x: float,
    y: float,
    z: float,
    size: float,
    camera=None,
    *,
    yaw: Optional[float] = None,
) -> tuple[list, list]:
    """カメラ向きの四角。``verts`` は ``[x,y,z,nx,ny,nz,u,v]``。"""
    theta = _yaw_of(camera, yaw)
    rx, rz = math.cos(theta), -math.sin(theta)
    s = float(size)
    corners = (
        (-rx * s, -s, -rz * s, 0.0, 0.0),
        (rx * s, -s, rz * s, 1.0, 0.0),
        (rx * s, s, rz * s, 1.0, 1.0),
        (-rx * s, s, -rz * s, 0.0, 1.0),
    )
    nx, ny, nz = math.sin(theta), 0.2, math.cos(theta)
    nl = math.hypot(nx, ny, nz) or 1.0
    nx, ny, nz = nx / nl, ny / nl, nz / nl
    verts = [[x + dx, y + dy, z + dz, nx, ny, nz, u, v] for dx, dy, dz, u, v in corners]
    return verts, [0, 1, 2, 0, 2, 3]


def disk_mesh(
    cx: float,
    cy: float,
    cz: float,
    radius: float,
    segs: int = 48,
) -> tuple[list, list]:
    """Y 上向きの円盤。床用。"""
    segs = max(3, int(segs))
    verts = [[cx, cy, cz, 0.0, 1.0, 0.0, 0.5, 0.5]]
    idx: list[int] = []
    for i in range(segs):
        a0 = i / segs * math.tau
        a1 = (i + 1) / segs * math.tau
        x0, z0 = cx + math.cos(a0) * radius, cz + math.sin(a0) * radius
        x1, z1 = cx + math.cos(a1) * radius, cz + math.sin(a1) * radius
        base = len(verts)
        verts.append([x0, cy, z0, 0.0, 1.0, 0.0, 0.5 + math.cos(a0) * 0.5, 0.5 + math.sin(a0) * 0.5])
        verts.append([x1, cy, z1, 0.0, 1.0, 0.0, 0.5 + math.cos(a1) * 0.5, 0.5 + math.sin(a1) * 0.5])
        idx += [0, base, base + 1]
    return verts, idx


def quad_y_mesh(cx: float, cy: float, cz: float, size: float) -> tuple[list, list]:
    """Y 上向きの正方形（半辺 ``size``）。"""
    s = float(size)
    verts = [
        [cx - s, cy, cz - s, 0.0, 1.0, 0.0, 0.0, 0.0],
        [cx + s, cy, cz - s, 0.0, 1.0, 0.0, 1.0, 0.0],
        [cx + s, cy, cz + s, 0.0, 1.0, 0.0, 1.0, 1.0],
        [cx - s, cy, cz + s, 0.0, 1.0, 0.0, 0.0, 1.0],
    ]
    return verts, [0, 1, 2, 0, 2, 3]


def _smooth01(t: float) -> float:
    t = max(0.0, min(1.0, float(t)))
    return t * t * (3.0 - 2.0 * t)


def _pingpong01(t: float) -> float:
    """Fold into 0..1. Sampler is ClampToEdge, so UVs must not wrap past 1."""
    t = float(t)
    n = math.floor(t)
    f = t - n
    if int(n) % 2:
        return 1.0 - f
    return f


def heightfield_mesh(
    fn,
    half: float = 16.0,
    cells: int = 32,
    *,
    origin_x: float = 0.0,
    origin_z: float = 0.0,
    uv_half: float | None = None,
    uv_period: float | None = None,
    uv_blend: float = 0.0,
    uv_pad: float = 0.0,
) -> tuple[list, list]:
    """``(x, z) → y`` の格子メッシュ。``verts`` は ``[x,y,z,nx,ny,nz,u,v]``。

    中心は ``(origin_x, origin_z)``。UV は ``uv_half``（省略時は ``half``）の
    ワールド範囲に合わせるので、タイルしても 1 枚の地形テクスチャが使える。
    法線はタイルの外まで ``fn`` を取るので、隣接チャンクのライティングが
    片側差分のナイフ線にならない。``uv_period`` は ClampToEdge 向けの
    ping-pong 繰り返し（ワールド連続。タイル局所の 0..1 ではない）。
    ``uv_pad`` は JPEG の土縁を避ける inset。``uv_blend`` はタイル縁の
    わずかなゆらぎ（Crest Isle は 0。継ぎ目は period で連続）。
    """
    cells = max(2, int(cells))
    half = float(half)
    ox, oz = float(origin_x), float(origin_z)
    uh = float(uv_half) if uv_half is not None else half
    uh = max(uh, 1e-6)
    period = None if uv_period is None or float(uv_period) <= 1e-6 else float(uv_period)
    blend = max(0.0, float(uv_blend))
    pad = max(0.0, min(0.45, float(uv_pad)))
    step = (2.0 * half) / float(cells)
    n = cells + 1
    ys = [[0.0] * n for _ in range(n)]
    for j in range(n):
        for i in range(n):
            x = ox - half + i * step
            z = oz - half + j * step
            ys[j][i] = float(fn(x, z))
    x0, x1 = ox - half, ox + half
    z0, z1 = oz - half, oz + half

    def _uv_at(x: float, z: float) -> tuple[float, float]:
        if period is not None:
            u = _pingpong01(x / period)
            v = _pingpong01(z / period)
        else:
            u = (x / uh + 1.0) * 0.5
            v = (z / uh + 1.0) * 0.5
        if pad > 0:
            u = pad + u * (1.0 - 2.0 * pad)
            v = pad + v * (1.0 - 2.0 * pad)
        if blend > 1e-6:
            edge = min(x - x0, x1 - x, z - z0, z1 - z)
            w = _smooth01(edge / blend)
            # World-continuous wobble: both tiles share x or z on the join.
            u += 0.018 * math.sin(z * 0.41) * (1.0 - w)
            v += 0.018 * math.sin(x * 0.37) * (1.0 - w)
            if pad > 0:
                u = max(pad, min(1.0 - pad, u))
                v = max(pad, min(1.0 - pad, v))
        return u, v

    verts: list[list[float]] = []
    two = 2.0 * step
    for j in range(n):
        for i in range(n):
            x = ox - half + i * step
            z = oz - half + j * step
            y = ys[j][i]
            # Sample *outside* the tile so adjacent chunks share the same normal.
            hx = float(fn(x + step, z)) - float(fn(x - step, z))
            hz = float(fn(x, z + step)) - float(fn(x, z - step))
            nx = -hx / two
            nz = -hz / two
            ny = 1.0
            leng = math.sqrt(nx * nx + ny * ny + nz * nz) or 1.0
            u, v = _uv_at(x, z)
            verts.append([x, y, z, nx / leng, ny / leng, nz / leng, u, v])
    indices: list[int] = []
    for j in range(cells):
        for i in range(cells):
            a = j * n + i
            b = a + 1
            c = a + n
            d = c + 1
            indices += [a, c, b, b, c, d]
    return verts, indices


def ramp_mesh(
    x0: float,
    x1: float,
    z0: float,
    z1: float,
    y0: float,
    y1: float,
) -> tuple[list, list]:
    """+X 方向に上がる坂の 2 三角形。``verts`` は ``[x,y,z,nx,ny,nz,u,v]``。"""
    dx = float(x1) - float(x0)
    dy = float(y1) - float(y0)
    nx, ny, nz = -dy, dx, 0.0
    leng = math.sqrt(nx * nx + ny * ny + nz * nz) or 1.0
    nx, ny, nz = nx / leng, ny / leng, nz / leng
    verts = [
        [float(x0), float(y0), float(z0), nx, ny, nz, 0.0, 0.0],
        [float(x1), float(y1), float(z0), nx, ny, nz, 1.0, 0.0],
        [float(x1), float(y1), float(z1), nx, ny, nz, 1.0, 1.0],
        [float(x0), float(y0), float(z1), nx, ny, nz, 0.0, 1.0],
    ]
    return verts, [0, 1, 2, 0, 2, 3]


def heightfield_tile(
    fn,
    origin_x: float,
    origin_z: float,
    tile: float = 10.0,
    cells: int = 8,
    *,
    uv_half: float | None = None,
    uv_period: float | None = None,
    uv_blend: float = 0.0,
    uv_pad: float = 0.0,
) -> tuple[list, list]:
    """南西角 ``(origin_x, origin_z)``、辺 ``tile`` の高さ場タイル。"""
    t = float(tile)
    half = t * 0.5
    return heightfield_mesh(
        fn,
        half,
        cells,
        origin_x=float(origin_x) + half,
        origin_z=float(origin_z) + half,
        uv_half=uv_half,
        uv_period=uv_period,
        uv_blend=uv_blend,
        uv_pad=uv_pad,
    )


def box_mesh(
    cx: float,
    cy: float,
    cz: float,
    w: float,
    h: float,
    d: float,
) -> tuple[list, list]:
    """軸平行の箱。``cy`` は中心。``verts`` は ``[x,y,z,nx,ny,nz,u,v]``。"""
    hx, hy, hz = float(w) * 0.5, float(h) * 0.5, float(d) * 0.5
    faces = (
        ((0.0, 1.0, 0.0), (
            (-hx, hy, -hz, 0.0, 0.0), (hx, hy, -hz, 1.0, 0.0),
            (hx, hy, hz, 1.0, 1.0), (-hx, hy, hz, 0.0, 1.0),
        )),
        ((0.0, -1.0, 0.0), (
            (-hx, -hy, hz, 0.0, 0.0), (hx, -hy, hz, 1.0, 0.0),
            (hx, -hy, -hz, 1.0, 1.0), (-hx, -hy, -hz, 0.0, 1.0),
        )),
        ((1.0, 0.0, 0.0), (
            (hx, -hy, -hz, 0.0, 0.0), (hx, -hy, hz, 1.0, 0.0),
            (hx, hy, hz, 1.0, 1.0), (hx, hy, -hz, 0.0, 1.0),
        )),
        ((-1.0, 0.0, 0.0), (
            (-hx, -hy, hz, 0.0, 0.0), (-hx, -hy, -hz, 1.0, 0.0),
            (-hx, hy, -hz, 1.0, 1.0), (-hx, hy, hz, 0.0, 1.0),
        )),
        ((0.0, 0.0, 1.0), (
            (-hx, -hy, hz, 0.0, 0.0), (hx, -hy, hz, 1.0, 0.0),
            (hx, hy, hz, 1.0, 1.0), (-hx, hy, hz, 0.0, 1.0),
        )),
        ((0.0, 0.0, -1.0), (
            (hx, -hy, -hz, 0.0, 0.0), (-hx, -hy, -hz, 1.0, 0.0),
            (-hx, hy, -hz, 1.0, 1.0), (hx, hy, -hz, 0.0, 1.0),
        )),
    )
    verts: list[list[float]] = []
    idx: list[int] = []
    for (nx, ny, nz), corners in faces:
        base = len(verts)
        for dx, dy, dz, u, v in corners:
            verts.append([cx + dx, cy + dy, cz + dz, nx, ny, nz, u, v])
        idx += [base, base + 1, base + 2, base, base + 2, base + 3]
    return verts, idx


def sphere_mesh(
    cx: float = 0.0,
    cy: float = 0.0,
    cz: float = 0.0,
    radius: float = 0.5,
    segs: int = 16,
) -> tuple[list, list]:
    """UV 球。``verts`` は ``[x,y,z,nx,ny,nz,u,v]``。既定は直径 1。"""
    segs = max(6, int(segs))
    rings = max(4, segs // 2)
    verts: list[list[float]] = []
    idx: list[int] = []
    r = float(radius)
    for i in range(rings + 1):
        v = i / rings
        phi = v * math.pi
        sp, cp = math.sin(phi), math.cos(phi)
        for j in range(segs + 1):
            u = j / segs
            th = u * math.tau
            st, ct = math.sin(th), math.cos(th)
            nx, ny, nz = st * sp, cp, ct * sp
            verts.append([
                cx + nx * r, cy + ny * r, cz + nz * r,
                nx, ny, nz, u, 1.0 - v,
            ])
    for i in range(rings):
        for j in range(segs):
            a = i * (segs + 1) + j
            b = a + segs + 1
            idx += [a, b, a + 1, a + 1, b, b + 1]
    return verts, idx


def cylinder_mesh(
    cx: float = 0.0,
    cy: float = 0.0,
    cz: float = 0.0,
    radius: float = 0.5,
    height: float = 1.0,
    segs: int = 16,
) -> tuple[list, list]:
    """Y 軸円柱。中心 ``(cx,cy,cz)``。既定は直径 1・高さ 1。"""
    segs = max(6, int(segs))
    r = float(radius)
    hy = float(height) * 0.5
    verts: list[list[float]] = []
    idx: list[int] = []
    for i in range(segs + 1):
        u = i / segs
        th = u * math.tau
        st, ct = math.sin(th), math.cos(th)
        nx, nz = st, ct
        verts.append([cx + nx * r, cy - hy, cz + nz * r, nx, 0.0, nz, u, 0.0])
        verts.append([cx + nx * r, cy + hy, cz + nz * r, nx, 0.0, nz, u, 1.0])
    for i in range(segs):
        a = i * 2
        idx += [a, a + 2, a + 1, a + 1, a + 2, a + 3]
    top = len(verts)
    verts.append([cx, cy + hy, cz, 0.0, 1.0, 0.0, 0.5, 0.5])
    bot = len(verts)
    verts.append([cx, cy - hy, cz, 0.0, -1.0, 0.0, 0.5, 0.5])
    for i in range(segs):
        th0 = i / segs * math.tau
        th1 = (i + 1) / segs * math.tau
        x0, z0 = cx + math.sin(th0) * r, cz + math.cos(th0) * r
        x1, z1 = cx + math.sin(th1) * r, cz + math.cos(th1) * r
        t0 = len(verts)
        verts.append([x0, cy + hy, z0, 0.0, 1.0, 0.0, 0.5 + math.sin(th0) * 0.5, 0.5 + math.cos(th0) * 0.5])
        verts.append([x1, cy + hy, z1, 0.0, 1.0, 0.0, 0.5 + math.sin(th1) * 0.5, 0.5 + math.cos(th1) * 0.5])
        idx += [top, t0, t0 + 1]
        b0 = len(verts)
        verts.append([x0, cy - hy, z0, 0.0, -1.0, 0.0, 0.5 + math.sin(th0) * 0.5, 0.5 + math.cos(th0) * 0.5])
        verts.append([x1, cy - hy, z1, 0.0, -1.0, 0.0, 0.5 + math.sin(th1) * 0.5, 0.5 + math.cos(th1) * 0.5])
        idx += [bot, b0 + 1, b0]
    return verts, idx
