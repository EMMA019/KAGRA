"""Ribbon of emissive spheres behind a moving point.

The shared renderer has no GPU trail pass. ``Trail.props`` is a list of
sphere props a dump can append. ``emissive`` above 0 is the shared HDR add
(``WorldProp.emissive``) so bloom picks the streak up. ``enabled`` False
emits nothing.
"""
from __future__ import annotations

from collections import deque


class Trail:
    """A short tail. ``push`` records a point, ``props`` is the draw list."""

    def __init__(
        self,
        length: int = 16,
        *,
        width: float = 0.18,
        color: tuple[int, int, int] = (80, 230, 255),
        emissive: float = 3.2,
        name: str = "trail",
        enabled: bool = True,
    ):
        self.length = max(0, int(length))
        self.width = max(0.02, float(width))
        self.color = (int(color[0]), int(color[1]), int(color[2]))
        self.emissive = max(0.0, float(emissive))
        self.name = str(name)
        self.enabled = bool(enabled)
        self._points: deque[tuple[float, float, float]] = deque()

    def clear(self) -> None:
        self._points.clear()

    def __len__(self) -> int:
        return len(self._points)

    def push(self, position: tuple[float, float, float]) -> None:
        if not self.enabled or self.length <= 0:
            return
        self._points.append((float(position[0]), float(position[1]), float(position[2])))
        while len(self._points) > self.length:
            self._points.popleft()

    def props(self) -> list[dict]:
        """Tapered spheres, oldest first. Empty when disabled."""
        if not self.enabled or not self._points:
            return []
        n = len(self._points)
        out: list[dict] = []
        for i, (x, y, z) in enumerate(self._points):
            t = 1.0 if n <= 1 else i / (n - 1)
            s = self.width * (0.28 + 0.72 * t)
            gain = self.emissive * (0.3 + 0.7 * t)
            item = {
                "id": f"prop:{self.name}-{i}",
                "type": "prop",
                "name": self.name,
                "position": [x, y, z],
                "yaw": 0.0,
                "model": "sphere",
                "scale": [s, s, s * 1.6],
                "enabled": True,
                "color": [self.color[0], self.color[1], self.color[2]],
                "metallic": 0.0,
                "roughness": 0.3,
                "collider": "none",
                "is_static": True,
            }
            if gain > 0.0:
                item["emissive"] = gain
            out.append(item)
        return out
