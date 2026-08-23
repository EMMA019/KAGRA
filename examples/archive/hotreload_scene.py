"""
hotreload_scene.py
==================
このファイルを保存するたびに phase9_demo2.py のシーン1に即反映されます。
ゲームを再起動しなくていい！

試してみてください:
  - bg を変える
  - テキストを変える
  - 図形を追加する
  - speed を変える
"""

import kagra
import math

class LiveScene:
    """ホットリロードで差し替えられるシーン。"""

    def __init__(self):
        # ここを変えると即反映 ↓
        self.bg         = (200, 100, 100)
        self.msg        = "このファイルを編集して保存！"
        self.box_color  = (250, 200, 100)
        self.speed      = 1500.0
        self.t          = 0.0
        self.x          = 200.0

    def update(self, dt):
        self.t += dt
        # 左右に往復する箱
        self.x = 200 + math.sin(self.t * self.speed / 100) * 300

    def draw(self):
        # 背景
        r, g, b = self.bg
        kagra.cls(r, g, b)

        # 動く箱
        br, bg_, bb = self.box_color
        kagra.fill(self.x - 30, 300, 60, 60, (br, bg_, bb))

        # メッセージ
        kagra.text(self.msg, 100, 200, 28, (220, 220, 255))

        # ヒント
        kagra.text("↑ この行を変えて保存してみてください", 100, 250, 16, (120, 140, 180))


# ─── 変えてみる例 ─────────────────────────────────────────────
# 下のコメントを外して保存するとどうなるか試してみてください:
#
# class LiveScene:
#     def __init__(self):
#         self.t = 0.0
#
#     def update(self, dt):
#         self.t += dt
#
#     def draw(self):
#         kagra.cls(40, 10, 40)
#         # 虹色の波
#         for i in range(20):
#             import math
#             x = i * 65
#             y = 400 + math.sin(self.t * 3 + i * 0.5) * 100
#             r = int(127 + 127 * math.sin(self.t + i * 0.3))
#             g = int(127 + 127 * math.sin(self.t + i * 0.3 + 2))
#             b = int(127 + 127 * math.sin(self.t + i * 0.3 + 4))
#             kagra.fill(x, y, 50, 50, (r, g, b))
#         kagra.text("虹色の波！", 500, 250, 48, (255, 255, 200))
