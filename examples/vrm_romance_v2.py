"""
vrm_romance_v2.py - KAGRA 恋愛シミュレーション
================================================
モジュール構成:
  kagra_romance/persona.py     - 性格定義・進化
  kagra_romance/components.py  - ECS コンポーネント
  kagra_romance/scripts.py     - ECS スクリプト
  kagra_romance/chat_engine.py - API・会話管理
  kagra_romance/ui.py          - 描画

起動:
  pip install openai python-dotenv
  .env に DEEPSEEK_API_KEY=sk-... を記述
  python vrm_romance_v2.py
"""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))

import kagra
import math
from kagra_romance import (
    PersonalityComp, EmotionComp, ChatComp, EventComp, TimeComp, EffectComp,
    PersonalityScript, ExpressionScript, EffectScript, ChatInputScript,
    EMOTION_EXPR, ui,
)

SW, SH = 1280, 720


class RomanceScene(kagra.EntityScene):

    def on_enter(self):
        self.font    = kagra.assets.font("meiryo")
        self._time   = 0.0
        self._cursor = 0.0
        self._debug  = False  # D キーでデバッグパネル表示

        # ── VRM ─────────────────────────────────────────────
        self.av  = kagra.avatar("assets/Emma.vrm")
        self.av.play("idle")
        self.cam = kagra.Camera3D(SW, SH, fov_deg=55.0)
        self.cam.use_orbit(
            radius=3.5, theta=0.0, phi=0.05,
            target=(-1.5, 1.0, 0.0)
        )

        # ── GPU Boids（エフェクト用）────────────────────────
        self.boid_id = kagra.create_boid_system_gpu(200_000, float(SW), float(SH))
        kagra.set_boid_active_count(self.boid_id, 0)

        # ── ECS ─────────────────────────────────────────────
        self.emma = self.world.create("emma", tag="emma")
        self.emma.add(PersonalityComp())
        self.emma.add(EmotionComp())
        self.emma.add(ChatComp())
        self.emma.add(EventComp())
        self.emma.add(TimeComp())

        ef = EffectComp()
        ef.boid_id = self.boid_id
        self.emma.add(ef)

        self.emma.add(PersonalityScript())
        self.emma.add(ExpressionScript())
        self.emma.add(EffectScript())
        ci = ChatInputScript()
        self.emma.add(ci)

        # ChatInputScript の history を ui 側で参照できるよう保存
        self._ci = ci

    def update(self, dt):
        if kagra.pressed("ESCAPE"):   raise SystemExit
        if kagra.pressed("D"):        self._debug = not self._debug

        super().update(dt)   # ECS world.update（全スクリプト実行）

        self._time   += dt
        self._cursor += dt

        # 時間更新
        tm = self.emma.get(TimeComp)
        if tm: tm.update(dt)

        # VRM 表情
        em = self.emma.get(EmotionComp)
        if em and em.timer > 0:
            self.av.set_expression(EMOTION_EXPR.get(em.current,'Fcl_ALL_Neutral'), 1.0)
        else:
            self.av.reset_expressions()
        self.av.update(dt)

        # カメラ
        self.cam.update(kagra.get_engine())

        # エンディング判定
        p = self.emma.get(PersonalityComp)
        if p and p.route:
            self._time_for_ending = getattr(self, '_time_for_ending', 0) + dt

    def draw(self):
        # 背景
        ui.draw_background(self.emma, self._time)

        # VRM
        kagra.draw_vrm(self.av.vrm_id)

        # Boids
        ef = self.emma.get(EffectComp)
        if ef and ef.active:
            kagra.draw_boids_gpu(self.boid_id)

        # UI
        # pending 状態を history に渡す
        self._ci.history._pending_flag = self._ci.engine.pending
        ui.draw_chat(
            font         = self.font,
            emma_entity  = self.emma,
            history      = self._ci.history,
            cursor_f     = self._cursor,
            debug        = self._debug,
        )

        # フラッシュ
        ui.draw_flash(ef)

        # エンディング
        p = self.emma.get(PersonalityComp)
        if p and p.route:
            ui.draw_ending(self.font, self.emma,
                           getattr(self,'_time_for_ending',0))


if __name__ == "__main__":
    kagra.init(SW, SH, "KAGRA - Emma Romance / 性格進化 AI", 60)
    kagra.run(start_scene=RomanceScene())
