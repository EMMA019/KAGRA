"""Screen directions for a right-handed, Y-up world.

World +X is the right side of the frame only when the camera looks toward
-Z. The default camera (eye ``(0, 2, 6)``, target origin) does that. A chase
camera looking along +Z has ``screen_right = -X``.

``D`` / ``ArrowRight`` / ``+lx`` means screen-right, not world +X.
``screen_right`` on a world dump is the same vector, in world space.

2D HUD pixels start at the top-left: +x is right, +y is down.
glTF / VRM bind poses face -Z (the model's right hand is +X). A walker with
yaw 0 moves toward +Z, so yaw 0 shows that bind-pose front along -Z.
"""
from __future__ import annotations

from typing import Any

COORDINATE_NOTE = (
    "Right-handed, Y up. World +X is screen-right only when the camera "
    "looks toward -Z. The default camera sits at (0, 2, 6) looking at the "
    "origin, so +X is the right side of the frame and +Y is up. A chase "
    "camera looking along +Z (behind a yaw-0 actor; move-forward is +Z) has "
    "screen_right = -X and screen_left = +X. D, ArrowRight, and +lx mean "
    "screen-right, not world +X. Each camera's screen_right and screen_left "
    "are world-space unit vectors. 2D HUD pixels start at the top-left; +x "
    "is right and +y is down. glTF/VRM bind poses face -Z (the model's right "
    "hand is +X)."
)

__all__ = [
    "COORDINATE_NOTE",
    "annotate_dump",
    "screen_basis",
    "screen_left",
    "screen_right",
    "wish_world",
]


def _dot(a, b) -> float:
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def _cross(a, b):
    return (
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )


def _norm(v):
    length = _dot(v, v) ** 0.5
    if length < 1e-8:
        return (0.0, 0.0, 0.0)
    return (v[0] / length, v[1] / length, v[2] / length)


def _vec3(v, fallback) -> tuple[float, float, float]:
    if v is None:
        return fallback
    try:
        x, y, z = float(v[0]), float(v[1]), float(v[2])
    except (TypeError, IndexError, ValueError):
        return fallback
    return (x, y, z)


def screen_basis(eye, target, up=(0.0, 1.0, 0.0)):
    """``(forward, screen_right, screen_up)`` for a right-handed look-at.

    ``screen_right`` is ``forward × up``. Matches ``look_at_rh``: view-space
    +X is the right edge of the frame, and the screenshot path does not
    mirror X.
    """
    eye_v = _vec3(eye, (0.0, 2.0, 6.0))
    target_v = _vec3(target, (0.0, 0.0, 0.0))
    forward = _norm(
        (
            target_v[0] - eye_v[0],
            target_v[1] - eye_v[1],
            target_v[2] - eye_v[2],
        )
    )
    if forward == (0.0, 0.0, 0.0):
        forward = (0.0, 0.0, -1.0)
    up_v = _norm(_vec3(up, (0.0, 1.0, 0.0)))
    if up_v == (0.0, 0.0, 0.0):
        up_v = (0.0, 1.0, 0.0)
    right = _norm(_cross(forward, up_v))
    if right == (0.0, 0.0, 0.0):
        up_v = (0.0, 1.0, 0.0) if abs(forward[1]) < 0.9 else (1.0, 0.0, 0.0)
        right = _norm(_cross(forward, up_v))
    screen_up = _cross(right, forward)
    return forward, right, screen_up


def screen_right(eye, target, up=(0.0, 1.0, 0.0)):
    """World-space unit vector toward the right edge of the frame."""
    return screen_basis(eye, target, up)[1]


def screen_left(eye, target, up=(0.0, 1.0, 0.0)):
    """World-space unit vector toward the left edge of the frame."""
    right = screen_right(eye, target, up)
    return (-right[0], -right[1], -right[2])


def wish_world(lx: float, lz: float, eye, target, up=(0.0, 1.0, 0.0)):
    """Map a stick to a world XZ wish.

    ``+lx`` is screen-right and ``+lz`` is into the frame (camera forward,
    flattened onto the ground). Returns ``(x, z)``.
    """
    forward, right, _up = screen_basis(eye, target, up)
    flat_fwd = _norm((forward[0], 0.0, forward[2]))
    flat_right = _norm((right[0], 0.0, right[2]))
    if flat_fwd == (0.0, 0.0, 0.0):
        flat_fwd = (0.0, 0.0, -1.0)
    if flat_right == (0.0, 0.0, 0.0):
        flat_right = (1.0, 0.0, 0.0)
    x = flat_right[0] * float(lx) + flat_fwd[0] * float(lz)
    z = flat_right[2] * float(lx) + flat_fwd[2] * float(lz)
    return (x, z)


def annotate_dump(world: dict[str, Any]) -> dict[str, Any]:
    """Write ``coordinates`` and each camera's ``screen_left`` / ``screen_right``.

    Mutates ``world`` and returns it. Same fields as ``WorldDoc`` JSON, so a
    Python-built dump and a Rust ``dump()`` tell the same story.
    """
    if not isinstance(world, dict):
        return world
    world["coordinates"] = {
        "handedness": "right",
        "up": [0.0, 1.0, 0.0],
        "note": COORDINATE_NOTE,
    }
    cameras = world.get("cameras")
    if isinstance(cameras, list):
        for cam in cameras:
            if not isinstance(cam, dict):
                continue
            eye = cam.get("position") or (0.0, 2.0, 6.0)
            target = cam.get("target") or (0.0, 0.0, 0.0)
            right = screen_right(eye, target)
            left = screen_left(eye, target)
            cam["screen_right"] = [round(c, 6) for c in right]
            cam["screen_left"] = [round(c, 6) for c in left]
    return world
