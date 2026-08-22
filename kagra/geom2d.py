"""2D 幾何ヘルパ（描画の近似に使う。GPU 不要）。"""
from __future__ import annotations

import math


def line_rects(
    x1: float, y1: float, x2: float, y2: float, width: float = 1.0,
) -> list[tuple[float, float, float, float]]:
    """線分を軸揃え矩形の列で近似する。(x, y, w, h) のリスト。"""
    dx = x2 - x1
    dy = y2 - y1
    length = math.sqrt(dx * dx + dy * dy)
    if length < 0.5:
        return []
    width = max(1.0, float(width))
    steps = max(2, int(length / width))
    pw = max(1.0, length / steps + width * 0.3)
    ph = width
    out: list[tuple[float, float, float, float]] = []
    for i in range(steps):
        t = i / steps
        px = x1 + dx * t
        py = y1 + dy * t
        out.append((px, py - ph / 2, pw, ph))
    return out
