"""Genre-agnostic game spec: author in Python, run in Rust.

Core sections (any kit): identity + seed, ``player``, entity tables
(``obstacles``: glb + hitbox + weight), hit tuning, ``camera``, neon
``style``, HUD, reserved ``items`` / ``placements``. A genre kit adds one
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
ITEM_EFFECTS = ("shield", "magnet", "slowmo", "double", "boost")
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
class PlayerSpec:
    glb: str = "assets/models/kenney/cube-pets/animal-cat.glb"
    scale: float = 0.50
    foot: float = 0.30
    tint: list[int] = field(default_factory=lambda: [226, 206, 255])


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
    """Reserved: parsed and validated, not simulated yet."""

    effect: str
    glb: str = ""
    weight: int = 1
    duration: float = 0.0

    def __post_init__(self) -> None:
        if self.effect not in ITEM_EFFECTS:
            raise ValueError(f"item effect {self.effect!r} not in {ITEM_EFFECTS}")


@dataclass
class Placement:
    """Reserved: one baked spawn (``kind`` is an obstacle name or ``item:<effect>``)."""

    z: float
    lane: int
    kind: str


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
    items: list[ItemSpec] = field(default_factory=list)
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
        return asdict(self)

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
