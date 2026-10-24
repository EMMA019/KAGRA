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
import json
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from kagra.distribute import (  # noqa: E402
    ManifestError,
    _expand,
    _scan_world,
    _sidecars,
    load_manifest,
)

PLAY_HTML = Path(__file__).resolve().parent / "preview_site" / "play.html"
# Opening the preview with no ?game= loads this slug. Box Hop stays in the list.
LANDING_SLUG = "neon_run"


def pages_base(owner: str, repo: str) -> str:
    """Public GitHub Pages origin for this repository. No trailing slash."""
    host = owner.strip().lower()
    name = repo.strip().split("/")[-1]
    if not host or not name:
        raise ValueError(f"need owner and repo, got {owner!r} {repo!r}")
    return f"https://{host}.github.io/{name}"


def build_preview(repo: Path, out: Path, pkg: Path | None, *, require_pkg: bool) -> dict:
    repo = repo.resolve()
    out = out.resolve()
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    games = []
    for path in sorted((repo / "games").glob("*.json")):
        manifest = load_manifest(path, repo)
        games.append(_stage_one(repo, out, manifest))
    games.sort(key=lambda g: (not g["browser"], g["slug"] != LANDING_SLUG, g["title"]))
    catalog = {"games": games}
    (out / "catalog.json").write_text(
        json.dumps(catalog, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    (out / "index.html").write_text(render_index(games), encoding="utf-8")
    shutil.copyfile(PLAY_HTML, out / "play.html")
    (out / ".nojekyll").write_bytes(b"")
    if pkg is not None:
        js = pkg / "kagra_shared.js"
        if js.is_file():
            shutil.copytree(pkg, out / "pkg")
        elif require_pkg:
            raise SystemExit(f"wasm package missing: {js} (run ./scripts/build_wasm.sh)")
    elif require_pkg:
        raise SystemExit("wasm package missing (run ./scripts/build_wasm.sh)")
    return catalog


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
        {"path": "world.json", "url": world_url},
        {"path": "kagra_game.json", "url": marker_url},
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
        files.append({"path": rel, "url": rel})
    bundle = {
        "slug": manifest.slug,
        "title": manifest.title,
        "version": manifest.version,
        "genre": genre,
        "world": "world.json",
        "files": files,
    }
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
    if audio.is_dir():
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
    args = parser.parse_args(argv)
    out = Path(args.out)
    if not out.is_absolute():
        out = ROOT / out
    pkg = Path(args.pkg)
    if not pkg.is_absolute():
        pkg = ROOT / pkg
    try:
        catalog = build_preview(ROOT, out, pkg, require_pkg=args.require_pkg)
    except ManifestError as exc:
        print(f"manifest: {exc}", file=sys.stderr)
        return 2
    browser = sum(1 for g in catalog["games"] if g["browser"])
    print(f"preview {out} ({browser} browser games)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
