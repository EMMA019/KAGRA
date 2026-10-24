"""Neon Cat store art: cover images drawn by this script (CC0-1.0, no outside images).

A night-city runner scene: sky, a banded sun, a skyline, a neon floor grid, a cat
with a cyan rim light, gold coins, and (in the titled versions) the NEON CAT title
in a stroke font made here. Every pixel is computed from the formulas below with
the standard library only, so the same command gives the same bytes anywhere and
CI needs nothing installed.

    python3 tools/gen_assets/cover_art.py                    # write docs/media/neon_cat/*.png
    python3 tools/gen_assets/cover_art.py --only square_512x512
    python3 tools/gen_assets/cover_art.py --list

The text-free versions are for portals that ban text on thumbnails (Poki).
The cat is a flat silhouette of ellipses and triangles, not Kenney's cat model.
"""
from __future__ import annotations

import argparse
import math
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "docs" / "media" / "neon_cat"

# name -> (width, height, titled)
IMAGES: dict[str, tuple[int, int, bool]] = {
    "landscape_1920x1080": (1920, 1080, True),
    "landscape_1280x720": (1280, 720, True),
    "portrait_800x1200": (800, 1200, True),
    "square_800x800": (800, 800, True),
    "square_800x800_notext": (800, 800, False),
    "square_512x512": (512, 512, True),
    "landscape_512x384": (512, 384, True),
    "landscape_200x120_notext": (200, 120, False),
}

CYAN = (0.0, 1.0, 0.92)
PINK = (1.0, 0.25, 0.7)
GOLD = (1.0, 0.78, 0.25)
HORIZON = -0.18


def clamp(x: float, lo: float = 0.0, hi: float = 1.0) -> float:
    return lo if x < lo else hi if x > hi else x


def smooth(e0: float, e1: float, x: float) -> float:
    t = clamp((x - e0) / (e1 - e0))
    return t * t * (3 - 2 * t)


def mix(a, b, t: float):
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)


def hash2(i: int, j: int) -> float:
    n = (i * 374761393 + j * 668265263) & 0xFFFFFFFF
    n = ((n ^ (n >> 13)) * 1274126177) & 0xFFFFFFFF
    return ((n ^ (n >> 16)) & 0xFFFF) / 65535.0


# ---- shapes (signed distance, negative inside) ---------------------------------------


def sd_ellipse(px: float, py: float, cx: float, cy: float, rx: float, ry: float) -> float:
    # A cheap bound that is exact on the axes: good enough for a soft edge.
    k = math.hypot((px - cx) / rx, (py - cy) / ry)
    return (k - 1.0) * min(rx, ry)


def sd_tri(px: float, py: float, a, b, c) -> float:
    def edge(p, q):
        ex, ey = q[0] - p[0], q[1] - p[1]
        t = clamp(((px - p[0]) * ex + (py - p[1]) * ey) / (ex * ex + ey * ey))
        return math.hypot(px - (p[0] + ex * t), py - (p[1] + ey * t))

    d = min(edge(a, b), edge(b, c), edge(c, a))

    def side(p, q):
        return (q[0] - p[0]) * (py - p[1]) - (q[1] - p[1]) * (px - p[0])

    s = (side(a, b), side(b, c), side(c, a))
    inside = all(t >= 0 for t in s) or all(t <= 0 for t in s)
    return -d if inside else d


def sd_seg(px: float, py: float, ax: float, ay: float, bx: float, by: float) -> float:
    ex, ey = bx - ax, by - ay
    t = clamp(((px - ax) * ex + (py - ay) * ey) / (ex * ex + ey * ey))
    return math.hypot(px - (ax + ex * t), py - (ay + ey * t))


# ---- the title: strokes in a 0..1 (x) by 0..1.5 (y) box ------------------------------

_O = [(0.18, 0.0), (0.82, 0.0), (1.0, 0.18), (1.0, 1.32), (0.82, 1.5), (0.18, 1.5), (0.0, 1.32), (0.0, 0.18), (0.18, 0.0)]
_C = [(1.0, 0.3), (0.82, 0.0), (0.18, 0.0), (0.0, 0.18), (0.0, 1.32), (0.18, 1.5), (0.82, 1.5), (1.0, 1.2)]
LETTERS: dict[str, list[list[tuple[float, float]]]] = {
    "N": [[(0.0, 0.0), (0.0, 1.5), (1.0, 0.0), (1.0, 1.5)]],
    "E": [[(1.0, 1.5), (0.0, 1.5), (0.0, 0.0), (1.0, 0.0)], [(0.0, 0.75), (0.78, 0.75)]],
    "O": [_O],
    "C": [_C],
    "A": [[(0.0, 0.0), (0.5, 1.5), (1.0, 0.0)], [(0.2, 0.55), (0.8, 0.55)]],
    "T": [[(0.0, 1.5), (1.0, 1.5)], [(0.5, 1.5), (0.5, 0.0)]],
}


def title_segments(text: str, height: float, cx: float, cy: float):
    """Line segments of the title, centred on (cx, cy); ``height`` is the letter height."""
    s = height / 1.5
    adv = 1.45 * s
    words = text.split(" ")
    widths = [len(w) * adv - 0.45 * s for w in words]
    gap = 1.1 * s
    total = sum(widths) + gap * (len(words) - 1)
    x0 = cx - total / 2
    segs = []
    for w, ww in zip(words, widths):
        for k, ch in enumerate(w):
            ox = x0 + k * adv
            for line in LETTERS[ch]:
                pts = [(ox + x * s, cy - height / 2 + y * s) for x, y in line]
                segs += [(pts[i][0], pts[i][1], pts[i + 1][0], pts[i + 1][1]) for i in range(len(pts) - 1)]
        x0 += ww + gap
    return segs


# ---- the scene -------------------------------------------------------------------------


class Scene:
    def __init__(self, w: int, h: int, titled: bool) -> None:
        self.w, self.h, self.titled = w, h, titled
        self.aspect = w / h
        a = self.aspect
        # Layout in a space where height is 2 (y up) and x runs -a..a.
        wide = a >= 1.2
        self.k = 0.85 if a >= 1.2 else 0.8 if a > 0.8 else 0.7
        self.cat_x = -0.22 * a if wide else 0.0
        self.cat_y = -0.72 if wide else -0.7
        self.px = 2.0 / h  # one pixel
        # Coins arc over the cat's right shoulder.
        self.coins = []
        for i in range(5):
            t = i / 4
            cx = self.cat_x + (0.55 + 0.75 * t) * self.k * (1.0 if wide else 0.62)
            cy = self.cat_y + (0.55 + 0.5 * math.sin(math.pi * t)) * self.k * (1.0 if wide else 1.1)
            self.coins.append((cx, cy, 0.075 * self.k * (0.8 + 0.4 * t)))
        if titled:
            th = min(0.34, 0.21 * a) if wide else 0.26
            ty = 0.7 if wide else 0.56
            tx = 0.0
            if a < 0.9:  # portrait: two lines
                self.title = title_segments("NEON", th * 1.0, tx, ty + 0.2) + title_segments("CAT", th * 1.0, tx, ty - 0.2)
            else:
                self.title = title_segments("NEON CAT", th, tx, ty)
            xs = [v for s in self.title for v in (s[0], s[2])]
            ys = [v for s in self.title for v in (s[1], s[3])]
            m = 0.16
            self.title_box = (min(xs) - m, max(xs) + m, min(ys) - m, max(ys) + m)
            self.title_w = th * 0.1
        else:
            self.title = []
            self.title_box = None
        self.seed_city = [hash2(i, 7) for i in range(256)]

    # -- layers --

    def sky(self, x: float, y: float):
        t = clamp((y - HORIZON) / (1.0 - HORIZON))
        col = mix((0.55, 0.12, 0.62), (0.03, 0.02, 0.14), smooth(0.0, 0.75, t) ** 0.8)
        # sun
        sx, sy, sr = 0.0 if self.aspect < 1.2 else 0.28 * self.aspect, HORIZON + 0.42 * self.k, 0.52 * self.k
        d = math.hypot(x - sx, y - sy) - sr
        bands = 1.0
        if y < sy - 0.02:  # horizontal slits widen toward the bottom
            u = (sy - y) / sr
            bands = 1.0 if math.sin(u * 22.0) * (0.15 + 0.85 * u) < 0.45 * u - 0.1 else 0.0
            bands = clamp(bands + 0.0)
        sun = mix(PINK, (1.0, 0.85, 0.3), clamp((y - (sy - sr)) / (2 * sr)))
        glow = math.exp(-max(d, 0.0) / 0.22) * 0.55
        col = mix(col, (0.9, 0.2, 0.6), glow * 0.7)
        col = mix(col, sun, (1.0 - smooth(-self.px, self.px, d)) * bands)
        # stars
        if y > HORIZON + 0.55:
            gx, gy = int((x + 3) * 60), int((y + 1) * 60)
            hsh = hash2(gx, gy)
            if hsh > 0.985:
                col = mix(col, (1, 1, 1), 0.35 + 0.6 * hash2(gy, gx))
        return col

    def skyline(self, x: float, y: float, col):
        if y > HORIZON + 0.5:
            return col
        i = int(math.floor((x + 4.0) / 0.13))
        hgt = 0.08 + 0.34 * self.seed_city[i % 256] ** 1.4
        if y < HORIZON or y > HORIZON + hgt:
            return col
        body = (0.04, 0.025, 0.1)
        wx, wy = int((x + 4.0) / 0.04), int((y - HORIZON) / 0.045)
        lit = hash2(wx, wy + i) > 0.86
        if lit:
            body = mix(body, CYAN if hash2(wx, wy) > 0.35 else PINK, 0.55)
        return body

    def floor(self, x: float, y: float):
        t = HORIZON - y  # distance below the horizon
        base = mix((0.05, 0.02, 0.14), (0.02, 0.01, 0.06), smooth(0.0, 1.0, t))
        wz = 0.12 / (t + 0.004)
        wx = x * wz * 1.3
        fx = abs((wx % 1.0) - 0.5)
        fz = abs((wz % 1.0) - 0.5)
        width = 0.035 + 0.0015 * wz
        lx = smooth(0.5 - width, 0.5, fx)
        lz = smooth(0.5 - width * 1.3, 0.5, fz)
        line = max(lx, lz)
        fade = smooth(0.0, 0.12, t)
        col = mix(base, mix(PINK, CYAN, clamp(abs(x) * 0.5)), line * fade * 0.9)
        col = mix(col, (0.9, 0.2, 0.6), math.exp(-t / 0.05) * 0.5)
        return col

    def cat_distance(self, x: float, y: float) -> float:
        """Signed distance of the sitting cat, seen from behind-left, tail curling."""
        k = self.k
        ox, oy = x - self.cat_x, y - self.cat_y
        ox, oy = ox / k, oy / k
        body = sd_ellipse(ox, oy, 0.0, 0.34, 0.34, 0.42)
        head = sd_ellipse(ox, oy, 0.02, 0.9, 0.27, 0.23)
        ear_l = sd_tri(ox, oy, (-0.25, 1.0), (-0.1, 1.08), (-0.27, 1.34))
        ear_r = sd_tri(ox, oy, (0.29, 1.0), (0.14, 1.08), (0.32, 1.34))
        haunch = sd_ellipse(ox, oy, 0.16, 0.12, 0.34, 0.16)
        # tail: a few capsules along a curve
        tail = 9.0
        for i in range(8):
            t0, t1 = i / 8, (i + 1) / 8
            ax, ay = 0.34 + 0.5 * math.sin(t0 * 2.2), 0.1 + 0.95 * t0 ** 1.2
            bx, by = 0.34 + 0.5 * math.sin(t1 * 2.2), 0.1 + 0.95 * t1 ** 1.2
            tail = min(tail, sd_seg(ox, oy, ax, ay, bx, by) - (0.075 - 0.02 * t0))
        d = min(body, head, ear_l, ear_r, haunch, tail)
        return d * k

    def eyes(self, x: float, y: float) -> float:
        k = self.k
        ox, oy = (x - self.cat_x) / k, (y - self.cat_y) / k
        e1 = sd_ellipse(ox, oy, -0.1, 0.92, 0.055, 0.075)
        e2 = sd_ellipse(ox, oy, 0.15, 0.92, 0.055, 0.075)
        return min(e1, e2) * k

    def pixel(self, ix: int, iy: int):
        a = self.aspect
        x = ((ix + 0.5) / self.w * 2 - 1) * a
        y = 1 - (iy + 0.5) / self.h * 2
        col = self.sky(x, y) if y >= HORIZON else self.floor(x, y)
        if y >= HORIZON:
            col = self.skyline(x, y, col)
        px = self.px
        # coins
        for cx, cy, r in self.coins:
            if abs(x - cx) < r * 2.6 and abs(y - cy) < r * 2.6:
                d = math.hypot(x - cx, y - cy)
                rim = abs(d - r) - r * 0.28
                col = mix(col, GOLD, math.exp(-max(d - r, 0) / (r * 0.7)) * 0.35 * (1.0 - smooth(r * 1.6, r * 2.5, d)))
                col = mix(col, (1.0, 0.9, 0.5), 1.0 - smooth(-px, px, rim))
                if d < r * 0.72:
                    col = mix(col, (0.5, 0.3, 0.05), 0.25)
        # cat
        k = self.k
        if abs(x - self.cat_x) < 1.5 * k and self.cat_y - 0.6 * k < y < self.cat_y + 1.8 * k:
            d = self.cat_distance(x, y)
            if d < 0.2:
                glow = math.exp(-max(d, 0) / 0.035) * (1.0 - smooth(0.08, 0.2, d))
                col = mix(col, CYAN, glow * 0.55 * (1.0 if d > 0 else 0.0))
                fill = 1.0 - smooth(-px * 1.2, px * 1.2, d)
                inner = mix((0.03, 0.02, 0.09), (0.07, 0.05, 0.18), clamp(-d / 0.18))
                # rim light on the left edge: stronger where the surface faces up-left
                rim = math.exp(-max(-d, 0) / 0.012) * (0.6 + 0.4 * clamp((self.cat_x - x) / (0.4 * k) + 0.5))
                col = mix(col, mix(inner, CYAN, clamp(rim)), fill)
                ed = self.eyes(x, y)
                col = mix(col, (0.75, 1.0, 0.35), (1.0 - smooth(-px, px, ed)) * fill)
        # title
        tb = self.title_box
        if tb and tb[0] < x < tb[1] and tb[2] < y < tb[3]:
            d = min(sd_seg(x, y, *s) for s in self.title) - self.title_w * 0.5
            glow = math.exp(-max(d, 0) / 0.03)
            col = mix(col, PINK, glow * 0.55)
            col = mix(col, (1.0, 0.95, 0.98), 1.0 - smooth(-px, px, d))
        # vignette
        v = 1.0 - 0.28 * clamp((math.hypot(x / a, y) - 0.6) / 0.9)
        return (clamp(col[0] * v), clamp(col[1] * v), clamp(col[2] * v))


def render_rgb(w: int, h: int, titled: bool) -> bytes:
    scene = Scene(w, h, titled)
    rows = bytearray()
    for iy in range(h):
        rows.append(0)
        for ix in range(w):
            r, g, b = scene.pixel(ix, iy)
            rows += bytes((int(r * 255 + 0.5), int(g * 255 + 0.5), int(b * 255 + 0.5)))
    return bytes(rows)


def png_bytes(w: int, h: int, titled: bool) -> bytes:
    raw = render_rgb(w, h, titled)

    def chunk(tag: bytes, payload: bytes) -> bytes:
        return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Draw the Neon Cat store images")
    ap.add_argument("--out", default=str(OUT_DIR), help="folder (default docs/media/neon_cat)")
    ap.add_argument("--only", action="append", default=[], help="one image name (repeatable)")
    ap.add_argument("--list", action="store_true", help="list the image names and exit")
    args = ap.parse_args(argv)
    if args.list:
        for name, (w, h, titled) in IMAGES.items():
            print(f"{name}.png  {w}x{h}  {'title' if titled else 'no text'}")
        return 0
    unknown = [n for n in args.only if n not in IMAGES]
    if unknown:
        print(f"unknown image: {', '.join(unknown)} (see --list)", file=sys.stderr)
        return 2
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    for name, (w, h, titled) in IMAGES.items():
        if args.only and name not in args.only:
            continue
        data = png_bytes(w, h, titled)
        (out / f"{name}.png").write_bytes(data)
        print(f"{out / (name + '.png')}  {w}x{h}  {len(data) // 1024} KB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
