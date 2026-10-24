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

from kagra.runner_kit import RunnerKit, Section

ROOT = Path(__file__).resolve().parents[1]
KINDS = ("runner",)
#: Closed item vocabulary. A new effect needs engine code, so specs can't add one.
ITEM_EFFECTS = ("shield", "magnet", "slowmo", "double", "boost")

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
class EntityKind:
    """One entity table row. ``mesh_min`` / ``mesh_max`` (the hitbox) are
    measured from the GLB when omitted."""

    name: str
    glb: str
    scale: list[float] = field(default_factory=lambda: [1.0, 1.0, 1.0])
    mesh_min: list[float] | None = None
    mesh_max: list[float] | None = None
    weight: int = 1

    def __post_init__(self) -> None:
        if isinstance(self.scale, (int, float)):
            self.scale = [float(self.scale)] * 3
        if self.mesh_min is None or self.mesh_max is None:
            self.mesh_min, self.mesh_max = measure_glb(ROOT / self.glb)


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
    fill_emissive: float = 0.35
    obstacle_color: list[int] = field(default_factory=lambda: [36, 26, 54])


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
    return [EntityKind(**CATALOG[n]) for n in ("CRATE", "BARREL", "BARRIER")]


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
    items: list[ItemSpec] = field(default_factory=list)
    placements: list[Placement] = field(default_factory=list)
    runner: RunnerKit = field(default_factory=RunnerKit)
    input: InputSpec = field(default_factory=InputSpec)

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
