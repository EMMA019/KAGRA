# kagra/effects.py
# エフェクトシステム
#
# ワンライナーでエフェクトを生成:
#   kagra.effects.damage(x, y, 42)           # ダメージ数字
#   kagra.effects.heal(x, y, 20)             # 回復数字（緑）
#   kagra.effects.slash(x, y)                # 剣閃
#   kagra.effects.spark(x, y, count=8)       # 火花パーティクル
#   kagra.effects.levelup(x, y)              # レベルアップ
#   kagra.effects.flash(r=255,g=255,b=255)   # 画面フラッシュ
#
#   # 毎フレーム呼ぶ
#   kagra.effects.update(dt)
#   kagra.effects.draw()                     # draw()内で呼ぶ

from __future__ import annotations
import random
import math
from dataclasses import dataclass, field
from typing import Optional


# ── 個別エフェクト ────────────────────────────────────────────

@dataclass
class _FloatText:
    """浮き上がるテキスト（ダメージ数字等）。"""
    x: float; y: float
    text: str
    r: int; g: int; b: int
    size: int  = 18
    vx: float = 0.0
    vy: float = -60.0   # px/秒 上方向
    life: float = 0.0
    max_life: float = 1.2
    font_id: int = 0

    @property
    def alpha(self) -> float:
        t = self.life / self.max_life
        return 1.0 - max(0.0, (t - 0.6) / 0.4)  # 後半でフェードアウト


@dataclass
class _Particle:
    """1粒のパーティクル。"""
    x: float; y: float
    vx: float; vy: float
    r: int; g: int; b: int
    life: float = 0.0
    max_life: float = 0.6
    size: float = 4.0
    gravity: float = 120.0


@dataclass
class _SlashEffect:
    """剣閃エフェクト（矩形のフラッシュ）。"""
    x: float; y: float
    w: float = 32.0; h: float = 8.0
    angle: float = 0.0
    r: int = 255; g: int = 255; b: int = 255
    life: float = 0.0
    max_life: float = 0.18

    @property
    def alpha(self) -> float:
        return 1.0 - self.life / self.max_life


@dataclass
class _ScreenFlash:
    """画面全体フラッシュ。"""
    r: int; g: int; b: int
    life: float = 0.0
    max_life: float = 0.25

    @property
    def alpha(self) -> float:
        return max(0.0, 1.0 - self.life / self.max_life) * 0.6


# ── EffectManager ─────────────────────────────────────────────

class EffectManager:
    """エフェクト全体を管理するシングルトン。

    kagra/__init__.py で kagra.effects = EffectManager() として登録する。
    """

    def __init__(self):
        self._texts:    list[_FloatText]   = []
        self._particles: list[_Particle]  = []
        self._slashes:  list[_SlashEffect] = []
        self._flashes:  list[_ScreenFlash] = []
        self._font: int = 0

    def set_font(self, font_id: int):
        """描画フォントを設定する。on_map_enter() などで呼ぶ。"""
        self._font = font_id

    # ── 生成 API ──────────────────────────────────────────────

    def damage(self, x: float, y: float, amount: int, critical: bool = False):
        """ダメージ数字（赤、クリティカルは大きく）。"""
        self._texts.append(_FloatText(
            x=x + random.uniform(-8, 8),
            y=y,
            text=f"{'⚡' if critical else ''}{amount}",
            r=255, g=80 if critical else 120, b=60,
            size=22 if critical else 18,
            font_id=self._font,
            vy=-80 if critical else -60,
        ))

    def heal(self, x: float, y: float, amount: int):
        """回復数字（緑）。"""
        self._texts.append(_FloatText(
            x=x + random.uniform(-6, 6), y=y,
            text=f"+{amount}",
            r=80, g=240, b=120, size=16,
            font_id=self._font, vy=-50,
        ))

    def miss(self, x: float, y: float):
        """ミス表示。"""
        self._texts.append(_FloatText(
            x=x, y=y, text="MISS",
            r=200, g=200, b=200, size=14,
            font_id=self._font, vy=-40,
        ))

    def levelup(self, x: float, y: float):
        """レベルアップ表示＋パーティクル。"""
        self._texts.append(_FloatText(
            x=x, y=y - 10, text="LEVEL UP!",
            r=255, g=240, b=60, size=20,
            font_id=self._font, vy=-50,
            max_life=2.0,
        ))
        self.spark(x, y, count=16, r=255, g=220, b=60)

    def slash(self, x: float, y: float, angle: float = 45.0):
        """剣閃エフェクト（白いフラッシュライン）。"""
        self._slashes.append(_SlashEffect(
            x=x, y=y,
            w=random.uniform(28, 40),
            h=random.uniform(6, 10),
            angle=angle + random.uniform(-15, 15),
        ))

    def spark(
        self,
        x: float, y: float,
        count: int = 8,
        r: int = 255, g: int = 180, b: int = 60,
        speed: float = 80.0,
    ):
        """放射状の火花パーティクル。"""
        for i in range(count):
            angle = (i / count) * math.pi * 2 + random.uniform(-0.3, 0.3)
            spd   = speed * random.uniform(0.5, 1.5)
            self._particles.append(_Particle(
                x=x, y=y,
                vx=math.cos(angle) * spd,
                vy=math.sin(angle) * spd,
                r=r, g=g, b=b,
                size=random.uniform(3, 6),
                max_life=random.uniform(0.3, 0.7),
            ))

    def puff(self, x: float, y: float, count: int = 6):
        """연기 パフエフェクト（グレー系）。"""
        for _ in range(count):
            self._particles.append(_Particle(
                x=x + random.uniform(-8, 8),
                y=y + random.uniform(-4, 4),
                vx=random.uniform(-30, 30),
                vy=random.uniform(-60, -20),
                r=160, g=160, b=160,
                size=random.uniform(5, 10),
                gravity=0,
                max_life=random.uniform(0.4, 0.8),
            ))

    def flash(self, r: int = 255, g: int = 255, b: int = 255, duration: float = 0.25):
        """画面全体フラッシュ。ダメージ・魔法演出に。"""
        self._flashes.append(_ScreenFlash(r=r, g=g, b=b, max_life=duration))

    def blood(self, x: float, y: float):
        """被ダメ演出（赤パーティクル）。"""
        self.spark(x, y, count=5, r=220, g=40, b=40, speed=50)
        self.flash(r=160, g=0, b=0, duration=0.15)

    # ── 毎フレーム ────────────────────────────────────────────

    def update(self, dt: float):
        for t in self._texts:
            t.life += dt; t.x += t.vx * dt; t.y += t.vy * dt
        for p in self._particles:
            p.life += dt; p.x += p.vx * dt
            p.y += (p.vy + p.gravity * p.life) * dt
        for s in self._slashes:
            s.life += dt
        for f in self._flashes:
            f.life += dt

        self._texts    = [t for t in self._texts    if t.life < t.max_life]
        self._particles= [p for p in self._particles if p.life < p.max_life]
        self._slashes  = [s for s in self._slashes  if s.life < s.max_life]
        self._flashes  = [f for f in self._flashes  if f.life < f.max_life]

    def draw(self):
        """全エフェクトを描画する。draw() の最後に呼ぶ。"""
        import kagra

        # 剣閃
        for s in self._slashes:
            a = int(s.alpha * 255)
            if a <= 0:
                continue
            rad = math.radians(s.angle)
            cos_r, sin_r = math.cos(rad), math.sin(rad)
            # 細い矩形を回転させて描画（簡易: 複数の線として描く）
            for i in range(3):
                offset = (i - 1) * 3
                ox = -sin_r * offset; oy = cos_r * offset
                lx = s.x - cos_r * s.w/2 + ox
                ly = s.y - sin_r * s.w/2 + oy
                kagra.rect(lx, ly, max(1, s.w), max(1, s.h/3),
                           s.r, s.g, s.b)

        # パーティクル
        for p in self._particles:
            age_ratio = p.life / p.max_life
            size = max(1, int(p.size * (1 - age_ratio * 0.5)))
            kagra.rect(p.x - size//2, p.y - size//2, size, size, p.r, p.g, p.b)

        # 浮きテキスト
        for t in self._texts:
            if t.font_id:
                a = max(0, min(255, int(t.alpha * 255)))
                kagra.draw_text(t.font_id, t.text,
                                t.x, t.y, t.size, t.r, t.g, t.b, a)

        # 画面フラッシュ（最前面）
        sw, sh = kagra.get_screen_size()
        for f in self._flashes:
            a = max(0, int(f.alpha * 200))
            if a > 0:
                kagra.rect(0, 0, sw, sh, f.r, f.g, f.b)

    def clear(self):
        """全エフェクトを消去する（シーン切り替え時など）。"""
        self._texts.clear()
        self._particles.clear()
        self._slashes.clear()
        self._flashes.clear()
