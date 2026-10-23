"""Runner kit shell: window, ``--headless``, ``--shots``, ``--bake``.

A runner game is a ``games/*.json`` manifest whose ``"game"`` object
overrides Neon Cat's defaults, plus ``play(manifest)``. See
``docs/runner_kit.md``.
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

from kagra.audio import set_listener
from kagra.audio import tick as audio_tick
from kagra.game_spec import ROOT, GameSpec
from kagra.gameloop import (
    Scene,
    mouse_clicked,
    mouse_pos,
    pressed,
    render_rgba,
    rgba_to_png,
    run,
    was_pressed,
)
from kagra.juice import Juice
from kagra.particles import Emitter

W, H = 1280, 720
# +lx is screen-right, which is world -X on this camera.
AIM_GAIN = 1.4
_ON = frozenset({"1", "true", "yes", "on"})


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


def _session(world_json: str):
    import kagra

    cls = getattr(kagra, "NeonCat", None)
    if cls is None:
        import kagra_shared

        cls = kagra_shared.NeonCat
    return cls.from_json(world_json)


def _clamp(v: float, lo: float = -1.0, hi: float = 1.0) -> float:
    return lo if v < lo else hi if v > hi else v


class RunnerScene(Scene):
    """Window / headless shell around the Rust runner sim."""

    def __init__(self, world_json: str) -> None:
        super().__init__()
        self.session = _session(world_json)
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


def _write(scene: RunnerScene, path: Path) -> None:
    scene.draw()
    png = rgba_to_png(scene.frame() or b"", scene.width, scene.height)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png)
    print(f"wrote {path} ({len(png)} bytes) phase={scene._flow()} score={scene.session.score}")


def _boot(world_json: str, scripted: bool = False) -> RunnerScene:
    scene = RunnerScene(world_json)
    scene._scripted = scripted
    scene.width = W
    scene.height = H
    return scene


def shots(world_json: str, directory: Path, name: str = "neon_cat") -> int:
    directory.mkdir(parents=True, exist_ok=True)
    title = _boot(world_json)
    for _ in range(90):
        title.update(1.0 / 60.0)
    _write(title, directory / f"{name}_title.png")

    play = _boot(world_json, scripted=True)
    play.session.confirm()
    for _ in range(160):
        play.update(1.0 / 60.0)
    _write(play, directory / f"{name}_play.png")
    (directory / f"{name}_play.json").write_text(json.dumps(play._picture()), encoding="utf-8")

    crash = _boot(world_json, scripted=True)
    crash.session.confirm()
    crash._crash = True
    for _ in range(220):
        crash.update(1.0 / 60.0)
        if crash._flow() == "result":
            break
    _write(crash, directory / f"{name}_result.png")
    if play.session.score < 100:
        print(f"{name} play score {play.session.score} < 100")
        return 1
    if crash._flow() != "result":
        print(f"{name} crash stayed in {crash._flow()}")
        return 1
    return 0


def headless(world_json: str, out: str, name: str = "neon_cat") -> int:
    scene = _boot(world_json, scripted=True)
    scene.session.confirm()
    for _ in range(160):
        scene.update(1.0 / 60.0)
    path = Path(out)
    _write(scene, path)
    world_path = path.parent / f"{name}_world.json"
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
        print(f"{name} headless missed the first ring or crashed")
        return 1
    return 0


def main(spec: GameSpec, world_path: str | Path, argv: list[str] | None = None) -> int:
    """``--bake`` writes the world JSON, ``--shots DIR``, ``--headless PNG``, else a window."""
    argv = sys.argv if argv is None else argv
    name = spec.id
    world_path = Path(world_path)
    if "--bake" in argv:
        print(f"wrote {spec.write(world_path)}")
        return 0
    world_json = spec.world_json()
    env_headless = _truthy_env(f"KAGRA_{name.upper()}_HEADLESS")
    if "--shots" in argv:
        idx = argv.index("--shots")
        directory = Path(argv[idx + 1] if len(argv) > idx + 1 else "docs/screenshots")
        return shots(world_json, directory, name)
    if "--headless" in argv or env_headless:
        out = f"scratch/{name}.png"
        if "--headless" in argv:
            idx = argv.index("--headless")
            if len(argv) > idx + 1 and not argv[idx + 1].startswith("--"):
                out = argv[idx + 1]
        return headless(world_json, out, name)
    run(RunnerScene(world_json), width=W, height=H, title=spec.title)
    return 0


def play(manifest: str | Path, argv: list[str] | None = None) -> int:
    """Run a game from its ``games/*.json`` manifest.

    The manifest's ``"game"`` object holds only what differs from Neon Cat;
    ``"world"`` is where ``--bake`` writes the full spec for wasm / Android.
    """
    path = Path(manifest)
    path = path if path.is_absolute() else ROOT / path
    data = json.loads(path.read_text(encoding="utf-8"))
    return main(GameSpec.from_overrides(data.get("game")), ROOT / data["world"], argv)
