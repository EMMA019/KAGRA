"""Switch Room rules. No kagra import — GPU-free tests.

Walk a boxed room and stand on a floor switch. Not a collect-on-a-disc.
"""
from __future__ import annotations

import math

START_XZ = (0.0, 3.4)
SWITCH_XZ = (0.0, -4.4)
SWITCH_HALF = 0.7
ARENA_HALF = 5.6
PLAYER_SPEED = 3.2
HOLD_SEC = 0.4
WALL_H = 1.8
WALL_T = 0.35

# (x, y, z, w, h, d) — y is the bottom of the AABB
BOXES = (
    (-2.1, 0.0, 0.6, 1.3, 1.15, 1.2),
    (2.2, 0.0, -0.4, 1.4, 1.0, 1.1),
    (0.15, 0.0, -1.8, 1.7, 1.35, 0.85),
    (-3.5, 0.0, -2.6, 1.05, 1.5, 1.7),
    (3.3, 0.0, -3.2, 1.15, 1.05, 1.2),
)


def walls(half: float = ARENA_HALF) -> list[tuple[float, float, float, float, float, float]]:
    """Thin boxes around the arena."""
    t = WALL_T
    h = WALL_H
    return [
        (0.0, 0.0, -half, half * 2 + t, h, t),
        (0.0, 0.0, half, half * 2 + t, h, t),
        (-half, 0.0, 0.0, t, h, half * 2),
        (half, 0.0, 0.0, t, h, half * 2),
    ]


def on_switch(
    x: float,
    z: float,
    sx: float | None = None,
    sz: float | None = None,
    half: float = SWITCH_HALF,
) -> bool:
    sx = SWITCH_XZ[0] if sx is None else sx
    sz = SWITCH_XZ[1] if sz is None else sz
    return abs(x - sx) <= half and abs(z - sz) <= half


def wish_velocity(ax: float, az: float, speed: float = PLAYER_SPEED) -> tuple[float, float]:
    mag = math.hypot(ax, az)
    if mag < 1e-6:
        return 0.0, 0.0
    return ax / mag * speed, az / mag * speed


def facing_yaw(dx: float, dz: float, fallback: float = 0.0) -> float:
    if dx * dx + dz * dz < 1e-8:
        return fallback
    return math.atan2(dx, dz)
