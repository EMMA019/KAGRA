"""はじまりの村。`RpgSession`（ステータス、ターン戦闘、経験値、所持品、ゴールド、店は Rust）。

村人に話しかけ、宿屋で薬を買い、魔物を倒してレベルが上がる。
WASD で歩く。J / Z で話す・買う・戦う。Space で持ち物。
画面下の 話す / 買う / 戦う / 持ち物 はタップか 1 / 2 / 3 / 4。
Space / Enter ではじめる。やりなおしも同じ。

    python examples/rpg_minimal.py
    python examples/rpg_minimal.py --headless scratch/rpg_minimal.png

HUD の日本語は PixelMplus、数字は ASCII。字形の無い文字は出さない。
"""
from __future__ import annotations

import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.gameloop import Scene, draw_world, mouse_clicked, mouse_pos, pressed, run, was_pressed  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/rpg_world.json"
WIDTH = 960
HEIGHT = 540


def _load_session():
    try:
        from kagra.kagra_shared import RpgSession
    except ImportError:
        from kagra_shared import RpgSession  # type: ignore[no-redef]
    return RpgSession.from_json(DUMP.read_text(encoding="utf-8"))


def _button_center(kind: int, width: int, height: int) -> tuple[float, float]:
    """Same rects as Rust `action_buttons`. 0 話す, 1 買う, 2 戦う, 3 持ち物."""
    w = float(width)
    h = float(height)
    scale = min(w, h) / 720.0
    scale = 0.5 if scale < 0.5 else (2.0 if scale > 2.0 else scale)
    bw = 140.0 * scale
    bh = 48.0 * scale
    gap = 12.0 * scale
    y = h - bh - 18.0 * scale
    total = bw * 4.0 + gap * 3.0
    x0 = (w - total) * 0.5
    x = x0 + kind * (bw + gap)
    return x + bw * 0.5, y + bh * 0.5


def _steer(session, tx: float, tz: float) -> tuple[float, float]:
    world = json.loads(session.dump())
    pos = world["player"]["position"]
    cam = world["cameras"][0]
    dx = float(cam["position"][0]) - float(cam["target"][0])
    dz = float(cam["position"][2]) - float(cam["target"][2])
    yaw = math.atan2(-dx, -dz)
    wx = tx - float(pos[0])
    wz = tz - float(pos[2])
    dist = math.hypot(wx, wz)
    if dist < 0.8:
        return 0.0, 0.0
    wx /= dist
    wz /= dist
    sy, cy = math.sin(yaw), math.cos(yaw)
    return -cy * wx + sy * wz, sy * wx + cy * wz


def _walk_to(session, tx: float, tz: float) -> None:
    for _ in range(240):
        lx, lz = _steer(session, tx, tz)
        session.set_input(lx, lz, False, False, False)
        session.tick(1.0 / 60.0)
        if lx == 0.0 and lz == 0.0:
            return
    raise AssertionError(f"did not reach {(tx, tz)}")


def _tap(session, kind: int, width: int, height: int) -> None:
    x, y = _button_center(kind, width, height)
    session.pointer(x, y, width, height)
    session.tick(1.0 / 60.0)


class RpgPlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.width = WIDTH
        self.height = HEIGHT
        self.session = _load_session()
        self.world = json.loads(self.session.dump())

    def update(self, dt: float) -> None:
        if mouse_clicked(1):
            x, y = mouse_pos()
            self.session.pointer(float(x), float(y), self.width, self.height)
        elif self.session.phase != "playing" and (
            was_pressed("space") or was_pressed("enter") or was_pressed("return")
        ):
            self.session.confirm()
        for key, kind in (("1", 0), ("2", 1), ("3", 2), ("4", 3)):
            if was_pressed(key):
                x, y = _button_center(kind, self.width, self.height)
                self.session.pointer(x, y, self.width, self.height)
        lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
            1.0 if pressed("a") or pressed("left") else 0.0
        )
        lz = (1.0 if pressed("w") or pressed("up") else 0.0) - (
            1.0 if pressed("s") or pressed("down") else 0.0
        )
        playing = self.session.phase == "playing"
        attack = playing and (was_pressed("j") or was_pressed("z"))
        jump = playing and was_pressed("space")
        dodge = playing and was_pressed("x")
        self.session.set_input(lx, lz, jump, attack, dodge)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())

    def draw(self) -> None:
        hud = json.loads(self.session.hud_json(self.width, self.height))
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=hud)


def _assert_glyphs(hud: str) -> None:
    assert "\u2014" not in hud
    data = json.loads(hud)
    texts = [row["text"] for row in data.get("texts") or []]
    assert texts, hud
    blob = "\n".join(texts)
    assert "話す" in blob or "はじまりの村" in blob or "レベル" in blob


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/rpg_minimal.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = RpgPlay()
    if headless:
        scene.session.pointer(12.0, 12.0, scene.width, scene.height)
        assert scene.session.phase == "playing"
        _tap(scene.session, 3, scene.width, scene.height)
        assert scene.session.menu
        _tap(scene.session, 3, scene.width, scene.height)
        assert not scene.session.menu
        _walk_to(scene.session, 0.2, 2.4)
        _tap(scene.session, 0, scene.width, scene.height)
        assert scene.session.talking
        assert scene.session.has_flag("met")
        assert "薬" in scene.session.subtitle()
        scene.world = json.loads(scene.session.dump())
        scene.draw()
        Path(out).write_bytes(scene._canvas_png or b"")
        hud = scene.session.hud_json(scene.width, scene.height)
        _assert_glyphs(hud)
        assert "店で薬を買え" in hud
        _tap(scene.session, 0, scene.width, scene.height)
        assert not scene.session.talking
        _walk_to(scene.session, -5.2, -0.4)
        _tap(scene.session, 1, scene.width, scene.height)
        assert scene.session.shop_open
        _tap(scene.session, 1, scene.width, scene.height)
        assert scene.session.has_item("potion")
        assert scene.session.gold == 12
        _walk_to(scene.session, 4.6, 1.6)
        _tap(scene.session, 2, scene.width, scene.height)
        assert scene.session.combat
        assert scene.session.foe_hp == 6
        for _ in range(2):
            scene.session.set_input(0.0, 0.0, False, True, False)
            scene.session.tick(1.0 / 60.0)
        assert scene.session.won and scene.session.leveled
        assert scene.session.level == 2
        assert scene.session.atk == 4
        assert scene.session.gold == 16
        assert scene.session.phase == "complete"
        done = scene.session.hud_json(scene.width, scene.height)
        _assert_glyphs(done)
        assert "レベルが上がった" in done
        scene.session.pointer(12.0, 12.0, scene.width, scene.height)
        assert scene.session.phase == "playing"
        assert scene.session.level == 1
        assert scene.session.gold == 20
        print(
            f"wrote {out} phase={scene.session.phase} level={scene.session.level} "
            f"gold={scene.session.gold} hp={scene.session.hp}"
        )
        return
    run(scene, width=WIDTH, height=HEIGHT, title="KAGRA - はじまりの村")


if __name__ == "__main__":
    main()
