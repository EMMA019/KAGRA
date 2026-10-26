#!/usr/bin/env python3
"""Moonlit Swarm's arena floor: a seamless, hand-drawn-looking paver texture.

Original work for this project (CC0, see assets/LEDGER.json `project-survivor-textures`).
Deterministic: the same seed writes the same bytes. 512x512 RGBA, tiles in both directions.

Usage: sv_floor.py [OUT.png]
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image

N = 512
CELLS = 8  # pavers per tile side


def main(out):
    rng = np.random.default_rng(1907)
    cell = N // CELLS
    img = np.zeros((N, N, 3), np.float32)
    base = np.array([40.0, 54.0, 80.0])  # moonlit slate blue
    moss = np.array([36.0, 62.0, 72.0])
    yy, xx = np.mgrid[0:N, 0:N]
    # Brick-bond pavers: every other row is shifted half a paver.
    for r in range(CELLS):
        shift = (cell // 2) if r % 2 else 0
        for c in range(CELLS):
            x0 = (c * cell + shift) % N
            y0 = r * cell
            tone = rng.uniform(0.93, 1.07)
            mix = rng.uniform(0.0, 0.3) if rng.random() < 0.35 else 0.0
            col = (base * (1 - mix) + moss * mix) * tone
            xs = (np.arange(x0, x0 + cell) % N)
            ys = np.arange(y0, y0 + cell)
            # Bevel: lighter top-left edge, darker bottom-right, grout round the rim.
            u = np.linspace(0, 1, cell)[None, :].repeat(cell, 0)
            v = np.linspace(0, 1, cell)[:, None].repeat(cell, 1)
            edge = np.minimum(np.minimum(u, 1 - u), np.minimum(v, 1 - v))
            grout = np.clip(edge / 0.035, 0, 1)
            lift = 1.0 + 0.22 * np.clip(1 - np.minimum(u, v) / 0.07, 0, 1) - 0.25 * np.clip(1 - np.minimum(1 - u, 1 - v) / 0.07, 0, 1)
            tile = col[None, None, :] * (lift * (0.45 + 0.55 * grout))[:, :, None]
            img[np.ix_(ys, xs)] = tile
    # Fine grain, periodic by construction (noise tiled from a smaller block).
    g = rng.normal(0, 1, (N // 4, N // 4)).astype(np.float32)
    g = np.kron(g, np.ones((4, 4), np.float32))
    img *= (1 + 0.02 * g)[:, :, None]
    # A few pale flecks (moon-dust / tiny flowers), wrapped at the borders.
    for _ in range(36):
        x, y = rng.integers(0, N, 2)
        s = rng.integers(2, 4)
        tint = np.array([200.0, 215.0, 255.0]) if rng.random() < 0.7 else np.array([255.0, 200.0, 140.0])
        for dy in range(s):
            for dx in range(s):
                img[(y + dy) % N, (x + dx) % N] = img[(y + dy) % N, (x + dx) % N] * 0.7 + tint * 0.3
    rgba = np.dstack([np.clip(img, 0, 255), np.full((N, N), 255.0)]).astype(np.uint8)
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(rgba, "RGBA").save(out, optimize=True)
    print(out, Path(out).stat().st_size)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "assets/textures/survivor_arena/floor.png")
