"""Pretty enclosed room — public APIs only.

``room()`` + ``Walk`` + a few ``Prop``s. Not an agent-built log.
Play-surface counterpart of Prop Garden (outdoor). Indoor look is
``apply_room_look`` (spot + studio HDRI + exposure).

操作:
  WASD / 左スティック : 歩く
  マウス / 右スティック : 視点（一人称はポインタロック）
  クリック            : Prop を持つ / 置く
  F / Start           : 一人称 / 三人称（既定は三人称）
  ESC                 : 終了

スモーク: KAGRA_SMOKE=1 python examples/vrm_pretty_room.py
"""
from __future__ import annotations

import math
import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(_HERE))

import kagra
from kagra.camera3d import Camera3D

SW, SH = 960, 540
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "36"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/pretty_room_smoke.png")


def _brick_albedo(x: int, y: int):
    u, v = x / 64.0, y / 64.0
    row = int(v * 8)
    ox = 0.5 if row % 2 else 0.0
    fx = (u * 8 + ox) % 1.0
    fy = (v * 8) % 1.0
    if fx < 0.12 or fy < 0.12:
        return (90, 78, 70, 255)
    return (168, 92, 64, 255)


def _brick_normal(x: int, y: int):
    import math

    u, v = x / 64.0, y / 64.0
    row = int(v * 8)
    ox = 0.5 if row % 2 else 0.0
    fx = (u * 8 + ox) % 1.0
    fy = (v * 8) % 1.0
    nx = ny = 0.0
    if fx < 0.12:
        nx = -1.0
    elif fx > 0.88:
        nx = 1.0
    if fy < 0.12:
        ny = -1.0
    elif fy > 0.88:
        ny = 1.0
    leng = math.sqrt(nx * nx + ny * ny + 1.0)
    nx, ny, nz = nx / leng, ny / leng, 1.0 / leng
    return (
        int((nx * 0.5 + 0.5) * 255),
        int((ny * 0.5 + 0.5) * 255),
        int((nz * 0.5 + 0.5) * 255),
        255,
    )


class PrettyRoom(kagra.Scene):
    def on_enter(self):
        kagra.font()
        kagra.Prop.clear()
        self.avatar = kagra.avatar(str(kagra.ensure_vrm()))
        self.avatar.play("idle", loop=True)
        self.world = kagra.World3D(half=5.6)
        self.world.add_player(0.0, 2.4)
        kagra.room(half=6.0, height=3.2, world=self.world)
        kagra.Prop(
            "box", x=0.0, y=0.38, z=-1.4, scale=(1.7, 0.76, 0.9),
            color=(92, 58, 36), world=self.world,
        )
        kagra.Prop(
            "sphere", x=0.35, y=0.92, z=-1.35, scale=0.34, color="white",
            metallic=1.0, roughness=0.08, world=self.world,
        )
        kagra.Prop(
            "box", x=-1.8, y=0.42, z=0.6, scale=(0.46, 0.84, 0.48),
            color=(120, 86, 64), world=self.world,
        )
        kagra.Prop(
            "cylinder", x=2.1, y=0.55, z=-0.4, scale=(0.55, 1.1, 0.55),
            color=(210, 200, 188), world=self.world,
        )
        if not SMOKE:
            nrm = kagra.texture_from_fn(64, 64, _brick_normal, name="brick_n", srgb=False)
            alb = kagra.texture_from_fn(64, 64, _brick_albedo, name="brick")
            kagra.Prop(
                "box", x=-2.4, y=0.7, z=-1.6, scale=(1.2, 1.4, 0.28),
                texture=alb, normal=nrm, world=self.world,
            )
            kagra.set_point_light(
                -2.1, 1.7, 1.6, r=1.0, g=0.55, b=0.32, intensity=0.55, radius=7.0, slot=1,
            )
            kagra.set_point_light(
                2.0, 1.5, -1.4, r=0.4, g=0.6, b=1.0, intensity=0.4, radius=7.0, slot=2,
            )
        kagra.Prop.bake_all()
        self.cam = Camera3D(SW, SH, fov_deg=58.0)
        p = self.world.player
        # Third-person behind the spawn. first_person=True put the eye at the
        # capsule (z=2.4, y=1.55) looking slightly down — inside the VRM mesh.
        yaw = math.pi
        dist, height, look_y = 3.2, 1.7, 1.15
        self.cam.follow(
            p.x, p.y, p.z,
            lerp=1.0, yaw=yaw, distance=dist, height=height, look_y=look_y,
        )
        kagra.set_camera3d(self.cam)
        self.walk = kagra.Walk(
            self.world, self.cam, speed=2.8, yaw=yaw,
            distance=dist, height=height, look_y=look_y,
        )
        self.title = kagra.Label("Pretty Room", 18, 14, 24, (240, 220, 190))
        self.hint = kagra.Label("click  carry   F  view", 18, 46, 16, (190, 175, 155))

    def update(self, dt):
        dt = min(dt, 0.05)
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        if SMOKE:
            n = kagra.tick_count()
            if n == 6:
                kagra.inject_key("W")
            if n == 20:
                kagra.screenshot(SMOKE_SHOT)
            if n >= SMOKE_FRAMES:
                kagra.quit()
                return
        if (kagra.pressed("F") or kagra.pad_pressed("start")) and not SMOKE:
            self.walk.first_person = not self.walk.first_person
            self.avatar.first_person = self.walk.first_person
        if not SMOKE:
            hit = kagra.clicked_prop(self.cam)
            if hit is not None:
                if self.walk.held is None:
                    self.walk.carry(hit)
                    kagra.sound("ok")
                else:
                    self.walk.carry(None)
                    kagra.sound("ok")
        kagra.Prop.update_all(dt)
        self.walk.step(dt)
        p = self.world.player
        moving = False
        if p is not None:
            moving = p.vx * p.vx + p.vz * p.vz > 0.04
        want = "walk" if moving else "idle"
        if getattr(self.avatar, "clip", None) != want:
            self.avatar.play(want, loop=True)
        self.avatar.update(dt)
        if p is None:
            return
        self.avatar.set_position(p.x, p.y, p.z)
        self.avatar.set_yaw(self.walk.face)

    def draw(self):
        kagra.cls(18, 14, 12)
        self.world.draw()
        kagra.Prop.draw_all()
        kagra.draw_vrm(self.avatar.vrm_id)
        kagra.draw_vignette(strength=0.28)
        kagra.fill(0, 0, 360, 72, (12, 10, 8), 160)
        self.title.draw()
        mode = "first person  F" if self.walk.first_person else "third person  F"
        self.hint.text = ("carrying  click drop  " if self.walk.held is not None else "click  carry   ") + mode
        self.hint.draw()


if __name__ == "__main__":
    os.makedirs(os.path.dirname(SMOKE_SHOT) or ".", exist_ok=True)
    kagra.init(
        width=SW,
        height=SH,
        title="VRM Pretty Room",
        fps=60,
        visible=not SMOKE,
    )
    kagra.run(
        start_scene=PrettyRoom(),
        max_frames=SMOKE_FRAMES + 2 if SMOKE else None,
        fixed_dt=1.0 / 60.0 if SMOKE else None,
    )
