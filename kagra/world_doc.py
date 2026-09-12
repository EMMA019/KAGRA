"""Pure-Python WorldDoc / WorldPlay when ``kagra_shared`` is not built.

``import kagra`` must succeed without the Rust extension (python-unit CI).
Tick / render stay on ``kagra_shared`` (``maturin develop``).
"""
from __future__ import annotations

import json
from typing import Any

WORLD_DUMP_VERSION = 1

__all__ = ["WORLD_DUMP_VERSION", "WorldDoc", "WorldPlay", "render_world_doc"]


class WorldDoc:
    """Dump JSON as data. Same names as ``kagra.kagra_shared.WorldDoc``."""

    def __init__(self, data: dict[str, Any]):
        self._data = data

    @classmethod
    def from_json(cls, raw: str) -> WorldDoc:
        data = json.loads(raw)
        if not isinstance(data, dict):
            raise ValueError("world dump must be an object")
        ver = int(data.get("version") or 0)
        if ver != WORLD_DUMP_VERSION:
            raise ValueError(
                f"unsupported world dump version {ver} (want {WORLD_DUMP_VERSION})"
            )
        return cls(data)

    def to_json(self) -> str:
        out = dict(self._data)
        if int(out.get("version") or 0) == 0:
            out["version"] = WORLD_DUMP_VERSION
        return json.dumps(out)

    def player_position(self) -> list[float] | None:
        player = self._data.get("player")
        if not isinstance(player, dict):
            return None
        pos = player.get("position")
        if not isinstance(pos, (list, tuple)) or len(pos) < 3:
            return None
        return [float(pos[0]), float(pos[1]), float(pos[2])]

    def props_named(self, name: str) -> list[tuple[str, list[float]]]:
        found: list[tuple[str, list[float]]] = []
        for prop in self._data.get("props") or []:
            if not isinstance(prop, dict):
                continue
            if prop.get("name") != name or prop.get("enabled") is False:
                continue
            pos = prop.get("position") or [0.0, 0.0, 0.0]
            if not isinstance(pos, (list, tuple)) or len(pos) < 3:
                continue
            pid = str(prop.get("id") or "")
            found.append((pid, [float(pos[0]), float(pos[1]), float(pos[2])]))
        return found


class WorldPlay:
    """JSON holder. Live tick needs ``kagra_shared``."""

    def __init__(self, doc: WorldDoc):
        self.doc = doc

    @classmethod
    def from_json(cls, raw: str) -> WorldPlay:
        return cls(WorldDoc.from_json(raw))

    def dump(self) -> str:
        return self.doc.to_json()

    def tick(self, dt: float) -> None:
        raise ImportError(_SHARED_HINT)

    def confirm(self) -> None:
        raise ImportError(_SHARED_HINT)

    def set_input(
        self,
        lx: float,
        lz: float,
        jump: bool,
        attack: bool,
        dodge: bool,
    ) -> None:
        raise ImportError(_SHARED_HINT)

    def emit_event(self, name: str, data_json: str | None = None) -> None:
        raise ImportError(_SHARED_HINT)

    def take_events(self, name: str) -> list[str]:
        raise ImportError(_SHARED_HINT)

    def start_timer(self, name: str, seconds: float, on_done: str | None = None) -> str:
        raise ImportError(_SHARED_HINT)

    def step_interact(self) -> None:
        raise ImportError(_SHARED_HINT)

    def anim(self) -> str:
        player = self.doc._data.get("player")
        if isinstance(player, dict):
            return str(player.get("anim") or "")
        return ""


def render_world_doc(*_a: Any, **_k: Any) -> bytes:
    raise ImportError(_SHARED_HINT)


_SHARED_HINT = (
    "kagra_shared not installed: `maturin develop --release` "
    "(root pyproject) or `cd kagra-shared && maturin develop --release`"
)
