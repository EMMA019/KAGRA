#!/usr/bin/env python3
"""Write ``<slug>.html`` entry pages at a site root that forward to ``latest/``.

``latest/play.html?game=<slug>`` is the stable address of a game on the hosted
site. ``<slug>.html`` is the short form (``neon_cat.html``). Extra query
parameters (``?debug=1``) are kept.
"""
from __future__ import annotations

import argparse
import re
from pathlib import Path

SLUG = re.compile(r"^[a-z0-9_]+$")

TEMPLATE = """<!doctype html>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{slug}</title>
<meta http-equiv="refresh" content="0; url=latest/play.html?game={slug}">
<script>
  location.replace("latest/play.html?game={slug}" + location.search.replace(/^\\?/, "&") + location.hash);
</script>
<a href="latest/play.html?game={slug}">{slug}</a>
"""


def write_entry(root: Path, slug: str) -> Path:
    if not SLUG.match(slug):
        raise SystemExit(f"bad slug: {slug!r}")
    if not (root / "latest" / "games" / slug / "world.json").is_file():
        raise SystemExit(f"latest/games/{slug}/world.json is missing under {root}")
    path = root / f"{slug}.html"
    path.write_text(TEMPLATE.format(slug=slug), encoding="utf-8")
    return path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default="dist/web")
    parser.add_argument("slugs", nargs="+")
    args = parser.parse_args(argv)
    for slug in args.slugs:
        print("wrote", write_entry(Path(args.root), slug))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
