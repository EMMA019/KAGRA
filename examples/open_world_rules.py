"""Crest Isle rules. No kagra import — GPU-free tests.

Mario-like collectathon on a wide outdoor peninsula (not Nintendo IP).
Agent-built showcase: docs/agent-runs/20260824-open-world/
"""
from __future__ import annotations

import math
import random
from dataclasses import dataclass

HALF = 80.0
TILE = 16.0
STREAM_RADIUS = 64.0
LOD_RADIUS = 28.0
# 3-cell far tiles + period < TILE made 16 m chunks a 1-axis UV sliver
# (barcode). Keep coarser than CELLS=8, but not a 5 m triangle on a fold.
LOD_CELLS = 6
CELLS = 8
WATER_Y = 0.0
LAND_MIN_Y = WATER_Y - 0.04

START_XZ = (0.0, -8.0)
PEAK_XZ = (8.0, 52.0)

CAM_DISTANCE = 12.2
CAM_HEIGHT = 4.4
CAM_LOOK_Y = 1.25
PLAYER_SPEED = 5.6
JUMP = 7.2
FOV_DEG = 54.0
# Chase cam must stay in this band. Wall-clip / hitch lerp must not
# explode to a tiny speck or slam into the VRM skull. Max is the authored
# 3D eye distance (hypot of CAM_DISTANCE and height-look_y), not the
# horizontal CAM_DISTANCE alone.
CAM_MIN_DISTANCE = 6.0
CAM_MAX_DISTANCE = math.hypot(CAM_DISTANCE, CAM_HEIGHT - CAM_LOOK_Y)
# Player zoom ([ ] / - = / wheel). Horizontal step; 3D clamp stays on the arm.
CAM_ZOOM_STEP = 0.55

# aerial_grass_rock_diff_1k.jpg mean is brown dirt (R=0.45, G=0.38, B=0.14).
# Crest Isle only: multiply mesh_mat.base so Lambert reads as 草原.
GRASS_TINT = (0.55, 1.55, 0.70)
AERIAL_GRASS_ALBEDO = (0.446, 0.381, 0.143)
# The JPEG is a non-tiling aerial photo (1024²). Mossy speckle sits in the
# interior; UV 0 and 1 are bare earth (higher B, yellowish dirt). Engine
# sampler is ClampToEdge + Nearest. Do not map 0..1 onto a tile (that stamps
# the JPEG square). Do not Repeat this photo (dirt rim at every fold).
# Period must be significantly *larger* than TILE so one 16 m chunk maps to
# a small 2D window of the moss interior. Period 9.5 < TILE ping-ponged the
# whole moss window inside one tile; lod_cells=3 triangles that straddled a
# fold interpolated as a 1D UV sliver (barcode / 1-axis stretch). Period is
# a multiple of TILE so folds land on chunk edges, not inside a coarse
# triangle. Pad 0.28 skips the square dirt rim (~0.12 UV) but the *interior*
# is still mixed moss + brown rock. Period 48 then made each TILE a different
# biome slice of that interior (green tile glued to bald dirt). TERRAIN_UV_RECT
# is a compact meadow-green window measured on the tinted 1K (low B, no dirt
# rim / rock patch). Ping-pong maps into that rect only. Relic Run keeps the
# uncropped JPEG / default UV. Blend stays 0.
AERIAL_GRASS_DIRT_RIM = 0.12
TERRAIN_UV_PERIOD = 48.0
TERRAIN_UV_BLEND = 0.0
TERRAIN_UV_PAD = 0.28
# (u0, v0, u1, v1) into aerial_grass_rock_diff_1k.jpg after GRASS_TINT.
TERRAIN_UV_RECT = (0.535, 0.485, 0.640, 0.590)

PICK_REACH = 1.25
STAR_NEED = 6
STAR_SCALE = 1.55
# Kenney dungeon/coin.glb is a painted yellow disc (still reads plastic at
# metallic 0.85 / roughness 0.22). Crest coins are gold PBR spheres.
GOLD_METALLIC = 1.0
GOLD_ROUGHNESS = 0.12
COIN_SCALE = 0.48
COIN_HOVER = 0.32
COIN_GLOW = 0.55
SPHERE_HALF_Y = 0.5

_START_FACE = 0.0  # body +Z; camera behind looks at grass / sea / mountains


def _pingpong01(t: float) -> float:
    """Fold into 0..1. Same as ``kagra.gamekit`` (Crest UVs stay GPU-free)."""
    t = float(t)
    n = math.floor(t)
    f = t - n
    if int(n) % 2:
        return 1.0 - f
    return f


def terrain_uv(x: float, z: float) -> tuple[float, float]:
    """Crest Isle world XZ → JPEG UV. Ping-pong into ``TERRAIN_UV_RECT``."""
    u = _pingpong01(float(x) / TERRAIN_UV_PERIOD)
    v = _pingpong01(float(z) / TERRAIN_UV_PERIOD)
    u0, v0, u1, v1 = TERRAIN_UV_RECT
    return u0 + u * (u1 - u0), v0 + v * (v1 - v0)


# Flatten-centered half-height (Kenney Mini Forest / Nature / Town / Castle / Dungeon).
GLTF_HALF_Y = {
    "forest/tree.glb": 0.842,
    "forest/tree-high.glb": 1.142,
    "forest/plant.glb": 0.096,
    "forest/fence.glb": 0.200,
    "forest/flag.glb": 0.375,
    "forest/patch-grass.glb": 0.086,
    "forest/patch-dirt.glb": 0.050,
    "forest/bridge.glb": 0.236,
    "forest/tent.glb": 0.500,
    "forest/rocks-high.glb": 0.500,
    "forest/rocks-low.glb": 0.261,
    "forest/rocks-ramp.glb": 0.250,
    "forest/stones.glb": 0.227,
    "forest/platform.glb": 0.235,
    "forest/ladder.glb": 0.500,
    "nature/flower_redA.glb": 0.146,
    "nature/flower_redB.glb": 0.129,
    "nature/flower_yellowA.glb": 0.096,
    "nature/flower_purpleA.glb": 0.121,
    "nature/grass.glb": 0.127,
    "nature/grass_large.glb": 0.127,
    "nature/grass_leafs.glb": 0.071,
    "nature/plant_bush.glb": 0.122,
    "nature/plant_bushLarge.glb": 0.121,
    "nature/plant_bushDetailed.glb": 0.180,
    "nature/plant_bushSmall.glb": 0.104,
    "nature/cliff_large_rock.glb": 0.500,
    "nature/cliff_rock.glb": 0.500,
    "nature/cliff_corner_rock.glb": 0.500,
    "nature/cliff_half_rock.glb": 0.250,
    "nature/cliff_block_rock.glb": 0.500,
    "nature/tree_pineTallA.glb": 0.765,
    "nature/tree_pineTallB.glb": 0.967,
    "nature/tree_oak.glb": 0.613,
    "nature/tree_palm.glb": 0.757,
    "nature/tree_palmDetailedTall.glb": 0.712,
    "nature/tree_default.glb": 0.854,
    "nature/tree_tall.glb": 0.844,
    "nature/fence_simple.glb": 0.173,
    "nature/fence_simpleHigh.glb": 0.173,
    "nature/fence_planks.glb": 0.173,
    "nature/rock_largeA.glb": 0.130,
    "nature/rock_largeB.glb": 0.215,
    "nature/rock_tallA.glb": 0.498,
    "nature/rock_tallB.glb": 0.442,
    "nature/rock_smallA.glb": 0.096,
    "nature/log.glb": 0.087,
    "nature/stump_round.glb": 0.103,
    "nature/path_stone.glb": 0.025,
    "nature/stone_smallTopA.glb": 0.110,
    "town/banner-red.glb": 0.420,
    "town/banner-green.glb": 0.420,
    "town/wall-broken.glb": 0.500,
    "town/wall-arch.glb": 0.500,
    "town/stairs-stone.glb": 0.500,
    "town/pillar-stone.glb": 0.500,
    "town/tree-high-round.glb": 1.375,
    "town/tree-crooked.glb": 1.206,
    "town/rock-large.glb": 0.581,
    "town/rock-wide.glb": 0.510,
    "town/fence.glb": 0.190,
    "town/wall.glb": 0.500,
    "castle/flag.glb": 0.433,
    "castle/flag-wide.glb": 0.433,
    "castle/flag-pennant.glb": 0.433,
    "castle/tree-large.glb": 0.925,
    "castle/tree-small.glb": 0.675,
    "castle/rocks-large.glb": 0.250,
    "castle/wall-half.glb": 0.655,
    "dungeon/coin.glb": 0.208,
    "dungeon/chest.glb": 0.225,
    "dungeon/banner.glb": 0.323,
    "dungeon/column.glb": 0.550,
}


# Eight crests. Last is the peak flag. Opening camera looks +Z from START_XZ.
STAR_XZ = (
    (3.2, -0.6),
    (-7.2, 7.5),
    (10.4, 5.8),
    (1.4, 17.5),
    (-8.4, 19.2),
    (13.6, 15.8),
    (8.0, 34.0),
    PEAK_XZ,
)

STAR_MODELS = (
    "forest/flag.glb",
    "forest/flag.glb",
    "town/banner-red.glb",
    "dungeon/chest.glb",
    "town/banner-green.glb",
    "castle/flag-pennant.glb",
    "castle/flag.glb",
    "castle/flag-wide.glb",
)

STAR_SCALES = (1.7, 1.6, 1.8, 1.9, 1.8, 1.7, 1.85, 2.4)


def _coin_path() -> tuple[tuple[float, float], ...]:
    pts: list[tuple[float, float]] = []
    for i, z in enumerate(range(-5, 30, 3)):
        pts.append((1.1 + (i % 3 - 1) * 1.35, float(z)))
    for i, z in enumerate(range(1, 20, 3)):
        pts.append((-8.6 + (i % 2) * 0.7, float(z)))
    for i, z in enumerate(range(8, 22, 4)):
        pts.append((7.4 + (i % 2) * 0.9, float(z)))
    pts.append((8.0, 28.0))
    pts.append((8.0, 40.0))
    pts.append((-4.0, 12.0))
    pts.append((5.0, -3.5))
    # drop anything on top of spawn / a crest
    out = []
    for x, z in pts:
        if math.hypot(x - START_XZ[0], z - START_XZ[1]) < 1.6:
            continue
        if any(math.hypot(x - sx, z - sz) < 1.4 for sx, sz in STAR_XZ):
            continue
        out.append((x, z))
    return tuple(out)


COIN_XZ = _coin_path()


def _j(i: int, amp: float = 0.35) -> float:
    return ((i * 37 + 11) % 100) / 100.0 * amp * 2.0 - amp


def _vista_props() -> tuple[tuple[str, float, float, float, float, bool], ...]:
    """Dense Kenney in the first chase-cam shot (look +Z, sea to -X)."""
    items: list[tuple[str, float, float, float, float, bool]] = []

    trees = (
        ("forest/tree-high.glb", -3.4, 1.2, 2.15, 0.40, True),
        ("forest/tree.glb", 4.1, 2.0, 1.85, 1.10, True),
        ("forest/tree-high.glb", 2.6, 6.4, 2.25, -0.35, True),
        ("forest/tree.glb", -5.2, 4.8, 1.95, 0.80, True),
        ("forest/tree-high.glb", 6.8, 3.1, 2.05, 0.20, True),
        ("forest/tree.glb", -2.0, 8.6, 1.75, -0.90, True),
        ("town/tree-high-round.glb", 8.8, 9.4, 1.55, 0.50, True),
        ("town/tree-crooked.glb", -6.6, 11.2, 1.45, 1.30, True),
        ("forest/tree-high.glb", 3.8, 12.5, 2.10, -0.20, True),
        ("forest/tree.glb", 11.2, 2.4, 1.80, 2.00, True),
        ("forest/tree-high.glb", -1.2, 14.8, 2.00, 0.70, True),
        ("castle/tree-large.glb", 5.4, 16.2, 1.70, -0.55, True),
        ("forest/tree.glb", 9.6, 13.0, 1.90, 0.15, True),
        ("nature/tree_pineTallA.glb", 12.4, 18.5, 3.40, 0.40, True),
        ("nature/tree_pineTallB.glb", 15.0, 21.0, 3.80, -0.30, True),
        ("nature/tree_pineTallA.glb", 18.2, 24.6, 4.10, 0.80, True),
        ("nature/tree_pineTallB.glb", 6.2, 22.4, 3.60, 1.10, True),
        ("nature/tree_oak.glb", -4.8, 9.8, 2.80, 0.25, True),
        ("nature/tree_palmDetailedTall.glb", -11.2, 3.4, 2.60, 0.60, True),
        ("nature/tree_palm.glb", -10.4, 8.2, 2.40, -0.40, True),
        ("nature/tree_palmDetailedTall.glb", -9.6, 13.6, 2.70, 1.20, True),
        ("nature/tree_default.glb", 1.6, 10.2, 2.20, 0.10, True),
        ("castle/tree-small.glb", -7.8, 2.2, 1.80, 0.90, True),
        ("forest/tree.glb", 7.2, 7.8, 1.70, -1.40, True),
        ("forest/tree-high.glb", -3.8, 17.4, 2.05, 0.45, True),
        # Extra pines / oak / default in the opening cone (not one cloned tree).
        ("nature/tree_pineTallB.glb", -8.4, 6.8, 3.20, 0.55, True),
        ("nature/tree_pineTallA.glb", 8.4, 5.6, 3.10, -0.70, True),
        ("nature/tree_tall.glb", -1.0, 5.4, 2.40, 0.35, True),
        ("nature/tree_oak.glb", 10.2, 16.8, 2.90, 1.05, True),
        ("nature/tree_pineTallB.glb", -5.6, 20.2, 3.50, -0.90, True),
        ("nature/tree_default.glb", 4.4, 19.0, 2.30, 0.60, True),
        ("nature/tree_pineTallA.glb", 13.8, 8.8, 3.30, 1.40, True),
        ("nature/tree_tall.glb", 0.8, 22.8, 2.55, -0.20, True),
    )
    items.extend(trees)

    rocks = (
        ("forest/rocks-high.glb", 1.4, 0.8, 1.55, 0.20, True),
        ("forest/rocks-low.glb", -1.6, 3.2, 1.65, 0.90, True),
        ("forest/stones.glb", 5.0, 1.1, 1.50, -0.40, True),
        ("town/rock-large.glb", -9.2, 5.4, 1.35, 0.55, True),
        ("town/rock-wide.glb", 12.0, 7.2, 1.20, -0.80, True),
        ("nature/rock_tallA.glb", 2.2, 4.6, 1.40, 0.30, True),
        ("nature/rock_largeB.glb", -4.4, 6.2, 3.20, 1.10, True),
        ("castle/rocks-large.glb", 9.0, 19.4, 2.40, 0.15, True),
        ("forest/rocks-ramp.glb", 4.6, 9.0, 1.45, 0.70, True),
        ("nature/rock_tallB.glb", -6.0, 15.4, 1.50, -0.25, True),
        ("nature/log.glb", 0.6, 5.5, 2.40, 1.20, True),
        ("nature/stump_round.glb", 3.0, 3.4, 2.20, 0.40, True),
        ("nature/rock_smallA.glb", -2.8, 1.4, 1.80, 0.85, True),
        ("nature/rock_largeA.glb", 7.6, 14.2, 2.40, -0.50, True),
        ("nature/stone_smallTopA.glb", 1.0, 7.2, 2.00, 0.15, False),
        ("nature/rock_smallA.glb", 5.8, 4.0, 1.70, 1.35, True),
        ("nature/stump_round.glb", -3.2, 12.6, 2.10, -0.70, True),
        ("nature/log.glb", 8.8, 18.0, 2.20, 0.40, True),
    )
    items.extend(rocks)

    fences = (
        ("forest/fence.glb", -1.8, -1.2, 1.35, 0.05, True),
        ("forest/fence.glb", 0.4, -1.05, 1.35, 0.08, True),
        ("forest/fence.glb", 2.6, -0.9, 1.35, 0.10, True),
        ("nature/fence_simple.glb", 6.2, 4.4, 1.80, 1.20, True),
        ("nature/fence_simpleHigh.glb", 6.4, 6.2, 1.80, 1.25, True),
        ("nature/fence_planks.glb", -4.0, 2.4, 1.70, 0.40, True),
        ("town/fence.glb", 8.2, 11.6, 1.50, 0.20, True),
    )
    items.extend(fences)

    cliffs = (
        ("nature/cliff_large_rock.glb", -14.2, 2.0, 8.5, 0.15, True),
        ("nature/cliff_rock.glb", -13.6, 6.5, 7.2, 0.40, True),
        ("nature/cliff_large_rock.glb", -14.8, 11.0, 9.0, -0.20, True),
        ("nature/cliff_half_rock.glb", -13.2, 15.4, 7.8, 0.55, True),
        ("nature/cliff_corner_rock.glb", -14.0, 19.8, 8.2, 0.10, True),
        ("nature/cliff_block_rock.glb", -13.8, -1.5, 6.4, 0.80, True),
        ("nature/cliff_large_rock.glb", -15.4, 24.0, 10.0, -0.35, True),
        ("nature/cliff_rock.glb", 20.5, 26.0, 8.0, 1.10, True),
    )
    items.extend(cliffs)

    ruins = (
        ("town/wall-broken.glb", 11.4, 11.8, 1.80, 0.30, True),
        ("town/wall-arch.glb", 12.6, 12.6, 1.90, 0.30, True),
        ("town/pillar-stone.glb", 10.6, 12.2, 1.70, 0.0, True),
        ("town/stairs-stone.glb", 7.2, 27.4, 1.60, 0.0, True),
        ("dungeon/column.glb", 9.4, 33.2, 1.80, 0.20, True),
        ("castle/wall-half.glb", 10.8, 36.4, 1.50, 0.05, True),
        ("forest/tent.glb", 4.8, 8.4, 1.15, -0.60, True),
        ("forest/bridge.glb", -8.8, 10.6, 1.20, 1.40, True),
        ("dungeon/banner.glb", 11.0, 13.4, 1.60, 0.30, True),
    )
    items.extend(ruins)

    patches = (
        ("forest/patch-grass.glb", 0.8, -4.2, 2.20, 0.2, False),
        ("forest/patch-grass.glb", -2.4, -3.0, 2.00, 1.1, False),
        ("forest/patch-grass.glb", 3.4, -2.4, 2.10, -0.4, False),
        ("forest/patch-dirt.glb", 1.6, 2.8, 1.80, 0.6, False),
        ("forest/patch-grass.glb", -5.0, 7.0, 2.30, 0.9, False),
        ("forest/patch-grass.glb", 6.0, 10.4, 2.00, -0.2, False),
        ("forest/patch-grass.glb", -1.2, -5.4, 2.40, 0.55, False),
        ("forest/patch-grass.glb", 2.2, -5.0, 2.20, -0.30, False),
        ("forest/patch-grass.glb", -3.6, 0.6, 2.10, 0.80, False),
        ("forest/patch-grass.glb", 4.8, 6.2, 2.25, 1.05, False),
        ("nature/path_stone.glb", 0.4, 1.6, 1.70, 0.05, False),
        ("nature/path_stone.glb", 0.6, 4.6, 1.70, 0.08, False),
    )
    items.extend(patches)

    flowers = (
        "nature/flower_redA.glb",
        "nature/flower_yellowA.glb",
        "nature/flower_purpleA.glb",
        "nature/flower_redB.glb",
        "nature/grass.glb",
        "nature/grass_large.glb",
        "nature/grass_leafs.glb",
        "forest/plant.glb",
        "nature/plant_bush.glb",
        "nature/plant_bushSmall.glb",
        "nature/plant_bushDetailed.glb",
        "nature/plant_bushLarge.glb",
    )
    n = 0
    # 1 m checkerboard + jitter: denser meadow than the old 2 m lattice,
    # without one cloned stamp.
    for z in range(-6, 22, 1):
        for x in range(-9, 14, 1):
            if (x + z) % 2 == 0:
                continue
            if math.hypot(x - START_XZ[0], z - START_XZ[1]) < 1.8:
                continue
            name = flowers[n % len(flowers)]
            scale = 2.15 + (n % 6) * 0.12
            items.append(
                (
                    name,
                    float(x) + _j(n, 0.45),
                    float(z) + _j(n + 4, 0.35),
                    scale,
                    (n * 0.47) % 6.28,
                    False,
                )
            )
            n += 1
    return tuple(items)


VISTA_PROPS = _vista_props()


def chunk_decor(
    ix: int, iz: int, *, tile: float = TILE,
) -> list[tuple[str, float, float, float, float, bool]]:
    """Kenney when a far grass tile streams in. Spawn tiles stay authored.

    Variation (pines / oak / grass / flowers / bushes / rocks), not one clone.
    """
    if abs(int(ix)) <= 1 and -1 <= int(iz) <= 1:
        return []
    seed = abs(int(ix) * 17 + int(iz) * 31)
    if seed % 7 == 0:
        return []
    t = float(tile)
    cx = (int(ix) + 0.5) * t
    cz = (int(iz) + 0.5) * t
    trees = (
        "forest/tree.glb",
        "forest/tree-high.glb",
        "nature/tree_pineTallA.glb",
        "nature/tree_pineTallB.glb",
        "nature/tree_oak.glb",
        "nature/tree_tall.glb",
        "nature/tree_default.glb",
        "town/tree-crooked.glb",
        "castle/tree-small.glb",
    )
    ground = (
        "nature/flower_redA.glb",
        "nature/flower_redB.glb",
        "nature/flower_yellowA.glb",
        "nature/flower_purpleA.glb",
        "nature/grass.glb",
        "nature/grass_large.glb",
        "nature/grass_leafs.glb",
        "forest/plant.glb",
        "nature/plant_bush.glb",
        "nature/plant_bushSmall.glb",
        "nature/plant_bushDetailed.glb",
        "nature/plant_bushLarge.glb",
    )
    rocks = (
        "forest/rocks-low.glb",
        "nature/rock_smallA.glb",
        "nature/rock_largeA.glb",
        "nature/stump_round.glb",
        "nature/log.glb",
    )
    out: list[tuple[str, float, float, float, float, bool]] = []
    n_tree = 2 + seed % 3
    for i in range(n_tree):
        dx = ((seed + i * 13) % 11 - 5) * 0.55
        dz = ((seed + i * 7) % 11 - 5) * 0.55
        name = trees[(seed + i) % len(trees)]
        scale = 1.7 + ((seed + i) % 5) * 0.18
        pine = "pine" in name
        out.append(
            (name, cx + dx, cz + dz, scale * (1.7 if pine else 1.0), (i * 0.9), True)
        )
    n_ground = 6 + seed % 4
    for i in range(n_ground):
        dx = ((seed + i * 19) % 13 - 6) * 0.5
        dz = ((seed + i * 23) % 13 - 6) * 0.5
        name = ground[(seed + i) % len(ground)]
        out.append(
            (name, cx + dx, cz + dz, 2.0 + (i % 4) * 0.18, i * 0.6, False)
        )
    if seed % 3 == 1:
        rname = rocks[(seed // 3) % len(rocks)]
        out.append(
            (rname, cx + 1.2, cz - 0.8, 1.4 + (seed % 5) * 0.08, 0.3, True)
        )
    return out


@dataclass
class Pickup:
    x: float
    z: float
    live: bool = True
    phase: float = 0.0
    kind: str = "coin"


@dataclass
class Spark:
    """One CPU billboard spark. Fade is 1 at spawn, 0 at expire."""

    x: float
    y: float
    z: float
    vx: float
    vy: float
    vz: float
    life: float = 0.0
    max_life: float = 0.55
    size: float = 0.22

    @property
    def fade(self) -> float:
        if self.max_life <= 1e-9:
            return 0.0
        return max(0.0, 1.0 - self.life / self.max_life)

    @property
    def draw_size(self) -> float:
        return self.size * (0.28 + 0.72 * self.fade)


class SparkBurst:
    """Tiny CPU particle list. Draw with ``draw_billboard_instances``."""

    def __init__(self) -> None:
        self.sparks: list[Spark] = []

    def burst(
        self,
        x: float,
        y: float,
        z: float,
        *,
        count: int = 14,
        speed: float = 2.8,
        life: float = 0.58,
        size: float = 0.20,
        seed: int | None = None,
    ) -> int:
        rng = random.Random(seed)
        n = max(1, int(count))
        for i in range(n):
            a = (i / n) * math.tau + rng.uniform(-0.35, 0.35)
            up = rng.uniform(0.4, 1.05)
            spd = speed * rng.uniform(0.5, 1.15)
            self.sparks.append(
                Spark(
                    x=float(x),
                    y=float(y),
                    z=float(z),
                    vx=math.cos(a) * spd * 0.78,
                    vy=spd * up,
                    vz=math.sin(a) * spd * 0.78,
                    life=0.0,
                    max_life=life * rng.uniform(0.72, 1.12),
                    size=size * rng.uniform(0.7, 1.2),
                )
            )
        return n

    def update(self, dt: float, *, gravity: float = 3.8) -> None:
        dt = float(dt)
        live: list[Spark] = []
        for s in self.sparks:
            s.life += dt
            if s.life >= s.max_life:
                continue
            s.vy -= gravity * dt
            s.x += s.vx * dt
            s.y += s.vy * dt
            s.z += s.vz * dt
            live.append(s)
        self.sparks = live

    def items(self) -> list[tuple[float, float, float, float]]:
        """``(x, y, z, size)`` for ``draw_billboard_instances``."""
        out: list[tuple[float, float, float, float]] = []
        for s in self.sparks:
            if s.fade <= 1e-4:
                continue
            sz = s.draw_size
            if sz < 1e-4:
                continue
            out.append((s.x, s.y, s.z, sz))
        return out


def start_face() -> float:
    return float(_START_FACE)


def hero_theta(face: float) -> float:
    return float(face) + math.pi


def sit_y(ground: float, half_y: float, scale: float = 1.0) -> float:
    return float(ground) + float(half_y) * float(scale)


def spawn_stars() -> list[Pickup]:
    return [
        Pickup(x=float(x), z=float(z), live=True, phase=i * 0.6, kind="star")
        for i, (x, z) in enumerate(STAR_XZ)
    ]


def spawn_coins() -> list[Pickup]:
    return [
        Pickup(x=float(x), z=float(z), live=True, phase=i * 0.35, kind="coin")
        for i, (x, z) in enumerate(COIN_XZ)
    ]


def nearest_live(px: float, pz: float, items: list[Pickup]) -> Pickup | None:
    best: Pickup | None = None
    best_d = 1e18
    for it in items:
        if not it.live:
            continue
        d = math.hypot(float(px) - it.x, float(pz) - it.z)
        if d < best_d:
            best_d = d
            best = it
    return best


def round_score(stars: int, coins: int, time_s: float, *, need: int = STAR_NEED) -> int:
    stars = max(0, min(len(STAR_XZ), int(stars)))
    coins = max(0, min(len(COIN_XZ), int(coins)))
    base = stars * 250 + coins * 10
    if stars <= 0:
        return coins * 10
    bonus = 0
    if stars >= need:
        leftover = max(0.0, 180.0 - float(time_s))
        bonus = int(leftover * 2.5)
        if (PEAK_XZ in STAR_XZ) and stars >= len(STAR_XZ):
            bonus += 400
    return base + bonus


def grade_for(score: int) -> str:
    if score >= 2200:
        return "S"
    if score >= 1600:
        return "A"
    if score >= 1100:
        return "B"
    if score >= 600:
        return "C"
    return "D"


def won(stars: int, *, need: int = STAR_NEED) -> bool:
    return int(stars) >= int(need)


if __name__ == "__main__":
    raise SystemExit(
        "これは判定ロジックだけです。窓は開きません。\n"
        "  python examples/vrm_open_world.py"
    )
