#!/usr/bin/env python3
"""Make a mobile-friendly, self-contained rest-pose copy of a Kenney-style .glb.

What it does (nothing is invented, only removed or re-packed):
  * drops animations, skins and the JOINTS_0/WEIGHTS_0 attributes (the survivor
    swarm draws batched static props; the game animates them with a bob and a
    squash, which is cheap, and CPU skinning 200 foes would not be),
  * prunes every accessor / buffer view / image that nothing references,
  * embeds the external `Textures/colormap.png` into the file, so one .glb is one asset.

  * with --lean also drops the TANGENT and second-UV streams (the game has no normal
    maps and every material samples UV 0): about a quarter fewer bytes to download and
    to upload to the GPU, and no visible change.

Usage:  pack_glb.py SRC.glb DST.glb [--clip NAME --time SECONDS] [--lean]
  (with --clip the skinned pose of that clip frame is baked in instead of the rest pose)
"""
import json
import struct
import sys
from pathlib import Path

COMP = {5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4}
NCOMP = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT2": 4, "MAT3": 9, "MAT4": 16}


def read_glb(path):
    b = Path(path).read_bytes()
    assert b[:4] == b"glTF", "not a glb"
    jlen = struct.unpack("<I", b[12:16])[0]
    j = json.loads(b[20 : 20 + jlen])
    off = 20 + jlen
    blen, btype = struct.unpack("<II", b[off : off + 8])
    assert btype == 0x004E4942, "no BIN chunk"
    return j, b[off + 8 : off + 8 + blen]


def pad4(b, fill=b"\0"):
    return b + fill * (-len(b) % 4)


def to_rgba_png(data):
    """Re-encode a palette / grey / RGB PNG as plain RGBA.

    The page's renderer decodes RGBA PNGs only; a palette PNG (the Kenney
    mini-characters ship one) comes out white.
    """
    import io

    from PIL import Image

    im = Image.open(io.BytesIO(data))
    if im.mode == "RGBA" and im.format == "PNG":
        return data
    out = io.BytesIO()
    im.convert("RGBA").save(out, "PNG", optimize=True)
    return out.getvalue()


def pack(src, dst, clip=None, at=0.0, lean=False):
    j, bin_ = read_glb(src)
    src = Path(src)
    posed = {}
    if clip:
        sys.path.insert(0, str(Path(__file__).parent))
        from bake_pose import bake_pose

        posed = bake_pose(j, bin_, clip, at)
    # 1. which accessors do the meshes use (without skin data)?
    used = []
    for m in j.get("meshes", []):
        for p in m["primitives"]:
            drop = ("JOINTS_", "WEIGHTS_") + (("TANGENT",) if clip or lean else ())
            if lean:
                drop += ("TEXCOORD_1",)
            p["attributes"] = {k: v for k, v in p["attributes"].items() if not k.startswith(drop)}
            for v in p["attributes"].values():
                used.append(v)
            if "indices" in p:
                used.append(p["indices"])
            for t in p.get("targets", []):
                used.extend(t.values())
    used = sorted(set(used))
    new_bin = bytearray()
    new_views, new_acc, remap = [], [], {}
    for ai in used:
        a = j["accessors"][ai]
        assert "sparse" not in a, "sparse accessors are not handled"
        v = j["bufferViews"][a["bufferView"]]
        el = COMP[a["componentType"]] * NCOMP[a["type"]]
        stride = v.get("byteStride", el)
        start = v.get("byteOffset", 0) + a.get("byteOffset", 0)
        n = a["count"]
        if stride == el:
            data = bin_[start : start + n * el]
        else:
            data = b"".join(bin_[start + i * stride : start + i * stride + el] for i in range(n))
        extra = {}
        if ai in posed:
            arr = posed[ai]
            data = arr.tobytes()
            if a["type"] == "VEC3" and a.get("min") is not None:
                extra = {"min": [float(x) for x in arr.min(0)], "max": [float(x) for x in arr.max(0)]}
        off = len(new_bin)
        new_bin += pad4(data)
        nv = {"buffer": 0, "byteOffset": off, "byteLength": len(data)}
        if v.get("target"):
            nv["target"] = v["target"]
        remap[ai] = len(new_acc)
        new_views.append(nv)
        na = {k: val for k, val in a.items() if k not in ("bufferView", "byteOffset", "sparse")}
        na.update(extra)
        na["bufferView"] = len(new_views) - 1
        new_acc.append(na)
    for m in j.get("meshes", []):
        for p in m["primitives"]:
            p["attributes"] = {k: remap[v] for k, v in p["attributes"].items()}
            if "indices" in p:
                p["indices"] = remap[p["indices"]]
    # 2. embed images
    for img in j.get("images", []):
        uri = img.pop("uri", None)
        if uri:
            data = (src.parent / uri).read_bytes()
            img["mimeType"] = "image/png"
        else:
            v = j["bufferViews"][img["bufferView"]]
            data = bin_[v.get("byteOffset", 0) : v.get("byteOffset", 0) + v["byteLength"]]
        data = to_rgba_png(data)
        data = to_rgba_png(data)
        off = len(new_bin)
        new_bin += pad4(data)
        new_views.append({"buffer": 0, "byteOffset": off, "byteLength": len(data)})
        img["bufferView"] = len(new_views) - 1
    # 3. drop skins / animations
    j.pop("animations", None)
    j.pop("skins", None)
    for n in j.get("nodes", []):
        n.pop("skin", None)
    j["accessors"] = new_acc
    j["bufferViews"] = new_views
    j["buffers"] = [{"byteLength": len(new_bin)}]
    prior = (j["asset"].get("extras") or {}).get("note", "")
    note = f"static copy of clip {clip}@{at}s made by tools/gen_assets/pack_glb.py" if clip else "rest-pose copy made by tools/gen_assets/pack_glb.py"
    if not clip and prior.startswith("static copy of clip"):
        note = prior.split(";")[0]  # a re-pack of a baked pose keeps saying so
    j["asset"]["extras"] = {"note": note + ("; lean (no tangent / second UV)" if lean else "")}
    jb = pad4(json.dumps(j, separators=(",", ":")).encode(), b" ")
    out = struct.pack("<4sII", b"glTF", 2, 12 + 8 + len(jb) + 8 + len(new_bin))
    out += struct.pack("<II", len(jb), 0x4E4F534A) + jb
    out += struct.pack("<II", len(new_bin), 0x004E4942) + bytes(new_bin)
    Path(dst).parent.mkdir(parents=True, exist_ok=True)
    Path(dst).write_bytes(out)
    return len(out)


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("dst")
    ap.add_argument("--clip")
    ap.add_argument("--time", type=float, default=0.0)
    ap.add_argument("--lean", action="store_true")
    a = ap.parse_args()
    print(pack(a.src, a.dst, a.clip, a.time, a.lean))
