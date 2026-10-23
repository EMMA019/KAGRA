"""Neon Cat — synthwave runner.

The cat, the dungeon arches, the racing hoops, and the low obstacles are
CC0 glTF. Credits live next to the files (``assets/models/**/CREDITS.md``)
and the collect step is ``tools/fetch_assets.py`` (see ``docs/assets.md``).
The floor grid, wall wireframe, and magenta rails are emissive fat lines.

    python examples/neon_cat_minimal.py
    python examples/neon_cat_minimal.py --headless scratch/neon_cat.png
    python examples/neon_cat_minimal.py --shots docs/screenshots

``--shots`` writes title / play / crash. ``KAGRA_NEON_CAT_HEADLESS=1`` is
the same path as ``--headless``.

A and D (or the stick) steer. Screen-right is world −X because the chase
camera looks along +Z. Space, W, or stick-up jumps. Rings score. A low
crate, barrel, or barrier ends the run.

The wasm / Android build runs this same Rust sim from
``games/neon_cat_minimal.json``. ``KAGRA_QUALITY=0`` drops bloom on the
picture the window draws. The dump's own grade is what phones use.
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
        "Could not import kagra. Run Neon Cat from a checkout of this repo "
        f"so {_ROOT} is on sys.path."
    ) from exc
from kagra.audio import set_listener  # noqa: E402
from kagra.audio import tick as audio_tick  # noqa: E402
from kagra.gameloop import (  # noqa: E402
    Scene,
    mouse_clicked,
    mouse_pos,
    pressed,
    render_rgba,
    rgba_to_png,
    run,
    was_pressed,
)
from kagra.juice import Juice  # noqa: E402
from kagra.particles import Emitter  # noqa: E402

DUMP = _ROOT / "kagra-shared/tests/fixtures/neon_cat_world.json"
W, H = 1280, 720
# +lx is screen-right, which is world -X on this camera.
AIM_GAIN = 1.4
_ON = frozenset({"1", "true", "yes", "on"})


def _truthy_env(name: str, *, default: bool = False, only_off: str | None = None) -> bool:
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
    cls = getattr(kagra, "NeonCat", None)
    if cls is None:
        import kagra_shared

        cls = kagra_shared.NeonCat
    return cls.from_json(DUMP.read_text(encoding="utf-8"))


def _clamp(v: float, lo: float = -1.0, hi: float = 1.0) -> float:
    return lo if v < lo else hi if v > hi else v


class NeonCatRun(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _session()
        self.world = json.loads(self.session.dump())
        self.quality = _truthy_env("KAGRA_QUALITY", default=True, only_off="0")
        self.juice = Juice()
        self.sparks = Emitter(max_count=36, seed=3, name="burst", emissive=1.8)
        self._scripted = False
        self._crash = False

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
        lx = (1.0 if any(pressed(key) for key in ("d", "right")) else 0.0) - (
            1.0 if any(pressed(key) for key in ("a", "left")) else 0.0
        )
        jump = any(pressed(key) for key in ("space", "w", "up"))
        return _clamp(lx), 0.0, jump

    def _aim(self) -> tuple[float, float, bool]:
        if self._crash:
            return 1.0, 0.0, False
        sim = self.world.get("sim") if isinstance(self.world, dict) else None
        if not isinstance(sim, dict):
            return 0.0, 0.0, False
        if sim.get("phase") != "play" or not sim.get("alive", True):
            return 0.0, 0.0, False
        dx = float(sim["next_ring_x"]) - float(sim["x"])
        lx = _clamp(-dx * AIM_GAIN)
        ahead = float(sim["next_obs_z"]) - float(sim["z"])
        aligned = abs(float(sim["x"]) - float(sim["next_obs_x"])) < 0.95
        jump = 0.15 < ahead < 2.5 and aligned
        return lx, 0.0, jump

    def update(self, dt: float) -> None:
        if not self._scripted:
            self._ui_key()
        if self._flow() == "play":
            lx, lz, jump = self._steer()
        else:
            lx, lz, jump = 0.0, 0.0, False
        self.session.set_input(lx, lz, jump)
        self.session.tick(dt)
        self.world = json.loads(self.session.dump())
        self._audio(dt)
        self._feel(dt)

    def _audio(self, dt: float) -> None:
        audio_tick(dt)
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

    def _cat(self) -> tuple[float, float, float]:
        player = self.world.get("player") if isinstance(self.world, dict) else None
        if isinstance(player, dict):
            p = player.get("position") or [0, 0, 0]
            return float(p[0]), float(p[1]), float(p[2])
        return 0.0, 0.0, 0.0

    def _feel(self, dt: float) -> None:
        if not self.quality:
            return
        feel = json.loads(self.session.take_feel())
        cat = self._cat()
        if int(feel.get("rings") or 0) > 0:
            self.juice.squash(0.18, 0.12)
            self.sparks.burst(
                cat, count=16, color=(80, 255, 255), speed=4.0, life=0.4, size=0.16, upward=1.4
            )
        if feel.get("jump"):
            self.juice.squash(0.12, 0.1)
        if feel.get("crash"):
            self.juice.shake(0.22, 0.35)
            self.sparks.burst(
                cat, count=20, color=(255, 50, 170), speed=4.2, life=0.5, size=0.18, upward=2.0
            )
        self.sparks.tick(dt)
        if self.quality:
            self.juice.tick(dt)

    def _picture(self) -> dict:
        src = self.world if isinstance(self.world, dict) else {}
        picture = dict(src)
        picture["cameras"] = [
            dict(cam) if isinstance(cam, dict) else cam for cam in (src.get("cameras") or [])
        ]
        picture["props"] = list(src.get("props") or [])
        player = src.get("player")
        if isinstance(player, dict):
            picture["player"] = dict(player)
        if not self.quality:
            for key in ("bloom", "bloom_threshold", "vignette", "lut", "lut_strength", "fxaa"):
                picture.pop(key, None)
            return picture
        self.juice.apply_world(picture)
        picture["props"] = list(picture.get("props") or [])
        picture["props"].extend(self.sparks.props())
        return picture

    def draw(self) -> None:
        hud = json.loads(self.session.hud_json(self.width, self.height))
        self._frame_rgba = render_rgba(self._picture(), self.width, self.height, hud)


def _write(scene: NeonCatRun, path: Path) -> None:
    scene.draw()
    png = rgba_to_png(scene.frame() or b"", scene.width, scene.height)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png)
    print(f"wrote {path} ({len(png)} bytes) phase={scene._flow()} score={scene.session.score}")


def _boot(scripted: bool = False) -> NeonCatRun:
    scene = NeonCatRun()
    scene._scripted = scripted
    scene.width = W
    scene.height = H
    return scene


def shots(directory: Path) -> int:
    directory.mkdir(parents=True, exist_ok=True)
    title = _boot()
    for _ in range(90):
        title.update(1.0 / 60.0)
    _write(title, directory / "neon_cat_title.png")

    play = _boot(scripted=True)
    play.session.confirm()
    for _ in range(160):
        play.update(1.0 / 60.0)
    _write(play, directory / "neon_cat_play.png")
    (directory / "neon_cat_play.json").write_text(json.dumps(play._picture()), encoding="utf-8")

    crash = _boot(scripted=True)
    crash.session.confirm()
    crash._crash = True
    for _ in range(220):
        crash.update(1.0 / 60.0)
        if crash._flow() == "result":
            break
    _write(crash, directory / "neon_cat_result.png")
    if play.session.score < 100:
        print(f"neon cat play score {play.session.score} < 100")
        return 1
    if crash._flow() != "result":
        print(f"neon cat crash stayed in {crash._flow()}")
        return 1
    return 0


def headless(out: str) -> int:
    scene = _boot(scripted=True)
    scene.session.confirm()
    for _ in range(160):
        scene.update(1.0 / 60.0)
    path = Path(out)
    _write(scene, path)
    world_path = path.parent / "neon_cat_world.json"
    world_path.write_text(json.dumps(scene._picture()), encoding="utf-8")
    print(
        json.dumps(
            {
                "score": scene.session.score,
                "rings": scene.session.rings,
                "phase": scene._flow(),
                "alive": scene.session.alive,
            }
        )
    )
    if scene.session.score < 100 or not scene.session.alive:
        print("neon cat headless missed the first ring or crashed")
        return 1
    return 0


def main() -> int:
    shots_flag = "--shots" in sys.argv
    headless_flag = "--headless" in sys.argv or _truthy_env("KAGRA_NEON_CAT_HEADLESS")
    if shots_flag and headless_flag:
        print("neon cat: --shots wins over --headless", file=sys.stderr)
    if shots_flag:
        idx = sys.argv.index("--shots")
        directory = Path(sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "docs/screenshots")
        return shots(directory)
    if headless_flag:
        out = "scratch/neon_cat.png"
        if "--headless" in sys.argv:
            idx = sys.argv.index("--headless")
            if len(sys.argv) > idx + 1 and not sys.argv[idx + 1].startswith("--"):
                out = sys.argv[idx + 1]
        return headless(out)
    run(NeonCatRun(), width=W, height=H, title="NEON CAT")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
