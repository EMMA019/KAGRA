"""VRM Island Relic Run — 30s outdoor relic collect showcase.

エージェント実証用（公開 API のみ）。ログ:
docs/agent-runs/20260824-relic-run-walk-assets/

Sample VRM via ensure_vrm is Alicia Solid (ニコニ立体ちゃん) © Dwango —
credit the character if you post screenshots.

Art (CC0, not in the pip wheel):
  Kenney Mini Forest + Nature Kit glTF (Prop("tree.glb") etc.) — https://kenney.nl (CC0)
  Poly Haven aerial_grass_rock + kloofendal_48d_partly_cloudy_puresky —
  https://polyhaven.com (CC0). See examples/assets/relic_run/LICENSE.md.

Walk: built-in VrmAvatar idle/walk (T-pose arm drop + opposite-phase swing).
Mixamo/synthetic BVH walk is NOT loaded — those clips rest in T-pose, so
bind*delta leaves Emma's arms folded forward like a carry/formal pose.

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
from pathlib import Path

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(_HERE))
sys.path.insert(0, _HERE)

import kagra
from kagra.camera3d import Camera3D
from kagra.contracts import AssetKind, resolve_asset

from relic_run_rules import (
    CAM_DISTANCE,
    GLTF_HALF_Y,
    JUMP,
    PEDESTAL,
    PLAYER_SPEED,
    RELIC_GLOW,
    RELIC_SCALE,
    RELIC_XZ,
    ROCK_PLACEMENTS,
    ROUND_SEC,
    START_XZ,
    TREE_PLACEMENTS,
    WATER_Y,
    can_pick,
    grade_for,
    hero_theta,
    nearest_live,
    round_score,
    sit_y,
    spawn_relics,
    start_face,
)

SW, SH = 960, 540
HALF = 24.0
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "40"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/relic_run_smoke.png")
_ASSETS = Path(_HERE) / "assets" / "relic_run"
_KENNEY = _ASSETS / "kenney"
_POLYHAVEN = _ASSETS / "polyhaven"


def _gltf(name: str) -> str:
    path = (_KENNEY / name).resolve()
    if not path.is_file():
        raise FileNotFoundError(
            f"Relic Run glTF missing: {path}. "
            "CC0 files live under examples/assets/relic_run/kenney/."
        )
    return str(path)


def _bind_locomotion(avatar) -> None:
    """Keep engine idle/walk. Mixamo/BVH walk rest is T-pose → folded arms.

    A shipped VRMA next to the example is loaded with no silent except.
    """
    vrma = _ASSETS / "walk.vrma"
    if vrma.is_file():
        try:
            avatar.load_motion("walk", str(vrma))
        except Exception as exc:
            raise RuntimeError(
                f"Relic Run walk.vrma failed to load: {vrma}"
            ) from exc
        print(f"[RelicRun] walk ← {vrma}")
        return
    print("[RelicRun] walk ← built-in idle/walk (arm swing). Mixamo/BVH skipped.")


def _relic_glow_tex():
    def px(x, y):
        d = math.hypot(x - 31.5, y - 31.5) / 31.5
        core = max(0.0, 1.0 - d * 0.95)
        rim = max(0.0, 1.0 - abs(d - 0.55) * 6.0)
        a = max(core, rim * 0.65)
        return (255, 230, 110, max(0, int(a * 255)))

    return kagra.texture_from_fn(64, 64, px, name="relic_glow")


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


def _place_gltf(name: str, x: float, z: float, scale: float, yaw: float, world, *, collision: bool = True):
    half = GLTF_HALF_Y[name]
    gy = world.ground_y(x, z)
    return kagra.Prop(
        _gltf(name),
        x=x, y=sit_y(gy, half, scale), z=z,
        scale=scale, yaw=yaw, world=world, collision=collision,
    )


class RelicRun(kagra.Scene):
    def on_enter(self):
        kagra.font()
        kagra.Prop.clear()

        vrm_path = resolve_asset(AssetKind.VRM, "Emma", required=False)
        if vrm_path is None:
            vrm_path = kagra.ensure_vrm()
        self.avatar = kagra.avatar(str(vrm_path))
        _bind_locomotion(self.avatar)
        self.avatar.stop_upper()
        self.avatar.play("idle", loop=True)
        self.avatar.enable_emotion()
        self.action = kagra.ActionController(self.avatar)

        self.world = kagra.World3D(half=HALF)
        self.world.set_height_fn(
            kagra.overworld_height, cells=8, tile=10.0, stream_radius=28.0,
        )
        self.world.set_water_y(WATER_Y)
        self.world.add_player(*START_XZ)

        grass = _POLYHAVEN / "aerial_grass_rock_diff_1k.jpg"
        if grass.is_file():
            tex = kagra.load(str(grass))
        else:
            print("[RelicRun] Poly Haven grass missing; procedural terrain fallback")

            def _terrain_px(_x, _y):
                return (76, 140, 62, 255)

            tex = kagra.texture_from_fn(128, 128, _terrain_px, name="relic_land")
        self.world.bake_terrain(tex)
        wall = kagra.solid_tex((148, 128, 108))
        self.world.bake(tex, wall)

        kagra.apply_outdoor_look()
        sky_png = _POLYHAVEN / "kloofendal_48d_partly_cloudy_puresky_1k.png"
        self.sky_stage = None
        if sky_png.is_file():
            self.sky_stage = kagra.stage(str(sky_png), radius=48.0)
            kagra.set_hdri(str(sky_png), strength=0.32)
        kagra.set_fog(start=22.0, end=46.0, color=(150, 175, 195), enabled=True)
        kagra.set_bloom(threshold=0.78, intensity=0.32)
        if not SMOKE:
            kagra.set_spot_light(
                4.0, 10.0, -2.0, -0.25, -1.0, 0.15,
                angle=0.9, penumbra=0.35, intensity=1.35, radius=22.0,
                r=1.0, g=0.95, b=0.82, slot=0,
            )
            kagra.set_point_light(
                -2.0, 3.5, 2.0, r=0.55, g=0.75, b=1.0,
                intensity=0.55, radius=14.0, slot=1,
            )

        for name, tx, tz, scale, yaw in TREE_PLACEMENTS:
            hit = name.startswith(("tree", "fence", "tent", "flag"))
            _place_gltf(name, tx, tz, scale, yaw, self.world, collision=hit)
        for name, sx, sz, scale, yaw in ROCK_PLACEMENTS:
            _place_gltf(name, sx, sz, scale, yaw, self.world)

        ped_name, ped_scale, ped_yaw = PEDESTAL
        self.pedestals = []
        for rx, rz in RELIC_XZ:
            self.pedestals.append(
                _place_gltf(ped_name, rx, rz, ped_scale, ped_yaw, self.world, collision=True)
            )

        self.tex_glow = _relic_glow_tex()
        self.relic_props: list = []
        ped_half = GLTF_HALF_Y[ped_name]
        for rx, rz in RELIC_XZ:
            gy = self.world.ground_y(rx, rz)
            top = gy + ped_half * ped_scale * 2.0
            prop = kagra.Prop(
                "sphere", x=rx, y=top + RELIC_SCALE * 0.55, z=rz,
                scale=RELIC_SCALE, color="gold", world=self.world, collision=False,
                metallic=0.85, roughness=0.22,
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

    def _relic_bob_y(self, relic) -> float:
        ped_name, ped_scale, _yaw = PEDESTAL
        gy = self.world.ground_y(relic.x, relic.z)
        top = gy + GLTF_HALF_Y[ped_name] * ped_scale * 2.0
        return top + RELIC_SCALE * 0.55 + 0.16 * math.sin(relic.phase)

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
            prop.set_position(relic.x, self._relic_bob_y(relic), relic.z)
        p = self.world.player
        if p is not None:
            gy = self.world.ground_y(*START_XZ)
            p.x, p.z = START_XZ[0], START_XZ[1]
            p.y = gy
            p.vx = p.vy = p.vz = 0.0
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

        self.time_left -= dt
        kagra.Prop.update_all(dt)
        self.walk.step(dt)

        p = self.world.player
        if p is not None:
            for relic, prop in zip(self.relics, self.relic_props):
                if not relic.live:
                    continue
                relic.phase += dt * 3.2
                prop.y = self._relic_bob_y(relic)
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
        kagra.cls(150, 175, 195)
        if self.sky_stage is not None:
            self.sky_stage.draw()
        else:
            kagra.sky(radius=42.0, look=False)
        self.world.draw()
        kagra.water(WATER_Y, half=HALF, world=self.world)
        kagra.Prop.draw_all()

        glow_items = []
        for relic in self.relics:
            if not relic.live:
                continue
            y = self._relic_bob_y(relic)
            size = RELIC_GLOW + 0.12 * math.sin(relic.phase * 1.7)
            glow_items.append((relic.x, y + 0.15, relic.z, size))
            glow_items.append((relic.x, y + 0.15, relic.z, size * 0.45))
        if glow_items:
            kagra.draw_billboard_instances(self.tex_glow, glow_items, self.cam)

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
        credit = "VRM sample: Alicia Solid © Dwango   Art: Kenney + Poly Haven (CC0)"
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
