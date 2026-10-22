"""Local 1v1 on the fight ring. Two humans, one device.

Player 1: WASD move, F attack, C block.
Player 2: arrow keys, 1 attack, 2 block.
Touch: the left half of the screen is player 1, the right half is player 2
(stick on the outer corner, ATK / BLOCK toward the center).

    python examples/versus_local.py
    python examples/versus_local.py --headless scratch/versus_local.png

Rust `FightSession.set_player_input` drives both slots. Slot 1 replaces
the opponent AI. HUD text is ASCII.
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import kagra  # noqa: E402
from kagra.gameloop import Scene, draw_world, mouse_down, mouse_pos, pressed, run  # noqa: E402
from kagra.netplay import half_pads, halves_input, scripted_stick  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/fight_hitstun_world.json"


def _load_session():
    session_cls = kagra.FightSession
    if session_cls is None:
        from kagra_shared import FightSession as session_cls  # type: ignore[no-redef]
    return session_cls.from_json(DUMP.read_text(encoding="utf-8"))


def _axis(neg: str, pos: str) -> float:
    return (1.0 if pressed(pos) else 0.0) - (1.0 if pressed(neg) else 0.0)


def _keys() -> tuple[tuple[float, float, bool, bool], tuple[float, float, bool, bool]]:
    p1 = (_axis("a", "d"), _axis("s", "w"), pressed("f"), pressed("c"))
    p2 = (_axis("left", "right"), _axis("down", "up"), pressed("1"), pressed("2"))
    return p1, p2


def _mix(keys, touch) -> tuple[float, float, bool, bool]:
    lx = max(-1.0, min(1.0, keys[0] + touch[0]))
    lz = max(-1.0, min(1.0, keys[1] + touch[1]))
    return lx, lz, keys[2] or touch[2], keys[3] or touch[3]


class VersusLocal(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.session.confirm()
        self.world = json.loads(self.session.dump())
        self._scripted = False
        self._tick = 0
        self._touch = (
            (0.0, 0.0, False, False),
            (0.0, 0.0, False, False),
        )

    def update(self, dt: float) -> None:
        if self.session.phase != "playing":
            self.session.confirm()
        if self._scripted:
            for slot in (0, 1):
                lx, lz, _jump, attack, dodge = scripted_stick(slot, self._tick)
                self.session.set_player_input(slot, lx, lz, False, attack, dodge)
            self._touch = ((0.0, 0.0, False, False), (0.0, 0.0, False, False))
        else:
            p1, p2 = _keys()
            if mouse_down():
                mx, my = mouse_pos()
                touch = halves_input(
                    self.width,
                    self.height,
                    [{"x": mx, "y": my, "phase": "move"}],
                )
                p1, p2 = _mix(p1, touch[0]), _mix(p2, touch[1])
                self._touch = touch
            else:
                self._touch = ((0.0, 0.0, False, False), (0.0, 0.0, False, False))
            for slot, row in enumerate((p1, p2)):
                lx, lz, attack, block = row
                self.session.set_player_input(slot, lx, lz, False, attack, block)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())
        self._tick += 1

    def draw(self) -> None:
        pads = half_pads(self.width, self.height)
        status = f"P1 {self.session.hp}/{self.session.hp_max}  P2 {self.session.opp_hp}/{self.session.opp_hp_max}"
        if self.session.won:
            status = "P1 wins  space retry"
        elif self.session.ko:
            status = "P2 wins  space retry"
        quads = [{"x": 8, "y": 8, "w": 460, "h": 36, "color": [20, 16, 18, 210]}]
        texts = [
            {
                "text": status,
                "x": 16,
                "y": 14,
                "size": 16,
                "color": [244, 236, 228, 255],
                "align": "left",
            }
        ]
        for slot, pad in enumerate(pads):
            touch = self._touch[slot]
            cx, cy, radius = pad.stick
            quads.append(
                {
                    "x": int(cx - radius),
                    "y": int(cy - radius),
                    "w": int(radius * 2),
                    "h": int(radius * 2),
                    "color": [36, 40, 52, 210],
                }
            )
            quads.append(
                {
                    "x": int(cx - 8 + touch[0] * radius * 0.55),
                    "y": int(cy - 8 - touch[1] * radius * 0.55),
                    "w": 16,
                    "h": 16,
                    "color": [230, 236, 244, 235],
                }
            )
            for rect, label, down, fill in (
                (pad.attack, "ATK", touch[2], [176, 56, 48, 230]),
                (pad.block, "BLK", touch[3], [48, 110, 150, 230]),
            ):
                color = [min(255, c + 36) if down else c for c in fill[:3]] + [230]
                quads.append(
                    {
                        "x": int(rect[0]),
                        "y": int(rect[1]),
                        "w": int(rect[2]),
                        "h": int(rect[3]),
                        "color": color,
                    }
                )
                texts.append(
                    {
                        "text": label,
                        "x": rect[0] + rect[2] * 0.5,
                        "y": rect[1] + 14,
                        "size": 13,
                        "color": [255, 244, 236, 255],
                        "align": "center",
                    }
                )
        texts.append(
            {
                "text": "P1 WASD F/C    P2 arrows 1/2",
                "x": 16,
                "y": 48,
                "size": 12,
                "color": [210, 206, 198, 255],
                "align": "left",
            }
        )
        hud = {"quads": quads, "texts": texts}
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=hud)


def main() -> None:
    headless = "--headless" in sys.argv or os.environ.get("KAGRA_HEADLESS") == "1"
    out = "scratch/versus_local.png"
    if "--headless" in sys.argv:
        idx = sys.argv.index("--headless")
        if len(sys.argv) > idx + 1 and not sys.argv[idx + 1].startswith("--"):
            out = sys.argv[idx + 1]
    if headless:
        Path(out).parent.mkdir(parents=True, exist_ok=True)
        scene = VersusLocal()
        scene.width = 640
        scene.height = 360
        scene._scripted = True
        shot = b""
        for _ in range(70):
            scene.update(1.0 / 60.0)
            if scene._tick == 55:
                scene.draw()
                shot = scene._canvas_png or b""
        scene.draw()
        png = shot or scene._canvas_png or b""
        Path(out).write_bytes(png)
        player = (scene.world.get("player") or {}).get("position")
        print(
            f"wrote {out} slots={scene.session.player_slots} "
            f"hp={scene.session.hp} opp={scene.session.opp_hp} "
            f"hits={scene.session.hits} phase={scene.session.phase} pos={player}"
        )
        return
    run(VersusLocal(), width=960, height=540, title="KAGRA - local versus")


if __name__ == "__main__":
    main()
