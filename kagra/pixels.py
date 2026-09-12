"""GPU-free PNG region checks for verify (not golden pixels).

Used after ``expect_offscreen`` writes a PNG. Catches "the wall is the
outdoor sky" / "the floor is a solid green plane" without storing a
reference image.
"""
from __future__ import annotations

import struct
import zlib
from pathlib import Path
from typing import Any

__all__ = ["decode_png_rgba", "region_stats", "eval_expect_pixels"]


def decode_png_rgba(path: str | Path) -> tuple[int, int, bytes]:
    """8-bit RGB/RGBA, non-interlaced. Returns (w, h, RGBA bytes)."""
    data = Path(path).read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"not a PNG: {path}")
    pos = 8
    width = height = 0
    color_type = 6
    raw = bytearray()
    while pos + 8 <= len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        typ = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if typ == b"IHDR":
            width, height, _bit, color_type, _c, _f, inter = struct.unpack(">IIBBBBB", chunk[:13])
            if inter != 0:
                raise ValueError("interlaced PNG not supported")
            if _bit != 8 or color_type not in (2, 6):
                raise ValueError(f"need 8-bit RGB/RGBA, got bit={_bit} type={color_type}")
        elif typ == b"IDAT":
            raw.extend(chunk)
        elif typ == b"IEND":
            break
    rows = zlib.decompress(bytes(raw))
    bpp = 4 if color_type == 6 else 3
    stride = 1 + width * bpp
    out = bytearray(width * height * 4)
    for y in range(height):
        row = rows[y * stride : (y + 1) * stride]
        if not row:
            break
        filt = row[0]
        src = bytearray(row[1:])
        prev = out[(y - 1) * width * 4 : y * width * 4] if y > 0 else b""

        def pred(i: int) -> tuple[int, int, int]:
            c = i % bpp
            px = i // bpp
            left = src[i - bpp] if i >= bpp else 0
            up = prev[px * 4 + c] if prev else 0
            ul = prev[(px - 1) * 4 + c] if prev and px else 0
            return left, up, ul

        if filt == 1:
            for i in range(bpp, len(src)):
                src[i] = (src[i] + src[i - bpp]) & 255
        elif filt == 2:
            for i in range(len(src)):
                src[i] = (src[i] + pred(i)[1]) & 255
        elif filt == 3:
            for i in range(len(src)):
                left, up, _ = pred(i)
                src[i] = (src[i] + (left + up) // 2) & 255
        elif filt == 4:
            for i in range(len(src)):
                a, b, c = pred(i)
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                src[i] = (src[i] + pr) & 255
        elif filt != 0:
            raise ValueError(f"PNG filter {filt} not supported")
        dst = y * width * 4
        if bpp == 4:
            out[dst : dst + width * 4] = src
        else:
            for x in range(width):
                out[dst + x * 4 : dst + x * 4 + 3] = src[x * 3 : x * 3 + 3]
                out[dst + x * 4 + 3] = 255
    return int(width), int(height), bytes(out)


def region_stats(
    rgba: bytes, width: int, height: int, x: int, y: int, w: int, h: int
) -> dict[str, Any]:
    x0 = max(0, int(x))
    y0 = max(0, int(y))
    x1 = min(int(width), x0 + max(1, int(w)))
    y1 = min(int(height), y0 + max(1, int(h)))
    if x0 >= x1 or y0 >= y1:
        raise ValueError("empty pixel region")
    n = (x1 - x0) * (y1 - y0)
    sr = sg = sb = 0
    rmin = gmin = bmin = 255
    rmax = gmax = bmax = 0
    for yy in range(y0, y1):
        row = yy * width * 4
        for xx in range(x0, x1):
            i = row + xx * 4
            r, g, b = rgba[i], rgba[i + 1], rgba[i + 2]
            sr += r
            sg += g
            sb += b
            rmin = min(rmin, r)
            gmin = min(gmin, g)
            bmin = min(bmin, b)
            rmax = max(rmax, r)
            gmax = max(gmax, g)
            bmax = max(bmax, b)
    mean = [sr / n, sg / n, sb / n]
    spread = max(rmax - rmin, gmax - gmin, bmax - bmin)
    return {"mean": mean, "spread": spread, "n": n}


def eval_expect_pixels(path: str | Path, checks: list[Any] | None) -> list[str]:
    """Return error strings (empty = ok). Each check is a region dict."""
    if not checks:
        return []
    try:
        width, height, rgba = decode_png_rgba(path)
    except Exception as exc:
        return [f"pixels: cannot decode {path}: {exc}"]
    errors: list[str] = []
    for i, raw in enumerate(checks):
        if not isinstance(raw, dict):
            errors.append(f"pixels[{i}]: expected object")
            continue
        try:
            st = region_stats(
                rgba,
                width,
                height,
                int(raw.get("x", 0)),
                int(raw.get("y", 0)),
                int(raw.get("w", width)),
                int(raw.get("h", height)),
            )
        except Exception as exc:
            errors.append(f"pixels[{i}]: {exc}")
            continue
        mean = st["mean"]
        if raw.get("not_solid"):
            need = float(raw.get("spread", 8))
            if st["spread"] < need:
                errors.append(
                    f"pixels[{i}]: solid color (spread {st['spread']} < {need})"
                )
        if "mean_min" in raw:
            lo = [float(v) for v in raw["mean_min"]]
            if any(mean[c] < lo[c] for c in range(min(3, len(lo)))):
                errors.append(f"pixels[{i}]: mean {mean} < min {lo}")
        if "mean_max" in raw:
            hi = [float(v) for v in raw["mean_max"]]
            if any(mean[c] > hi[c] for c in range(min(3, len(hi)))):
                errors.append(f"pixels[{i}]: mean {mean} > max {hi}")
    return errors
