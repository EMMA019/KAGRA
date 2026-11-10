"""Moonlit Swarm's launcher icon (CC0-1.0, original, written by this script).

A night tile, a moon, and a small swarm of lights. No input images.

    python3 tools/gen_assets/moonlit_swarm_icon.py
    python3 tools/gen_assets/moonlit_swarm_icon.py --check
"""
from __future__ import annotations

import math
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "assets" / "icons" / "moonlit_swarm.png"
N = 192


def pixel(x: int, y: int) -> tuple[int, int, int]:
    u, v = (x + 0.5) / N * 2 - 1, (y + 0.5) / N * 2 - 1
    r = max(abs(u), abs(v))
    # Rounded night tile.
    bg_t = (v + 1) / 2
    col = [
        18 + 16 * (1 - bg_t),
        14 + 10 * (1 - bg_t),
        48 + 36 * (1 - bg_t),
    ]
    # Moon, upper right.
    mx, my = u - 0.28, v + 0.22
    md = math.hypot(mx, my)
    moon = math.exp(-((md - 0.0) / 0.34) ** 2) if md < 0.42 else 0.0
    bite = math.hypot(mx - 0.16, my + 0.06)
    if bite < 0.28:
        moon *= 0.15
    col[0] += 230 * moon
    col[1] += 220 * moon
    col[2] += 180 * moon
    # A thin gold ring under the moon.
    ring = math.exp(-((md - 0.50) / 0.03) ** 2)
    col[0] += 210 * ring
    col[1] += 160 * ring
    col[2] += 60 * ring
    # Swarm: a few pale lights curling toward the moon.
    for sx, sy, rad, tint in (
        (-0.42, 0.28, 0.07, (140, 255, 190)),
        (-0.18, 0.42, 0.055, (180, 230, 255)),
        (-0.55, 0.02, 0.05, (255, 170, 210)),
        (-0.28, -0.05, 0.045, (255, 220, 140)),
        (0.02, 0.18, 0.04, (160, 255, 230)),
    ):
        d = math.hypot(u - sx, v - sy)
        glow = math.exp(-((d / rad) ** 2))
        col[0] += tint[0] * glow * 0.85
        col[1] += tint[1] * glow * 0.85
        col[2] += tint[2] * glow * 0.85
    if r > 0.94:
        fade = max(0.0, 1.0 - (r - 0.94) / 0.06)
        col = [c * fade for c in col]
    return tuple(max(0, min(255, int(c))) for c in col)


def png() -> bytes:
    raw = bytearray()
    for y in range(N):
        raw.append(0)
        for x in range(N):
            raw += bytes(pixel(x, y))

    def chunk(tag: bytes, payload: bytes) -> bytes:
        return (
            struct.pack(">I", len(payload))
            + tag
            + payload
            + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def main(argv: list[str]) -> int:
    data = png()
    if "--check" in argv:
        return 0 if OUT.is_file() and OUT.read_bytes() == data else 1
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(data)
    print("wrote", OUT.relative_to(ROOT), len(data))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
