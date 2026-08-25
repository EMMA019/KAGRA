"""Slope-float detector — per-frame foot vs terrain, JSONL for agents.

Physics-feel bugs that LOOK like they need video are often ``|foot_y - ground_y|``
while ``on_ground``. GPU-free. Rapier is not involved.
"""
from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any, Callable, Optional

DEFAULT_PATH = Path("scratch") / "debug_trace.jsonl"
DEFAULT_THRESHOLD = 0.05


def resolve_ground_y(
    x: float,
    z: float,
    *,
    ground_y: float | None = None,
    height_fn: Callable[[float, float], float] | None = None,
    world=None,
) -> Optional[float]:
    """Explicit ``ground_y``, else ``height_fn(x, z)``, else ``world.ground_y``."""
    if ground_y is not None:
        return float(ground_y)
    if height_fn is not None:
        return float(height_fn(float(x), float(z)))
    if world is not None and hasattr(world, "ground_y"):
        return float(world.ground_y(float(x), float(z)))
    return None


def append_jsonl(record: dict, path: str | Path | None = None) -> Path:
    dest = Path(path) if path is not None else DEFAULT_PATH
    dest.parent.mkdir(parents=True, exist_ok=True)
    with dest.open("a", encoding="utf-8") as f:
        f.write(json.dumps(record, ensure_ascii=False) + "\n")
    return dest


def format_spans(spans: list[tuple[int, int, float]]) -> str:
    """``frames 32-48 floated 0.15`` (peak |delta|). Empty → ``ok``."""
    if not spans:
        return "ok"
    parts = []
    for a, b, peak in spans:
        label = f"frame {a}" if a == b else f"frames {a}-{b}"
        parts.append(f"{label} floated {peak:.2f}")
    return "; ".join(parts)


class DebugTrace:
    """Accumulator. ``sample`` emits only over-threshold grounded frames."""

    def __init__(
        self,
        *,
        height_fn: Callable[[float, float], float] | None = None,
        world=None,
        threshold: float = DEFAULT_THRESHOLD,
        path: str | Path | None = None,
        persist: bool = True,
    ):
        self.height_fn = height_fn
        self.world = world
        self.threshold = float(threshold)
        self.path = Path(path) if path is not None else DEFAULT_PATH
        self.persist = bool(persist)
        self.hits: list[dict[str, Any]] = []
        self.frame = 0
        self._span_a: Optional[int] = None
        self._span_b: Optional[int] = None
        self._span_peak = 0.0
        self._spans: list[tuple[int, int, float]] = []

    def _close_span(self) -> None:
        if self._span_a is None or self._span_b is None:
            return
        self._spans.append((self._span_a, self._span_b, self._span_peak))
        self._span_a = self._span_b = None
        self._span_peak = 0.0

    def _extend_span(self, frame: int, delta: float) -> None:
        mag = abs(float(delta))
        if self._span_a is None:
            self._span_a = self._span_b = int(frame)
            self._span_peak = mag
            return
        if int(frame) == self._span_b + 1 or int(frame) == self._span_b:
            self._span_b = int(frame)
            if mag > self._span_peak:
                self._span_peak = mag
            return
        self._close_span()
        self._span_a = self._span_b = int(frame)
        self._span_peak = mag

    def sample(
        self,
        *,
        foot_y: float,
        x: float = 0.0,
        z: float = 0.0,
        ground_y: float | None = None,
        vx: float | None = None,
        vz: float | None = None,
        on_ground: bool | None = None,
        camera_distance: float | None = None,
        frame: int | None = None,
        timestamp: float | None = None,
    ) -> Optional[dict[str, Any]]:
        """One frame. Returns the JSON row if it was over threshold, else None."""
        self.frame += 1
        fr = self.frame if frame is None else int(frame)
        gy = resolve_ground_y(
            x, z, ground_y=ground_y, height_fn=self.height_fn, world=self.world,
        )
        if gy is None:
            return None
        delta = float(foot_y) - float(gy)
        if on_ground is False or abs(delta) <= self.threshold:
            self._close_span()
            return None
        rec: dict[str, Any] = {
            "frame": fr,
            "foot_y": float(foot_y),
            "ground_y": float(gy),
            "delta": float(delta),
            "x": float(x),
            "z": float(z),
            "timestamp": time.time() if timestamp is None else float(timestamp),
        }
        if on_ground is not None:
            rec["on_ground"] = bool(on_ground)
        if vx is not None:
            rec["vx"] = float(vx)
        if vz is not None:
            rec["vz"] = float(vz)
        if camera_distance is not None:
            rec["camera_distance"] = float(camera_distance)
        self.hits.append(rec)
        self._extend_span(fr, delta)
        if self.persist:
            rec["path"] = str(append_jsonl(
                {k: v for k, v in rec.items() if k != "path"},
                self.path,
            ))
        return rec

    def summary(self) -> str:
        """Compact run of floats, e.g. ``frames 32-48 floated 0.15``."""
        self._close_span()
        return format_spans(self._spans)


_ACTIVE: DebugTrace | None = None


def debug_trace(
    *,
    foot_y: float,
    x: float = 0.0,
    z: float = 0.0,
    ground_y: float | None = None,
    height_fn: Callable[[float, float], float] | None = None,
    world=None,
    vx: float | None = None,
    vz: float | None = None,
    on_ground: bool | None = None,
    camera_distance: float | None = None,
    threshold: float = DEFAULT_THRESHOLD,
    frame: int | None = None,
    path: str | Path | None = None,
    persist: bool = True,
    tracer: DebugTrace | None = None,
    reset: bool = False,
) -> Optional[dict[str, Any]]:
    """Record one physics-feel frame. Emits JSONL only over ``threshold``.

    Use ``DebugTrace`` when you need ``summary()``. ``reset=True`` starts a
    new default tracer. GPU-free; pass a fake ``height_fn`` in tests.
    """
    global _ACTIVE
    if reset or tracer is None and _ACTIVE is None:
        _ACTIVE = DebugTrace(
            height_fn=height_fn, world=world, threshold=threshold,
            path=path, persist=persist,
        )
    tr = tracer if tracer is not None else _ACTIVE
    assert tr is not None
    if height_fn is not None:
        tr.height_fn = height_fn
    if world is not None:
        tr.world = world
    if path is not None:
        tr.path = Path(path)
    tr.threshold = float(threshold)
    tr.persist = bool(persist)
    return tr.sample(
        foot_y=foot_y, x=x, z=z, ground_y=ground_y,
        vx=vx, vz=vz, on_ground=on_ground,
        camera_distance=camera_distance, frame=frame,
    )


def debug_trace_summary() -> str:
    """Summary of the default tracer. ``ok`` if nothing floated."""
    if _ACTIVE is None:
        return "ok"
    return _ACTIVE.summary()
