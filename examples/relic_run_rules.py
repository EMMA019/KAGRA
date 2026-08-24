"""Island Relic Run rules. No kagra import — GPU-free tests.

Collect 5 relics on the outdoor grass island within 30 seconds.
Agent-built showcase demo (see docs/agent-runs/20260824-relic-run-walk-assets/).

Kenney Mini Forest / Nature Kit sit heights are half-extents after Prop
centers the glTF (y_center = ground + half_y * scale).
"""
from __future__ import annotations

import math
from dataclasses import dataclass

# Spawn on grass near origin (overworld_height > WATER_Y).
START_XZ = (0.0, -2.5)

# Five relics on grass above water (verified vs kagra.land biome_at).
RELIC_XZ = (
    (2.5, -1.5),
    (-1.5, -3.5),
    (1.5, 3.5),
    (4.0, -3.0),
    (0.5, 2.0),
)

# (file under examples/assets/relic_run/kenney/, x, z, scale, yaw)
# Starting camera looks +Z from START_XZ — keep several of these in that frustum.
TREE_PLACEMENTS = (
    ("tree-high.glb", -1.2, -0.4, 1.90, 0.35),
    ("tree.glb", 3.2, 0.6, 1.70, 1.10),
    ("tree-high.glb", 2.8, 4.8, 2.05, -0.40),
    ("tree.glb", -1.4, 3.8, 1.80, 0.80),
    ("tree-high.glb", 4.2, 2.4, 2.00, 0.15),
    ("tree.glb", 3.5, 2.0, 1.55, 1.70),
    ("tree-high.glb", 0.8, 5.2, 1.85, -1.10),
    ("tree.glb", -3.6, 4.6, 2.00, 0.55),
    ("tree.glb", 6.0, -2.0, 1.70, 2.20),
    ("tree-high.glb", 5.2, -0.5, 1.60, -0.70),
    ("tree.glb", 4.8, 3.5, 1.75, 0.25),
    ("plant.glb", 2.0, -0.8, 2.80, 0.90),
    ("plant.glb", -0.8, 2.6, 2.40, -0.20),
    ("plant.glb", 1.8, -2.2, 2.60, 0.40),
    ("plant.glb", -0.4, 3.2, 2.50, 1.10),
)

ROCK_PLACEMENTS = (
    ("rocks-high.glb", 1.2, 0.4, 1.45, 0.20),
    ("rocks-low.glb", -1.0, 1.6, 1.60, 0.90),
    ("stones.glb", 3.8, 0.2, 1.50, -0.40),
    ("rocks-high.glb", 5.0, 1.0, 1.35, 1.30),
    ("stones.glb", -0.6, -1.2, 1.40, 0.10),
    ("rock_tallA.glb", 1.0, -1.8, 1.20, 0.50),
    ("rock_largeA.glb", 4.6, -1.6, 3.40, -0.80),
)

# Pedestal under each relic (same XZ order as RELIC_XZ).
PEDESTAL = ("stone_smallTopA.glb", 2.6, 0.0)

TREE_XZ = tuple((x, z) for _n, x, z, _s, _y in TREE_PLACEMENTS)
STONE_XZ = tuple((x, z) for _n, x, z, _s, _y in ROCK_PLACEMENTS)

# Flatten-centered half-height (Kenney Mini Forest / Nature Kit, scale=1).
GLTF_HALF_Y = {
    "tree.glb": 0.842,
    "tree-high.glb": 1.142,
    "rocks-high.glb": 0.500,
    "rocks-low.glb": 0.261,
    "stones.glb": 0.227,
    "plant.glb": 0.096,
    "rock_largeA.glb": 0.130,
    "rock_tallA.glb": 0.498,
    "stone_smallTopA.glb": 0.110,
    "mushroom_red.glb": 0.101,
}

WATER_Y = 0.0
# Matches land.biome_at sea cut: height below this is sea.
LAND_MIN_Y = WATER_Y - 0.04

PICK_REACH = 1.15
ROUND_SEC = 30.0
CAM_DISTANCE = 6.6
PLAYER_SPEED = 4.0
JUMP = 6.0
RELIC_SCALE = 0.95
RELIC_GLOW = 1.15

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


def sit_y(ground: float, half_y: float, scale: float = 1.0) -> float:
    """Prop center Y so the centered glTF rests on ``ground``."""
    return float(ground) + float(half_y) * float(scale)


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
