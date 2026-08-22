"""エージェント自己検証デモ。

ウィンドウを出さず、固定 dt で N フレーム回し、入力注入 → スクリーンショット → 終了する。
人間の操作なしで描画結果を PNG として得られる。

使い方:
    python examples/agent_verify_demo.py
    # → scratch/agent_verify/ に idle.png / banzai.png が出力される
"""
from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import kagra
from kagra.vrm_action import ActionController

SW, SH = 640, 360
OUT_DIR = os.path.join("scratch", "agent_verify")


class VerifyScene(kagra.Scene):
    def on_enter(self):
        os.makedirs(OUT_DIR, exist_ok=True)
        kagra.font()

        vrm_path = "assets/Emma.vrm"
        if not os.path.exists(vrm_path):
            print(f"VRM not found: {vrm_path}")
            kagra.quit()
            return

        self.avatar = kagra.avatar(vrm_path)
        self.action = ActionController(self.avatar)
        self.avatar.play("idle", loop=True)
        self.avatar.enable_emotion()

        self.cam = kagra.Camera3D(SW, SH, fov_deg=35.0)
        self.cam.use_orbit(radius=2.8, theta=0.0, phi=0.2, target=(0, 0.9, 0))

        self.phase = "warmup"

    def update(self, dt):
        # tick_count は Phase9 フック由来（1 始まりに近い）
        t = kagra.tick_count()

        self.avatar.update(dt)
        self.action.update(dt)

        engine = kagra.get_engine()
        if engine:
            self.cam.update(engine)

        # フレーム 30: idle をキャプチャ
        if t == 30 and self.phase == "warmup":
            kagra.screenshot(os.path.join(OUT_DIR, "idle.png"))
            self.phase = "idle_shot"

        # フレーム 31: バンザイを予約（次フレームの update で pressed になる）
        if t == 31:
            kagra.inject_key("1")

        if kagra.pressed("1"):
            self.action.play("banzai")
            self.avatar.feel("joy", intensity=1.0)
            self.phase = "banzai"

        # バンザイのピーク付近をキャプチャ
        if t == 55 and self.phase == "banzai":
            kagra.screenshot(os.path.join(OUT_DIR, "banzai.png"))
            self.phase = "done"

        if t >= 70:
            kagra.quit()

    def draw(self):
        kagra.cls(40, 45, 50)
        kagra.draw_vrm(self.avatar.vrm_id)
        kagra.text(f"t={kagra.tick_count()}  phase={self.phase}", 12, 12, 16, (255, 255, 120))


if __name__ == "__main__":
    kagra.init(
        width=SW,
        height=SH,
        title="Agent Verify",
        fps=60,
        visible=False,  # 隠れウィンドウ
    )
    # 固定 dt・70 フレームで自動終了（全速）
    kagra.run(start_scene=VerifyScene(), max_frames=70, fixed_dt=1.0 / 60.0)
    print(f"done. frames={kagra.frame_count()}  out={OUT_DIR}")
    for name in ("idle.png", "banzai.png"):
        path = os.path.join(OUT_DIR, name)
        print(f"  {'OK' if os.path.exists(path) else 'MISSING'}: {path}")
