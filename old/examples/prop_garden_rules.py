"""Prop Garden rules. No kagra import — GPU-free tests.

Walk the colored props and stand near the gold sphere.
Play-surface demo (not an agent-built log).
"""
from __future__ import annotations

import math

START_XZ = (0.0, -3.8)
GOLD_XZ = (0.0, 3.6)
GOLD_REACH = 0.9
PLAYER_SPEED = 3.2

# (model, x, y, z, scale, color) — y is the Prop center
PROPS = (
    ("box", -2.2, 0.45, -1.2, (0.9, 0.9, 0.9), "orange"),
    ("box", 2.4, 0.55, 0.2, (1.1, 1.1, 1.1), "blue"),
    ("cylinder", -3.0, 0.6, 1.4, (0.7, 1.2, 0.7), "teal"),
    ("cylinder", 3.1, 0.5, -2.0, (0.6, 1.0, 0.6), "purple"),
    ("sphere", 1.7, 0.4, 1.8, 0.8, "pink"),
    ("sphere", GOLD_XZ[0], 0.5, GOLD_XZ[1], 1.0, "gold"),
)


def near_gold(x: float, z: float, reach: float = GOLD_REACH) -> bool:
    return math.hypot(x - GOLD_XZ[0], z - GOLD_XZ[1]) <= reach


def facing_yaw(dx: float, dz: float, fallback: float = 0.0) -> float:
    if dx * dx + dz * dz < 1e-8:
        return fallback
    return math.atan2(dx, dz)


if __name__ == "__main__":
    raise SystemExit(
        "これは判定ロジックだけです。窓は開きません。\n"
        "  python examples/vrm_prop_garden.py"
    )
