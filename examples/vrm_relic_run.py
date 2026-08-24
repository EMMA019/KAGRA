"""VRM Island Relic Run — 30s outdoor relic collect showcase.

エージェント実証用（公開 API のみ）。ログ:
docs/agent-runs/2026-08-24-island-relic-run.md

Sample VRM via ensure_vrm is Alicia Solid (ニコニ立体ちゃん) © Dwango —
credit the character if you post screenshots.

操作:
  WASD / 左スティック : 歩く
  マウス / 右スティック : 視点（三人称のみ）
  SPACE / A           : ジャンプ
  SPACE / ENTER       : スタート（タイトル）/ リトライ（結果）
  ESC                 : 終了

スモーク: KAGRA_SMOKE=1 python examples/vrm_relic_run.py
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
from kagra.contracts import AssetKind, resolve_asset
from kagra.land import terrain_rgba

from relic_run_rules import (
    CAM_DISTANCE,
    JUMP,
    PLAYER_SPEED,
    RELIC_XZ,
    ROUND_SEC,
    START_XZ,
    STONE_XZ,
    TREE_XZ,
    WATER_Y,
    can_pick,
    grade_for,
    hero_theta,
    nearest_live,
    round_score,
    spawn_relics,
    start_face,
)

SW, SH = 960, 540
HALF = 24.0
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "40"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/relic_run_smoke.png")


def _terrain_px(x, y):
    return terrain_rgba(x / 127.0, y / 127.0, half=HALF)


def _relic_glow_tex():
    def px(x, y):
        d = math.hypot(x - 15.5, y - 15.5) / 15.5
        core = max(0.0, 1.0 - d * 1.15)
        return (255, 220, 90, max(0, int(core * 255)))

    return kagra.texture_from_fn(32, 32, px, name="relic_glow")


def _make_sfx() -> dict[str, str]:
    return {
        "pick": kagra.tone("relic_pick", (880, 1320, 1760), 0.12, 0.30),
        "start": kagra.tone("relic_start", (523, 659, 784), 0.26, 0.28),
        "win": kagra.tone("relic_win", (523, 659, 784, 1046), 0.42, 0.26),
        "lose": kagra.tone("relic_lose", (392, 311, 247), 0.40, 0.30),
        "tick": kagra.tone("relic_tick", (660,), 0.07, 0.18),
    }


def _se(sfx: dict, key: str, volume: float = 1.0):
    path = sfx.get(key)
    if not path:
        return
    try:
        kagra.play_se(path, volume=volume)
    except Exception:
        try:
            kagra.se(path, vol=volume)
        except Exception:
            pass


class RelicRun(kagra.Scene):
    def on_enter(self):
        kagra.font()
        kagra.Prop.clear()

        vrm_path = resolve_asset(AssetKind.VRM, "Emma", required=False)
        if vrm_path is None:
            vrm_path = kagra.ensure_vrm()
        self.avatar = kagra.avatar(str(vrm_path))
        walk = resolve_asset(AssetKind.ANY, "walk", required=False)
        if walk is not None:
            try:
                self.avatar.load_motion("walk", str(walk))
            except Exception:
                pass
        self.avatar.play("idle", loop=True)
        self.avatar.enable_emotion()
        self.action = kagra.ActionController(self.avatar)

        self.world = kagra.World3D(half=HALF)
        self.world.set_height_fn(
            kagra.overworld_height, cells=8, tile=10.0, stream_radius=28.0,
        )
        self.world.set_water_y(WATER_Y)
        self.world.add_player(*START_XZ)

        tex = kagra.texture_from_fn(128, 128, _terrain_px, name="relic_land")
        self.world.bake_terrain(tex)
        wall = kagra.solid_tex((148, 128, 108))
        self.world.bake(tex, wall)

        try:
            kagra.apply_outdoor_look()
        except Exception:
            pass
        if not SMOKE:
            try:
                kagra.set_spot_light(
                    4.0, 10.0, -2.0, -0.25, -1.0, 0.15,
                    angle=0.9, penumbra=0.35, intensity=1.35, radius=22.0,
                    r=1.0, g=0.95, b=0.82, slot=0,
                )
                kagra.set_point_light(
                    -2.0, 3.5, 2.0, r=0.55, g=0.75, b=1.0,
                    intensity=0.55, radius=14.0, slot=1,
                )
            except Exception:
                pass

        self.tree_props = []
        for tx, tz in TREE_XZ:
            gy = self.world.ground_y(tx, tz)
            trunk = kagra.Prop(
                "cylinder", x=tx, y=gy + 0.7, z=tz,
                scale=(0.35, 1.4, 0.35), color=(90, 60, 35), world=self.world,
            )
            crown = kagra.Prop(
                "sphere", x=0.0, y=1.05, z=0.0, scale=1.35, color=(48, 130, 55),
                collision=False,
            )
            crown.set_parent(trunk, keep_world=False)
            self.tree_props.append(trunk)

        for sx, sz in STONE_XZ:
            gy = self.world.ground_y(sx, sz)
            kagra.Prop(
                "sphere", x=sx, y=gy + 0.28, z=sz, scale=0.55,
                color=(150, 145, 140), world=self.world,
            )

        self.tex_glow = _relic_glow_tex()
        self.relic_props: list = []
        for rx, rz in RELIC_XZ:
            gy = self.world.ground_y(rx, rz)
            prop = kagra.Prop(
                "sphere", x=rx, y=gy + 0.45, z=rz, scale=0.55,
                color="gold", world=self.world, collision=False,
            )
            self.relic_props.append(prop)

        kagra.Prop.bake_all()

        face0 = start_face()
        yaw0 = hero_theta(face0)
        self.cam = Camera3D(SW, SH, fov_deg=50.0)
        p = self.world.player
        self.cam.follow(
            p.x, p.y, p.z,
            lerp=1.0, yaw=yaw0, distance=CAM_DISTANCE, height=2.7, look_y=1.1,
        )
        kagra.set_camera3d(self.cam)
        self.walk = kagra.Walk(
            self.world, self.cam,
            speed=PLAYER_SPEED, jump=JUMP, yaw=yaw0,
            distance=CAM_DISTANCE, height=2.7, look_y=1.1,
        )
        self.walk.face = face0

        self.sfx = _make_sfx()
        self.mode = "play" if SMOKE else "title"
        self.t = 0.0
        self.hi = int((kagra.load_json("relic_run") or {}).get("hi") or 0)
        self.title = kagra.Label("Island Relic Run", 18, 14, 26, (255, 230, 150))
        self.hud = kagra.Label("", 18, 48, 18, (200, 220, 235))
        self._reset_round()

    def _reset_round(self):
        self.relics = spawn_relics()
        self.picked = 0
        self.time_left = ROUND_SEC
        self.score = 0
        self.msg = ""
        self.msg_t = 0.0
        self.grade = "D"
        for prop, relic in zip(self.relic_props, self.relics):
            prop.enabled = True
            gy = self.world.ground_y(relic.x, relic.z)
            prop.set_position(relic.x, gy + 0.45, relic.z)
        p = self.world.player
        if p is not None:
            # respawn near start on land
            gy = self.world.ground_y(*START_XZ)
            try:
                p.x, p.z = START_XZ[0], START_XZ[1]
                p.y = gy
                p.vx = p.vy = p.vz = 0.0
            except Exception:
                pass
        self.walk.face = start_face()
        self.walk.yaw = hero_theta(self.walk.face)

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
            if n == 6:
                kagra.inject_key("W")
            if n == 14:
                kagra.inject_key("SPACE")
            if n == 24:
                kagra.screenshot(SMOKE_SHOT)
            if n >= SMOKE_FRAMES:
                kagra.quit()
                return

        if self.mode == "title":
            if kagra.pressed("SPACE") or kagra.pressed("RETURN"):
                _se(self.sfx, "start")
                self.mode = "play"
                self._reset_round()
            self._pose(dt, move=False)
            return

        if self.mode == "result":
            if kagra.pressed("SPACE") or kagra.pressed("R") or kagra.pressed("RETURN"):
                _se(self.sfx, "start")
                self.mode = "play"
                self._reset_round()
            self._pose(dt, move=False)
            return

        # play
        self.time_left -= dt
        kagra.Prop.update_all(dt)
        self.walk.step(dt)

        p = self.world.player
        if p is not None:
            for relic, prop in zip(self.relics, self.relic_props):
                if not relic.live:
                    continue
                relic.phase += dt * 3.2
                bob = 0.45 + 0.12 * math.sin(relic.phase)
                gy = self.world.ground_y(relic.x, relic.z)
                prop.y = gy + bob
                if can_pick(p.x, p.z, relic.x, relic.z):
                    relic.live = False
                    prop.enabled = False
                    self.picked += 1
                    self.msg = f"Relic {self.picked}/5"
                    self.msg_t = 0.9
                    self.avatar.feel("joy", min(1.0, 0.45 + self.picked * 0.12))
                    self.action.play("clap" if self.picked >= 5 else "nod")
                    _se(self.sfx, "pick")

        done = self.picked >= len(self.relics) or self.time_left <= 0.0
        if done:
            self.score = round_score(self.picked, max(0.0, self.time_left))
            self.grade = grade_for(self.score)
            self.hi = max(self.hi, self.score)
            kagra.save_json("relic_run", {"hi": self.hi, "grade": self.grade})
            self.mode = "result"
            if self.picked >= len(self.relics):
                self.avatar.feel("joy", 1.0)
                self.action.play("banzai")
                _se(self.sfx, "win")
            else:
                self.avatar.feel("sorrow", 0.85)
                self.action.play("bow")
                _se(self.sfx, "lose")

        self._pose(dt, move=True)

    def _pose(self, dt, *, move: bool):
        p = self.world.player
        moving = False
        if move and p is not None:
            moving = p.vx * p.vx + p.vz * p.vz > 0.04 or abs(getattr(p, "vy", 0.0)) > 0.4
        want = "walk" if moving else "idle"
        if getattr(self.avatar, "clip", None) != want:
            self.avatar.play(want, loop=True)
        self.avatar.update(dt)
        self.action.update(dt)
        if p is None:
            return
        # Body uses walk.face — never walk.yaw (S would not turn around).
        self.avatar.set_position(p.x, p.y, p.z)
        self.avatar.set_yaw(self.walk.face)

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
        kagra.water(WATER_Y, half=HALF, world=self.world)
        kagra.Prop.draw_all()

        # Procedural glow billboards over live relics
        glow_items = []
        for relic in self.relics:
            if not relic.live:
                continue
            gy = self.world.ground_y(relic.x, relic.z)
            bob = 0.55 + 0.12 * math.sin(relic.phase)
            size = 0.42 + 0.06 * math.sin(relic.phase * 1.7)
            glow_items.append((relic.x, gy + bob, relic.z, size))
        if glow_items:
            try:
                kagra.draw_billboard_instances(self.tex_glow, glow_items, self.cam)
            except Exception:
                pass

        kagra.draw_vrm(self.avatar.vrm_id)
        kagra.draw_vignette()

        if self.mode == "title":
            self._banner(
                "Island Relic Run",
                "島の遺跡を 30 秒で集めろ  SPACE でスタート",
            )
            return
        if self.mode == "result":
            self._banner(
                "RESULT",
                f"Score {self.score}  Grade {self.grade}  Best {self.hi}  SPACE でもう一回",
            )
            return

        kagra.fill(0, 0, 380, 100, (8, 18, 28), 160)
        self.title.draw()
        left = max(0.0, self.time_left)
        hint = nearest_live(
            self.world.player.x if self.world.player else START_XZ[0],
            self.world.player.z if self.world.player else START_XZ[1],
            self.relics,
        )
        tip = ""
        if hint is not None and self.world.player is not None:
            tip = f"  nearest {math.hypot(self.world.player.x - hint.x, self.world.player.z - hint.z):.1f}m"
        self.hud.text = f"RELICS  {self.picked}/5   TIME  {left:4.1f}{tip}"
        self.hud.draw()
        if self.msg_t > 0 and self.msg:
            w, _ = kagra.measure(self.msg, 30)
            kagra.text(self.msg, (SW - w) // 2, 120, 30, (255, 240, 160))

    def _banner(self, title: str, sub: str):
        kagra.fill(0, 0, SW, SH, (6, 12, 20), 150)
        w, _ = kagra.measure(title, 48)
        kagra.text(title, (SW - w) // 2, SH // 2 - 70, 48, (255, 220, 130))
        w2, _ = kagra.measure(sub, 20)
        pulse = 170 + int(50 * math.sin(self.t * 3.5))
        kagra.text(sub, (SW - w2) // 2, SH // 2 + 10, 20, (pulse, pulse, 255))
        if self.hi:
            hs = f"Best  {self.hi}"
            w3, _ = kagra.measure(hs, 18)
            kagra.text(hs, (SW - w3) // 2, SH // 2 + 48, 18, (255, 190, 140))
        credit = "VRM sample: Alicia Solid © Dwango"
        wc, _ = kagra.measure(credit, 14)
        kagra.text(credit, (SW - wc) // 2, SH - 36, 14, (160, 175, 190))


if __name__ == "__main__":
    os.makedirs(os.path.dirname(SMOKE_SHOT) or ".", exist_ok=True)
    kagra.init(
        width=SW,
        height=SH,
        title="VRM Island Relic Run",
        fps=60,
        visible=not SMOKE,
    )
    kagra.run(
        start_scene=RelicRun(),
        max_frames=SMOKE_FRAMES + 2 if SMOKE else None,
        fixed_dt=1.0 / 60.0 if SMOKE else None,
    )
