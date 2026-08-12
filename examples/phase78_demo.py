"""
KAGRA Phase 7+8 デモ（リファクタ版）
====================================
VRM キャラクターに視線追従・リップシンク・IK・感情表情・AI 会話を
統合したデモ。公開 API（`avatar.spring_bone` / `avatar.emotion` ほか）
経由でアクセスし、サブシステムの状態を HUD で可視化する。

操作:
    マウス移動   : キャラが目で追う
    Z キー       : 喜び表情 + hand wave
    X キー       : 悲しみ表情 + bow（sad シェイプが無いモデルは fallback）
    SPACE        : AI チャット（テスト発話）
    数字 1〜5    : 各感情テスト (feel 経由)
    数字 6       : 直接 set_expression("happy", 1.0)
    数字 7       : 強制 happy ON/OFF
    T キー       : 右腕 IK トグル
    B キー       : SpringBone 有効／無効トグル
    R キー       : ポーズリセット
    ESC          : 終了
"""
from __future__ import annotations
import logging
from datetime import datetime

import kagra
from kagra.ai_character import AiCharacter

SW, SH = 1280, 720


class DemoScene(kagra.Scene):

    # ── 初期化 ────────────────────────────────────────────────

    def on_enter(self):
        logging.basicConfig(
            level=logging.INFO,
            format="%(levelname)s %(name)s: %(message)s",
        )

        self.font = kagra.assets.font("C:/Windows/Fonts/meiryo.ttc")

        self.char = AiCharacter(
            "assets/Emma.vrm",
            system_prompt="あなたは明るく元気なアシスタントです。短く答えてください。",
            eye_height=1.55,
        )
        av = self.char.avatar

        # 起動時診断（プライベート属性は触らず diagnostics() を使う）
        diag = av.diagnostics()
        print("=" * 50)
        print("利用可能なブレンドシェイプ:", diag["blendshapes"])
        print("=" * 50)
        print(f"[DEBUG] SpringBone: loaded={diag['spring_bone']['loaded']}, "
              f"chains={diag['spring_bone']['chains']}, "
              f"enabled={diag['spring_bone']['enabled']}")
        print(f"[DEBUG] lookat={diag['lookat']} lipsync={diag['lipsync']} "
              f"ik={diag['ik']}")
        if diag["emotion"]:
            print("[DEBUG] emotion.resolved:", diag["emotion"].get("resolved"))

        # 姿勢崩れ対策: 起動時は SpringBone 無効化 + ポーズリセット
        if av.spring_bone:
            av.spring_bone.enabled = False
            print("[DEBUG] SpringBone を無効化しました")
        av.reset_pose()
        print("[DEBUG] ポーズリセット完了")

        self._ik_active = False
        self._force_joy = False
        self._log: list[str] = ["Phase 7+8 デモ起動 (リファクタ版)"]

        self.cam = kagra.Camera3D(SW, SH, fov_deg=45.0)
        self.cam.use_orbit(radius=1.5, theta=0.0, phi=0.2,
                           target=(0.0, 1.0, 0.0))

    # ── 毎フレーム更新 ────────────────────────────────────────

    def update(self, dt):
        self.cam.update(kagra.get_engine())
        self.char.update(dt)
        av = self.char.avatar

        self._handle_emotion_keys(av)
        self._handle_direct_expression_keys(av)
        self._handle_motion_keys(av)
        self._handle_subsystem_keys(av)
        self._handle_ai(av)

        if kagra.pressed("ESCAPE"):
            exit()

    # ── キーハンドラ ──────────────────────────────────────────

    def _handle_emotion_keys(self, av):
        emo_map = {
            "1": ("joy", 0.9),
            "2": ("angry", 0.9),
            "3": ("sorrow", 0.9),
            "4": ("surprised", 0.9),
            "5": ("shy", 0.9),
        }
        for key, (emo, intensity) in emo_map.items():
            if kagra.pressed(key):
                av.feel(emo, intensity=intensity)
                self._log_append(f"feel({emo})")

    def _handle_direct_expression_keys(self, av):
        if kagra.pressed("6"):
            av.set_expression("happy", 1.0)
            self._log_append("direct set_expression happy")

        if kagra.pressed("7"):
            self._force_joy = not self._force_joy
            self._log_append(f"force happy = {self._force_joy}")

        if self._force_joy:
            kagra.set_blend_shape(av.vrm_id, "happy", 1.5)
        else:
            # force 解除時も直接ゼロを書き込む（emotion 経由は継続させる）
            kagra.set_blend_shape(av.vrm_id, "happy", 0.0)

    def _handle_motion_keys(self, av):
        if kagra.pressed("Z"):
            av.feel("joy", 0.9)
            av.play("wave", loop=False, on_finish=lambda: av.play("idle"))
            self._log_append("joy + wave")

        if kagra.pressed("X"):
            av.feel("sorrow", 0.8)
            av.play("bow", loop=False, on_finish=lambda: av.play("idle"))
            self._log_append("sorrow + bow")

    def _handle_subsystem_keys(self, av):
        # SpringBone トグル（公開 API 経由）
        if kagra.pressed("B") and av.spring_bone:
            av.spring_bone.enabled = not av.spring_bone.enabled
            if av.spring_bone.enabled:
                av.spring_bone.reset()
            self._log_append(f"SpringBone = {av.spring_bone.enabled}")

        # ポーズリセット
        if kagra.pressed("R"):
            av.reset_pose()
            self._log_append("reset_pose")

        # IK トグル
        if kagra.pressed("T") and av.ik:
            self._ik_active = not self._ik_active
            if not self._ik_active:
                av.ik.release_right()
            self._log_append(f"IK = {self._ik_active}")

        if self._ik_active and av.ik:
            mx, my = kagra.mouse()
            wx = (mx / SW - 0.5) * 2.0
            wy = 1.2 - (my / SH - 0.5) * 1.5
            wz = 0.3
            av.reach_right(wx, wy, wz, weight=0.8)

    def _handle_ai(self, av):
        if kagra.pressed("SPACE"):
            phrases = [
                "こんにちは！元気ですか？",
                "今日も一日頑張りましょう！",
                "何かお手伝いできることはありますか？",
            ]
            import random
            phrase = random.choice(phrases)
            self.char.speak(phrase)
            self._log_append(f"speak: {phrase[:20]}...")

    # ── 描画 ──────────────────────────────────────────────────

    def draw(self):
        kagra.cls(15, 15, 35)
        kagra.draw_vrm(self.char.avatar.vrm_id)

        self.char.draw_bubble(self.font, x=50, y=60, w=600)
        self.char.draw_state(self.font, x=50, y=30)

        self._draw_log()
        self._draw_guide()
        self._draw_hud()

    def _draw_log(self):
        for i, msg in enumerate(self._log[-6:]):
            alpha = max(80, 200 - i * 20)
            kagra.text(msg, SW - 420, SH - 180 + i * 22,
                       font=self.font, size=14,
                       color=(180, 180, 200, alpha))

    def _draw_guide(self):
        guide = [
            "Z: joy+wave   X: sorrow+bow",
            "1-5: feel() 感情テスト",
            "6: set_expression 直接テスト",
            "7: 強制 happy ON/OFF",
            "SPACE: AI 発話",
            "T: 右腕 IK トグル",
            "B: SpringBone トグル   R: ポーズリセット",
            "マウス: 視線追従",
        ]
        for i, g in enumerate(guide):
            kagra.text(g, 20, SH - 200 + i * 22,
                       font=self.font, size=13, color=(120, 120, 160))

    def _draw_hud(self):
        """サブシステム状態 HUD（右上）。"""
        av = self.char.avatar
        sb = av.spring_bone
        em = av.emotion

        sb_state = "-"
        if sb is not None:
            sb_state = f"ON ({len(sb.chains)}ch)" if sb.enabled else "OFF"

        emo_state = "-"
        if em is not None:
            resolved = em.diagnostics()["resolved"]
            ok = sum(1 for v in resolved.values() if v)
            emo_state = f"{ok}/{len(resolved)} (now:{em.current})"

        lines = [
            f"SpringBone : {sb_state}",
            f"LookAt     : {'ON' if av.lookat  else '-'}",
            f"LipSync    : {'ON' if av.lipsync else '-'}",
            f"IK         : {'ON' if av.ik      else '-'} "
            f"(active={self._ik_active})",
            f"Emotion    : {emo_state}",
        ]
        for i, line in enumerate(lines):
            kagra.text(line, SW - 260, 20 + i * 22,
                       font=self.font, size=13,
                       color=(200, 220, 200))

    # ── ログ ──────────────────────────────────────────────────

    def _log_append(self, msg: str):
        ts = datetime.now().strftime("%H:%M:%S")
        self._log.append(f"[{ts}] {msg}")
        print(f"[Demo] {msg}")


if __name__ == "__main__":
    kagra.init(SW, SH, "KAGRA Phase 7+8 Demo (Refactored)", fps=60)
    kagra.run(start_scene=DemoScene())
