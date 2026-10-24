"""Hop Beat store art: cover images drawn by this script (CC0-1.0, no outside images).

A dusk scene for a rhythm hopper: a banded sun that pulses in rings, a skyline made
of equaliser bars, a track of glowing beat tiles with gaps, and a round long-eared
hero caught mid-hop over a gap. In the titled versions the HOP BEAT title is drawn
in the stroke font of ``cover_art.py``. Every pixel comes from the formulas below
with the standard library only, so the same command gives the same bytes anywhere.

    python3 tools/gen_assets/hop_cover.py                    # write docs/media/hop_beat/*.png
    python3 tools/gen_assets/hop_cover.py --only square_512x512
    python3 tools/gen_assets/hop_cover.py --list

The text-free versions are for portals that ban text on thumbnails (Poki).
"""
from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import cover_art as ca  # noqa: E402  (shares the PNG writer, the stroke font and the shape maths)

ROOT = HERE.parents[1]
OUT_DIR = ROOT / "docs" / "media" / "hop_beat"
TITLE = "HOP BEAT"

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

# The stroke font has no H, P or B yet: add them here (box 0..1 by 0..1.5).
ca.LETTERS.setdefault("H", [[(0.0, 0.0), (0.0, 1.5)], [(1.0, 0.0), (1.0, 1.5)], [(0.0, 0.75), (1.0, 0.75)]])
ca.LETTERS.setdefault("P", [[(0.0, 0.0), (0.0, 1.5), (0.8, 1.5), (1.0, 1.32), (1.0, 0.93), (0.8, 0.75), (0.0, 0.75)]])
ca.LETTERS.setdefault(
    "B",
    [[(0.0, 0.0), (0.0, 1.5), (0.78, 1.5), (0.96, 1.34), (0.96, 1.0), (0.78, 0.8), (0.0, 0.8)],
     [(0.78, 0.8), (1.0, 0.62), (1.0, 0.18), (0.82, 0.0), (0.0, 0.0)]],
)

CYAN = (0.0, 1.0, 0.92)
PINK = (1.0, 0.25, 0.7)
GOLD = (1.0, 0.78, 0.25)
CREAM = (1.0, 0.93, 0.8)
HORIZON = -0.12
BPM = 126.0

# Which beat tiles are missing (the gaps), in track rows from the camera outwards.
GAP_ROWS = {4, 9, 10, 15, 21}


def _bars(n: int = 26) -> list[float]:
    return [0.35 + 0.65 * ca.hash2(i, 77) for i in range(n)]


class Scene:
    def __init__(self, w: int, h: int, titled: bool) -> None:
        self.w, self.h, self.titled = w, h, titled
        self.a = w / h
        self.bars = _bars()
        self.title_h = 0.34 if self.a > 1.2 else 0.30
        self.title: list[tuple[float, float, float, float]] = []
        if titled:
            if self.a > 1.2:
                self.title = ca.title_segments(TITLE, self.title_h, 0.0, 0.62)
            else:
                # tall or square images: two lines so the letters stay big
                top = ca.title_segments("HOP", self.title_h, 0.0, 0.70)
                bot = ca.title_segments("BEAT", self.title_h, 0.0, 0.70 - self.title_h * 1.45)
                self.title = top + bot
            self.title_w = 0.05

    def title_scale(self) -> float:
        return 1.0

    def pixel(self, ix: int, iy: int) -> tuple[float, float, float]:
        w, h, a = self.w, self.h, self.a
        x = (ix + 0.5 - w / 2) / (h / 2)       # -a..a, y up in -1..1
        y = -((iy + 0.5 - h / 2) / (h / 2))
        px = 1.5 / (h / 2)
        # sky: deep violet to hot pink at the horizon
        t = ca.clamp((y - HORIZON) / (1.0 - HORIZON))
        col = ca.mix((0.95, 0.30, 0.55), (0.07, 0.03, 0.20), t ** 0.7)
        # stars
        if y > 0.25:
            cell = (int((x + 4) * 90), int((y + 1) * 90))
            r = ca.hash2(*cell)
            if r > 0.985:
                col = ca.mix(col, (1, 1, 1), 0.7 * (r - 0.985) / 0.015)
        # sun: bands cut out of the lower half, ring pulses (one per beat) around it
        sx, sy, sr = 0.0, HORIZON + 0.42, 0.34
        d = math.hypot(x - sx, y - sy) - sr
        sun = 1.0 - ca.smooth(-px, px, d)
        band = 1.0
        if y < sy:
            k = (sy - y) / sr
            band = 0.0 if (math.sin(k * 26.0 - 1.0) > 0.25 and k > 0.18) else 1.0
        suncol = ca.mix(GOLD, PINK, ca.clamp((sy + sr - y) / (2 * sr)))
        col = ca.mix(col, suncol, sun * band)
        for i, (rr, wd, al) in enumerate(((0.50, 0.007, 0.55), (0.66, 0.006, 0.35), (0.84, 0.005, 0.2))):
            ring = abs(math.hypot(x - sx, y - sy) - rr)
            col = ca.mix(col, CYAN, al * (1.0 - ca.smooth(wd * 0.5, wd * 0.5 + 2 * px, ring)))
        # skyline of equaliser bars along the horizon
        if HORIZON <= y < HORIZON + 0.5:
            bi = int((x / a * 0.5 + 0.5) * len(self.bars))
            if 0 <= bi < len(self.bars):
                cx = ((bi + 0.5) / len(self.bars) - 0.5) * 2 * a
                half = a / len(self.bars) * 0.7
                top = HORIZON + 0.5 * self.bars[bi] * (0.55 + 0.45 * math.cos(abs(x) * 1.3))
                if abs(x - cx) < half and y < top:
                    seg = (y - HORIZON) / 0.035
                    lit = 0.0 if (seg % 1.0) > 0.78 else 1.0
                    base = ca.mix((0.10, 0.05, 0.24), (0.25, 0.10, 0.45), ca.clamp((y - HORIZON) / 0.5))
                    col = ca.mix(base, ca.mix(CYAN, PINK, ca.clamp((y - HORIZON) / 0.5)), 0.55 * lit)
        # floor: a perspective track of beat tiles, gaps showing the dark below
        if y < HORIZON:
            dy = HORIZON - y
            z = 0.42 / dy                                  # depth
            lane = x / (0.95 * (dy + 0.02)) * 0.42 / 0.42  # lateral position, 1 = track edge
            col = ca.mix((0.10, 0.04, 0.22), (0.03, 0.01, 0.10), ca.clamp(dy * 1.5))
            if abs(lane) < 1.0:
                row = int(z * 1.6)
                fz = z * 1.6 - row
                gap = row in GAP_ROWS
                edge = ca.smooth(0.0, 0.06, 1.0 - abs(lane))
                if not gap:
                    tile = ca.mix((0.30, 0.20, 0.62), (0.55, 0.30, 0.78), ca.clamp(1.0 - dy * 1.2))
                    seam = ca.smooth(0.0, 0.08, min(fz, 1.0 - fz))
                    col = ca.mix(PINK, tile, seam)
                    col = ca.mix(CYAN, col, edge)
                    col = ca.mix(col, CYAN, 0.0)
                else:
                    # the gap: a faint glow from deep below
                    col = ca.mix(col, (0.45, 0.12, 0.50), 0.35 * (1.0 - fz) * edge)
                    col = ca.mix(CYAN, col, ca.smooth(0.0, 0.05, 1.0 - abs(lane)))
            # distance haze into the horizon glow
            col = ca.mix(col, (0.95, 0.35, 0.65), ca.clamp(1.0 - dy * 6.0) * 0.55)
        # the hero: a round body, two long ears, a cyan rim light, in mid-hop over the gap
        hx, hy = 0.02, -0.34
        if self.a < 0.9:
            hy = -0.40
        body = ca.sd_ellipse(x, y, hx, hy, 0.20, 0.19)
        ear1 = ca.sd_ellipse(x, y, hx - 0.075, hy + 0.30, 0.045, 0.14)
        ear2 = ca.sd_ellipse(x, y, hx + 0.075, hy + 0.30, 0.045, 0.14)
        tail = ca.sd_ellipse(x, y, hx + 0.19, hy - 0.04, 0.05, 0.05)
        hero = min(body, ear1, ear2, tail)
        # shadow on the tile far below the hero, small because it is high
        sh = ca.sd_ellipse(x, y, hx, hy - 0.40, 0.14, 0.025)
        col = ca.mix(col, (0.02, 0.0, 0.08), 0.45 * (1.0 - ca.smooth(-px, 3 * px, sh)))
        if hero < 0.03:
            fill = 1.0 - ca.smooth(-px, px, hero)
            lit = ca.clamp(0.5 + 0.9 * ((x - hx) * -0.6 + (y - hy) * 0.8))
            body_col = ca.mix((0.97, 0.88, 0.70), (1.0, 0.98, 0.92), lit)
            ear_in = ca.sd_ellipse(x, y, hx - 0.075, hy + 0.30, 0.02, 0.10)
            ear_in2 = ca.sd_ellipse(x, y, hx + 0.075, hy + 0.30, 0.02, 0.10)
            if min(ear_in, ear_in2) < 0:
                body_col = ca.mix(body_col, PINK, 0.8)
            rim = math.exp(-max(hero + 0.02, 0) / 0.012)
            body_col = ca.mix(body_col, CYAN, 0.55 * rim * ca.clamp(-(x - hx) * 6.0))
            # an eye and a cheek
            eye = math.hypot(x - (hx + 0.06), y - (hy + 0.03)) - 0.018
            body_col = ca.mix(body_col, (0.08, 0.04, 0.18), 1.0 - ca.smooth(-px, px, eye))
            cheek = math.hypot(x - (hx + 0.10), y - (hy - 0.04)) - 0.026
            body_col = ca.mix(body_col, PINK, 0.5 * (1.0 - ca.smooth(-px, px, cheek)))
            col = ca.mix(col, body_col, fill)
        # a halo for the perfect landing the hero is about to make
        halo = abs(math.hypot((x - hx) * 1.0, (y - hy + 0.40) * 3.0) - 0.30)
        col = ca.mix(col, GOLD, 0.45 * (1.0 - ca.smooth(0.004, 0.004 + 2 * px, halo)) * (1.0 if y < hy - 0.30 else 0.0))
        # the title
        if self.title:
            dd = min(ca.sd_seg(x, y, *s) for s in self.title) - self.title_w * 0.5
            glow = math.exp(-max(dd, 0) / 0.03)
            col = ca.mix(col, CYAN, glow * 0.55)
            col = ca.mix(col, (1.0, 0.97, 0.9), 1.0 - ca.smooth(-px, px, dd))
        v = 1.0 - 0.28 * ca.clamp((math.hypot(x / a, y) - 0.6) / 0.9)
        return (ca.clamp(col[0] * v), ca.clamp(col[1] * v), ca.clamp(col[2] * v))


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
    import struct
    import zlib

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
    ap = argparse.ArgumentParser(description="Draw the Hop Beat store images")
    ap.add_argument("--out", default=str(OUT_DIR), help="folder (default docs/media/hop_beat)")
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
