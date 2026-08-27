"""Open a real desktop window for a ``World.dump()`` JSON via kagra-shared wgpu 30.

This is a **subprocess** helper so wgpu 0.19 (kagra-core ``RendererV2``) and
wgpu 30 never share a process. It presents through the existing shared
``Renderer`` (the same one collectathon / mobile / ``render_world`` use).
It does **not** use kagra-core ``window.rs`` or the ``(-12800,-12800)``
fake-headless path.

Official Crest play on this path is a collectathon (capsule fallback; walker dump `gltf` / `model` is CPU-skinned Vertex3 on wgpu 30; `.vrm` is glTF-binary on the same path) (title →
play → result, pick up, count, finish) walking a World.dump. WASD +
mouse/arrows. ``examples/vrm_open_world.py`` may keep RendererV2 for leftover
MToon / spring / look-at / blendshapes. New games must not start on RendererV2.

CLI::

    python -m kagra.play_world [dump.json] [--width 960] [--height 540] [--seconds 8]
    python examples/world_doc_window.py dump.json

Helper search order (skip cleanly when none are present, or there is no display):

1. ``$KAGRA_WORLD_WINDOW`` (installed binary, or a ``.py`` stand-in)
2. ``kagra-world-window`` / ``kagra-shared-window`` on ``PATH``
3. already-built ``target/{release,debug}/examples/window``
4. ``cargo run -p kagra-shared --features render --example window -- dump.json ...``
   (CLI default; tests only use cargo when ``KAGRA_WORLD_WINDOW_CARGO=1``)

No display / no adapter / missing helper → skip (exit 0) unless ``--require``.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]

HELPER_NAMES = ("kagra-world-window", "kagra-shared-window")
EXAMPLE_REL = (
    Path("target") / "release" / "examples" / "window",
    Path("target") / "debug" / "examples" / "window",
)
DEFAULT_DUMP = Path("kagra-shared") / "tests" / "fixtures" / "crest_isle_world.json"

_NO_ADAPTER_MARKERS = (
    "no adapter",
    "no compatible adapter",
    "failed to find an appropriate adapter",
    "requestadaptererror",
    "adapter not found",
    "no suitable gpu",
    "no suitable graphics adapter",
    "noop support not compiled",
)

_NO_DISPLAY_MARKERS = (
    "no display",
    "display handle",
    "wayland",
    "could not connect",
    "cannot open display",
    "no screens found",
    "not supported",
    "oserror",
    "libxkbcommon",
    "xkbcommon",
    "failed to create surface",
    "missingdisplayhandle",
)


def repo_root() -> Path:
    return ROOT


def default_world_dump(*, root: Path | None = None) -> Path:
    """Crest Isle fixture. A live ``World.dump()`` JSON is also accepted."""
    root = Path(root) if root is not None else ROOT
    return root / DEFAULT_DUMP


def has_display() -> bool:
    """True when a real desktop is plausible. Tests skip when this is false."""
    if os.environ.get("KAGRA_WORLD_WINDOW_SKIP", "").strip().lower() in (
        "1",
        "true",
        "yes",
    ):
        return False
    if os.environ.get("KAGRA_WORLD_WINDOW_FORCE", "").strip().lower() in (
        "1",
        "true",
        "yes",
    ):
        return True
    if os.name == "nt" or sys.platform == "darwin":
        return True
    return bool(os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"))


def _is_executable_path(path: Path) -> bool:
    if not path.is_file():
        return False
    exe = path.with_suffix(".exe")
    if os.name == "nt" and not path.suffix and exe.is_file():
        return True
    return os.access(path, os.X_OK) or path.suffix.lower() in {".py", ".exe"}


def _existing_file(path: Path) -> Path | None:
    if path.is_file():
        return path
    exe = path.with_suffix(".exe")
    if exe.is_file():
        return exe
    return None


def find_window_helper(*, root: Path | None = None) -> Path | None:
    """Installed or already-built helper. ``None`` if missing (not an error)."""
    root = Path(root) if root is not None else ROOT
    env = os.environ.get("KAGRA_WORLD_WINDOW", "").strip()
    if env:
        p = Path(env)
        if not p.is_absolute():
            p = root / p
        found = _existing_file(p)
        if found is not None:
            return found
        return None
    for name in HELPER_NAMES:
        hit = shutil.which(name)
        if hit:
            return Path(hit)
    for rel in EXAMPLE_REL:
        found = _existing_file(root / rel)
        if found is not None and _is_executable_path(found):
            return found
    return None


def cargo_available(*, root: Path | None = None) -> bool:
    root = Path(root) if root is not None else ROOT
    return shutil.which("cargo") is not None and (root / "kagra-shared" / "Cargo.toml").is_file()


def helper_argv(
    helper: Path,
    world: Path,
    *,
    width: int,
    height: int,
    seconds: float | None,
) -> list[str]:
    args = [str(world), "--width", str(width), "--height", str(height)]
    if seconds is not None:
        args.extend(["--seconds", str(seconds)])
    if helper.suffix.lower() == ".py":
        return [sys.executable, str(helper), *args]
    return [str(helper), *args]


def cargo_argv(
    world: Path,
    *,
    width: int,
    height: int,
    seconds: float | None,
    root: Path | None = None,
) -> list[str]:
    root = Path(root) if root is not None else ROOT
    cargo = shutil.which("cargo") or "cargo"
    cmd = [
        cargo,
        "run",
        "-p",
        "kagra-shared",
        "--features",
        "render",
        "--example",
        "window",
        "--locked",
        "--manifest-path",
        str(root / "kagra-shared" / "Cargo.toml"),
        "--",
        str(world),
        "--width",
        str(width),
        "--height",
        str(height),
    ]
    if seconds is not None:
        cmd.extend(["--seconds", str(seconds)])
    return cmd


def resolve_window_cmd(
    world: Path,
    *,
    width: int,
    height: int,
    seconds: float | None,
    allow_cargo: bool = True,
    root: Path | None = None,
) -> list[str] | None:
    """Command to open ``world`` in a shared window, or ``None`` to skip."""
    root = Path(root) if root is not None else ROOT
    helper = find_window_helper(root=root)
    if helper is not None:
        return helper_argv(helper, world, width=width, height=height, seconds=seconds)
    if allow_cargo and cargo_available(root=root):
        return cargo_argv(world, width=width, height=height, seconds=seconds, root=root)
    return None


def walk_input_from_keys(held) -> dict:
    """Map held key names to collectathon ``WalkInput`` + look.

    WASD = wish (camera-relative). Arrows = look (race: arrows also steer+throttle; fight: arrows also walk; novel: arrows also pick a choice; stealth: arrows also walk; puzzle: arrows also walk; sports: arrows also walk; sim: arrows also walk; 2d action: arrows also walk). Space = jump (novel: page advance). J/Z/F/click = attack (novel: page advance / confirm choice; rhythm: hit on beat; fish: cast then land catch; shop: buy at stall). R = reload (FPS). Shift/C/K = dodge (fight: hold guard).
    Shared ``WorldPlay`` applies this; Python ``CharacterController`` is
    the leftover VRM motor (accel/decel / foot ring) and is not copied.
    """
    names = {str(h).strip().lower() for h in (held or ())}
    lx = (1.0 if "d" in names else 0.0) - (1.0 if "a" in names else 0.0)
    lz = (1.0 if "w" in names else 0.0) - (1.0 if "s" in names else 0.0)
    look_x = (1.0 if names & {"arrowright", "right"} else 0.0) - (
        1.0 if names & {"arrowleft", "left"} else 0.0
    )
    look_y = (1.0 if names & {"arrowup", "up"} else 0.0) - (
        1.0 if names & {"arrowdown", "down"} else 0.0
    )
    jump = bool(names & {"space", " ", "jump"})
    attack = bool(names & {"j", "z", "f", "mouse1", "click", "attack", "fire"})
    dodge = bool(names & {"shift", "c", "k", "control", "ctrl", "dodge", "r", "reload"})
    return {
        "lx": max(-1.0, min(1.0, lx)),
        "lz": max(-1.0, min(1.0, lz)),
        "look_x": max(-1.0, min(1.0, look_x)),
        "look_y": max(-1.0, min(1.0, look_y)),
        "jump": jump,
        "attack": attack,
        "dodge": dodge,
    }


def looks_like_no_adapter(text: str) -> bool:
    low = (text or "").lower()
    return any(m in low for m in _NO_ADAPTER_MARKERS)


def looks_like_no_display(text: str) -> bool:
    low = (text or "").lower()
    return any(m in low for m in _NO_DISPLAY_MARKERS)


@dataclass
class PlayWorldResult:
    ok: bool
    skipped: bool = False
    skip_reason: str | None = None
    error: str | None = None
    path: str | None = None
    cmd: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


def play_world_dump(
    world: str | Path | None = None,
    *,
    width: int = 960,
    height: int = 540,
    seconds: float | None = None,
    timeout_sec: float = 180.0,
    allow_cargo: bool = True,
    root: Path | None = None,
    cwd: str | Path | None = None,
    require_display: bool = True,
) -> PlayWorldResult:
    """Shell to the shared wgpu 30 window. Never imports wgpu 0.19."""
    root = Path(root) if root is not None else ROOT
    work = Path(cwd) if cwd is not None else root
    if world is None:
        world_p = default_world_dump(root=root)
    else:
        world_p = Path(world)
        if not world_p.is_absolute():
            world_p = work / world_p

    if not world_p.is_file():
        return PlayWorldResult(
            ok=False,
            error=f"world dump missing: {world_p}",
            path=str(world_p),
        )

    if require_display and not has_display():
        return PlayWorldResult(
            ok=True,
            skipped=True,
            skip_reason="no display (set DISPLAY / WAYLAND_DISPLAY, or KAGRA_WORLD_WINDOW_FORCE=1)",
            path=str(world_p),
        )

    cmd = resolve_window_cmd(
        world_p,
        width=int(width),
        height=int(height),
        seconds=seconds,
        allow_cargo=allow_cargo,
        root=root,
    )
    if cmd is None:
        return PlayWorldResult(
            ok=True,
            skipped=True,
            skip_reason="no shared window helper (install kagra-world-window or "
            "cargo build -p kagra-shared --features render --example window)",
            path=str(world_p),
        )

    try:
        proc = subprocess.run(
            cmd,
            cwd=str(root),
            capture_output=True,
            text=True,
            timeout=float(timeout_sec),
            encoding="utf-8",
            errors="replace",
        )
    except FileNotFoundError as e:
        return PlayWorldResult(
            ok=True,
            skipped=True,
            skip_reason=f"window helper not runnable: {e}",
            path=str(world_p),
            cmd=cmd,
        )
    except subprocess.TimeoutExpired:
        return PlayWorldResult(
            ok=False,
            error=f"window timeout after {timeout_sec}s",
            path=str(world_p),
            cmd=cmd,
        )

    log = (proc.stdout or "") + "\n" + (proc.stderr or "")
    if proc.returncode != 0:
        if looks_like_no_display(log) or looks_like_no_adapter(log):
            reason = (
                "shared window helper has no display"
                if looks_like_no_display(log)
                else "shared window helper has no GPU adapter"
            )
            return PlayWorldResult(
                ok=True,
                skipped=True,
                skip_reason=reason,
                path=str(world_p),
                cmd=cmd,
            )
        tail = log.strip()[-800:]
        return PlayWorldResult(
            ok=False,
            error=f"window exit={proc.returncode}: {tail}",
            path=str(world_p),
            cmd=cmd,
        )
    return PlayWorldResult(ok=True, path=str(world_p), cmd=cmd)


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        prog="python -m kagra.play_world",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    p.add_argument(
        "world",
        nargs="?",
        default=None,
        help="World.dump() JSON (default: Crest Isle fixture)",
    )
    p.add_argument("--width", type=int, default=960)
    p.add_argument("--height", type=int, default=540)
    p.add_argument(
        "--seconds",
        type=float,
        default=None,
        help="inject forward walk then exit (default: until Esc / close)",
    )
    p.add_argument(
        "--cargo",
        action="store_true",
        default=True,
        help="fall back to cargo run of the window example (default)",
    )
    p.add_argument(
        "--no-cargo",
        action="store_true",
        help="do not invoke cargo; skip if no installed/built helper",
    )
    p.add_argument(
        "--require",
        action="store_true",
        help="exit 1 when the helper or display is missing (default: skip / exit 0)",
    )
    p.add_argument("--timeout", type=float, default=180.0)
    args = p.parse_args(list(sys.argv[1:] if argv is None else argv))
    allow_cargo = bool(args.cargo) and not bool(args.no_cargo)
    result = play_world_dump(
        args.world,
        width=args.width,
        height=args.height,
        seconds=args.seconds,
        timeout_sec=args.timeout,
        allow_cargo=allow_cargo,
    )
    print(json.dumps(result.to_dict(), ensure_ascii=False, indent=2))
    if result.skipped and args.require:
        return 1
    return 0 if result.ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
