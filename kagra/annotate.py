"""Click annotations — turn 「ここもう少し」 into numbers for agents.

Not a visual editor. A preview click writes one JSONL row under ``scratch/``.
Screen xy always; world xyz / bone / Prop id when the ray can resolve them.
"""
from __future__ import annotations

import json
import math
import time
from pathlib import Path
from typing import Any, Callable, Optional

DEFAULT_PATH = Path("scratch") / "annotations.jsonl"


def make_note(
    sx: float,
    sy: float,
    *,
    world: tuple[float, float, float] | None = None,
    bone: str | None = None,
    prop_id: int | None = None,
    timestamp: float | None = None,
    screenshot: str | None = None,
    note: str | None = None,
    frame: int | None = None,
) -> dict[str, Any]:
    """Build the JSON object. Missing optional fields are omitted."""
    rec: dict[str, Any] = {
        "sx": float(sx),
        "sy": float(sy),
        "timestamp": time.time() if timestamp is None else float(timestamp),
    }
    if world is not None:
        rec["wx"] = float(world[0])
        rec["wy"] = float(world[1])
        rec["wz"] = float(world[2])
    if bone:
        rec["bone"] = str(bone)
    if prop_id is not None:
        rec["prop_id"] = int(prop_id)
    if screenshot:
        rec["screenshot"] = str(screenshot)
    if note:
        rec["note"] = str(note)
    if frame is not None:
        rec["frame"] = int(frame)
    return rec


def append_jsonl(record: dict, path: str | Path | None = None) -> Path:
    """Append one JSON object as a line. Creates ``scratch/`` if needed."""
    dest = Path(path) if path is not None else DEFAULT_PATH
    dest.parent.mkdir(parents=True, exist_ok=True)
    with dest.open("a", encoding="utf-8") as f:
        f.write(json.dumps(record, ensure_ascii=False) + "\n")
    return dest


def _norm_dir(dx: float, dy: float, dz: float) -> Optional[tuple[float, float, float]]:
    length = math.sqrt(dx * dx + dy * dy + dz * dz)
    if length < 1e-8:
        return None
    return dx / length, dy / length, dz / length


def plane_hit(
    origin: tuple[float, float, float],
    direction: tuple[float, float, float],
    y: float = 0.0,
    max_dist: float = 80.0,
) -> Optional[tuple[float, float, float]]:
    """Ray vs Y = ``y`` plane. Miss / behind → None."""
    nd = _norm_dir(*direction)
    if nd is None:
        return None
    dx, dy, dz = nd
    if abs(dy) < 1e-8:
        return None
    t = (float(y) - origin[1]) / dy
    if t < 0.0 or t > float(max_dist):
        return None
    return origin[0] + dx * t, float(y), origin[2] + dz * t


def height_hit(
    origin: tuple[float, float, float],
    direction: tuple[float, float, float],
    height_fn: Callable[[float, float], float],
    max_dist: float = 80.0,
    steps: int = 48,
) -> Optional[tuple[float, float, float]]:
    """March the ray and stop at the first crossing of ``y`` vs ``height_fn(x, z)``."""
    nd = _norm_dir(*direction)
    if nd is None:
        return None
    dx, dy, dz = nd
    ox, oy, oz = origin
    prev_above: Optional[bool] = None
    n = max(4, int(steps))
    for i in range(1, n + 1):
        t = float(max_dist) * i / n
        px, py, pz = ox + dx * t, oy + dy * t, oz + dz * t
        gy = float(height_fn(px, pz))
        above = py >= gy
        if prev_above is True and not above:
            return px, gy, pz
        if prev_above is None and not above and i == 1:
            return px, gy, pz
        prev_above = above
    return None


def world_from_ray(
    origin: tuple[float, float, float],
    direction: tuple[float, float, float],
    *,
    world=None,
    height_fn: Callable[[float, float], float] | None = None,
    floor_y: float = 0.0,
    max_dist: float = 80.0,
    ignore=None,
) -> Optional[tuple[float, float, float]]:
    """Closest world hit: static physics, then height field, then floor plane."""
    nd = _norm_dir(*direction)
    if nd is None:
        return None
    dx, dy, dz = nd
    phys = getattr(world, "physics", None) if world is not None else None
    if phys is not None and hasattr(phys, "raycast"):
        skip = ignore
        if skip is None:
            skip = getattr(world, "player", None)
        hit = phys.raycast(
            origin[0], origin[1], origin[2], dx, dy, dz,
            max_dist=float(max_dist),
            ignore=skip,
            skip_triggers=True,
            static_only=True,
        )
        if hit is not None:
            return float(hit[2]), float(hit[3]), float(hit[4])
    fn = height_fn
    if fn is None and world is not None:
        fn = getattr(world, "_height_fn", None)
        if fn is None and hasattr(world, "ground_y"):
            fn = lambda x, z, _w=world: _w.ground_y(x, z)
    if fn is not None:
        h = height_hit(origin, (dx, dy, dz), fn, max_dist=max_dist)
        if h is not None:
            return h
    fy = float(floor_y)
    if world is not None and hasattr(world, "floor_y"):
        fy = float(world.floor_y)
    return plane_hit(origin, (dx, dy, dz), y=fy, max_dist=max_dist)


def _prop_id(prop) -> int | None:
    if prop is None:
        return None
    pid = getattr(prop, "id", None)
    return int(pid) if pid is not None else None


def annotate(
    sx: float,
    sy: float,
    *,
    cam=None,
    origin: tuple[float, float, float] | None = None,
    direction: tuple[float, float, float] | None = None,
    avatar=None,
    world=None,
    height_fn=None,
    screenshot: str | None = None,
    note: str | None = None,
    path: str | Path | None = None,
    timestamp: float | None = None,
    frame: int | None = None,
    persist: bool = True,
    bone: str | None = None,
    prop_id: int | None = None,
    max_dist: float = 80.0,
) -> dict[str, Any]:
    """Record a preview click. Returns the note; appends JSONL when ``persist``.

    Pass ``origin`` / ``direction`` (or a ``cam`` with ``ray_from_screen``)
    to fill world xyz. ``avatar.pick`` fills bone. ``hovered_prop`` fills prop id.
    GPU is not required.
    """
    if origin is None or direction is None:
        if cam is not None and hasattr(cam, "ray_from_screen"):
            ray = cam.ray_from_screen(float(sx), float(sy))
            if ray is not None:
                origin, direction = ray
    xyz = None
    if origin is not None and direction is not None:
        xyz = world_from_ray(
            origin, direction,
            world=world, height_fn=height_fn, max_dist=max_dist,
        )
        if prop_id is None:
            try:
                from kagra.play import hovered_prop as _hover

                hit = _hover(
                    origin[0], origin[1], origin[2],
                    direction[0], direction[1], direction[2],
                    max_dist=float(max_dist),
                )
                prop_id = _prop_id(hit)
                if xyz is None and hit is not None:
                    xyz = (
                        float(getattr(hit, "x", 0.0)),
                        float(getattr(hit, "y", 0.0)),
                        float(getattr(hit, "z", 0.0)),
                    )
            except Exception:
                pass
    if bone is None and avatar is not None and hasattr(avatar, "pick"):
        try:
            bone = avatar.pick(float(sx), float(sy), camera=cam)
        except Exception:
            bone = None
    rec = make_note(
        sx, sy,
        world=xyz,
        bone=bone,
        prop_id=prop_id,
        timestamp=timestamp,
        screenshot=screenshot,
        note=note,
        frame=frame,
    )
    if persist:
        rec["path"] = str(append_jsonl(rec, path))
    return rec
