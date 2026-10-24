#!/usr/bin/env python3
"""Asset ledger: which committed asset came from where, under which license.

`assets/LEDGER.json` has one entry (a "pack") per source. Every tracked asset
file (3D, audio, fonts, textures, ...) must fall under a pack's `paths` globs.
The repo is public (GitHub Pages), so an asset that is not marked
`redistributable_in_public_repo` may never be committed. Those assets belong
under the gitignored private directory named in the ledger.

    python3 tools/asset_ledger.py check            # CI check (exit 1 on error)
    python3 tools/asset_ledger.py credits --write  # regenerate CREDITS.md
    python3 tools/asset_ledger.py unverified       # entries still awaiting the owner

Stdlib only. Not imported by the wheel.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEDGER = ROOT / "assets" / "LEDGER.json"
CREDITS = ROOT / "CREDITS.md"
MANIFEST_DIR = ROOT / "tools" / "fetch_assets"

LICENSES = {"cc0", "cc-by", "standard-eula", "restricted", "custom"}
VERIFIED = {"checked", "owner_confirmation_needed"}
USES = {"personal", "commercial", "modify", "redistribute-in-game", "redistribute-standalone"}
# License types that can never be committed to a public repo.
NEVER_PUBLIC = {"standard-eula", "restricted"}
# Files with these extensions are "assets" and need a ledger entry.
ASSET_EXTS = {
    ".glb", ".gltf", ".bin", ".fbx", ".obj", ".mtl", ".blend", ".dae", ".vrm", ".vrma", ".bvh",
    ".wav", ".ogg", ".mp3", ".flac", ".ttf", ".otf", ".woff", ".woff2",
    ".hdr", ".exr", ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ktx2", ".mp4", ".webm", ".psd",
}
BOOL_FIELDS = {"attribution_required", "redistributable_in_public_repo"}
LIST_FIELDS = {"allowed_uses", "paths"}
REQUIRED_FIELDS = (
    "id", "name", "source", "author", "url", "license", "license_url", "obtained",
    "attribution_required", "redistributable_in_public_repo", "allowed_uses", "paths",
    "verified", "verified_how",
)


def glob_to_regex(pattern: str) -> re.Pattern[str]:
    """`**` crosses directories, `*` and `?` do not."""
    out, i = [], 0
    while i < len(pattern):
        ch = pattern[i]
        if pattern.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
            continue
        if pattern.startswith("**", i):
            out.append(".*")
            i += 2
            continue
        out.append("[^/]*" if ch == "*" else "[^/]" if ch == "?" else re.escape(ch))
        i += 1
    return re.compile("^" + "".join(out) + "$")


def matches(path: str, patterns: list[str]) -> bool:
    return any(glob_to_regex(p).match(path) for p in patterns)


def tracked_files(root: Path = ROOT) -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", "-z"], cwd=root, capture_output=True, check=True
    ).stdout.decode("utf-8")
    return [p for p in out.split("\0") if p]


def load_ledger(path: Path = LEDGER) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def is_asset(path: str, ledger: dict) -> bool:
    if matches(path, ledger.get("exempt_paths", [])):
        return False
    if matches(path, ledger.get("extra_required_paths", [])):
        return True
    return Path(path).suffix.lower() in ASSET_EXTS


def check_pack_fields(pack: dict, private_dir: str) -> list[str]:
    pid = pack.get("id", "?")
    errs = []
    for key in REQUIRED_FIELDS:
        if key not in pack:
            errs.append(f"{pid}: missing field {key}")
        elif key in BOOL_FIELDS:
            if not isinstance(pack[key], bool):
                errs.append(f"{pid}: {key} must be true or false")
        elif key in LIST_FIELDS:
            if not isinstance(pack[key], list) or not pack[key]:
                errs.append(f"{pid}: {key} must be a non-empty list")
        elif not isinstance(pack[key], str) or not pack[key].strip():
            errs.append(f"{pid}: {key} must be a non-empty string")
    lic = pack.get("license")
    if lic not in LICENSES:
        errs.append(f"{pid}: license {lic!r} is not one of {sorted(LICENSES)}")
    if pack.get("verified") not in VERIFIED:
        errs.append(f"{pid}: verified must be one of {sorted(VERIFIED)}")
    bad_uses = [u for u in pack.get("allowed_uses", []) if u not in USES]
    if bad_uses:
        errs.append(f"{pid}: unknown allowed_uses {bad_uses}")
    redistributable = pack.get("redistributable_in_public_repo")
    if lic in NEVER_PUBLIC and redistributable:
        errs.append(f"{pid}: {lic} assets cannot be redistributable_in_public_repo")
    if redistributable is False:
        outside = [p for p in pack.get("paths", []) if not p.startswith(private_dir)]
        if outside:
            errs.append(f"{pid}: not public-redistributable, so paths must be under {private_dir}: {outside}")
    if pack.get("attribution_required") and not str(pack.get("attribution_text") or "").strip():
        errs.append(f"{pid}: attribution_required is true but attribution_text is empty")
    if lic == "cc-by" and not pack.get("attribution_required"):
        errs.append(f"{pid}: cc-by requires attribution_required=true")
    if lic == "cc0" and not (pack.get("license_evidence") or pack.get("license_url")):
        errs.append(f"{pid}: cc0 needs license_evidence or license_url")
    for ev in pack.get("license_evidence", []):
        if not (ROOT / ev).is_file():
            errs.append(f"{pid}: license_evidence file {ev} does not exist")
    if pack.get("receipt_ref") and re.search(r"[/\\]|\.(pdf|png|jpe?g|eml)$", str(pack["receipt_ref"]), re.I):
        errs.append(f"{pid}: receipt_ref must be an opaque id, not a file path (no receipts in the repo)")
    return errs


def check(
    ledger: dict,
    files: list[str],
    credits_text: str | None,
    manifest_dir: Path | None = MANIFEST_DIR,
) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    warnings: list[str] = []
    private_dir = ledger.get("private_dir", "assets-private/")
    packs = ledger.get("packs", [])
    ids = [p.get("id") for p in packs]
    for dup in {i for i in ids if ids.count(i) > 1}:
        errors.append(f"duplicate ledger id {dup}")
    for pack in packs:
        errors += check_pack_fields(pack, private_dir)

    # 1. nothing under the private dir is ever tracked
    for f in files:
        if f.startswith(private_dir):
            errors.append(f"{f}: tracked file under the private asset dir {private_dir}")

    # 2. every asset file is covered; non-redistributable packs match nothing tracked
    for f in files:
        owners = [p for p in packs if matches(f, p.get("paths", []))]
        for p in owners:
            if p.get("redistributable_in_public_repo") is False:
                errors.append(f"{f}: committed, but ledger {p['id']} is not public-redistributable")
        if is_asset(f, ledger) and not owners:
            errors.append(f"{f}: asset file has no ledger entry")
    for pack in packs:
        if pack.get("redistributable_in_public_repo") and not any(
            matches(f, pack.get("paths", [])) for f in files
        ):
            warnings.append(f"{pack['id']}: no tracked file matches its paths (stale entry?)")
        if pack.get("verified") == "owner_confirmation_needed":
            warnings.append(f"{pack['id']}: awaiting owner confirmation: {pack.get('notes', '')}")

    # 3. fetch_assets manifests must agree with the ledger
    for manifest in sorted(manifest_dir.glob("*.json")) if manifest_dir else []:
        for mp in json.loads(manifest.read_text(encoding="utf-8")).get("packs", []):
            dest = str(mp.get("dest", "")).rstrip("/") + "/x"
            owner = [p for p in packs if matches(dest, p.get("paths", []))]
            if not owner:
                errors.append(f"{manifest.name}: pack {mp.get('id')} ({mp.get('dest')}) has no ledger entry")
                continue
            spdx = owner[0].get("license_spdx")
            if mp.get("license") != spdx:
                errors.append(
                    f"{manifest.name}: pack {mp.get('id')} license {mp.get('license')} != ledger {spdx}"
                )

    # 4. CC-BY attribution must be in CREDITS.md; CREDITS.md must be current
    if credits_text is not None:
        for pack in packs:
            text = str(pack.get("attribution_text") or "").strip()
            if pack.get("attribution_required") and text and text not in credits_text:
                errors.append(f"{pack['id']}: attribution text is not in CREDITS.md")
        if credits_text != render_credits(ledger):
            errors.append("CREDITS.md is out of date: run tools/asset_ledger.py credits --write")
    return errors, warnings


def render_credits(ledger: dict) -> str:
    lines = [
        "# Credits and asset licenses",
        "",
        "Generated from `assets/LEDGER.json` by `tools/asset_ledger.py credits --write`. Do not edit by hand.",
        "",
        "Every third-party asset committed to this public repository is listed below with its",
        "license. Assets under other terms (paid, EULA, restricted) are never committed; see",
        "`docs/asset_ledger.md`.",
        "",
    ]
    packs = sorted(ledger["packs"], key=lambda p: (p["source"], p["id"]))
    lines += ["| Pack | Author | License | Attribution | Where |", "|---|---|---|---|---|"]
    for p in packs:
        lic = p.get("license_spdx") or p["license"]
        attr = "required" if p.get("attribution_required") else "not required"
        where = ", ".join(f"`{x}`" for x in p["paths"][:2]) + (" …" if len(p["paths"]) > 2 else "")
        lines.append(f"| [{p['name']}]({p['url']}) | {p['author']} | {lic} | {attr} | {where} |")
    lines.append("")
    needed = [p for p in packs if p.get("attribution_required")]
    if needed:
        lines += ["## Required attribution", ""]
        for p in needed:
            lines.append(f"- {p['attribution_text']}")
        lines.append("")
    credited = [p for p in packs if p.get("credits")]
    for p in credited:
        lines += [f"## {p['name']}: per-asset authors", ""]
        for name, author in sorted(p["credits"].items()):
            lines.append(f"- {name}: {author}")
        lines.append("")
    waiting = [p for p in packs if p.get("verified") == "owner_confirmation_needed"]
    if waiting:
        lines += ["## Provenance not yet confirmed", ""]
        for p in waiting:
            lines.append(f"- `{p['id']}`: {p.get('notes', '')}")
        lines.append("")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("check")
    cr = sub.add_parser("credits")
    cr.add_argument("--write", action="store_true")
    sub.add_parser("unverified")
    args = ap.parse_args(argv)
    ledger = load_ledger()
    if args.cmd == "credits":
        text = render_credits(ledger)
        if args.write:
            CREDITS.write_text(text, encoding="utf-8")
            print(f"wrote {CREDITS.relative_to(ROOT)}")
        else:
            sys.stdout.write(text)
        return 0
    if args.cmd == "unverified":
        for p in ledger["packs"]:
            if p["verified"] != "checked":
                print(f"{p['id']}: {p.get('notes', '')}")
        return 0
    credits_text = CREDITS.read_text(encoding="utf-8") if CREDITS.is_file() else ""
    errors, warnings = check(ledger, tracked_files(), credits_text)
    for w in warnings:
        print(f"warning: {w}")
    for e in errors:
        print(f"error: {e}")
    n_assets = sum(1 for f in tracked_files() if is_asset(f, ledger))
    print(f"{len(ledger['packs'])} ledger entries, {n_assets} asset files, "
          f"{len(errors)} errors, {len(warnings)} warnings")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
