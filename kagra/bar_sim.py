"""Bar 経営シミュレーション — ゲームロジックは全部 Python。

1 日 = 仕込み → 営業（客が来る）→ 閉店帳簿。決定論 RNG + SlotStore セーブ。
世界は free dump（genre なし）。名前マジックでジャンルが起動しない。
"""
from __future__ import annotations

from pathlib import Path
from typing import Any

from kagra.audio import se
from kagra.gameloop import Scene, draw_world, mouse_clicked, mouse_pos, was_pressed
from kagra.i18n import add_table, t as _t
from kagra.save import load_data, save_data
from kagra.ui2d import bar, choice_menu, image, list_lines, merge, message, panel

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets" / "bar"
DUMP = ROOT / "kagra-shared" / "tests" / "fixtures" / "bar_room_world.json"

add_table("ja", {
    "hud.day": "DAY {day}    {money}G    評判 {rep}",
    "hud.help": "↑↓ 選択 / Z 決定 / X 戻る / ESC セーブ終了",
    "menu.prep": "仕入れ",
    "menu.open": "営業開始",
    "menu.ledger": "帳簿を見る",
    "menu.quit": "閉店してセーブ",
    "prep.whiskey": "ウイスキー仕入れ (40G)",
    "prep.gin": "ジン仕入れ (30G)",
    "prep.wine": "ワイン仕入れ (35G)",
    "prep.back": "戻る",
    "serve.make": "{drink}を出す",
    "serve.talk": "世間話をする",
    "serve.refuse": "お断りする",
    "drink.old_fashioned": "オールドファッション",
    "drink.highball": "ハイボール",
    "drink.gin_tonic": "ジントニック",
    "drink.wine": "赤ワイン",
    "msg.welcome": "「{name}」{bar}へようこそ。今日も一杯、始めよう。",
    "msg.guest": "{name}が席に着いた。「{line}」",
    "msg.served": "{name}は{drink}を飲んだ。満足 +{delta}",
    "msg.talk": "{name}「{line}」 常連度 +{delta}",
    "msg.refuse": "{name}は肩をすくめて店を出た。",
    "msg.stock": "在庫が足りない。",
    "msg.close": "閉店。売上 {sales}G / 家賃 {rent}G。残金 {money}G。",
    "msg.broke": "資金が尽きた。店を畳むしかない…",
    "msg.win": "30日持った。この店は町の夜になった。",
    "msg.buy": "{item}を仕入れた。在庫 {stock}。",
    "msg.nomoney": "仕入れ資金が足りない。",
    "char.ren": "レン",
    "char.sora": "ソラ",
    "char.kai": "カイ",
    "char.mio": "ミオ",
    "char.yuki": "ユキ",
    "char.haru": "ハル",
})
add_table("en", {
    "hud.day": "DAY {day}    {money}G    Rep {rep}",
    "hud.help": "Up/Down choose / Z OK / X back / ESC save & quit",
    "menu.prep": "Stock up",
    "menu.open": "Open the bar",
    "menu.ledger": "Ledger",
    "menu.quit": "Close & save",
    "prep.whiskey": "Buy whiskey (40G)",
    "prep.gin": "Buy gin (30G)",
    "prep.wine": "Buy wine (35G)",
    "prep.back": "Back",
    "serve.make": "Serve {drink}",
    "serve.talk": "Small talk",
    "serve.refuse": "Turn them away",
    "drink.old_fashioned": "Old Fashioned",
    "drink.highball": "Highball",
    "drink.gin_tonic": "Gin Tonic",
    "drink.wine": "Red wine",
    "msg.welcome": "Welcome to {bar}. Another night, another pour.",
    "msg.guest": "{name} sits down. \"{line}\"",
    "msg.served": "{name} drinks {drink}. Satisfaction +{delta}",
    "msg.talk": "{name}: \"{line}\" Regular +{delta}",
    "msg.refuse": "{name} shrugs and leaves.",
    "msg.stock": "Not enough stock.",
    "msg.close": "Closed. Sales {sales}G / rent {rent}G. Cash {money}G.",
    "msg.broke": "Out of money. The lights go dark.",
    "msg.win": "30 nights in. The bar became the town's night.",
    "msg.buy": "Bought {item}. Stock {stock}.",
    "msg.nomoney": "Not enough cash to buy in.",
    "char.ren": "Ren",
    "char.sora": "Sora",
    "char.kai": "Kai",
    "char.mio": "Mio",
    "char.yuki": "Yuki",
    "char.haru": "Haru",
})

W, H = 480, 300
BAR_NAME = "Lumen"
SAVE_VERSION = 1
RENT = 80
WIN_DAYS = 30
START_MONEY = 220
STOCK_COST = {"whiskey": 40, "gin": 30, "wine": 35}

DRINKS = {
    "old_fashioned": {"stock": "whiskey", "price": 120, "cost": 40, "delta": 8},
    "highball": {"stock": "whiskey", "price": 80, "cost": 25, "delta": 5},
    "gin_tonic": {"stock": "gin", "price": 70, "cost": 20, "delta": 6},
    "wine": {"stock": "wine", "price": 90, "cost": 30, "delta": 7},
}

GUESTS = [
    {"id": "ren", "likes": "old_fashioned", "budget": 200, "line": "今日は長くてさ。"},
    {"id": "sora", "likes": "gin_tonic", "budget": 140, "line": "雨の音、好きなんだ。"},
    {"id": "kai", "likes": "highball", "budget": 160, "line": "仕事の話はなしで。"},
    {"id": "mio", "likes": "wine", "budget": 180, "line": "赤、少しだけ。"},
    {"id": "yuki", "likes": "highball", "budget": 120, "line": "一人でいたい夜。"},
    {"id": "haru", "likes": "old_fashioned", "budget": 220, "line": "マスター、いつもの。"},
]


def default_game() -> dict[str, Any]:
    return {
        "version": SAVE_VERSION,
        "day": 1,
        "money": START_MONEY,
        "rep": 20,
        "stock": {"whiskey": 2, "gin": 2, "wine": 1},
        "regulars": {g["id"]: 0 for g in GUESTS},
        "sales_today": 0,
        "ledger": [],
        "lost": False,
        "won": False,
    }


def migrate(data: dict[str, Any]) -> dict[str, Any]:
    base = default_game()
    base.update(data or {})
    base["stock"] = {**default_game()["stock"], **(data.get("stock") or {})}
    base["regulars"] = {**default_game()["regulars"], **(data.get("regulars") or {})}
    base["version"] = SAVE_VERSION
    return base


class Rng:
    """決定論 LCG。同じ seed → 同じ客順。"""

    def __init__(self, seed: int):
        self.n = seed & 0xFFFFFFFF

    def next(self) -> int:
        self.n = (1664525 * self.n + 1013904223) & 0xFFFFFFFF
        return self.n

    def pick(self, items: list) -> Any:
        return items[self.next() % len(items)]

    def guests_for_day(self, day: int) -> list[dict[str, Any]]:
        n = 2 + (self.next() % 3)
        out = []
        for _ in range(n):
            out.append(self.pick(GUESTS))
        return out


class BarSim(Scene):
    def __init__(self, *, save_path: Path | None = None, seed: int = 1):
        super().__init__()
        self.width = W
        self.height = H
        self.save_path = save_path or (Path.home() / ".kagra" / "bar_sim.json")
        self.seed = int(seed)
        loaded = load_data(self.save_path, default=None)
        self.game = migrate(loaded) if loaded else default_game()
        self.rng = Rng(self.seed + self.game["day"] * 7919)
        self.state = "msg"
        self.queue: list[str] = []
        self.sel = 0
        self._choice_rects: list[tuple[float, float, float, float]] = []
        self.guest: dict[str, Any] | None = None
        self.tonight: list[dict[str, Any]] = []
        self.night_open = False
        self.world = _room_world()
        self._push(_t("msg.welcome", name=_t("char.haru"), bar=BAR_NAME))
        self._show_next()

    def _push(self, text: str) -> None:
        self.queue.append(text)

    def _show_next(self) -> None:
        if self.queue:
            self.message = self.queue.pop(0)
            self.state = "msg"
            return
        self.message = ""
        if self.game["lost"] or self.game["won"]:
            self.state = "end"
            return
        if self.guest is not None:
            self.state = "serve"
            self.sel = 0
            return
        self.state = "menu"
        self.sel = 0

    def _drain(self) -> None:
        while self.queue:
            self.message = self.queue.pop(0)
        self.message = ""

    def _save(self) -> None:
        save_data(self.save_path, self.game)

    def on_close(self) -> None:
        self._save()
        self.quit()

    def _confirm(self) -> bool:
        return any(was_pressed(k) for k in ("z", "j", "return", "space")) or mouse_clicked(1)

    def _nav(self, n: int) -> None:
        if n <= 0:
            return
        if was_pressed("down") or was_pressed("s"):
            self.sel = (self.sel + 1) % n
        elif was_pressed("up") or was_pressed("w"):
            self.sel = (self.sel - 1) % n

    def _clicked(self) -> int | None:
        if not mouse_clicked(1):
            return None
        mx, my = mouse_pos()
        for i, (x, y, w, h) in enumerate(self._choice_rects):
            if x <= mx <= x + w and y <= my <= y + h:
                return i
        return None

    def _choices_menu(self) -> list[str]:
        return [_t("menu.prep"), _t("menu.open"), _t("menu.ledger"), _t("menu.quit")]

    def _choices_prep(self) -> list[str]:
        return [_t("prep.whiskey"), _t("prep.gin"), _t("prep.wine"), _t("prep.back")]

    def _choices_serve(self) -> list[str]:
        g = self.guest or GUESTS[0]
        drink = _t(f"drink.{g['likes']}")
        return [
            _t("serve.make", drink=drink),
            _t("serve.talk"),
            _t("serve.refuse"),
        ]

    def _do_menu(self, i: int) -> None:
        if i == 0:
            self.state = "prep"
            self.sel = 0
            return
        if i == 1:
            self._open_night()
            return
        if i == 2:
            lines = self.game["ledger"][-6:] or ["—"]
            self._push(" / ".join(str(x) for x in lines))
            self._show_next()
            return
        self.on_close()

    def _do_prep(self, i: int) -> None:
        keys = ["whiskey", "gin", "wine"]
        if i >= 3:
            self.state = "menu"
            self.sel = 0
            return
        key = keys[i]
        cost = STOCK_COST[key]
        if self.game["money"] < cost:
            self._push(_t("msg.nomoney"))
            self._show_next()
            return
        self.game["money"] -= cost
        self.game["stock"][key] = self.game["stock"].get(key, 0) + 2
        se("ok")
        self._push(_t("msg.buy", item=key, stock=self.game["stock"][key]))
        self._show_next()

    def _open_night(self) -> None:
        self.rng = Rng(self.seed + self.game["day"] * 7919)
        self.tonight = self.rng.guests_for_day(self.game["day"])
        self.game["sales_today"] = 0
        self.night_open = True
        self._next_guest()

    def _next_guest(self) -> None:
        if not self.tonight:
            if self.night_open:
                self.night_open = False
                self._close_day()
            else:
                self.guest = None
                self._show_next()
            return
        self.guest = dict(self.tonight.pop(0))
        name = _t(f"char.{self.guest['id']}")
        se("coin")
        self._push(_t("msg.guest", name=name, line=self.guest["line"]))
        self._show_next()

    def _do_serve(self, i: int) -> None:
        g = self.guest
        if g is None:
            self._next_guest()
            return
        name = _t(f"char.{g['id']}")
        if i == 2:
            self.game["rep"] = max(0, self.game["rep"] - 2)
            self.guest = None
            self._push(_t("msg.refuse", name=name))
            self._next_guest()
            return
        if i == 1:
            delta = 1 + self.game["regulars"].get(g["id"], 0) // 3
            self.game["regulars"][g["id"]] = self.game["regulars"].get(g["id"], 0) + 1
            self.game["rep"] = min(100, self.game["rep"] + 1)
            self.guest = None
            self._push(_t("msg.talk", name=name, line=g["line"], delta=delta))
            self._next_guest()
            return
        spec = DRINKS[g["likes"]]
        stock_key = spec["stock"]
        if self.game["stock"].get(stock_key, 0) <= 0:
            self._push(_t("msg.stock"))
            self._show_next()
            return
        self.game["stock"][stock_key] -= 1
        self.game["money"] += spec["price"]
        self.game["sales_today"] += spec["price"]
        self.game["rep"] = min(100, self.game["rep"] + spec["delta"] // 2)
        self.game["regulars"][g["id"]] = self.game["regulars"].get(g["id"], 0) + 1
        se("bite")
        self.guest = None
        self._push(_t("msg.served", name=name, drink=_t(f"drink.{g['likes']}"), delta=spec["delta"]))
        self._next_guest()

    def _close_day(self) -> None:
        sales = self.game["sales_today"]
        self.game["money"] -= RENT
        self.game["ledger"].append(
            f"D{self.game['day']} +{sales}/-{RENT}={self.game['money']}"
        )
        self._push(_t("msg.close", sales=sales, rent=RENT, money=self.game["money"]))
        if self.game["money"] < 0:
            self.game["lost"] = True
            self._push(_t("msg.broke"))
        elif self.game["day"] >= WIN_DAYS:
            self.game["won"] = True
            self._push(_t("msg.win"))
        else:
            self.game["day"] += 1
        self._save()
        self._show_next()

    def _next_day(self) -> None:
        if self.game["lost"] or self.game["won"]:
            return
        self.rng = Rng(self.seed + self.game["day"] * 7919)
        self._push(_t("msg.welcome", name=_t("char.haru"), bar=BAR_NAME))
        self._show_next()

    def update(self, dt: float) -> None:
        if was_pressed("escape"):
            self.on_close()
            return
        if was_pressed("x") and self.state in {"prep", "serve"}:
            self.state = "menu"
            self.sel = 0
            return
        if self.state == "msg":
            if self._confirm() or any(was_pressed(k) for k in ("up", "down", "w", "s")):
                se("ok")
                self._show_next()
            return
        if self.state == "end":
            if self._confirm():
                self.on_close()
            return
        items = {
            "menu": self._choices_menu,
            "prep": self._choices_prep,
            "serve": self._choices_serve,
        }.get(self.state)
        if not items:
            return
        opts = items()
        self._nav(len(opts))
        click = self._clicked()
        if self._confirm() or click is not None:
            i = click if click is not None else self.sel
            if self.state == "menu":
                self._do_menu(i)
            elif self.state == "prep":
                self._do_prep(i)
            elif self.state == "serve":
                self._do_serve(i)

    def draw(self) -> None:
        g = self.game
        stock = "  ".join(f"{k[0]}:{v}" for k, v in g["stock"].items())
        parts = [
            panel(8, 8, 464, 36, color=(18, 14, 16, 220), border=(160, 110, 70, 255)),
            message(_t("hud.day", day=g["day"], money=g["money"], rep=g["rep"]), 14, 12, 300, size=14),
            bar(320, 16, 140, 10, ratio=g["rep"] / 100.0, label=stock, color=[220, 160, 80, 255]),
            list_lines([_t("hud.help")], x=10, y=48, size=10, color=[170, 160, 150, 255]),
        ]
        portrait = ASSETS / "tex" / f"{(self.guest or {}).get('id', 'ren')}.png"
        if portrait.is_file() and self.guest is not None:
            parts.append(image(str(portrait), 16, 70, 72, 90))
        logo = ASSETS / "tex" / "logo.png"
        if logo.is_file():
            parts.append(image(str(logo), 400, 70, 64, 28))
        if self.state == "menu":
            m = choice_menu(self._choices_menu(), selected=self.sel, x=280, y=120, w=180, size=15)
            self._choice_rects = [(q["x"], q["y"], q["w"], q["h"]) for q in m["quads"]]
            parts.append(m)
        elif self.state == "prep":
            m = choice_menu(self._choices_prep(), selected=self.sel, x=260, y=120, w=200, size=14)
            self._choice_rects = [(q["x"], q["y"], q["w"], q["h"]) for q in m["quads"]]
            parts.append(m)
        elif self.state == "serve":
            m = choice_menu(self._choices_serve(), selected=self.sel, x=240, y=170, w=220, size=14)
            self._choice_rects = [(q["x"], q["y"], q["w"], q["h"]) for q in m["quads"]]
            parts.append(m)
        if self.message:
            parts.append(message(self.message, 10, H - 92, 460, size=15))
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=merge(*parts))


def _room_world() -> dict[str, Any]:
    if DUMP.is_file():
        import json

        return json.loads(DUMP.read_text(encoding="utf-8"))
    return {"version": 1, "half": 8, "floor_y": 0, "props": [], "lights": [], "cameras": []}
