"""
KAGRA Phase 9 デモ
==================
Phase 9 の全機能を1ファイルで確認できるデモ。

操作:
  ~ (バッククォート) または F1 : コンソール開閉
  H キー                       : HTTP テスト（ポケモン API）
  R キー                       : ジオメトリデモ切り替え
  ESC                          : 終了

コンソール使用例:
  >>> scene.score = 9999
  >>> kagra.se("assets/audio/coin.wav")
  >>> kagra.fill(0,0,100,100,(255,0,0))
"""

import math
import kagra
from kagra.console     import DevConsole
from kagra.http_client import http_get, http_tick

SW, SH = 1280, 720


class Phase9DemoScene(kagra.Scene):

    def on_enter(self):
        self.font  = kagra.assets.font("meiryo")
        self.score = 0
        self.t     = 0.0

        # ── Phase 9b: コンソール ──────────────────────────
        self.console = DevConsole(self)

        # ── HTTP テスト用 ─────────────────────────────────
        self._http_req  = None
        self._http_text = "H キーで HTTP テスト"

        # ── ジオメトリデモ用 ──────────────────────────────
        self._show_geo  = False
        self._geo_items = [
            {"x": 200, "y": 300, "w": 120, "h": 80},
            {"x": 700, "y": 200, "w": 100, "h": 100},
            {"x": 500, "y": 400, "w": 80,  "h": 60},
        ]

        # ── tick_count / frame_index デモ用 ───────────────
        self._anim_tex  = None  # テクスチャがあれば使う

    def update(self, dt):
        self.t += dt

        # コンソール更新（これで ~ キーが機能する）
        self.console.update(dt)

        # HTTP 結果チェック
        if self._http_req and self._http_req.done:
            if self._http_req.ok:
                try:
                    data = self._http_req.json()
                    name   = data.get("name", "?")
                    height = data.get("height", "?")
                    weight = data.get("weight", "?")
                    self._http_text = f"GET OK! name={name}  h={height}  w={weight}"
                except Exception as e:
                    self._http_text = f"JSON エラー: {e}"
            else:
                self._http_text = f"HTTP エラー: {self._http_req.status}"
            self._http_req = None

        # H キーで HTTP テスト
        if kagra.pressed("H") and self._http_req is None:
            pid = (kagra.tick_count() % 151) + 1
            self._http_text = f"リクエスト送信中... (#{pid})"
            self._http_req  = http_get(f"https://pokeapi.co/api/v2/pokemon/{pid}")

        # R キーでジオメトリデモ切り替え
        if kagra.pressed("R"):
            self._show_geo = not self._show_geo

        # ESC で終了
        if kagra.pressed("ESCAPE"):
            exit()

        # スコア増加
        if kagra.every(60):
            self.score += 1

    def draw(self):
        kagra.cls(18, 20, 38)

        # ── tick_count / frame_index デモ ─────────────────
        tc = kagra.tick_count()

        # 4フレーム4枚の数字アニメ（見た目的に色変化で代用）
        fi = kagra.frame_index(count=4, hold_for=15)
        colors = [(255,100,100), (100,255,100), (100,100,255), (255,255,100)]
        kagra.fill(40, 40, 40, 40, colors[fi])
        kagra.text(f"frame_index: {fi}", 90, 48, 16,
                   color=(200,200,200), font=self.font)

        # tick_count 表示
        kagra.text(f"tick_count : {tc}  ({tc/60:.1f}s)",
                   40, 80, 16, color=(180,180,200), font=self.font)

        # every() デモ（1秒ごとに点滅する丸）
        pulse = kagra.every(30)  # 0.5秒周期
        alpha = 255 if pulse else 60
        kagra.fill(40, 110, 20, 20, (100, 220, 100, alpha))
        kagra.text("every(30)", 70, 114, 16, color=(180,200,180), font=self.font)

        # lerp デモ（サイン波追従）
        target_x = SW // 2 + math.sin(self.t) * 300
        _Phase9DemoScene_lerp_x[0] = kagra.lerp(
            _Phase9DemoScene_lerp_x[0], target_x, 0.05
        )
        kagra.fill(_Phase9DemoScene_lerp_x[0] - 12, 155, 24, 24, (255, 180, 80))
        kagra.text("lerp追従", _Phase9DemoScene_lerp_x[0] - 20, 140, 13,
                   color=(200,180,100), font=self.font)

        # ── スコア ────────────────────────────────────────
        kagra.text(f"SCORE: {self.score}", 40, 200, 28,
                   color=(255,220,100), font=self.font)

        # ── HTTP 結果 ─────────────────────────────────────
        kagra.fill(40, 250, SW - 80, 34, (30, 35, 55))
        kagra.text(self._http_text, 50, 258, 16,
                   color=(150, 220, 255), font=self.font)

        # ── ジオメトリデモ ────────────────────────────────
        if self._show_geo:
            mx, my = kagra.mouse()
            for item in self._geo_items:
                hit = kagra.intersect_rect(
                    mx, my, 1, 1,
                    item["x"], item["y"], item["w"], item["h"]
                )
                col = (100, 255, 100) if hit else (100, 100, 200)
                kagra.fill(item["x"], item["y"], item["w"], item["h"], col + (100,))
                if hit:
                    kagra.text("HIT!", item["x"] + 5, item["y"] + 5, 14,
                               color=(255,255,255), font=self.font)

            # マウスと各オブジェクトの距離
            for i, item in enumerate(self._geo_items):
                cx = item["x"] + item["w"] / 2
                cy = item["y"] + item["h"] / 2
                d  = kagra.distance(mx, my, cx, cy)
                kagra.text(f"d={d:.0f}", item["x"], item["y"] - 18, 12,
                           color=(180,180,220), font=self.font)

            # circle hit test
            cr = 60
            circle_hit = kagra.inside_circle(mx, my, SW//2, 500, cr)
            col = (255, 100, 100) if circle_hit else (80, 80, 150)
            # 円を疑似描画（四角で近似）
            kagra.fill(SW//2 - cr, 500 - cr, cr*2, cr*2, col + (60,))
            kagra.text("circle", SW//2 - 25, 492, 13,
                       color=(200,200,220), font=self.font)
        else:
            kagra.text("R: ジオメトリデモ ON", 40, 310, 16,
                       color=(120,120,160), font=self.font)

        # ── 操作ガイド ────────────────────────────────────
        guide = [
            "~ / F1 : コンソール開閉",
            "H      : HTTP テスト（PokeAPI）",
            "R      : ジオメトリデモ",
            "ESC    : 終了",
        ]
        for i, g in enumerate(guide):
            kagra.text(g, SW - 280, SH - 110 + i * 22, 14,
                       color=(100, 100, 140), font=self.font)

        # ── コンソールを最前面に描画（必ず最後）────────────
        self.console.draw()


# lerp 状態（モジュールレベルで保持）
_Phase9DemoScene_lerp_x = [SW // 2]


if __name__ == "__main__":
    kagra.init(SW, SH, "KAGRA Phase 9 Demo", fps=60)
    kagra.run(start_scene=Phase9DemoScene())
