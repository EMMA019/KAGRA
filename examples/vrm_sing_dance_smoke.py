"""vrm_sing_dance の自己検証（エージェント閉ループ用）。

隠れウィンドウ・固定 dt で 60 フレーム回し、歌とダンスの最中を
スクリーンショットして終了する。人間の操作は不要。

    python examples/vrm_sing_dance_smoke.py
    # → scratch/sing_dance_smoke.png
"""
from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import kagra

SW, SH = 640, 360
OUT = os.path.join("scratch", "sing_dance_smoke.png")


class SmokeScene(kagra.Scene):
    def on_enter(self):
        from kagra.contracts import AssetKind, resolve_asset

        vrm = resolve_asset(AssetKind.VRM, "Emma", required=False)
        if vrm is None:
            print("VRM not found (assets/Emma.vrm) — abort")
            kagra.quit()
            return

        self.cam = kagra.Camera3D(SW, SH, fov_deg=32.0)
        self.cam.use_orbit(radius=2.8, phi=0.15, target=(0.0, 0.9, 0.0))

        self.av = kagra.avatar(str(vrm))
        self.av.dance()
        duration = self.av.sing()
        print(f"song: {duration:.1f}s  clips: {self.av.clips}")

    def update(self, dt):
        t = kagra.tick_count()
        self.av.update(dt)
        self.cam.update(kagra.get_engine())

        # フレーム 45 (0.75秒): ダンス中・発声中
        if t == 45:
            os.makedirs("scratch", exist_ok=True)
            kagra.screenshot(OUT)
        if t >= 60:
            kagra.quit()

    def draw(self):
        kagra.cls(16, 12, 32)
        kagra.draw_vrm(self.av.vrm_id)


if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="sing_dance smoke", fps=60, visible=False)
    kagra.run(start_scene=SmokeScene(), max_frames=60, fixed_dt=1.0 / 60.0)
    ok = os.path.exists(OUT)
    print(f"{'OK' if ok else 'MISSING'}: {OUT}")
