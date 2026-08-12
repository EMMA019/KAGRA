"""
boids_demo.py - 100万匹のボイド（Rust 計算版）
=================================================
Python ループなし。位置計算は全部 Rust。

操作:
  ↑ ↓ : 匹数を増減（×10）
  ESC  : 終了
"""
import kagra

SW, SH = 1960, 1080
COUNT  = 1_000_000  # 初期匹数

class BoidScene(kagra.Scene):
    def on_enter(self):
        self.font = kagra.assets.font("meiryo")

        # Rust 側にボイドシステムを作成
        self.boid_id = kagra.get_engine().create_boid_system(COUNT, SW, SH)

        # GPU インスタンスバッチを作成（最大 100万）
        self.batch_id = kagra.get_engine().create_instance_batch(
            0, 1_000_000, 6.0, 3.0
        )

        self.fps_display = 60.0

    def update(self, dt):
        if kagra.pressed("ESCAPE"): raise SystemExit

        # Rust で全匹を更新（Python ループなし）
        kagra.get_engine().update_boids(self.boid_id, dt)

        self.fps_display = self.fps_display * 0.95 + kagra.get_fps() * 0.05

        # fps をコンソールに出力（1秒おき）
        self._timer = getattr(self, '_timer', 0.0) + dt
        if self._timer >= 1.0:
            self._timer = 0.0
            count = kagra.get_engine().boid_count(self.boid_id)
            print(f"Rust Boids: {count:,}  {self.fps_display:.0f} fps")

    def draw(self):
    # cls で背景色を設定（最初のレンダーパスでクリアされる）
       kagra.cls(5, 5, 15)
    # Boids を描画（インスタンスレンダリング）
       kagra.get_engine().draw_boids(self.boid_id, self.batch_id, 6.0, 3.0)


if __name__ == "__main__":
    kagra.init(SW, SH, "KAGRA - Rust Boids", 60)
    kagra.run(start_scene=BoidScene())
