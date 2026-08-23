"""VRM Prop Garden — short 3D play surface (Ursina-shaped, not 2D Entity).

Play-surface demo. Not an agent-built log. Public APIs only:
Prop / Walk / sky / hovered_prop / destroy / World3D / Camera3D.follow / ensure_vrm.

操作:
  WASD / 矢印 : 歩く
  マウス       : 視点（一人称は上下も）
  F           : 一人称 / 三人称
  E           : ホバー中の Prop を消す
  ESC         : 終了

スモーク: KAGRA_SMOKE=1 python examples/vrm_prop_garden.py
"""
from __future__ import annotations

import math
import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(_HERE))
sys.path.insert(0, _HERE)

import kagra
from kagra.camera3d import Camera3D

from prop_garden_rules import GOLD_XZ, PLAYER_SPEED, PROPS, START_XZ, facing_yaw, near_gold

SW, SH = 960, 540
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "48"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/prop_garden_smoke.png")


class PropGarden(kagra.Scene):
    def on_enter(self):
        kagra.font()
        kagra.Prop.clear()
        self.avatar = kagra.avatar(str(kagra.ensure_vrm()))
        self.avatar.play("idle", loop=True)
        self.avatar.enable_emotion()
        self.action = kagra.ActionController(self.avatar)
        self.world = kagra.World3D(half=7.0)
        self.world.add_player(*START_XZ)
        kagra.Prop("plane", x=0, y=0, z=0, scale=14.0, color="gray", collision=False)
        self.gold = None
        for model, x, y, z, scale, color in PROPS:
            prop = kagra.Prop(model, x=x, y=y, z=z, scale=scale, color=color, world=self.world)
            if color == "gold":
                self.gold = prop
        kagra.Prop.bake_all()
        self.cam = Camera3D(SW, SH, fov_deg=42.0)
        self.cam.follow(START_XZ[0], 0.0, START_XZ[1], lerp=1.0, yaw=0.0)
        kagra.set_camera3d(self.cam)
        self.walk = kagra.Walk(self.world, self.cam, speed=PLAYER_SPEED)
        self.found = False
        self.hi = int((kagra.load_json("prop_garden") or {}).get("found") or 0)
        self.look = None

    def update(self, dt):
        dt = min(dt, 0.05)
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        if SMOKE:
            n = kagra.tick_count()
            if n == 8:
                kagra.inject_key("W")
            if n == 24:
                kagra.screenshot(SMOKE_SHOT)
            if n >= SMOKE_FRAMES:
                kagra.quit()
                return

        if kagra.pressed("F") and not SMOKE:
            self.walk.first_person = not self.walk.first_person
            self.avatar.first_person = self.walk.first_person
        if self.gold is not None and self.gold.enabled and not SMOKE:
            self.gold.y = 0.5 + 0.18 * math.sin(kagra.tick_count() * 0.1)
        kagra.Prop.update_all(dt)
        self.walk.step(dt)
        self.look = kagra.hovered_prop(self.cam)
        if kagra.pressed("E") and self.look is not None and not SMOKE:
            gone = self.look
            kagra.destroy(gone)
            if gone is self.gold:
                self.gold = None
            self.look = None
        p = self.world.player
        moving = False
        if p is not None:
            moving = p.vx * p.vx + p.vz * p.vz > 0.04
            if near_gold(p.x, p.z) and not self.found:
                self.found = True
                self.hi += 1
                kagra.save_json("prop_garden", {"found": self.hi})
                self.avatar.feel("joy", 1.0)
                self.action.play("clap")
        want = "walk" if moving else "idle"
        if getattr(self.avatar, "clip", None) != want:
            self.avatar.play(want, loop=True)
        self.avatar.update(dt)
        self.action.update(dt)
        if p is None:
            return
        face = facing_yaw(p.vx, p.vz, self.walk.yaw)
        self.avatar.set_position(p.x, p.y, p.z)
        self.avatar.set_yaw(face)

    def draw(self):
        kagra.cls(8, 12, 22)
        kagra.sky()
        self.world.draw()
        kagra.Prop.draw_all()
        kagra.draw_vrm(self.avatar.vrm_id)
        kagra.draw_vignette()
        kagra.fill(0, 0, 380, 110, (8, 10, 18), 170)
        kagra.text("Prop Garden", 18, 14, 26, (230, 220, 160))
        hint = "gold sphere — found" if self.found else "WASD  walk to the gold sphere"
        kagra.text(hint, 18, 50, 18, (160, 255, 190) if self.found else (190, 200, 220))
        mode = "first person  F" if self.walk.first_person else "third person  F"
        kagra.text(mode, 18, 72, 16, (160, 170, 190))
        if self.look is not None:
            kagra.text(self.look.model + "  E", 18, 92, 18, self.look.color)
        hit = self.cam.world_to_screen(GOLD_XZ[0], 1.1, GOLD_XZ[1])
        if hit and not self.found and self.gold is not None and self.gold.enabled:
            kagra.text("GOLD", hit[0] - 28, hit[1] - 18, 16, (255, 220, 90))


if __name__ == "__main__":
    os.makedirs(os.path.dirname(SMOKE_SHOT) or ".", exist_ok=True)
    kagra.init(
        width=SW,
        height=SH,
        title="VRM Prop Garden",
        fps=60,
        visible=not SMOKE,
    )
    kagra.run(
        start_scene=PropGarden(),
        max_frames=SMOKE_FRAMES + 2 if SMOKE else None,
        fixed_dt=1.0 / 60.0 if SMOKE else None,
    )
