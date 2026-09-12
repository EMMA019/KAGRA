"""WorldDoc / WorldPlay when ``kagra_shared`` (Rust) is not installed.

``pytest tests -m "not golden"`` and the python-unit CI job stay
extension-free. Dump JSON still roundtrips. Tick / render need the
shared crate (``maturin develop`` / the wheel).
"""
from __future__ import annotations

import json
from typing import Any

WORLD_DUMP_VERSION = 1

__all__ = ["WorldDoc", "WorldPlay", "render_world_doc"]


def _need_shared(what: str) -> ImportError:
    return ImportError(
        f"{what} needs kagra_shared: `maturin develop --release` "
        "(root pyproject) or `cd kagra-shared && maturin develop --release`"
    )


class WorldDoc:
    """Dump JSON in / out. Same entry as the Rust type; no tick, no GPU."""

    def __init__(self, data: dict[str, Any]):
        self._data = data

    @staticmethod
    def from_json(text: str) -> WorldDoc:
        data = json.loads(text)
        if not isinstance(data, dict):
            raise ValueError("world dump must be an object")
        ver = data.get("version")
        if ver != WORLD_DUMP_VERSION:
            raise ValueError(f"unsupported world dump version: {ver!r}")
        return WorldDoc(data)

    def to_json(self) -> str:
        return json.dumps(self._data, ensure_ascii=False)

    def player_position(self) -> list[float] | None:
        player = self._data.get("player") or {}
        pos = player.get("position")
        if not isinstance(pos, (list, tuple)) or len(pos) < 3:
            return None
        return [float(pos[0]), float(pos[1]), float(pos[2])]

    def props_named(self, name: str) -> list[tuple[str, list[float]]]:
        out: list[tuple[str, list[float]]] = []
        want = str(name)
        for prop in self._data.get("props") or []:
            if not isinstance(prop, dict):
                continue
            if prop.get("name") != want or not prop.get("enabled", True):
                continue
            pos = prop.get("position") or [0.0, 0.0, 0.0]
            pid = str(prop.get("id") or "")
            if not isinstance(pos, (list, tuple)) or len(pos) < 3:
                continue
            out.append((pid, [float(pos[0]), float(pos[1]), float(pos[2])]))
        return out


class WorldPlay:
    """Holds a dump. ``tick`` / input need the Rust ``WorldPlay``."""

    def __init__(self, doc: WorldDoc):
        self._doc = doc

    @staticmethod
    def from_json(text: str) -> WorldPlay:
        return WorldPlay(WorldDoc.from_json(text))

    def dump(self) -> str:
        return self._doc.to_json()

    def tick(self, dt: float) -> None:
        raise _need_shared("WorldPlay.tick")

    def confirm(self) -> None:
        raise _need_shared("WorldPlay.confirm")

    def set_input(
        self,
        lx: float,
        lz: float,
        jump: bool,
        attack: bool,
        dodge: bool,
    ) -> None:
        raise _need_shared("WorldPlay.set_input")

    def emit_event(self, name: str, data_json: str | None = None) -> None:
        raise _need_shared("WorldPlay.emit_event")

    def take_events(self, name: str) -> list[str]:
        raise _need_shared("WorldPlay.take_events")

    def start_timer(
        self,
        name: str,
        seconds: float,
        on_done: str | None = None,
    ) -> str:
        raise _need_shared("WorldPlay.start_timer")

    def step_interact(self) -> None:
        raise _need_shared("WorldPlay.step_interact")

    def anim(self) -> str:
        player = self._doc._data.get("player") or {}
        return str(player.get("anim") or "")


def render_world_doc(*_a, **_k):
    raise _need_shared("render_world_doc")
