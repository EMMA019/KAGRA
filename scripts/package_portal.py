#!/usr/bin/env python3
"""Build a single-game zip for a free web-game portal (CrazyGames, GameMonetize, ...).

    python scripts/package_portal.py --game neon_cat_minimal --portal crazygames
    python scripts/package_portal.py --game neon_cat_minimal --wasm-opt   # smaller wasm

The zip has ``index.html`` at its root, that game's files only, and relative
paths throughout, so it runs from any folder or iframe:

    index.html            play.html with the portal build flags in <meta>
    i18n.js portal.js     page strings and the portal shim
    pkg/                  kagra_shared.js and kagra_shared_bg.wasm
    games/<slug>/         bundle.json, world.json, kagra_game.json
    assets/...            only what the game's world.json uses (audio is pruned
                          to the clips it names; models and textures come from
                          the manifest's bundle_assets)

No catalog, no link to the other games, and no external request unless a portal
is chosen: the SDK script URL appears only inside portal.js and is only loaded
when ``--portal`` (the ``kagra-portal`` meta) or ``?portal=`` selects it.

It prints a size table (raw and gzip) and checks the budget:

    initial  page + scripts + wasm + the files the page loads before the first frame
    total    everything in the zip (audio loads after the first frame)

A budget is exceeded when either the raw or the gzip number is over it, because
portals do not agree on which one they count. Over budget prints WARN and still
exits 0; ``--strict`` makes it exit 1. A text file with an unexpected
``http(s)://`` URL fails the build.
"""
from __future__ import annotations

import argparse
import gzip
import json
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
for _p in (ROOT, ROOT / "scripts"):
    if str(_p) not in sys.path:
        sys.path.insert(0, str(_p))

from build_browser_preview import PLAY_HTML, SITE_DIR, SITE_SCRIPTS, _stage_one  # noqa: E402

from kagra.distribute import ManifestError, manifest_for_game  # noqa: E402

PORTALS = ("none", "crazygames", "gamemonetize")
MB = 1024 * 1024
INITIAL_BUDGET = 5 * MB
TOTAL_BUDGET = 8 * MB
PKG_FILES = ("kagra_shared.js", "kagra_shared_bg.wasm")
AUDIO_SUFFIXES = (".wav", ".ogg", ".mp3")
# Zip entries get one fixed time so the same input gives the same bytes.
ZIP_TIME = (2026, 1, 1, 0, 0, 0)
# Code a browser runs. Licence text (.txt) may name a licence URL; nothing fetches it.
CODE_SUFFIXES = (".html", ".js", ".json", ".css", ".svg")
_URL_RE = re.compile(r"https?://[^\s\"'<>)\\`]+")
# Namespace identifiers, not requests.
_NAMESPACE_URLS = ("http://www.w3.org/",)
_HIDE_RE = re.compile(r"<(\w+)\b[^>]*\bdata-portal=\"hide\"[^>]*>.*?</\1>\s*", re.S)


class PackageError(RuntimeError):
    pass


# ---- the page ----------------------------------------------------------------


def portal_index_html(source: str, slug: str, portal: str) -> str:
    """play.html as the zip's index.html: build flags in, catalog links out."""
    flags = (
        '  <meta name="kagra-build" content="portal" />\n'
        f'  <meta name="kagra-game" content="{slug}" />\n'
        f'  <meta name="kagra-portal" content="{portal}" />\n'
    )
    html = source.replace('  <meta name="robots" content="noindex" />\n', "", 1)
    marker = '  <meta charset="utf-8" />\n'
    if marker not in html:
        raise PackageError("play.html changed: no charset meta to anchor the build flags")
    html = html.replace(marker, marker + flags, 1)
    html, hidden = _HIDE_RE.subn("", html)
    if hidden < 1:
        raise PackageError("play.html changed: no data-portal=\"hide\" element to strip")
    return html


# ---- staging -----------------------------------------------------------------


def _strings(node):
    if isinstance(node, str):
        yield node
    elif isinstance(node, dict):
        for value in node.values():
            yield from _strings(value)
    elif isinstance(node, list):
        for value in node:
            yield from _strings(value)


def prune_audio(files: list[dict], world: object) -> list[dict]:
    """Drop audio clips the world never names, and licence files of folders left empty."""
    named = set(_strings(world))

    def is_audio(f: dict) -> bool:
        return f["path"].lower().endswith(AUDIO_SUFFIXES)

    def folder(f: dict) -> str:
        return Path(f["path"]).parent.as_posix()

    had = {folder(f) for f in files if is_audio(f)}
    kept = [f for f in files if not is_audio(f) or f["path"] in named]
    left = {folder(f) for f in kept if is_audio(f)}
    return [
        f for f in kept
        if not (Path(f["path"]).name.upper().startswith("LICENSE") and folder(f) in had and folder(f) not in left)
    ]


def run_wasm_opt(tool: str, src: Path, dst: Path) -> None:
    proc = subprocess.run([tool, "-Oz", "-o", str(dst), str(src)], capture_output=True, text=True)
    if proc.returncode != 0:
        raise PackageError(f"wasm-opt failed:\n{proc.stderr.strip()[-800:]}")


def find_wasm_opt(asked: str | None) -> str:
    tool = shutil.which(asked or "wasm-opt")
    if not tool:
        raise PackageError(f"{asked or 'wasm-opt'} not found (install binaryen, or pass --wasm-opt /path/to/wasm-opt)")
    return tool


def stage(repo: Path, manifest, pkg: Path, out: Path, portal: str, wasm_opt: str | None) -> dict:
    """Write the zip's tree into ``out``. Returns facts for the report."""
    if not manifest.world:
        raise PackageError(f"{manifest.slug} has no world: it is not a browser game")
    out.mkdir(parents=True, exist_ok=True)
    _stage_one(repo, out, manifest)
    gdir = out / "games" / manifest.slug
    (gdir / "icon.png").unlink(missing_ok=True)
    bundle = json.loads((gdir / "bundle.json").read_text(encoding="utf-8"))
    world = json.loads((gdir / "world.json").read_text(encoding="utf-8"))
    bundle["files"] = prune_audio(bundle["files"], world)
    keep = {f["url"] for f in bundle["files"]}
    # _stage_one copied every listed asset; remove what the pruned list no longer names.
    for path in sorted((out / "assets").rglob("*")) if (out / "assets").is_dir() else []:
        if path.is_file() and path.relative_to(out).as_posix() not in keep:
            path.unlink()
    for path in sorted((out / "assets").rglob("*"), reverse=True) if (out / "assets").is_dir() else []:
        if path.is_dir() and not any(path.iterdir()):
            path.rmdir()
    (gdir / "bundle.json").write_text(json.dumps(bundle, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    (out / "index.html").write_text(
        portal_index_html(PLAY_HTML.read_text(encoding="utf-8"), manifest.slug, portal), encoding="utf-8"
    )
    for name in SITE_SCRIPTS:
        shutil.copyfile(SITE_DIR / name, out / name)
    (out / "pkg").mkdir()
    for name in PKG_FILES:
        src = pkg / name
        if not src.is_file():
            raise PackageError(f"wasm package missing: {src} (run ./scripts/build_wasm.sh)")
        shutil.copyfile(src, out / "pkg" / name)
    facts: dict = {"wasm_opt": None}
    if wasm_opt:
        wasm = out / "pkg" / "kagra_shared_bg.wasm"
        before = wasm.stat().st_size
        tmp = wasm.with_suffix(".opt")
        run_wasm_opt(wasm_opt, wasm, tmp)
        tmp.replace(wasm)
        facts["wasm_opt"] = {"before": before, "after": wasm.stat().st_size}
    return facts


# ---- checks ------------------------------------------------------------------


def external_urls(root: Path) -> list[tuple[str, str]]:
    """(file, url) for every http(s) URL in a code file, namespace identifiers aside."""
    found = []
    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.suffix.lower() not in CODE_SUFFIXES:
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for url in _URL_RE.findall(text):
            if not url.startswith(_NAMESPACE_URLS):
                found.append((path.relative_to(root).as_posix(), url.rstrip(".,;")))
    return found


def unexpected_urls(root: Path) -> list[tuple[str, str]]:
    """URLs other than the portal SDK addresses that portal.js itself names."""
    sdk = _sdk_urls()
    return [(f, u) for f, u in external_urls(root) if not (f == "portal.js" and u in sdk)]


def _sdk_urls() -> set[str]:
    text = (SITE_DIR / "portal.js").read_text(encoding="utf-8")
    block = re.search(r"const SDK_URL = \{(.*?)\};", text, re.S)
    return set(_URL_RE.findall(block.group(1))) if block else set()


def broken_paths(root: Path) -> list[str]:
    """Bundle entries and page links that point outside the zip or at nothing."""
    problems = []
    for bundle_path in (root / "games").glob("*/bundle.json"):
        bundle = json.loads(bundle_path.read_text(encoding="utf-8"))
        for item in bundle.get("files", []):
            url = item["url"]
            if url.startswith(("/", "http", "..")) or ".." in Path(url).parts:
                problems.append(f"{bundle_path.name}: url not relative: {url}")
            elif not (root / url).is_file():
                problems.append(f"{bundle_path.name}: missing {url}")
    html = (root / "index.html").read_text(encoding="utf-8")
    for ref in re.findall(r"""(?:src|href)=["']([^"']+)["']""", html):
        if ref.startswith(("/", "http", "#")):
            problems.append(f"index.html: not a relative path: {ref}")
        elif not (root / ref.split("?")[0]).exists():
            problems.append(f"index.html: missing {ref}")
    return problems


# ---- sizes -------------------------------------------------------------------


def _gz(data: bytes) -> int:
    return len(gzip.compress(data, 9, mtime=0))


def measure(root: Path) -> dict:
    """Raw and gzip bytes per file, then the initial and total sums."""
    bundle = json.loads(next((root / "games").glob("*/bundle.json")).read_text(encoding="utf-8"))
    sizes: dict[str, tuple[int, int]] = {}
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        data = path.read_bytes()
        sizes[path.relative_to(root).as_posix()] = (len(data), _gz(data))
    # The page fetches every bundle file before the first frame except audio,
    # which the Web Audio side loads once the game is on screen.
    lazy = {u["url"] for u in bundle["files"] if u["url"].lower().endswith(AUDIO_SUFFIXES)}
    initial = [k for k in sizes if k not in lazy]
    total = list(sizes)
    pick = lambda keys, col: sum(sizes[k][col] for k in keys)  # noqa: E731
    return {
        "files": sizes,
        "initial": {"raw": pick(initial, 0), "gzip": pick(initial, 1)},
        "total": {"raw": pick(total, 0), "gzip": pick(total, 1)},
        "audio": {"raw": pick(lazy, 0), "gzip": pick(lazy, 1)},
    }


def budget_warnings(m: dict, initial: int = INITIAL_BUDGET, total: int = TOTAL_BUDGET) -> list[str]:
    out = []
    for label, limit in (("initial", initial), ("total", total)):
        for basis in ("raw", "gzip"):
            got = m[label][basis]
            if got > limit:
                out.append(f"{label} {basis} {got / MB:.2f} MB is over the {limit / MB:.0f} MB budget")
    return out


def write_zip(root: Path, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    names = sorted(p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file())
    names.sort(key=lambda n: (n != "index.html", n))
    with zipfile.ZipFile(dest, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for name in names:
            info = zipfile.ZipInfo(name, ZIP_TIME)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            zf.writestr(info, (root / name).read_bytes(), compresslevel=9)


def report(m: dict, zip_path: Path | None, facts: dict, warnings: list[str]) -> str:
    mb = lambda n: f"{n / MB:6.2f} MB"  # noqa: E731
    lines = [f"{'':10s}{'raw':>11s}{'gzip':>11s}"]
    for label in ("initial", "total"):
        lines.append(f"{label:10s}{mb(m[label]['raw']):>11s}{mb(m[label]['gzip']):>11s}")
    lines.append(f"{'audio':10s}{mb(m['audio']['raw']):>11s}{mb(m['audio']['gzip']):>11s}  (loads after the first frame)")
    if zip_path is not None:
        lines.append(f"zip       {zip_path.stat().st_size / MB:6.2f} MB  {zip_path}")
    if facts.get("wasm_opt"):
        w = facts["wasm_opt"]
        lines.append(f"wasm-opt -Oz: {w['before'] / MB:.2f} MB -> {w['after'] / MB:.2f} MB raw")
    biggest = sorted(m["files"].items(), key=lambda kv: -kv[1][0])[:5]
    lines.append("largest: " + ", ".join(f"{k} {v[0] / 1024:.0f} KB" for k, v in biggest))
    lines += [f"WARN {w}" for w in warnings]
    return "\n".join(lines)


# ---- main --------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Build a single-game portal zip")
    ap.add_argument("--game", default="neon_cat_minimal", help="games/<name>.json stem, or examples/<name>.py")
    ap.add_argument("--portal", choices=PORTALS, default="none",
                    help="portal built into the page (default none: no SDK is ever requested)")
    ap.add_argument("--pkg", default="kagra-shared/www/pkg", help="wasm-bindgen output folder")
    ap.add_argument("--out", default="dist/portal", help="folder for the zip and, with --keep-tree, the tree")
    ap.add_argument("--wasm-opt", nargs="?", const="wasm-opt", default=None, metavar="PATH",
                    help="shrink the wasm with binaryen's wasm-opt -Oz (needs binaryen)")
    ap.add_argument("--keep-tree", action="store_true", help="also leave the unzipped tree next to the zip")
    ap.add_argument("--strict", action="store_true", help="exit 1 when a size budget is exceeded")
    ap.add_argument("--initial-budget-mb", type=float, default=INITIAL_BUDGET / MB)
    ap.add_argument("--total-budget-mb", type=float, default=TOTAL_BUDGET / MB)
    args = ap.parse_args(argv)

    try:
        manifest = manifest_for_game(ROOT, args.game)
        tool = find_wasm_opt(args.wasm_opt) if args.wasm_opt else None
    except (ManifestError, PackageError) as exc:
        print(f"package_portal: {exc}", file=sys.stderr)
        return 2
    out = Path(args.out)
    out = out if out.is_absolute() else ROOT / out
    pkg = Path(args.pkg)
    pkg = pkg if pkg.is_absolute() else ROOT / pkg
    suffix = "" if args.portal == "none" else f"_{args.portal}"
    zip_path = out / f"{manifest.slug}{suffix}_v{manifest.version}.zip"

    with tempfile.TemporaryDirectory(prefix="kagra-portal-") as tmp:
        tree = Path(tmp) / manifest.slug
        try:
            facts = stage(ROOT, manifest, pkg, tree, args.portal, tool)
        except (PackageError, FileNotFoundError) as exc:
            print(f"package_portal: {exc}", file=sys.stderr)
            return 2
        problems = broken_paths(tree)
        problems += [f"{f}: external URL {u}" for f, u in unexpected_urls(tree)]
        if problems:
            print("package_portal: the tree is not portal-safe:", file=sys.stderr)
            for line in problems:
                print(f"  {line}", file=sys.stderr)
            return 1
        m = measure(tree)
        write_zip(tree, zip_path)
        if args.keep_tree:
            kept = out / manifest.slug
            if kept.exists():
                shutil.rmtree(kept)
            shutil.copytree(tree, kept)
    warnings = budget_warnings(m, int(args.initial_budget_mb * MB), int(args.total_budget_mb * MB))
    print(report(m, zip_path, facts, warnings))
    return 1 if (warnings and args.strict) else 0


if __name__ == "__main__":
    raise SystemExit(main())
