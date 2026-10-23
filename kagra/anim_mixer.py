"""Named glTF clips with crossfades for walker dicts.

Python twin of ``kagra-shared/src/anim_mixer.rs``. Idea after three.js
``AnimationMixer`` / ``AnimationAction.crossFadeTo`` (three.js, MIT License):
the new clip fades in while the old one fades out, both clocks keep running.

    mixer = AnimMixer()
    mixer.play("Idle")
    mixer.cross_fade_to("Run", 0.25)
    mixer.update(dt)
    walker.update(mixer.fields())   # clip_name / clip / fade_from / ...
"""

from __future__ import annotations

from typing import Any

__all__ = ["AnimMixer"]


class AnimMixer:
    def __init__(self, time_scale: float = 1.0) -> None:
        self.current: str | None = None
        self.time = 0.0
        self.previous: str | None = None
        self.previous_time = 0.0
        self.fade_duration = 0.0
        self.fade_elapsed = 0.0
        self.time_scale = float(time_scale)

    def play(self, name: str) -> None:
        """Hard switch to ``name``. Re-playing the current clip is a no-op."""
        if name == self.current:
            return
        self.current, self.time = name, 0.0
        self.previous, self.previous_time = None, 0.0
        self.fade_duration = self.fade_elapsed = 0.0

    def cross_fade_to(self, name: str, duration: float) -> None:
        """Fade from the current clip to ``name`` over ``duration`` seconds."""
        if name == self.current:
            return
        if duration <= 0.0 or self.current is None:
            self.play(name)
            return
        self.previous, self.previous_time = self.current, self.time
        self.current, self.time = name, 0.0
        self.fade_duration, self.fade_elapsed = float(duration), 0.0

    def update(self, dt: float) -> None:
        dt = max(0.0, float(dt))
        self.time += dt * self.time_scale
        if self.previous is not None:
            self.previous_time += dt * self.time_scale
            self.fade_elapsed += dt
            if self.fade_elapsed >= self.fade_duration:
                self.previous, self.previous_time = None, 0.0
                self.fade_duration = self.fade_elapsed = 0.0

    def previous_weight(self) -> float:
        if self.previous is None or self.fade_duration <= 0.0:
            return 0.0
        return min(1.0, max(0.0, 1.0 - self.fade_elapsed / self.fade_duration))

    def current_weight(self) -> float:
        return 0.0 if self.current is None else 1.0 - self.previous_weight()

    def fields(self) -> dict[str, Any]:
        """Walker dump keys (merge into the walker dict, with ``anim_blend`` 1)."""
        out: dict[str, Any] = {}
        if self.current is not None:
            out["clip_name"] = self.current
            out["clip"] = max(self.time, 1e-4)  # clip 0 = rest pose
        w = self.previous_weight()
        if self.previous is not None and w > 0.0:
            out.update(fade_from=self.previous, fade_from_clip=self.previous_time, fade_from_weight=w)
        else:
            out.update(fade_from=None, fade_from_clip=0.0, fade_from_weight=0.0)
        return out
