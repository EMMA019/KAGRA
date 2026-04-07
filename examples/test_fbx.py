"""
test_fbx.py - 歩行FBXテスト（地面蹴り・左右揺れ修正・アヘ顔対策付き）
"""
import os
import math
import struct
import zlib
import tempfile

import kagra
from kagra import fbx_player
from kagra.camera3d import Camera3D

SW, SH = 1280, 720
VRM_PATH = "assets/Emma.vrm"
FBX_PATH = "assets/walking.fbx"          # 歩行FBXに変更
T_POSE_PATH = "assets/T-Pose.fbx"

# ---------- 床 ----------
def _make_floor():
    def px(x, y):
        s = (x//16 + y//64) % 2
        b = (160, 100, 50) if s == 0 else (140, 85, 40)
        n = ((x*7 + y*13) % 20) - 10
        return (min(255, b[0]+n), min(255, b[1]+n//2), b[2], 255)
    rows = b""
    for y in range(128):
        row = b"\x00"
        for x in range(128):
            row += bytes(px(x, y))
        rows += row
    raw = zlib.compress(rows)
    def chunk(t, d):
        c = zlib.crc32(t+d) & 0xFFFFFFFF
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", c)
    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", 128, 128, 8, 6, 0, 0, 0))
           + chunk(b"IDAT", raw) + chunk(b"IEND", b""))
    p = os.path.join(tempfile.gettempdir(), "floor128.png")
    open(p, "wb").write(png)
    tex = kagra.load_texture(p)

    S = 2.0; segs = 32; verts = []; idx = []
    for i in range(segs):
        a0 = math.radians(i*360/segs)
        a1 = math.radians((i+1)*360/segs)
        x0,z0 = math.cos(a0)*S, math.sin(a0)*S
        x1,z1 = math.cos(a1)*S, math.sin(a1)*S
        b = len(verts)
        verts += [[0,0,0,0,1,0,.5,.5],
                  [x0,0,z0,0,1,0,.5+math.cos(a0)*.5,.5+math.sin(a0)*.5],
                  [x1,0,z1,0,1,0,.5+math.cos(a1)*.5,.5+math.sin(a1)*.5]]
        idx += [b,b+1,b+2]
    return tex, verts, idx

# ---------- シーン ----------
class TestFbxScene(kagra.Scene):
    def on_enter(self):
        self.font = kagra.assets.font("meiryo")
        self.cam = Camera3D(SW, SH, fov_deg=28.0)
        self.cam.use_orbit(radius=3.2, theta=0.0, phi=0.12, target=(0.0, 0.85, 0.0))
        self._drag = False
        self._last_mx = self._last_my = 0
        self._status = "Loading..."

        self.av = kagra.avatar(VRM_PATH)
        self.av.play("idle")

        if os.path.exists(FBX_PATH):
            try:
                if not os.path.exists(T_POSE_PATH):
                    raise FileNotFoundError(f"T-Pose FBX not found: {T_POSE_PATH}")
                motion = fbx_player.load_fbx(FBX_PATH, bind_fbx_path=T_POSE_PATH)
                self.av.add_motion("walk", motion)
                self._status = "✅ Walk FBX loaded (baked)  H=walk"
                print(f"[TEST] Walk FBX baked: {FBX_PATH}")
            except Exception as e:
                self._status = f"❌ FBX error: {e}"
                print(f"[TEST] FBX error: {e}")
        else:
            self._status = f"❌ {FBX_PATH} not found"

        self._floor_tex, self._floor_v, self._floor_i = _make_floor()

    def _update_eyes(self):
        """頭の回転を打ち消すのをやめ、目を正面（無回転）に固定する"""
        try:
            # 複雑なクォータニオン計算を削除し、無回転 (0, 0, 0, 1) を直接セット
            kagra._engine.set_vrm_bone_rot(self.av.vrm_id, "J_Bip_L_Eye", 0.0, 0.0, 0.0, 1.0)
            kagra._engine.set_vrm_bone_rot(self.av.vrm_id, "J_Bip_R_Eye", 0.0, 0.0, 0.0, 1.0)
        except Exception as e:
            # APIが無い場合などは無視
            pass

    def update(self, dt):
        if kagra.pressed("ESCAPE"):
            raise SystemExit
        if kagra.pressed("H"):
            if "walk" in self.av.clips:
                self.av.play("walk", loop=True)
                self._status = "▶ walking  [SPACE: stop]"
            else:
                self._status = "walk clip not loaded"
        if kagra.pressed("SPACE"):
            self.av.play("idle")
            self._status = "■ idle  [H: walk]"

        self.av.update(dt)
        self._update_eyes()   # ★ アヘ顔対策

        # カメラ操作
        mx, my = kagra.mouse_pos()
        if kagra.mouse_pressed(kagra.MOUSE_LEFT):
            self._drag = True
            self._last_mx, self._last_my = mx, my
        if kagra.mouse_down(kagra.MOUSE_LEFT) and self._drag:
            self.cam.orbit_by((mx - self._last_mx) * 0.008,
                              -(my - self._last_my) * 0.008)
            self._last_mx, self._last_my = mx, my
        if not kagra.mouse_down(kagra.MOUSE_LEFT):
            self._drag = False
        _, wy = kagra.mouse_wheel()
        if wy:
            self.cam.zoom(-wy * 0.2)
        if not self._drag:
            self.cam.orbit_by(dt * 0.05, 0)
        self.cam.update(kagra._engine)

    def draw(self):
        kagra.cls(10, 8, 20)
        kagra.draw_mesh_3d(self._floor_tex, self._floor_v, self._floor_i)
        kagra.draw_vrm(self.av.vrm_id)

        kagra.rect(0, SH-44, SW, 44, 0, 0, 0, 160)
        kagra.draw_text(self.font, self._status, 20, SH-36, 20, color=(100, 255, 150))
        kagra.draw_text(self.font, "H:歩行  SPACE:停止  ドラッグ:回転  ESC:終了",
                        20, SH-16, 15, color=(160, 160, 180))
        kagra.draw_text(self.font, "KAGRA - Walk Test (Eye Control Active)",
                        20, 20, 26, color=(255, 210, 80))


if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="KAGRA Walk Test", fps=60)
    kagra.run(start_scene=TestFbxScene())