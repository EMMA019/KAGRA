"""バニーガーデン系ミニマルゲーム — ゲームロジックは全部 Python。

1 日 = 営業 → 会話/ドリンク/ほめる → 閉店。好感度で台詞が変わる。
VRM キャラ（Emma）と会話し、好感度を上げて日々を経営する。

- 世界はデータ: 部屋 dump を `kagra.gameloop.draw_world(world, w, h, hud=)`
  で描く（このモジュールは kagra_core に依存しない）
- システムは Python: 会話 / ドリンク在庫 / お金 / 日付 / 決定論 RNG /
  セーブ・ロード / 特別イベント
- 入力: ↑↓ で選択、Z / J / Enter で決定、X で戻る。マウスクリックも可
- 音: 選択・ドリンク・イベントで SE（kagra.audio）

実行は `examples/bunny_garden_minimal.py`（窓 / ヘッドレス verify）。
"""
from __future__ import annotations

import json
from pathlib import Path

from kagra.audio import se  # noqa: F401  (再生は Windows winsound、他は no-op)
from kagra.gameloop import Scene, draw_world, mouse_clicked, mouse_pos, was_pressed
from kagra.ui2d import bar, choice_menu, list_lines, merge, message

W, H = 360, 200
CHAR = "ミミ"
SAVE_DEFAULT = Path.home() / ".kagra" / "bunny_garden.json"

_DRINK_PRICE = 80


class BunnyGarden(Scene):
    """1 日 = 営業 → 会話/ドリンク/ほめる → 閉店。好感度で台詞が変わる。"""

    def __init__(self, save_path: Path | None = None, start_day: int = 1) -> None:
        super().__init__()
        self.width, self.height = W, H
        self.save_path = Path(save_path) if save_path else SAVE_DEFAULT
        self.game = self._load() if (start_day <= 1 and self.save_path.exists()) else self._new_game()
        self._rng = 0xC0FFEE + self.game["day"] * 7919
        self.world = self._build_world()
        self.sel = 0
        self.state = "msg"          # msg | menu | drink | end
        self.message = ""
        self.queue: list[str] = []
        self._choice_rects: list[tuple[float, float, float, float]] = []
        self._push(f"{self.game['day']}日目、営業開始！")
        self._show_next()

    # ── 状態 ──────────────────────────────────────────────────────────────

    def _new_game(self) -> dict:
        return {
            "day": 1,
            "money": 300,
            "stock": {"モヒート": 3, "オレンジジュース": 2},
            "affection": {CHAR: 0},
            "events": [],
        }

    def _load(self) -> dict:
        try:
            return json.loads(self.save_path.read_text(encoding="utf-8"))
        except Exception:
            return self._new_game()

    def _save(self) -> None:
        self.save_path.parent.mkdir(parents=True, exist_ok=True)
        self.save_path.write_text(json.dumps(self.game, ensure_ascii=False, indent=1), encoding="utf-8")

    def _rnd(self, n: int) -> int:
        self._rng = (self._rng * 1103515245 + 12345) & 0x7FFFFFFF
        return self._rng % n

    def _aff(self) -> int:
        return self.game["affection"][CHAR]

    def _set_aff(self, v: int) -> None:
        self.game["affection"][CHAR] = max(0, min(100, v))

    def _push(self, text: str) -> None:
        self.queue.append(text)

    def _show_next(self) -> None:
        if self.queue:
            self.message = self.queue.pop(0)
            self.state = "msg"
        else:
            self.message = ""
            self.state = "menu"
            self.sel = 0

    def _drain(self) -> None:
        """メッセージ待ちを飛ばしてメニューへ（ヘッドレス / verify 用）。"""
        self.queue.clear()
        self.message = ""
        self.state = "menu"

    # ── 会話 ──────────────────────────────────────────────────────────────

    def _tier(self) -> str:
        a = self._aff()
        if a >= 70:
            return "high"
        if a >= 30:
            return "mid"
        return "low"

    def _talk_line(self) -> str:
        return {
            "low": "いらっしゃいませ〜。今日は何にしますか？",
            "mid": "おかえりなさい！待ってたんですよ。",
            "high": "…うふふ。今日は、たくさんお話ししたいな。",
        }[self._tier()]

    def _praise_line(self, delta: int) -> str:
        return {
            "low": "え、ほめてくれたの？ うれしいな…（+%d）" % delta,
            "mid": "もっとほめて！ もっと！ …えへへ（+%d）" % delta,
            "high": "あなたにほめられると、心臓がどきどきする…（+%d）" % delta,
        }[self._tier()]

    def _check_event(self) -> None:
        if self._aff() >= 50 and "special" not in self.game["events"]:
            self.game["events"].append("special")
            se("cast")
            self._push("…あの、ずっと一緒にいても、いいですか？")
            self._push("（特別イベント発生！ 好感度 50 到達）")

    # ── メニュー処理（UI とヘッドレスが同じ道を通る） ─────────────────────

    def _choices(self) -> list[str]:
        return ["話す", "飲み物を出す", "ほめる", "閉店"]

    def _do_choice(self, i: int) -> None:
        se("ok")
        if i == 0:  # 話す
            self._set_aff(self._aff() + 3)
            self._push(self._talk_line())
        elif i == 1:  # 飲み物
            self.state = "drink"
            self.sel = 0
            return
        elif i == 2:  # ほめる
            d = 2 + self._rnd(4)
            self._set_aff(self._aff() + d)
            self._push(self._praise_line(d))
        else:  # 閉店
            self._close_day()
            return
        self._check_event()
        self._show_next()

    def _drink_items(self) -> list[str]:
        items = [f"{n} x{c}" for n, c in self.game["stock"].items() if c > 0]
        items.append("やめる")
        return items

    def _do_drink(self, i: int) -> None:
        items = self._drink_items()
        if i >= len(items) - 1:  # やめる
            self.state = "menu"
            self.sel = 0
            return
        name = list(self.game["stock"])[i]
        if self.game["money"] < _DRINK_PRICE:
            self._push("お金が足りない…（ドリンク %dG）" % _DRINK_PRICE)
            se("hurt")
            self._show_next()
            return
        self.game["money"] -= _DRINK_PRICE
        self.game["stock"][name] -= 1
        self._set_aff(self._aff() + 8)
        se("coin")
        self._push(f"{CHAR}は「{name}」を飲んだ！ ご機嫌になった（+8）")
        self._check_event()
        self._show_next()

    def _close_day(self) -> None:
        income = 100 + (self._aff() // 10) * 20
        self.game["money"] += income
        se("bite")
        self._push(f"閉店。売上 {income}G。DAY {self.game['day']} 終了。")
        self.state = "end"
        self._save()

    def _next_day(self) -> None:
        self.game["day"] += 1
        self._rng = 0xC0FFEE + self.game["day"] * 7919
        self._push(f"{self.game['day']}日目、営業開始！")
        self._show_next()

    # ── 入力 ──────────────────────────────────────────────────────────────

    def _confirm(self) -> bool:
        return was_pressed("z") or was_pressed("j") or was_pressed("return")

    def _nav(self, n: int) -> None:
        if was_pressed("down") or was_pressed("s"):
            self.sel = (self.sel + 1) % n
        elif was_pressed("up") or was_pressed("w"):
            self.sel = (self.sel - 1) % n

    def _clicked_choice(self) -> int | None:
        if not mouse_clicked(1):
            return None
        mx, my = mouse_pos()
        for i, (x, y, w, h) in enumerate(self._choice_rects):
            if x <= mx <= x + w and y <= my <= y + h:
                return i
        return None

    def update(self, dt: float) -> None:
        if self.state == "msg":
            if self._confirm():
                se("ok")
                self._show_next()
            return
        if self.state == "end":
            if self._confirm():
                self._next_day()
            return
        if self.state == "menu":
            items = self._choices()
            self._nav(len(items))
            click = self._clicked_choice()
            if self._confirm() or click is not None:
                self._do_choice(click if click is not None else self.sel)
            return
        if self.state == "drink":
            items = self._drink_items()
            self._nav(len(items))
            click = self._clicked_choice()
            if self._confirm() or click is not None:
                self._do_drink(click if click is not None else self.sel)

    # ── 世界と描画 ────────────────────────────────────────────────────────

    @staticmethod
    def _build_world() -> dict:
        return {
            "version": 1,
            "half": 8.0,
            "floor_y": 0.0,
            "gravity": 9.8,
            "water_y": None,
            "coins": 0,
            "player": None,
            "props": [
                {"id": "prop:floor", "type": "prop", "name": "floor", "position": [0, -0.5, 0],
                 "model": "box", "scale": [16, 1, 12], "enabled": True, "color": [72, 50, 40], "roughness": 0.9},
                {"id": "prop:counter", "type": "prop", "name": "counter", "position": [0, 0.55, 3.4],
                 "model": "box", "scale": [5.0, 1.1, 0.7], "enabled": True, "color": [130, 88, 52], "roughness": 0.6},
                {"id": "prop:lamp", "type": "prop", "name": "lamp", "position": [2.6, 1.3, 1.2],
                 "model": "sphere", "scale": [0.35, 0.35, 0.35], "enabled": True, "color": [255, 214, 140], "metallic": 0.4, "roughness": 0.3},
                {"id": "prop:lamp2", "type": "prop", "name": "lamp2", "position": [-2.6, 1.3, 1.2],
                 "model": "sphere", "scale": [0.35, 0.35, 0.35], "enabled": True, "color": [255, 214, 140], "metallic": 0.4, "roughness": 0.3},
            ],
            "walkers": [
                {"id": "walker:mimi", "type": "walker", "name": CHAR, "position": [0, 0, 0.6],
                 "yaw": 0.0, "face": 0.0, "on_ground": True, "model": "capsule",
                 "gltf": "assets/Emma.vrm", "clip": 0.0, "anim": "idle", "expression": "smile"},
            ],
            "lights": [
                {"id": "light:warm", "type": "light", "name": "warm", "position": [0, 2.6, 1.0],
                 "kind": "point", "slot": 0, "intensity": 3.0, "radius": 6.0, "color": [1.0, 0.85, 0.65]},
            ],
            "cameras": [
                {"id": "camera:main", "type": "camera", "name": "main", "position": [0, 1.7, 5.6],
                 "target": [0, 0.95, 0.0], "fov": 50},
            ],
            "heightfield": None,
        }

    def draw(self) -> None:
        g = self.game
        parts = [
            message(f"DAY {g['day']}    所持金 {g['money']}G", 8, 8, 200, size=11, color=[220, 220, 205, 255]),
            bar(220, 10, 132, 8, ratio=self._aff() / 100.0, label=f"{CHAR} 好感度", color=[255, 150, 170, 255]),
        ]
        stock_str = "  ".join(f"{n}x{c}" for n, c in g["stock"].items() if c > 0) or "なし"
        parts.append(list_lines([f"在庫: {stock_str}"], x=8, y=30, size=9, color=[170, 170, 160, 255]))
        if self.state == "menu":
            items = self._choices()
            m = choice_menu(items, selected=self.sel, x=216, y=96, w=136)
            self._choice_rects = [(q["x"], q["y"], q["w"], q["h"]) for q in m["quads"]]
            parts.append(m)
        elif self.state == "drink":
            parts.append(choice_menu(self._drink_items(), selected=self.sel, x=216, y=96, w=136))
        # メッセージウィンドウは常時（空なら隠れる）
        if self.message:
            parts.append(message(self.message, 8, H - 74, 344, size=13))
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=merge(*parts))
