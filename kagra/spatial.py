"""Listener-relative gain and stereo pan. GPU-free. Keep in sync with old/kagra-core/src/audio.rs."""
from __future__ import annotations

import math

_EPS = 1e-8


def _sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def _dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def _cross(a, b):
    return (
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )


def _norm(v):
    length = math.sqrt(_dot(v, v))
    if length < _EPS:
        return (0.0, 0.0, 0.0)
    return (v[0] / length, v[1] / length, v[2] / length)


def spatial_mix(
    lx: float,
    ly: float,
    lz: float,
    fx: float,
    fy: float,
    fz: float,
    sx: float,
    sy: float,
    sz: float,
    *,
    ref_distance: float = 4.0,
    max_distance: float = 48.0,
    ux: float = 0.0,
    uy: float = 1.0,
    uz: float = 0.0,
) -> tuple[float, float, float, float]:
    """Inverse-distance gain + equal-power stereo pan.

    Returns ``(gain, pan, left, right)``. ``pan`` is -1 (left) … +1 (right).
    Listener right is ``up × forward`` so look +Z makes world +X the right
    speaker (Crest Isle sea at -X is the left speaker). No HRTF.
    """
    ref_d = max(1e-4, float(ref_distance))
    max_d = max(ref_d, float(max_distance))
    to = _sub((sx, sy, sz), (lx, ly, lz))
    dist = math.sqrt(_dot(to, to))
    if dist >= max_d:
        gain = 0.0
    else:
        gain = min(1.0, ref_d / max(dist, ref_d))
    pan = 0.0
    if dist > 1e-6 and gain > 0.0:
        fwd = _norm((fx, fy, fz))
        up = _norm((ux, uy, uz))
        right = _norm(_cross(up, fwd))
        direction = _norm(to)
        pan = max(-1.0, min(1.0, _dot(direction, right)))
    angle = (pan + 1.0) * (math.pi * 0.25)
    left = gain * math.cos(angle)
    right = gain * math.sin(angle)
    return gain, pan, left, right
