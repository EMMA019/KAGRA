"""Lumen — neon endless flyer.

The ship, the rocks and turrets around each gate, the crystal pickups, and
the side dishes are Kenney Space Kit GLBs (CC0). License:
``assets/models/kenney/space-kit/LICENSE.txt``. The neon ribs, rails, rings,
and stars stay procedural. A missing GLB falls back to a sphere or a box.
Sound is synthesized ``kagra.audio`` tones plus the repo's CC0 clips in
``assets/audio`` (see ``assets/audio/LICENSE.txt``).

    python examples/neon_run_minimal.py
    python examples/neon_run_minimal.py --headless scratch/neon_run.png
    python examples/neon_run_minimal.py --shots docs/screenshots

``--shots`` writes title / play / crash and wins when ``--headless`` is
also passed (a warning is printed). ``KAGRA_NEON_HEADLESS=1`` is the
same path as ``--headless``.

WASD or arrows steer. Shift or Space boosts. Drag on the window (or a
phone) aims the ship. P pauses. Fly through the glowing rings. Hitting
the wall or missing a ring ends the run.

One ``dump()`` per tick. Aim, the listener, and the next gate come from
the dump's ``sim`` object. Wasm does not JSON-round-trip the world:
``render`` draws the document, and ``gameJson`` is only the small status
blob. Phone picture (on unless ``KAGRA_QUALITY=0``): the Rust dump's grade
(exposure, bloom, fog, cyan gates, the horizontal trail). Juice and
sparks sit on top. The shared ``night`` preset is brighter and is not
applied, so the window matches wasm. The wasm / Android build runs the
same Rust sim from ``games/neon_run_minimal.json``.
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
if str(_ROOT) not in sys.path:
    sys.path.insert(0, str(_ROOT))

try:
    import kagra  # noqa: E402
except ModuleNotFoundError as exc:  # pragma: no cover - checkout layout
    if exc.name not in {None, "kagra"}:
        raise
    raise ImportError(
        "Could not import kagra. Run Lumen from a checkout of this repo "
        f"so {_ROOT} is on sys.path."
    ) from exc
from kagra.audio import play_bgm, play_se, se, set_listener, stop_bgm  # noqa: E402
from kagra.audio import tick as audio_tick  # noqa: E402
from kagra.gameloop import (  # noqa: E402
    Scene,
    draw_world,
    mouse_clicked,
    mouse_down,
    mouse_pos,
    pressed,
    render_rgba,
    rgba_to_png,
    run,
    was_pressed,
)
from kagra.juice import Juice  # noqa: E402
from kagra.particles import Emitter  # noqa: E402

DUMP = _ROOT / "kagra-shared/tests/fixtures/neon_run_world.json"
W, H = 1280, 720
# Screen-right gain toward the next gate. The chase camera looks along +Z,
# so +lx is world -X. 1.6 reaches the hole without oscillating past it.
AIM_GAIN = 1.6
_ON = frozenset({"1", "true", "yes", "on"})


def _truthy_env(name: str, *, default: bool = False, only_off: str | None = None) -> bool:
    """Read a boolean env var.

    With ``only_off``, every token except that one is on. Quality uses this
    so ``KAGRA_QUALITY`` stays on unless the value is ``0``. Other flags are
    on for ``1`` / ``true`` / ``yes`` / ``on``.
    """
    raw = os.environ.get(name)
    if raw is None:
        return default
    token = raw.strip().lower()
    if only_off is not None:
        return token != only_off
    if token == "":
        return default
    return token in _ON


def _session():
    cls = getattr(kagra, "NeonRun", None)
    if cls is None:
        import kagra_shared

        cls = kagra_shared.NeonRun
    return cls.from_json(DUMP.read_text(encoding="utf-8"))


def _clamp(v: float, lo: float = -1.0, hi: float = 1.0) -> float:
    return lo if v < lo else hi if v > hi else v


class Lumen(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _session()
        self.world = json.loads(self.session.dump())
        self.quality = _truthy_env("KAGRA_QUALITY", default=True, only_off="0")
        self.juice = Juice()
        self.sparks = Emitter(max_count=48, seed=7, name="burst", emissive=1.6)
        self._music = False
        self._scripted = False
        self._crash = False
        self._pause: dict | None = None
        self._pause_key: tuple | None = None

    def _flow(self) -> str:
        phase = getattr(self.session, "flow_phase", None)
        if isinstance(phase, str) and phase:
            return phase
        return "play"

    def _ui_key(self) -> None:
        for key in ("escape", "p", "enter", "space", "r", "t", "m", "minus", "equal"):
            if was_pressed(key):
                self.session.flow_key(key)
        if mouse_clicked():
            mx, my = mouse_pos()
            self.session.flow_pointer(mx, my, self.width, self.height)

    def _steer(self) -> tuple[float, float, bool]:
        if self._scripted:
            return self._aim()
        axes: list[float] = []
        for positive, negative in (
            (("d", "right"), ("a", "left")),
            (("w", "up"), ("s", "down")),
        ):
            axes.append(
                (1.0 if any(pressed(key) for key in positive) else 0.0)
                - (1.0 if any(pressed(key) for key in negative) else 0.0)
            )
        lx, ly = axes
        boost = any(pressed(key) for key in ("shift", "space", "k"))
        if mouse_down() and self._flow() == "play":
            mx, my = mouse_pos()
            if not self._pointer_on_pause(mx, my):
                lx = _clamp(lx + (mx / max(1, self.width) - 0.5) * 2.1)
                ly = _clamp(ly + (0.5 - my / max(1, self.height)) * 2.1)
        return _clamp(lx), _clamp(ly), boost

    def _pointer_on_pause(self, mx: float, my: float) -> bool:
        """True when the pointer is inside the sim's PAUSE button.

        The rect is ``hud_json``'s ``pause`` field. It is cached until the
        phase or the frame size changes, so a drag does not re-parse HUD.
        """
        key = (self.width, self.height, self._flow())
        if self._pause_key != key:
            hud = json.loads(self.session.hud_json(self.width, self.height))
            pause = hud.get("pause")
            self._pause = pause if isinstance(pause, dict) else None
            self._pause_key = key
        pause = self._pause
        if not pause:
            return False
        try:
            x = float(pause["x"])
            y = float(pause["y"])
            w = float(pause["w"])
            h = float(pause["h"])
        except (KeyError, TypeError, ValueError):
            return False
        return x <= mx <= x + w and y <= my <= y + h

    def _aim(self) -> tuple[float, float, bool]:
        # Scripted crash: hard screen-right, which is world -X on this camera.
        if self._crash:
            return 1.0, 0.0, False
        sim = self.world.get("sim") if isinstance(self.world, dict) else None
        if not isinstance(sim, dict):
            return 0.0, 0.0, False
        if sim.get("phase") != "play" or not sim.get("alive", True):
            return 0.0, 0.0, False
        dx = float(sim["next_x"]) - float(sim["x"])
        dy = float(sim["next_y"]) - float(sim["y"])
        return _clamp(-dx * AIM_GAIN), _clamp(dy * AIM_GAIN), False

    def update(self, dt: float) -> None:
        if not self._scripted:
            self._ui_key()
        flow = self._flow()
        if flow != "play":
            lx, ly, boost = 0.0, 0.0, False
        else:
            lx, ly, boost = self._steer()
        dt_sim = self.juice.tick(dt) if self.quality else dt
        self.session.set_input(lx, ly, boost)
        self.session.tick(dt_sim)
        self.world = json.loads(self.session.dump())
        self._audio(dt)
        self._feel(dt)

    def _audio(self, dt: float) -> None:
        audio_tick(dt)
        flow = self._flow()
        if flow == "play" and not self._music:
            play_bgm("bgm", 0.35)
            self._music = True
        if flow != "play" and self._music:
            stop_bgm(0.25)
            self._music = False
        cam = (self.world.get("cameras") or [{}])[0]
        pos = cam.get("position") or [0, 0, 0]
        sim = self.world.get("sim") if isinstance(self.world, dict) else None
        fwd = sim.get("cam_forward") if isinstance(sim, dict) else None
        if not isinstance(fwd, (list, tuple)) or len(fwd) < 3:
            fwd = (0.0, 0.0, 1.0)
        set_listener(
            float(pos[0]),
            float(pos[1]),
            float(pos[2]),
            float(fwd[0]),
            float(fwd[1]),
            float(fwd[2]),
        )

    def _ship(self) -> tuple[float, float, float]:
        for prop in self.world.get("props") or []:
            if prop.get("name") == "ship-core":
                p = prop.get("position") or [0, 0, 0]
                return float(p[0]), float(p[1]), float(p[2])
        return 0.0, 0.0, 0.0

    def _feel(self, dt: float) -> None:
        if not self.quality:
            return
        feel = json.loads(self.session.take_feel())
        ship = self._ship()
        if int(feel.get("orbs") or 0) > 0:
            self.juice.squash(0.28, 0.16)
            self.juice.shake(0.08, 0.14)
            self.sparks.burst(ship, count=26, color=(255, 236, 160), speed=5.2, life=0.45, size=0.28, upward=1.8)
            # One coin. ``play_se`` is the CC0 sample in 3D. ``se("coin")``
            # is a second synthesized coin and used to stack on top.
            play_se("coin", x=ship[0], y=ship[1], z=ship[2])
        if feel.get("near"):
            self.juice.shake(0.05, 0.1)
        if feel.get("crash"):
            self.juice.hitstop(0.08)
            self.juice.shake(0.28, 0.4)
            self.sparks.burst(ship, count=22, color=(255, 60, 150), speed=4.4, life=0.55, size=0.22, upward=2.2)
            se("hit")
        self.sparks.tick(dt)

    def _picture(self) -> dict:
        # Copy only the dicts this frame writes. Juice moves camera 0 and
        # scales the ship; ``annotate_dump`` writes the camera side vectors;
        # sparks append props. A full ``json`` round-trip or ``deepcopy``
        # walked every rock and star for those few writes.
        picture = self._picture_base()
        if not self.quality:
            for key in ("bloom", "bloom_threshold", "vignette", "lut", "lut_strength", "fxaa"):
                picture.pop(key, None)
            return picture
        self.juice.apply_world(picture)
        sx, sy, sz = self.juice.scale()
        for prop in picture.get("props") or []:
            if str(prop.get("name", "")).startswith("ship"):
                sc = prop.get("scale") or [1, 1, 1]
                prop["scale"] = [float(sc[0]) * sx, float(sc[1]) * sy, float(sc[2]) * sz]
        picture["props"].extend(self.sparks.props())
        return picture

    def _picture_base(self) -> dict:
        src = self.world if isinstance(self.world, dict) else {}
        picture = dict(src)
        picture["cameras"] = [
            dict(cam) if isinstance(cam, dict) else cam for cam in (src.get("cameras") or [])
        ]
        props = []
        for prop in src.get("props") or []:
            if isinstance(prop, dict) and str(prop.get("name", "")).startswith("ship"):
                copied = dict(prop)
                scale = copied.get("scale")
                if isinstance(scale, (list, tuple)):
                    copied["scale"] = list(scale)
                props.append(copied)
            else:
                props.append(prop)
        picture["props"] = props
        player = src.get("player")
        if isinstance(player, dict):
            picture["player"] = dict(player)
        walkers = src.get("walkers")
        if isinstance(walkers, list):
            picture["walkers"] = [
                dict(item) if isinstance(item, dict) else item for item in walkers
            ]
        return picture

    def draw(self) -> None:
        hud = json.loads(self.session.hud_json(self.width, self.height))
        self._frame_rgba = render_rgba(self._picture(), self.width, self.height, hud)


def _write(scene: Lumen, path: Path) -> None:
    scene.draw()
    png = rgba_to_png(scene.frame() or b"", scene.width, scene.height)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png)
    print(f"wrote {path} ({len(png)} bytes) phase={scene._flow()} score={scene.session.score}")


def _boot(scripted: bool = False) -> Lumen:
    scene = Lumen()
    scene._scripted = scripted
    scene.width = W
    scene.height = H
    return scene


def shots(directory: Path) -> int:
    """Title, a threaded ring, and the crash result."""
    directory.mkdir(parents=True, exist_ok=True)
    title = _boot()
    for _ in range(90):
        title.update(1.0 / 60.0)
    _write(title, directory / "neon_run_title.png")

    play = _boot(scripted=True)
    play.session.confirm()
    for _ in range(150):
        play.update(1.0 / 60.0)
    _write(play, directory / "neon_run_play.png")
    world_path = directory / "neon_run_play.json"
    world_path.write_text(json.dumps(play._picture()), encoding="utf-8")

    crash = _boot(scripted=True)
    crash.session.confirm()
    for _ in range(120):
        crash.update(1.0 / 60.0)
    crash._crash = True
    for _ in range(90):
        crash.update(1.0 / 60.0)
        if crash._flow() == "result":
            break
    _write(crash, directory / "neon_run_result.png")
    if crash.session.score < 40:
        print(f"lumen result score {crash.session.score} < 40")
        return 1
    if play.session.score < 50:
        print(f"lumen play score {play.session.score} < 50")
        return 1
    return 0


def headless(out: str) -> int:
    scene = _boot(scripted=True)
    scene.session.confirm()
    for _ in range(160):
        scene.update(1.0 / 60.0)
    path = Path(out)
    _write(scene, path)
    world_path = path.parent / "neon_run_world.json"
    world_path.write_text(json.dumps(scene._picture()), encoding="utf-8")
    print(json.dumps({"score": scene.session.score, "phase": scene._flow()}))
    if scene.session.score < 50:
        print(f"lumen headless score {scene.session.score} < 50")
        return 1
    return 0


def main() -> int:
    shots_flag = "--shots" in sys.argv
    headless_flag = "--headless" in sys.argv or _truthy_env("KAGRA_NEON_HEADLESS")
    if shots_flag and headless_flag:
        print("lumen: --shots wins over --headless", file=sys.stderr)
    if shots_flag:
        idx = sys.argv.index("--shots")
        directory = Path(sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "docs/screenshots")
        return shots(directory)
    if headless_flag:
        out = "scratch/neon_run.png"
        if "--headless" in sys.argv:
            idx = sys.argv.index("--headless")
            if len(sys.argv) > idx + 1 and not sys.argv[idx + 1].startswith("--"):
                out = sys.argv[idx + 1]
        return headless(out)
    run(Lumen(), width=W, height=H, title="LUMEN")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
