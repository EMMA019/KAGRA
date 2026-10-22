"""CPU particle emitter. Draws as small sphere props on a WorldDoc dump.

No GPU particle pass: WebGL2 stays on the existing instanced box/sphere
pipeline, and a phone caps the list at ``max_count``. ``enabled`` False
emits nothing. The sim should not keep these props — append them to the
draw dump only, so they never become physics bodies.
"""
from __future__ import annotations

import math
import random
from dataclasses import dataclass


@dataclass
class _Part:
    x: float
    y: float
    z: float
    vx: float
    vy: float
    vz: float
    life: float
    max_life: float
    size: float
    r: int
    g: int
    b: int
    gravity: float


class Emitter:
    """A burst list. ``burst`` spawns, ``tick`` integrates, ``props`` is dump props."""

    def __init__(self, max_count: int = 48, *, seed: int = 1, enabled: bool = True, name: str = "spark"):
        self.max_count = max(0, int(max_count))
        self.enabled = bool(enabled)
        self.name = str(name)
        self._rng = random.Random(int(seed))
        self._parts: list[_Part] = []

    def clear(self) -> None:
        self._parts.clear()

    def __len__(self) -> int:
        return len(self._parts)

    def burst(
        self,
        position: tuple[float, float, float],
        *,
        count: int = 8,
        color: tuple[int, int, int] = (255, 220, 120),
        speed: float = 2.2,
        life: float = 0.45,
        size: float = 0.16,
        gravity: float = -8.0,
        upward: float = 1.4,
    ) -> int:
        """Spawn up to ``count`` particles. Oldest are dropped past ``max_count``."""
        if not self.enabled or self.max_count <= 0 or count <= 0:
            return 0
        x0, y0, z0 = float(position[0]), float(position[1]), float(position[2])
        spawned = 0
        for _ in range(int(count)):
            if len(self._parts) >= self.max_count:
                self._parts.pop(0)
            ang = self._rng.random() * math.tau
            rad = self._rng.random()
            spd = float(speed) * (0.45 + self._rng.random())
            vx = math.cos(ang) * spd * rad
            vz = math.sin(ang) * spd * rad
            vy = float(upward) * (0.4 + self._rng.random()) + float(speed) * (1.0 - rad) * 0.35
            span = float(life) * (0.65 + 0.35 * self._rng.random())
            self._parts.append(
                _Part(
                    x=x0,
                    y=y0,
                    z=z0,
                    vx=vx,
                    vy=vy,
                    vz=vz,
                    life=0.0,
                    max_life=max(1e-3, span),
                    size=max(0.02, float(size)),
                    r=int(color[0]),
                    g=int(color[1]),
                    b=int(color[2]),
                    gravity=float(gravity),
                )
            )
            spawned += 1
        return spawned

    def tick(self, dt: float) -> None:
        dt = max(0.0, float(dt))
        if not self.enabled or dt <= 0.0:
            return
        alive: list[_Part] = []
        for p in self._parts:
            p.life += dt
            if p.life >= p.max_life:
                continue
            p.vy += p.gravity * dt
            p.x += p.vx * dt
            p.y += p.vy * dt
            p.z += p.vz * dt
            alive.append(p)
        self._parts = alive

    def props(self) -> list[dict]:
        """Sphere props the shared renderer already draws. Empty when disabled."""
        if not self.enabled:
            return []
        out: list[dict] = []
        for i, p in enumerate(self._parts):
            age = 0.0 if p.max_life <= 0.0 else p.life / p.max_life
            fade = max(0.0, 1.0 - age)
            s = max(0.03, p.size * (0.45 + 0.55 * fade))
            out.append(
                {
                    "id": f"prop:{self.name}-{i}",
                    "type": "prop",
                    "name": self.name,
                    "position": [p.x, p.y, p.z],
                    "yaw": 0.0,
                    "model": "sphere",
                    "scale": [s, s, s],
                    "enabled": True,
                    "color": [p.r, p.g, p.b],
                    "metallic": 0.0,
                    "roughness": 0.4,
                    "collider": "none",
                    "is_static": True,
                }
            )
        return out
