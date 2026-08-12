"""
bvh_demo.py - BVH モーションプレイヤーデモ
=============================================
assets/hiphop.bvh を VRM キャラクターで再生します。

操作:
  SPACE : 再生 / 停止
  ← →  : 速度変更 (0.2x ~ 3.0x)
  R     : リセット（頭出し）
  ESC   : 終了

必要ファイル:
  assets/Emma.vrm (または player.vrm / 1.vrm)
  assets/hiphop.bvh
"""
import math
import os
import kagra
from kagra.camera3d import Camera3D

SW, SH = 1280, 720

# ── 設定（お好みで変更）────────────────────────────────────────
VRM_PATH = "assets/Emma.vrm"      # 使用するVRMモデル（存在しない場合は1.vrm等に変更）
BVH_PATH = "assets/Hip_Hop_Dancing.fbx"    # 再生するBVH/FBXモーション


class BvhDemoScene(kagra.Scene):
    def on_enter(self):
        # ── フォント ───────────────────────────────────────────
        # システムフォントから自動検索
        self.font = kagra.assets.font("meiryo")
        print(f"[Demo] Font loaded via assets.font('meiryo'): font_id={self.font}")

        # ── 3Dカメラ ──────────────────────────────────────────
        self.cam = Camera3D(SW, SH, fov_deg=45.0)
        self.cam.use_orbit(radius=2.8, theta=0.0, phi=1.2, target=(0.0, 1.0, 0.0))
        self.cam_angle = 0.0

        # ── ファイル存在チェック ──────────────────────────────
        self.error = None
        if not os.path.exists(VRM_PATH):
            self.error = f"VRM not found: {VRM_PATH}"
            print(f"[Demo] ERROR: {self.error}")
            self.avatar = None
            return
        if not os.path.exists(BVH_PATH):
            self.error = f"BVH not found: {BVH_PATH}"
            print(f"[Demo] ERROR: {self.error}")
            self.avatar = None
            return

        # ── VRM アバター ──────────────────────────────────────
        self.avatar = kagra.avatar(VRM_PATH)
        print(f"[Demo] VRM loaded: {VRM_PATH}")
        print(f"[Demo]   blend shapes: {self.avatar.expressions}")

        # ── BVH モーションをロードして登録 ────────────────────
        # 1行API: avatar.load_motion("name", "file.bvh")
        self.avatar.load_motion("hiphop", BVH_PATH)

        # # 詳細制御したい場合は代わりに以下を使う:
        # motion = kagra.load_bvh(BVH_PATH)
        # print(f"[Demo] BVH: {motion.fps:.1f}fps / {motion.duration:.1f}sec / {len(motion.frames)} frames")
        # self.avatar.add_motion("hiphop", motion)

        # 再生開始
        self.avatar.play("hiphop", loop=True)
        self.is_playing = True
        self.play_speed = 1.0

        # モーション情報を取得
        self.motion_info = self._load_motion_info()

        print(f"[Demo] Playing: hiphop.bvh")
        print(f"[Demo]   available clips: {self.avatar.clips}")

    def _load_motion_info(self):
        """BVH を再ロードしてメタ情報を取得する。"""
        try:
            motion = kagra.load_bvh(BVH_PATH)
            return {
                "fps":    motion.fps,
                "dur":    motion.duration,
                "frames": len(motion.frames),
            }
        except Exception:
            return {"fps": 30, "dur": 0, "frames": 0}

    def update(self, dt):
        if self.avatar is None:
            return

        dt = min(dt, 0.05)

        # ── 入力 ──────────────────────────────────────────────
        if kagra.pressed("ESCAPE"):
            raise SystemExit

        # SPACE: 再生/停止
        if kagra.pressed("SPACE"):
            self.is_playing = not self.is_playing
            if self.is_playing:
                self.avatar._anim._playing = True
                print("[Demo] ▶ Resumed")
            else:
                self.avatar._anim._playing = False
                print("[Demo] ⏸ Paused")

        # ←→: 速度変更
        if kagra.pressed("LEFT"):
            self.play_speed = max(0.2, self.play_speed - 0.2)
            print(f"[Demo] Speed: {self.play_speed:.1f}x")
        if kagra.pressed("RIGHT"):
            self.play_speed = min(3.0, self.play_speed + 0.2)
            print(f"[Demo] Speed: {self.play_speed:.1f}x")

        # R: リセット（先頭フレームへ）
        if kagra.pressed("R"):
            self.avatar._anim._fidx = 0
            self.avatar._anim._t = 0.0
            print("[Demo] ↺ Reset to frame 0")

        # ── アバター更新 ──────────────────────────────────────
        if self.is_playing:
            # speed を乗せた dt でアニメーション更新
            self.avatar.update(dt * self.play_speed)

        # ── カメラ（ゆっくり自動回転） ────────────────────────
        self.cam_angle += dt * 0.3
        self.cam.use_orbit(radius=2.8, theta=self.cam_angle,
                           phi=1.2, target=(0.0, 1.0, 0.0))
        self.cam.update(kagra.get_engine())

    def draw(self):
        kagra.cls(20, 24, 40)

        # ── エラー表示 ────────────────────────────────────────
        if self.error:
            kagra.draw_text(self.font, f"[ERROR] {self.error}",
                            20, SH // 2 - 10, 28, (255, 80, 80))
            return

        # ── VRM 描画 ───────────────────────────────────────────
        kagra.draw_vrm(self.avatar.vrm_id)

        # ── HUD（半透明背景） ──────────────────────────────────
        kagra.rect(0, 0, SW, 130, 0, 0, 0, 140)

        # タイトル
        kagra.draw_text(self.font, "BVH Motion Player Demo",
                        20, 12, 28, (255, 220, 100))

        # -- 1行目: BVH メタ情報 --
        mi = self.motion_info
        kagra.draw_text(self.font,
                        f"BVH: hiphop.bvh  |  {mi['frames']} frames  "
                        f"@ {mi['fps']:.0f}fps  ({mi['dur']:.1f}sec)",
                        20, 48, 16, (180, 200, 220))

        # -- 2行目: 再生状態 --
        anim = self.avatar._anim
        total = max(1, len(anim._frames))
        frame_idx = min(anim._fidx, total - 1)

        status = "▶ PLAYING" if self.is_playing else "⏸ PAUSED"
        status_color = (180, 255, 180) if self.is_playing else (255, 200, 150)

        kagra.draw_text(self.font,
                        f"Clip: \"{anim.clip}\"  |  {status}  |  "
                        f"Frame: {frame_idx + 1}/{total}  |  "
                        f"Speed: {self.play_speed:.1f}x",
                        20, 70, 16, status_color)

        # -- プログレスバー --
        progress = (frame_idx + 1) / total
        bar_x, bar_y = 20, 95
        bar_w, bar_h = 500, 12
        kagra.rect(bar_x, bar_y, bar_w, bar_h, 60, 60, 80)
        kagra.rect(bar_x, bar_y, int(bar_w * progress), bar_h,
                   100, 200, 255)

        # 時間表記
        current_sec = frame_idx * (mi["dur"] / max(1, total))
        kagra.draw_text(self.font,
                        f"{current_sec:.1f}s / {mi['dur']:.1f}s",
                        bar_x + bar_w + 10, bar_y - 2, 14, (160, 180, 200))

        # -- 下部 操作ガイド --
        kagra.rect(0, SH - 45, SW, 45, 0, 0, 0, 140)
        kagra.draw_text(self.font,
                        "SPACE: 再生/停止  |  ←→: 速度変更  |  "
                        "R: リセット  |  ESC: 終了",
                        20, SH - 32, 16, (180, 180, 200))

        # FPS
        try:
            fps = kagra.tick_count()
            kagra.draw_text(self.font, f"FPS: {fps}",
                            SW - 130, 12, 16, (180, 220, 180))
        except Exception:
            pass


if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="KAGRA - BVH Motion Player", fps=60)
    kagra.run(start_scene=BvhDemoScene())
