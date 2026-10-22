"""box_hop side-scroller. `PlatformerSession` (Rust owns jump / land / goal).

Platforms run along X. The chase camera sits on +Z, so A/D (and the on-screen
L/R pads) move the knight left and right. SPACE / the JUMP pad hops.

    python examples/platformer_minimal.py
    python examples/platformer_minimal.py --headless scratch/platformer.png

HUD text is ASCII (hyphen, not an em dash).
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import kagra  # noqa: E402
from kagra.gameloop import Scene, draw_world, mouse_down, mouse_pos, pressed, run, was_pressed  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/platformer_world.json"


def _load_session():
    session_cls = kagra.PlatformerSession
    if session_cls is None:
        from kagra_shared import PlatformerSession as session_cls  # type: ignore[no-redef]
    return session_cls.from_json(DUMP.read_text(encoding="utf-8"))


def _jump_edges(world: dict) -> list[float]:
    """Right lip of each slab except the goal (rightmost) platform."""
    edges: list[float] = []
    for prop in world.get("props") or []:
        if prop.get("name") != "platform":
            continue
        x = float(prop["position"][0])
        sx = abs(float(prop["scale"][0]))
        edges.append(x + 0.5 * sx)
    edges.sort()
    if edges:
        edges.pop()
    return edges


def _touch_layout(width: int, height: int) -> tuple[tuple[int, int, int, int], ...]:
    """On-screen L / R / JUMP pads. Origin is the top-left, y grows down."""
    y = height - 58
    left = (14, y, 62, 46)
    right = (84, y, 62, 46)
    jump = (width - 86, y, 72, 46)
    return left, right, jump


def _hit(rect: tuple[int, int, int, int], x: float, y: float) -> bool:
    rx, ry, rw, rh = rect
    return rx <= x <= rx + rw and ry <= y <= ry + rh


class PlatformPlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.session.confirm()
        self.world = json.loads(self.session.dump())
        self._edges = _jump_edges(self.world)
        self._scripted = False
        self._touch = (False, False, False)

    def _scripted_wish(self) -> tuple[float, bool]:
        if self.session.won:
            return 0.0, False
        if self.session.dead and self.session.phase != "playing":
            self.session.confirm()
        player = self.world.get("player") or {}
        pos = player.get("position") or [0.0, 0.0, 0.0]
        x = float(pos[0])
        on_ground = bool(player.get("on_ground"))
        jump = on_ground and any((edge - 0.85) < x < (edge - 0.12) for edge in self._edges)
        return 1.0, jump

    def update(self, dt: float) -> None:
        if self._scripted:
            lx, jump = self._scripted_wish()
            self._touch = (False, False, False)
        else:
            if self.session.phase != "playing" and (
                was_pressed("space") or was_pressed("enter") or was_pressed("return")
            ):
                self.session.confirm()
            lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
                1.0 if pressed("a") or pressed("left") else 0.0
            )
            jump = pressed("space") or pressed("w") or pressed("up") or pressed("k")
            left, right, pad = _touch_layout(self.width, self.height)
            mx, my = mouse_pos()
            held = mouse_down()
            touch_l = held and _hit(left, mx, my)
            touch_r = held and _hit(right, mx, my)
            touch_j = held and _hit(pad, mx, my)
            if touch_l:
                lx -= 1.0
            if touch_r:
                lx += 1.0
            if touch_j:
                jump = True
            lx = max(-1.0, min(1.0, lx))
            self._touch = (touch_l, touch_r, touch_j)
        self.session.set_input(lx, 0.0, jump, False, False)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())

    def draw(self) -> None:
        if self.session.won:
            status = "box_hop - goal"
        elif self.session.dead:
            status = "box_hop - fell"
        else:
            status = f"box_hop - {self.session.phase}"
        left, right, pad = _touch_layout(self.width, self.height)
        touch_l, touch_r, touch_j = self._touch

        def pad_color(active: bool, base: list[int]) -> list[int]:
            if active:
                return [base[0] + 40, base[1] + 40, base[2] + 30, 235]
            return base

        hud = {
            "quads": [
                {"x": 8, "y": 8, "w": 300, "h": 36, "color": [20, 28, 40, 210]},
                {
                    "x": left[0],
                    "y": left[1],
                    "w": left[2],
                    "h": left[3],
                    "color": pad_color(touch_l, [36, 44, 58, 220]),
                },
                {
                    "x": right[0],
                    "y": right[1],
                    "w": right[2],
                    "h": right[3],
                    "color": pad_color(touch_r, [36, 44, 58, 220]),
                },
                {
                    "x": pad[0],
                    "y": pad[1],
                    "w": pad[2],
                    "h": pad[3],
                    "color": pad_color(touch_j, [48, 110, 170, 230]),
                },
            ],
            "texts": [
                {
                    "text": status,
                    "x": 16,
                    "y": 16,
                    "size": 15,
                    "color": [236, 242, 248, 255],
                    "align": "left",
                },
                {
                    "text": "A/D or L/R   SPACE jump",
                    "x": 16,
                    "y": 32,
                    "size": 11,
                    "color": [200, 214, 226, 255],
                    "align": "left",
                },
                {
                    "text": "L",
                    "x": left[0] + left[2] * 0.5,
                    "y": left[1] + 14,
                    "size": 16,
                    "color": [240, 244, 248, 255],
                    "align": "center",
                },
                {
                    "text": "R",
                    "x": right[0] + right[2] * 0.5,
                    "y": right[1] + 14,
                    "size": 16,
                    "color": [240, 244, 248, 255],
                    "align": "center",
                },
                {
                    "text": "JUMP",
                    "x": pad[0] + pad[2] * 0.5,
                    "y": pad[1] + 14,
                    "size": 13,
                    "color": [248, 252, 255, 255],
                    "align": "center",
                },
            ],
        }
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=hud)


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/platformer.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = PlatformPlay()
    if headless:
        scene.width = 640
        scene.height = 360
        scene._scripted = True
        for _ in range(900):
            scene.update(1.0 / 60.0)
            if scene.session.won:
                break
        scene.draw()
        Path(out).write_bytes(scene._canvas_png or b"")
        player = (scene.world.get("player") or {}).get("position")
        print(
            f"wrote {out} won={scene.session.won} dead={scene.session.dead} "
            f"phase={scene.session.phase} landed={scene.session.landed} pos={player}"
        )
        return
    run(scene, width=640, height=360, title="KAGRA - box hop")


if __name__ == "__main__":
    main()
