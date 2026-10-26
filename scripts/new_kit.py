#!/usr/bin/env python3
"""Start a new game kit from the Starter template.

    python scripts/new_kit.py lantern_run "Lantern Run"

makes a whole, playable game you then change:

    kagra-shared/src/<slug>.rs                     the kit (a copy of starter.rs, renamed)
    kagra-shared/src/lib.rs                        `pub mod <slug>;`
    kagra-shared/src/kit_host.rs                   one line in KITS (the session finds it)
    kagra-shared/tests/fixtures/<slug>_world.json  the world the page loads
    games/<slug>_minimal.json                      the manifest (title, assets, page features)
    examples/<slug>_minimal.py                     `--check` for the world

It shows up in the PR preview (`play.html?game=<slug>`) and in the CI browser smoke with no
other edit. Next: change the parts of `<slug>.rs` marked `GAME:`, then
`KAGRA_BAKE=1 cargo test -p kagra-shared --lib <slug>::` whenever the spec's defaults change.
The walk-through is docs/making_a_game.md.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = "starter"
KITS_MARK = "    // new_kit.py adds kits above this line.\n"


def camel(slug: str) -> str:
    return "".join(part.capitalize() for part in slug.split("_"))


def rust_kit(src: str, slug: str, title: str) -> str:
    head, sep, body = src.partition("\nuse serde")
    head = (
        f"//! {title}: a kit made from the Starter template (`scripts/new_kit.py`).\n"
        "//!\n"
        "//! Change the parts marked `GAME:` (the spec, the rules in `step_sim`, the look in\n"
        "//! `build_doc` and the HUD). docs/making_a_game.md walks through it.\n"
    )
    body = body.replace('title: "Starter".into()', f"title: {json.dumps(title)}.into()")
    body = re.sub(r"\bStarterSpec\b", f"{camel(slug)}Spec", body)
    body = re.sub(r"\bStarter\b", camel(slug), body)
    body = body.replace("is_starter_dump", f"is_{slug}_dump")
    body = body.replace('"starter"', f'"{slug}"').replace("starter::", f"{slug}::")
    body = body.replace("starter_world.json", f"{slug}_world.json").replace("starter spec", f"{slug} spec")
    body = body.replace("starter dump", f"{slug} dump").replace('"starter: ', f'"{slug}: ')
    body = body.replace("format!(\"starter: ", f"format!(\"{slug}: ")
    return head + sep + body


def add_module(lib: str, slug: str) -> str:
    line = f"pub mod {slug};\n"
    if line in lib:
        raise SystemExit(f"lib.rs already has {line.strip()}")
    mods = re.findall(r"^pub mod (\w+);\n", lib, re.M)
    after = [m for m in mods if m < slug]
    if after:
        anchor = f"pub mod {after[-1]};\n"
        return lib.replace(anchor, anchor + line, 1)
    first = f"pub mod {mods[0]};\n"
    return lib.replace(first, line + first, 1)


def add_kit(kit_host: str, slug: str) -> str:
    if KITS_MARK not in kit_host:
        raise SystemExit("kit_host.rs has no new_kit.py marker in KITS")
    entry = (
        f"    (crate::{slug}::is_{slug}_dump, |json| {{\n"
        f"        Ok(Box::new(crate::{slug}::{camel(slug)}::from_json(json)?))\n"
        "    }),\n"
    )
    return kit_host.replace(KITS_MARK, entry + KITS_MARK, 1)


def make(root: Path, slug: str, title: str) -> list[Path]:
    if not re.fullmatch(r"[a-z][a-z0-9_]{1,30}", slug) or slug == TEMPLATE:
        raise SystemExit(f"slug must be lower_snake_case and not {TEMPLATE!r}: {slug!r}")
    src = root / "kagra-shared/src"
    out = {
        "kit": src / f"{slug}.rs",
        "fixture": root / f"kagra-shared/tests/fixtures/{slug}_world.json",
        "manifest": root / f"games/{slug}_minimal.json",
        "example": root / f"examples/{slug}_minimal.py",
    }
    taken = [p for p in out.values() if p.exists()]
    if taken:
        raise SystemExit("already there: " + ", ".join(str(p.relative_to(root)) for p in taken))

    out["kit"].write_text(rust_kit((src / f"{TEMPLATE}.rs").read_text(encoding="utf-8"), slug, title), encoding="utf-8")
    lib = src / "lib.rs"
    lib.write_text(add_module(lib.read_text(encoding="utf-8"), slug), encoding="utf-8")
    kh = src / "kit_host.rs"
    kh.write_text(add_kit(kh.read_text(encoding="utf-8"), slug), encoding="utf-8")

    world = json.loads((root / f"kagra-shared/tests/fixtures/{TEMPLATE}_world.json").read_text(encoding="utf-8"))
    world["genre"] = slug
    world["game"]["id"] = slug
    world["game"]["title"] = title
    out["fixture"].write_text(json.dumps(world, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    man = json.loads((root / f"games/{TEMPLATE}_minimal.json").read_text(encoding="utf-8"))
    man["title"] = title
    man["entry"] = f"examples/{slug}_minimal.py"
    man["world"] = f"kagra-shared/tests/fixtures/{slug}_world.json"
    out["manifest"].write_text(json.dumps(man, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    ex = (root / f"examples/{TEMPLATE}_minimal.py").read_text(encoding="utf-8")
    ex = ex.replace('GAME_ID = "starter"', f'GAME_ID = "{slug}"')
    ex = ex.replace("Starter: the smallest whole kit (stick, stars, hazards, a timer) and the new-game template.",
                    f"{title}: made from the Starter template (scripts/new_kit.py).")
    ex = ex.replace("starter.rs", f"{slug}.rs").replace("game=starter", f"game={slug}").replace("starter::", f"{slug}::")
    out["example"].write_text(ex, encoding="utf-8")
    return [out["kit"], lib, kh, out["fixture"], out["manifest"], out["example"]]


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("slug", help="lower_snake_case id: the genre, the file names, ?game=")
    ap.add_argument("title", help="the game's display title")
    ap.add_argument("--root", type=Path, default=ROOT, help=argparse.SUPPRESS)
    args = ap.parse_args(argv)
    for path in make(args.root.resolve(), args.slug, args.title):
        print("wrote", path.relative_to(args.root.resolve()))
    print(f"next: cargo test -p kagra-shared --lib {args.slug}::  then open play.html?game={args.slug}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
