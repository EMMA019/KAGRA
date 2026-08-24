"""VRM Overworld — tiled island + city JSON + mesh ramp + crate stack.

Play-surface. Not an agent-built log. Public APIs:
load_city / add_trimesh / add_box(is_static=False) / set_shadow_cascades(2).

Not OSM, not Rapier. CSM is 2 cascades (near/far), not a film-grade split.

操作:
  WASD / 左スティック : 歩く（坂に沿う。急斜面は滑る。水中は遅くなる）
  マウス / 右スティック : 視点
  SPACE / A           : ジャンプ（coyote + バッファ。水中は泳ぐ）
  クリック            : 金の球を持つ / 置く
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
        self.world.set_height_fn(
            kagra.overworld_height, cells=8, tile=10.0, stream_radius=28.0,
        )
        self.world.set_water_y(0.0)
        self.world.set_chunk_fill(self._fill_chunk)
        city_path = os.path.join(_HERE, "data", "overworld_city.json")
        self.city = self.world.load_city(city_path)
        self.world.add_player(0.0, 4.0)
        gy = self.world.ground_y
        ramp_v, ramp_i = kagra.ramp_mesh(-1.6, 2.4, 6.2, 8.0, gy(-1.6, 7.1), gy(-1.6, 7.1) + 1.4)
        self.world.add_trimesh(ramp_v, ramp_i)
        stack_x, stack_z = 1.6, 2.2
        base = gy(stack_x, stack_z)
        for y in (base + 0.55, base + 1.15, base + 1.75):
            self.world.add_box(stack_x, y, stack_z, 0.55, 0.5, 0.55, is_static=False)
        tex = kagra.texture_from_fn(128, 128, _terrain_px, name="overworld_land")
        self.world.bake_terrain(tex)
        wall = kagra.solid_tex((148, 128, 108))
        self.world.bake(tex, wall)
        try:
            rtex = kagra.solid_tex((180, 120, 90))
            mid = kagra.upload_mesh_3d(rtex, ramp_v, ramp_i)
            if mid:
                self.world.mesh_ids.append(int(mid))
        except Exception:
            pass
        try:
            kagra.apply_outdoor_look()
        except Exception:
            pass
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
        self.title = kagra.Label("Overworld", 18, 14, 24, (240, 230, 180))
        self.hint = kagra.Label("", 18, 46, 16, (200, 220, 230))

    def _fill_chunk(self, ix, iz):
        if kagra.city_chunk(self.city, ix, iz):
            return
        for x, y, z, w, h, d in kagra.city_boxes(ix, iz, tile=10.0):
            self.world.add_box(x, y, z, w, h, d)

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
        if not SMOKE:
            hit = kagra.clicked_prop(self.cam)
            if hit is not None:
                if self.walk.held is None:
                    self.walk.carry(hit)
                    kagra.sound("ok")
                else:
                    self.walk.carry(None)
        kagra.Prop.update_all(dt)
        self.walk.step(dt)
        p = self.world.player
        moving = False
        if p is not None:
            moving = p.vx * p.vx + p.vz * p.vz > 0.04 or abs(p.vy) > 0.4
            if math.hypot(p.x - 3.2, p.z + 1.2) < 1.1:
                if not self.found:
                    kagra.sound("coin")
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
        if not getattr(self, "_outdoor", False):
            try:
                kagra.apply_outdoor_look()
            except Exception:
                pass
            self._outdoor = True
        self.world.draw()
        kagra.water(0.0, half=HALF, world=self.world)
        kagra.Prop.draw_all()
        kagra.draw_vrm(self.avatar.vrm_id)
        kagra.fill(0, 0, 420, 78, (8, 18, 28), 150)
        self.title.draw()
        self.hint.text = "gold  found" if self.found else "SPACE  stairs / mesh ramp / crates"
        self.hint.color = (160, 255, 190) if self.found else (200, 220, 230)
        self.hint.draw()


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
