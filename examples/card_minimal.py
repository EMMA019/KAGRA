"""カード対戦の最小例 - 1対1、20枚、相手AI、結果と再戦。

ルールは Rust の CardSession。扇・ドラッグ・めくりは kagra.card_ui。

操作: 手札を上へドラッグして出す。場のユニットを相手の体力かユニットへ
ドラッグして攻撃。Space でエンド。タイトルと結果はタップか Space で
開始 / 再戦。A/D で手札を選び、J/Z でそのカードを出す。
マウスとタッチは同じポインタ (begin / move / end)。

    python examples/card_minimal.py
    python examples/card_minimal.py --headless scratch/card_duel.png
    python examples/card_minimal.py --seed 3
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.card_ui import CardFace, DragPlay, TableView, duel_slots, fan_slots  # noqa: E402
from kagra.gameloop import (  # noqa: E402
    Scene,
    draw_world,
    mouse_clicked,
    mouse_down,
    mouse_pos,
    pressed,
    run,
    was_pressed,
)
from kagra.ui2d import merge, panel  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/card_world.json"
WIDTH = 960
HEIGHT = 540


def _load_session(seed: int):
    data = json.loads(DUMP.read_text(encoding="utf-8"))
    data["card"] = {"seed": int(seed), "phase": "title"}
    text = json.dumps(data)
    try:
        from kagra.kagra_shared import CardSession
    except ImportError:
        from kagra_shared import CardSession  # type: ignore[no-redef]
    return CardSession.from_json(text)


def _face(card: dict, *, on_board: bool = False) -> CardFace:
    ready = bool(card.get("ready", True)) if on_board else True
    return CardFace(
        title=str(card.get("name", "")),
        cost=int(card.get("cost", 0)),
        attack=int(card.get("atk", 0)),
        health=int(card.get("hp", 0)),
        text=str(card.get("line", "")),
        art=int(card.get("art", 0)),
        kind=str(card.get("kind", "unit")),
        ready=ready,
    )


def _hit(rect: tuple[float, float, float, float] | None, x: float, y: float) -> bool:
    if rect is None:
        return False
    rx, ry, rw, rh = rect
    return rx <= x <= rx + rw and ry <= y <= ry + rh


class CardPlay(Scene):
    def __init__(self, seed: int = 1) -> None:
        super().__init__()
        self.width = WIDTH
        self.height = HEIGHT
        self.session = _load_session(seed)
        self.table = TableView()
        self.drag = DragPlay()
        self._down = False
        self._end_arm = False
        self._end_rect: tuple[float, float, float, float] | None = None
        self._zones: list = []
        self._slots: dict = {}
        self._view = json.loads(self.session.view_json())
        self.world = json.loads(self.session.dump())
        self._layout()

    def update(self, dt: float) -> None:
        self._keys()
        self._mouse()
        self.session.tick(dt)
        self._refresh()
        self.table.tick(dt)

    def _keys(self) -> None:
        if was_pressed("escape"):
            self.quit()
            return
        lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
            1.0 if pressed("a") or pressed("left") else 0.0
        )
        jump = was_pressed("z") or was_pressed("j")
        self.session.set_input(lx, 0.0, jump, False, False)
        if was_pressed("space") or was_pressed("return"):
            if self.session.phase == "playing":
                self.session.end_turn()
            else:
                self.session.confirm()

    def _mouse(self) -> None:
        x, y = mouse_pos()
        down = mouse_down()
        if mouse_clicked() and not self._down:
            self.feed_pointer("begin", float(x), float(y))
        elif down and self._down:
            self.feed_pointer("move", float(x), float(y))
        elif self._down and not down:
            self.feed_pointer("end", float(x), float(y))
        self._down = down

    def feed_pointer(self, phase: str, x: float, y: float) -> None:
        """Mouse and touch share this. ``phase`` is begin, move, end, or cancel."""
        phase = str(phase)
        x = float(x)
        y = float(y)
        if self.session.phase != "playing":
            if phase == "begin":
                self.session.confirm()
                self.drag.clear()
                self._end_arm = False
            return
        if phase == "begin" and _hit(self._end_rect, x, y):
            self._end_arm = True
            self.drag.clear()
            return
        if self._end_arm:
            if phase in {"end", "cancel"}:
                hit = phase == "end" and _hit(self._end_rect, x, y)
                self._end_arm = False
                if hit:
                    self.session.end_turn()
            return
        result = self.drag.pointer(phase, x, y, self.table.views, self._zones)
        if result is None or result.action == "cancel":
            return
        if result.action == "play":
            self.session.play(result.index)
        elif result.action == "attack":
            self.session.attack(result.index, result.target)

    def feed_pointer_event(self, event) -> None:
        phase = event.phase.value if hasattr(event.phase, "value") else str(event.phase)
        self.feed_pointer(phase, float(event.x), float(event.y))

    def _refresh(self) -> None:
        self._view = json.loads(self.session.view_json())
        self.world = json.loads(self.session.dump())
        if self._view["phase"] != "playing":
            self.drag.clear()
            self._end_arm = False
        self._layout()

    def _layout(self) -> None:
        view = self._view
        you = view["you"]
        foe = view["foe"]
        slots = duel_slots(
            self.width,
            self.height,
            len(you.get("hand") or []),
            len(you.get("board") or []),
            int(foe.get("hand_n") or 0),
            len(foe.get("board") or []),
        )
        if view["phase"] == "title":
            slots["show"] = fan_slots(5, self.width * 0.5, self.height * 0.40, 78.0, 108.0)
        self._slots = slots
        self._zones = list(slots["zones"])
        self.drag.play_line = float(slots["play_line"])
        self.table.card_w = float(slots["card_w"])
        self.table.card_h = float(slots["card_h"])
        self.table.origin = (self.width * 0.08, self.height * 0.62)
        self.table.sync(self._entries(view), slots)
        bw, bh = 120.0, 36.0
        self._end_rect = (self.width - bw - 18.0, self.height * 0.46, bw, bh)

    def _entries(self, view: dict) -> list[tuple[int, CardFace, str, int]]:
        if view["phase"] == "title":
            rows = []
            for i, card in enumerate(view.get("catalog") or []):
                if i >= 5:
                    break
                rows.append((1000 + i, _face(card), "show", i))
            return rows
        you = view["you"]
        foe = view["foe"]
        rows = []
        for i, card in enumerate(you.get("hand") or []):
            rows.append((int(card["uid"]), _face(card), "hand", i))
        for i, card in enumerate(you.get("board") or []):
            rows.append((int(card["uid"]), _face(card, on_board=True), "board", i))
        for i, card in enumerate(foe.get("board") or []):
            rows.append((int(card["uid"]), _face(card, on_board=True), "foe_board", i))
        for i in range(int(foe.get("hand_n") or 0)):
            rows.append((-1000 - i, CardFace(face_down=True, title=""), "foe_hand", i))
        return rows

    def _chrome(self) -> dict:
        view = self._view
        w = float(self.width)
        h = float(self.height)
        labels = view["labels"]
        you = view["you"]
        foe = view["foe"]
        parts = [
            panel(12, 8, w - 24, 36, color=(16, 26, 24, 220), border=(88, 120, 96, 255), radius=8),
            {
                "texts": [
                    {
                        "text": str(view["status"]),
                        "x": w * 0.5,
                        "y": 16,
                        "size": 15,
                        "color": [236, 232, 214, 255],
                        "align": "center",
                    }
                ]
            },
            _plate(16, 56, 168, labels["life"], you["life"], you["mana"], you["mana_max"], labels["mana"]),
            _plate(w - 184, 56, 168, labels["life"], foe["life"], foe["mana"], foe["mana_max"], labels["mana"]),
            {
                "texts": [
                    {
                        "text": f"{labels['pile']} {you['deck_n']}",
                        "x": 20,
                        "y": h - 28,
                        "size": 13,
                        "color": [236, 228, 206, 255],
                    },
                    {
                        "text": f"{labels['pile']} {foe['deck_n']}",
                        "x": w - 120,
                        "y": 112,
                        "size": 13,
                        "color": [236, 228, 206, 255],
                    },
                    {
                        "text": str(view["hint"]),
                        "x": w * 0.5,
                        "y": h * 0.70,
                        "size": 14,
                        "color": [236, 228, 206, 255],
                        "align": "center",
                    },
                ]
            },
        ]
        if view["phase"] == "playing" and self._end_rect is not None:
            x, y, bw, bh = self._end_rect
            parts.append(
                panel(x, y, bw, bh, color=(48, 72, 58, 235), border=(186, 164, 96, 255), radius=8)
            )
            parts.append(
                {
                    "texts": [
                        {
                            "text": str(labels["end"]),
                            "x": x + bw * 0.5,
                            "y": y + 8,
                            "size": 16,
                            "color": [248, 242, 220, 255],
                            "align": "center",
                        }
                    ]
                }
            )
        if view["phase"] == "complete":
            parts.append(
                panel(w * 0.3, h * 0.38, w * 0.4, 72, color=(16, 22, 20, 230), border=(210, 176, 96, 255), radius=10)
            )
            parts.append(
                {
                    "texts": [
                        {
                            "text": str(view["status"]),
                            "x": w * 0.5,
                            "y": h * 0.38 + 22,
                            "size": 22,
                            "color": [248, 236, 196, 255],
                            "align": "center",
                        }
                    ]
                }
            )
        return merge(*parts)

    def draw(self) -> None:
        hud = merge(self._chrome(), self.table.hud())
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=hud)


def _plate(x, y, w, life_label, life, mana, mana_max, mana_label) -> dict:
    pips = []
    for i in range(int(mana_max)):
        filled = i < int(mana)
        pips.append(
            {
                "x": x + 8 + i * 14,
                "y": y + 28,
                "w": 10,
                "h": 10,
                "color": [220, 176, 72, 255] if filled else [36, 32, 28, 200],
            }
        )
    return merge(
        panel(x, y, w, 46, color=(18, 28, 26, 214), border=(90, 118, 96, 255), radius=8),
        {
            "quads": pips,
            "texts": [
                {
                    "text": f"{life_label} {int(life)}  {mana_label} {int(mana)}/{int(mana_max)}",
                    "x": x + 8,
                    "y": y + 6,
                    "size": 13,
                    "color": [240, 236, 220, 255],
                }
            ],
        },
    )


def _greedy(session) -> None:
    if session.phase != "playing":
        return
    if session.active != 0:
        session.end_turn()
        return
    for _ in range(8):
        played = False
        for index in range(session.you_hand - 1, -1, -1):
            if session.play(index):
                played = True
                break
        if not played:
            break
    if session.phase != "playing":
        return
    for index in range(session.you_board):
        session.attack(index, -1)
    if session.phase == "playing":
        session.end_turn()


def _settle(scene: CardPlay, frames: int = 24) -> None:
    for _ in range(frames):
        scene._refresh()
        scene.table.tick(1.0 / 30.0)


def _drag_one(scene: CardPlay) -> bool:
    """Touch-shaped drag of one hand card across the play line."""
    scene._refresh()
    _settle(scene, 8)
    hand = [view for view in scene.table.views if view.zone == "hand"]
    if not hand:
        return False
    card = hand[0]
    x = card.x + card.w * 0.5
    y = card.y + card.h * 0.4
    scene.feed_pointer("begin", x, y)
    scene.feed_pointer("move", x, scene.drag.play_line - 36)
    scene.feed_pointer("end", x, scene.drag.play_line - 36)
    return True


def _headless(scene: CardPlay, out: str) -> None:
    scene.feed_pointer("begin", 40.0, 40.0)
    assert scene.session.phase == "playing"
    assert scene.session.you_hand == 4
    dragged = _drag_one(scene)
    for _ in range(3):
        if scene.session.phase != "playing":
            break
        _greedy(scene.session)
    _settle(scene, 28)
    scene.draw()
    path = Path(out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(scene._canvas_png or b"")
    guard = 0
    while scene.session.phase == "playing" and guard < 80:
        guard += 1
        _greedy(scene.session)
    assert scene.session.phase == "complete", scene.session.status()
    winner = scene.session.winner
    scene.session.confirm()
    assert scene.session.phase == "playing"
    assert scene.session.you_life == 20
    assert scene.session.foe_life == 20
    print(
        f"wrote {path} bytes={path.stat().st_size} dragged={dragged} "
        f"winner={winner} rematch_turn={scene.session.turn} "
        f"status={scene.session.status()}"
    )


def main() -> None:
    seed = 1
    if "--seed" in sys.argv:
        seed = int(sys.argv[sys.argv.index("--seed") + 1])
    headless = "--headless" in sys.argv
    scene = CardPlay(seed=seed)
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/card_duel.png"
        _headless(scene, out)
        return
    run(scene, width=WIDTH, height=HEIGHT, title="カード対戦")


if __name__ == "__main__":
    main()
