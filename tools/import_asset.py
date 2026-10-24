#!/usr/bin/env python3
"""Import checks for a 3D asset (GLB, glTF, FBX) before it is committed.

    python3 tools/import_asset.py assets/models/foo.glb [--profile prop] [--json]

Checks (all stdlib, no trimesh needed, so CI can run them):
  * triangle count (unique meshes and as instanced by the scene) against a budget
  * texture size (largest side, in pixels) against a budget
  * file size, and whether geometry is meshopt/Draco compressed
  * extensions the engine cannot load (meshopt and Draco are not decoded by
    kagra-shared today, so a *required* one is an error)
  * the file is covered by an entry in assets/LEDGER.json (license first)

FBX is read as binary FBX 7.x only (polygon counts and texture file names).
Convert ASCII FBX with Blender or FBX2glTF first.

Exit code 1 when any error is reported. Warnings do not fail.
"""

from __future__ import annotations

import argparse
import base64
import json
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import asset_ledger  # noqa: E402

PROFILES = {
    "prop": {"max_triangles": 20_000, "max_texture": 1024, "max_file_mb": 4.0},
    "character": {"max_triangles": 60_000, "max_texture": 2048, "max_file_mb": 12.0},
    "environment": {"max_triangles": 150_000, "max_texture": 2048, "max_file_mb": 20.0},
    "default": {"max_triangles": 100_000, "max_texture": 2048, "max_file_mb": 8.0},
}
MESHOPT_ADVISE_BYTES = 512 * 1024
# kagra-shared/src/gltf_load.rs does not decode these. A file that requires
# one cannot be loaded at all.
UNSUPPORTED_REQUIRED = {
    "EXT_meshopt_compression",
    "KHR_draco_mesh_compression",
    "KHR_texture_basisu",
}
COMPRESSION_EXTS = ("EXT_meshopt_compression", "KHR_draco_mesh_compression")


def image_size(data: bytes) -> tuple[int, int] | None:
    if data[:8] == b"\x89PNG\r\n\x1a\n" and len(data) >= 24:
        return struct.unpack(">II", data[16:24])
    if data[:2] == b"\xff\xd8":
        i = 2
        while i + 9 < len(data):
            if data[i] != 0xFF:
                i += 1
                continue
            marker = data[i + 1]
            if marker in (0xD8, 0x01) or 0xD0 <= marker <= 0xD7:
                i += 2
                continue
            seg = struct.unpack(">H", data[i + 2 : i + 4])[0]
            if 0xC0 <= marker <= 0xCF and marker not in (0xC4, 0xC8, 0xCC):
                h, w = struct.unpack(">HH", data[i + 5 : i + 9])
                return w, h
            i += 2 + seg
        return None
    if data[:12] == b"\xabKTX 20\xbb\r\n\x1a\n" and len(data) >= 24:
        w, h = struct.unpack("<II", data[20:28]) if len(data) >= 28 else (0, 0)
        return (w, h) if w else None
    if data[:4] == b"RIFF" and data[8:12] == b"WEBP":
        if data[12:16] == b"VP8X" and len(data) >= 30:
            w = 1 + int.from_bytes(data[24:27], "little")
            h = 1 + int.from_bytes(data[27:30], "little")
            return w, h
        return None
    return None


def read_glb(path: Path) -> tuple[dict, bytes | None]:
    raw = path.read_bytes()
    if raw[:4] != b"glTF":
        raise ValueError("not a GLB (missing glTF magic)")
    length = struct.unpack("<I", raw[8:12])[0]
    pos, doc, binchunk = 12, None, None
    while pos + 8 <= min(length, len(raw)):
        clen, ctype = struct.unpack("<I4s", raw[pos : pos + 8])
        body = raw[pos + 8 : pos + 8 + clen]
        if ctype == b"JSON":
            doc = json.loads(body.decode("utf-8"))
        elif ctype == b"BIN\x00" and binchunk is None:
            binchunk = body
        pos += 8 + clen + (-clen % 4)
    if doc is None:
        raise ValueError("GLB has no JSON chunk")
    return doc, binchunk


def _buffer_bytes(doc: dict, base: Path, glb_bin: bytes | None, index: int) -> bytes | None:
    buf = doc.get("buffers", [])[index]
    uri = buf.get("uri")
    if uri is None:
        return glb_bin
    if uri.startswith("data:"):
        return base64.b64decode(uri.split(",", 1)[1])
    target = base / uri
    return target.read_bytes() if target.is_file() else None


def check_gltf(path: Path, doc: dict, glb_bin: bytes | None, budgets: dict) -> dict:
    rep: dict = {"errors": [], "warnings": []}
    accessors = doc.get("accessors", [])
    meshes = doc.get("meshes", [])
    unique = []
    for mesh in meshes:
        tris = 0
        for prim in mesh.get("primitives", []):
            mode = prim.get("mode", 4)
            if "indices" in prim:
                n = accessors[prim["indices"]]["count"]
            elif "POSITION" in prim.get("attributes", {}):
                n = accessors[prim["attributes"]["POSITION"]]["count"]
            else:
                n = 0
            if mode == 4:
                tris += n // 3
            elif mode in (5, 6):
                tris += max(n - 2, 0)
        unique.append(tris)
    uses = [0] * len(meshes)
    nodes = doc.get("nodes", [])
    scene_nodes = doc.get("scenes", [{}])[doc.get("scene", 0)].get("nodes", []) if doc.get("scenes") else []

    def walk(i: int, seen: set[int]) -> None:
        if i in seen or i >= len(nodes):
            return
        seen = seen | {i}
        node = nodes[i]
        if "mesh" in node:
            uses[node["mesh"]] += 1
        for child in node.get("children", []):
            walk(child, seen)

    for n in scene_nodes or range(len(nodes)):
        walk(n, set())
    instanced = sum(t * max(u, 0) for t, u in zip(unique, uses)) if scene_nodes or nodes else sum(unique)
    rep["triangles_unique"] = sum(unique)
    rep["triangles_instanced"] = instanced

    # textures
    sizes = []
    for i, img in enumerate(doc.get("images", [])):
        data = None
        uri = img.get("uri")
        if "bufferView" in img:
            bv = doc["bufferViews"][img["bufferView"]]
            blob = _buffer_bytes(doc, path.parent, glb_bin, bv.get("buffer", 0))
            if blob is not None:
                off = bv.get("byteOffset", 0)
                data = blob[off : off + bv["byteLength"]]
        elif uri and uri.startswith("data:"):
            data = base64.b64decode(uri.split(",", 1)[1])
        elif uri:
            target = path.parent / uri
            if target.is_file():
                data = target.read_bytes()
            else:
                rep["errors"].append(f"image {i}: missing file {uri}")
                continue
        size = image_size(data) if data else None
        if size is None:
            rep["warnings"].append(f"image {i}: size could not be read")
            continue
        sizes.append({"index": i, "width": size[0], "height": size[1]})
    rep["textures"] = sizes
    rep["max_texture_side"] = max((max(s["width"], s["height"]) for s in sizes), default=0)

    # compression
    used = set(doc.get("extensionsUsed", []))
    required = set(doc.get("extensionsRequired", []))
    rep["compression"] = sorted(used & set(COMPRESSION_EXTS))
    for ext in sorted(required & UNSUPPORTED_REQUIRED):
        rep["errors"].append(f"requires {ext}, which the engine does not decode (decompress before import)")
    for ext in sorted((used - required) & UNSUPPORTED_REQUIRED):
        rep["warnings"].append(f"uses optional {ext}; the engine falls back to uncompressed data only if present")
    geo = set()
    for acc in accessors:
        if "bufferView" in acc:
            geo.add(acc["bufferView"])
    img_views = {img["bufferView"] for img in doc.get("images", []) if "bufferView" in img}
    geo_bytes = sum(doc["bufferViews"][i]["byteLength"] for i in geo - img_views)
    rep["geometry_bytes"] = geo_bytes
    if geo_bytes > MESHOPT_ADVISE_BYTES and not rep["compression"]:
        rep["warnings"].append(
            f"geometry is {geo_bytes // 1024} KiB uncompressed; for web delivery consider "
            "`gltfpack -cc` or gltf-transform meshopt (the engine cannot load meshopt yet, so keep an uncompressed copy)"
        )
    if not meshes:
        rep["errors"].append("no meshes")
    return rep


def _fbx_value(buf: bytes, pos: int):
    t = chr(buf[pos])
    pos += 1
    fixed = {"Y": ("<h", 2), "C": ("<B", 1), "I": ("<i", 4), "F": ("<f", 4), "D": ("<d", 8), "L": ("<q", 8)}
    if t in fixed:
        fmt, size = fixed[t]
        return struct.unpack(fmt, buf[pos : pos + size])[0], pos + size
    if t in "SR":
        n = struct.unpack("<I", buf[pos : pos + 4])[0]
        return buf[pos + 4 : pos + 4 + n], pos + 4 + n
    if t in "fdlib":
        count, enc, clen = struct.unpack("<III", buf[pos : pos + 12])
        pos += 12
        data = buf[pos : pos + clen]
        if enc == 1:
            data = zlib.decompress(data)
        size = {"f": 4, "d": 8, "l": 8, "i": 4, "b": 1}[t]
        fmt = {"f": "f", "d": "d", "l": "q", "i": "i", "b": "b"}[t]
        return list(struct.unpack(f"<{count}{fmt}", data[: count * size])), pos + clen
    raise ValueError(f"unknown FBX property type {t!r}")


def _fbx_nodes(buf: bytes, pos: int, end: int, wide: bool, out: list, depth: int = 0) -> int:
    hdr = 24 if wide else 12
    while pos + hdr <= end:
        if wide:
            end_off, nprops, plen, nlen = struct.unpack("<QQQB", buf[pos : pos + 25])
            start = pos + 25
        else:
            end_off, nprops, plen, nlen = struct.unpack("<IIIB", buf[pos : pos + 13])
            start = pos + 13
        if end_off == 0:
            return pos + (25 if wide else 13)
        name = buf[start : start + nlen].decode("latin-1")
        p = start + nlen
        props = []
        if name in ("PolygonVertexIndex", "Content", "RelativeFilename", "FileName", "Filename"):
            for _ in range(nprops):
                try:
                    v, p = _fbx_value(buf, p)
                except Exception:
                    break
                props.append(v)
            out.append((name, props))
        p = start + nlen + plen
        if p < end_off:
            _fbx_nodes(buf, p, end_off, wide, out, depth + 1)
        pos = end_off
    return pos


def check_fbx(path: Path, budgets: dict) -> dict:
    rep: dict = {"errors": [], "warnings": []}
    buf = path.read_bytes()
    if not buf.startswith(b"Kaydara FBX Binary"):
        rep["errors"].append("ASCII FBX is not supported; convert to GLB first (Blender or FBX2glTF)")
        return rep
    version = struct.unpack("<I", buf[23:27])[0]
    out: list = []
    try:
        _fbx_nodes(buf, 27, len(buf), version >= 7500, out)
    except Exception as exc:  # corrupt file
        rep["errors"].append(f"FBX parse error: {exc}")
        return rep
    tris = 0
    sizes = []
    for name, props in out:
        if name == "PolygonVertexIndex" and props and isinstance(props[0], list):
            n = 0
            for idx in props[0]:
                n += 1
                if idx < 0:
                    tris += max(n - 2, 0)
                    n = 0
        if name in ("RelativeFilename", "FileName", "Filename") and props and isinstance(props[0], bytes):
            ref = props[0].decode("utf-8", "replace").replace("\\", "/")
            if ref.lower().endswith((".png", ".jpg", ".jpeg", ".webp")):
                cand = path.parent / Path(ref).name
                size = image_size(cand.read_bytes()) if cand.is_file() else None
                if size:
                    sizes.append({"file": ref, "width": size[0], "height": size[1]})
                else:
                    rep["warnings"].append(f"texture {ref} not found next to the FBX; size unchecked")
        if name == "Content" and props and isinstance(props[0], bytes) and len(props[0]) > 16:
            size = image_size(props[0])
            if size:
                sizes.append({"file": "(embedded)", "width": size[0], "height": size[1]})
    rep["triangles_unique"] = rep["triangles_instanced"] = tris
    rep["textures"] = sizes
    rep["max_texture_side"] = max((max(s["width"], s["height"]) for s in sizes), default=0)
    rep["compression"] = []
    rep["warnings"].append("FBX is not loaded by the engine; convert to GLB for runtime (Quaternius FBX packs need this)")
    if tris == 0:
        rep["errors"].append("no polygons found")
    return rep


def run(path: Path, profile: str = "default", overrides: dict | None = None, ledger_check: bool = True) -> dict:
    path = Path(path)
    budgets = dict(PROFILES[profile])
    budgets.update({k: v for k, v in (overrides or {}).items() if v is not None})
    ext = path.suffix.lower()
    rep: dict = {"path": str(path), "format": ext.lstrip("."), "budgets": budgets, "errors": [], "warnings": []}
    try:
        if ext == ".glb":
            doc, binchunk = read_glb(path)
            rep.update(check_gltf(path, doc, binchunk, budgets))
        elif ext == ".gltf":
            doc = json.loads(path.read_text(encoding="utf-8"))
            rep.update(check_gltf(path, doc, None, budgets))
        elif ext == ".fbx":
            rep.update(check_fbx(path, budgets))
        else:
            rep["errors"].append(f"unsupported format {ext}; use .glb, .gltf or .fbx")
            return rep
    except (ValueError, OSError, KeyError, IndexError, json.JSONDecodeError) as exc:
        rep["errors"].append(f"could not read asset: {exc}")
        return rep
    size = path.stat().st_size
    rep["file_bytes"] = size
    tris = rep.get("triangles_instanced", 0)
    if tris > budgets["max_triangles"]:
        rep["errors"].append(f"{tris} triangles exceeds the {budgets['max_triangles']} budget")
    if rep.get("max_texture_side", 0) > budgets["max_texture"]:
        rep["errors"].append(f"texture {rep['max_texture_side']}px exceeds the {budgets['max_texture']}px budget")
    if size > budgets["max_file_mb"] * 1024 * 1024:
        rep["errors"].append(f"file is {size / 1048576:.1f} MB, over the {budgets['max_file_mb']} MB budget")
    if ledger_check:
        try:
            rel = path.resolve().relative_to(asset_ledger.ROOT).as_posix()
        except ValueError:
            rel = None
        if rel is not None:
            ledger = asset_ledger.load_ledger()
            owners = [p for p in ledger["packs"] if asset_ledger.matches(rel, p.get("paths", []))]
            if not owners:
                rep["errors"].append(f"{rel}: no assets/LEDGER.json entry covers it; add the license entry first")
            elif not owners[0].get("redistributable_in_public_repo"):
                rep["errors"].append(f"{rel}: ledger {owners[0]['id']} is not public-redistributable; keep it under assets-private/")
    return rep


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("files", nargs="+")
    ap.add_argument("--profile", choices=sorted(PROFILES), default="default")
    ap.add_argument("--max-triangles", type=int)
    ap.add_argument("--max-texture", type=int)
    ap.add_argument("--max-file-mb", type=float)
    ap.add_argument("--no-ledger-check", action="store_true")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args(argv)
    over = {"max_triangles": args.max_triangles, "max_texture": args.max_texture, "max_file_mb": args.max_file_mb}
    failed = 0
    for f in args.files:
        rep = run(Path(f), args.profile, over, not args.no_ledger_check)
        if args.json:
            print(json.dumps(rep, indent=1))
        else:
            print(
                f"{f}: {rep.get('triangles_instanced', '?')} tris, "
                f"tex max {rep.get('max_texture_side', '?')}px, {rep.get('file_bytes', 0) // 1024} KiB, "
                f"compression {rep.get('compression') or 'none'}"
            )
            for w in rep["warnings"]:
                print(f"  warning: {w}")
            for e in rep["errors"]:
                print(f"  error: {e}")
        failed |= 1 if rep["errors"] else 0
    return failed


if __name__ == "__main__":
    raise SystemExit(main())
