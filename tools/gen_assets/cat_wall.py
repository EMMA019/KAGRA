"""Neon Cat's concrete wall texture (CC0-1.0, drawn here, no input images).

One panel of poured concrete with form joints, tie holes, rain-drip stains and
a damp foot. It is 8 m wide and 6.9 m tall at 40 px per metre, and it tiles
left to right (every feature wraps), so neighbouring panels meet without a seam.
The game stands it on the tunnel walls (`atmosphere.wall`).

    python3 tools/gen_assets/cat_wall.py          # write concrete_wall.png and the half-size concrete_wall_s.png
    python3 tools/gen_assets/cat_wall.py --check  # exit 1 on drift
"""
from __future__ import annotations

import math
import random
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "assets" / "textures" / "neon_cat" / "concrete_wall.png"
PPM = 40  # pixels per metre
PANEL_W, PANEL_H = 8.0, 6.9  # metres; matches `atmosphere.wall.panel_len` and the wall height
W, H = int(PANEL_W * PPM), int(PANEL_H * PPM)
JOINT_X = 2.0  # form joints every 2 m across
JOINT_Y = 1.15  # and every 1.15 m up (6 courses)


def _lattice(rng: random.Random, nx: int, ny: int) -> list[list[float]]:
    return [[rng.random() for _ in range(nx)] for _ in range(ny)]


def _smooth(t: float) -> float:
    return t * t * (3 - 2 * t)


def _noise(grid: list[list[float]], x: float, y: float) -> float:
    """Value noise on a lattice that wraps in x."""
    ny, nx = len(grid), len(grid[0])
    xi, yi = int(math.floor(x)), int(math.floor(y))
    fx, fy = _smooth(x - xi), _smooth(y - yi)
    x0, x1 = xi % nx, (xi + 1) % nx
    y0, y1 = min(yi, ny - 1), min(yi + 1, ny - 1)
    a = grid[y0][x0] * (1 - fx) + grid[y0][x1] * fx
    b = grid[y1][x0] * (1 - fx) + grid[y1][x1] * fx
    return a * (1 - fy) + b * fy


def render() -> list[list[tuple[int, int, int]]]:
    rng = random.Random(20261024)
    octaves = []
    for k, cells in enumerate((3, 6, 12, 24)):
        # cells across the panel (wraps) and a matching count up it
        ny = max(2, int(cells * PANEL_H / PANEL_W) + 2)
        octaves.append((cells, _lattice(rng, cells, ny), 0.5**k))
    grain = [[rng.random() for _ in range(W)] for _ in range(H)]
    # Drip stains: a column, a width, how far down it runs, how dark.
    drips = []
    for _ in range(26):
        drips.append((rng.uniform(0, W), rng.uniform(1.0, 3.2), rng.uniform(0.18, 0.7) * H, rng.uniform(0.35, 1.0)))
    # Chips and pits.
    pits = [(rng.uniform(0, W), rng.uniform(0, H), rng.uniform(0.8, 2.2), rng.random() < 0.5) for _ in range(260)]
    px = [[(0, 0, 0)] * W for _ in range(H)]
    jx, jy = JOINT_X * PPM, JOINT_Y * PPM
    for y in range(H):
        v = y / H
        for x in range(W):
            n = 0.0
            tot = 0.0
            for cells, grid, amp in octaves:
                n += amp * _noise(grid, x / W * cells, y / H * cells * PANEL_H / PANEL_W)
                tot += amp
            n /= tot
            g = grain[y][x]
            lum = 0.52 + (n - 0.5) * 0.55 + (g - 0.5) * 0.16
            # Rain-washed: a little lighter at the top, damp and dark at the foot.
            lum += 0.10 * (1 - v) - 0.22 * max(0.0, (v - 0.80) / 0.20) ** 1.5
            px[y][x] = [lum, lum, lum]
    for cx, w, length, dark in drips:
        for y in range(int(length)):
            fall = 1.0 - y / length
            for dx in range(-int(w) - 2, int(w) + 3):
                x = int(cx + dx) % W
                a = max(0.0, 1 - abs(dx) / (w + 1.5)) * fall * dark * 0.34
                p = px[y][x]
                p[0] -= a
                p[1] -= a
                p[2] -= a * 0.9
    for cx, cy, r, light in pits:
        for dy in range(-3, 4):
            for dx in range(-3, 4):
                d = math.hypot(dx, dy)
                if d > r:
                    continue
                x, y = int(cx + dx) % W, int(cy + dy)
                if 0 <= y < H:
                    k = (1 - d / r) * (0.10 if light else -0.16)
                    for c in range(3):
                        px[y][x][c] += k
    # Form joints: a dark groove with a pale lip on its lower edge.
    for y in range(H):
        for x in range(W):
            on_x = x % jx
            on_y = y % jy
            groove = 0.0
            if on_x < 1.5 or on_x > jx - 1.0:
                groove = max(groove, 0.30 if on_x < 1.0 or on_x > jx - 0.5 else 0.15)
            if on_y < 1.5:
                groove = max(groove, 0.34 if on_y < 1.0 else 0.16)
            if on_y > jy - 1.0 and groove == 0.0:
                px[y][x] = [c + 0.05 for c in px[y][x]]
            if groove:
                px[y][x] = [c - groove for c in px[y][x]]
    # Tie holes: four to a panel cell, set in from the joints.
    for cy in range(0, H, int(jy)):
        for cx in range(0, W, int(jx)):
            for ox, oy in ((0.25, 0.25), (1.75, 0.25), (0.25, 0.9), (1.75, 0.9)):
                hx, hy = cx + ox * PPM, cy + oy * PPM
                for dy in range(-3, 4):
                    for dx in range(-3, 4):
                        d = math.hypot(dx, dy)
                        if d <= 2.6:
                            x, y = int(hx + dx) % W, int(hy + dy)
                            if 0 <= y < H:
                                k = -0.30 * (1 - d / 3.2) + (0.06 if dx + dy < -1 else 0.0)
                                for c in range(3):
                                    px[y][x][c] += k
    out = []
    for row in px:
        line = []
        for r, g, b in row:
            r, g, b = max(0.0, min(1.0, r)), max(0.0, min(1.0, g)), max(0.0, min(1.0, b))
            # Cool grey with a little violet so it sits in the neon dark.
            line.append((int(46 + 98 * r * 0.95), int(48 + 100 * g * 0.95), int(62 + 112 * b)))
        out.append(line)
    return out


def half(img: list[list[tuple[int, int, int]]]) -> list[list[tuple[int, int, int]]]:
    """2 x 2 box average: the phone tier (`concrete_wall_s.png`)."""
    out = []
    for y in range(0, len(img), 2):
        row = []
        for x in range(0, len(img[0]), 2):
            px = [img[y][x], img[y][x + 1], img[y + 1][x], img[y + 1][x + 1]]
            row.append(tuple(sum(p[c] for p in px) // 4 for c in range(3)))
        out.append(row)
    return out


def png(img: list[list[tuple[int, int, int]]]) -> bytes:
    h, w = len(img), len(img[0])
    raw = bytearray()
    for row in img:
        raw.append(0)
        for r, g, b in row:
            raw += bytes((r, g, b))

    def chunk(tag: bytes, payload: bytes) -> bytes:
        return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def build() -> dict[Path, bytes]:
    img = render()
    return {OUT: png(img), OUT.with_name("concrete_wall_s.png"): png(half(img))}


def main(argv: list[str]) -> int:
    files = build()
    if "--check" in argv:
        return 0 if all(p.is_file() and p.read_bytes() == d for p, d in files.items()) else 1
    OUT.parent.mkdir(parents=True, exist_ok=True)
    for path, data in files.items():
        path.write_bytes(data)
        print("wrote", path.relative_to(ROOT), len(data))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
