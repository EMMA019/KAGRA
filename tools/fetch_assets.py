#!/usr/bin/env python3
"""Collect CC0 / CC-BY glTF into ``assets/models`` and check each file.

Future games use the same flow: write a pack manifest, run this script,
then point props at the extracted ``.glb``. Paid packs and other licenses
are refused. The mesh bytes stay the author's. Two optional packaging
steps are recorded in the credits file when a manifest asks for them:

* ``embed_images`` pulls a relative image (Kenney's ``Textures/colormap.png``)
  into the GLB. The shared loader reads buffer views, not external URIs.
* ``seat_origin`` zeros a root node translation so the mesh sits on the
  origin. Kenney's older exports use a non-zero root. Vertices are not
  edited.

    python tools/fetch_assets.py tools/fetch_assets/neon_cat.json
    python tools/fetch_assets.py tools/fetch_assets/neon_cat.json --check
"""
from __future__ import annotations

import argparse
import io
import json
import struct
import subprocess
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CHECK = ROOT / "tools" / "check_gltf_asset.py"
ALLOWED = {
    "CC0-1.0": "https://creativecommons.org/publicdomain/zero/1.0/",
    "CC-BY-4.0": "https://creativecommons.org/licenses/by/4.0/",
    "CC-BY-3.0": "https://creativecommons.org/licenses/by/3.0/",
}


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Fetch and check CC0/CC-BY glTF packs")
    parser.add_argument("manifest", type=Path)
    parser.add_argument(
        "--check",
        action="store_true",
        help="run the glTF checker on files already extracted; do not download",
    )
    args = parser.parse_args(argv)
    spec = json.loads(args.manifest.read_text(encoding="utf-8"))
    packs = spec.get("packs")
    if not isinstance(packs, list) or not packs:
        print("manifest needs a packs list", file=sys.stderr)
        return 2
    failed = 0
    for pack in packs:
        try:
            failed += _one(pack, check_only=args.check)
        except Exception as exc:  # noqa: BLE001 — one pack should not hide the rest
            print(f"FAIL {pack.get('id', '?')}: {exc}", file=sys.stderr)
            failed += 1
    return 1 if failed else 0


def _one(pack: dict, *, check_only: bool) -> int:
    license_id = str(pack.get("license") or "").strip()
    if license_id not in ALLOWED:
        raise SystemExit(
            f"{pack.get('id')}: license {license_id!r} is not CC0 or CC-BY. "
            "Paid and other licenses are refused."
        )
    dest = ROOT / str(pack["dest"])
    dest.mkdir(parents=True, exist_ok=True)
    files = pack.get("files") or []
    if not isinstance(files, list) or not files:
        raise SystemExit(f"{pack.get('id')}: files must be a non-empty list")
    if not check_only and pack.get("download_url"):
        _download(pack, dest, files)
    elif not check_only and not all(item.get("check_only") for item in files):
        raise SystemExit(f"{pack.get('id')}: download_url is required unless every file is check_only")
    notes = []
    glbs: list[Path] = []
    outs: list[tuple[dict, Path]] = []
    for item in files:
        out = dest / str(item["out"])
        if not out.is_file():
            raise SystemExit(f"missing {out.relative_to(ROOT)}")
        outs.append((item, out))
    for item, out in outs:
        if item.get("embed_images"):
            _embed_images(out)
            notes.append(f"{out.name}: external images embedded (bufferView)")
        if item.get("seat_origin"):
            _seat_origin(out)
            notes.append(f"{out.name}: root translation zeroed")
        if out.suffix.lower() == ".glb":
            glbs.append(out)
    _write_credits(dest, pack, notes)
    return _check(glbs)


def _download(pack: dict, dest: Path, files: list[dict]) -> None:
    url = str(pack["download_url"])
    if not url.startswith("https://"):
        raise SystemExit(f"{pack.get('id')}: download_url must be https")
    print(f"get {url}")
    req = urllib.request.Request(url, headers={"User-Agent": "kagra-fetch-assets"})
    with urllib.request.urlopen(req, timeout=120) as resp:
        blob = resp.read()
    with tempfile.TemporaryDirectory() as tmp:
        folder = Path(tmp)
        if url.lower().endswith(".zip") or blob[:2] == b"PK":
            with zipfile.ZipFile(io.BytesIO(blob)) as archive:
                archive.extractall(folder)
        else:
            (folder / "download.bin").write_bytes(blob)
        license_name = pack.get("license_file")
        if isinstance(license_name, str) and license_name:
            src = _find(folder, license_name)
            if src is not None:
                (dest / "LICENSE.txt").write_bytes(src.read_bytes())
        for item in files:
            if item.get("check_only"):
                continue
            src = _find(folder, str(item["zip"]))
            if src is None:
                raise SystemExit(f"{pack.get('id')}: {item['zip']} is not in the archive")
            out = dest / str(item["out"])
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_bytes(src.read_bytes())
            print(f"  {out.relative_to(ROOT)}")


def _find(folder: Path, name: str) -> Path | None:
    direct = folder / name
    if direct.is_file():
        return direct
    matches = [p for p in folder.rglob(Path(name).name) if p.is_file() and p.as_posix().endswith(name)]
    if matches:
        return matches[0]
    matches = [p for p in folder.rglob(Path(name).name) if p.is_file()]
    return matches[0] if len(matches) == 1 else None


def _split_glb(data: bytes) -> tuple[dict, bytes]:
    if data[:4] != b"glTF":
        raise SystemExit("not a GLB")
    off = 12
    length, kind = struct.unpack_from("<I4s", data, off)
    if kind != b"JSON":
        raise SystemExit("GLB JSON chunk missing")
    doc = json.loads(data[off + 8 : off + 8 + length])
    off += 8 + length
    blob = b""
    if off + 8 <= len(data):
        length, kind = struct.unpack_from("<I4s", data, off)
        if kind == b"BIN\x00":
            blob = data[off + 8 : off + 8 + length]
    return doc, bytearray(blob)


def _pack_glb(doc: dict, blob: bytes) -> bytes:
    js = json.dumps(doc, separators=(",", ":")).encode("utf-8")
    if len(js) % 4:
        js += b" " * (4 - len(js) % 4)
    if len(blob) % 4:
        blob += b"\x00" * (4 - len(blob) % 4)
    total = 12 + 8 + len(js) + 8 + len(blob)
    out = struct.pack("<4sII", b"glTF", 2, total)
    out += struct.pack("<I4s", len(js), b"JSON") + js
    out += struct.pack("<I4s", len(blob), b"BIN\x00") + blob
    return out


def _embed_images(path: Path) -> None:
    doc, blob = _split_glb(path.read_bytes())
    changed = False
    views = doc.setdefault("bufferViews", [])
    buffers = doc.setdefault("buffers", [{"byteLength": len(blob)}])
    if not buffers:
        buffers.append({"byteLength": len(blob)})
    for image in doc.get("images") or []:
        uri = image.get("uri")
        if not uri or uri.startswith("data:") or image.get("bufferView") is not None:
            continue
        src = (path.parent / uri).resolve()
        if not src.is_file():
            raise SystemExit(f"{path.name}: image {uri} is not beside the GLB")
        raw = src.read_bytes()
        while len(blob) % 4:
            blob.append(0)
        view = {"buffer": 0, "byteOffset": len(blob), "byteLength": len(raw)}
        blob.extend(raw)
        views.append(view)
        image.pop("uri", None)
        image["bufferView"] = len(views) - 1
        mime = "image/png" if src.suffix.lower() == ".png" else "image/jpeg"
        image["mimeType"] = mime
        changed = True
    if changed:
        buffers[0]["byteLength"] = len(blob)
        buffers[0].pop("uri", None)
        path.write_bytes(_pack_glb(doc, bytes(blob)))


def _seat_origin(path: Path) -> None:
    doc, blob = _split_glb(path.read_bytes())
    for node in doc.get("nodes") or []:
        if "translation" in node:
            node["translation"] = [0.0, 0.0, 0.0]
    path.write_bytes(_pack_glb(doc, bytes(blob)))


def _write_credits(dest: Path, pack: dict, notes: list[str]) -> None:
    lines = [
        f"# {pack.get('credit', pack.get('id'))}",
        "",
        f"License: {pack['license']}",
        f"License URL: {pack.get('license_url') or ALLOWED[pack['license']]}",
        f"Source: {pack.get('source_url', '')}",
    ]
    if pack.get("download_url"):
        lines.append(f"Download: {pack['download_url']}")
    lines.append("")
    lines.append(
        "Attribution is required for CC-BY. CC0 does not require it; "
        "credit is still recorded here."
    )
    if notes:
        lines.append("")
        lines.extend(f"- {note}" for note in notes)
    lines.append("")
    (dest / "CREDITS.md").write_text("\n".join(lines), encoding="utf-8")


def _check(paths: list[Path]) -> int:
    if not paths:
        return 0
    proc = subprocess.run([sys.executable, str(CHECK), *map(str, paths)], check=False)
    return 0 if proc.returncode == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
