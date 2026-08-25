"""Nearby extra VRM avatars for FPS measurement.

GPU-free helpers. Crest Isle play stays single-player; spawn extras via
``examples/vrm_multi_avatar.py`` (or ``kagra.avatar(path)`` N times).
Same-path loads share mesh/texture/MToon in the engine.
"""
from __future__ import annotations

import math


DEFAULT_COUNT = 4
DEFAULT_RADIUS = 2.2


def crowd_count(raw: str | int | None = None, *, default: int = DEFAULT_COUNT) -> int:
    """``KAGRA_AVATARS`` → clamp 1..32 (player + extras)."""
    if raw is None:
        n = default
    else:
        n = int(raw)
    return max(1, min(32, n))


def crowd_offsets(n: int, *, radius: float = DEFAULT_RADIUS) -> list[tuple[float, float]]:
    """XZ offsets for *extra* avatars (not including the player at origin).

    ``n`` extras sit on a ring so a chase cam looking +Z sees several bodies.
    """
    n = max(0, int(n))
    if n == 0:
        return []
    out: list[tuple[float, float]] = []
    for i in range(n):
        a = (i / n) * 2.0 * math.pi + 0.35
        out.append((math.cos(a) * radius, math.sin(a) * radius))
    return out


def same_path_is_shared(stats: dict) -> bool:
    """True when N clones of one VRM did not duplicate GPU vertex buffers.

    Invariant: ``vertex_buffers * live == primitives`` and
    ``shared_instances == live - unique_paths``.
    """
    live = int(stats.get("live") or 0)
    paths = int(stats.get("unique_paths") or 0)
    shared = int(stats.get("shared_instances") or 0)
    prims = int(stats.get("primitives") or 0)
    vbufs = int(stats.get("vertex_buffers") or 0)
    if live < 2 or paths != 1 or prims <= 0 or vbufs <= 0:
        return False
    return shared == live - 1 and vbufs * live == prims


def gpu_uniques_do_not_scale(one: dict, many: dict) -> list[str]:
    """Compare 1-avatar vs N-avatar stats. Empty list means sharing held."""
    fails: list[str] = []
    if int(many.get("live") or 0) <= int(one.get("live") or 0):
        fails.append("many.live must exceed one.live")
    for key in ("vertex_buffers", "textures"):
        if int(one.get(key) or 0) != int(many.get(key) or 0):
            fails.append(
                f"{key} scaled {one.get(key)} → {many.get(key)} (must stay constant)"
            )
    return fails
