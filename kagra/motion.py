"""Tween / Sequence. GPU 不要。Rapier ではない。"""
from __future__ import annotations

import math
from typing import Any, Callable, Optional


def ease_smooth(t: float) -> float:
    t = 0.0 if t < 0.0 else 1.0 if t > 1.0 else float(t)
    return t * t * (3.0 - 2.0 * t)


def ease_linear(t: float) -> float:
    t = 0.0 if t < 0.0 else 1.0 if t > 1.0 else float(t)
    return t


_EASERS = {
    "smooth": ease_smooth,
    "smoothstep": ease_smooth,
    "linear": ease_linear,
}


class Tween:
    """1 属性を ``duration`` 秒で ``end`` まで。"""

    def __init__(
        self,
        obj: Any,
        attr: str,
        end: float,
        duration: float = 0.35,
        *,
        ease: str = "smooth",
        on_done: Optional[Callable[[], None]] = None,
    ):
        self.obj = obj
        self.attr = str(attr)
        self.start = float(getattr(obj, attr))
        self.end = float(end)
        self.duration = max(1e-4, float(duration))
        self.ease = _EASERS.get(str(ease), ease_smooth)
        self.on_done = on_done
        self.t = 0.0
        self.done = False

    def step(self, dt: float) -> bool:
        if self.done:
            return True
        self.t += float(dt)
        u = self.ease(self.t / self.duration)
        setattr(self.obj, self.attr, self.start + (self.end - self.start) * u)
        if self.t >= self.duration:
            setattr(self.obj, self.attr, self.end)
            self.done = True
            if self.on_done is not None:
                self.on_done()
            return True
        return False


class Sequence:
    """Tween を順に。``tick_animations`` が回す。"""

    def __init__(self, *tweens: Tween):
        self.tweens = list(tweens)
        self.i = 0
        self.done = not self.tweens

    def step(self, dt: float) -> bool:
        if self.done:
            return True
        while self.i < len(self.tweens):
            if not self.tweens[self.i].step(dt):
                return False
            self.i += 1
        self.done = True
        return True


_active: list[Tween | Sequence] = []


def animate(
    obj: Any,
    attr: str,
    end: float,
    duration: float = 0.35,
    *,
    ease: str = "smooth",
    on_done: Optional[Callable[[], None]] = None,
) -> Tween:
    """``obj.attr`` を ``end`` まで補間する。``Prop.update_all`` が回す。"""
    tw = Tween(obj, attr, end, duration, ease=ease, on_done=on_done)
    _active.append(tw)
    return tw


def sequence(*tweens: Tween) -> Sequence:
    seq = Sequence(*tweens)
    _active.append(seq)
    return seq


def tick_animations(dt: float) -> None:
    dt = float(dt)
    keep: list[Tween | Sequence] = []
    for item in _active:
        if not item.step(dt):
            keep.append(item)
    _active[:] = keep


def clear_animations() -> None:
    _active.clear()
