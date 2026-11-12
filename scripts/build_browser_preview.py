#!/usr/bin/env python3
"""Stage ``games/*.json`` into a static browser preview.

Relative URLs only, so the same tree works at ``/`` and ``/pr/<number>/``.
Games with a ``world`` dump run in wasm WorldPlay. Assets are stored once
at the site root (only files the dump references, plus ``assets/audio`` and
``bundle_assets``). The others are listed and not linked: the browser runtime
does not execute their Python rules.

    python scripts/build_browser_preview.py --out dist/preview
    python scripts/build_browser_preview.py --out dist/preview --require-pkg
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from kagra.distribute import (  # noqa: E402
    _ASSET_KEYS,
    _ASSET_SUFFIXES,
    ManifestError,
    _expand,
    _scan_world,
    _sidecars,
    load_manifest,
)

SITE_DIR = Path(__file__).resolve().parent / "preview_site"
PLAY_HTML = SITE_DIR / "play.html"
# Classic scripts play.html loads next to itself: page strings and the portal shim.
SITE_SCRIPTS = ("i18n.js", "quality.js", "portal.js", "meta.js")
# Dev-only scripts (?dev=1 measurement overlay): the preview site gets them, a portal zip never does.
DEV_SCRIPTS = ("perf.js",)
# Opening the preview with no ?game= loads this slug. Box Hop stays in the list.
LANDING_SLUG = "neon_run"


def pages_base(owner: str, repo: str) -> str:
    """Public GitHub Pages origin for this repository. No trailing slash."""
    host = owner.strip().lower()
    name = repo.strip().split("/")[-1]
    if not host or not name:
        raise ValueError(f"need owner and repo, got {owner!r} {repo!r}")
    return f"https://{host}.github.io/{name}"


def build_preview(
    repo: Path,
    out: Path,
    pkg: Path | None,
    *,
    require_pkg: bool,
    pack_slugs: list[str] | tuple[str, ...] = (),
    pack_roots: list[Path] | tuple[Path, ...] = (),
    pack_release: bool = False,
    pack_compress: bool = True,
) -> dict:
    repo = repo.resolve()
    out = out.resolve()
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    games = []
    for path in sorted((repo / "games").glob("*.json")):
        manifest = load_manifest(path, repo)
        games.append(_stage_one(repo, out, manifest))
    for slug in pack_slugs:
        _apply_pack(
            repo,
            out,
            slug,
            [Path(r) for r in pack_roots],
            release=pack_release,
            compress=pack_compress,
        )
    games.sort(key=lambda g: (not g["browser"], g["slug"] != LANDING_SLUG, g["title"]))
    catalog = {"games": games}
    (out / "catalog.json").write_text(
        json.dumps(catalog, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    (out / "index.html").write_text(render_index(games), encoding="utf-8")
    shutil.copyfile(PLAY_HTML, out / "play.html")
    for name in SITE_SCRIPTS + DEV_SCRIPTS:
        shutil.copyfile(SITE_DIR / name, out / name)
    (out / ".nojekyll").write_bytes(b"")
    if pkg is not None:
        js = pkg / "kagra_shared.js"
        if js.is_file():
            shutil.copytree(pkg, out / "pkg")
        elif require_pkg:
            raise SystemExit(f"wasm package missing: {js} (run ./scripts/build_wasm.sh)")
    elif require_pkg:
        raise SystemExit("wasm package missing (run ./scripts/build_wasm.sh)")
    stamp_build(out)
    return catalog


BUILD_TOKEN = "__KAGRA_BUILD__"
# Classic scripts next to the page (i18n, portal, meta): stamped here, not in the source.
_SCRIPT_SRC = re.compile(r'<script src="(\./[\w.]+\.js)"')


def file_tag(data: bytes) -> str:
    """Short content hash for a ``?v=`` query on one asset."""
    return hashlib.sha256(data).hexdigest()[:10]


def stamp_build(out: Path) -> str:
    """One build id for the whole staged site, so a deploy never mixes old and new.

    The id is a hash of every staged file (the page is hashed with its token still in
    it). It goes into play.html (meta, script, wasm and bundle.json URLs), into each
    bundle.json as ``build`` and into ``build.json``, which the page re-reads with
    ``no-store`` to notice that it is older than the site. Assets carry their own
    content tag as ``v`` in bundle.json. Nothing here touches the network.
    """
    digest = hashlib.sha256()
    for path in sorted(p for p in out.rglob("*") if p.is_file()):
        rel = path.relative_to(out).as_posix()
        if rel == "build.json":
            continue
        digest.update(rel.encode("utf-8") + b"\0")
        digest.update(path.read_bytes())
    build = digest.hexdigest()[:12]
    play = out / "play.html"
    if play.is_file():
        play.write_text(
            _SCRIPT_SRC.sub(
                lambda m: f'<script src="{m.group(1)}?v={build}"',
                play.read_text(encoding="utf-8").replace(BUILD_TOKEN, build),
            ),
            encoding="utf-8",
        )
    for bundle_path in sorted((out / "games").glob("*/bundle.json")):
        bundle = json.loads(bundle_path.read_text(encoding="utf-8"))
        bundle["build"] = build
        bundle_path.write_text(
            json.dumps(bundle, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
    (out / "build.json").write_text(json.dumps({"build": build}) + "\n", encoding="utf-8")
    return build


def _load_pack_builder():
    import importlib.util

    spec = importlib.util.spec_from_file_location(
        "make_asset_pack", Path(__file__).with_name("make_asset_pack.py")
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _world_asset_refs(node: object, found: set[str]) -> None:
    """Asset paths a world dump references (same keys the staging scan uses)."""
    if isinstance(node, dict):
        for key, value in node.items():
            if key in _ASSET_KEYS and isinstance(value, str):
                found.add(value)
            else:
                _world_asset_refs(value, found)
    elif isinstance(node, list):
        for item in node:
            _world_asset_refs(item, found)


def _apply_pack(
    repo: Path,
    out: Path,
    slug: str,
    extra_roots: list[Path],
    *,
    release: bool,
    compress: bool,
) -> None:
    """Fold one staged game's files into a single asset pack (see make_asset_pack).

    The bundle keeps its ``files`` list as the page's fallback unless
    ``release`` drops it — a release build must not leave per-file URLs for
    the packed assets, so loose staged files no other game references are
    removed too. Bundle files the repo could not stage (models that live in
    an asset-supply root, never in the repo) and assets the world references
    but staging missed are resolved against ``extra_roots``.
    """
    pack_builder = _load_pack_builder()
    bundle_path = out / "games" / slug / "bundle.json"
    if not bundle_path.is_file():
        raise SystemExit(f"asset pack: no staged bundle for game {slug!r}")
    bundle = json.loads(bundle_path.read_text(encoding="utf-8"))
    roots = [out, *extra_roots, repo]
    entries = pack_builder.resolve_bundle_entries(bundle, roots)
    have = {name for name, _ in entries}
    # Assets the world references but the bundle does not list: staging only
    # lists files the repo has, and a packed game's models may exist solely
    # in a supply root. Anything resolvable joins the pack under its world
    # path (the mount key the engine asks for).
    world_name = bundle.get("world") or "world.json"
    world_bytes = dict(entries).get(world_name)
    refs: set[str] = set()
    added: list[tuple[str, bytes]] = []
    if world_bytes is not None:
        try:
            _world_asset_refs(json.loads(world_bytes.decode("utf-8")), refs)
        except (UnicodeDecodeError, json.JSONDecodeError):
            refs = set()
        for ref in sorted(refs):
            if ref in have or ".." in Path(ref).parts:
                continue
            if "/" not in ref and not ref.lower().endswith(_ASSET_SUFFIXES):
                continue
            for root in roots:
                cand = root / ref
                if cand.is_file():
                    data = cand.read_bytes()
                    entries.append((ref, data))
                    added.append((ref, data))
                    have.add(ref)
                    break
    # One model under two names: staging name-resolves a world reference to
    # the repo's canonical path, and the supply root also carries the model
    # under the world path itself. Identical bytes under both names are dead
    # weight — keep the name the world asks for (a direct mount hit, no alias
    # lookup). Same-named files whose bytes differ keep both entries.
    by_hash: dict[str, list[str]] = {}
    for name, data in entries:
        by_hash.setdefault(hashlib.sha256(data).hexdigest(), []).append(name)
    drop: set[str] = set()
    for names in by_hash.values():
        referenced = [n for n in names if n in refs]
        if len(names) > 1 and len(referenced) == 1:
            drop.update(n for n in names if n != referenced[0])
    if drop:
        entries = [(n, d) for n, d in entries if n not in drop]
    info = pack_builder.pack_info(entries, compress=compress)
    pack_rel = f"games/{slug}/assets.pack.zip"
    (out / pack_rel).write_bytes(info.pop("blob"))
    bundle["pack"] = {
        "url": pack_rel,
        "v": info["v"],
        "format": "zip",
        "entries": info["entries"],
    }
    if not release:
        # A dev bundle keeps a complete files fallback: what the pack took
        # from a supply root is staged loose too, listed under its pack
        # (world) path, so the fallback boots the same game the pack does.
        listed = {
            (item.get("path") or "")
            for item in bundle.get("files") or []
            if isinstance(item, dict)
        }
        for name, data in added:
            if name in listed:
                continue
            target = out / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            bundle.setdefault("files", []).append(
                {"path": name, "url": name, "v": file_tag(data)}
            )
            listed.add(name)
    if release:
        own_urls = {
            (item.get("url") or item.get("path") or "")
            for item in bundle.get("files") or []
            if isinstance(item, dict)
        }
        shared: set[str] = set()
        for other in (out / "games").glob("*/bundle.json"):
            if other == bundle_path:
                continue
            for item in json.loads(other.read_text(encoding="utf-8")).get("files") or []:
                if isinstance(item, dict):
                    shared.add(item.get("url") or item.get("path") or "")
        for rel in sorted(own_urls - shared):
            target = out / rel
            if rel and target.is_file():
                target.unlink()
        bundle.pop("files", None)
    bundle_path.write_text(
        json.dumps(bundle, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    mode = "release (no files fallback)" if release else "with files fallback"
    print(
        f"asset pack {slug}: {info['entries']} entries, "
        f"{info['bytes'] / 1e6:.2f} MB pack / {info['content_bytes'] / 1e6:.2f} MB content, {mode}"
    )


def _stage_one(repo: Path, out: Path, manifest) -> dict:
    browser = bool(manifest.world)
    genre = _genre(repo, manifest.world)
    entry = {
        "slug": manifest.slug,
        "title": manifest.title,
        "version": manifest.version,
        "browser": browser,
        "genre": genre,
        "entry": manifest.entry,
        "world": manifest.world,
        "href": f"play.html?game={manifest.slug}" if browser else None,
        "icon": None,
    }
    if not browser:
        return entry
    dest = out / "games" / manifest.slug
    dest.mkdir(parents=True, exist_ok=True)
    world_url = f"games/{manifest.slug}/world.json"
    (out / world_url).write_bytes((repo / manifest.world).read_bytes())
    marker = {
        "title": manifest.title,
        "version": manifest.version,
        "slug": manifest.slug,
        "entry": manifest.entry,
        "world": manifest.world,
    }
    marker_url = f"games/{manifest.slug}/kagra_game.json"
    (out / marker_url).write_text(
        json.dumps(marker, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    # path = where WorldPlay looks, relative to the extracted game root.
    # url = where Pages stores the bytes (assets are shared across games).
    files = [
        {"path": "world.json", "url": world_url, "v": file_tag((out / world_url).read_bytes())},
        {"path": "kagra_game.json", "url": marker_url, "v": file_tag((out / marker_url).read_bytes())},
    ]
    for asset in _preview_assets(repo, manifest):
        try:
            rel = asset.resolve().relative_to(repo.resolve()).as_posix()
        except ValueError:
            continue
        target = out / rel
        if not target.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(asset, target)
        files.append({"path": rel, "url": rel, "v": file_tag(target.read_bytes())})
    bundle = {
        "slug": manifest.slug,
        "title": manifest.title,
        "version": manifest.version,
        "genre": genre,
        "world": "world.json",
        "files": files,
    }
    if manifest.platform:
        bundle["platform"] = manifest.platform
    if manifest.meta:
        bundle["meta"] = manifest.meta
    if manifest.page:
        bundle["page"] = manifest.page
    (dest / "bundle.json").write_text(
        json.dumps(bundle, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    icon_url = f"games/{manifest.slug}/icon.png"
    shutil.copy2(repo / manifest.icon, out / icon_url)
    entry["icon"] = icon_url
    return entry


def _preview_assets(repo: Path, manifest) -> list[Path]:
    """World-referenced files only. The Python scan pulls whole libraries."""
    repo = repo.resolve()
    found: set[Path] = set(_scan_world(repo, manifest.world))
    for rel in manifest.bundle_assets:
        path = (repo / rel).resolve()
        if path == (repo / "assets").resolve():
            continue
        found.update(_expand(path))
    audio = repo / "assets" / "audio"
    # A game that names its own audio packs (bundle_assets entries inside assets/audio)
    # ships those and what its world references, not every clip in the repo: the
    # blanket scan added ~2.5 MB of other games' music and sfx to such a bundle.
    names_packs = any(
        audio.resolve() in (repo / rel).resolve().parents for rel in manifest.bundle_assets
    )
    if audio.is_dir() and not names_packs:
        for child in audio.rglob("*"):
            if child.is_file() and child.suffix.lower() in {".wav", ".ogg", ".mp3"}:
                found.add(child.resolve())
    expanded: set[Path] = set()
    for path in found:
        expanded.update(_sidecars(path))
    return sorted(path for path in expanded if path.is_file())


def _genre(repo: Path, world: str | None) -> str:
    if not world:
        return ""
    try:
        data = json.loads((repo / world).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return ""
    return str(data.get("genre") or "")


def render_index(games: list[dict]) -> str:
    def esc(value: object) -> str:
        return (
            str(value)
            .replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace('"', "&quot;")
        )

    playable = [g for g in games if g.get("browser")]
    quiet = [g for g in games if not g.get("browser")]
    featured = next((g for g in playable if g["slug"] == LANDING_SLUG), None)
    if featured is None and playable:
        featured = playable[0]
    feature_html = ""
    if featured is not None:
        feature_html = (
            f'<p class="feature"><a class="go" href="{esc(featured["href"])}">'
            f'{esc(featured["title"])} を開く</a></p>'
        )
    rows = []
    for game in playable:
        icon = ""
        if game.get("icon"):
            icon = f'<img src="{esc(game["icon"])}" alt="" width="40" height="40" />'
        genre = f' · {esc(game["genre"])}' if game.get("genre") else ""
        rows.append(
            "<li>"
            f'<a href="{esc(game["href"])}">{icon}<span>'
            f'<strong>{esc(game["title"])}</strong>'
            f'<small>{esc(game["slug"])}{genre}</small></span></a></li>'
        )
    quiet_rows = "".join(f"<li>{esc(g['title'])}（{esc(g['slug'])}）</li>" for g in quiet)
    quiet_html = ""
    if quiet_rows:
        quiet_html = (
            "<h2>ブラウザでは進まない例</h2>"
            "<p class='note'>world が無いので WorldPlay では動かない。ルールは Python 側。</p>"
            f"<ul class='quiet'>{quiet_rows}</ul>"
        )
    body = "\n".join(rows)
    return f"""<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover" />
  <meta name="robots" content="noindex" />
  <title>KAGRA プレビュー</title>
  <style>
    :root {{ color-scheme: dark; }}
    body {{
      margin: 0;
      font-family: system-ui, "Hiragino Sans", "Noto Sans JP", sans-serif;
      background: #121722;
      color: #f4f1ea;
    }}
    main {{ max-width: 720px; margin: 0 auto; padding: 24px 16px 48px; }}
    h1 {{ font-size: 28px; margin: 0 0 8px; }}
    .note {{ opacity: 0.8; line-height: 1.5; }}
    a.go {{
      display: inline-block;
      margin: 8px 0 18px;
      padding: 14px 20px;
      border-radius: 999px;
      background: #f0c36a;
      color: #1a1408;
      font-weight: 700;
      text-decoration: none;
    }}
    ul {{ list-style: none; padding: 0; margin: 0; }}
    li a {{
      display: flex;
      gap: 12px;
      align-items: center;
      padding: 12px 4px;
      color: inherit;
      text-decoration: none;
      border-bottom: 1px solid rgba(255,255,255,.12);
    }}
    img {{ border-radius: 8px; background: #000; object-fit: cover; }}
    small {{ display: block; opacity: 0.65; margin-top: 2px; }}
    .quiet li {{ padding: 6px 0; opacity: 0.75; }}
  </style>
</head>
<body>
  <main>
    <h1>KAGRA プレビュー</h1>
    <p class="note">スマホのブラウザで開く。左スティックで移動、右下でジャンプ、右上でアクション。縦でも横でも同じ操作です。ケーブルは要りません。</p>
    {feature_html}
    <h2>ブラウザで遊べる例</h2>
    <ul>{body}</ul>
    {quiet_html}
  </main>
</body>
</html>
"""


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build the KAGRA browser preview site")
    parser.add_argument("--out", default="dist/preview")
    parser.add_argument("--pkg", default="kagra-shared/www/pkg")
    parser.add_argument("--require-pkg", action="store_true")
    parser.add_argument(
        "--pack",
        action="append",
        default=[],
        metavar="SLUG",
        help="also fold this game's files into an asset pack (repeatable)",
    )
    parser.add_argument(
        "--pack-root",
        action="append",
        default=[],
        metavar="DIR",
        help="asset-supply root for pack files the repo does not stage (repeatable)",
    )
    parser.add_argument(
        "--pack-release",
        action="store_true",
        help="packed bundles drop their files list and unshared loose files",
    )
    parser.add_argument(
        "--pack-store", action="store_true", help="write packs uncompressed (default: deflate)"
    )
    args = parser.parse_args(argv)
    out = Path(args.out)
    if not out.is_absolute():
        out = ROOT / out
    pkg = Path(args.pkg)
    if not pkg.is_absolute():
        pkg = ROOT / pkg
    pack_roots = [Path(r) if Path(r).is_absolute() else ROOT / r for r in args.pack_root]
    try:
        catalog = build_preview(
            ROOT,
            out,
            pkg,
            require_pkg=args.require_pkg,
            pack_slugs=args.pack,
            pack_roots=pack_roots,
            pack_release=args.pack_release,
            pack_compress=not args.pack_store,
        )
    except ManifestError as exc:
        print(f"manifest: {exc}", file=sys.stderr)
        return 2
    browser = sum(1 for g in catalog["games"] if g["browser"])
    print(f"preview {out} ({browser} browser games)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
