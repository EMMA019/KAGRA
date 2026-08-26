"""The one World: World3D plus query / dump / load.

``World`` is ``World3D``. Stable string ids (not GPU mesh integers). An agent
reads position / name / type / id — and for tiles ``loaded`` / ``albedo_ok`` —
without a screenshot. Drawing is unchanged.
"""
from __future__ import annotations

import json
from typing import Any, Callable, Optional

from kagra.land import island_height, open_world_height, overworld_height, tile_origin
from kagra.world3d import World3D, _terrain_albedo_ok

HEIGHT_FNS: dict[str, Callable[[float, float], float]] = {
    "island_height": island_height,
    "overworld_height": overworld_height,
    "open_world_height": open_world_height,
}

WORLD_SCHEMA_VERSION = 1


def height_fn_name(fn) -> str | None:
    """Serializable name for a demo height_fn, or None if it cannot dump."""
    if fn is None:
        return None
    for name, cand in HEIGHT_FNS.items():
        if fn is cand:
            return name
    raw = getattr(fn, "__name__", None)
    if isinstance(raw, str) and raw in HEIGHT_FNS:
        return raw
    return None


def height_fn_from_name(name: str | None):
    if not name:
        return None
    return HEIGHT_FNS.get(str(name))


def _sid(kind: str, key: str) -> str:
    return f"{kind}:{key}"


def _aabb_hit(
    x: float, y: float, z: float, aabb
) -> bool:
    """``aabb`` is ``(minx, minz, maxx, maxz)`` or ``(minx, miny, minz, maxx, maxy, maxz)``."""
    if aabb is None:
        return True
    box = tuple(aabb)
    if len(box) == 4:
        minx, minz, maxx, maxz = (float(v) for v in box)
        return minx <= float(x) <= maxx and minz <= float(z) <= maxz
    if len(box) == 6:
        minx, miny, minz, maxx, maxy, maxz = (float(v) for v in box)
        return (
            minx <= float(x) <= maxx
            and miny <= float(y) <= maxy
            and minz <= float(z) <= maxz
        )
    raise ValueError("aabb must be 4 (xz) or 6 (xyz) numbers")


def _type_ok(got: str, want: str | None) -> bool:
    if want is None or want == "":
        return True
    a = str(got).lower()
    b = str(want).lower()
    if a == b:
        return True
    aliases = {
        "tile": "terrain_tile",
        "terrain": "terrain_tile",
        "walk": "walker",
        "player": "walker",
    }
    return aliases.get(b, b) == a


def _name_ok(got: str | None, want: str | None) -> bool:
    if want is None or want == "":
        return True
    return str(got or "") == str(want)


def _tile_albedo_ok(world: World3D, key: tuple[int, int]) -> bool:
    """False when a leftover loaded tile has no mesh or a 1×1 / missing albedo.

    GPU-free stream (``_terrain_tex == 0``) with a key in ``_loaded_tiles`` and
    no mesh is *not* bald leftover — tests still track keys without GPU.
    Bald leftover is: key in ``_loaded_tiles`` AND (GPU tex set with no mesh,
    or live tex is missing/1×1).
    """
    loaded = key in world._loaded_tiles
    if not loaded:
        return False
    has_mesh = bool(world._tile_meshes.get(key))
    tex = int(getattr(world, "_terrain_tex", 0) or 0)
    if tex <= 0:
        return True
    if not has_mesh:
        return False
    try:
        import kagra

        return bool(_terrain_albedo_ok(kagra, tex))
    except Exception:
        return has_mesh


def _tile_rows(world: World3D) -> list[dict[str, Any]]:
    tile = float(world._tile or 0.0)
    keys = set(world._loaded_tiles)
    keys.update(world._tile_meshes.keys())
    rows: list[dict[str, Any]] = []
    for key in sorted(keys):
        ix, iz = int(key[0]), int(key[1])
        if tile > 0:
            ox, oz = tile_origin(ix, iz, tile)
            cx = ox + tile * 0.5
            cz = oz + tile * 0.5
        else:
            cx, cz = 0.0, 0.0
        gy = world.ground_y(cx, cz)
        loaded = key in world._loaded_tiles
        has_mesh = bool(world._tile_meshes.get(key))
        albedo_ok = _tile_albedo_ok(world, key)
        rows.append(
            {
                "id": _sid("tile", f"{ix},{iz}"),
                "type": "terrain_tile",
                "name": f"tile:{ix},{iz}",
                "position": [float(cx), float(gy), float(cz)],
                "ix": ix,
                "iz": iz,
                "loaded": loaded,
                "has_mesh": has_mesh,
                "albedo_ok": albedo_ok,
            }
        )
    return rows


def _prop_rows(world: World3D) -> list[dict[str, Any]]:
    try:
        from kagra.play import Prop
    except Exception:
        return []
    rows: list[dict[str, Any]] = []
    for p in list(Prop._all):
        if getattr(p, "world", None) is not world:
            continue
        if getattr(p, "_destroyed", False):
            continue
        wx, wy, wz, _yaw = p.world_pose()
        parent = p.parent
        parent_id = None
        if parent is not None:
            parent_id = str(getattr(parent, "sid", "") or _sid("prop", str(parent.id)))
        sid = str(getattr(p, "sid", "") or _sid("prop", str(p.id)))
        rows.append(
            {
                "id": sid,
                "type": "prop",
                "name": str(getattr(p, "name", "") or ""),
                "position": [float(wx), float(wy), float(wz)],
                "yaw": float(p.yaw),
                "model": str(getattr(p, "model", "box") or "box"),
                "gltf": str(p.gltf_path) if getattr(p, "gltf_path", None) else None,
                "scale": [float(p.sx), float(p.sy), float(p.sz)],
                "enabled": bool(p.enabled),
                "parent": parent_id,
                "color": [int(c) for c in (p.color or (255, 255, 255))[:3]],
            }
        )
    return rows


def _walker_rows(world: World3D) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    seen: set[str] = set()
    for walk in list(getattr(world, "_walkers", None) or []):
        p = world.player
        if p is None:
            continue
        sid = str(getattr(walk, "sid", "") or "walker:player")
        if sid in seen:
            continue
        seen.add(sid)
        rows.append(
            {
                "id": sid,
                "type": "walker",
                "name": str(getattr(walk, "name", "") or "player"),
                "position": [float(p.x), float(p.y), float(p.z)],
                "yaw": float(getattr(walk, "yaw", 0.0) or 0.0),
                "face": float(getattr(walk, "face", 0.0) or 0.0),
                "on_ground": bool(getattr(p, "on_ground", False)),
            }
        )
    p = world.player
    if p is not None and not seen:
        rows.append(
            {
                "id": "walker:player",
                "type": "walker",
                "name": "player",
                "position": [float(p.x), float(p.y), float(p.z)],
                "yaw": 0.0,
                "face": 0.0,
                "on_ground": bool(getattr(p, "on_ground", False)),
            }
        )
    return rows


def _light_rows(world: World3D) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    stored = list(getattr(world, "_lights", None) or [])
    if stored:
        for item in stored:
            slot = int(item.get("slot", 0))
            rows.append(
                {
                    "id": str(item.get("id") or _sid("light", str(slot))),
                    "type": "light",
                    "name": str(item.get("name") or f"light:{slot}"),
                    "position": list(item.get("position") or [0.0, 0.0, 0.0]),
                    "kind": str(item.get("kind") or "point"),
                    "slot": slot,
                    "intensity": float(item.get("intensity") or 0.0),
                    "radius": float(item.get("radius") or 0.0),
                    "color": list(item.get("color") or [1.0, 1.0, 1.0]),
                    "direction": item.get("direction"),
                }
            )
        return rows
    try:
        from kagra.look import current_lights
    except Exception:
        return rows
    try:
        lights = current_lights()
    except Exception:
        return rows
    for item in lights:
        if not item:
            continue
        slot = int(item.get("slot", 0))
        pos = item.get("position") or [0.0, 0.0, 0.0]
        rows.append(
            {
                "id": _sid("light", str(slot)),
                "type": "light",
                "name": f"light:{slot}",
                "position": [float(pos[0]), float(pos[1]), float(pos[2])],
                "kind": str(item.get("kind") or "point"),
                "slot": slot,
                "intensity": float(item.get("intensity") or 0.0),
                "radius": float(item.get("radius") or 0.0),
                "color": list(item.get("color") or [1.0, 1.0, 1.0]),
                "direction": item.get("direction"),
            }
        )
    return rows


def _camera_rows(world: World3D) -> list[dict[str, Any]]:
    cams: list[Any] = []
    seen: set[int] = set()
    for walk in list(getattr(world, "_walkers", None) or []):
        cam = getattr(walk, "cam", None)
        if cam is not None and id(cam) not in seen:
            cams.append(cam)
            seen.add(id(cam))
    extra = getattr(world, "_cameras", None) or []
    for cam in extra:
        if cam is not None and id(cam) not in seen:
            cams.append(cam)
            seen.add(id(cam))
    try:
        import kagra

        g = kagra.get_camera3d()
        if g is not None and id(g) not in seen:
            cams.append(g)
    except Exception:
        pass
    rows: list[dict[str, Any]] = []
    for i, cam in enumerate(cams):
        pos = getattr(cam, "position", (0.0, 0.0, 0.0))
        tgt = getattr(cam, "target", (0.0, 0.0, 0.0))
        sid = str(getattr(cam, "sid", "") or _sid("camera", str(i)))
        rows.append(
            {
                "id": sid,
                "type": "camera",
                "name": str(getattr(cam, "name", "") or ("main" if i == 0 else f"camera:{i}")),
                "position": [float(pos[0]), float(pos[1]), float(pos[2])],
                "target": [float(tgt[0]), float(tgt[1]), float(tgt[2])],
                "fov": float(getattr(cam, "fov_deg", 30.0) or 30.0),
            }
        )
    return rows


def _height_samples(world: World3D, *, step: float | None = None, limit: int = 9) -> list:
    """Serializable height samples. The live fn cannot dump."""
    fn = world._height_fn
    if fn is None:
        return []
    tile = float(world._tile or 10.0)
    st = float(step if step is not None else max(tile, 1.0))
    half = float(world.half)
    n = int(min(limit, max(3, round(2.0 * half / st) + 1)))
    if n <= 1:
        return [[0.0, 0.0, float(fn(0.0, 0.0))]]
    out: list[list[float]] = []
    for i in range(n):
        x = -half + (2.0 * half) * i / (n - 1)
        for j in range(n):
            z = -half + (2.0 * half) * j / (n - 1)
            out.append([round(x, 4), round(z, 4), round(float(fn(x, z)), 4)])
    return out


def query(
    world: World3D,
    type: str | None = None,
    name: str | None = None,
    aabb=None,
) -> list[dict[str, Any]]:
    """Filter world objects. Returns dicts an agent can read without a screenshot.

    ``type``: ``prop`` / ``walker`` / ``light`` / ``camera`` / ``terrain_tile``.
    ``name``: exact name (props set ``name=``; player is ``player``).
    ``aabb``: ``(minx, minz, maxx, maxz)`` or ``(minx, miny, minz, maxx, maxy, maxz)``.
    """
    rows: list[dict[str, Any]] = []
    rows.extend(_prop_rows(world))
    rows.extend(_walker_rows(world))
    rows.extend(_light_rows(world))
    rows.extend(_camera_rows(world))
    rows.extend(_tile_rows(world))
    out: list[dict[str, Any]] = []
    for row in rows:
        if not _type_ok(str(row.get("type") or ""), type):
            continue
        if not _name_ok(row.get("name"), name):
            continue
        pos = row.get("position") or [0.0, 0.0, 0.0]
        px, py, pz = float(pos[0]), float(pos[1]), float(pos[2])
        if not _aabb_hit(px, py, pz, aabb):
            continue
        out.append(row)
    return out


def dump(world: World3D) -> dict[str, Any]:
    """JSON-ready world. GPU mesh ids are not game objects."""
    fn_name = height_fn_name(world._height_fn)
    tiles = _tile_rows(world)
    walkers = _walker_rows(world)
    player = next((w for w in walkers if w.get("name") == "player"), None)
    if player is None and walkers:
        player = walkers[0]
    coins = [
        r
        for r in _prop_rows(world)
        if r.get("name") == "coin" and r.get("enabled", True)
    ]
    heightfield = None
    if world._height_fn is not None:
        heightfield = {
            "fn": fn_name,
            "tile": world._tile,
            "stream_radius": world._stream_radius,
            "cells": world._height_cells,
            "lod_radius": world._lod_radius,
            "lod_cells": world._lod_cells,
            "uv": {
                "half": world.terrain_uv_half,
                "period": world.terrain_uv_period,
                "blend": world.terrain_uv_blend,
                "pad": world.terrain_uv_pad,
                "rect": list(world.terrain_uv_rect)
                if world.terrain_uv_rect is not None
                else None,
            },
            "tiles": [
                {
                    "id": t["id"],
                    "ix": t["ix"],
                    "iz": t["iz"],
                    "loaded": t["loaded"],
                    "has_mesh": t["has_mesh"],
                    "albedo_ok": t["albedo_ok"],
                    "position": t["position"],
                }
                for t in tiles
            ],
            "samples": _height_samples(world),
        }
    return {
        "version": WORLD_SCHEMA_VERSION,
        "half": float(world.half),
        "floor_y": float(world.floor_y),
        "gravity": float(getattr(world.physics, "gravity", 9.8) or 0.0),
        "water_y": world._water_y,
        "player": player,
        "coins": len(coins),
        "props": _prop_rows(world),
        "walkers": walkers,
        "lights": _light_rows(world),
        "cameras": _camera_rows(world),
        "heightfield": heightfield,
    }


def dump_json(world: World3D, path: str | None = None) -> dict[str, Any]:
    data = dump(world)
    if path:
        from pathlib import Path

        p = Path(path)
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
    return data


def _interp_height(samples: list, x: float, z: float) -> float:
    if not samples:
        return 0.0
    best = samples[0]
    best_d = 1e30
    for row in samples:
        if len(row) < 3:
            continue
        dx = float(row[0]) - x
        dz = float(row[1]) - z
        d = dx * dx + dz * dz
        if d < best_d:
            best_d = d
            best = row
    return float(best[2])


def load(world: World3D, data: dict[str, Any] | str) -> World3D:
    """Mutate ``world`` from a dump dict or JSON path. No GPU mesh ids."""
    if isinstance(data, str):
        from pathlib import Path

        data = json.loads(Path(data).read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise TypeError("world.load expects a dict or JSON path")

    world.half = float(data.get("half", world.half))
    world.floor_y = float(data.get("floor_y", world.floor_y))
    if "gravity" in data:
        try:
            world.physics.gravity = float(data["gravity"])
        except Exception:
            pass
    water = data.get("water_y")
    if water is not None:
        world.set_water_y(float(water))

    hf = data.get("heightfield") or {}
    fn = height_fn_from_name(hf.get("fn") if isinstance(hf, dict) else None)
    samples = list((hf or {}).get("samples") or [])
    if fn is None and samples:
        def _fn(x, z, _samples=samples):
            return _interp_height(_samples, float(x), float(z))

        fn = _fn
    if fn is not None:
        tile = hf.get("tile", 10.0)
        world.set_height_fn(
            fn,
            cells=hf.get("cells"),
            tile=None if tile is None else float(tile) if tile else None,
            stream_radius=hf.get("stream_radius"),
            lod_radius=hf.get("lod_radius"),
            lod_cells=hf.get("lod_cells"),
        )
        uv = hf.get("uv") or {}
        if "half" in uv:
            world.terrain_uv_half = uv["half"]
        if "period" in uv:
            world.terrain_uv_period = uv["period"]
        if "blend" in uv:
            world.terrain_uv_blend = float(uv.get("blend") or 0.0)
        if "pad" in uv:
            world.terrain_uv_pad = float(uv.get("pad") or 0.0)
        if "rect" in uv:
            world.terrain_uv_rect = uv.get("rect")
        for t in hf.get("tiles") or []:
            ix, iz = int(t.get("ix", 0)), int(t.get("iz", 0))
            key = (ix, iz)
            if t.get("loaded", True):
                world._loaded_tiles.add(key)
                world._tile_lod.setdefault(key, world._height_cells)
            if t.get("albedo_ok") is False and t.get("loaded"):
                world._tile_meshes.pop(key, None)

    player = data.get("player") or {}
    pos = player.get("position") if isinstance(player, dict) else None
    if world.player is None and pos:
        world.add_player(float(pos[0]), float(pos[2]))
    if world.player is not None and pos and len(pos) >= 3:
        world.player.x = float(pos[0])
        world.player.y = float(pos[1])
        world.player.z = float(pos[2])
        if "on_ground" in player:
            world.player.on_ground = bool(player["on_ground"])

    world._lights = []
    for lit in data.get("lights") or []:
        slot = int(lit.get("slot", 0))
        world._lights.append(
            {
                "id": lit.get("id") or _sid("light", str(slot)),
                "name": lit.get("name") or f"light:{slot}",
                "kind": lit.get("kind") or "point",
                "slot": slot,
                "position": list(lit.get("position") or [0.0, 0.0, 0.0]),
                "intensity": float(lit.get("intensity") or 0.0),
                "radius": float(lit.get("radius") or 0.0),
                "color": list(lit.get("color") or [1.0, 1.0, 1.0]),
                "direction": lit.get("direction"),
            }
        )

    world._cameras = []
    for cam in data.get("cameras") or []:
        world._cameras.append(_LoadedCamera(cam))

    _load_props(world, list(data.get("props") or []))
    return world


class _LoadedCamera:
    def __init__(self, rec: dict):
        pos = rec.get("position") or [0.0, 1.0, 3.0]
        tgt = rec.get("target") or [0.0, 1.0, 0.0]
        self.sid = str(rec.get("id") or "camera:0")
        self.name = str(rec.get("name") or "main")
        self.position = (float(pos[0]), float(pos[1]), float(pos[2]))
        self.target = (float(tgt[0]), float(tgt[1]), float(tgt[2]))
        self.fov_deg = float(rec.get("fov") or 30.0)


def _load_props(world: World3D, recs: list[dict]) -> None:
    try:
        from kagra.play import Prop, destroy
    except Exception:
        return
    for p in list(Prop._all):
        if getattr(p, "world", None) is world:
            try:
                destroy(p)
            except Exception:
                p._destroyed = True
    created: dict[str, Any] = {}
    pending_parent: list[tuple[Any, str]] = []
    for rec in recs:
        sid = str(rec.get("id") or "")
        model = rec.get("gltf") or rec.get("model") or "box"
        pos = rec.get("position") or [0.0, 0.5, 0.0]
        scale = rec.get("scale") or [1.0, 1.0, 1.0]
        color = rec.get("color") or (230, 230, 235)
        try:
            prop = Prop(
                str(model),
                x=float(pos[0]),
                y=float(pos[1]),
                z=float(pos[2]),
                scale=(float(scale[0]), float(scale[1]), float(scale[2])),
                color=tuple(color) if not isinstance(color, str) else color,
                collision=False,
                world=world,
                yaw=float(rec.get("yaw") or 0.0),
                name=str(rec.get("name") or ""),
            )
        except Exception:
            continue
        prop.sid = sid or prop.sid
        if rec.get("enabled") is False:
            prop.enabled = False
        created[prop.sid] = prop
        parent_id = rec.get("parent")
        if parent_id:
            pending_parent.append((prop, str(parent_id)))
    for prop, pid in pending_parent:
        parent = created.get(pid)
        if parent is not None:
            try:
                prop.set_parent(parent, keep_world=True)
            except Exception:
                pass


def eval_world_expect(data: dict[str, Any], spec: dict[str, Any]) -> list[str]:
    """GPU-free world assertions. Returns error strings (empty = ok)."""
    errors: list[str] = []
    player = data.get("player") or {}
    if "player.on_ground" in spec:
        want = bool(spec["player.on_ground"])
        got = bool(player.get("on_ground"))
        if got != want:
            errors.append(f"player.on_ground: want {want} got {got}")
    if "coins" in spec:
        want_n = int(spec["coins"])
        got_n = int(data.get("coins", -1))
        if got_n < 0:
            got_n = sum(
                1
                for p in data.get("props") or []
                if p.get("name") == "coin" and p.get("enabled", True)
            )
        if got_n != want_n:
            errors.append(f"coins: want {want_n} got {got_n}")
    for item in spec.get("query") or []:
        if not isinstance(item, dict):
            continue
        typ = item.get("type")
        nam = item.get("name")
        rows = [
            r
            for r in _flatten_dump(data)
            if _type_ok(str(r.get("type") or ""), typ) and _name_ok(r.get("name"), nam)
        ]
        if "count" in item and len(rows) != int(item["count"]):
            errors.append(
                f"query type={typ!r} name={nam!r}: want count {item['count']} got {len(rows)}"
            )
        if "min_count" in item and len(rows) < int(item["min_count"]):
            errors.append(
                f"query type={typ!r} name={nam!r}: want min {item['min_count']} got {len(rows)}"
            )
        if "albedo_ok" in item:
            bad = [r for r in rows if bool(r.get("albedo_ok", True)) != bool(item["albedo_ok"])]
            if item.get("require_any") and not rows:
                errors.append(f"query type={typ!r}: no rows for albedo_ok check")
            elif "count" not in item and "min_count" not in item and bad and not item.get("allow_mixed"):
                if all(bool(r.get("albedo_ok", True)) != bool(item["albedo_ok"]) for r in rows) or item.get("all"):
                    errors.append(
                        f"query type={typ!r}: albedo_ok want {item['albedo_ok']}"
                    )
        if item.get("loaded") is True and not any(r.get("loaded") for r in rows):
            errors.append(f"query type={typ!r}: expected a loaded tile")
    return errors


def _flatten_dump(data: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for key in ("props", "walkers", "lights", "cameras"):
        rows.extend(list(data.get(key) or []))
    hf = data.get("heightfield") or {}
    for t in hf.get("tiles") or []:
        row = dict(t)
        row.setdefault("type", "terrain_tile")
        row.setdefault("name", f"tile:{row.get('ix')},{row.get('iz')}")
        rows.append(row)
    return rows


def _bind_world_api() -> None:
    """Attach query/dump/load onto World3D so World and World3D are the same type."""
    if getattr(World3D, "_world_data_api", False):
        return

    def query_m(self, type: str | None = None, name: str | None = None, aabb=None):
        return query(self, type=type, name=name, aabb=aabb)

    def dump_m(self, path: str | None = None) -> dict[str, Any]:
        return dump_json(self, path)

    def load_m(self, data: dict[str, Any] | str) -> World3D:
        return load(self, data)

    World3D.query = query_m
    World3D.dump = dump_m
    World3D.load = load_m
    World3D._world_data_api = True


_bind_world_api()

World = World3D

__all__ = [
    "HEIGHT_FNS",
    "WORLD_SCHEMA_VERSION",
    "World",
    "World3D",
    "dump",
    "dump_json",
    "eval_world_expect",
    "height_fn_from_name",
    "height_fn_name",
    "load",
    "query",
]
