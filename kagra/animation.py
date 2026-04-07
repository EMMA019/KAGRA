
# kagra/animation.py
# Phase 5 Animation System (engine-level abstraction)

import json
import os


class AnimationClip:
    def __init__(self, name, frames, fps=10, loop=True, events=None):
        self.name   = name
        self.frames = frames
        self.fps    = fps
        self.loop   = loop
        self.events = events or {}
        self.length = len(frames) / fps

    def frame_at(self, time):
        if self.loop:
            time = time % self.length
        frame = int(time * self.fps)
        frame = min(frame, len(self.frames) - 1)
        return self.frames[frame]

    # ── シリアライズ ──────────────────────────────────────────

    def to_dict(self) -> dict:
        return {
            "name":   self.name,
            "frames": self.frames,
            "fps":    self.fps,
            "loop":   self.loop,
            "events": self.events,
        }

    @classmethod
    def from_dict(cls, d: dict) -> "AnimationClip":
        return cls(
            name   = d["name"],
            frames = d["frames"],
            fps    = d.get("fps", 10),
            loop   = d.get("loop", True),
            events = d.get("events", {}),
        )


class Animator:
    def __init__(self, target):
        self.target  = target
        self.clips:  dict[str, AnimationClip] = {}
        self.current: AnimationClip = None
        self.time    = 0.0
        self.speed   = 1.0
        self.playing = False

    def add_clip(self, clip: AnimationClip):
        self.clips[clip.name] = clip

    def play(self, name: str, reset: bool = True):
        if name not in self.clips:
            raise ValueError(f"Animation clip '{name}' not found")
        self.current = self.clips[name]
        if reset:
            self.time = 0
        self.playing = True

    def stop(self):
        self.playing = False

    def set_speed(self, speed: float):
        self.speed = speed

    def is_finished(self) -> bool:
        if not self.current:
            return True
        return (not self.current.loop) and self.time >= self.current.length

    def update(self, dt: float):
        if not self.playing or not self.current:
            return
        self.time += dt * self.speed
        frame = self.current.frame_at(self.time)

        if hasattr(self.target, "set_frame"):
            self.target.set_frame(frame)

        if self.is_finished():
            self.playing = False

    # ── シリアライズ（クリップ定義のみ） ─────────────────────

    def clips_to_dict(self) -> dict:
        """登録されているクリップ定義を保存する。"""
        return {
            "version": 1,
            "clips":   [c.to_dict() for c in self.clips.values()],
            "current": self.current.name if self.current else None,
        }

    def load_clips_from_dict(self, d: dict):
        """clips_to_dict() の出力からクリップを復元する。"""
        for cd in d.get("clips", []):
            clip = AnimationClip.from_dict(cd)
            self.clips[clip.name] = clip
        current_name = d.get("current")
        if current_name and current_name in self.clips:
            self.current = self.clips[current_name]

    def save_clips(self, path: str):
        """クリップ定義を JSON ファイルに保存する。"""
        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            json.dump(self.clips_to_dict(), f, ensure_ascii=False, indent=2)

    def load_clips(self, path: str):
        """クリップ定義を JSON ファイルから読み込む。"""
        with open(path, "r", encoding="utf-8") as f:
            self.load_clips_from_dict(json.load(f))


class AnimationStateMachine:
    def __init__(self, animator: Animator):
        self.animator    = animator
        self.state       = None
        self.transitions: dict[str, list] = {}

    def add_transition(self, from_state: str, to_state: str, condition):
        self.transitions.setdefault(from_state, []).append((to_state, condition))

    def set_state(self, state: str):
        self.state = state
        self.animator.play(state)

    def update(self, dt: float):
        self.animator.update(dt)
        if self.state in self.transitions:
            for to_state, cond in self.transitions[self.state]:
                if cond():
                    self.set_state(to_state)
                    break
