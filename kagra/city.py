"""小さな街 JSON。OSM / ``*.kagra.json`` 道路マップではない。

箱の一覧をタイルに分けて ``World3D.set_chunk_fill`` から置く。
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from kagra.land import TILE, tile_index


def load_city(path: str | Path) -> dict[str, Any]:
    """街ファイルを読む。``version`` は 1。"""
    raw = Path(path).read_text(encoding="utf-8")
    data = json.loads(raw)
    if not isinstance(data, dict):
        raise ValueError("city file must be a JSON object")
    ver = int(data.get("version", 1))
    if ver != 1:
        raise ValueError(f"unsupported city version {ver}")
    data.setdefault("tile", TILE)
    data.setdefault("boxes", [])
    if not isinstance(data["boxes"], list):
        raise ValueError("city.boxes must be a list")
    return data


def city_chunk(
    city: dict[str, Any],
    ix: int,
    iz: int,
    *,
    tile: float | None = None,
) -> list[tuple[float, float, float, float, float, float]]:
    """そのタイルに属する箱 ``(x, y, z, w, h, d)``。``y`` 省略は 0。"""
    t = float(city.get("tile", TILE) if tile is None else tile)
    out: list[tuple[float, float, float, float, float, float]] = []
    for raw in city.get("boxes") or []:
        if not isinstance(raw, dict):
            continue
        x = float(raw["x"])
        z = float(raw["z"])
        kx, kz = tile_index(x, z, t)
        if kx != int(ix) or kz != int(iz):
            continue
        y = float(raw["y"]) if raw.get("y") is not None else 0.0
        w = float(raw.get("w", 2.0))
        h = float(raw.get("h", 2.0))
        d = float(raw.get("d", 2.0))
        out.append((x, y, z, w, h, d))
    return out
