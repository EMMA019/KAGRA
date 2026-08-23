"""
boids_demo_gpu.py - 100万匹（GPU Compute Shader 版）
=====================================================
CPU 転送ゼロ。ボイド計算も描画も全部 GPU。

操作: ESC で終了
"""
import kagra

SW, SH  = 1280, 720
COUNT   = 2_000_000

class BoidGpuScene(kagra.Scene):
    def on_enter(self):
        self.font = kagra.assets.font("meiryo")

        # GPU 上にボイドシステムを作成（CPU 転送ゼロ）
        self.boid_id = kagra.get_engine().create_boid_system_gpu(COUNT, SW, SH)
        self.fps_display = 60.0
        self._timer = 0.0

    def update(self, dt):
        if kagra.pressed("ESCAPE"): raise SystemExit

        # GPU で全匹を更新（CPU 関与ゼロ）
        kagra.get_engine().update_boids_gpu(self.boid_id, dt)

        self.fps_display = self.fps_display * 0.95 + kagra.get_fps() * 0.05
        self._timer += dt
        if self._timer >= 1.0:
            self._timer = 0.0
            print(f"GPU Boids: {COUNT:,}  {self.fps_display:.0f} fps")

    def draw(self):
        kagra.cls(5, 5, 15)
        kagra.rect(0, 0, SW, SH, 5, 5, 15, 255)

        # GPU バッファを直接描画（転送なし）
        kagra.get_engine().draw_boids_gpu(self.boid_id)

if __name__ == "__main__":
    kagra.init(SW, SH, "KAGRA - GPU Compute Boids", 60)
    kagra.run(start_scene=BoidGpuScene())
