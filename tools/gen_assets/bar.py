#!/usr/bin/env python3
"""Generate Bar sim textures / SE / copy (deterministic, no network).

AI image models can replace the PNG later; this writer is the contract:
same paths, same manifest keys, play-time has no generator dependency.
"""
from __future__ import annotations

import json
import math
import struct
import sys
import wave
import zlib
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "assets" / "bar"
TEX = OUT / "tex"
SE = OUT / "se"


def rgba_png(rgba: bytes, w: int, h: int) -> bytes:
    def chunk(typ: bytes, data: bytes) -> bytes:
        c = struct.pack(">I", len(data)) + typ + data
        return c + struct.pack(">I", zlib.crc32(typ + data) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + rgba[y * w * 4 : (y + 1) * w * 4] for y in range(h))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 6))
        + chunk(b"IEND", b"")
    )


def write_png(path: Path, w: int, h: int, px) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(rgba_png(bytes(px), w, h))


def wood(w=64, h=64, seed=3) -> list[int]:
    out = []
    for y in range(h):
        for x in range(w):
            n = math.sin((x + seed * 3) * 0.35) * 18 + math.sin(y * 0.08) * 8
            r = int(96 + n + (y % 7) * 2)
            g = int(58 + n * 0.6)
            b = int(36 + n * 0.3)
            out.extend([max(0, min(255, r)), max(0, min(255, g)), max(0, min(255, b)), 255])
    return out


def wall(w=64, h=64) -> list[int]:
    out = []
    for y in range(h):
        for x in range(w):
            v = 28 + (x * 3 + y * 5) % 11
            out.extend([v + 18, v + 8, v + 14, 255])
    return out


def portrait(hue: tuple[int, int, int], w=48, h=64) -> list[int]:
    out = []
    cx, cy = w / 2, h * 0.42
    for y in range(h):
        for x in range(w):
            dx, dy = x - cx, y - cy
            face = dx * dx / (14 * 14) + dy * dy / (16 * 16)
            if y < 8 or y > h - 6:
                out.extend([0, 0, 0, 0])
            elif face < 1.0:
                shade = 1.0 - face * 0.25
                out.extend([int(hue[0] * shade), int(hue[1] * shade), int(hue[2] * shade), 255])
            elif 22 < y < 48 and abs(dx) < 16:
                out.extend([40, 28, 48, 255])
            else:
                out.extend([0, 0, 0, 0])
    # eyes
    def put(px, py, c):
        if 0 <= px < w and 0 <= py < h:
            i = (py * w + px) * 4
            out[i : i + 4] = c

    for ox in (-6, 6):
        put(int(cx + ox), int(cy), [20, 16, 18, 255])
    return out


def logo(w=64, h=28) -> list[int]:
    out = []
    for y in range(h):
        for x in range(w):
            edge = x < 2 or x > w - 3 or y < 2 or y > h - 3
            out.extend([210, 170, 90, 255] if edge else [32, 18, 22, 230])
    return out


def bottle(color, w=24, h=48) -> list[int]:
    out = []
    for y in range(h):
        for x in range(w):
            neck = y < 12 and 8 <= x <= 15
            body = y >= 12 and 4 <= x <= 19
            if neck or body:
                a = 255 if 6 <= x <= 17 or neck else 200
                out.extend([color[0], color[1], color[2], a])
            else:
                out.extend([0, 0, 0, 0])
    return out


def tone_wav(freq: float, dur: float, vol=0.35, rate=22050) -> bytes:
    n = int(rate * dur)
    frames = bytearray()
    for i in range(n):
        t = i / rate
        env = max(0.0, 1.0 - t / dur)
        s = int(vol * env * 32767 * math.sin(2 * math.pi * freq * t))
        frames += struct.pack("<h", max(-32767, min(32767, s)))
    buf = BytesIO()
    with wave.open(buf, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(bytes(frames))
    return buf.getvalue()


def main() -> None:
    TEX.mkdir(parents=True, exist_ok=True)
    SE.mkdir(parents=True, exist_ok=True)
    write_png(TEX / "wood.png", 64, 64, wood())
    write_png(TEX / "wall.png", 64, 64, wall())
    write_png(TEX / "logo.png", 64, 28, logo())
    hues = {
        "ren": (210, 140, 120),
        "sora": (160, 180, 210),
        "kai": (190, 150, 110),
        "mio": (200, 130, 150),
        "yuki": (180, 190, 200),
        "haru": (170, 140, 100),
    }
    for name, hue in hues.items():
        write_png(TEX / f"{name}.png", 48, 64, portrait(hue))
    write_png(TEX / "bottle_whiskey.png", 24, 48, bottle((140, 70, 40)))
    write_png(TEX / "bottle_gin.png", 24, 48, bottle((80, 150, 130)))
    write_png(TEX / "bottle_wine.png", 24, 48, bottle((120, 40, 50)))
    (SE / "glass.wav").write_bytes(tone_wav(880, 0.12))
    (SE / "door.wav").write_bytes(tone_wav(220, 0.18, vol=0.25))
    (SE / "register.wav").write_bytes(tone_wav(660, 0.09) + tone_wav(990, 0.07))
    manifest = {
        "generator": "tools/gen_assets/bar.py",
        "license": "AI / procedural — generated for KAGRA Bar sim. Not third-party art.",
        "textures": sorted(p.name for p in TEX.glob("*.png")),
        "se": sorted(p.name for p in SE.glob("*.wav")),
    }
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    sys.exit(main())
