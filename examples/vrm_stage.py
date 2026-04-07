"""
vrm_stage.py - KAGRA VRM Live Stage デモ
=========================================
操作:
  ↑        : 歩く
  →        : 走る
  ←        : 忍び足
  H        : ダンス開始（SPACE で停止）
  SPACE    : 停止してアイドルに戻る
  ドラッグ  : カメラ回転
  ホイール  : ズーム
  ESC      : 終了
"""
import math
import os
import struct
import zlib
import tempfile

import kagra
from kagra.camera3d import Camera3D

SW, SH   = 1280, 720
VRM_PATH = "assets/Emma.vrm"


# ── ステージメッシュ生成 ──────────────────────────────────────

def _make_png(w, h, px_fn):
    rows = b""
    for y in range(h):
        row = b"\x00"
        for x in range(w):
            row += bytes(px_fn(x, y))
        rows += row
    raw = zlib.compress(rows)
    def chunk(t, d):
        c = zlib.crc32(t+d) & 0xFFFFFFFF
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", c)
    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
           + chunk(b"IDAT", raw) + chunk(b"IEND", b""))
    p = os.path.join(tempfile.gettempdir(), f"stage_{w}_{h}.png")
    open(p, "wb").write(png)
    return kagra.load_texture(p)

def _build_stage():
    # 床テクスチャ
    def floor_px(x, y):
        s = (x//16 + y//64) % 2
        b = (160, 100, 50) if s == 0 else (140, 85, 40)
        n = ((x*7 + y*13) % 20) - 10
        return (min(255, b[0]+n), min(255, b[1]+n//2), b[2], 255)
    floor_tex = _make_png(128, 128, floor_px)

    # スポットライトテクスチャ
    def spot_px(x, y):
        d = math.sqrt((x-32)**2+(y-32)**2) / 32.0
        return (255, 240, 180, max(0, int((1.0-d)*200)))
    spot_tex = _make_png(64, 64, spot_px)

    S = 1.5; segs = 32
    meshes = []

    # 床
    fv, fi = [], []
    for i in range(segs):
        a0 = math.radians(i*360/segs); a1 = math.radians((i+1)*360/segs)
        x0,z0 = math.cos(a0)*S, math.sin(a0)*S
        x1,z1 = math.cos(a1)*S, math.sin(a1)*S
        b = len(fv)
        fv += [[0,0,0,0,1,0,.5,.5],
               [x0,0,z0,0,1,0,.5+math.cos(a0)*.5,.5+math.sin(a0)*.5],
               [x1,0,z1,0,1,0,.5+math.cos(a1)*.5,.5+math.sin(a1)*.5]]
        fi += [b,b+1,b+2]
    meshes.append((floor_tex, fv, fi))

    # スポットライト
    SR = 0.6; sv, si = [], []
    for i in range(segs):
        a0 = math.radians(i*360/segs); a1 = math.radians((i+1)*360/segs)
        x0,z0 = math.cos(a0)*SR, math.sin(a0)*SR
        x1,z1 = math.cos(a1)*SR, math.sin(a1)*SR
        b = len(sv)
        sv += [[0,.005,0,0,1,0,.5,.5],
               [x0,.005,z0,0,1,0,.5+math.cos(a0)*.5,.5+math.sin(a0)*.5],
               [x1,.005,z1,0,1,0,.5+math.cos(a1)*.5,.5+math.sin(a1)*.5]]
        si += [b,b+1,b+2]
    meshes.append((spot_tex, sv, si))

    return meshes


# ── メインシーン ──────────────────────────────────────────────

class StageScene(kagra.Scene):

    def on_enter(self):
        self.font = kagra.assets.font("meiryo")
        self.time = 0.0
        self._dancing = False

        # カメラ
        self.cam = Camera3D(SW, SH, fov_deg=28.0)
        self.cam.use_orbit(radius=3.2, theta=0.0, phi=0.12,
                           target=(0.0, 0.85, 0.0))
        self._drag = False
        self._last_mx = self._last_my = 0

        # VRM ロード
        print("Loading VRM...")
        self.av = kagra.avatar(VRM_PATH)

        # BVH ダンス（assets/hiphop.bvh があれば読み込む）
        bvh = "assets/hiphop.bvh"
        if os.path.exists(bvh):
            self.av.load_motion("dance", bvh)
            print(f"Dance loaded: {bvh}")
        else:
            print(f"[skip] {bvh} not found")

        # 起動時はアイドル
        self.av.play("idle")

        # ステージ
        self.stage_meshes = _build_stage()

        # VRM を地面に合わせるオフセット
        # ステージ Y=0、VRM Hips は約 0.85m → そのままで OK
        # ダンス中は Root オフセットが入るのでリセット用に保持
        self._base_offset = (0.0, 0.0, 0.0)

    def update(self, dt: float):
        if kagra.pressed("ESCAPE"): raise SystemExit
        self.time += dt

        # ── 入力 ─────────────────────────────────────────────
        if kagra.pressed("UP"):
            self._stop_dance()
            self.av.play("walk")

        if kagra.pressed("RIGHT"):
            self._stop_dance()
            self.av.play("run")

        if kagra.pressed("LEFT"):
            self._stop_dance()
            self.av.play("sneak")

        if kagra.pressed("H"):
            if "dance" in self.av.clips:
                self._dancing = True
                self.av.play("dance", loop=True)
            else:
                print("hiphop.bvh が assets/ にありません")

        if kagra.pressed("SPACE"):
            self._stop_dance()
            self.av.play("idle")

        # ダンス中でない場合、移動キーが離れたらアイドルに戻す
        if not self._dancing:
            walking = (kagra.key("UP") or kagra.key("RIGHT") or kagra.key("LEFT"))
            if not walking and self.av.clip in ("walk","run","sneak"):
                self.av.play("idle")

        # ── アニメ更新 ────────────────────────────────────────
        self.av.update(dt)

        # ★ ここを追加：ダンス中なのに再生が終了していたら再度呼び出す
        if self._dancing and not self.av.playing:
            self.av.play("dance", loop=True)
        # ── カメラ操作 ────────────────────────────────────────
        mx, my = kagra.mouse_pos()
        if kagra.mouse_pressed(kagra.MOUSE_LEFT):
            self._drag = True
            self._last_mx, self._last_my = mx, my
        if kagra.mouse_down(kagra.MOUSE_LEFT) and self._drag:
            self.cam.orbit_by((mx-self._last_mx)*.008, -(my-self._last_my)*.008)
            self._last_mx, self._last_my = mx, my
        if not kagra.mouse_down(kagra.MOUSE_LEFT):
            self._drag = False
        _, wy = kagra.mouse_wheel()
        if wy: self.cam.zoom(-wy * 0.2)
        if not self._drag:
            self.cam.orbit_by(dt * 0.06, 0)
        self.cam.update(kagra._engine)

    def _stop_dance(self):
        """ダンスを止めてオフセットをリセット。"""
        self._dancing = False
        kagra._engine.set_vrm_offset(self.av.vrm_id, 0.0, 0.0, 0.0)

    def draw(self):
        kagra.cls(10, 8, 20)

        for tex, verts, idx in self.stage_meshes:
            kagra.draw_mesh_3d(tex, verts, idx)

        kagra.draw_vrm(self.av.vrm_id)

        # UI バー
        kagra.rect(0, SH-44, SW, 44, 0, 0, 0, 150)

        icon = "▶" if self.av.playing else "■"
        clip = self.av.clip or "idle"
        status = f"{icon} {clip}"
        if self._dancing:
            status += "  ［SPACE で停止］"
        kagra.draw_text(self.font, status, 20, SH-36, 20, color=(100,255,150))

        hint = "↑:歩く  →:走る  ←:忍び足  H:ダンス  SPACE:停止  ドラッグ:回転  ESC:終了"
        kagra.draw_text(self.font, hint, 20, SH-16, 15, color=(160,160,180))

        kagra.draw_text(self.font, "KAGRA - VRM Live Stage",
                        20, 20, 28, color=(255,210,80))
        kagra.draw_text(self.font, "GPU Skinning + SpringBone + BlendShape",
                        20, 52, 18, color=(120,180,255))


if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="KAGRA VRM Live Stage", fps=60)
    kagra.run(start_scene=StageScene())
