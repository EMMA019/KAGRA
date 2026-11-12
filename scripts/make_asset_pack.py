#!/usr/bin/env python3
"""Build one game's asset pack: a single ZIP the browser page fetches whole.

The pack is the release shape for games whose models may be *incorporated*
into the game but not redistributed as loose files (Mixamo terms): the page
never sees a per-model URL, it fetches ``assets.pack.zip`` once, expands it
in memory and mounts every entry under its bundle path — the same mount keys
the per-file staging used, so the engine cannot tell the difference.

Entries are ``(mount key, bytes)`` pairs, usually a staged ``bundle.json``'s
``files`` list resolved against one or more roots (the staged site first,
then any asset-supply roots — the models live outside the repo). The ZIP is
written with a fixed timestamp and sorted entries, and the returned ``v`` is
a content hash over ``(name, sha256(bytes))`` pairs, so it changes exactly
when the packed content changes and slots into the page's ``?v=`` scheme.

    python scripts/make_asset_pack.py --bundle dist/preview/games/<slug>/bundle.json \
        --root dist/preview --root ~/asset-supply --out pack.zip [--store]
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import sys
import zipfile
from pathlib import Path

# DOS epoch: the earliest ZIP timestamp, so the archive bytes depend only on content.
FIXED_DATE = (1980, 1, 1, 0, 0, 0)


def pack_version(entries: list[tuple[str, bytes]]) -> str:
    """Short content hash over the sorted (name, content) pairs."""
    digest = hashlib.sha256()
    for name, data in sorted(entries):
        digest.update(name.encode("utf-8") + b"\0")
        digest.update(hashlib.sha256(data).hexdigest().encode("ascii") + b"\n")
    return digest.hexdigest()[:16]


def build_pack(entries: list[tuple[str, bytes]], *, compress: bool = True) -> bytes:
    """The pack archive bytes for ``entries`` (mount key → bytes)."""
    buf = io.BytesIO()
    compression = zipfile.ZIP_DEFLATED if compress else zipfile.ZIP_STORED
    with zipfile.ZipFile(buf, "w", compression=compression) as zf:
        for name, data in sorted(entries):
            info = zipfile.ZipInfo(name, date_time=FIXED_DATE)
            info.compress_type = compression
            info.external_attr = 0o644 << 16
            zf.writestr(info, data)
    return buf.getvalue()


def pack_info(entries: list[tuple[str, bytes]], *, compress: bool = True) -> dict:
    """Build the pack in memory and describe it (what bundle.json declares)."""
    blob = build_pack(entries, compress=compress)
    return {
        "v": pack_version(entries),
        "entries": len(entries),
        "bytes": len(blob),
        "content_bytes": sum(len(data) for _, data in entries),
        "blob": blob,
    }


def resolve_bundle_entries(bundle: dict, roots: list[Path]) -> list[tuple[str, bytes]]:
    """A bundle's ``files`` list as (mount key, bytes), resolved against roots.

    Each file is looked up as ``root/<url>`` then ``root/<path>`` for every
    root in order. Anything unresolved is a hard error: a pack that silently
    drops a file would boot a game that only fails later, on a phone.
    """
    entries: list[tuple[str, bytes]] = []
    missing: list[str] = []
    for item in bundle.get("files") or []:
        if isinstance(item, str):
            path = url = item
        else:
            path, url = item.get("path") or "", item.get("url") or ""
        data = None
        for root in roots:
            for rel in (url, path):
                if not rel:
                    continue
                cand = root / rel
                if cand.is_file():
                    data = cand.read_bytes()
                    break
            if data is not None:
                break
        if data is None:
            missing.append(path or url)
        else:
            entries.append((path, data))
    if missing:
        raise SystemExit(
            "asset pack: cannot resolve bundle files against the given roots: "
            + ", ".join(sorted(missing))
        )
    return entries


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build a KAGRA asset pack (ZIP) from a bundle.json")
    parser.add_argument("--bundle", required=True, help="staged bundle.json whose files get packed")
    parser.add_argument(
        "--root",
        action="append",
        required=True,
        help="directory to resolve bundle file URLs against (repeatable, tried in order)",
    )
    parser.add_argument("--out", required=True, help="where to write the pack ZIP")
    parser.add_argument("--store", action="store_true", help="no compression (default: deflate)")
    args = parser.parse_args(argv)
    bundle = json.loads(Path(args.bundle).read_text(encoding="utf-8"))
    roots = [Path(r) for r in args.root]
    entries = resolve_bundle_entries(bundle, roots)
    info = pack_info(entries, compress=not args.store)
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(info.pop("blob"))
    print(json.dumps(info))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
