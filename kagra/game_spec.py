"""Genre-agnostic game spec: author in Python, run in Rust.

Core sections (any kit): identity + seed, ``player``, entity tables
(``obstacles``: glb + hitbox + weight), hit tuning, ``camera``, neon
``style``, HUD, ``items`` (pick-ups), reserved ``placements``. A genre kit adds one
section (``runner`` today, see ``kagra.runner_kit``). Mirrors
``kagra-shared/src/game_spec.rs``; every default is Neon Cat.

Write a game as a small override dict / JSON::

    GameSpec.from_overrides({"title": "METEOR CAT", "obstacles": ["METEOR"]})

Obstacles may be catalog names (``CATALOG``), ``{"name": .., "weight": ..}``
or full rows; a full row's hitbox is measured from its GLB.
"""
from __future__ import annotations

import json
import struct
import typing
from dataclasses import asdict, dataclass, field, fields, is_dataclass
from pathlib import Path

from kagra.runner_kit import BEHAVIORS, RunnerKit, Section

ROOT = Path(__file__).resolve().parents[1]
KINDS = ("runner",)
#: Closed item vocabulary. A new effect needs engine code, so specs can't add one.
ITEM_EFFECTS = ("magnet", "boost", "superjump", "double", "meow")
ITEM_ICONS = ("horseshoe", "chevron", "arrow", "two_x", "heart")
COIN_COLOR = (255, 214, 90)
ITEM_COLORS = {
    "magnet": [255, 0, 128],
    "boost": [255, 110, 0],
    "superjump": [100, 255, 60],
    "double": [255, 240, 0],
    "meow": [255, 200, 235],
}
ITEM_DEFAULT_ICON = {"magnet": "horseshoe", "boost": "chevron", "superjump": "arrow", "double": "two_x", "meow": "heart"}
MISCHIEF_KINDS = ("vase", "bottle", "can", "yarn")
PERCH_KINDS = ("ac_unit", "pipe", "sign")
PERCH_HEIGHT = {"ac_unit": 0.6, "pipe": 1.1, "sign": 1.6}
PART_ROLES = ("solid", "ride", "ramp", "decor")

_K = "assets/models/kenney"
#: Bundled CC0 entities (hitboxes measured from the GLBs).
CATALOG: dict[str, dict] = {
    "CRATE": {"name": "CRATE", "glb": f"{_K}/platformer-kit/crate.glb", "scale": [1.15] * 3,
              "mesh_min": [-0.25, 0.0, -0.25], "mesh_max": [0.25, 0.5, 0.25]},
    "BARREL": {"name": "BARREL", "glb": f"{_K}/platformer-kit/barrel.glb", "scale": [1.2] * 3,
               "mesh_min": [-0.258813, 0.0, -0.258813], "mesh_max": [0.258813, 0.47619, 0.258813]},
    "BARRIER": {"name": "BARRIER", "glb": f"{_K}/racing-kit/barrierRed.glb",
                "scale": [4.4, 3.8, 3.4], "mesh_min": [0.0, 0.0, -0.123],
                "mesh_max": [0.25, 0.1312, 0.0]},
    # Subway-style kinds (Kenney Space Kit). Ramps, lintels and wall sides are parts.
    "GENERATOR": {"name": "GENERATOR", "glb": f"{_K}/space-kit/machine_generatorLarge.glb",
                  "scale": [1.5, 3.0, 1.3], "behavior": "solid"},
    # A low-hanging sign on two posts: a clear gap under it.
    "OVERPASS": {"name": "OVERPASS", "glb": "", "behavior": "roll_under",
                 "parts": [{"glb": f"{_K}/space-kit/structure_closed.glb", "scale": [4.7, 1.1, 0.25], "offset": [0.0, 0.5, 0.0]},
                           {"glb": f"{_K}/space-kit/structure_closed.glb", "scale": [0.25, 1.6, 0.25], "offset": [2.5, 0.0, 0.0],
                            "role": "decor"},
                           {"glb": f"{_K}/space-kit/structure_closed.glb", "scale": [0.25, 1.6, 0.25], "offset": [-2.5, 0.0, 0.0],
                            "role": "decor"}]},
    "TRAIN": {"name": "TRAIN", "glb": f"{_K}/space-kit/monorail_trainBox.glb",
              "scale": [2.4, 1.0, 4.4], "behavior": "rideable",
              "parts": [{"glb": f"{_K}/space-kit/terrain_ramp.glb", "scale": [1.2, 1.552, 4.0],
                         "offset": [0.0, 0.0, -2.2], "role": "ramp", "run": 2.0}]},
    # One lane wide, half a metre high, lit from inside.
    "WALL": {"name": "WALL", "glb": "", "behavior": "cat_gap",
             "parts": [{"glb": f"{_K}/space-kit/structure_closed.glb", "scale": [2.2, 1.6, 0.8], "offset": [1.7, 0.0, 0.0]},
                       {"glb": f"{_K}/space-kit/structure_closed.glb", "scale": [2.2, 1.6, 0.8], "offset": [-1.7, 0.0, 0.0]},
                       {"glb": f"{_K}/space-kit/structure_closed.glb", "scale": [1.2, 1.1, 0.8], "offset": [0.0, 0.5, 0.0]},
                       {"glb": f"{_K}/space-kit/structure_closed.glb", "scale": [1.2, 0.5, 0.06], "offset": [0.0, 0.0, 0.0],
                        "role": "decor", "glow": 1.8}]},
    "METEOR": {"name": "METEOR", "glb": f"{_K}/space-kit/meteor.glb", "scale": [1.1] * 3},
    "CRYSTALS": {"name": "CRYSTALS", "glb": f"{_K}/space-kit/rock_crystals.glb",
                 "scale": [1.3] * 3},
    "ROCK": {"name": "ROCK", "glb": f"{_K}/space-kit/rock_largeA.glb", "scale": [1.1] * 3},
    "DISH": {"name": "DISH", "glb": f"{_K}/space-kit/satelliteDish.glb", "scale": [1.0] * 3},
    "TURRET": {"name": "TURRET", "glb": f"{_K}/space-kit/turret_single.glb", "scale": [0.9] * 3},
}


def measure_glb(path: str | Path) -> tuple[list[float], list[float]]:
    """Model-space AABB of a .glb scene (node TRS applied), rounded to 1e-6."""
    data = Path(path).read_bytes()
    size = struct.unpack_from("<I", data, 12)[0]
    gltf = json.loads(data[20 : 20 + size])
    lo, hi = [float("inf")] * 3, [float("-inf")] * 3

    def local(node: dict) -> list[list[float]]:
        t = node.get("translation", [0, 0, 0])
        s = node.get("scale", [1, 1, 1])
        x, y, z, w = node.get("rotation", [0, 0, 0, 1])
        r = [[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
             [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
             [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]]
        return [[r[i][j] * s[j] for j in range(3)] + [t[i]] for i in range(3)]

    def mul(a: list, b: list) -> list:
        return [[sum(a[i][k] * b[k][j] for k in range(3)) + (a[i][3] if j == 3 else 0)
                 for j in range(4)] for i in range(3)]

    def walk(index: int, m: list) -> None:
        node = gltf["nodes"][index]
        m = mul(m, local(node))
        for prim in gltf["meshes"][node["mesh"]]["primitives"] if "mesh" in node else []:
            acc = gltf["accessors"][prim["attributes"]["POSITION"]]
            for cx in (acc["min"][0], acc["max"][0]):
                for cy in (acc["min"][1], acc["max"][1]):
                    for cz in (acc["min"][2], acc["max"][2]):
                        for k in range(3):
                            v = m[k][0] * cx + m[k][1] * cy + m[k][2] * cz + m[k][3]
                            lo[k], hi[k] = min(lo[k], v), max(hi[k], v)
        for child in node.get("children", []):
            walk(child, m)

    for root in gltf["scenes"][gltf.get("scene", 0)]["nodes"]:
        walk(root, [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0]])
    return [round(v, 6) + 0.0 for v in lo], [round(v, 6) + 0.0 for v in hi]


@dataclass
class LivesSpec:
    """Nine lives: a hit costs one and the cat is safe (blinking) for
    ``invuln_secs``. ``count`` 1 is the old one-hit rule."""

    count: int = 3
    invuln_secs: float = 1.6
    blink_hz: float = 10.0

    def __post_init__(self) -> None:
        if not 1 <= self.count <= 9:
            raise ValueError("lives.count must be 1..=9")
        if not 0.0 <= self.invuln_secs <= 10.0:
            raise ValueError("lives.invuln_secs must be 0..=10")
        if not 0.0 <= self.blink_hz <= 30.0:
            raise ValueError("lives.blink_hz must be 0..=30")


@dataclass
class RimSpec:
    """Two coloured point lights just outside the cat's flanks (linear RGB)."""

    left: list[float] = field(default_factory=lambda: [1.0, 0.25, 0.75])
    right: list[float] = field(default_factory=lambda: [0.3, 0.9, 1.0])
    intensity: float = 3.6
    radius: float = 0.55
    side: float = 0.5
    height: float = 0.5


@dataclass
class EyesSpec:
    """Glow marks: eyes on the face, ear glints, a collar and a tail tip (the last three read from behind); offsets in model units."""

    enabled: bool = True
    color: list[float] = field(default_factory=lambda: [0.35, 1.0, 0.95])
    glow: float = 2.4
    size: float = 0.13
    spread: float = 0.24
    height: float = 0.92
    forward: float = 0.64
    ears: bool = True
    ear_x: float = 0.4
    ear_y: float = 1.5
    ear_z: float = 0.05
    collar: bool = False
    collar_color: list[float] = field(default_factory=lambda: [1.0, 0.25, 0.75])
    collar_y: float = 0.95
    tail_tip: bool = True
    tail_color: list[float] = field(default_factory=lambda: [1.0, 0.25, 0.75])
    tail_x: float = 0.0
    tail_y: float = 1.62
    tail_z: float = -0.6


@dataclass
class PlayerSpec:
    glb: str = "assets/models/kenney/cube-pets/animal-cat.glb"
    scale: float = 0.50
    foot: float = 0.30
    tint: list[int] = field(default_factory=lambda: [226, 206, 255])
    rim: RimSpec = field(default_factory=lambda: RimSpec())
    eyes: EyesSpec = field(default_factory=lambda: EyesSpec())


@dataclass
class Part:
    """One mesh of a multi-mesh obstacle (ramp, lintel, wall side).

    ``offset``: mesh centre in x/z relative to the anchor, y = height of the
    mesh's lowest point. ``hit_y``: model-space y range that is solid (a roof
    slab of an open mesh); ``run``: a ramp's horizontal incline length.
    """

    glb: str
    scale: list[float] = field(default_factory=lambda: [1.0, 1.0, 1.0])
    mesh_min: list[float] | None = None
    mesh_max: list[float] | None = None
    offset: list[float] = field(default_factory=lambda: [0.0, 0.0, 0.0])
    role: str = "solid"  # solid | ride | ramp | decor
    hit_y: list[float] | None = None
    run: float = 0.0
    glow: float = 0.0  # emissive strength of a decor part that should shine

    def __post_init__(self) -> None:
        if self.role not in PART_ROLES:
            raise ValueError(f"part role {self.role!r} not in {PART_ROLES}")
        if self.mesh_min is None or self.mesh_max is None:
            self.mesh_min, self.mesh_max = measure_glb(ROOT / self.glb)


@dataclass
class EntityKind:
    """One entity table row. ``mesh_min`` / ``mesh_max`` (the hitbox) are
    measured from the GLB when omitted. ``behavior`` says what the player
    must do; ``parts`` are extra meshes (an empty ``glb`` = parts only)."""

    name: str
    glb: str
    scale: list[float] = field(default_factory=lambda: [1.0, 1.0, 1.0])
    mesh_min: list[float] | None = None
    mesh_max: list[float] | None = None
    weight: int = 1
    behavior: str = "jump_over"
    parts: list[Part] = field(default_factory=list)
    hit_y: list[float] | None = None  # model-space solid y range of the base mesh
    color: list[int] | None = None  # neon edge override; default is style.codes[behavior]

    def __post_init__(self) -> None:
        if isinstance(self.scale, (int, float)):
            self.scale = [float(self.scale)] * 3
        if self.behavior not in BEHAVIORS:
            raise ValueError(f"behavior {self.behavior!r} not in {BEHAVIORS}")
        self.parts = [Part(**p) if isinstance(p, dict) else p for p in self.parts]
        if self.mesh_min is None or self.mesh_max is None:
            if self.glb:
                self.mesh_min, self.mesh_max = measure_glb(ROOT / self.glb)
            else:
                self.mesh_min, self.mesh_max = [-0.5, 0.0, -0.5], [0.5, 1.0, 0.5]


@dataclass
class CameraSpec:
    chase_back: float = 5.9
    chase_lift: float = 2.35
    chase_look: float = 13.0
    chase_look_y: float = 1.12
    fov: float = 50.0
    fov_fast: float = 58.0
    roll: float = 0.07
    classic_back: float = 7.4
    classic_lift: float = 1.72
    classic_fov: float = 50.0


@dataclass
class Style:
    sky: list[int] = field(default_factory=lambda: [4, 2, 12, 255])
    edge_a: list[int] = field(default_factory=lambda: [40, 255, 255, 255])
    edge_b: list[int] = field(default_factory=lambda: [255, 42, 180, 255])
    fill_a: list[float] = field(default_factory=lambda: [0.10, 0.55, 0.65])
    fill_b: list[float] = field(default_factory=lambda: [0.60, 0.10, 0.42])
    fill_emissive: float = 0.8
    obstacle_color: list[int] = field(default_factory=lambda: [36, 26, 54])
    # One neon colour per thing the player has to do. ``edge_a/b`` and
    # ``fill_a/b`` are older alternating tints that obstacles no longer use.
    codes: dict[str, list[int]] = field(
        default_factory=lambda: {
            "jump_over": [255, 170, 40],  # low: hop it
            "roll_under": [255, 48, 186],  # high / hanging: roll under
            "solid": [255, 60, 70],  # a wall: go round
            "cat_gap": [176, 110, 255],  # low opening in a wall: roll through
            "rideable": [60, 150, 255],  # a roof: ramp up and run on it
            "pass_through": [150, 255, 225],  # gates and arches: friendly
        }
    )

    def __post_init__(self) -> None:
        # Overrides may name only the codes they change.
        base = Style.__dataclass_fields__["codes"].default_factory()
        unknown = set(self.codes) - set(base)
        if unknown:
            raise ValueError(f"style.codes: unknown {sorted(unknown)}")
        self.codes = {**base, **self.codes}


@dataclass
class ItemSpec:
    """A pick-up. ``duration`` 0 = the effect's default seconds (``meow`` is instant)."""

    effect: str
    glb: str = ""
    weight: int = 1
    duration: float = 0.0
    color: list[int] = field(default_factory=lambda: [0, 0, 0])
    icon: str | None = None

    def __post_init__(self) -> None:
        if self.effect not in ITEM_EFFECTS:
            raise ValueError(f"item effect {self.effect!r} not in {ITEM_EFFECTS}")
        if self.icon is not None and self.icon not in ITEM_ICONS:
            raise ValueError(f"item icon {self.icon!r} not in {ITEM_ICONS}")

    def rgb(self) -> list[int]:
        """The colour in use (0,0,0 = the effect's own)."""
        return list(self.color) if any(self.color) else list(ITEM_COLORS[self.effect])


@dataclass
class Placement:
    """Reserved: one baked spawn (``kind`` is an obstacle name or ``item:<effect>``)."""

    z: float
    lane: int
    kind: str


@dataclass
class ItemRules:
    """Where pick-ups appear (free slots of rest and reward sections) and how strong they are."""

    rest_every: int = 3
    reward_every: int = 2
    hover: float = 0.7
    radius: float = 1.1
    magnet_radius: float = 4.0
    magnet_lead: float = 4.0
    pull_speed: float = 6.0
    pull_accel: float = 70.0
    warn_secs: float = 1.5
    boost_speed: float = 1.5
    jump_scale: float = 1.5

    def __post_init__(self) -> None:
        if not 0.2 <= self.radius <= 3.0 or not 0.0 <= self.hover <= 3.0:
            raise ValueError("item_rules radius 0.2..=3, hover 0..=3")
        if not 1.0 <= self.magnet_radius <= 12.0:
            raise ValueError("item_rules.magnet_radius must be 1..=12")
        if not (
            0.0 <= self.magnet_lead <= 10.0
            and 1.0 <= self.pull_speed <= 40.0
            and 0.0 <= self.pull_accel <= 400.0
            and 0.0 <= self.warn_secs <= 5.0
        ):
            raise ValueError("item_rules magnet_lead 0..=10, pull_speed 1..=40, pull_accel 0..=400, warn_secs 0..=5")
        if not 1.0 <= self.boost_speed <= 2.5 or not 1.0 <= self.jump_scale <= 2.5:
            raise ValueError("item_rules boost_speed and jump_scale must be 1..=2.5")


@dataclass
class MischiefSpec:
    """A knockable prop kind."""

    kind: str
    weight: int = 1

    def __post_init__(self) -> None:
        if self.kind not in MISCHIEF_KINDS:
            raise ValueError(f"mischief kind {self.kind!r} not in {MISCHIEF_KINDS}")


@dataclass
class MischiefRules:
    """Where knockable props stand (free slots, beside the track centre) and what a knock pays."""

    rest_every: int = 2
    reward_every: int = 3
    other_every: int = 5
    side: float = 1.3
    radius: float = 0.75
    score: int = 25
    streak_secs: float = 2.5
    streak_bonus: int = 10
    streak_cap: int = 6

    def __post_init__(self) -> None:
        if not 0.0 <= self.side <= 2.0 or not 0.2 <= self.radius <= 2.0:
            raise ValueError("mischief_rules side 0..=2, radius 0.2..=2")
        if not 0.2 <= self.streak_secs <= 10.0 or not 1 <= self.streak_cap <= 20:
            raise ValueError("mischief_rules streak_secs 0.2..=10, streak_cap 1..=20")


def _default_mischief() -> list[MischiefSpec]:
    return [MischiefSpec(k, w) for k, w in (("vase", 2), ("bottle", 2), ("can", 2), ("yarn", 1))]


@dataclass
class PerchStep:
    """One ledge of a perch route. ``height`` 0 means the kind's own top height."""

    kind: str
    height: float = 0.0

    def __post_init__(self) -> None:
        if self.kind not in PERCH_KINDS:
            raise ValueError(f"perch kind {self.kind!r} not in {PERCH_KINDS}")

    def top(self) -> float:
        return self.height if self.height > 0 else PERCH_HEIGHT[self.kind]


def _default_perch_route() -> list[PerchStep]:
    return [PerchStep(k) for k in PERCH_KINDS]


@dataclass
class PerchRules:
    """Where perch routes stand (free slots, beside the track centre) and what running on them pays."""

    rest_every: int = 2
    reward_every: int = 2
    other_every: int = 6
    side: float = 1.35
    half_width: float = 0.7
    step_len: float = 3.6
    step_up: float = 0.55
    land_score: int = 30
    run_score: int = 40
    route_bonus: int = 120
    coins_per_step: int = 3

    def __post_init__(self) -> None:
        if not 0.0 <= self.side <= 1.8 or not 0.4 <= self.half_width <= 1.2:
            raise ValueError("perch_rules side 0..=1.8, half_width 0.4..=1.2")
        if not 1.5 <= self.step_len <= 8.0 or not 0.1 <= self.step_up <= 1.0:
            raise ValueError("perch_rules step_len 1.5..=8, step_up 0.1..=1")
        if not 0 <= self.coins_per_step <= 6:
            raise ValueError("perch_rules coins_per_step 0..=6")


@dataclass
class ChaserSpec:
    """The cleaning robot that gives chase after a hit; off unless the spec sets ``enabled`` (``far_gap`` stays near: past ~2 m it leaves the bottom of the screen)."""

    enabled: bool = False
    secs: float = 10.0
    near_gap: float = 1.2
    far_gap: float = 1.7
    perch_drain: float = 2.5
    scare_score: int = 50
    color: list = field(default_factory=lambda: [255, 80, 40])

    def __post_init__(self) -> None:
        if not 2.0 <= self.secs <= 60.0 or not 1.0 <= self.perch_drain <= 3.0:
            raise ValueError("chaser secs 2..=60, perch_drain 1..=3")
        if not 0.8 <= self.near_gap <= self.far_gap or not self.far_gap <= 2.2:
            raise ValueError("chaser near_gap 0.8.., far_gap near_gap..=2.2")


def _default_items() -> list[ItemSpec]:
    return [ItemSpec(e, weight=w) for e, w in (("magnet", 3), ("double", 3), ("superjump", 2), ("boost", 2), ("meow", 1))]


def _neon_obstacles() -> list[EntityKind]:
    rows = [("CRATE", 2), ("BARREL", 1), ("BARRIER", 2), ("GENERATOR", 1),
            ("OVERPASS", 1), ("TRAIN", 1), ("WALL", 1)]
    return [EntityKind(**{**CATALOG[n], "weight": w}) for n, w in rows]


@dataclass
class InputSpec:
    """Touch scheme (mirrors Rust ``InputSpec``). Distances are short-side fractions."""

    scheme: str = "drag"  # "drag" (hold and drag to steer) or "swipe" (one lane per swipe)
    drag_lane_frac: float = 0.18
    drag_smooth_secs: float = 0.05
    drag_snap: bool = True
    flick_dist: float = 0.07
    flick_angle_deg: float = 30.0
    swipe_dist: float = 0.09
    swipe_cooldown_secs: float = 0.3
    buffer_secs: float = 0.15
    reverse_guard_secs: float = 0.2  # an up flick this soon after a down flick is the finger coming back


BOARD_COLUMNS = ("time", "gates", "coins", "score")


@dataclass
class PanelSpec:
    """9-slice HUD panel colours (RGBA) and the rim size in HUD units."""

    fill: list[int] = field(default_factory=lambda: [14, 10, 32, 205])
    border: list[int] = field(default_factory=lambda: [90, 210, 230, 255])
    coin: list[int] = field(default_factory=lambda: [255, 205, 70, 255])
    score: list[int] = field(default_factory=lambda: [190, 248, 255, 255])
    slice: float = 3.0


@dataclass
class BoardSpec:
    """Local top-N ranking on the result screen (saved on the device only)."""

    enabled: bool = True
    top_n: int = 5
    columns: list[str] = field(default_factory=lambda: list(BOARD_COLUMNS))
    sort: str = "score"

    def __post_init__(self) -> None:
        if not 1 <= self.top_n <= 10:
            raise ValueError("hud.result.board.top_n must be 1..=10")
        unknown = [c for c in self.columns if c not in BOARD_COLUMNS]
        if unknown or not self.columns:
            raise ValueError(f"hud.result.board.columns: use some of {BOARD_COLUMNS}, got {self.columns}")
        if self.sort not in self.columns:
            raise ValueError("hud.result.board.sort must be one of its columns")


def _death_lines() -> dict[str, str]:
    return {
        "CRATE": "木箱にぶつかった",
        "BARREL": "樽にぶつかった",
        "BARRIER": "バリケードにぶつかった",
        "GENERATOR": "発電機にぶつかった",
        "OVERPASS": "ひさしに頭をぶつけた（下フリックでころがる）",
        "TRAIN": "電車にぶつかった",
        "WALL": "壁にぶつかった（すき間は下フリックでくぐる）",
        "METEOR": "隕石にぶつかった",
        "CRYSTALS": "クリスタルにぶつかった",
        "ROCK": "岩にぶつかった",
    }


@dataclass
class ResultSpec:
    beat_secs: float = 0.7  # the crash plays out this long before the result
    board: BoardSpec = field(default_factory=BoardSpec)
    deaths: dict[str, str] = field(default_factory=_death_lines)  # obstacle name -> line
    death_fallback: str = "ぶつかった"

    def __post_init__(self) -> None:
        # Overrides may reword only the obstacles they name.
        self.deaths = {**_death_lines(), **self.deaths}


@dataclass
class HudSpec:
    scale: float = 1.0
    panel: PanelSpec = field(default_factory=PanelSpec)
    result: ResultSpec = field(default_factory=ResultSpec)


@dataclass
class GameSpec:
    kind: str = "runner"
    id: str = "neon_cat"
    title: str = "NEON CAT"
    seed: int = 11
    player: PlayerSpec = field(default_factory=PlayerSpec)
    obstacles: list[EntityKind] = field(default_factory=_neon_obstacles)
    hit_inset: float = 0.06
    hit_top: float = 0.82
    camera: CameraSpec = field(default_factory=CameraSpec)
    style: Style = field(default_factory=Style)
    hud_top: float = 96.0
    hud: HudSpec = field(default_factory=HudSpec)
    lives: LivesSpec = field(default_factory=LivesSpec)
    items: list[ItemSpec] = field(default_factory=_default_items)
    item_rules: ItemRules = field(default_factory=ItemRules)
    mischief: list[MischiefSpec] = field(default_factory=_default_mischief)
    mischief_rules: MischiefRules = field(default_factory=MischiefRules)
    perch_route: list[PerchStep] = field(default_factory=_default_perch_route)
    perch_rules: PerchRules = field(default_factory=PerchRules)
    chaser: ChaserSpec = field(default_factory=ChaserSpec)
    placements: list[Placement] = field(default_factory=list)
    runner: RunnerKit = field(default_factory=RunnerKit)
    input: InputSpec = field(default_factory=InputSpec)
    # Declarative feel presets, e.g. {"preset": "snappy_runner", "movement": {"jump": "forgiving"}}.
    # Resolved and validated by the engine (kagra-shared game_core::feel).
    feel: dict = field(default_factory=dict)

    @classmethod
    def from_overrides(cls, overrides: dict | None = None) -> GameSpec:
        """Neon Cat defaults with ``overrides`` merged in (unknown keys raise)."""
        return _build(cls, overrides or {})

    @classmethod
    def load(cls, path: str | Path) -> GameSpec:
        return cls.from_overrides(json.loads(Path(path).read_text(encoding="utf-8")))

    def to_dict(self) -> dict:
        if self.input.scheme not in ("drag", "swipe"):
            raise ValueError(f"input scheme {self.input.scheme!r} not in ('drag', 'swipe')")
        if self.kind not in KINDS:
            raise ValueError(f"game kind {self.kind!r} not in {KINDS}")
        if not self.obstacles or all(o.weight <= 0 for o in self.obstacles):
            raise ValueError("game spec needs an obstacle with weight > 0")
        self._check_item_colors()
        self._check_perch_route()
        return asdict(self)

    def _check_perch_route(self) -> None:
        """A route has to be climbable and fit a slot (same rules as Rust)."""
        r = self.perch_rules
        if len(self.perch_route) > 6:
            raise ValueError("perch_route has at most 6 steps")
        if not self.perch_route:
            return
        jump = self.runner.jump
        apex = jump.velocity**2 / (2.0 * max(-jump.gravity, 1e-3))
        if any(not 0.2 <= p.top() <= 2.4 for p in self.perch_route):
            raise ValueError("perch_route heights 0.2..=2.4")
        if self.perch_route[0].top() > apex * 0.9 + r.step_up:
            raise ValueError(f"the first perch step ({self.perch_route[0].top():.2f} m) is out of reach of a jump")
        for a, b in zip(self.perch_route, self.perch_route[1:]):
            if b.top() - a.top() > r.step_up + 1e-4:
                raise ValueError("a perch step is too high above the one before it")
        if len(self.perch_route) * r.step_len > self.runner.obstacle_track.gap - 3.0:
            raise ValueError("the perch route is longer than a slot")

    def _check_item_colors(self) -> None:
        """Orb colours must tell apart (sum of channel differences >= 90), same test as Rust."""

        def dist(a, b) -> int:
            return sum(abs(int(x) - int(y)) for x, y in zip(a, b))

        fixed = {"coin": COIN_COLOR, **{k: v for k, v in self.style.codes.items()}}
        for i, a in enumerate(self.items):
            for name, f in fixed.items():
                if dist(a.rgb(), f) < 90:
                    raise ValueError(f"item {a.effect} colour is too close to {name}")
            for b in self.items[i + 1 :]:
                if a.effect != b.effect and dist(a.rgb(), b.rgb()) < 90:
                    raise ValueError(f"items {a.effect} and {b.effect} are too alike to tell apart")

    def world(self, **extra) -> dict:
        """World dump the Rust sim loads (``NeonCat.from_json``)."""
        return {"version": 1, "genre": "neon_cat", **extra, "game": self.to_dict()}

    def world_json(self, **extra) -> str:
        return json.dumps(self.world(**extra), indent=1) + "\n"

    def write(self, path: str | Path, **extra) -> Path:
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(self.world_json(**extra), encoding="utf-8")
        return path


def _entity(value: object) -> EntityKind:
    if isinstance(value, str):
        value = {"name": value}
    if not isinstance(value, dict):
        raise TypeError(f"obstacle must be a name or object, got {value!r}")
    base = CATALOG.get(value.get("name", ""), {}) if "glb" not in value else {}
    return EntityKind(**{**base, **value})


def _build(cls: type, data: dict, base: object | None = None) -> object:
    base = cls() if base is None else base
    hints = typing.get_type_hints(cls)
    names = {f.name for f in fields(cls)}
    unknown = set(data) - names
    if unknown:
        raise KeyError(f"{cls.__name__}: unknown keys {sorted(unknown)}")
    kwargs = {}
    for name in names:
        current = getattr(base, name)
        if name not in data:
            kwargs[name] = current
            continue
        value = data[name]
        if cls is GameSpec and name == "obstacles":
            kwargs[name] = [_entity(v) for v in value]
        elif cls is GameSpec and name == "items":
            kwargs[name] = [ItemSpec(**v) if isinstance(v, dict) else ItemSpec(v) for v in value]
        elif cls is GameSpec and name == "mischief":
            kwargs[name] = [MischiefSpec(**v) if isinstance(v, dict) else MischiefSpec(v) for v in value]
        elif cls is GameSpec and name == "perch_route":
            kwargs[name] = [PerchStep(**v) if isinstance(v, dict) else PerchStep(v) for v in value]
        elif cls is GameSpec and name == "placements":
            kwargs[name] = [Placement(**v) for v in value]
        elif cls is RunnerKit and name == "sections":
            kwargs[name] = [Section(**v) for v in value]
        elif is_dataclass(current) and isinstance(value, dict):
            kwargs[name] = _build(type(current), value, current)
        else:
            if hints.get(name) is float and isinstance(value, int):
                value = float(value)
            kwargs[name] = value
    return cls(**kwargs)
