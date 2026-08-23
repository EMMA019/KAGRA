"""3-lane catch rules for VRM Heart Catch. No kagra import — GPU-free tests."""
from __future__ import annotations

from dataclasses import dataclass

LANES = (-1.6, 0.0, 1.6)
CATCH_Z = 0.45
MISS_Z = 1.15
SPAWN_Z = -8.0
HEART_SPEED = 5.2
ROUND_SEC = 25.0


def clamp_lane(i: int) -> int:
    return 0 if i < 0 else 2 if i > 2 else int(i)


def lane_x(i: int) -> float:
    return LANES[clamp_lane(i)]


@dataclass
class Heart:
    lane: int
    z: float
    phase: float = 0.0
    alive: bool = True


def spawn_heart(lane: int | None = None, *, rng_lane: int = 1) -> Heart:
    return Heart(lane=clamp_lane(rng_lane if lane is None else lane), z=SPAWN_Z)


def step_heart(h: Heart, dt: float, speed: float = HEART_SPEED) -> Heart:
    if not h.alive:
        return h
    return Heart(lane=h.lane, z=h.z + speed * dt, phase=h.phase + dt * 4.0, alive=True)


def is_catch(player_lane: int, h: Heart) -> bool:
    return h.alive and h.lane == clamp_lane(player_lane) and abs(h.z) <= CATCH_Z


def is_miss(h: Heart) -> bool:
    return h.alive and h.z > MISS_Z


def catch_score(combo: int) -> int:
    return 10 + min(40, max(0, combo) * 2)
