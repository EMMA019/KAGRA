"""
VRM 顔トラッキングデモ (Phase 7+) - エンジン対応版

MediaPipe Face Mesh を使ってカメラの顔をリアルタイムに VRM キャラクターに反映。

必要: pip install mediapipe opencv-python
      または pip install kagra[facetrack]

使い方:
    1. カメラを起動
    2. 顔を映すと VRM が追従
    3. ESC で終了

Example::
    python examples/vrm_facetrack_demo.py
"""
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

import kagra

class FaceTrackDemo(kagra.Scene):
    def on_enter(self):
        print("=" * 60)
        print("VRM 顔トラッキングデモ")
        print("=" * 60)

        # VRM モデルをロード
        vrm_path = "assets/Emma.vrm"
        if not os.path.exists(vrm_path):
            # フォールバック: 他の VRM を探す
            import glob
            candidates = glob.glob("assets/*.vrm") + glob.glob("assets/**/*.vrm")
            if candidates:
                vrm_path = candidates[0]
                print(f"使用モデル: {vrm_path}")
            else:
                print("VRM モデルが見つかりません。 assets/ に .vrm ファイルを配置してください。")
                self._skip = True
                return

        self._skip = False

        # VrmAvatar を作成
        self.avatar = kagra.avatar(vrm_path)
        self.avatar.play("idle")

        # 顔トラッキングを有効化 (カメラID=1)
        try:
            self.facetrack = self.avatar.enable_facetracking(camera_id=1)
            self.facetrack.enable()
            print()
            print("✅ 顔トラッキング開始！")
            print("   カメラに顔を映すと VRM が追従します。")
        except ImportError as e:
            print(f"❌ {e}")
            print("   pip install mediapipe opencv-python")
            self._skip = True
            return

        # 背景
        self.bg_color = (30, 30, 50)

        # 3Dカメラ設定（VRM表示に必須）
        sw, sh = kagra.get_screen_size()
        self.cam = kagra.Camera3D(sw, sh, fov_deg=35.0)
        self.cam.use_orbit(radius=2.5, theta=0.0, phi=0.05, target=(0, 0.75, 0))

    def update(self, dt):
        if self._skip:
            return

        # VrmAvatar.update() の中で自動的に facetrack.update() が呼ばれる
        self.avatar.update(dt)

        # カメラ更新 (kagra のエンジンを取得して渡す)
        engine = getattr(kagra, 'get_engine', lambda: getattr(kagra, '_engine', None))()
        if engine:
            self.cam.update(engine)
        # フォールバック：もし上記で取得できなければ _engine を直接使う
        elif hasattr(kagra, '_engine'):
            self.cam.update(kagra._engine)

        # ESC で終了
        if kagra.pressed("ESCAPE"):
            self.facetrack.disable()
            kagra.exit()

    def draw(self):
        if self._skip:
            kagra.cls(0, 0, 0)
            kagra.draw_text(self.font, "VRM モデルが見つかりません。", 100, 200, 20, (255, 100, 100))
            kagra.draw_text(self.font, "assets/ に .vrm ファイルを配置してください。", 100, 230, 16, (200, 200, 200))
            return

        # 背景クリア
        kagra.cls(*self.bg_color)

        # VRM を描画（3Dカメラは自動で適用される想定）
        kagra.draw_vrm(self.avatar.vrm_id)


# ── エントリポイント ─────────────────────────────────────────────
if __name__ == "__main__":
    kagra.init(width=800, height=600, title="VRM FaceTrack Demo", fps=60)
    kagra.run(start_scene=FaceTrackDemo())