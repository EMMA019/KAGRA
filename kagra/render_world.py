"""Render a ``World.dump()`` JSON via kagra-shared wgpu 30 offscreen.

This is a **subprocess** helper so wgpu 0.19 (kagra-core ``RendererV2``) and
wgpu 30 never share a process. It does not open a desktop window and does not
use the ``(-12800,-12800)`` fake-headless path.

CLI::

    python -m kagra.render_world dump.json [out.png] [--width 320] [--height 180]

Helper search order (skip cleanly when none are present):

1. ``$KAGRA_OFFSCREEN`` (installed binary, or a ``.py`` stand-in)
2. ``kagra-offscreen`` / ``kagra-shared-offscreen`` on ``PATH``
3. already-built ``target/{release,debug}/examples/offscreen``
4. ``cargo run -p kagra-shared --features render --example offscreen -- W H out.png world dump.json``
   (CLI default; verify scenarios only use cargo when ``KAGRA_OFFSCREEN_CARGO=1``
   or ``expect_offscreen.cargo`` is true)

PNG checks are smoke: file exists, non-empty, IHDR width/height. Not golden pixels.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import struct
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]

HELPER_NAMES = ("kagra-offscreen", "kagra-shared-offscreen")
EXAMPLE_REL = (
    Path("target") / "release" / "examples" / "offscreen",
    Path("target") / "debug" / "examples" / "offscreen",
)

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


def repo_root() -> Path:
    return ROOT


def png_dimensions(path: str | Path) -> tuple[int, int]:
    """Read IHDR width/height without decoding pixels (stdlib only)."""
    data = Path(path).read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"not a PNG: {path}")
    ihdr = data.find(b"IHDR")
    if ihdr < 0 or ihdr + 12 > len(data):
        raise ValueError(f"PNG IHDR missing: {path}")
    width, height = struct.unpack(">II", data[ihdr + 4 : ihdr + 12])
    return int(width), int(height)


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


def find_offscreen_helper(*, root: Path | None = None) -> Path | None:
    """Installed or already-built helper. ``None`` if missing (not an error)."""
    root = Path(root) if root is not None else ROOT
    env = os.environ.get("KAGRA_OFFSCREEN", "").strip()
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
    width: int,
    height: int,
    out: Path,
    world: Path,
) -> list[str]:
    """Match ``offscreen -- W H out.png world dump.json``."""
    args = [str(width), str(height), str(out), "world", str(world)]
    if helper.suffix.lower() == ".py":
        return [sys.executable, str(helper), *args]
    return [str(helper), *args]


def cargo_argv(
    width: int,
    height: int,
    out: Path,
    world: Path,
    *,
    root: Path | None = None,
) -> list[str]:
    root = Path(root) if root is not None else ROOT
    cargo = shutil.which("cargo") or "cargo"
    return [
        cargo,
        "run",
        "-p",
        "kagra-shared",
        "--features",
        "render",
        "--example",
        "offscreen",
        "--locked",
        "--manifest-path",
        str(root / "kagra-shared" / "Cargo.toml"),
        "--",
        str(width),
        str(height),
        str(out),
        "world",
        str(world),
    ]


def resolve_offscreen_cmd(
    width: int,
    height: int,
    out: Path,
    world: Path,
    *,
    allow_cargo: bool = True,
    root: Path | None = None,
) -> list[str] | None:
    """Command to render ``world`` → ``out``, or ``None`` to skip."""
    root = Path(root) if root is not None else ROOT
    helper = find_offscreen_helper(root=root)
    if helper is not None:
        return helper_argv(helper, width, height, out, world)
    if allow_cargo and cargo_available(root=root):
        return cargo_argv(width, height, out, world, root=root)
    return None


def looks_like_no_adapter(text: str) -> bool:
    low = (text or "").lower()
    return any(m in low for m in _NO_ADAPTER_MARKERS)


@dataclass
class RenderWorldResult:
    ok: bool
    skipped: bool = False
    skip_reason: str | None = None
    error: str | None = None
    path: str | None = None
    width: int | None = None
    height: int | None = None
    size_bytes: int | None = None
    cmd: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


def check_offscreen_png(
    path: str | Path,
    *,
    width: int,
    height: int,
    min_bytes: int = 200,
) -> list[str]:
    """Smoke checks: exists, non-empty, IHDR size. Not golden pixels."""
    p = Path(path)
    if not p.is_file():
        return [f"offscreen png missing: {p}"]
    size = p.stat().st_size
    if size <= 0:
        return [f"offscreen png empty: {p}"]
    if size < int(min_bytes):
        return [f"offscreen png too small: {p} ({size} < {min_bytes})"]
    try:
        got_w, got_h = png_dimensions(p)
    except Exception as e:
        return [f"offscreen png unreadable: {e}"]
    errors: list[str] = []
    if int(got_w) != int(width) or int(got_h) != int(height):
        errors.append(f"offscreen png size {got_w}x{got_h} != {width}x{height}")
    return errors


def render_world_dump(
    world: str | Path,
    out: str | Path,
    *,
    width: int = 320,
    height: int = 180,
    min_bytes: int = 200,
    timeout_sec: float = 180.0,
    allow_cargo: bool = True,
    root: Path | None = None,
    cwd: str | Path | None = None,
) -> RenderWorldResult:
    """Shell to the shared offscreen helper. Never imports wgpu 0.19."""
    root = Path(root) if root is not None else ROOT
    work = Path(cwd) if cwd is not None else root
    world_p = Path(world)
    if not world_p.is_absolute():
        world_p = work / world_p
    out_p = Path(out)
    if not out_p.is_absolute():
        out_p = work / out_p

    if not world_p.is_file():
        return RenderWorldResult(
            ok=False,
            error=f"world dump missing: {world_p}",
            path=str(out_p),
        )

    cmd = resolve_offscreen_cmd(
        int(width),
        int(height),
        out_p,
        world_p,
        allow_cargo=allow_cargo,
        root=root,
    )
    if cmd is None:
        return RenderWorldResult(
            ok=True,
            skipped=True,
            skip_reason="no shared offscreen helper (install kagra-offscreen or "
            "cargo build -p kagra-shared --features render --example offscreen)",
            path=str(out_p),
        )

    out_p.parent.mkdir(parents=True, exist_ok=True)
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
        return RenderWorldResult(
            ok=True,
            skipped=True,
            skip_reason=f"offscreen helper not runnable: {e}",
            path=str(out_p),
            cmd=cmd,
        )
    except subprocess.TimeoutExpired:
        return RenderWorldResult(
            ok=False,
            error=f"offscreen timeout after {timeout_sec}s",
            path=str(out_p),
            cmd=cmd,
        )

    log = (proc.stdout or "") + "\n" + (proc.stderr or "")
    if proc.returncode != 0:
        if looks_like_no_adapter(log):
            return RenderWorldResult(
                ok=True,
                skipped=True,
                skip_reason="shared offscreen helper has no GPU adapter",
                path=str(out_p),
                cmd=cmd,
            )
        tail = log.strip()[-800:]
        return RenderWorldResult(
            ok=False,
            error=f"offscreen exit={proc.returncode}: {tail}",
            path=str(out_p),
            cmd=cmd,
        )

    errors = check_offscreen_png(out_p, width=int(width), height=int(height), min_bytes=int(min_bytes))
    size = out_p.stat().st_size if out_p.is_file() else 0
    got_w = got_h = None
    if out_p.is_file() and not errors:
        try:
            got_w, got_h = png_dimensions(out_p)
        except Exception:
            pass
    return RenderWorldResult(
        ok=not errors,
        error="; ".join(errors) if errors else None,
        path=str(out_p),
        width=got_w,
        height=got_h,
        size_bytes=size,
        cmd=cmd,
    )


def eval_expect_offscreen(
    spec: dict[str, Any] | None,
    world_spec: dict[str, Any] | None,
    cwd: Path,
    *,
    root: Path | None = None,
    allow_cargo: bool | None = None,
) -> tuple[list[str], str | None, RenderWorldResult | None]:
    """Run optional scenario offscreen smoke. Skip is not a failure.

    Returns ``(errors, skip_reason, result)``.
    """
    if not spec:
        return [], None, None
    world_path = spec.get("world") or spec.get("path")
    if not world_path and world_spec:
        world_path = world_spec.get("path")
    if not world_path:
        return ["expect_offscreen.world missing (and expect_world.path missing)"], None, None

    out = spec.get("out") or spec.get("png") or "scratch/shared_offscreen.png"
    width = int(spec.get("width", 320))
    height = int(spec.get("height", 180))
    min_bytes = int(spec.get("min_bytes", spec.get("min_file_bytes", 200)))
    timeout_sec = float(spec.get("timeout_sec", 180))
    required = bool(spec.get("required", False))
    if allow_cargo is None:
        if "cargo" in spec:
            allow_cargo = bool(spec.get("cargo"))
        else:
            allow_cargo = os.environ.get("KAGRA_OFFSCREEN_CARGO", "").strip() in (
                "1",
                "true",
                "TRUE",
                "yes",
            )

    result = render_world_dump(
        world_path,
        out,
        width=width,
        height=height,
        min_bytes=min_bytes,
        timeout_sec=timeout_sec,
        allow_cargo=bool(allow_cargo),
        root=root,
        cwd=cwd,
    )
    if result.skipped:
        if required:
            return [result.skip_reason or "offscreen helper missing"], None, result
        return [], result.skip_reason, result
    if not result.ok:
        return [result.error or "offscreen failed"], None, result
    return [], None, result


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        prog="python -m kagra.render_world",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    p.add_argument("world", help="World.dump() JSON (docs/schemas/world.json)")
    p.add_argument("out", nargs="?", default="scratch/shared_offscreen.png")
    p.add_argument("--width", type=int, default=320)
    p.add_argument("--height", type=int, default=180)
    p.add_argument("--min-bytes", type=int, default=200)
    p.add_argument(
        "--cargo",
        action="store_true",
        default=True,
        help="fall back to cargo run of the offscreen example (default)",
    )
    p.add_argument(
        "--no-cargo",
        action="store_true",
        help="do not invoke cargo; skip if no installed/built helper",
    )
    p.add_argument(
        "--require",
        action="store_true",
        help="exit 1 when the helper is missing (default: skip / exit 0)",
    )
    p.add_argument("--timeout", type=float, default=180.0)
    args = p.parse_args(list(sys.argv[1:] if argv is None else argv))
    allow_cargo = bool(args.cargo) and not bool(args.no_cargo)
    result = render_world_dump(
        args.world,
        args.out,
        width=args.width,
        height=args.height,
        min_bytes=args.min_bytes,
        timeout_sec=args.timeout,
        allow_cargo=allow_cargo,
    )
    print(json.dumps(result.to_dict(), ensure_ascii=False, indent=2))
    if result.skipped and args.require:
        return 1
    return 0 if result.ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
