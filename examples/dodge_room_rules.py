"""Meteor Dodge rules. No kagra import — GPU-free tests.

VRM stands in a small arena. Meteors fall straight down from the sky at
random X/Z spots. Walk out of the way before they land. Survive — no
catching, no switches. Difficulty ramps with elapsed time (faster falls,
shorter spawn gaps).
"""
from __future__ import annotations

import math
from dataclasses import dataclass

ARENA_HALF = 4.2
PLAYER_SPEED = 3.4
PLAYER_RADIUS = 0.32

GROUND_Y = 0.0
SPAWN_Y = 6.0
HIT_Y = 0.35          # meteor is "at ground level" at/below this height
HIT_RADIUS = 0.55      # meteor radius + a little grace

BASE_FALL_SPEED = 2.6
FALL_SPEED_PER_SEC = 0.05      # ramps up over the run
BASE_SPAWN_GAP = 1.35
MIN_SPAWN_GAP = 0.45
SPAWN_GAP_HALFLIFE = 18.0      # seconds for gap to roughly halve

INVULN_SEC = 0.9
START_LIVES = 3


def clamp_arena(v: float, half: float = ARENA_HALF) -> float:
    return -half if v < -half else half if v > half else v


def wish_velocity(ax: float, az: float, speed: float = PLAYER_SPEED) -> tuple[float, float]:
    mag = math.hypot(ax, az)
    if mag < 1e-6:
        return 0.0, 0.0
    return ax / mag * speed, az / mag * speed


def facing_yaw(dx: float, dz: float, fallback: float = 0.0) -> float:
    """移動方向の yaw。真後ろ（atan2(0,+z)=0）も 0 として採用する。

    ``cond and atan2(...) or fallback`` だと 0 が falsy で向きが残る。
    """
    if dx * dx + dz * dz < 1e-8:
        return fallback
    return math.atan2(dx, dz)


def fall_speed(elapsed: float) -> float:
    return BASE_FALL_SPEED + FALL_SPEED_PER_SEC * elapsed


def spawn_gap(elapsed: float) -> float:
    decay = 0.5 ** (elapsed / SPAWN_GAP_HALFLIFE)
    gap = MIN_SPAWN_GAP + (BASE_SPAWN_GAP - MIN_SPAWN_GAP) * decay
    return max(MIN_SPAWN_GAP, gap)


@dataclass
class Meteor:
    x: float
    z: float
    y: float = SPAWN_Y
    alive: bool = True


def spawn_meteor(x: float, z: float) -> Meteor:
    return Meteor(x=clamp_arena(x), z=clamp_arena(z), y=SPAWN_Y)


def step_meteor(m: Meteor, dt: float, speed: float) -> Meteor:
    if not m.alive:
        return m
    return Meteor(x=m.x, z=m.z, y=m.y - speed * dt, alive=True)


def has_landed(m: Meteor) -> bool:
    """True once it reaches the floor, whether or not it hit the player."""
    return m.alive and m.y <= GROUND_Y


def is_hit(px: float, pz: float, m: Meteor, radius: float = HIT_RADIUS) -> bool:
    if not m.alive or m.y > HIT_Y:
        return False
    return math.hypot(px - m.x, pz - m.z) <= radius


def survival_score(elapsed: float) -> int:
    return int(elapsed * 12)
