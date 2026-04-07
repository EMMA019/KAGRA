"""
vrm_stage.py - VRM ライブステージデモ（VrmAvatar 版）
============================================================
kagra.avatar() を使った簡潔な実装。

操作:
  Z         : 投げキッス
  X         : 両手キッス
  C         : ウェーブ
  SPACE     : お辞儀
  ENTER     : バインドポーズに戻す
  ↑         : 歩く
  →         : 走る
  ←         : 忍び足
  マウスドラッグ : カメラ回転
  ホイール      : ズーム
  ESC       : 終了
"""
import math
import random
import os
import struct
import zlib
import tempfile

import kagra
from kagra.camera3d import Camera3D

SW, SH   = 1280, 720
VRM_PATH = "assets/Emma.vrm"


# ── ステージ生成（変更なし） ──────────────────────────────────

def make_png(w, h, pixels_fn):
    rows = b""
    for y in range(h):
        row = b"\x00"
        for x in range(w):
            row += bytes(pixels_fn(x, y))
        rows += row
    raw = zlib.compress(rows)
    def chunk(t, d):
        c = zlib.crc32(t+d) & 0xFFFFFFFF
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", c)
    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
           + chunk(b"IDAT", raw) + chunk(b"IEND", b""))
    p = os.path.join(tempfile.gettempdir(), f"kagra_stage_{w}_{h}.png")
    open(p, "wb").write(png)
    return kagra.load_texture(p)


def make_stage_floor_tex():
    def px(x, y):
        stripe = (x//16 + y//64) % 2
        base = (160, 100, 50, 255) if stripe == 0 else (140, 85, 40, 255)
        noise = ((x*7 + y*13) % 20) - 10
        return (min(255, base[0]+noise), min(255, base[1]+noise//2), base[2], 255)
    return make_png(128, 128, px)


def make_spotlight_tex():
    def px(x, y):
        d = math.sqrt((x-32)**2+(y-32)**2) / 32.0
        a = max(0, int((1.0-d)*200))
        return (255, 240, 180, a)
    return make_png(64, 64, px)


def make_stage_meshes(floor_tex, spot_tex):
    meshes = []
    S = 1.5; segs = 32
    # 床
    floor_v, floor_i = [], []
    for i in range(segs):
        a0 = math.radians(i*360/segs); a1 = math.radians((i+1)*360/segs)
        x0,z0 = math.cos(a0)*S, math.sin(a0)*S
        x1,z1 = math.cos(a1)*S, math.sin(a1)*S
        base = len(floor_v)
        floor_v += [[0,0,0,0,1,0,.5,.5],
                    [x0,0,z0,0,1,0,.5+math.cos(a0)*.5,.5+math.sin(a0)*.5],
                    [x1,0,z1,0,1,0,.5+math.cos(a1)*.5,.5+math.sin(a1)*.5]]
        floor_i += [base,base+1,base+2]
    meshes.append((floor_tex, floor_v, floor_i))
    # スポットライト
    SR = 0.6; spot_v, spot_i = [], []
    for i in range(segs):
        a0 = math.radians(i*360/segs); a1 = math.radians((i+1)*360/segs)
        x0,z0 = math.cos(a0)*SR, math.sin(a0)*SR
        x1,z1 = math.cos(a1)*SR, math.sin(a1)*SR
        base = len(spot_v)
        spot_v += [[0,.01,0,0,1,0,.5,.5],
                   [x0,.01,z0,0,1,0,.5+math.cos(a0)*.5,.5+math.sin(a0)*.5],
                   [x1,.01,z1,0,1,0,.5+math.cos(a1)*.5,.5+math.sin(a1)*.5]]
        spot_i += [base,base+1,base+2]
    meshes.append((spot_tex, spot_v, spot_i))
    return meshes


# ── メインシーン ──────────────────────────────────────────────

class StageScene(kagra.Scene):

    def on_enter(self):
        self.font = kagra.assets.font("meiryo")
        self.time = 0.0

        # カメラ
        self.cam = Camera3D(SW, SH, fov_deg=28.0)
        self.cam.use_orbit(radius=3.2, theta=0.0, phi=0.12,
                           target=(0.0, 0.85, 0.0))

        # ★ VrmAvatar で VRM をロード（アニメ・スプリング・表情を統合管理）
        print("Loading VRM...")
        self.av = kagra.avatar(VRM_PATH)
        print(f"  clips:       {self.av.clips}")
        print(f"  expressions: {self.av.expressions}")

        # BVH ダンスモーション（あれば読み込む）
        import os
        bvh_path = "assets/dataset-1_dance-long_normal_001.bvh"
        if os.path.exists(bvh_path):
            self.av.load_motion("dance", bvh_path)
            print("BVH dance loaded!")

        # ステージ
        self.floor_tex    = make_stage_floor_tex()
        self.spot_tex     = make_spotlight_tex()
        self.stage_meshes = make_stage_meshes(self.floor_tex, self.spot_tex)

        # カメラ操作状態
        self._drag = False
        self._last_mx = self._last_my = 0

        # 自動アニメ（操作がないと自動で流れる）
        self._auto_timer = 0.0
        self._auto_clips = ["kiss","bow","walk","run","kiss_both","arm_up","sneak","wave"]
        self._auto_idx   = 0

        # 自動表情切り替え
        self._expr_timer   = 0.0
        self._expr_current = ""

    def update(self, dt: float):
        if kagra.pressed("ESCAPE"): raise SystemExit
        self.time += dt

        # ── キー入力でクリップ再生 ──────────────────────────
        if kagra.pressed("Z"):
            self.av.play("kiss", loop=False)
        if kagra.pressed("X"):
            self.av.play("kiss_both", loop=False)
        if kagra.pressed("V"):
            self.av.play("wave", loop=False)
        if kagra.pressed("SPACE"):
            self.av.play("bow", loop=False)
        if kagra.pressed("RETURN"):
            self.av.play("bind", loop=False)
        if kagra.pressed("D"):
            self.av.play("dance", loop=True)
        if kagra.pressed("UP"):
            self.av.play("walk")
        if kagra.pressed("RIGHT"):
            self.av.play("run")
        if kagra.pressed("LEFT"):
            self.av.play("sneak")

        # ── 自動アニメ ────────────────────────────────────
        self._auto_timer += dt
        auto_wait = 4.0 if self.av.clip in ("walk","run","sneak") else 2.5
        if not self.av.playing and self._auto_timer > auto_wait:
            clip = self._auto_clips[self._auto_idx % len(self._auto_clips)]
            loop = clip in ("walk","run","sneak","idle")
            self.av.play(clip, loop=loop)
            self._auto_idx   += 1
            self._auto_timer  = 0.0

        # ── 自動表情 ──────────────────────────────────────
        if self.av.expressions:
            self._expr_timer += dt
            if self._expr_timer > 3.0:
                self._expr_timer = 0.0
                if self._expr_current:
                    self.av.set_expression(self._expr_current, 0.0)
                self._expr_current = random.choice(self.av.expressions + [""])
                if self._expr_current:
                    self.av.set_expression(self._expr_current, 0.85)

        # ── 自然な風 ──────────────────────────────────────
        ws = abs(math.sin(self.time * 0.7)) * 0.15
        wd = (math.cos(self.time * 0.4), 0.0, math.sin(self.time * 0.3))
        self.av.set_wind(ws, wd)

        # ★ アニメ更新（1行）
        self.av.update(dt)   # アニメ + SpringBone + まばたき

        # ── カメラ操作 ────────────────────────────────────
        mx, my = kagra.mouse_pos()
        if kagra.mouse_pressed(kagra.MOUSE_LEFT):
            self._drag = True; self._last_mx, self._last_my = mx, my
        if kagra.mouse_down(kagra.MOUSE_LEFT) and self._drag:
            self.cam.orbit_by((mx-self._last_mx)*.008, -(my-self._last_my)*.008)
            self._last_mx, self._last_my = mx, my
        if not kagra.mouse_down(kagra.MOUSE_LEFT):
            self._drag = False
        _, wy = kagra.mouse_wheel()
        if wy: self.cam.zoom(-wy * 0.2)
        if not self._drag:
            self.cam.orbit_by(dt * 0.08, 0)
        self.cam.update(kagra._engine)

    def draw(self):
        kagra.cls(10, 8, 20)
        for tex, verts, idx in self.stage_meshes:
            kagra.draw_mesh_3d(tex, verts, idx)

        # ★ VRM 描画（1行）
        kagra.draw_vrm(self.av.vrm_id)

        # UI
        kagra.rect(0, SH-54, SW, 54, 0, 0, 0, 140)
        clip_str = f"{'▶' if self.av.playing else '■'} {self.av.clip}"
        kagra.draw_text(self.font, clip_str, 20, SH-46, 20, color=(100,255,150))
        kagra.draw_text(self.font,
            "Z:キッス  X:両手  V:ウェーブ  SPC:お辞儀  ENT:戻す  "
            "↑:歩く  →:走る  ←:忍び足  ドラッグ:回転  ESC:終了",
            20, SH-22, 17, color=(180,180,200))
        kagra.draw_text(self.font, "KAGRA - VRM Live Stage",
                        20, 20, 28, color=(255,210,80))
        kagra.draw_text(self.font, "GPU Skinning + SpringBone + BlendShape",
                        20, 52, 18, color=(120,180,255))


if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="KAGRA VRM Live Stage", fps=60)
    kagra.run(start_scene=StageScene())
