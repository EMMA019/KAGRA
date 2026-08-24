"""VRM Prop Garden — short 3D play surface (Ursina-shaped, not 2D Entity).

Play-surface demo. Not an agent-built log. Public APIs only:
Prop / Walk / sky / hovered_prop / destroy / World3D / Camera3D.follow / ensure_vrm.
Non-smoke also places ``cube.glb`` (static glTF part, not ``stage()``).

操作:
  WASD / 左スティック : 歩く
  マウス / 右スティック : 視点（一人称は上下も。ロックする）
  クリック            : ホバー中の Prop を持つ / 置く
  F / Start           : 一人称 / 三人称
  E                   : ホバー中の Prop を消す（親を消すと子も消える）
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


def _crate_px(x: int, y: int):
    """Checker crate for texture_from_fn (pixel coords)."""
    c = ((int(x) // 8) + (int(y) // 8)) % 2
    return (210, 130, 55, 255) if c == 0 else (120, 60, 28, 255)

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
        crate_tex = 0
        if not SMOKE:
            crate_tex = kagra.texture_from_fn(64, 64, _crate_px, name="prop_crate")
        self.gold = None
        for model, x, y, z, scale, color in PROPS:
            tex = crate_tex if model == "box" and color == "orange" else 0
            prop = kagra.Prop(
                model, x=x, y=y, z=z, scale=scale, color=color, world=self.world, texture=tex,
            )
            if color == "gold":
                self.gold = prop
        if not SMOKE and self.gold is not None:
            gem = kagra.Prop(
                "box", x=0.0, y=0.58, z=0.0, scale=0.26, color="green",
                collision=False,
            )
            gem.set_parent(self.gold, keep_world=False)
            spark = kagra.Prop(
                "sphere", x=0.0, y=0.22, z=0.0, scale=0.10, color="white",
                collision=False,
            )
            spark.set_parent(gem, keep_world=False)
            kagra.Prop("cube.glb", x=-1.4, y=0.5, z=2.6, color="white", world=self.world)
            kagra.Prop(
                "sphere", x=2.2, y=0.5, z=-1.6, scale=0.9, color="white",
                metallic=1.0, roughness=0.12, world=self.world,
            )

            def _bump_n(x, y):
                import math
                u, v = (int(x) / 32.0, int(y) / 32.0)
                nx = math.sin(u * 12.0)
                ny = math.cos(v * 12.0)
                leng = math.sqrt(nx * nx + ny * ny + 4.0)
                return (
                    int((nx / leng * 0.5 + 0.5) * 255),
                    int((ny / leng * 0.5 + 0.5) * 255),
                    int((2.0 / leng * 0.5 + 0.5) * 255),
                    255,
                )

            bump = kagra.texture_from_fn(32, 32, _bump_n, name="garden_n", srgb=False)
            kagra.Prop(
                "box", x=2.2, y=0.35, z=1.4, scale=(0.8, 0.7, 0.8),
                color=(180, 160, 140), normal=bump, world=self.world,
            )
            kagra.set_hdri("studio", 0.40)
            kagra.set_point_light(1.4, 2.2, 0.8, intensity=1.1, radius=11.0)

            def _bob_down():
                if self.gold is None or not self.gold.enabled:
                    return
                kagra.animate(self.gold, "y", 0.34, duration=0.55, on_done=_bob_up)

            def _bob_up():
                if self.gold is None or not self.gold.enabled:
                    return
                kagra.animate(self.gold, "y", 0.68, duration=0.55, on_done=_bob_down)

            _bob_up()
        kagra.Prop.bake_all()
        self.cam = Camera3D(SW, SH, fov_deg=42.0)
        self.cam.follow(START_XZ[0], 0.0, START_XZ[1], lerp=1.0, yaw=0.0)
        kagra.set_camera3d(self.cam)
        self.walk = kagra.Walk(self.world, self.cam, speed=PLAYER_SPEED)
        self.found = False
        self.hi = int((kagra.load_json("prop_garden") or {}).get("found") or 0)
        self.look = None
        self.title = kagra.Label("Prop Garden", 18, 14, 26, (230, 220, 160))
        self.hint = kagra.Label("", 18, 50, 18, (190, 200, 220))
        self.mode = kagra.Label("", 18, 72, 16, (160, 170, 190))

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

        kagra.poll_pad()
        if (kagra.pressed("F") or kagra.pad_pressed("start")) and not SMOKE:
            self.walk.first_person = not self.walk.first_person
            self.avatar.first_person = self.walk.first_person
        if self.gold is not None and self.gold.enabled and SMOKE:
            self.gold.y = 0.5 + 0.18 * math.sin(kagra.tick_count() * 0.1)
        kagra.Prop.update_all(dt)
        self.walk.step(dt)
        self.look = kagra.hovered_prop(self.cam)
        if not SMOKE:
            hit = kagra.clicked_prop(self.cam)
            if hit is not None:
                if self.walk.held is None:
                    self.walk.carry(hit)
                    kagra.sound("ok")
                else:
                    self.walk.carry(None)
                    kagra.sound("ok")
        if (kagra.pressed("E")) and self.look is not None and not SMOKE:
            gone = self.look
            if self.walk.held is gone:
                self.walk.carry(None)
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
                kagra.sound("coin")
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
        self.title.draw()
        self.hint.text = "gold sphere — found" if self.found else "WASD  walk to the gold sphere"
        self.hint.color = (160, 255, 190) if self.found else (190, 200, 220)
        self.hint.draw()
        self.mode.text = "first person  F" if self.walk.first_person else "third person  F"
        self.mode.draw()
        if self.look is not None:
            kagra.text(self.look.model + "  click/E", 18, 92, 18, self.look.color)
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
