"""VRM Crest Isle — Mario-like outdoor collectathon (not Nintendo IP).

Wide peninsula: grass meadow in front, sea on the left horizon, mountains
ahead. Kenney Mini Forest / Nature Kit / Fantasy Town / Castle / Mini Dungeon
+ Poly Haven grass & puresky. Public APIs only.

エージェント実証: docs/agent-runs/20260824-open-world/

Sample VRM via ensure_vrm is Alicia Solid (ニコニ立体ちゃん) © Dwango —
credit the character if you post screenshots.

Art (CC0, not in the pip wheel): examples/assets/open_world/LICENSE.md

Walk: ``avatar.set_locomotion`` (idle/walk/run speed blend). Local Mixamo
Idle/Walk/Run FBX is loaded when present (rest+roll retarget onto VRoid).
Otherwise built-in clips. The ``walk`` alias (synthetic_walk.bvh) is not used.
Motor: ``CharacterController`` (accel/decel, slope sit, step-up, jump).
Upper-body clap/banzai stay on ActionController and do not fight walk arms.
Spatial audio: looping sea to the west + pickup SE at the crest/coin.
Fake AO: a ground blob under the feet. Pickups spawn a CPU billboard burst.
Coins are gold PBR spheres (metallic 1 / roughness 0.12), not yellow plastic.

操作:
  WASD / 左スティック : 歩く
  マウス / 右スティック : 視点（三人称のみ）
  [ ] / - = / ホイール : カメラ距離（頭の中と Tiny speck には入らない）
  SPACE / A           : ジャンプ
  SPACE / ENTER       : スタート（タイトル）/ リトライ（結果）
  ESC                 : 終了

スモーク: KAGRA_SMOKE=1 python examples/vrm_open_world.py
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

from open_world_rules import (
    CAM_DISTANCE,
    CAM_HEIGHT,
    CAM_LOOK_Y,
    CAM_MAX_DISTANCE,
    CAM_MIN_DISTANCE,
    CAM_ZOOM_STEP,
    CELLS,
    COIN_GLOW,
    COIN_HOVER,
    COIN_SCALE,
    COIN_XZ,
    FOV_DEG,
    GLTF_HALF_Y,
    GOLD_METALLIC,
    GOLD_ROUGHNESS,
    GRASS_TINT,
    HALF,
    JUMP,
    LOD_CELLS,
    LOD_RADIUS,
    PEAK_XZ,
    PICK_REACH,
    PLAYER_SPEED,
    SPHERE_HALF_Y,
    STAR_MODELS,
    STAR_NEED,
    STAR_SCALES,
    STAR_XZ,
    START_XZ,
    STREAM_RADIUS,
    TERRAIN_UV_BLEND,
    TERRAIN_UV_PAD,
    TERRAIN_UV_PERIOD,
    TILE,
    VISTA_PROPS,
    WATER_Y,
    SparkBurst,
    chunk_decor,
    grade_for,
    hero_theta,
    nearest_live,
    round_score,
    sit_y,
    spawn_coins,
    spawn_stars,
    start_face,
    won,
)

SW, SH = 960, 540
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "40"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/open_world_smoke.png")
_ASSETS = Path(_HERE) / "assets" / "open_world"
_KENNEY = _ASSETS / "kenney"
_POLY = (
    _ASSETS / "polyhaven",
    Path(_HERE) / "assets" / "relic_run" / "polyhaven",
)


def _gltf(rel: str) -> str:
    path = (_KENNEY / rel).resolve()
    if not path.is_file():
        raise FileNotFoundError(
            f"Crest Isle glTF missing: {path}. "
            "CC0 files live under examples/assets/open_world/kenney/."
        )
    return str(path)


def _poly(name: str) -> Path | None:
    for folder in _POLY:
        cand = folder / name
        if cand.is_file():
            return cand
    return None


def _bind_locomotion(avatar) -> None:
    """Optional VRMA, else local Mixamo Idle/Walk/Run, else built-in clips.

    Never resolves the ``walk`` alias (synthetic_walk.bvh overwrote built-in).
    """
    vrma = _ASSETS / "walk.vrma"
    if vrma.is_file():
        try:
            avatar.load_motion("walk", str(vrma))
        except Exception as exc:
            raise RuntimeError(
                f"Crest Isle walk.vrma failed to load: {vrma}"
            ) from exc
        print(f"[CrestIsle] walk ← {vrma}")
        return
    loaded = avatar.bind_locomotion()
    if loaded:
        print(f"[CrestIsle] locomotion ← Mixamo {loaded}")
        return
    print("[CrestIsle] walk ← built-in idle/walk/run (arm swing). Mixamo FBX not found.")


def _glow_tex():
    def px(x, y):
        d = math.hypot(x - 31.5, y - 31.5) / 31.5
        core = max(0.0, 1.0 - d * 0.95)
        rim = max(0.0, 1.0 - abs(d - 0.55) * 6.0)
        a = max(core, rim * 0.65)
        return (255, 230, 110, max(0, int(a * 255)))

    return kagra.texture_from_fn(64, 64, px, name="crest_glow")


def _blob_tex():
    """Soft ground ellipse. Not SSAO — a cheap always-on character blob."""

    def px(x, y):
        nx = (x - 31.5) / 30.5
        ny = (y - 31.5) / 22.0
        d = math.hypot(nx, ny)
        a = max(0.0, 1.0 - d)
        a = a * a * 0.52
        return (6, 8, 12, max(0, int(a * 255)))

    return kagra.texture_from_fn(64, 64, px, name="crest_blob")


def _spark_tex():
    def px(x, y):
        d = math.hypot(x - 15.5, y - 15.5) / 15.5
        core = max(0.0, 1.0 - d * 1.35)
        a = core * core
        return (255, 240, 170, max(0, int(a * 255)))

    return kagra.texture_from_fn(32, 32, px, name="crest_spark")


# West water. Spawn looks +Z; sea is screen-left (-X). Looping drone lives here.
SEA_LOOP_XZ = (-28.0, 8.0)


def _make_sfx() -> dict[str, str]:
    return {
        "coin": kagra.tone("crest_coin", (988, 1318), 0.09, 0.28),
        "star": kagra.tone("crest_star", (784, 1174, 1568), 0.16, 0.30),
        "start": kagra.tone("crest_start", (523, 659, 784), 0.26, 0.28),
        "win": kagra.tone("crest_win", (523, 659, 784, 1046), 0.42, 0.26),
        "tick": kagra.tone("crest_tick", (660,), 0.07, 0.18),
        "sea": kagra.tone(
            "crest_sea", (98, 131, 165), duration=1.8, volume=0.22, decay=False,
        ),
    }


def _se(sfx: dict, key: str, volume: float = 1.0, *, pos=None):
    path = sfx.get(key)
    if not path:
        return
    try:
        if pos is not None:
            kagra.play_se(
                path, volume=volume, x=pos[0], y=pos[1], z=pos[2],
                ref_distance=2.0, max_distance=28.0,
            )
        else:
            kagra.play_se(path, volume=volume)
    except Exception:
        kagra.sound("coin" if key in ("coin", "star") else "ok")


def _place_gltf(rel: str, x: float, z: float, scale: float, yaw: float, world, *, collision: bool):
    half = GLTF_HALF_Y[rel]
    gy = world.ground_y(x, z)
    return kagra.Prop(
        _gltf(rel),
        x=x, y=sit_y(gy, half, scale), z=z,
        scale=scale, yaw=yaw, world=world, collision=collision,
    )


class CrestIsle(kagra.Scene):
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
            kagra.open_world_height,
            cells=CELLS, tile=TILE, stream_radius=STREAM_RADIUS,
            lod_radius=LOD_RADIUS, lod_cells=LOD_CELLS,
        )
        self.world.set_water_y(WATER_Y)
        # Crest Isle meadow only: world-continuous UVs inside the JPEG moss,
        # not a 16 m ClampToEdge stamp of the dirt rim.
        self.world.terrain_uv_half = HALF
        self.world.terrain_uv_period = TERRAIN_UV_PERIOD
        self.world.terrain_uv_blend = TERRAIN_UV_BLEND
        self.world.terrain_uv_pad = TERRAIN_UV_PAD
        # bake_terrain streams immediately and calls _fill_chunk.
        self._chunk_props = 0
        self.world.set_chunk_fill(self._fill_chunk)
        self.world.add_player(*START_XZ)

        grass = _poly("aerial_grass_rock_diff_1k.jpg")
        if grass is not None:
            tex = kagra.load(str(grass))
        else:
            print("[CrestIsle] Poly Haven grass missing; procedural terrain fallback")

            def _terrain_px(_x, _y):
                return (76, 140, 62, 255)

            tex = kagra.texture_from_fn(128, 128, _terrain_px, name="crest_land")
        # Shared JPEG is brown soil+rock. Tint mesh_mat.base here only (Relic Run keeps 1,1,1).
        self.world.terrain_base = GRASS_TINT
        self.world.bake_terrain(tex)
        wall = kagra.solid_tex((148, 128, 108))
        self.world.bake(tex, wall)

        kagra.apply_outdoor_look()
        sky_png = _poly("kloofendal_48d_partly_cloudy_puresky_1k.png")
        self.sky_stage = None
        if sky_png is not None:
            self.sky_stage = kagra.stage(str(sky_png), radius=140.0)
            # IBL is fill (Lambert * albedo * 0.35). 0.95 blew the meadow white.
            kagra.set_hdri(str(sky_png), strength=0.32)
        kagra.set_fog(start=48.0, end=102.0, color=(150, 175, 195), enabled=True)
        kagra.set_bloom(threshold=0.80, intensity=0.28)
        # Vector toward the sun (+Y). -1.0 left the ground at Lambert floor 0.2.
        kagra.set_light_dir(-0.32, 1.0, 0.22)
        if not SMOKE:
            kagra.set_spot_light(
                6.0, 18.0, -8.0, -0.18, -1.0, 0.22,
                angle=0.95, penumbra=0.40, intensity=1.15, radius=36.0,
                r=1.0, g=0.96, b=0.86, slot=0,
            )
            kagra.set_point_light(
                -4.0, 5.0, 2.0, r=0.45, g=0.70, b=1.0,
                intensity=0.40, radius=18.0, slot=1,
            )

        for rel, x, z, scale, yaw, hit in VISTA_PROPS:
            _place_gltf(rel, x, z, scale, yaw, self.world, collision=hit)

        self.tex_glow = _glow_tex()
        self.tex_blob = _blob_tex()
        self.tex_spark = _spark_tex()
        self.sparks = SparkBurst()
        self.star_props = []
        for (sx, sz), model, sc in zip(STAR_XZ, STAR_MODELS, STAR_SCALES):
            self.star_props.append(
                _place_gltf(model, sx, sz, sc, 0.15, self.world, collision=False)
            )
        self.coin_props = []
        for cx, cz in COIN_XZ:
            gy = self.world.ground_y(cx, cz)
            self.coin_props.append(
                kagra.Prop(
                    "sphere",
                    x=cx, y=sit_y(gy, SPHERE_HALF_Y, COIN_SCALE) + COIN_HOVER, z=cz,
                    scale=COIN_SCALE, world=self.world, collision=False,
                    color="gold", metallic=GOLD_METALLIC, roughness=GOLD_ROUGHNESS,
                )
            )

        kagra.Prop.bake_all()

        face0 = start_face()
        yaw0 = hero_theta(face0)
        self.cam = Camera3D(SW, SH, fov_deg=FOV_DEG)
        p = self.world.player
        self.cam.follow(
            p.x, p.y, p.z,
            lerp=1.0, yaw=yaw0,
            distance=CAM_DISTANCE, height=CAM_HEIGHT, look_y=CAM_LOOK_Y,
            min_distance=CAM_MIN_DISTANCE,
            max_distance=CAM_MAX_DISTANCE,
        )
        kagra.set_camera3d(self.cam)
        self.walk = kagra.Walk(
            self.world, self.cam,
            speed=PLAYER_SPEED, jump=JUMP, yaw=yaw0,
            distance=CAM_DISTANCE, height=CAM_HEIGHT, look_y=CAM_LOOK_Y,
            min_distance=CAM_MIN_DISTANCE,
            max_distance=CAM_MAX_DISTANCE,
            controller=kagra.CharacterController(
                speed=PLAYER_SPEED, jump=JUMP, accel=14.0, decel=22.0,
            ),
        )
        self.walk.face = face0

        print("[CrestIsle] zoom: [ ] or - = or mouse wheel (clamped)")
        self.sfx = _make_sfx()
        self.sea_loop = 0
        sea = self.sfx.get("sea")
        if sea:
            try:
                self.sea_loop = kagra.play_loop(
                    sea, SEA_LOOP_XZ[0], WATER_Y, SEA_LOOP_XZ[1],
                    volume=0.48, ref_distance=14.0, max_distance=80.0,
                )
            except Exception:
                self.sea_loop = 0
        self.mode = "play" if SMOKE else "title"
        self.t = 0.0
        self.hi = int((kagra.load_json("crest_isle") or {}).get("hi") or 0)
        self.title = kagra.Label("Crest Isle", 18, 14, 26, (255, 230, 150))
        self.hud = kagra.Label("", 18, 48, 18, (200, 220, 235))
        self._reset_round()

    def _fill_chunk(self, ix, iz):
        if SMOKE:
            return
        for rel, x, z, scale, yaw, hit in chunk_decor(ix, iz, tile=TILE):
            gy = self.world.ground_y(x, z)
            if gy < WATER_Y - 0.04:
                continue
            _place_gltf(rel, x, z, scale, yaw, self.world, collision=hit)
            self._chunk_props += 1

    def _star_y(self, star, i: int) -> float:
        model = STAR_MODELS[i]
        sc = STAR_SCALES[i]
        gy = self.world.ground_y(star.x, star.z)
        return sit_y(gy, GLTF_HALF_Y[model], sc) + 0.12 * math.sin(star.phase)

    def _coin_y(self, coin) -> float:
        gy = self.world.ground_y(coin.x, coin.z)
        return sit_y(gy, SPHERE_HALF_Y, COIN_SCALE) + COIN_HOVER + 0.14 * math.sin(coin.phase)

    def _reset_round(self):
        self.stars = spawn_stars()
        self.coins = spawn_coins()
        self.star_got = 0
        self.coin_got = 0
        self.time_s = 0.0
        self.score = 0
        self.msg = ""
        self.msg_t = 0.0
        self.grade = "D"
        for prop, star, i in zip(self.star_props, self.stars, range(len(self.stars))):
            prop.enabled = True
            prop.set_position(star.x, self._star_y(star, i), star.z)
        for prop, coin in zip(self.coin_props, self.coins):
            prop.enabled = True
            prop.set_position(coin.x, self._coin_y(coin), coin.z)
        p = self.world.player
        if p is not None:
            gy = self.world.ground_y(*START_XZ)
            p.x, p.z = START_XZ[0], START_XZ[1]
            p.y = gy
            p.vx = p.vy = p.vz = 0.0
        self.walk.face = start_face()
        self.walk.yaw = hero_theta(self.walk.face)
        self.sparks = SparkBurst()

    def update(self, dt):
        dt = min(dt, 0.05)
        self.t += dt
        if self.msg_t > 0:
            self.msg_t -= dt
        if kagra.pressed("ESCAPE"):
            if self.sea_loop:
                try:
                    kagra.stop_loop(self.sea_loop)
                except Exception:
                    self.sea_loop = 0
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
            self._sync_listener()
            return

        if self.mode == "result":
            if kagra.pressed("SPACE") or kagra.pressed("R") or kagra.pressed("RETURN"):
                _se(self.sfx, "start")
                self.mode = "play"
                self._reset_round()
            self._pose(dt, move=False)
            self._sync_listener()
            return

        self.time_s += dt
        kagra.Prop.update_all(dt)
        self.walk.step(dt)
        self._zoom_input()
        self.sparks.update(dt)

        p = self.world.player
        if p is not None:
            for i, (star, prop) in enumerate(zip(self.stars, self.star_props)):
                if not star.live:
                    continue
                star.phase += dt * 2.6
                prop.y = self._star_y(star, i)
                if kagra.can_pick(p.x, p.z, star.x, star.z, reach=PICK_REACH + 0.25):
                    star.live = False
                    prop.enabled = False
                    self.star_got += 1
                    peak = (star.x, star.z) == PEAK_XZ
                    self.msg = "PEAK FLAG!" if peak else f"Crest {self.star_got}/{STAR_NEED}"
                    self.msg_t = 1.1
                    self.avatar.feel("joy", min(1.0, 0.4 + self.star_got * 0.1))
                    self.action.play("banzai" if peak else "clap")
                    _se(self.sfx, "star", pos=(star.x, self._star_y(star, i), star.z))
                    self.sparks.burst(star.x, self._star_y(star, i) + 0.4, star.z, count=16)
            for coin, prop in zip(self.coins, self.coin_props):
                if not coin.live:
                    continue
                coin.phase += dt * 4.0
                prop.y = self._coin_y(coin)
                prop.yaw = getattr(prop, "yaw", 0.0) + dt * 2.4
                if kagra.can_pick(p.x, p.z, coin.x, coin.z, reach=PICK_REACH):
                    coin.live = False
                    prop.enabled = False
                    self.coin_got += 1
                    _se(self.sfx, "coin", volume=0.7, pos=(coin.x, self._coin_y(coin), coin.z))
                    self.sparks.burst(coin.x, self._coin_y(coin) + 0.2, coin.z, count=10, size=0.16)

        if won(self.star_got):
            self.score = round_score(self.star_got, self.coin_got, self.time_s)
            self.grade = grade_for(self.score)
            self.hi = max(self.hi, self.score)
            kagra.save_json("crest_isle", {"hi": self.hi, "grade": self.grade})
            self.mode = "result"
            self.avatar.feel("joy", 1.0)
            self.action.play("banzai")
            _se(self.sfx, "win")

        self._pose(dt, move=True)
        self._sync_listener()

    def _sync_listener(self):
        cam = self.cam
        px, py, pz = cam.position
        tx, ty, tz = cam.target
        try:
            kagra.set_listener(px, py, pz, tx - px, ty - py, tz - pz, *cam.up)
        except Exception:
            return

    def _pose(self, dt, *, move: bool):
        p = self.world.player
        speed = 0.0
        if move and p is not None:
            speed = math.hypot(p.vx, p.vz)
        self.avatar.set_locomotion(
            speed,
            walk_speed=2.2,
            run_speed=float(getattr(self.walk, "speed", PLAYER_SPEED)),
        )
        self.avatar.update(dt)
        self.action.update(dt)
        if p is None:
            return
        # Body uses walk.face — never walk.yaw (S would not turn around).
        self.avatar.set_position(p.x, p.y, p.z)
        self.avatar.set_yaw(self.walk.face)

    def _zoom_input(self):
        """[ ] or - = hold. Wheel is Walk.zoom_chase. Clamp stays on the arm."""
        closer = False
        farther = False
        for name in ("[", "BracketLeft", "-", "Minus"):
            try:
                if kagra.key(name):
                    closer = True
                    break
            except Exception:
                continue
        for name in ("]", "BracketRight", "=", "Equal"):
            try:
                if kagra.key(name):
                    farther = True
                    break
            except Exception:
                continue
        if closer:
            self.walk.zoom_chase(-CAM_ZOOM_STEP)
        if farther:
            self.walk.zoom_chase(CAM_ZOOM_STEP)

    def _draw_blob(self):
        """Cheap ellipse under the feet. Ground-projected; shrinks in air."""
        p = self.world.player
        if p is None:
            return
        gy = self.world.ground_y(p.x, p.z)
        air = max(0.0, float(p.y) - gy)
        half = 0.62 * (1.0 / (1.0 + air * 0.55))
        verts, idx = kagra.quad_y_mesh(p.x, gy + 0.03, p.z, half)
        kagra.draw_mesh_3d(self.tex_blob, verts, idx, skip_fog=True)

    def draw(self):
        if self.mode == "title":
            # Live island stays off the title. Half-streamed / UV-split terrain
            # used to show through alpha-118 fill (Emma's pre-SPACE shot).
            kagra.cls(6, 12, 20)
            self._banner(
                "Crest Isle",
                "草原・海・山を走れ  紋章を 6 つ集めろ  SPACE でスタート",
                overlay_alpha=255,
            )
            return

        kagra.cls(150, 175, 195)
        if self.sky_stage is not None:
            self.sky_stage.draw()
        else:
            kagra.sky(radius=140.0, look=False)
        self.world.draw()
        kagra.water(WATER_Y, half=HALF, world=self.world)
        kagra.Prop.draw_all()
        self._draw_blob()

        glow_items = []
        for i, star in enumerate(self.stars):
            if not star.live:
                continue
            y = self._star_y(star, i)
            size = 0.85 + 0.12 * math.sin(star.phase * 1.6)
            glow_items.append((star.x, y + 0.35, star.z, size))
        for coin in self.coins:
            if not coin.live:
                continue
            y = self._coin_y(coin)
            glow_items.append((coin.x, y + 0.12, coin.z, COIN_GLOW))
        if glow_items:
            kagra.draw_billboard_instances(self.tex_glow, glow_items, self.cam)
        spark_items = self.sparks.items()
        if spark_items:
            kagra.draw_billboard_instances(self.tex_spark, spark_items, self.cam)

        kagra.draw_vrm(self.avatar.vrm_id)
        kagra.draw_vignette()

        if self.mode == "result":
            self._banner(
                "CLEAR",
                f"Score {self.score}  Grade {self.grade}  Best {self.hi}  SPACE でもう一回",
            )
            return

        kagra.fill(0, 0, 560, 118, (8, 18, 28), 160)
        self.title.draw()
        hint = nearest_live(
            self.world.player.x if self.world.player else START_XZ[0],
            self.world.player.z if self.world.player else START_XZ[1],
            self.stars,
        )
        tip = ""
        if hint is not None and self.world.player is not None:
            tip = f"  crest {math.hypot(self.world.player.x - hint.x, self.world.player.z - hint.z):.0f}m"
        self.hud.text = (
            f"CRESTS  {self.star_got}/{STAR_NEED}   "
            f"COINS  {self.coin_got}/{len(COIN_XZ)}   "
            f"{self.time_s:4.0f}s{tip}"
        )
        self.hud.draw()
        kagra.text("[ ] / - = / wheel  zoom", 18, 78, 14, (170, 190, 205))
        if self.msg_t > 0 and self.msg:
            w, _ = kagra.measure(self.msg, 30)
            kagra.text(self.msg, (SW - w) // 2, 120, 30, (255, 240, 160))

    def _banner(self, title: str, sub: str, *, overlay_alpha: int = 118):
        kagra.fill(0, 0, SW, SH, (6, 12, 20), overlay_alpha)
        w, _ = kagra.measure(title, 48)
        kagra.text(title, (SW - w) // 2, SH // 2 - 70, 48, (255, 220, 130))
        w2, _ = kagra.measure(sub, 18)
        pulse = 170 + int(50 * math.sin(self.t * 3.5))
        kagra.text(sub, (SW - w2) // 2, SH // 2 + 10, 18, (pulse, pulse, 255))
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
        title="VRM Crest Isle",
        fps=60,
        visible=not SMOKE,
    )
    kagra.run(
        start_scene=CrestIsle(),
        max_frames=SMOKE_FRAMES + 2 if SMOKE else None,
        fixed_dt=1.0 / 60.0 if SMOKE else None,
    )
