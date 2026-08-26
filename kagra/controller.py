"""Capsule character motor. GPU-free. Not Rapier.

``Walk`` owns one of these. Agents can also call ``wish`` / ``move`` /
``jump`` on ``Walk`` or on ``CharacterController`` directly.

Accel/decel live here so ``Physics3D`` can keep being a solver (capsules
skip ground friction; the motor owns stop). Sticky-walk quiet gap 3 is
input (``kagra-core``), not this file.
"""
from __future__ import annotations

import math
from typing import Optional


DEFAULT_ACCEL = 14.0
DEFAULT_DECEL = 22.0
DEFAULT_AIR_CONTROL = 0.38
# After physics, kill leftover collision kicks below this. Decel handles the rest.
IDLE_SNAP = 0.12


def jump_vy(
    on_ground: bool,
    in_water: bool,
    jump: float,
    *,
    coyote: bool = False,
) -> float | None:
    """ジャンプ／泳ぎの鉛直速度。しないなら None。``coyote`` は接地猶予。"""
    jump = float(jump)
    if jump <= 0.0:
        return None
    if in_water:
        return jump * 0.42
    if on_ground or coyote:
        return jump
    return None


def accelerate_xz(
    vx: float,
    vz: float,
    wish_x: float,
    wish_z: float,
    dt: float,
    *,
    accel: float = DEFAULT_ACCEL,
    decel: float = DEFAULT_DECEL,
    air: bool = False,
    air_control: float = DEFAULT_AIR_CONTROL,
) -> tuple[float, float]:
    """Move ``(vx, vz)`` toward wish. GPU-free. Not Rapier.

    Ground: ``accel`` toward wish, ``decel`` when idle or reversing.
    Air: both rates scale by ``air_control`` (keep takeoff speed, slight steer).
    """
    dt = max(float(dt), 0.0)
    vx, vz = float(vx), float(vz)
    wish_x, wish_z = float(wish_x), float(wish_z)
    scale = 1.0 if not air else max(0.0, float(air_control))
    a = max(0.0, float(accel)) * scale
    d = max(0.0, float(decel)) * scale
    wish_speed = math.hypot(wish_x, wish_z)
    cur_speed = math.hypot(vx, vz)
    if wish_speed < 1e-8:
        if cur_speed < 1e-8 or d <= 0.0 or dt <= 0.0:
            return (0.0, 0.0) if cur_speed < 1e-8 else (vx, vz)
        drop = d * dt
        if drop >= cur_speed:
            return 0.0, 0.0
        s = (cur_speed - drop) / cur_speed
        return vx * s, vz * s
    dx, dz = wish_x - vx, wish_z - vz
    gap = math.hypot(dx, dz)
    if gap < 1e-8:
        return wish_x, wish_z
    reversing = (vx * wish_x + vz * wish_z) < 0.0 and cur_speed > 0.15
    rate = (d if reversing else a) * dt
    if rate >= gap:
        return wish_x, wish_z
    t = rate / gap
    return vx + dx * t, vz + dz * t


class CharacterController:
    """Capsule motor: wish / move / jump + accel/decel.

    Does not step ``Physics3D`` — call ``apply`` then ``world.update``.
    ``Walk.update`` does that. Not Rapier.
    """

    def __init__(
        self,
        *,
        speed: float = 3.2,
        accel: float = DEFAULT_ACCEL,
        decel: float = DEFAULT_DECEL,
        jump: float = 0.0,
        coyote: float = 0.12,
        jump_buffer: float = 0.12,
        air_control: float = DEFAULT_AIR_CONTROL,
    ):
        self.speed = float(speed)
        self.accel = float(accel)
        self.decel = float(decel)
        self.jump = float(jump)
        self.coyote = float(coyote)
        self.jump_buffer = float(jump_buffer)
        self.air_control = float(air_control)
        self.wish_x = 0.0
        self.wish_z = 0.0
        self.on_ground = False
        self.landed = False
        self._jump_queued = False
        self._coyote_left = 0.0
        self._buffer_left = 0.0
        self._inited = False

    def wish(self, wx: float, wz: float) -> None:
        """World-space desired XZ velocity (m/s). Accel-limited in ``apply``."""
        self.wish_x = float(wx)
        self.wish_z = float(wz)

    def move(self, vx: float, vz: float) -> None:
        """Alias of ``wish`` — agents search for move/wish/jump."""
        self.wish(vx, vz)

    def jump_now(self) -> None:
        """Buffer a jump. Fires in ``apply`` if grounded / coyote / water."""
        self._jump_queued = True

    # ``jump`` is also the impulse height; keep a method name agents will search.
    def try_jump(self) -> None:
        self.jump_now()

    def apply(self, body, dt: float, *, in_water: bool = False) -> None:
        """Write ``body.vx`` / ``body.vz`` (and maybe ``vy``). No physics step."""
        dt = float(dt)
        try:
            body.controlled = True
        except Exception:
            pass
        grounded = bool(getattr(body, "on_ground", False))
        if self._inited:
            self.landed = bool(grounded and not self.on_ground)
        else:
            self.landed = False
            self._inited = True
        self.on_ground = grounded
        if grounded:
            self._coyote_left = self.coyote
        else:
            self._coyote_left = max(0.0, self._coyote_left - dt)
        if self._jump_queued:
            self._buffer_left = self.jump_buffer
            self._jump_queued = False
        else:
            self._buffer_left = max(0.0, self._buffer_left - dt)

        air = (not grounded) and (not in_water)
        body.vx, body.vz = accelerate_xz(
            float(body.vx), float(body.vz),
            self.wish_x, self.wish_z, dt,
            accel=self.accel, decel=self.decel,
            air=air, air_control=self.air_control,
        )
        if self._buffer_left > 0.0:
            vy = jump_vy(
                grounded,
                bool(in_water),
                self.jump,
                coyote=self._coyote_left > 0.0,
            )
            if vy is not None:
                body.vy = float(vy)
                self._buffer_left = 0.0
                self._coyote_left = 0.0
                body.on_ground = False
                self.on_ground = False
