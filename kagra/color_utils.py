"""色ユーティリティ（GPU / kagra_core 非依存）。"""

from __future__ import annotations


def clamp_u8(v) -> int:
    iv = int(v)
    return max(0, min(255, iv))


def norm_color(value, default_a: int = 255):
    if not isinstance(value, (tuple, list)):
        raise ValueError("color must be (r,g,b) or (r,g,b,a)")
    r, g, b = value[0], value[1], value[2]
    a = value[3] if len(value) > 3 else default_a
    return clamp_u8(r), clamp_u8(g), clamp_u8(b), clamp_u8(a)


def resolve_rgb(first, g=None, b=None, a: int = 255):
    """後方互換: rect(..., 255,128,0) 形式と (r,g,b) タプル両対応。"""
    if isinstance(first, (tuple, list)):
        return norm_color(first, a)
    if g is None and b is None:
        v = clamp_u8(first)
        return v, v, v, clamp_u8(a)
    return clamp_u8(first), clamp_u8(g), clamp_u8(b), clamp_u8(a)
