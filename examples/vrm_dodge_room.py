"""VRM Meteor Dodge — walk out of the way of falling boxes.

Agent-built game (public APIs only). Prompt:
docs/agent-runs/20260823-dodge-room/prompt.md.

Not a collect or a switch: dynamic meteors spawned every frame in pure
Python (examples/dodge_room_rules.py), drawn as retained mesh, checked
against the player position for a hit. Difficulty ramps with survival time.

操作:
  WASD / 矢印 : 歩く
  SPACE / ENTER : スタート / リトライ
  ESC               : 終了

スモーク: KAGRA_SMOKE=1 python examples/vrm_dodge_room.py
"""
from __future__ import annotations

import math
import os
import random
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(_HERE))
sys.path.insert(0, _HERE)

import kagra
from kagra.camera3d import Camera3D

from dodge_room_rules import (
    ARENA_HALF,
    START_LIVES,
    INVULN_SEC,
    fall_speed,
    has_landed,
    is_hit,
    spawn_gap,
    spawn_meteor,
    step_meteor,
    survival_score,
    facing_yaw,
    wish_velocity,
)

SW, SH = 960, 540
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "48"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/dodge_room_smoke.png")

WALL_H = 1.6
WALL_T = 0.3


def _walls(half: float = ARENA_HALF):
    t, h = WALL_T, WALL_H
    return [
        (0.0, 0.0, -half, half * 2 + t, h, t),
        (0.0, 0.0, half, half * 2 + t, h, t),
        (-half, 0.0, 0.0, t, h, half * 2),
        (half, 0.0, 0.0, t, h, half * 2),
    ]


def _floor_tex():
    def px(x, y):
        c = (x // 8 + y // 8) % 2
        return (46, 40, 60, 255) if c else (34, 30, 46, 255)

    return kagra.texture_from_fn(64, 64, px, name="dodge_floor")


def _box_tex():
    def px(x, y):
        edge = x < 3 or y < 3 or x > 28 or y > 28
        return (30, 24, 34, 255) if edge else (120, 90, 130, 255)

    return kagra.texture_from_fn(32, 32, px, name="dodge_box")


def _meteor_tex():
    def px(x, y):
        d = math.hypot(x - 11.5, y - 11.5) / 11.5
        if d > 0.95:
            return (0, 0, 0, 0)
        glow = 1.0 - d * 0.5
        return (255, 120, 90, int(235 * glow))

    return kagra.texture_from_fn(24, 24, px, name="dodge_meteor")


class DodgeRoom(kagra.Scene):
    def on_enter(self):
        kagra.font()
        kagra.apply_live_look()
        self.tex_floor = _floor_tex()
        self.tex_box = _box_tex()
        self.tex_meteor = _meteor_tex()
        self.sfx = {
            "go": kagra.tone("dodge_go", (659, 880), 0.16, 0.28),
            "hit": kagra.tone("dodge_hit", (220, 140), 0.18, 0.3),
        }
        self.avatar = kagra.avatar(str(kagra.ensure_vrm()))
        self.avatar.play("idle", loop=True)
        self.avatar.enable_emotion()
        self.action = kagra.ActionController(self.avatar)
        self.world = kagra.World3D(half=ARENA_HALF + 1.0)
        self.world.add_floor()
        for spec in _walls():
            self.world.add_box(*spec)
        self.world.add_player(0.0, 1.5)
        self.world.bake(self.tex_floor, self.tex_box)
        self.cam = Camera3D(SW, SH, fov_deg=44.0)
        # Heart Catch と同じ：カメラは常に +Z 側。WASD はワールド固定
        # （W=-Z で画面奥、A=-X で左）。facing でカメラを回すと
        # 一歩目のあと上下左右が入れ替わって見える。
        # 既定 distance=4.8 は ARENA_HALF=4.2 の外。短め + bounds_half。
        self.cam.follow(
            0.0, 0.0, 1.5, lerp=1.0, yaw=math.pi,
            distance=3.0, height=2.0, look_y=1.0,
            bounds_half=ARENA_HALF,
        )
        kagra.set_camera3d(self.cam)
        self.mode = "play" if SMOKE else "title"
        self.facing = math.pi
        self.hi = float((kagra.load_json("dodge_room") or {}).get("best") or 0.0)
        self.meteors: list = []
        self.mesh_ids: dict[int, int] = {}
        self._rng = random.Random(7)
        self._reset()

    def _reset(self):
        self.elapsed = 0.0
        self.lives = START_LIVES
        self.invuln = 0.0
        self.since_spawn = 0.0
        self.meteors.clear()
        for mid in self.mesh_ids.values():
            pass  # meshes are reused by position each frame; nothing to unload
        p = self.world.player
        if p is not None:
            p.teleport(0.0, 0.0, 1.5)
            p.vx = p.vy = p.vz = 0.0
        self.facing = math.pi

    def _se(self, key: str):
        try:
            kagra.se(self.sfx[key])
        except Exception:
            pass

    def _spawn(self):
        margin = 0.6
        x = self._rng.uniform(-ARENA_HALF + margin, ARENA_HALF - margin)
        z = self._rng.uniform(-ARENA_HALF + margin, ARENA_HALF - margin)
        self.meteors.append(spawn_meteor(x, z))

    def update(self, dt):
        dt = min(dt, 0.05)
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        if SMOKE:
            n = kagra.tick_count()
            if n == 8:
                self.world.move_player(0.6, -0.3)
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

        self.elapsed += dt
        self.invuln = max(0.0, self.invuln - dt)
        self.since_spawn += dt
        gap = spawn_gap(self.elapsed)
        if self.since_spawn >= gap:
            self.since_spawn = 0.0
            self._spawn()

        speed = fall_speed(self.elapsed)
        p = self.world.player
        px, pz = (p.x, p.z) if p is not None else (0.0, 0.0)
        survivors = []
        for m in self.meteors:
            m = step_meteor(m, dt, speed)
            if self.invuln <= 0.0 and is_hit(px, pz, m):
                self.lives -= 1
                self.invuln = INVULN_SEC
                self.avatar.feel("surprised", 1.0)
                self._se("hit")
                if self.lives <= 0:
                    self.mode = "result"
                    best = max(self.hi, survival_score(self.elapsed))
                    self.hi = best
                    kagra.save_json("dodge_room", {"best": self.hi})
                continue
            if has_landed(m):
                continue
            survivors.append(m)
        self.meteors = survivors
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
        self.cam.follow(
            p.x, p.y, p.z, yaw=math.pi, lerp=0.22,
            distance=3.0, height=2.0, look_y=1.0,
            bounds_half=ARENA_HALF,
        )
        eng = kagra.get_engine()
        if eng:
            self.cam.update(eng)

    def draw(self):
        kagra.cls(8, 6, 14)
        self.world.draw()
        mets = [(m.x, max(m.y, 0.05), m.z, 0.5) for m in self.meteors]
        if mets:
            kagra.draw_billboard_instances(self.tex_meteor, mets, self.cam)
        kagra.draw_vrm(self.avatar.vrm_id)
        kagra.draw_vignette()
        if self.mode == "title":
            self._banner("Meteor Dodge", "WASD で歩く  隕石を避けて生き残る")
        elif self.mode == "result":
            best = f"  Best {self.hi:5.0f}" if self.hi else ""
            self._banner("GAME OVER", f"Score {survival_score(self.elapsed):5.0f}{best}   SPACE でもう一回")
        else:
            kagra.fill(0, 0, 300, 90, (10, 8, 16), 170)
            kagra.text(f"TIME  {self.elapsed:5.1f}", 18, 16, 24, (255, 220, 200))
            kagra.text(f"LIFE  {max(self.lives, 0)}", 18, 46, 22, (255, 160, 160))

    def _banner(self, title: str, sub: str):
        kagra.fill(0, 0, SW, SH, (6, 4, 10), 140)
        w, _ = kagra.measure(title, 50)
        kagra.text(title, (SW - w) // 2, SH // 2 - 70, 50, (255, 210, 190))
        w2, _ = kagra.measure(sub, 22)
        kagra.text(sub, (SW - w2) // 2, SH // 2 + 10, 22, (220, 210, 225))


if __name__ == "__main__":
    os.makedirs(os.path.dirname(SMOKE_SHOT) or ".", exist_ok=True)
    kagra.init(
        width=SW,
        height=SH,
        title="VRM Meteor Dodge",
        fps=60,
        visible=not SMOKE,
    )
    kagra.run(
        start_scene=DodgeRoom(),
        max_frames=SMOKE_FRAMES + 2 if SMOKE else None,
        fixed_dt=1.0 / 60.0 if SMOKE else None,
    )
