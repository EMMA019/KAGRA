"""Lumen — neon endless flyer.

Procedural geometry only (boxes, spheres, rings). Sound is synthesized
``kagra.audio`` tones plus the repo's existing CC0 clips in ``assets/audio``
(see ``assets/audio/LICENSE.txt``). No new third-party meshes.

    python examples/neon_run_minimal.py
    python examples/neon_run_minimal.py --headless scratch/neon_run.png
    python examples/neon_run_minimal.py --shots docs/screenshots

WASD or arrows steer. Shift or Space boosts. Drag on the window (or a
phone) aims the ship. P pauses. Fly through the glowing rings. Hitting
the wall or missing a ring ends the run.

Phone picture (on unless ``KAGRA_QUALITY=0``): ``apply_look(world, "night")``,
``cull``, ``prop_lod``, juice, sparks, and ``kagra.Trail`` (emissive quad
ribbon). Glow is dump ``emissive``. Reflections are the night hemisphere
and ``env_map: preset:studio``. The wasm / Android build runs the same
Rust sim from ``games/neon_run_minimal.json``.
"""
from __future__ import annotations

import json
import os
import sys
from collections import deque
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import kagra  # noqa: E402
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
from kagra.juice import Juice, apply_look  # noqa: E402
from kagra.particles import Emitter, Trail  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/neon_run_world.json"
W, H = 1280, 720


def _session():
    cls = kagra.NeonRun
    if cls is None:
        from kagra_shared import NeonRun as cls  # type: ignore[no-redef]
    return cls.from_json(DUMP.read_text(encoding="utf-8"))


def _clamp(v: float, lo: float = -1.0, hi: float = 1.0) -> float:
    return lo if v < lo else hi if v > hi else v


class Lumen(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _session()
        self.world = json.loads(self.session.dump())
        self.quality = os.environ.get("KAGRA_QUALITY", "1") != "0"
        self.juice = Juice()
        self.sparks = Emitter(max_count=28, seed=7, name="burst", emissive=4.2)
        self.trail = Trail(max_points=16, width=0.40, color=(255, 186, 255), name="trail")
        # Real samples. Trail measures length in XY and faces +Z, so the
        # forward step is folded into Y when the ribbon is rebuilt.
        self._path: deque[tuple[float, float, float]] = deque(maxlen=16)
        self._music = False
        self._scripted = False

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
        lx = (1.0 if pressed("d") or pressed("right") else 0.0) - (
            1.0 if pressed("a") or pressed("left") else 0.0
        )
        ly = (1.0 if pressed("w") or pressed("up") else 0.0) - (
            1.0 if pressed("s") or pressed("down") else 0.0
        )
        boost = pressed("shift") or pressed("space") or pressed("k")
        if mouse_down() and self._flow() == "play":
            mx, my = mouse_pos()
            # Top-right is the PAUSE button drawn by the sim.
            if not (mx > self.width * 0.78 and my < self.height * 0.16):
                lx = _clamp(lx + (mx / max(1, self.width) - 0.5) * 2.1)
                ly = _clamp(ly + (0.5 - my / max(1, self.height)) * 2.1)
        return _clamp(lx), _clamp(ly), boost

    def _aim(self) -> tuple[float, float, bool]:
        state = json.loads(self.session.state_json())
        if state.get("phase") != "play" or not state.get("alive", True):
            return 0.0, 0.0, False
        dx = float(state["next_x"]) - float(state["x"])
        dy = float(state["next_y"]) - float(state["y"])
        # +lx is screen-right. The chase camera looks along +Z, so that is world -X.
        return _clamp(-dx * 1.6), _clamp(dy * 1.6), False

    def update(self, dt: float) -> None:
        if not self._scripted:
            self._ui_key()
        flow = self._flow()
        if flow != "play":
            lx, ly, boost = 0.0, 0.0, False
        else:
            lx, ly, boost = self._steer()
        dt_sim = self.juice.tick(dt) if self.quality else dt
        self.session.set_input(lx, ly, boost and flow == "play")
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
        set_listener(float(pos[0]), float(pos[1]), float(pos[2]), 0.0, 0.0, 1.0)

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
        if self._flow() == "play":
            self._path.append(ship)
            self._rebuild_trail()
        else:
            self._path.clear()
            self.trail.clear()
        if int(feel.get("orbs") or 0) > 0:
            self.juice.squash(0.28, 0.16)
            self.juice.shake(0.08, 0.14)
            self.sparks.burst(ship, count=12, color=(255, 236, 160), speed=2.4, life=0.4, size=0.12, upward=1.6)
            se("coin")
            play_se("coin", x=ship[0], y=ship[1], z=ship[2])
        if feel.get("near"):
            self.juice.shake(0.05, 0.1)
        if feel.get("crash"):
            self.juice.hitstop(0.08)
            self.juice.shake(0.28, 0.4)
            self.sparks.burst(ship, count=18, color=(255, 60, 150), speed=3.4, life=0.55, size=0.16, upward=2.2)
            se("hit")
        self.sparks.tick(dt)

    def _rebuild_trail(self) -> None:
        """Feed ``kagra.Trail``. Quads face the camera, so Z motion is folded into Y."""
        self.trail.clear()
        if len(self._path) < 2:
            return
        _hx, _hy, hz = self._path[-1]
        for x, y, z in self._path:
            self.trail.push((x, y - (hz - z) * 0.40, z))

    def _picture(self) -> dict:
        picture = json.loads(json.dumps(self.world))
        if not self.quality:
            for key in ("bloom", "bloom_threshold", "vignette", "lut", "lut_strength", "fxaa"):
                picture.pop(key, None)
            return picture
        self.juice.apply_world(picture)
        kept = [
            prop
            for prop in (picture.get("props") or [])
            if prop.get("name") not in {"trail", "wake"}
        ]
        picture["props"] = kept
        sx, sy, sz = self.juice.scale()
        for prop in picture["props"]:
            if str(prop.get("name", "")).startswith("ship"):
                sc = prop.get("scale") or [1, 1, 1]
                prop["scale"] = [float(sc[0]) * sx, float(sc[1]) * sy, float(sc[2]) * sz]
        picture["props"].extend(self.sparks.props())
        apply_look(picture, "night")
        picture["cull"] = True
        picture["prop_lod"] = 80.0
        picture["props"].extend(self.trail.props())
        return picture

    def draw(self) -> None:
        hud = json.loads(self.session.hud_json(self.width, self.height))
        self._frame_rgba = render_rgba(self._picture(), self.width, self.height, hud)


def _write(scene: Lumen, path: Path) -> None:
    scene.draw()
    png = rgba_to_png(scene._frame_rgba or b"", scene.width, scene.height)
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
    crash._scripted = False
    for _ in range(90):
        crash.session.set_input(1.0, 0.0, False)
        crash.session.tick(1.0 / 60.0)
        crash.world = json.loads(crash.session.dump())
        crash._feel(1.0 / 60.0)
        if crash._flow() == "result":
            break
    _write(crash, directory / "neon_run_result.png")
    if crash.session.score < 40:
        print("lumen result did not score")
        return 1
    if play.session.score < 50:
        print("lumen did not score")
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
        print("lumen did not score")
        return 1
    return 0


def main() -> int:
    if "--shots" in sys.argv:
        idx = sys.argv.index("--shots")
        directory = Path(sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "docs/screenshots")
        return shots(directory)
    headless_flag = "--headless" in sys.argv or os.environ.get("KAGRA_NEON_HEADLESS", "").strip() in {
        "1",
        "true",
        "yes",
    }
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
