"""ポーズ回帰テスト用の子プロセスレンダラ。

同一プロセスで T ポーズと四肢ポーズの 2 枚を撮る。
マルチスキン VRM で四肢のボーン回転が描画に反映されない
リグレッション（共有スキンパレット上書きバグ）を検出する。

    python tests/render_pose_scene.py <vrm_path> <out_tpose.png> <out_posed.png>
"""
from __future__ import annotations

import math
import sys

import kagra

VRM_PATH, OUT_TPOSE, OUT_POSED = sys.argv[1], sys.argv[2], sys.argv[3]
SW, SH = 320, 240


class PoseScene(kagra.Scene):
    def on_enter(self):
        self.cam = kagra.Camera3D(SW, SH, fov_deg=32.0)
        self.cam.use_orbit(radius=2.8, theta=math.pi, phi=0.12, target=(0.0, 0.9, 0.0))
        self.vrm_id = kagra.load_vrm(VRM_PATH)
        self.posing = False

    def update(self, dt):
        t = kagra.tick_count()
        eng = kagra.get_engine()
        if self.posing:
            s45 = math.sin(math.pi / 4 / 2)
            c45 = math.cos(math.pi / 4 / 2)
            eng.set_vrm_bone_rot(self.vrm_id, "J_Bip_L_UpperArm", 0, 0, s45, c45)
            eng.set_vrm_bone_rot(self.vrm_id, "J_Bip_R_LowerArm", s45, 0, 0, c45)
            eng.set_vrm_bone_rot(self.vrm_id, "J_Bip_L_UpperLeg", s45, 0, 0, c45)
        self.cam.update(eng)
        if t == 10:
            kagra.screenshot(OUT_TPOSE)
        if t == 15:
            self.posing = True
        if t == 30:
            kagra.screenshot(OUT_POSED)
        if t >= 40:
            kagra.quit()

    def draw(self):
        kagra.cls(16, 12, 32)
        kagra.draw_vrm(self.vrm_id)


if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="pose regression", fps=60, visible=False)
    kagra.run(start_scene=PoseScene(), max_frames=60, fixed_dt=1.0 / 60.0)
