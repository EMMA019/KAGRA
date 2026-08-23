"""VRM Switch Room — walk a boxed room and stand on a floor switch.

Agent-built world game (public APIs only). Prompt:
docs/agent-runs/20260823-switch-room/prompt.md.

Not another disc-collect: retained floor+boxes, AABB collision, camera follow.

操作:
  WASD / 矢印 : 歩く
  SPACE / ENTER : スタート / リトライ
  ESC               : 終了

スモーク: KAGRA_SMOKE=1 python examples/vrm_switch_room.py
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

from switch_room_rules import (
    BOXES,
    HOLD_SEC,
    PLAYER_SPEED,
    START_XZ,
    SWITCH_HALF,
    SWITCH_XZ,
    facing_yaw,
    on_switch,
    walls,
    wish_velocity,
)

SW, SH = 960, 540
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "48"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/switch_room_smoke.png")


def _floor_tex():
    def px(x, y):
        c = (x // 8 + y // 8) % 2
        return (62, 78, 98, 255) if c else (48, 60, 78, 255)

    return kagra.texture_from_fn(64, 64, px, name="switch_floor")


def _box_tex():
    def px(x, y):
        edge = x < 3 or y < 3 or x > 28 or y > 28
        if edge:
            return (36, 30, 28, 255)
        return (168, 118, 72, 255)

    return kagra.texture_from_fn(32, 32, px, name="switch_box")


def _pad_tex():
    def px(x, y):
        d = math.hypot(x - 15.5, y - 15.5) / 15.5
        if d > 0.95:
            return (0, 0, 0, 0)
        glow = 1.0 - d * 0.45
        return (80, 255, 170, int(220 * glow))

    return kagra.texture_from_fn(32, 32, px, name="switch_pad")


class SwitchRoom(kagra.Scene):
    def on_enter(self):
        kagra.font()
        kagra.apply_live_look()
        self.tex_floor = _floor_tex()
        self.tex_box = _box_tex()
        self.tex_pad = _pad_tex()
        self.sfx = {
            "go": kagra.tone("switch_go", (659, 880), 0.16, 0.28),
            "ok": kagra.tone("switch_ok", (784, 1175, 1568), 0.22, 0.32),
        }
        self.avatar = kagra.avatar(str(kagra.ensure_vrm()))
        self.avatar.play("idle", loop=True)
        self.avatar.enable_emotion()
        self.action = kagra.ActionController(self.avatar)
        self.world = kagra.World3D(half=6.0)
        self.world.add_floor()
        for spec in BOXES:
            self.world.add_box(*spec)
        for spec in walls():
            self.world.add_box(*spec)
        self.switch = self.world.add_box(
            SWITCH_XZ[0], 0.0, SWITCH_XZ[1],
            SWITCH_HALF * 2, 0.08, SWITCH_HALF * 2,
            trigger=True,
        )
        self.world.add_player(*START_XZ)
        self.world.bake(self.tex_floor, self.tex_box)
        verts, idx = kagra.quad_y_mesh(SWITCH_XZ[0], 0.04, SWITCH_XZ[1], SWITCH_HALF)
        self.pad_mesh = kagra.upload_mesh_3d(self.tex_pad, verts, idx)
        self.cam = Camera3D(SW, SH, fov_deg=42.0)
        self.cam.follow(START_XZ[0], 0.0, START_XZ[1], lerp=1.0, yaw=math.pi)
        kagra.set_camera3d(self.cam)
        self.mode = "play" if SMOKE else "title"
        self.facing = math.pi
        self.hi = float((kagra.load_json("switch_room") or {}).get("best") or 0.0)
        self._reset()

    def _reset(self):
        self.elapsed = 0.0
        self.hold = 0.0
        self.won = False
        p = self.world.player
        if p is not None:
            p.teleport(START_XZ[0], 0.0, START_XZ[1])
            p.vx = p.vy = p.vz = 0.0
        self.facing = math.pi

    def _se(self, key: str):
        try:
            kagra.se(self.sfx[key])
        except Exception:
            pass

    def update(self, dt):
        dt = min(dt, 0.05)
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        if SMOKE:
            n = kagra.tick_count()
            if n == 8:
                self.world.move_player(0.0, -PLAYER_SPEED)
            if n == 24:
                kagra.screenshot(SMOKE_SHOT)
            if n >= SMOKE_FRAMES:
                kagra.quit()
                return

        if self.mode == "title":
            if kagra.pressed("SPACE") or kagra.pressed("RETURN"):
                self._se("go")
                self.mode = "play"
                self._reset()
            self._pose(dt, 0.0, 0.0)
            return
        if self.mode == "result":
            if kagra.pressed("SPACE") or kagra.pressed("R"):
                self._se("go")
                self.mode = "play"
                self._reset()
            self._pose(dt, 0.0, 0.0)
            return

        ax = (1.0 if kagra.key("D") or kagra.key("RIGHT") else 0.0) - (
            1.0 if kagra.key("A") or kagra.key("LEFT") else 0.0
        )
        az = (1.0 if kagra.key("S") or kagra.key("DOWN") else 0.0) - (
            1.0 if kagra.key("W") or kagra.key("UP") else 0.0
        )
        vx, vz = wish_velocity(ax, az)
        self.world.move_player(vx, vz)
        self.world.update(dt)
        p = self.world.player
        if p is not None and on_switch(p.x, p.z):
            self.hold += dt
            if self.hold >= HOLD_SEC and not self.won:
                self.won = True
                self.mode = "result"
                best = self.hi if self.hi > 0 else self.elapsed
                self.hi = min(best, self.elapsed) if self.hi > 0 else self.elapsed
                kagra.save_json("switch_room", {"best": self.hi})
                self.avatar.feel("joy", 1.0)
                self.action.play("clap")
                self._se("ok")
        else:
            self.hold = 0.0
        if not self.won:
            self.elapsed += dt
        self._pose(dt, vx, vz)

    def _pose(self, dt, vx, vz):
        moving = vx * vx + vz * vz > 0.04
        want = "walk" if self.mode == "play" and moving else "idle"
        if getattr(self.avatar, "clip", None) != want:
            self.avatar.play(want, loop=True)
        self.avatar.update(dt)
        self.action.update(dt)
        p = self.world.player
        if p is None:
            return
        self.facing = facing_yaw(vx, vz, self.facing)
        self.avatar.set_position(p.x, p.y, p.z)
        self.avatar.set_yaw(self.facing)
        self.cam.follow(p.x, p.y, p.z, yaw=self.facing)
        eng = kagra.get_engine()
        if eng:
            self.cam.update(eng)

    def draw(self):
        kagra.cls(10, 12, 20)
        self.world.draw()
        if self.pad_mesh:
            kagra.draw_mesh_id(self.pad_mesh)
        kagra.draw_vrm(self.avatar.vrm_id)
        kagra.draw_vignette()
        if self.mode == "title":
            self._banner("Switch Room", "WASD で歩く  奥のスイッチを踏む")
        elif self.mode == "result":
            best = f"  Best {self.hi:4.1f}s" if self.hi else ""
            self._banner("CLEAR", f"{self.elapsed:4.1f}s{best}   SPACE でもう一回")
        else:
            kagra.fill(0, 0, 300, 90, (8, 10, 18), 170)
            kagra.text(f"TIME   {self.elapsed:5.1f}", 18, 16, 26, (220, 235, 255))
            mark = "ON" if self.hold > 0 else "walk to the pad"
            kagra.text(mark, 18, 50, 20, (120, 255, 190) if self.hold > 0 else (180, 190, 210))
            p = self.world.player
            if p is not None:
                hit = self.cam.world_to_screen(SWITCH_XZ[0], 0.2, SWITCH_XZ[1])
                if hit:
                    kagra.text("SWITCH", hit[0] - 40, hit[1] - 18, 16, (160, 255, 200))

    def _banner(self, title: str, sub: str):
        kagra.fill(0, 0, SW, SH, (6, 8, 14), 140)
        w, _ = kagra.measure(title, 52)
        kagra.text(title, (SW - w) // 2, SH // 2 - 70, 52, (200, 230, 255))
        w2, _ = kagra.measure(sub, 22)
        kagra.text(sub, (SW - w2) // 2, SH // 2 + 10, 22, (210, 220, 235))


if __name__ == "__main__":
    os.makedirs(os.path.dirname(SMOKE_SHOT) or ".", exist_ok=True)
    kagra.init(
        width=SW,
        height=SH,
        title="VRM Switch Room",
        fps=60,
        visible=not SMOKE,
    )
    kagra.run(
        start_scene=SwitchRoom(),
        max_frames=SMOKE_FRAMES + 2 if SMOKE else None,
        fixed_dt=1.0 / 60.0 if SMOKE else None,
    )
