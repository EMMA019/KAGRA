from __future__ import annotations
from typing import Callable, Optional, Any


class System:
    priority = 100

    def update(self, dt: float, world=None, scene=None):
        pass


class SystemScheduler:
    """Scene / EntityScene から systems を順番付きで実行する。"""
    def __init__(self):
        self._systems: list[System] = []

    def add(self, system: System) -> System:
        self._systems.append(system)
        self._systems.sort(key=lambda s: getattr(s, "priority", 100))
        return system

    def clear(self):
        self._systems.clear()

    def update(self, dt: float, world=None, scene=None):
        for s in self._systems:
            s.update(dt, world=world, scene=scene)


class AnimationSystem(System):
    priority = 40

    def update(self, dt: float, world=None, scene=None):
        if world is None:
            return
        try:
            from kagra.entity import AnimatorComponent
        except Exception:
            return
        for e in world.query(AnimatorComponent):
            comp = e.get_component(AnimatorComponent)
            if comp and comp.enabled:
                comp.update(dt)


class TimelineSystem(System):
    priority = 50

    def update(self, dt: float, world=None, scene=None):
        if scene is None:
            return
        players = getattr(scene, "_timeline_players", None)
        if not players:
            return
        for p in list(players):
            p.update(dt)
        scene._timeline_players = [p for p in players if not p.finished]


class CameraSystem(System):
    priority = 60

    def update(self, dt: float, world=None, scene=None):
        if scene is None:
            return
        follower = getattr(scene, "_cam_follower", None)
        mover = getattr(scene, "mover", None)
        if follower and mover:
            follower.follow(mover.center_x, mover.center_y, mover.w, mover.h)


class EffectSystem(System):
    priority = 70

    def update(self, dt: float, world=None, scene=None):
        try:
            import kagra
            eff = getattr(kagra, "effects", None)
            if eff:
                eff.update(dt)
        except Exception:
            pass
