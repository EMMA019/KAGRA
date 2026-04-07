from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class AssetType(str, Enum):
    IMAGE = "image"
    FONT = "font"
    AUDIO = "audio"
    TILESET = "tileset"
    RIG = "rig"
    SCENE = "scene"
    PREFAB = "prefab"
    MAP = "map"
    JSON = "json"
    TEXT = "text"
    BINARY = "binary"


@dataclass(slots=True)
class AssetRecord:
    key: str
    asset_type: AssetType
    path: str
    loaded: Any = None
    metadata: dict[str, Any] = field(default_factory=dict)
    dependencies: list[str] = field(default_factory=list)
    load_count: int = 0
    dirty: bool = False

    @property
    def is_loaded(self) -> bool:
        return self.loaded is not None
