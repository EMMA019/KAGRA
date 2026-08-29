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
import math
from pathlib import Path

from kagra.audio import play_wav, se  # noqa: F401  (再生は Windows winsound、他は no-op)
from kagra.gameloop import Scene, draw_world, mouse_clicked, mouse_pos, was_pressed
from kagra.tts import VOWEL_TO_EXPRESSION, tts_ping, tts_speak  # noqa: F401
from kagra.ui2d import bar, choice_menu, list_lines, merge, message

W, H = 480, 300
CHAR = "ミミ"
SAVE_DEFAULT = Path.home() / ".kagra" / "bunny_garden.json"

_DRINK_PRICE = 80


class BunnyGarden(Scene):
    """1 日 = 営業 → 会話/ドリンク/ほめる → 閉店。好感度で台詞が変わる。"""

    def __init__(self, save_path: Path | None = None, start_day: int = 1) -> None:
        super().__init__()
        self.width, self.height = W, H
        self.save_path = Path(save_path) if save_path else SAVE_DEFAULT
        saved = self._load() if (start_day <= 1 and self.save_path.exists()) else None
        self.game = saved if saved is not None else self._new_game()
        self._rng = 0xC0FFEE + self.game["day"] * 7919
        # TTS リップシンク（VOICEVOX があれば）。(母音タイミング, 開始clock)
        self._lips: list[tuple[str, float, float]] | None = None
        self._lips_t0 = 0.0
        self.world = self._build_world()
        self.sel = 0
        self.state = "msg"          # msg | menu | drink | end
        self.message = ""
        self.queue: list[str] = []
        self._choice_rects: list[tuple[float, float, float, float]] = []
        if saved is not None:
            self._push(f"セーブデータから再開（DAY {self.game['day']}）")
        self._push(f"{self.game['day']}日目、営業開始！  ↑↓ で選んで Z で決定")
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
            self._speak(self.message)
        else:
            self.message = ""
            self.state = "menu"
            self.sel = 0
            self._lips = None

    def _speak(self, text: str) -> None:
        """VOICEVOX があれば音声 + リップタイミングを仕込む（無ければ黙って表示）。"""
        self._lips = None
        try:
            if not tts_ping():
                return
            wav, moras = tts_speak(text)
            if not moras:
                return
            play_wav(wav)
            self._lips = moras
            self._lips_t0 = self.clock
        except Exception:
            self._lips = None

    def _lipsync_expression(self) -> str | None:
        """現在のモーラ窓に対応する VRM 表情（口の形）。"""
        if not self._lips:
            return None
        t = self.clock - self._lips_t0
        for vowel, t0, t1 in self._lips:
            if t0 <= t < t1:
                return VOWEL_TO_EXPRESSION.get(vowel)
        return None

    def _gesture_overlay(self) -> dict:
        """TTS 発話中: 上半身の腕ジェスチャー（overlay_bones、ローカル回転）。

        歩きクリップ（anim_blend=1.0）の上に腕が重なり、無言なら空 → 何も
        動かない。ノード名は VRM humanoid 名（Emma = left/rightUpperArm）。
        """
        if not self._lips:
            return {}
        # ゆっくり両腕を外側へ揺らす（挨拶 / 説明の身振り）。
        a = 0.3 * math.sin(self.clock * 2.4)
        half = a * 0.5
        s, c = math.sin(half), math.cos(half)
        return {
            "leftUpperArm": [0.0, 0.0, s, c],
            "rightUpperArm": [0.0, 0.0, -s, c],
        }

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
        return (
            was_pressed("z")
            or was_pressed("j")
            or was_pressed("return")
            or was_pressed("space")
            or mouse_clicked(1)  # クリックでも進められる
        )

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

    def _any_key(self) -> bool:
        """どのキーでもメッセージを送れる（RPG 定番。キーが届くかを気にしない）。"""
        return any(
            was_pressed(k)
            for k in ("up", "down", "left", "right", "z", "j", "x", "return", "space", "w", "a", "s", "d")
        )

    def on_close(self) -> None:
        """窓を閉じたらセーブして終了（ESC も同じ）。"""
        self._save()
        self.quit()

    def update(self, dt: float) -> None:
        if was_pressed("escape"):
            self.on_close()
            return
        if self.state == "msg":
            if self._confirm() or self._any_key():
                se("ok")
                self._show_next()
            return
        if self.state == "end":
            if self._confirm() or self._any_key():
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

    def _build_world(self) -> dict:
        return {
            "version": 1,
            "half": 8.0,
            "floor_y": 0.0,
            "gravity": 9.8,
            "water_y": None,
            "coins": 0,
            "player": None,
            "props": [
                # 床とカーペット
                {"id": "prop:floor", "type": "prop", "name": "floor", "position": [0, -0.5, 0],
                 "model": "box", "scale": [16, 1, 12], "enabled": True, "color": [58, 40, 30], "roughness": 0.95},
                {"id": "prop:rug", "type": "prop", "name": "rug", "position": [0, -0.08, 0.2],
                 "model": "box", "scale": [4.4, 0.12, 3.2], "enabled": True, "color": [110, 42, 52], "roughness": 0.9},
                # 背面の壁と棚
                {"id": "prop:backwall", "type": "prop", "name": "backwall", "position": [0, 1.4, -4.4],
                 "model": "box", "scale": [15, 3.0, 0.5], "enabled": True, "color": [66, 38, 34], "roughness": 0.9},
                {"id": "prop:shelf", "type": "prop", "name": "shelf", "position": [0, 1.9, -4.1],
                 "model": "box", "scale": [6.0, 0.16, 0.6], "enabled": True, "color": [96, 62, 40], "roughness": 0.7},
                # 棚のボトル
                *[
                    {"id": f"prop:bottle{i}", "type": "prop", "name": "bottle", "position": [x, 2.12, -4.1],
                     "model": "cylinder", "scale": [0.14, 0.4, 0.14], "enabled": True,
                     "color": c, "metallic": 0.1, "roughness": 0.3}
                    for i, (x, c) in enumerate([(-2.2, [150, 60, 70]), (0.0, [60, 150, 90]), (2.2, [230, 190, 80])])
                ],
                # カウンター（手前）と客席スツール
                {"id": "prop:counter", "type": "prop", "name": "counter", "position": [0, 0.45, 3.4],
                 "model": "box", "scale": [5.0, 0.9, 0.7], "enabled": True, "color": [110, 74, 44], "roughness": 0.6},
                {"id": "prop:stool1", "type": "prop", "name": "stool", "position": [1.9, 0.14, 2.5],
                 "model": "box", "scale": [0.5, 0.28, 0.5], "enabled": True, "color": [80, 56, 40], "roughness": 0.7},
                {"id": "prop:stool2", "type": "prop", "name": "stool", "position": [-1.9, 0.14, 2.5],
                 "model": "box", "scale": [0.5, 0.28, 0.5], "enabled": True, "color": [80, 56, 40], "roughness": 0.7},
                # ランプ
                {"id": "prop:lamp", "type": "prop", "name": "lamp", "position": [2.6, 1.5, 0.6],
                 "model": "sphere", "scale": [0.4, 0.4, 0.4], "enabled": True, "color": [255, 214, 140], "metallic": 0.4, "roughness": 0.3},
                {"id": "prop:lamp2", "type": "prop", "name": "lamp2", "position": [-2.6, 1.5, 0.6],
                 "model": "sphere", "scale": [0.4, 0.4, 0.4], "enabled": True, "color": [255, 214, 140], "metallic": 0.4, "roughness": 0.3},
            ],
            "walkers": [
                {"id": "walker:mimi", "type": "walker", "name": CHAR,
                 # 体の上下動（呼吸っぽい）で静止感を消す
                 "position": [0, 0.07 * math.sin(self.clock * 3.0), 0.9],
                 "yaw": 0.0, "face": 0.0, "on_ground": True, "model": "capsule",
                 "gltf": "assets/Emma.vrm",
                 # clip を進めると歩行サイクルがその場でループ（常時動く）。
                 # rem_euclid で折り返すので長くても OK。
                 "clip": (self.clock * 1.6) % 10.0,
                 "anim": "walk",
                 # フルクリップ（ロコモーションブレンドは 1 で固定）
                 "anim_blend": 1.0,
                 # 頭を左右に見渡す（カメラ方向を中心に）
                 "look_yaw": 0.14 * math.sin(self.clock * 1.2),
                 "look_pitch": 0.06,
                 # 好感度が高いほど表情が明るくなる。TTS リップ中は口の形
                 "expression": self._lipsync_expression()
                 or ("joy" if self._aff() >= 70 else "smile"),
                 # TTS 発話中は腕ジェスチャー（上半身のみ、歩きクリップに乗る）
                 "overlay_bones": self._gesture_overlay(),
                 "overlay_weight": 0.5},
            ],
            "lights": [
                {"id": "light:warm", "type": "light", "name": "warm", "position": [0, 2.8, 0.8],
                 "kind": "point", "slot": 0, "intensity": 3.6, "radius": 7.0, "color": [1.0, 0.82, 0.62]},
                {"id": "light:cool", "type": "light", "name": "cool", "position": [0, 2.4, -3.0],
                 "kind": "point", "slot": 1, "intensity": 1.2, "radius": 6.0, "color": [0.6, 0.75, 1.0]},
            ],
            "cameras": [
                {"id": "camera:main", "type": "camera", "name": "main", "position": [0, 1.7, 5.6],
                 "target": [0, 1.05, 0.5], "fov": 42},
            ],
            "heightfield": None,
            # そよ風: SpringBone の布（スカート・髪）を揺らす
            "wind": [0.25, 0.0, 0.15],
            "ibl": 0.5,
        }

    def draw(self) -> None:
        # キャラが clip アニメするので毎フレーム世界を再構築（10ms 程度）
        self.world = self._build_world()
        g = self.game
        parts = [
            message(f"DAY {g['day']}    所持金 {g['money']}G", 10, 10, 260, size=14, color=[240, 236, 220, 255]),
            bar(280, 12, 190, 9, ratio=self._aff() / 100.0, label=f"{CHAR} 好感度", color=[255, 150, 170, 255]),
            list_lines(
                [f"在庫: {('  '.join(f'{n}x{c}' for n, c in g['stock'].items() if c > 0) or 'なし')}",
                 "↑↓ 選択 / Z 決定 / X 戻る / クリック可 / ESC セーブ終了"],
                x=10, y=42, size=10, color=[180, 180, 168, 255],
            ),
        ]
        if self.state == "menu":
            items = self._choices()
            m = choice_menu(items, selected=self.sel, x=300, y=120, w=170, size=15)
            self._choice_rects = [(q["x"], q["y"], q["w"], q["h"]) for q in m["quads"]]
            parts.append(m)
        elif self.state == "drink":
            parts.append(choice_menu(self._drink_items(), selected=self.sel, x=300, y=120, w=170, size=15))
        # メッセージウィンドウは常時（空なら隠れる）
        if self.message:
            parts.append(message(self.message, 10, H - 92, 460, size=16))
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=merge(*parts))
