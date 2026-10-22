"""Screen juice for phone games. Pure Python, no GPU.

``Juice`` owns shake, hitstop, squash/stretch, and a small easer set.
``apply_post`` writes the dump fields the shared renderer already understands
(bloom, vignette, LUT, FXAA). Omit those fields and the picture stays the
legacy one: bloom off, vignette at the old curve, LUT off, FXAA on.

Call ``tick`` with the real frame dt. Pass the returned value to the sim so
hitstop freezes the world while the shake keeps moving.
"""
from __future__ import annotations

import math
from typing import Any, Callable, Optional


def _clamp01(t: float) -> float:
    t = float(t)
    if t < 0.0:
        return 0.0
    if t > 1.0:
        return 1.0
    return t


def ease_linear(t: float) -> float:
    return _clamp01(t)


def ease_smooth(t: float) -> float:
    t = _clamp01(t)
    return t * t * (3.0 - 2.0 * t)


def ease_in_quad(t: float) -> float:
    t = _clamp01(t)
    return t * t


def ease_out_quad(t: float) -> float:
    t = _clamp01(t)
    return t * (2.0 - t)


def ease_in_out_quad(t: float) -> float:
    t = _clamp01(t)
    if t < 0.5:
        return 2.0 * t * t
    return -1.0 + (4.0 - 2.0 * t) * t


def ease_in_cubic(t: float) -> float:
    t = _clamp01(t)
    return t * t * t


def ease_out_cubic(t: float) -> float:
    t = _clamp01(t)
    u = 1.0 - t
    return 1.0 - u * u * u


def ease_in_out_cubic(t: float) -> float:
    t = _clamp01(t)
    if t < 0.5:
        return 4.0 * t * t * t
    u = 2.0 * t - 2.0
    return 0.5 * u * u * u + 1.0


def ease_out_back(t: float) -> float:
    t = _clamp01(t)
    c1 = 1.70158
    c3 = c1 + 1.0
    u = t - 1.0
    return 1.0 + c3 * u * u * u + c1 * u * u


def ease_out_bounce(t: float) -> float:
    t = _clamp01(t)
    n1 = 7.5625
    d1 = 2.75
    if t < 1.0 / d1:
        return n1 * t * t
    if t < 2.0 / d1:
        t -= 1.5 / d1
        return n1 * t * t + 0.75
    if t < 2.5 / d1:
        t -= 2.25 / d1
        return n1 * t * t + 0.9375
    t -= 2.625 / d1
    return n1 * t * t + 0.984375


def ease_out_elastic(t: float) -> float:
    t = _clamp01(t)
    if t <= 0.0 or t >= 1.0:
        return t
    c4 = (2.0 * math.pi) / 3.0
    return math.pow(2.0, -10.0 * t) * math.sin((t * 10.0 - 0.75) * c4) + 1.0


_EASERS = {
    "linear": ease_linear,
    "smooth": ease_smooth,
    "smoothstep": ease_smooth,
    "in_quad": ease_in_quad,
    "out_quad": ease_out_quad,
    "in_out_quad": ease_in_out_quad,
    "in_cubic": ease_in_cubic,
    "out_cubic": ease_out_cubic,
    "in_out_cubic": ease_in_out_cubic,
    "out_back": ease_out_back,
    "out_bounce": ease_out_bounce,
    "out_elastic": ease_out_elastic,
}


def ease(name: str, t: float) -> float:
    """Evaluate a named curve. Unknown names use smoothstep."""
    fn = _EASERS.get(str(name), ease_smooth)
    return fn(t)


# Dump fields for the shared post stack. ``legacy`` matches a dump that omits
# them: bloom off, the historical vignette curve, no LUT, FXAA on.
POST_PRESETS = {
    "legacy": {
        "bloom": 0.0,
        "bloom_threshold": 0.85,
        "vignette": 1.0,
        "lut": "off",
        "lut_strength": 0.0,
        "fxaa": True,
    },
    "off": {
        "bloom": 0.0,
        "bloom_threshold": 0.85,
        "vignette": 0.0,
        "lut": "off",
        "lut_strength": 0.0,
        "fxaa": False,
    },
    "mobile": {
        "bloom": 0.48,
        "bloom_threshold": 0.58,
        "vignette": 1.25,
        "lut": "punch",
        "lut_strength": 0.9,
        "fxaa": True,
    },
}


def apply_post(world: dict, preset: str = "mobile", **overrides: Any) -> dict:
    """Write post fields onto a dump dict. Returns the same dict."""
    spec = dict(POST_PRESETS.get(str(preset), POST_PRESETS["mobile"]))
    spec.update(overrides)
    world["bloom"] = float(spec["bloom"])
    world["bloom_threshold"] = float(spec["bloom_threshold"])
    world["vignette"] = float(spec["vignette"])
    world["lut"] = str(spec["lut"])
    world["lut_strength"] = float(spec["lut_strength"])
    world["fxaa"] = bool(spec["fxaa"])
    return world


class Tween:
    """One attribute, ``duration`` seconds, named ease."""

    def __init__(
        self,
        obj: Any,
        attr: str,
        end: float,
        duration: float = 0.35,
        *,
        ease: str = "out_cubic",
        on_done: Optional[Callable[[], None]] = None,
    ):
        self.obj = obj
        self.attr = str(attr)
        self.start = float(getattr(obj, attr))
        self.end = float(end)
        self.duration = max(1e-4, float(duration))
        self.ease_name = str(ease)
        self.on_done = on_done
        self.t = 0.0
        self.done = False

    def step(self, dt: float) -> bool:
        if self.done:
            return True
        self.t += float(dt)
        u = ease(self.ease_name, self.t / self.duration)
        setattr(self.obj, self.attr, self.start + (self.end - self.start) * u)
        if self.t + 1e-4 >= self.duration:
            setattr(self.obj, self.attr, self.end)
            self.done = True
            if self.on_done is not None:
                self.on_done()
            return True
        return False


class Juice:
    """Shake, hitstop, squash/stretch, and tweens for one scene.

    ``enabled`` False makes ``tick`` a pass-through and ``apply_world`` a no-op.
    """

    def __init__(self) -> None:
        self.enabled = True
        self._sx = 1.0
        self._sy = 1.0
        self._sz = 1.0
        self._pulse: Optional[dict[str, float]] = None
        self._shake_t = 0.0
        self._shake_dur = 0.0
        self._shake_amp = 0.0
        self._shake_freq = 28.0
        self._hit = 0.0
        self._tweens: list[Tween] = []

    def shake(self, amplitude: float = 0.08, duration: float = 0.18, frequency: float = 28.0) -> None:
        """Camera offset in meters. Decays with the square of remaining time."""
        self._shake_amp = max(0.0, float(amplitude))
        self._shake_dur = max(1e-4, float(duration))
        self._shake_freq = max(0.1, float(frequency))
        self._shake_t = 0.0

    def hitstop(self, seconds: float = 0.05) -> None:
        """Hold the sim clock. Feel clocks (shake, squash) keep running."""
        self._hit = max(self._hit, max(0.0, float(seconds)))

    def squash(self, amount: float = 0.22, duration: float = 0.18) -> None:
        """Dip Y, widen XZ, then settle. ``amount`` 0.22 → about 0.78 tall."""
        self._pulse = {
            "amount": -max(0.0, min(0.65, float(amount))),
            "duration": max(1e-3, float(duration)),
            "t": 0.0,
        }

    def stretch(self, amount: float = 0.18, duration: float = 0.14) -> None:
        """Stretch Y, narrow XZ, then settle."""
        self._pulse = {
            "amount": max(0.0, min(0.65, float(amount))),
            "duration": max(1e-3, float(duration)),
            "t": 0.0,
        }

    def tween(
        self,
        obj: Any,
        attr: str,
        end: float,
        duration: float = 0.35,
        *,
        ease: str = "out_cubic",
        on_done: Optional[Callable[[], None]] = None,
    ) -> Tween:
        tw = Tween(obj, attr, end, duration, ease=ease, on_done=on_done)
        self._tweens.append(tw)
        return tw

    def scale(self) -> tuple[float, float, float]:
        return (self._sx, self._sy, self._sz)

    def camera_offset(self) -> tuple[float, float, float]:
        if not self.enabled or self._shake_dur <= 1e-6 or self._shake_t >= self._shake_dur:
            return (0.0, 0.0, 0.0)
        u = 1.0 - (self._shake_t / self._shake_dur)
        amp = self._shake_amp * u * u
        phase = self._shake_t * self._shake_freq
        return (
            math.sin(phase * 17.0) * amp,
            math.sin(phase * 23.0) * amp * 0.55,
            math.sin(phase * 11.0) * amp * 0.25,
        )

    def tick(self, dt: float) -> float:
        """Advance feel clocks. Returns the dt the sim should consume."""
        dt = max(0.0, float(dt))
        if not self.enabled:
            return dt
        self._advance(dt)
        if self._hit > 0.0:
            if dt < self._hit:
                self._hit -= dt
                return 0.0
            rest = dt - self._hit
            self._hit = 0.0
            return rest
        return dt

    def apply_world(self, world: dict) -> dict:
        """Add the shake to camera 0 and write walker scale. One call per fresh dump."""
        if not self.enabled:
            return world
        ox, oy, oz = self.camera_offset()
        cams = world.get("cameras") or []
        if cams and isinstance(cams[0], dict):
            cam = cams[0]
            pos = cam.get("position")
            if isinstance(pos, (list, tuple)) and len(pos) >= 3:
                cam["position"] = [float(pos[0]) + ox, float(pos[1]) + oy, float(pos[2]) + oz]
            tgt = cam.get("target")
            if isinstance(tgt, (list, tuple)) and len(tgt) >= 3:
                cam["target"] = [float(tgt[0]) + ox, float(tgt[1]) + oy, float(tgt[2]) + oz]
        sc = [self._sx, self._sy, self._sz]
        player = world.get("player")
        if isinstance(player, dict):
            player["scale"] = list(sc)
            pid = player.get("id")
        else:
            pid = None
        for walk in world.get("walkers") or []:
            if not isinstance(walk, dict):
                continue
            if pid is not None and walk.get("id") == pid:
                walk["scale"] = list(sc)
            elif walk.get("name") == "player":
                walk["scale"] = list(sc)
        return world

    def _advance(self, dt: float) -> None:
        if self._shake_dur > 0.0 and self._shake_t < self._shake_dur:
            self._shake_t += dt
        if self._pulse is not None:
            pulse = self._pulse
            pulse["t"] += dt
            dur = pulse["duration"]
            u = 1.0 if dur <= 1e-6 else min(1.0, pulse["t"] / dur)
            if u < 0.35:
                k = u / 0.35
                k = k * k
            else:
                k = 1.0 - (u - 0.35) / 0.65
                k = 0.0 if k < 0.0 else k
            sy = 1.0 + pulse["amount"] * k
            if sy < 0.35:
                sy = 0.35
            sx = 1.0 / math.sqrt(sy)
            self._sx = sx
            self._sy = sy
            self._sz = sx
            if u >= 1.0:
                self._sx = self._sy = self._sz = 1.0
                self._pulse = None
        if self._tweens:
            self._tweens = [tw for tw in self._tweens if not tw.step(dt)]
