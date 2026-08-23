"""VRM Heart Catch — 3 レーンで奥から飛ぶハートを取る。

エージェント実証用（公開 API のみ）。プロンプトは
docs/agent-runs/20260823-heart-catch/prompt.md。

操作:
  A / D または ← → : レーン移動
  SPACE / ENTER     : スタート / リトライ
  ESC               : 終了

VRM は kagra.ensure_vrm()（checkout に Emma.vrm が無くてもサンプルを取る）。
スモーク: KAGRA_SMOKE=1 python examples/vrm_heart_catch.py
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

from heart_catch_rules import (
    HEART_SPEED,
    ROUND_SEC,
    catch_score,
    is_catch,
    is_miss,
    lane_x,
    spawn_heart,
    step_heart,
)

SW, SH = 960, 540
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "48"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/heart_catch_smoke.png")


def _heart_tex():
    def px(x, y):
        # 粗いハート（32x32）。中央が濃いピンク。
        u = (x - 15.5) / 15.5
        v = (15.5 - y) / 15.5
        left = (u + 0.32) ** 2 + (v - 0.22) ** 2
        right = (u - 0.32) ** 2 + (v - 0.22) ** 2
        bottom = (abs(u) * 1.15) + (0.55 - v)
        inside = left < 0.20 or right < 0.20 or (v < 0.35 and bottom < 0.85 and v > -0.75)
        if not inside:
            return (0, 0, 0, 0)
        return (255, 90, 130, 255)

    return kagra.texture_from_fn(32, 32, px, name="heart")


def _floor_tex():
    def px(x, y):
        c = (x // 8 + y // 8) % 2
        return (70, 42, 58, 255) if c else (48, 28, 42, 255)

    return kagra.texture_from_fn(64, 64, px, name="heart_floor")


class HeartCatch(kagra.Scene):
    def on_enter(self):
        kagra.font()
        kagra.apply_live_look()
        self.tex_heart = _heart_tex()
        self.tex_floor = _floor_tex()
        self.sfx = {
            "ok": kagra.tone("heart_ok", (880, 1320), 0.10, 0.30),
            "ng": kagra.tone("heart_ng", (160, 110), 0.18, 0.40),
            "go": kagra.tone("heart_go", (659, 880), 0.16, 0.28),
        }
        self.avatar = kagra.avatar(str(kagra.ensure_vrm()))
        self.avatar.play("idle", loop=True)
        self.avatar.enable_emotion()
        self.action = kagra.ActionController(self.avatar)
        self.cam = Camera3D(SW, SH, fov_deg=36.0)
        self.cam.use_orbit(radius=6.4, theta=0.0, phi=0.38, target=(0.0, 0.9, 0.0))
        kagra.set_camera3d(self.cam)
        self.mode = "play" if SMOKE else "title"
        self.t = 0.0
        self.hi = int((kagra.load_json("heart_catch") or {}).get("hi") or 0)
        self._reset()

    def _reset(self):
        self.lane = 1
        self.score = 0
        self.combo = 0
        self.lives = 3
        self.left = ROUND_SEC
        self.hearts = [spawn_heart(rng_lane=1)]
        self.spawn_cd = 0.7
        self.msg = ""
        self.msg_t = 0.0

    def _se(self, key: str):
        try:
            kagra.se(self.sfx[key])
        except Exception:
            pass

    def update(self, dt):
        dt = min(dt, 0.05)
        self.t += dt
        if self.msg_t > 0:
            self.msg_t -= dt
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        if SMOKE:
            n = kagra.tick_count()
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
            self._pose(dt)
            return
        if self.mode == "result":
            if kagra.pressed("SPACE") or kagra.pressed("R"):
                self._se("go")
                self.mode = "play"
                self._reset()
            self._pose(dt)
            return

        if kagra.pressed("A") or kagra.pressed("LEFT"):
            self.lane = max(0, self.lane - 1)
        if kagra.pressed("D") or kagra.pressed("RIGHT"):
            self.lane = min(2, self.lane + 1)

        self.left -= dt
        self.spawn_cd -= dt
        if self.spawn_cd <= 0:
            self.hearts.append(spawn_heart(rng_lane=random.randint(0, 2)))
            self.spawn_cd = max(0.38, 0.85 - (ROUND_SEC - self.left) * 0.015)

        nxt = []
        for h in self.hearts:
            h = step_heart(h, dt, HEART_SPEED)
            if is_catch(self.lane, h):
                self.combo += 1
                self.score += catch_score(self.combo)
                self.msg = f"+{catch_score(self.combo)}"
                self.msg_t = 0.6
                self.avatar.feel("joy", min(1.0, 0.4 + self.combo * 0.1))
                self.action.play("clap" if self.combo % 3 == 0 else "nod")
                self._se("ok")
                continue
            if is_miss(h):
                self.combo = 0
                self.lives -= 1
                self.msg = "miss"
                self.msg_t = 0.6
                self.avatar.feel("sorrow", 0.8)
                self._se("ng")
                continue
            nxt.append(h)
        self.hearts = nxt[-12:]

        if self.lives <= 0 or self.left <= 0:
            self.mode = "result"
            self.hi = max(self.hi, self.score)
            kagra.save_json("heart_catch", {"hi": self.hi})
            self.avatar.feel("joy" if self.lives > 0 else "sorrow", 1.0)
        self._pose(dt)

    def _pose(self, dt):
        want = "walk" if self.mode == "play" else "idle"
        if getattr(self.avatar, "clip", None) != want:
            self.avatar.play(want, loop=True)
        self.avatar.update(dt)
        self.action.update(dt)
        self.avatar.set_position(lane_x(self.lane), 0.0, 0.0)
        self.avatar.set_yaw(math.pi)  # カメラ（+Z）を向く
        self.cam.orbit_tgt = (lane_x(self.lane) * 0.35, 0.9, 0.0)
        eng = kagra.get_engine()
        if eng:
            self.cam.update(eng)

    def draw(self):
        kagra.cls(18, 10, 22)
        fv, fi = kagra.disk_mesh(0.0, 0.0, 0.0, 5.2, 48)
        kagra.draw_mesh_3d(self.tex_floor, fv, fi)
        hearts = [
            (lane_x(h.lane), 0.85 + 0.08 * math.sin(h.phase), h.z, 0.28)
            for h in self.hearts
        ]
        if hearts:
            kagra.draw_billboard_instances(self.tex_heart, hearts, self.cam)
        kagra.draw_vrm(self.avatar.vrm_id)
        kagra.draw_vignette()
        if self.mode == "title":
            self._banner("Heart Catch", "A / D でレーン  SPACE でスタート")
        elif self.mode == "result":
            self._banner("RESULT", f"Score {self.score}   Best {self.hi}   SPACE でもう一回")
        else:
            kagra.fill(0, 0, 280, 110, (12, 8, 18), 170)
            kagra.text(f"SCORE  {self.score}", 18, 16, 26, (255, 210, 160))
            kagra.text(f"TIME   {max(0.0, self.left):4.1f}", 18, 48, 22, (200, 220, 255))
            kagra.text(f"LIFE   {self.lives}   COMBO {self.combo}", 18, 76, 20, (255, 140, 170))
            if self.msg_t > 0:
                w, _ = kagra.measure(self.msg, 32)
                kagra.text(self.msg, (SW - w) // 2, 120, 32, (255, 230, 180))

    def _banner(self, title: str, sub: str):
        kagra.fill(0, 0, SW, SH, (8, 6, 14), 140)
        w, _ = kagra.measure(title, 52)
        kagra.text(title, (SW - w) // 2, SH // 2 - 70, 52, (255, 200, 140))
        w2, _ = kagra.measure(sub, 22)
        kagra.text(sub, (SW - w2) // 2, SH // 2 + 10, 22, (220, 210, 230))
        if self.hi:
            hs = f"Best  {self.hi}"
            w3, _ = kagra.measure(hs, 20)
            kagra.text(hs, (SW - w3) // 2, SH // 2 + 50, 20, (255, 180, 150))


if __name__ == "__main__":
    os.makedirs(os.path.dirname(SMOKE_SHOT) or ".", exist_ok=True)
    kagra.init(
        width=SW,
        height=SH,
        title="VRM Heart Catch",
        fps=60,
        visible=not SMOKE,
    )
    kagra.run(
        start_scene=HeartCatch(),
        max_frames=SMOKE_FRAMES + 2 if SMOKE else None,
        fixed_dt=1.0 / 60.0 if SMOKE else None,
    )
