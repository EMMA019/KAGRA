"""
boids_night_sky.py - 夜空デモ
================================
1匹から始まり、倍々で増殖していく GPU Compute Boids。
最終的に100万匹の星空になる。

操作: ESC で終了 / R でリセット
"""
import kagra
import math

SW, SH = 1280, 720

# 増殖ステップ: 1 → 2 → 4 → ... → 1,048,576
STEPS = [2**i for i in range(21)]   # 1〜1048576
STEP_INTERVAL = 2.0                  # 何秒ごとに増殖するか
UI_TOP    = 64     # UI エリアの高さ
UI_BOTTOM = 36     # 下部 UI の高さ

class NightSkyScene(kagra.Scene):

    def on_enter(self):
        self.font      = kagra.assets.font("meiryo")
        self.step_idx  = 0
        self.timer     = 0.0
        self.fps_avg   = 60.0
        self.boid_id   = None
        self._spawn(STEPS[0])

    def _spawn(self, count):
        """ボイドシステムを作り直す"""
        self.boid_id = kagra.get_engine().create_boid_system_gpu(count, float(SW), float(SH))

    def update(self, dt):
        if kagra.pressed("ESCAPE"): raise SystemExit
        if kagra.pressed("X"):
            self.step_idx = 0
            self.timer    = 0.0
            self._spawn(STEPS[0])
            return

        # 倍々増殖タイマー
        self.timer += dt
        if self.timer >= STEP_INTERVAL and self.step_idx < len(STEPS) - 1:
            self.timer    = 0.0
            self.step_idx += 1
            self._spawn(STEPS[self.step_idx])

        # GPU でボイドを更新
        kagra.get_engine().update_boids_gpu(self.boid_id, dt)

        self.fps_avg = self.fps_avg * 0.95 + kagra.get_fps() * 0.05

    def draw(self):
        kagra.cls(3, 3, 12)                 # 深夜色の背景
        kagra.rect(0, 0, SW, SH, 3, 3, 12, 255)

        # GPU ボイドを描画（転送ゼロ）
        kagra.get_engine().draw_boids_gpu(self.boid_id)

        # ── UI ──────────────────────────────────────────────
        count   = STEPS[self.step_idx]
        next_c  = STEPS[self.step_idx + 1] if self.step_idx < len(STEPS)-1 else count
        remain  = max(0.0, STEP_INTERVAL - self.timer)

        # 上部バー
        kagra.rect(0, 0, SW, 56, 0, 0, 0, 180)

        # 匹数・fps
        kagra.draw_text(self.font,
            f"{count:,} boids",
            20, 8, 28, 255, 230, 100)
        kagra.draw_text(self.font,
            f"{self.fps_avg:.0f} fps",
            SW - 130, 8, 28, 100, 255, 180)

        # 次の増殖までのカウントダウン
        if self.step_idx < len(STEPS) - 1:
            bar_w = int((1.0 - remain / STEP_INTERVAL) * (SW - 40))
            kagra.rect(20, 42, bar_w, 8, 80, 160, 255, 200)
            kagra.rect(20, 42, SW-40, 8, 255, 255, 255, 40)
            kagra.draw_text(self.font,
                f"next: {next_c:,}  ({remain:.1f}s)",
                SW//2 - 100, 44, 14, 160, 200, 255)
        else:
            kagra.draw_text(self.font,
                "MAX  1,048,576 boids",
                SW//2 - 120, 44, 14, 255, 180, 80)

        # 下部ヒント
        kagra.rect(0, SH-28, SW, 28, 0, 0, 0, 140)
        kagra.draw_text(self.font,
            "GPU Compute Shader  |  Python 3 lines  |  X:reset  ESC:quit",
            20, SH-20, 14, 120, 140, 180)


if __name__ == "__main__":
    kagra.init(SW, SH, "KAGRA - Night Sky Boids", 60)
    kagra.run(start_scene=NightSkyScene())
