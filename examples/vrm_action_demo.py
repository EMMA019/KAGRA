# examples/vrm_action_demo.py
"""
KAGRA Agent UI - ワンショットアクション テストデモ

テンキーや数字キーを押すことで、AIエージェントが自発的に行う
アクション（バンザイ、お辞儀など）をテストします。
"""
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

import kagra
from kagra.vrm_action import ActionController

SW, SH = 1280, 720

class ActionDemoScene(kagra.Scene):
    def on_enter(self):
        kagra.font("C:/Windows/Fonts/meiryo.ttc")
        
        # VRM モデルのロード
        vrm_path = "assets/Emma.vrm"
        if not os.path.exists(vrm_path):
            print("VRMモデルが見つかりません。")
            kagra.exit()
            
        self.avatar = kagra.avatar(vrm_path)
        
        # 新規作成したアクションコントローラーをアタッチ
        self.action = ActionController(self.avatar)
        
        # 基本の待機モーションを再生（瞬き・揺れ物も自動で動く）
        self.avatar.play("idle", loop=True)
        self.avatar.enable_emotion()

        # 3Dカメラの設定
        # target=(0, 0.9, 0) でモデルの胸付近を中心に合わせる
        # phi=0.2 で少し上から見下ろす角度に
        self.cam = kagra.Camera3D(SW, SH, fov_deg=35.0)
        self.cam.use_orbit(radius=2.8, theta=0.0, phi=0.2, target=(0, 0.9, 0))
        
        self.current_action_name = "None"
    

    def update(self, dt):
        # ── キー入力によるアクショントリガー ──
        if kagra.pressed("1"): self._trigger("banzai", "joy")
        if kagra.pressed("2"): self._trigger("nod", "neutral")
        if kagra.pressed("3"): self._trigger("shake_head", "sad")
        if kagra.pressed("4"): self._trigger("tilt_head", "surprised")
        if kagra.pressed("5"): self._trigger("jump_joy", "joy")
        if kagra.pressed("6"): self._trigger("wave", "happy")
        if kagra.pressed("7"): self._trigger("think", "neutral")
        if kagra.pressed("8"): self._trigger("bow", "neutral")
        if kagra.pressed("9"): self._trigger("clap", "joy")

        # ── 1. アバター全体の更新（待機モーション等） ──
        self.avatar.update(dt)
        
        # ── 2. アクションの更新（待機モーションの上に腕や首の動きを上書き） ──
        self.action.update(dt)

        # カメラ更新
        engine = getattr(kagra, 'get_engine', lambda: getattr(kagra, '_engine', None))()
        if engine:
            self.cam.update(engine)

        if kagra.pressed("ESCAPE"):
            kagra.exit()

    def _trigger(self, action_name: str, emotion: str):
        """アクションと表情を同時にセットする"""
        self.current_action_name = action_name
        self.action.play(action_name)
        self.avatar.feel(emotion, intensity=1.0) # 表情も合わせて変える！

    def draw(self):
        kagra.cls(40, 45, 50)
        
        # VRM の描画
        kagra.draw_vrm(self.avatar.vrm_id)
        
        # UI 描画
        kagra.fill(0, 0, 320, SH, (20, 25, 30), alpha=200)
        kagra.text("KAGRA Agent Action Test", 20, 20, 20, (150, 200, 255))
        kagra.text(f"Playing: {self.current_action_name}", 20, 50, 24, (255, 255, 100))
        
        y = 100
        controls = [
            ("1", "バンザイ (banzai)"),
            ("2", "うなずく (nod)"),
            ("3", "首を振る (shake_head)"),
            ("4", "首をかしげる (tilt_head)"),
            ("5", "ジャンプ (jump_joy)"),
            ("6", "手を振る (wave)"),
            ("7", "考える (think)"),
            ("8", "お辞儀 (bow)"),
            ("9", "拍手 (clap)"),
            ("ESC", "終了")
        ]
        
        for key, desc in controls:
            kagra.text(f"[{key}]", 20, y, 20, (255, 200, 100))
            kagra.text(desc, 70, y, 20, (200, 200, 200))
            y += 35


if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="Agent Action Demo", fps=60)
    kagra.run(start_scene=ActionDemoScene())