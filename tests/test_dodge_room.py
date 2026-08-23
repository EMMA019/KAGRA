"""Dodge Room 向き。GPU 不要。"""
from __future__ import annotations

import math
import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(_ROOT / "examples"))

from dodge_room_rules import facing_yaw, wish_velocity


def test_facing_south_is_zero_not_stale():
    # S / 下 = +Z。atan2(0, +speed) は 0.0。falsy 扱いすると横向きのまま残る。
    vx, vz = wish_velocity(0.0, 1.0)
    assert facing_yaw(vx, vz, fallback=math.pi / 2) == 0.0


def test_facing_keeps_fallback_when_still():
    assert facing_yaw(0.0, 0.0, 1.2) == 1.2
    assert facing_yaw(1.0, 0.0) != 0.0
