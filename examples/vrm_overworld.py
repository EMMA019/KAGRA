"""VRM Overworld — heightfield island (sea / grass / mountain).

Play-surface. Not an agent-built log. Public APIs:
World3D.set_height_fn / set_water_y / Walk(jump=) / water / sky / island_height.

操作:
  WASD / 左スティック : 歩く（水中は遅くなる）
  マウス / 右スティック : 視点
  SPACE / A           : ジャンプ（水中は泳ぐ）
  ESC                 : 終了

スモーク: KAGRA_SMOKE=1 python examples/vrm_overworld.py
"""
from __future__ import annotations

import math
import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(_HERE))

import kagra
from kagra.camera3d import Camera3D
from kagra.land import terrain_rgba

SW, SH = 960, 540
HALF = 24.0
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "40"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/overworld_smoke.png")


def _terrain_px(x, y):
    return terrain_rgba(x / 127.0, y / 127.0, half=HALF)


class Overworld(kagra.Scene):
    def on_enter(self):
        kagra.font()
        kagra.Prop.clear()
        self.avatar = kagra.avatar(str(kagra.ensure_vrm()))
        self.avatar.play("idle", loop=True)
        self.world = kagra.World3D(half=HALF)
        self.world.set_height_fn(kagra.island_height, cells=48)
        self.world.set_water_y(0.0)
        self.world.add_player(0.0, 4.0)
        tex = kagra.texture_from_fn(128, 128, _terrain_px, name="overworld_land")
        self.world.bake_terrain(tex)
        kagra.Prop(
            "sphere", x=3.2, y=self.world.ground_y(3.2, -1.2) + 0.35, z=-1.2,
            scale=0.55, color="gold", world=self.world,
        )
        kagra.Prop.bake_all()
        self.cam = Camera3D(SW, SH, fov_deg=52.0)
        p = self.world.player
        self.cam.follow(p.x, p.y, p.z, lerp=1.0, yaw=0.0, distance=6.2, height=2.6)
        kagra.set_camera3d(self.cam)
        self.walk = kagra.Walk(self.world, self.cam, speed=4.2, jump=6.2, distance=6.2, height=2.6)
        self.found = False

    def update(self, dt):
        dt = min(dt, 0.05)
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        if SMOKE:
            n = kagra.tick_count()
            if n == 6:
                kagra.inject_key("W")
            if n == 12:
                kagra.inject_key("SPACE")
            if n == 24:
                kagra.screenshot(SMOKE_SHOT)
            if n >= SMOKE_FRAMES:
                kagra.quit()
                return
        self.walk.step(dt)
        p = self.world.player
        moving = False
        if p is not None:
            moving = p.vx * p.vx + p.vz * p.vz > 0.04 or abs(p.vy) > 0.4
            if math.hypot(p.x - 3.2, p.z + 1.2) < 1.1:
                self.found = True
        want = "walk" if moving else "idle"
        if getattr(self.avatar, "clip", None) != want:
            self.avatar.play(want, loop=True)
        self.avatar.update(dt)
        if p is None:
            return
        self.avatar.set_position(p.x, p.y, p.z)
        self.avatar.set_yaw(self.walk.yaw)

    def draw(self):
        kagra.cls(110, 170, 210)
        kagra.sky(radius=42.0)
        self.world.draw()
        kagra.water(0.0, half=HALF, world=self.world)
        kagra.Prop.draw_all()
        kagra.draw_vrm(self.avatar.vrm_id)
        kagra.fill(0, 0, 420, 78, (8, 18, 28), 150)
        kagra.text("Overworld", 18, 14, 24, (240, 230, 180))
        hint = "gold  found" if self.found else "SPACE jump  —  walk to the gold"
        kagra.text(hint, 18, 46, 16, (160, 255, 190) if self.found else (200, 220, 230))


if __name__ == "__main__":
    os.makedirs(os.path.dirname(SMOKE_SHOT) or ".", exist_ok=True)
    kagra.init(
        width=SW,
        height=SH,
        title="VRM Overworld",
        fps=60,
        visible=not SMOKE,
    )
    kagra.run(
        start_scene=Overworld(),
        max_frames=SMOKE_FRAMES + 2 if SMOKE else None,
        fixed_dt=1.0 / 60.0 if SMOKE else None,
    )
