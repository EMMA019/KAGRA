"""Several VRM avatars in one scene — GPU share + FPS.

Crest Isle play stays single-player (title / input / camera untouched).
Same-path ``kagra.avatar()`` clones share mesh / texture / MToon; joint
palettes, pose, and SpringBone stay per instance.

Desktop::

    python examples/vrm_multi_avatar.py
    KAGRA_AVATARS=8 python examples/vrm_multi_avatar.py

Smoke / headless::

    KAGRA_SMOKE=1 python examples/vrm_multi_avatar.py
    python -m kagra.verify examples/verify_scenarios/multi_avatar_smoke.json

Sample VRM via ensure_vrm is Alicia Solid (ニコニ立体ちゃん) © Dwango.
"""
from __future__ import annotations

import json
import math
import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(_HERE))

import kagra
from kagra.camera3d import Camera3D
from kagra.vrm_crowd import crowd_count, crowd_offsets, same_path_is_shared

SW, SH = 960, 540
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "36"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/multi_avatar_smoke.png")
SMOKE_STATS = os.environ.get("KAGRA_SMOKE_STATS", "scratch/multi_avatar_stats.json")
N = crowd_count(os.environ.get("KAGRA_AVATARS"))


class MultiAvatar(kagra.Scene):
    def on_enter(self):
        kagra.font()
        kagra.Prop.clear()
        path = str(kagra.ensure_vrm())
        extras = crowd_offsets(N - 1)
        self.avatars = []
        player = kagra.avatar(path)
        player.play("idle", loop=True)
        player.set_position(0.0, 0.0, 0.0)
        player.set_yaw(0.0)
        self.avatars.append(player)
        for i, (x, z) in enumerate(extras):
            av = kagra.avatar(path)
            av.play("idle", loop=True)
            av.set_position(x, 0.0, z)
            av.set_yaw(math.atan2(x, z) + math.pi)
            self.avatars.append(av)
        self.world = kagra.World3D(half=6.0)
        self.world.add_player(0.0, 2.8)
        kagra.room(half=6.0, height=3.2, world=self.world)
        kagra.Prop.bake_all()
        self.cam = Camera3D(SW, SH, fov_deg=55.0)
        self.cam.follow(
            0.0, 0.0, 0.0,
            lerp=1.0, yaw=math.pi, distance=5.4, height=1.85, look_y=0.95,
            world=self.world,
        )
        kagra.set_camera3d(self.cam)
        self.stats = kagra.vrm_gpu_stats()
        self._fps_t = 0.0
        self._fps_n = 0
        self._fps = 0.0
        self._wrote = False
        print("[MultiAvatar]", json.dumps(self.stats, sort_keys=True))
        if N >= 2 and not same_path_is_shared(self.stats):
            print("[MultiAvatar] WARN: GPU share invariant failed", self.stats)

    def update(self, dt):
        dt = min(dt, 0.05)
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        self._fps_n += 1
        self._fps_t += max(dt, 1e-6)
        if self._fps_t >= 0.4:
            self._fps = self._fps_n / self._fps_t
            self._fps_n = 0
            self._fps_t = 0.0
        if SMOKE:
            n = kagra.tick_count()
            if n == 20:
                self._dump_stats()
                kagra.screenshot(SMOKE_SHOT)
            if n >= SMOKE_FRAMES:
                if not self._wrote:
                    self._dump_stats()
                kagra.quit()
                return
        for av in self.avatars:
            av.update(dt)

    def _dump_stats(self):
        self.stats = kagra.vrm_gpu_stats()
        payload = {
            "avatars": N,
            "fps": round(self._fps, 2),
            "smoke": SMOKE,
            "shared": same_path_is_shared(self.stats) if N >= 2 else True,
            **self.stats,
        }
        os.makedirs(os.path.dirname(SMOKE_STATS) or ".", exist_ok=True)
        with open(SMOKE_STATS, "w", encoding="utf-8") as f:
            json.dump(payload, f, indent=2)
            f.write("\n")
        self._wrote = True
        print("[MultiAvatar] stats", json.dumps(payload, sort_keys=True))

    def draw(self):
        kagra.cls(18, 16, 22)
        self.world.draw()
        kagra.Prop.draw_all()
        for av in self.avatars:
            kagra.draw_vrm(av.vrm_id)
        kagra.draw_vignette(strength=0.22)
        kagra.fill(0, 0, 520, 52, (10, 8, 14), 180)
        fps = f"{self._fps:.0f}" if self._fps > 0.5 else "—"
        kagra.text(
            f"avatars {N}   FPS {fps}   vb {self.stats.get('vertex_buffers', 0)}"
            f"/{self.stats.get('primitives', 0)}",
            14, 12, 18, (240, 230, 210),
        )
        kagra.text("ESC quit   KAGRA_AVATARS=N", 14, 34, 14, (170, 160, 150))


if __name__ == "__main__":
    os.makedirs(os.path.dirname(SMOKE_SHOT) or ".", exist_ok=True)
    kagra.init(
        width=SW,
        height=SH,
        title="VRM Multi Avatar",
        fps=60,
        visible=not SMOKE,
    )
    kagra.run(
        start_scene=MultiAvatar(),
        max_frames=SMOKE_FRAMES + 2 if SMOKE else None,
        fixed_dt=1.0 / 60.0 if SMOKE else None,
    )
