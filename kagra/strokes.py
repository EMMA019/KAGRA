"""Dump records for the shared GPU point cloud and fat line.

``Emitter`` and ``Trail`` stay CPU sphere and quad props. These helpers
only build the ``points`` and ``lines`` arrays on a world dump. The
renderer expands them. Nothing here imports the Rust extension.
"""

from __future__ import annotations


def _rgba(color: tuple[int, ...]) -> list[int]:
    if len(color) < 3:
        raise ValueError("color needs r, g, b")
    r, g, b = (int(color[0]), int(color[1]), int(color[2]))
    a = int(color[3]) if len(color) > 3 else 255
    return [r, g, b, a]


def _xyz(points) -> list[list[float]]:
    out: list[list[float]] = []
    for p in points:
        out.append([float(p[0]), float(p[1]), float(p[2])])
    return out


def point_cloud(
    positions,
    *,
    color: tuple[int, ...] = (255, 255, 255, 255),
    size: float = 0.06,
    id: str = "points",
) -> dict:
    """One camera-facing point sprite cloud. ``size`` is meters."""
    return {
        "id": str(id),
        "positions": _xyz(positions),
        "color": _rgba(color),
        "size": float(size),
    }


def fat_line(
    points,
    *,
    color: tuple[int, ...] = (255, 255, 255, 255),
    width: float = 3.0,
    id: str = "line",
) -> dict:
    """One screen-space polyline. ``width`` is pixels."""
    return {
        "id": str(id),
        "points": _xyz(points),
        "color": _rgba(color),
        "width": float(width),
    }
