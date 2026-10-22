"""Input-sync client for the Python bridge.

Rust (`kagra_shared.Lockstep`) owns quantization, the message JSON, and
the rule that a tick runs only when every player slot is present.
This module is the WebSocket transport plus the touch-half layout for
local versus on a phone.

Sockets are not part of `kagra_shared`, so Android, Windows, and wasm
can relay the same JSON with their own HTTP stack.
"""
from __future__ import annotations

import json
import math
import time
from dataclasses import dataclass
from typing import Any, Sequence

from kagra.wsproto import WsConn, connect


def _lockstep():
    try:
        from kagra.kagra_shared import Lockstep
    except ImportError:
        from kagra_shared import Lockstep  # type: ignore[no-redef]
    return Lockstep


@dataclass(frozen=True)
class HalfPad:
    """One player's touch controls, in window pixels. Origin is top-left."""

    stick: tuple[float, float, float]
    attack: tuple[float, float, float, float]
    block: tuple[float, float, float, float]


def half_pads(width: float, height: float) -> tuple[HalfPad, HalfPad]:
    """Left half is player 0, right half is player 1.

    Sticks sit on the outer bottom corners. Attack and block sit toward
    the center so two thumbs on a landscape phone do not cross.
    """
    width = float(width)
    height = float(height)
    mid = width * 0.5
    radius = min(58.0, max(28.0, width * 0.08))
    stick_y = height - max(72.0, height * 0.18)
    by = height - 64.0
    p0 = HalfPad(
        stick=(max(radius + 16.0, width * 0.12), stick_y, radius),
        attack=(mid - 168.0, by, 70.0, 48.0),
        block=(mid - 88.0, by, 70.0, 48.0),
    )
    p1 = HalfPad(
        stick=(width - max(radius + 16.0, width * 0.12), stick_y, radius),
        block=(mid + 18.0, by, 70.0, 48.0),
        attack=(mid + 98.0, by, 70.0, 48.0),
    )
    return p0, p1


def _pointer(item: Any) -> tuple[float, float, str]:
    if isinstance(item, dict):
        phase = item.get("phase", "move")
        return float(item["x"]), float(item["y"]), str(phase)
    phase = getattr(item, "phase", "move")
    phase = getattr(phase, "value", phase)
    return float(item.x), float(item.y), str(phase)


def _in_rect(rect: tuple[float, float, float, float], x: float, y: float) -> bool:
    rx, ry, rw, rh = rect
    return rx <= x <= rx + rw and ry <= y <= ry + rh


def halves_input(
    width: float,
    height: float,
    pointers: Sequence[Any],
) -> tuple[tuple[float, float, bool, bool], tuple[float, float, bool, bool]]:
    """Map pointers to ``(lx, lz, attack, block)`` for player 0 and player 1.

    ``x < width/2`` is player 0. ``phase`` of ``end`` or ``cancel`` is up.
    """
    pads = half_pads(width, height)
    out = [(0.0, 0.0, False, False), (0.0, 0.0, False, False)]
    mid = float(width) * 0.5
    for item in pointers:
        x, y, phase = _pointer(item)
        if phase in ("end", "cancel"):
            continue
        slot = 0 if x < mid else 1
        pad = pads[slot]
        lx, lz, attack, block = out[slot]
        cx, cy, radius = pad.stick
        if radius > 0.0 and math.hypot(x - cx, y - cy) <= radius + 8.0:
            lx = max(-1.0, min(1.0, (x - cx) / radius))
            lz = max(-1.0, min(1.0, -(y - cy) / radius))
        attack = attack or _in_rect(pad.attack, x, y)
        block = block or _in_rect(pad.block, x, y)
        out[slot] = (lx, lz, attack, block)
    return out[0], out[1]


def scripted_stick(slot: int, tick: int) -> tuple[float, float, bool, bool, bool]:
    """Shared demo stick. Both clients call this for their own slot only.

    Returns ``(lx, lz, jump, attack, dodge)``.
    """
    slot = int(slot)
    tick = int(tick)
    if tick < 40:
        lx = 1.0 if slot == 0 else -1.0
        return lx, 0.0, False, False, False
    attack = slot == 0 and (tick % 18) < 3
    guard = slot == 1 and 48 <= tick < 58
    lx = 0.2 if slot == 0 else -0.2
    return lx, 0.0, False, attack, guard


class RoomClient:
    """Join one relay room and feed a `FightSession` from lockstep inputs."""

    def __init__(self, host: str, port: int, room: str, name: str) -> None:
        self.host = host
        self.port = int(port)
        self.room = room
        self.name = name
        self.conn: WsConn | None = None
        self.lockstep: Any = None
        self.slot: int | None = None
        self.dt: float | None = None
        self.started = False
        self.peer_left = False

    def connect(self, timeout: float = 5.0) -> None:
        lockstep = _lockstep()
        self.conn = connect(self.host, self.port, timeout=timeout)
        self.conn.send_text(lockstep.hello(self.room, self.name))
        deadline = time.monotonic() + timeout
        while self.slot is None:
            if time.monotonic() > deadline:
                raise TimeoutError("welcome timed out")
            self.pump(min(0.2, max(0.01, deadline - time.monotonic())))

    def wait_start(self, timeout: float = 5.0) -> None:
        deadline = time.monotonic() + timeout
        while not self.started:
            if time.monotonic() > deadline:
                raise TimeoutError("start timed out")
            self.pump(min(0.2, max(0.01, deadline - time.monotonic())))

    def push_local(
        self,
        tick: int,
        lx: float,
        lz: float,
        jump: bool,
        attack: bool,
        dodge: bool,
    ) -> None:
        if self.conn is None or self.lockstep is None:
            raise RuntimeError("not connected")
        raw = self.lockstep.submit_local(int(tick), lx, lz, jump, attack, dodge)
        self.conn.send_text(raw)

    def ready(self, tick: int) -> bool:
        return bool(self.lockstep and self.lockstep.ready(int(tick)))

    def inputs(self, tick: int):
        if self.lockstep is None:
            return None
        return self.lockstep.inputs(int(tick))

    def pump(self, timeout: float = 0.0) -> None:
        if self.conn is None:
            return
        text = self.conn.recv_text(timeout)
        if text is None:
            return
        self._handle(text)
        while True:
            more = self.conn.recv_text(0.0)
            if more is None:
                break
            self._handle(more)

    def close(self) -> None:
        if self.conn is not None:
            try:
                self.conn.sock.close()
            except OSError:
                pass
            self.conn = None

    def _handle(self, text: str) -> None:
        lockstep = _lockstep()
        msg = json.loads(lockstep.decode(text))
        op = msg.get("op")
        if op == "welcome":
            self.slot = int(msg["slot"])
            self.lockstep = lockstep(self.slot, 2)
        elif op == "start":
            self.dt = float(lockstep.dt(int(msg["dt_num"]), int(msg["dt_den"])))
            self.started = True
        elif op == "input":
            if self.lockstep is None:
                raise RuntimeError("input before welcome")
            self.lockstep.ingest(text)
        elif op == "bye":
            reason = str(msg.get("reason") or "bye")
            # Once the match has started, a peer can finish its last tick
            # and close while this socket still has that tick's input queued
            # ahead of "peer left". Record it; the match loop fails only
            # when a tick is still missing.
            if self.slot is None or not self.started:
                raise ConnectionError(reason)
            self.peer_left = True
