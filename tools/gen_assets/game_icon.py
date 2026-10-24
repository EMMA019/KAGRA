"""One placeholder launcher icon (CC0-1.0, written by this script).

A dark tile with a neon ring and a play triangle. Every game manifest points
at it until a game gets its own icon.

    python3 tools/gen_assets/game_icon.py          # write assets/icons/game_placeholder.png
    python3 tools/gen_assets/game_icon.py --check  # exit 1 on drift
"""
from __future__ import annotations

import math
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "assets" / "icons" / "game_placeholder.png"
N = 192


def pixel(x: int, y: int) -> tuple[int, int, int]:
    u, v = (x + 0.5) / N * 2 - 1, (y + 0.5) / N * 2 - 1
    # rounded square mask
    r = max(abs(u), abs(v))
    bg = (10 + int(14 * (1 - (v + 1) / 2)), 6, 24 + int(26 * (1 - (v + 1) / 2)))
    d = math.hypot(u, v)
    ring = math.exp(-((d - 0.72) / 0.045) ** 2)
    col = [bg[0] + 255 * ring * 0.9, bg[1] + 60 * ring, bg[2] + 200 * ring]
    # play triangle, pointing right
    a = (-0.22, -0.34)
    b = (-0.22, 0.34)
    c = (0.38, 0.0)

    def side(p, q):
        return (q[0] - p[0]) * (v - p[1]) - (q[1] - p[1]) * (u - p[0])

    s = (side(a, b), side(b, c), side(c, a))
    if all(t >= 0 for t in s) or all(t <= 0 for t in s):
        col = [120, 255, 235]
    if r > 0.98:
        col = [c_ * 0.0 for c_ in col]
    return tuple(max(0, min(255, int(c_))) for c_ in col)


def png() -> bytes:
    raw = bytearray()
    for y in range(N):
        raw.append(0)
        for x in range(N):
            raw += bytes(pixel(x, y))

    def chunk(tag: bytes, payload: bytes) -> bytes:
        return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)

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
