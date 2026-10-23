"""box_hop side-scroller. `PlatformerSession` (Rust owns jump / land / goal).

Platforms run along X. The chase camera sits on +Z, so A/D (and the on-screen
L/R pads) move the knight left and right. SPACE / the JUMP pad hops.

Phone picture (on unless `KAGRA_QUALITY=0`): bloom + punch LUT + a stronger
vignette, landing squash, a short hitstop, a CPU spark burst, a glowing
well, split crate maps, a wider sun shadow, and a studio hemisphere plus
prefiltered env.

    python examples/platformer_minimal.py
    python examples/platformer_minimal.py --headless scratch/platformer.png
    python examples/platformer_minimal.py --compare docs/screenshots

HUD text is ASCII (hyphen, not an em dash).
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import kagra  # noqa: E402
from kagra.juice import Juice, apply_post  # noqa: E402
from kagra.particles import Emitter  # noqa: E402
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
        if hasattr(self.session, "load_progress"):
            self.session.load_progress()
        self.world = json.loads(self.session.dump())
        self._edges = _jump_edges(self.world)
        self._scripted = False
        self._touch = (False, False, False)
        self.quality = os.environ.get("KAGRA_QUALITY", "1") != "0"
        self.juice = Juice()
        self.sparks = Emitter(max_count=28, seed=4)
        self._was_ground = True
        self._was_dead = False

    def _flow(self) -> str:
        phase = getattr(self.session, "flow_phase", None)
        if isinstance(phase, str) and phase:
            return phase
        return "play" if self.session.phase == "playing" else "title"

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

    def _ui_key(self) -> None:
        if not hasattr(self.session, "flow_key"):
            if self._flow() != "play" and (
                was_pressed("space") or was_pressed("enter") or was_pressed("return")
            ):
                self.session.confirm()
            return
        for key in ("escape", "p", "enter", "space", "r", "t", "m", "minus", "equal"):
            if was_pressed(key):
                self.session.flow_key(key)
        if mouse_clicked() and hasattr(self.session, "flow_pointer"):
            mx, my = mouse_pos()
            self.session.flow_pointer(mx, my, self.width, self.height)

    def update(self, dt: float) -> None:
        if self._scripted:
            lx, jump = self._scripted_wish()
            self._touch = (False, False, False)
        else:
            self._ui_key()
            if self._flow() != "play":
                lx, jump = 0.0, False
                self._touch = (False, False, False)
            else:
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
        dt_sim = self.juice.tick(dt) if self.quality else dt
        self.session.set_input(lx, 0.0, jump, False, False)
        self.session.tick(dt_sim)
        self.world = json.loads(self.session.dump())
        self._feel(dt)

    def _feel(self, dt: float) -> None:
        player = self.world.get("player") or {}
        grounded = bool(player.get("on_ground"))
        dead = bool(getattr(self.session, "dead", False)) or player.get("name") == "dead"
        if self.quality:
            pos = player.get("position") or [0.0, 1.0, 0.0]
            feet = (float(pos[0]), float(pos[1]) - 0.15, float(pos[2]))
            if self._was_ground and not grounded and not dead:
                self.juice.stretch(0.16, 0.14)
                self.sparks.burst(
                    feet,
                    count=8,
                    color=(255, 244, 210),
                    speed=1.4,
                    life=0.32,
                    size=0.14,
                    upward=2.4,
                    gravity=-6.0,
                )
            if grounded and not self._was_ground and not dead:
                self.juice.squash(0.28, 0.18)
                self.juice.shake(0.07, 0.16)
                self.juice.hitstop(0.04)
                self.sparks.burst(
                    feet,
                    count=12,
                    color=(214, 196, 160),
                    speed=1.8,
                    life=0.4,
                    size=0.16,
                    upward=1.1,
                    gravity=-9.0,
                )
            if dead and not self._was_dead:
                self.juice.shake(0.22, 0.32)
                self.juice.hitstop(0.07)
                self.sparks.burst(
                    feet,
                    count=16,
                    color=(255, 120, 80),
                    speed=2.6,
                    life=0.5,
                    size=0.18,
                    upward=2.0,
                    gravity=-7.0,
                )
            self.sparks.tick(dt)
        self._was_ground = grounded
        self._was_dead = dead

    def _picture(self) -> dict:
        if not self.quality:
            return self.world
        picture = json.loads(json.dumps(self.world))
        self.juice.apply_world(picture)
        picture.setdefault("props", []).extend(self.sparks.props())
        apply_post(picture, "mobile")
        _apply_wave2(picture)
        _apply_ibl(picture)
        return picture

    def draw(self) -> None:
        if hasattr(self.session, "hud_json"):
            hud = json.loads(self.session.hud_json(self.width, self.height))
        else:
            hud = {"quads": [], "texts": []}
        hud.setdefault("quads", [])
        hud.setdefault("texts", [])
        left, right, pad = _touch_layout(self.width, self.height)
        touch_l, touch_r, touch_j = self._touch
        if self._flow() != "play":
            self._canvas_png = draw_world(self._picture(), self.width, self.height, hud=hud)
            return

        def pad_color(active: bool, base: list[int]) -> list[int]:
            if active:
                return [base[0] + 40, base[1] + 40, base[2] + 30, 235]
            return base

        hud["quads"].extend(
            [
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
            ]
        )
        hud["texts"].extend(
            [
                {
                    "text": "A/D or L/R   SPACE jump   P pause",
                    "x": 16,
                    "y": 16,
                    "size": 12,
                    "color": [236, 242, 248, 255],
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
            ]
        )
        self._canvas_png = draw_world(self._picture(), self.width, self.height, hud=hud)


def _landed_dump() -> dict:
    """Script a hop and return the dump on the landing frame. Quality stays off
    so hitstop does not change the pose used for the before/after pair."""
    scene = PlatformPlay()
    scene.quality = False
    scene.session.confirm()
    scene._scripted = True
    scene.update(1.0 / 60.0)
    air = False
    landed = None
    for _ in range(240):
        scene.update(1.0 / 60.0)
        player = scene.world.get("player") or {}
        grounded = bool(player.get("on_ground"))
        if not grounded:
            air = True
        elif air:
            landed = json.loads(json.dumps(scene.world))
            break
    if landed is None:
        landed = json.loads(json.dumps(scene.world))
    return landed


def _juicy_picture(world: dict, wave2: bool = True) -> dict:
    feel = Juice()
    feel.squash(0.34, 0.22)
    feel.shake(0.10, 0.28, frequency=18.0)
    feel.tick(0.075)
    sparks = Emitter(max_count=18, seed=3)
    pos = (world.get("player") or {}).get("position") or [0.0, 1.0, 0.0]
    sparks.burst(
        (float(pos[0]), float(pos[1]) - 0.1, float(pos[2])),
        count=14,
        color=(255, 236, 170),
        speed=1.6,
        life=0.55,
        size=0.22,
        gravity=-3.0,
        upward=2.2,
    )
    sparks.tick(0.08)
    picture = json.loads(json.dumps(world))
    feel.apply_world(picture)
    picture.setdefault("props", []).extend(sparks.props())
    apply_post(picture, "mobile")
    if wave2:
        _apply_wave2(picture)
    return picture


def _apply_wave2(picture: dict) -> None:
    """Authored glow, a worn metal/rough map on the crates, softer sun shadows.

    Omitted on the before frame so the pair shows this wave.
    """
    picture["shadow_softness"] = 4.5
    for prop in picture.get("props") or []:
        if prop.get("gltf") == "crate.glb":
            # Shared crate mesh, so every platform picks up the split.
            prop["roughness_map"] = "preset:split"
            prop["metalness_map"] = "preset:split"
        if prop.get("name") == "checkpoint":
            prop["emissive"] = 6.5
            prop["emissive_color"] = [1.0, 0.48, 0.08]


def _apply_ibl(picture: dict) -> None:
    """Authorable sky/ground ambient and a prefiltered studio lat-long.

    Omitted on the IBL before frame, which keeps the fixed studio SH.
    """
    # Cooler sky bounce, warmer ground. The sky clear stays put, so the
    # pair is the lit platforms and the metal sheen, not a recolored backdrop.
    picture["hemi_sky"] = [0.28, 0.55, 1.70]
    picture["hemi_ground"] = [1.25, 0.48, 0.10]
    picture["env_map"] = "preset:studio"


def _mean_abs(a: bytes, b: bytes) -> float:
    n = min(len(a), len(b)) // 4
    if n == 0:
        return 0.0
    total = 0
    for i in range(n):
        o = i * 4
        total += abs(a[o] - b[o]) + abs(a[o + 1] - b[o + 1]) + abs(a[o + 2] - b[o + 2])
    return total / (n * 3)


def write_compare(out_dir: Path) -> None:
    from kagra.gameloop import last_draw_was_fallback, render_rgba, rgba_to_png

    world = _landed_dump()
    before_world = _juicy_picture(world, wave2=False)
    juicy = _juicy_picture(world, wave2=True)
    width, height = 640, 360
    before = render_rgba(before_world, width, height)
    after = render_rgba(juicy, width, height)
    if last_draw_was_fallback():
        raise SystemExit("no GPU adapter; compare shots were not written")
    if _mean_abs(before, after) < 1.0:
        raise SystemExit("before/after frames are almost identical")
    out_dir.mkdir(parents=True, exist_ok=True)
    before_png = rgba_to_png(before, width, height)
    after_png = rgba_to_png(after, width, height)
    (out_dir / "box_hop_before.png").write_bytes(before_png)
    (out_dir / "box_hop_after.png").write_bytes(after_png)
    print(
        f"wrote {out_dir / 'box_hop_before.png'} and {out_dir / 'box_hop_after.png'} "
        f"mean_abs={_mean_abs(before, after):.2f}"
    )


def write_ibl_compare(out_dir: Path) -> None:
    """Wave-2 picture vs the same pose with hemisphere + studio env."""
    from kagra.gameloop import last_draw_was_fallback, render_rgba, rgba_to_png

    world = _landed_dump()
    before_world = _juicy_picture(world, wave2=True)
    after_world = _juicy_picture(world, wave2=True)
    _apply_ibl(after_world)
    width, height = 640, 360
    before = render_rgba(before_world, width, height)
    after = render_rgba(after_world, width, height)
    if last_draw_was_fallback():
        raise SystemExit("no GPU adapter; IBL shots were not written")
    # The clear color is unchanged, so a full-frame mean sits under the
    # wave-2 guard. The platforms still have to move.
    if _mean_abs(before, after) < 0.75:
        raise SystemExit("IBL before/after frames are almost identical")
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "box_hop_ibl_before.png").write_bytes(rgba_to_png(before, width, height))
    (out_dir / "box_hop_ibl_after.png").write_bytes(rgba_to_png(after, width, height))
    print(
        f"wrote {out_dir / 'box_hop_ibl_before.png'} and {out_dir / 'box_hop_ibl_after.png'} "
        f"mean_abs={_mean_abs(before, after):.2f}"
    )


def main() -> None:
    if "--compare-ibl" in sys.argv:
        idx = sys.argv.index("--compare-ibl")
        out = Path(sys.argv[idx + 1]) if len(sys.argv) > idx + 1 else Path("docs/screenshots")
        write_ibl_compare(out)
        return
    if "--compare" in sys.argv:
        idx = sys.argv.index("--compare")
        out = Path(sys.argv[idx + 1]) if len(sys.argv) > idx + 1 else Path("docs/screenshots")
        write_compare(out)
        return
    headless = "--headless" in sys.argv
    scene = PlatformPlay()
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/platformer.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
        scene.width = 640
        scene.height = 360
        scene.session.confirm()
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
