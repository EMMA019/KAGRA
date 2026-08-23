"""
KAGRA Phase 9 デモ（体感版）
==============================
「これが便利！」が分かる3つのシーンで構成。

[シーン1] hotreload_scene.py を編集して保存 → 即反映
[シーン2] ~ キーでコンソール → 変数をリアルタイム書き換え
[シーン3] ジオメトリヘルパーで当たり判定ゲーム

操作:
  1 2 3   : シーンを切り替え
  ESC     : 終了
"""

import kagra
from kagra.hot_reload import HotReloader
from kagra.console    import DevConsole
from pathlib import Path

SW, SH = 1280, 720

# このファイル（phase9_demo2.py）と同じフォルダを基準にする
_HERE = Path(__file__).parent

# ────────────────────────────────────────────────────────────────
# シーン 1: ホットリロード体験
# ────────────────────────────────────────────────────────────────
class HotReloadScene(kagra.Scene):
    """
    このデモと同じフォルダ（examples/）に
    hotreload_scene.py を置いてください。
    保存するたびにゲームが止まらず即反映されます。
    """

    def on_enter(self):
        self.font     = kagra.assets.font("meiryo")
        # __file__ 基準の絶対パスで解決 → どこから実行しても動く
        watch_path    = str(_HERE / "hotreload_scene.py")
        self.reloader = HotReloader(
            watch_path,
            scene_class = "LiveScene",
            on_reload   = lambda _: print("✓ リロード完了！"),
            on_error    = lambda e: print(f"✗ エラー: {e}"),
        )
        self.reloader.start()

    def on_exit(self):
        # シーン切り替え時に監視を止める
        self.reloader.stop()

    def update(self, dt):
        self.reloader.tick()
        if self.reloader.scene:
            self.reloader.scene.update(dt)
        if kagra.pressed("2"): kagra.go(ConsoleScene())
        if kagra.pressed("3"): kagra.go(GeometryScene())
        if kagra.pressed("ESCAPE"): exit()

    def draw(self):
        kagra.cls(18, 22, 40)

        if self.reloader.scene:
            self.reloader.scene.draw()
        else:
            # hotreload_scene.py がまだない場合の案内
            kagra.fill(SW//2 - 350, SH//2 - 80, 700, 160, (30, 35, 55))
            kagra.text("hotreload_scene.py を作成してください",
                       SW//2 - 280, SH//2 - 50, 22, (180, 200, 255), self.font)
            kagra.text("→ 保存するたびにここが即反映されます！",
                       SW//2 - 250, SH//2 - 10, 18, (120, 160, 200), self.font)
            kagra.text("サンプルは下の hotreload_scene.py を参照",
                       SW//2 - 220, SH//2 + 30, 16, (100, 120, 160), self.font)

        # HUD
        kagra.fill(0, 0, SW, 36, (10, 12, 22, 200))
        kagra.text("[1] ホットリロード", 20, 8, 16, (100, 180, 255), self.font)
        kagra.text("[2] コンソール",    220, 8, 16, (100, 120, 160), self.font)
        kagra.text("[3] ジオメトリ",    380, 8, 16, (100, 120, 160), self.font)
        # リロードインジケーター
        dot = (0, 255, 100) if self.reloader._running else (80, 80, 80)
        kagra.fill(SW - 100, 12, 10, 10, dot)
        kagra.text("監視中" if self.reloader._running else "停止",
                   SW - 82, 8, 14, dot, self.font)


# ────────────────────────────────────────────────────────────────
# シーン 2: コンソール体験
# ────────────────────────────────────────────────────────────────
class ConsoleScene(kagra.Scene):
    """
    ~ キーでコンソールが開きます。
    ゲーム実行中に変数を変えたり、関数を呼んだりできます。
    """

    def on_enter(self):
        self.font    = kagra.assets.font("meiryo")
        self.console = DevConsole(self, font_size=18)

        # コンソールから変えられる変数
        self.player_x   = 200.0
        self.player_y   = SH // 2
        self.player_size = 40
        self.speed      = 200.0
        self.bg_color   = (18, 22, 40)
        self.player_color = (100, 200, 255)
        self.message    = "~ キーでコンソールを開いてください"
        self.score      = 0

        # デモ: コンソールに最初から使い方を表示
        self.console.log("━━━ コンソールの使い方 ━━━", (180, 200, 255))
        self.console.log("scene.speed = 500       # 移動速度を変える", (200, 220, 180))
        self.console.log("scene.player_size = 100 # サイズを変える", (200, 220, 180))
        self.console.log("scene.player_color = (255, 100, 100)  # 色を変える", (200, 220, 180))
        self.console.log("scene.bg_color = (50, 0, 0)  # 背景を変える", (200, 220, 180))
        self.console.log("scene.score = 9999      # スコアを変える", (200, 220, 180))
        self.console.log("scene.message = 'hello'  # メッセージを変える", (200, 220, 180))

    def update(self, dt):
        self.console.update(dt)

        # プレイヤー移動（コンソールで speed を変えると即反映）
        if not self.console.enabled:
            if kagra.key("LEFT"):  self.player_x -= self.speed * dt
            if kagra.key("RIGHT"): self.player_x += self.speed * dt
            if kagra.key("UP"):    self.player_y -= self.speed * dt
            if kagra.key("DOWN"):  self.player_y += self.speed * dt
            self.player_x = kagra.clamp(self.player_x, 0, SW - self.player_size)
            self.player_y = kagra.clamp(self.player_y, 36, SH - self.player_size)

        if kagra.pressed("1"): kagra.go(HotReloadScene())
        if kagra.pressed("3"): kagra.go(GeometryScene())
        if kagra.pressed("ESCAPE"): exit()

    def draw(self):
        r, g, b = self.bg_color[0], self.bg_color[1], self.bg_color[2]
        kagra.cls(r, g, b)

        # プレイヤー（コンソールで色・サイズを変えると即反映）
        pr, pg, pb = (self.player_color + (255,))[:3]
        s = self.player_size
        kagra.fill(self.player_x, self.player_y, s, s, (pr, pg, pb))

        # スコア
        kagra.text(f"SCORE: {self.score}", 20, 60, 36,
                   (255, 220, 100), self.font)

        # メッセージ
        kagra.text(self.message, 20, 120, 20, (180, 180, 220), self.font)

        # 操作ガイド（コンソールが閉じている時）
        if not self.console.enabled:
            kagra.text("← → ↑ ↓ : 移動    ~ : コンソールを開く",
                       20, SH - 36, 16, (80, 100, 130), self.font)

        # HUD
        kagra.fill(0, 0, SW, 36, (10, 12, 22, 200))
        kagra.text("[1] ホットリロード", 20, 8, 16, (100, 120, 160), self.font)
        kagra.text("[2] コンソール",    220, 8, 16, (100, 180, 255), self.font)
        kagra.text("[3] ジオメトリ",    380, 8, 16, (100, 120, 160), self.font)
        kagra.text("~ : コンソール", SW - 160, 8, 16, (100, 180, 100), self.font)

        # コンソールは必ず最後に描画
        self.console.draw()


# ────────────────────────────────────────────────────────────────
# シーン 3: ジオメトリヘルパー体験
# ────────────────────────────────────────────────────────────────
import math, random

class Enemy:
    def __init__(self):
        self.x = random.randint(100, SW - 100)
        self.y = random.randint(80,  SH - 80)
        self.w = random.randint(40, 80)
        self.h = self.w
        self.vx = random.choice([-1, 1]) * random.uniform(60, 140)
        self.vy = random.choice([-1, 1]) * random.uniform(60, 140)
        self.alive = True

class GeometryScene(kagra.Scene):
    """
    マウスで敵を「当てる」ゲーム。
    kagra.intersect_rect / distance / lerp を使っています。
    """

    def on_enter(self):
        self.font    = kagra.assets.font("meiryo")
        self.console = DevConsole(self, font_size=16)
        self.enemies = [Enemy() for _ in range(8)]
        self.score   = 0
        self.t       = 0.0
        # マウスカーソルの追従（lerp 体験）
        self.cursor_x = SW // 2.0
        self.cursor_y = SH // 2.0
        self.killed_text = []  # ヒットエフェクト用

    def update(self, dt):
        self.console.update(dt)
        self.t += dt

        mx, my = kagra.mouse()

        # cursor が lerp でマウスを追いかける（これが lerp の体験）
        self.cursor_x = kagra.lerp(self.cursor_x, mx, min(1.0, dt * 12))
        self.cursor_y = kagra.lerp(self.cursor_y, my, min(1.0, dt * 12))

        # 敵の移動
        for e in self.enemies:
            if not e.alive: continue
            e.x += e.vx * dt
            e.y += e.vy * dt
            if e.x < 0 or e.x + e.w > SW: e.vx *= -1
            if e.y < 36 or e.y + e.h > SH: e.vy *= -1

        # クリックで当たり判定
        if kagra.mouse_click(1):
            for e in self.enemies:
                if not e.alive: continue
                # intersect_rect: カーソル vs 敵
                if kagra.intersect_rect(
                    self.cursor_x - 12, self.cursor_y - 12, 24, 24,
                    e.x, e.y, e.w, e.h
                ):
                    e.alive = False
                    self.score += 10
                    self.killed_text.append({
                        "x": e.x + e.w//2, "y": e.y,
                        "t": 1.0, "score": 10
                    })

        # 全滅したら復活
        if all(not e.alive for e in self.enemies):
            self.enemies = [Enemy() for _ in range(8)]

        # テキストフェードアウト
        self.killed_text = [
            {**k, "y": k["y"] - 40 * dt, "t": k["t"] - dt}
            for k in self.killed_text if k["t"] > 0
        ]

        if kagra.pressed("1"): kagra.go(HotReloadScene())
        if kagra.pressed("2"): kagra.go(ConsoleScene())
        if kagra.pressed("ESCAPE"): exit()

    def draw(self):
        kagra.cls(18, 22, 40)

        mx, my = kagra.mouse()

        # 敵
        for e in self.enemies:
            if not e.alive: continue
            # マウスと敵の距離を色に反映（distance 体験）
            d = kagra.distance(mx, my, e.x + e.w//2, e.y + e.h//2)
            danger = max(0.0, 1.0 - d / 200.0)
            r = int(80 + danger * 175)
            g = int(80 - danger * 60)
            b = int(200 - danger * 180)
            kagra.fill(e.x, e.y, e.w, e.h, (r, g, b))

            # hover 判定表示（inside_rect 体験）
            if kagra.inside_rect(mx, my, e.x, e.y, e.w, e.h):
                kagra.fill(e.x - 3, e.y - 3, e.w + 6, e.h + 6, (255, 255, 100, 40))

        # カーソル（lerp で遅れて追いかける）
        cx, cy = int(self.cursor_x), int(self.cursor_y)
        kagra.fill(cx - 14, cy - 14, 28, 28, (255, 220, 80, 180))
        kagra.fill(cx - 2,  cy - 14, 4,  28,  (255, 255, 255))
        kagra.fill(cx - 14, cy - 2,  28,  4,  (255, 255, 255))

        # 実マウス位置との線（lerp 遅延を視覚化）
        # 距離が離れているほど lerp が追いかけていることがわかる
        d = kagra.distance(mx, my, cx, cy)
        if d > 5:
            kagra.fill(cx, cy, int(mx - cx) if mx > cx else 0, 2,
                       (255, 220, 80, 80))

        # スコアポップアップ
        for k in self.killed_text:
            a = int(k["t"] * 255)
            kagra.text(f"+{k['score']}", k["x"], int(k["y"]), 24,
                       (255, 255, 100, a), self.font)

        # スコア & ガイド
        kagra.text(f"SCORE: {self.score}", 20, 50, 32,
                   (255, 220, 100), self.font)
        kagra.text("クリックで敵を当てる（黄色い十字が lerp 追従カーソル）",
                   20, SH - 60, 16, (140, 160, 200), self.font)
        kagra.text("敵がマウスに近づくほど赤くなる = distance() 使用",
                   20, SH - 36, 16, (140, 160, 200), self.font)

        # HUD
        kagra.fill(0, 0, SW, 36, (10, 12, 22, 200))
        kagra.text("[1] ホットリロード", 20, 8, 16, (100, 120, 160), self.font)
        kagra.text("[2] コンソール",    220, 8, 16, (100, 120, 160), self.font)
        kagra.text("[3] ジオメトリ",    380, 8, 16, (100, 180, 255), self.font)

        self.console.draw()


# ────────────────────────────────────────────────────────────────
# 起動
# ────────────────────────────────────────────────────────────────
if __name__ == "__main__":
    kagra.init(SW, SH, "KAGRA Phase 9 Demo", fps=60)
    # まずコンソールデモから開始（~ で開閉できることを確認してから [1] でホットリロードへ）
    kagra.run(start_scene=ConsoleScene())
