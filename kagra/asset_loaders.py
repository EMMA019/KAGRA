from __future__ import annotations

import csv
import json
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kagra.asset_db import AssetDatabase
    from kagra.asset_types import AssetRecord


class AssetTilesetProxy:
    """TileSet 相当の軽量プロキシ。"""

    def __init__(self, texture_id: int, tile_w: int, tile_h: int, spacing: int = 0):
        import kagra

        self.texture_id = texture_id
        self.tile_w = int(tile_w)
        self.tile_h = int(tile_h)
        self.spacing = int(spacing)
        tw, th = kagra.texture_size(texture_id)
        self.cols = (tw + self.spacing) // (self.tile_w + self.spacing)
        self.rows = (th + self.spacing) // (self.tile_h + self.spacing)

    def get_uv(self, tile_id: int) -> tuple[float, float, float, float]:
        col = tile_id % self.cols
        row = tile_id // self.cols
        sx = col * (self.tile_w + self.spacing)
        sy = row * (self.tile_h + self.spacing)
        return float(sx), float(sy), float(self.tile_w), float(self.tile_h)


def load_image(db: "AssetDatabase", rec: "AssetRecord") -> int:
    import kagra

    return kagra.load_texture(rec.path)


def load_font(db: "AssetDatabase", rec: "AssetRecord") -> int:
    import kagra

    return kagra.load_font(rec.path)


def load_audio(db: "AssetDatabase", rec: "AssetRecord") -> str:
    return rec.path


def load_tileset(db: "AssetDatabase", rec: "AssetRecord") -> AssetTilesetProxy:
    tex_key = rec.metadata["texture_key"]
    texture_id = db.get(tex_key)
    return AssetTilesetProxy(
        texture_id=texture_id,
        tile_w=rec.metadata["tile_w"],
        tile_h=rec.metadata["tile_h"],
        spacing=rec.metadata.get("spacing", 0),
    )


def load_rig(db: "AssetDatabase", rec: "AssetRecord") -> int:
    import kagra

    return kagra.load_rig(rec.path)


def load_json(db: "AssetDatabase", rec: "AssetRecord"):
    with open(rec.path, encoding="utf-8") as f:
        return json.load(f)


def load_text(db: "AssetDatabase", rec: "AssetRecord") -> str:
    return Path(rec.path).read_text(encoding=rec.metadata.get("encoding", "utf-8"))


def load_binary(db: "AssetDatabase", rec: "AssetRecord") -> bytes:
    return Path(rec.path).read_bytes()


def load_prefab(db: "AssetDatabase", rec: "AssetRecord"):
    from kagra.prefab import Prefab

    return Prefab.load(rec.path)


def load_scene_json(db: "AssetDatabase", rec: "AssetRecord"):
    with open(rec.path, encoding="utf-8") as f:
        return json.load(f)


def load_map_csv_path(db: "AssetDatabase", rec: "AssetRecord") -> str:
    return rec.path


def load_map_csv_data(db: "AssetDatabase", rec: "AssetRecord") -> list[list[int]]:
    rows: list[list[int]] = []
    with open(rec.path, encoding=rec.metadata.get("encoding", "utf-8"), newline="") as f:
        reader = csv.reader(f)
        for row in reader:
            if not row:
                continue
            if row[0].strip().startswith("#"):
                continue
            rows.append([int(cell.strip()) for cell in row])
    return rows
