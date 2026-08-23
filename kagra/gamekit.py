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
