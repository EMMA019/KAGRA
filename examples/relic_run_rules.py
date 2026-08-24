"""Island Relic Run rules. No kagra import — GPU-free tests.

Collect 5 relics on the outdoor grass island within 30 seconds.
Agent-built showcase demo (see docs/agent-runs/2026-08-24-island-relic-run.md).
"""
from __future__ import annotations

import math
from dataclasses import dataclass

# Spawn on grass near origin (island_height / overworld_height > WATER_Y).
START_XZ = (0.0, -2.5)

# Five relics on grass above water (verified vs kagra.land biome_at).
RELIC_XZ = (
    (2.5, -1.5),
    (-1.5, -3.5),
    (1.5, 3.5),
    (4.0, -3.0),
    (0.5, 2.0),
)

TREE_XZ = (
    (3.5, 2.0),
    (6.0, -2.0),
    (-0.5, -4.0),
    (2.0, 5.0),
)

STONE_XZ = (
    (1.0, -1.0),
    (5.0, 1.0),
    (7.0, 0.0),
)

WATER_Y = 0.0
# Matches land.biome_at sea cut: height below this is sea.
LAND_MIN_Y = WATER_Y - 0.04

PICK_REACH = 1.15
ROUND_SEC = 30.0
CAM_DISTANCE = 6.6
PLAYER_SPEED = 4.0
JUMP = 6.0

# Face +Z toward the first relic cluster at start.
_START_FACE = 0.0


@dataclass
class Relic:
    x: float
    z: float
    live: bool = True
    phase: float = 0.0


def start_face() -> float:
    """Initial body yaw (atan2 style: 0 = +Z)."""
    return float(_START_FACE)


def hero_theta(face: float) -> float:
    """Camera yaw behind the hero (third-person follow)."""
    return float(face) + math.pi


def can_pick(
    px: float,
    pz: float,
    rx: float,
    rz: float,
    reach: float = PICK_REACH,
) -> bool:
    return math.hypot(float(px) - float(rx), float(pz) - float(rz)) <= float(reach)


def spawn_relics() -> list[Relic]:
    return [Relic(x=float(x), z=float(z), live=True, phase=i * 0.7) for i, (x, z) in enumerate(RELIC_XZ)]


def nearest_live(px: float, pz: float, relics: list[Relic]) -> Relic | None:
    best: Relic | None = None
    best_d = 1e18
    for r in relics:
        if not r.live:
            continue
        d = math.hypot(float(px) - r.x, float(pz) - r.z)
        if d < best_d:
            best_d = d
            best = r
    return best


def round_score(picked: int, time_left: float, *, total: int = 5) -> int:
    """Base 100 per relic + time bonus (capped)."""
    picked = max(0, min(int(total), int(picked)))
    base = picked * 100
    if picked <= 0:
        return 0
    bonus = int(max(0.0, float(time_left)) * 4.0)
    if picked < total:
        bonus = bonus // 2
    return base + bonus


def grade_for(score: int) -> str:
    if score >= 700:
        return "S"
    if score >= 550:
        return "A"
    if score >= 350:
        return "B"
    if score >= 150:
        return "C"
    return "D"


if __name__ == "__main__":
    raise SystemExit(
        "これは判定ロジックだけです。窓は開きません。\n"
        "  python examples/vrm_relic_run.py"
    )
