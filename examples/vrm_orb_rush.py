"""
VRM Orb Rush — Emma で星を集めて爆弾を避ける 45 秒勝負

エージェント向け参照実装（公開 API のみ。音・粒子・難易度カーブ・タイトル演出つき）。

操作:
  WASD / 矢印 : 移動
  SPACE       : ジャンプ喜び（プレイ中）/ スタート（タイトル）
  R           : リトライ（結果画面）
  ESC         : 終了

必要なアセット: assets/Emma.vrm
任意: assets/walk.fbx（なければ synthetic_walk.bvh）
"""
from __future__ import annotations

import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import kagra
from kagra.camera3d import Camera3D
from kagra.vrm_action import ActionController

SW, SH = 1280, 720
VRM_PATH = os.environ.get("KAGRA_VRM") or "assets/Emma.vrm"
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "24"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/orb_rush_smoke.png")
FONT = None  # kagra.font() がシステムフォントを選ぶ

ARENA_R = 5.0
PLAYER_SPEED = 3.4
ROUND_SEC = 45.0
MAX_LIVES = 3


# ── 手続きテクスチャ / SE（公開 API） ─────────────────────────

def _floor_tex():
    def px(x, y):
        c = (x // 8 + y // 8) % 2
        if c:
            return (72, 88, 110, 255)
        return (58, 72, 92, 255)
    return kagra.texture_from_fn(64, 64, px, name="orb_floor")


def _star_tex():
    def px(x, y):
        d = math.hypot(x - 15.5, y - 15.5) / 15.5
        return (255, 230, 90, max(0, int((1.0 - d) * 255)))
    return kagra.texture_from_fn(32, 32, px, name="orb_star")


def _bomb_tex():
    def px(x, y):
        d = math.hypot(x - 15.5, y - 15.5) / 15.5
        return (255, 70, 90, max(0, int((1.0 - d) * 255)))
    return kagra.texture_from_fn(32, 32, px, name="orb_bomb")


def _ring_tex():
    def px(x, y):
        d = math.hypot(x - 31.5, y - 31.5) / 31.5
        band = abs(d - 0.92)
        return (120, 200, 255, max(0, int((1.0 - band * 18) * 180)))
    return kagra.texture_from_fn(64, 64, px, name="orb_ring")


def _make_sfx() -> dict[str, str]:
    return {
        "collect": kagra.tone("collect", (880, 1320), 0.10, 0.32),
        "combo": kagra.tone("combo", (990, 1485, 1980), 0.14, 0.28),
        "hit": kagra.tone("hit", (120, 90), 0.22, 0.45),
        "start": kagra.tone("start", (523, 659, 784), 0.28, 0.30),
        "go": kagra.tone("go", (784, 988), 0.18, 0.34),
        "win": kagra.tone("win", (523, 659, 784, 1046), 0.45, 0.28),
        "lose": kagra.tone("lose", (392, 311, 247), 0.50, 0.32),
        "tick": kagra.tone("tick", (660,), 0.08, 0.22),
    }


def _se(sfx: dict, key: str, volume=1.0):
    path = sfx.get(key)
    if not path:
        return
    try:
        kagra.play_se(path, volume=volume)
    except Exception:
        pass  # ヘッドレス / 無音環境でもゲームは継続


def _lerp(a, b, t):
    t = max(0.0, min(1.0, t))
    return a + (b - a) * t


# ── パーティクル / 浮き文字（2D、ワールド投影） ───────────────

class Spark:
    __slots__ = ("x", "y", "vx", "vy", "life", "max_life", "r", "g", "b", "size")

    def __init__(self, x, y, vx, vy, life, r, g, b, size):
        self.x, self.y = x, y
        self.vx, self.vy = vx, vy
        self.life = 0.0
        self.max_life = life
        self.r, self.g, self.b = r, g, b
        self.size = size


class FloatText:
    __slots__ = ("x", "y", "text", "life", "max_life", "color", "size")

    def __init__(self, x, y, text, color, size=28, life=0.9):
        self.x, self.y = x, y
        self.text = text
        self.life = 0.0
        self.max_life = life
        self.color = color
        self.size = size


class FX:
    def __init__(self):
        self.sparks: list[Spark] = []
        self.texts: list[FloatText] = []

    def burst(self, sx, sy, color, count=12, speed=180.0):
        r, g, b = color
        for i in range(count):
            ang = (i / count) * math.tau + random.uniform(-0.2, 0.2)
            spd = speed * random.uniform(0.45, 1.2)
            self.sparks.append(
                Spark(
                    sx, sy,
                    math.cos(ang) * spd, math.sin(ang) * spd - 40,
                    random.uniform(0.25, 0.55),
                    r, g, b,
                    random.uniform(3, 7),
                )
            )

    def float_text(self, sx, sy, text, color, size=28):
        self.texts.append(FloatText(sx, sy, text, color, size=size))

    def update(self, dt):
        for p in self.sparks:
            p.life += dt
            p.x += p.vx * dt
            p.y += p.vy * dt
            p.vy += 420 * dt
        for t in self.texts:
            t.life += dt
            t.y -= 55 * dt
        self.sparks = [p for p in self.sparks if p.life < p.max_life]
        self.texts = [t for t in self.texts if t.life < t.max_life]

    def draw(self):
        for p in self.sparks:
            age = p.life / p.max_life
            s = max(1, int(p.size * (1.0 - age * 0.6)))
            a = int(255 * (1.0 - age))
            kagra.rect(p.x - s // 2, p.y - s // 2, s, s, (p.r, p.g, p.b, a))
        for t in self.texts:
            age = t.life / t.max_life
            a = int(255 * (1.0 - age))
            col = (*t.color[:3], a) if len(t.color) == 3 else (*t.color[:3], a)
            w, _ = kagra.measure(t.text, t.size)
            kagra.text(t.text, t.x - w // 2, t.y, t.size, col)

    def clear(self):
        self.sparks.clear()
        self.texts.clear()


# ── ゲームオブジェクト ────────────────────────────────────────

class Orb:
    __slots__ = ("x", "z", "kind", "phase", "alive")

    def __init__(self, kind: str, avoid=None):
        for _ in range(12):
            ang = random.random() * math.tau
            r = random.uniform(1.2, ARENA_R - 0.6)
            x = math.cos(ang) * r
            z = math.sin(ang) * r
            if avoid is None or math.hypot(x - avoid[0], z - avoid[1]) > 1.4:
                break
        self.x, self.z = x, z
        self.kind = kind  # "star" | "bomb"
        self.phase = random.random() * math.tau
        self.alive = True


# ── シーン ────────────────────────────────────────────────────

class OrbRush(kagra.Scene):
    def on_enter(self):
        vrm = VRM_PATH if os.path.exists(VRM_PATH) else str(kagra.ensure_vrm())

        kagra.font()
        kagra.set_toon_params(threshold=0.48, softness=0.08, shade=0.42, lit=1.05)
        kagra.set_light_dir(0.35, 1.0, 0.55)
        kagra.set_fog(start=8.0, end=18.0, color=(28, 34, 48), enabled=True)

        self.tex_floor = _floor_tex()
        self.tex_star = _star_tex()
        self.tex_bomb = _bomb_tex()
        self.tex_ring = _ring_tex()
        self.sfx = _make_sfx()
        self.fx = FX()

        self.avatar = kagra.avatar(vrm)
        for walk_path in ("assets/walk.fbx", "tests/fixtures/synthetic_walk.bvh"):
            if not os.path.exists(walk_path):
                continue
            try:
                self.avatar.load_motion("walk", walk_path)
                print(f"[OrbRush] walk ← {walk_path}")
                break
            except Exception as e:
                print(f"[OrbRush] walk ロード失敗 ({walk_path}): {e}")
        self.avatar.play("idle", loop=True)
        self.avatar.enable_emotion()
        self.action = ActionController(self.avatar)

        self.cam = Camera3D(SW, SH, fov_deg=38.0)
        self.cam.use_orbit(radius=7.2, theta=0.55, phi=0.42, target=(0.0, 0.85, 0.0))

        # intro → title → countdown → play → result
        self.mode = "play" if SMOKE else "intro"
        self.mode_t = 0.0
        self.t = 0.0
        saved = kagra.load_json("orb_rush") or {}
        self.hi_score = int(saved.get("hi_score") or 0)
        self.countdown_n = 3
        self.title_alpha = 0.0
        self._reset_round()

    def _reset_round(self):
        self.px = 0.0
        self.pz = 0.0
        self.facing = 0.0
        self.vx = 0.0
        self.vz = 0.0
        self.score = 0
        self.lives = MAX_LIVES
        self.time_left = ROUND_SEC
        self.orbs: list[Orb] = []
        self.spawn_cd = 0.4
        self.hit_cd = 0.0
        self.msg = ""
        self.msg_t = 0.0
        self.combo = 0
        self._walk_phase = 0.0
        self.fx.clear()
        for _ in range(5):
            self.orbs.append(Orb("star", avoid=(0.0, 0.0)))
        for _ in range(2):
            self.orbs.append(Orb("bomb", avoid=(0.0, 0.0)))

    def _progress(self) -> float:
        """0=開始 → 1=終了。序盤は緩やか、終盤で急勾配。"""
        p = 1.0 - (self.time_left / ROUND_SEC)
        return max(0.0, min(1.0, p * p))

    def _difficulty(self) -> dict:
        p = self._progress()
        return {
            "spawn_interval": _lerp(1.35, 0.42, p),
            "bomb_chance": _lerp(0.14, 0.58, p),
            "chase": _lerp(0.45, 1.75, p),
            "max_stars": int(_lerp(6, 10, p)),
            "max_bombs": int(_lerp(2, 8, p)),
            "max_orbs": int(_lerp(10, 16, p)),
            "player_speed": _lerp(PLAYER_SPEED, PLAYER_SPEED * 1.12, p),
        }

    def _flash(self, text: str):
        self.msg = text
        self.msg_t = 1.2

    def _fx_at(self, wx, wy, wz, color, text=None, count=12):
        scr = self.cam.world_to_screen(wx, wy, wz)
        if not scr:
            return
        sx, sy = scr
        self.fx.burst(sx, sy, color, count=count)
        if text:
            self.fx.float_text(sx, sy - 20, text, color)

    def _spawn(self):
        d = self._difficulty()
        stars = sum(1 for o in self.orbs if o.alive and o.kind == "star")
        bombs = sum(1 for o in self.orbs if o.alive and o.kind == "bomb")
        kind = "bomb" if random.random() < d["bomb_chance"] else "star"
        if kind == "star" and stars >= d["max_stars"]:
            kind = "bomb" if bombs < d["max_bombs"] else "star"
        if kind == "bomb" and bombs >= d["max_bombs"]:
            kind = "star"
        self.orbs.append(Orb(kind, avoid=(self.px, self.pz)))
        self.orbs = [o for o in self.orbs if o.alive][-d["max_orbs"] :]

    def _begin_countdown(self):
        self.mode = "countdown"
        self.mode_t = 0.0
        self.countdown_n = 3
        self._reset_round()
        _se(self.sfx, "tick")

    def update(self, dt):
        dt = min(dt, 0.05)
        self.t += dt
        self.mode_t += dt
        if self.msg_t > 0:
            self.msg_t -= dt
        self.fx.update(dt)

        if SMOKE:
            n = kagra.tick_count()
            if n == 10:
                kagra.screenshot(SMOKE_SHOT)
            if n >= SMOKE_FRAMES:
                kagra.quit()
                return
        if kagra.pressed("ESCAPE"):
            raise SystemExit

        if self.mode == "intro":
            # カメラゆっくり周回 + タイトルフェードイン
            self.cam.orbit_th = 0.4 + self.mode_t * 0.35
            self.title_alpha = min(1.0, self.mode_t / 1.4)
            self.avatar.update(dt)
            self.action.update(dt)
            self._apply_pose()
            self._update_cam(dt, follow=False)
            if self.mode_t >= 2.2 or kagra.pressed("SPACE") or kagra.pressed("RETURN"):
                self.mode = "title"
                self.mode_t = 0.0
                self.title_alpha = 1.0
            return

        if self.mode == "title":
            self.cam.orbit_th = 0.55 + math.sin(self.t * 0.4) * 0.15
            self.avatar.update(dt)
            self.action.update(dt)
            self._apply_pose()
            self._update_cam(dt, follow=False)
            if kagra.pressed("SPACE") or kagra.pressed("RETURN"):
                _se(self.sfx, "start")
                self._begin_countdown()
            return

        if self.mode == "countdown":
            self.avatar.update(dt)
            self.action.update(dt)
            self._apply_pose()
            self._update_cam(dt)
            # 1 秒ごとに 3 → 2 → 1 → GO
            step = int(self.mode_t)
            shown = 3 - step
            if shown != self.countdown_n and shown >= 0:
                self.countdown_n = shown
                _se(self.sfx, "tick" if shown > 0 else "go")
            if self.mode_t >= 3.2:
                self.mode = "play"
                self.mode_t = 0.0
                self._flash("スタート！")
                self.avatar.feel("joy", intensity=0.8)
                _se(self.sfx, "go")
            return

        if self.mode == "result":
            if kagra.pressed("R") or kagra.pressed("SPACE"):
                _se(self.sfx, "start")
                self._begin_countdown()
            self.avatar.update(dt)
            self.action.update(dt)
            self._apply_pose()
            self._update_cam(dt)
            return

        # ── play ──
        d = self._difficulty()
        self.time_left -= dt
        if self.hit_cd > 0:
            self.hit_cd -= dt

        ix = (1 if kagra.key("D") or kagra.key("RIGHT") else 0) - (
            1 if kagra.key("A") or kagra.key("LEFT") else 0
        )
        iz = (1 if kagra.key("W") or kagra.key("UP") else 0) - (
            1 if kagra.key("S") or kagra.key("DOWN") else 0
        )
        moving = ix != 0 or iz != 0
        th = self.cam.orbit_th
        fx, fz = -math.sin(th), -math.cos(th)
        rx, rz = math.cos(th), -math.sin(th)
        mx = rx * ix + fx * iz
        mz = rz * ix + fz * iz
        speed_cap = d["player_speed"]
        if moving:
            length = math.hypot(mx, mz) or 1.0
            mx /= length
            mz /= length
            self.facing = math.atan2(mx, mz)
            self.vx += (mx * speed_cap - self.vx) * 10 * dt
            self.vz += (mz * speed_cap - self.vz) * 10 * dt
        else:
            self.vx *= max(0.0, 1.0 - 12 * dt)
            self.vz *= max(0.0, 1.0 - 12 * dt)

        self.px += self.vx * dt
        self.pz += self.vz * dt
        dist = math.hypot(self.px, self.pz)
        if dist > ARENA_R - 0.35:
            s = (ARENA_R - 0.35) / dist
            self.px *= s
            self.pz *= s
            self.vx *= 0.4
            self.vz *= 0.4

        speed = math.hypot(self.vx, self.vz)
        want = "walk" if moving or speed > 0.35 else "idle"
        if self.avatar.clip != want:
            self.avatar.play(want, loop=True)
        if speed > 0.2:
            self._walk_phase += speed * 3.2 * dt
        else:
            self._walk_phase *= 0.9

        if kagra.pressed("SPACE") and self.hit_cd <= 0:
            self.action.play("jump_joy")
            self.avatar.feel("happy", intensity=0.7)

        self.spawn_cd -= dt
        if self.spawn_cd <= 0:
            self._spawn()
            self.spawn_cd = d["spawn_interval"]

        for orb in self.orbs:
            if not orb.alive:
                continue
            orb.phase += dt * (3.2 if orb.kind == "star" else 2.0)
            if orb.kind == "bomb":
                dx, dz = self.px - orb.x, self.pz - orb.z
                dd = math.hypot(dx, dz) or 1.0
                orb.x += dx / dd * d["chase"] * dt
                orb.z += dz / dd * d["chase"] * dt

            if math.hypot(orb.x - self.px, orb.z - self.pz) < 0.55:
                orb.alive = False
                bob_y = 0.55 + math.sin(orb.phase) * 0.12
                if orb.kind == "star":
                    self.combo += 1
                    gain = 10 + min(40, self.combo * 2)
                    self.score += gain
                    self._flash(f"+{gain}  combo {self.combo}")
                    self.action.play("clap" if self.combo % 3 == 0 else "nod")
                    self.avatar.feel("joy", intensity=min(1.0, 0.5 + self.combo * 0.08))
                    self._fx_at(
                        orb.x, bob_y, orb.z, (255, 230, 90),
                        text=f"+{gain}", count=14 if self.combo < 3 else 22,
                    )
                    _se(self.sfx, "combo" if self.combo >= 3 else "collect")
                elif self.hit_cd <= 0:
                    self.combo = 0
                    self.lives -= 1
                    self.hit_cd = 1.1
                    self._flash("ouch...")
                    self.action.play("shake_head")
                    self.avatar.feel("angry", intensity=0.9)
                    self.vx *= -0.5
                    self.vz *= -0.5
                    self._fx_at(orb.x, bob_y, orb.z, (255, 80, 100), text="HIT", count=18)
                    _se(self.sfx, "hit", volume=0.9)

        self.orbs = [o for o in self.orbs if o.alive]

        if self.lives <= 0 or self.time_left <= 0:
            self.mode = "result"
            self.mode_t = 0.0
            self.hi_score = max(self.hi_score, self.score)
            kagra.save_json("orb_rush", {"hi_score": self.hi_score})
            if self.lives <= 0:
                self.avatar.feel("angry", intensity=1.0)
                self.action.play("bow")
                self._flash("負け…")
                _se(self.sfx, "lose")
            else:
                self.avatar.feel("joy", intensity=1.0)
                self.action.play("banzai")
                self._flash("タイムアップ！")
                _se(self.sfx, "win")

        self.avatar.update(dt)
        self.action.update(dt)
        self._apply_pose()
        self._update_cam(dt)

    def _apply_pose(self):
        speed = math.hypot(self.vx, self.vz)
        bob = abs(math.sin(self._walk_phase)) * 0.025 if speed > 0.2 else 0.0
        self.avatar.set_position(self.px, bob, self.pz)
        self.avatar.set_yaw(self.facing)

    def _update_cam(self, dt, follow=True):
        if follow:
            desired_theta = self.facing + math.pi
            cur = self.cam.orbit_th
            diff = (desired_theta - cur + math.pi) % math.tau - math.pi
            self.cam.orbit_th = cur + diff * min(1.0, 3.5 * dt)
            self.cam.orbit_tgt = (self.px, 0.9, self.pz)
        else:
            self.cam.orbit_tgt = (0.0, 0.85, 0.0)
        self.cam.orbit_r = 6.8
        self.cam.orbit_phi = 0.48
        eng = kagra.get_engine()
        if eng:
            self.cam.update(eng)

    def draw(self):
        if self.mode == "play" and self.hit_cd > 0.6:
            kagra.cls(90, 40, 50)
        else:
            kagra.cls(28, 34, 48)

        fv, fi = kagra.disk_mesh(0.0, 0.0, 0.0, ARENA_R, 56)
        kagra.draw_mesh_3d(self.tex_floor, fv, fi)
        rv, ri = kagra.quad_y_mesh(0.0, 0.02, 0.0, ARENA_R * 1.02)
        kagra.draw_mesh_3d(self.tex_ring, rv, ri)

        for orb in self.orbs:
            if not orb.alive:
                continue
            bob = 0.55 + math.sin(orb.phase) * 0.12
            size = 0.28 if orb.kind == "star" else 0.32
            # 爆弾は点滅警告
            if orb.kind == "bomb" and (int(orb.phase * 6) % 2 == 0):
                size *= 1.08
            verts, idx = kagra.billboard_mesh(orb.x, bob, orb.z, size, self.cam)
            tex = self.tex_star if orb.kind == "star" else self.tex_bomb
            kagra.draw_mesh_3d(tex, verts, idx)

        kagra.draw_vrm(self.avatar.vrm_id)

        if self.mode in ("intro", "title"):
            self._draw_title()
        elif self.mode == "countdown":
            self._draw_hud()
            self._draw_countdown()
        elif self.mode == "result":
            self._draw_hud()
            self._draw_result()
        else:
            self._draw_hud()
            # 難易度メーター（参照実装として可視化）
            self._draw_heat()

        self.fx.draw()

        if self.msg_t > 0 and self.msg and self.mode == "play":
            w, _ = kagra.measure(self.msg, 36)
            kagra.text(self.msg, (SW - w) // 2, 120, 36, (255, 240, 160))

    def _draw_title(self):
        a = int(140 * self.title_alpha)
        kagra.fill(0, 0, SW, SH, (10, 12, 20), a)
        title = "VRM Orb Rush"
        w, _ = kagra.measure(title, 64)
        # スライドイン
        yoff = int((1.0 - self.title_alpha) * 40)
        kagra.text(title, (SW - w) // 2, SH // 2 - 110 + yoff, 64, (255, 220, 120))
        if self.title_alpha > 0.55:
            sub = "星を集めて、赤い爆弾から逃げろ！"
            w2, _ = kagra.measure(sub, 26)
            kagra.text(sub, (SW - w2) // 2, SH // 2 - 30, 26, (210, 220, 240))
        if self.mode == "title":
            hint = "SPACE / ENTER でスタート"
            w3, _ = kagra.measure(hint, 28)
            pulse = 180 + int(50 * math.sin(self.t * 4))
            kagra.text(hint, (SW - w3) // 2, SH // 2 + 40, 28, (pulse, pulse, 255))
            kagra.text("WASD:移動   ESC:終了", 40, SH - 50, 20, (160, 170, 190))
            if self.hi_score > 0:
                hs = f"Best  {self.hi_score}"
                wh, _ = kagra.measure(hs, 22)
                kagra.text(hs, (SW - wh) // 2, SH // 2 + 90, 22, (255, 200, 140))

    def _draw_countdown(self):
        kagra.fill(0, 0, SW, SH, (0, 0, 0), 100)
        if self.countdown_n > 0:
            label = str(self.countdown_n)
            size = 120
        else:
            label = "GO!"
            size = 96
        w, _ = kagra.measure(label, size)
        # ポップスケール
        frac = self.mode_t - int(self.mode_t)
        pop = 1.0 + (1.0 - frac) * 0.15
        size_i = int(size * pop)
        w, _ = kagra.measure(label, size_i)
        kagra.text(label, (SW - w) // 2, SH // 2 - size_i // 2, size_i, (255, 230, 140))

    def _draw_heat(self):
        p = self._progress()
        bar_w = 160
        kagra.fill(SW - 200, 24, bar_w + 8, 18, (20, 24, 36), 180)
        kagra.fill(SW - 196, 28, int(bar_w * p), 10, (255, int(80 + 140 * (1 - p)), 70), 220)
        kagra.text("HEAT", SW - 200, 46, 16, (200, 160, 140))

    def _draw_hud(self):
        kagra.fill(0, 0, 340, 150, (15, 18, 28), 170)
        kagra.text(f"SCORE  {self.score}", 24, 20, 32, (255, 230, 120))
        kagra.text(f"TIME   {max(0.0, self.time_left):4.1f}", 24, 60, 26, (180, 220, 255))
        hearts = "* " * max(0, self.lives) + ". " * max(0, MAX_LIVES - self.lives)
        kagra.text(f"LIFE   {hearts.strip()}", 24, 98, 24, (255, 110, 140))
        if self.combo >= 2:
            kagra.text(f"COMBO x{self.combo}", 200, 98, 22, (120, 255, 180))

    def _draw_result(self):
        fade = min(1.0, self.mode_t / 0.45)
        kagra.fill(0, 0, SW, SH, (0, 0, 0), int(150 * fade))
        title = "RESULT"
        w, _ = kagra.measure(title, 56)
        kagra.text(title, (SW - w) // 2, SH // 2 - 100, 56, (255, 210, 100))
        line = f"Score  {self.score}    Best  {self.hi_score}"
        w2, _ = kagra.measure(line, 32)
        kagra.text(line, (SW - w2) // 2, SH // 2 - 20, 32, (240, 240, 255))
        grade = "S" if self.score >= 400 else "A" if self.score >= 250 else "B" if self.score >= 120 else "C"
        wg, _ = kagra.measure(grade, 72)
        kagra.text(grade, (SW - wg) // 2, SH // 2 + 20, 72, (255, 220, 160))
        hint = "R / SPACE でもう一回"
        w3, _ = kagra.measure(hint, 26)
        pulse = 160 + int(60 * math.sin(self.t * 3.5))
        kagra.text(hint, (SW - w3) // 2, SH // 2 + 110, 26, (pulse, pulse, 255))


if __name__ == "__main__":
    os.makedirs(os.path.dirname(SMOKE_SHOT) or ".", exist_ok=True)
    kagra.init(width=SW, height=SH, title="VRM Orb Rush", fps=60, visible=not SMOKE)
    kagra.run(
        start_scene=OrbRush(),
        max_frames=SMOKE_FRAMES + 2 if SMOKE else None,
        fixed_dt=1.0 / 60.0 if SMOKE else None,
    )
