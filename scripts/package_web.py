#!/usr/bin/env python3
"""Stage a phone-browser build of a KAGRA game.

    python scripts/package_web.py --game examples/neon_run_minimal.py --skip-wasm
    python scripts/package_web.py --game examples/neon_run_minimal.py --build-wasm

The default page is ``play.html`` (Lumen). Corridor Haul stays at
``corridor.html``. ``world.json`` is the manifest's dump, which wasm loads
with ``loadGameJson``. Audio clips from ``bundle_assets`` land in ``audio/``.
GLB and glTF files keep their repo-relative paths and are listed in
``assets.json`` so ``play.html`` can ``mountFile`` them before the first frame.
A model that fails to fetch is omitted; the sim draws the primitive instead.

GitHub Pages (``.github/workflows/pages.yml``) commits this folder to the
``gh-pages`` root on push to master and leaves ``pr/`` in place. The live
site is the full tree, deployed by ``pages-deploy.yml``.
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from kagra.distribute import ManifestError, manifest_for_game  # noqa: E402

_AUDIO = {".wav", ".ogg", ".mp3"}
_MODEL = {".glb", ".gltf", ".bin", ".png", ".jpg", ".jpeg", ".webp"}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Package a KAGRA game for the browser")
    parser.add_argument(
        "--game",
        default="examples/neon_run_minimal.py",
        help="examples/foo_minimal.py or games/foo.json (default: Lumen)",
    )
    parser.add_argument("--out", default="dist/web", help="output directory")
    parser.add_argument("--build-wasm", action="store_true", help="run scripts/build_wasm.sh first")
    parser.add_argument(
        "--skip-wasm",
        action="store_true",
        help="stage the page without pkg/ (layout check; the page will not boot)",
    )
    args = parser.parse_args(argv)
    if args.build_wasm and args.skip_wasm:
        print("pass only one of --build-wasm and --skip-wasm", file=sys.stderr)
        return 2
    try:
        manifest = manifest_for_game(ROOT, args.game)
    except ManifestError as exc:
        print(f"manifest: {exc}", file=sys.stderr)
        return 2

    if args.build_wasm:
        script = ROOT / "scripts" / "build_wasm.sh"
        proc = subprocess.run(["bash", str(script)], cwd=ROOT)
        if proc.returncode != 0:
            return proc.returncode

    out = Path(args.out)
    if not out.is_absolute():
        out = ROOT / out
    try:
        rels = stage_web(ROOT, manifest, out, include_wasm=not args.skip_wasm)
    except FileNotFoundError as exc:
        print(exc, file=sys.stderr)
        return 2
    print(f"staged {len(rels)} files -> {out}")
    print(f"play:  python -m http.server -d {out} 8000")
    print("open:  http://localhost:8000/play.html")
    return 0


def stage_web(repo: Path, manifest, dest: Path, *, include_wasm: bool) -> list[str]:
    """Copy the wasm shell, the game dump, and bundled audio."""
    www = repo / "kagra-shared" / "www"
    if not (www / "play.html").is_file():
        raise FileNotFoundError(f"missing {www / 'play.html'}")
    pkg = www / "pkg"
    if include_wasm and not (pkg / "kagra_shared.js").is_file():
        raise FileNotFoundError(
            f"missing {pkg}. Build it with ./scripts/build_wasm.sh or pass --skip-wasm."
        )
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    rels: list[str] = []
    for item in sorted(www.iterdir()):
        if item.name in {"pkg", "audio", "world.json"}:
            continue
        target = dest / item.name
        if item.is_dir():
            shutil.copytree(item, target)
            rels.extend(str(p.relative_to(dest)) for p in target.rglob("*") if p.is_file())
        else:
            shutil.copy2(item, target)
            rels.append(item.name)
    if include_wasm:
        shutil.copytree(pkg, dest / "pkg")
        rels.extend(str(p.relative_to(dest)) for p in (dest / "pkg").rglob("*") if p.is_file())
    if manifest.world:
        src = repo / manifest.world
        if not src.is_file():
            raise FileNotFoundError(f"world dump missing: {src}")
        shutil.copy2(src, dest / "world.json")
        rels.append("world.json")
    audio = dest / "audio"
    audio.mkdir()
    mounted: list[str] = []
    for rel in manifest.bundle_assets:
        src = repo / rel
        files = [src] if src.is_file() else list(src.rglob("*")) if src.is_dir() else []
        for path in files:
            if not path.is_file():
                continue
            suffix = path.suffix.lower()
            if suffix in _AUDIO:
                shutil.copy2(path, audio / path.name)
                rels.append(f"audio/{path.name}")
                continue
            if suffix not in _MODEL:
                continue
            try:
                repo_rel = path.resolve().relative_to(repo.resolve()).as_posix()
            except ValueError:
                continue
            target = dest / repo_rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
            rels.append(repo_rel)
            mounted.append(repo_rel)
    (dest / "assets.json").write_text(
        json.dumps(sorted(set(mounted)), indent=2) + "\n",
        encoding="utf-8",
    )
    rels.append("assets.json")
    note = (
        f"{manifest.title}\n"
        "Open play.html over http (a file:// page cannot load wasm).\n"
        "python -m http.server -d <this folder> 8000\n"
        "Keyboard: WASD or arrows, Shift or Space boosts, P pauses, Enter starts.\n"
        "Touch: drag to steer, BOOST button, tap the title to start.\n"
        "Sound clips are the CC0 tones in assets/audio/LICENSE.txt.\n"
        "Ship and obstacles are Kenney Space Kit GLBs (CC0). See assets.json.\n"
        "If a model fails to load, the game draws a sphere or a box instead.\n"
    )
    (dest / "README.txt").write_text(note, encoding="utf-8")
    rels.append("README.txt")
    return rels


if __name__ == "__main__":
    raise SystemExit(main())
