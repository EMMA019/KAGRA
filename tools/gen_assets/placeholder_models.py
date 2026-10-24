"""Original placeholder glTF models, written by this script (CC0-1.0).

These stand in for the third-party scenery that used to sit under
``assets/models/quaternius/``. Each model is a few flat-coloured primitives
fitted to the same bounding box as the mesh it replaced, so scale and
placement in existing worlds stay valid. They are placeholders, not art.

    python3 tools/gen_assets/placeholder_models.py          # write all
    python3 tools/gen_assets/placeholder_models.py --check  # exit 1 on drift

Pure standard library. Output is deterministic.
"""
from __future__ import annotations

import json
import math
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "assets" / "models" / "placeholder"
GATE = ROOT / "assets" / "models" / "neon_cat" / "gate_arch.glb"

Tri = tuple  # ((x,y,z) * 3)


def _box(c, s):
    cx, cy, cz = c
    hx, hy, hz = (v / 2 for v in s)
    p = [(cx + sx * hx, cy + sy * hy, cz + sz * hz) for sx in (-1, 1) for sy in (-1, 1) for sz in (-1, 1)]
    # corner index = (sx>0)*4 + (sy>0)*2 + (sz>0)
    quads = [
        (4, 5, 7, 6),  # +x
        (0, 2, 3, 1),  # -x
        (2, 6, 7, 3),  # +y
        (0, 1, 5, 4),  # -y
        (1, 3, 7, 5),  # +z
        (0, 4, 6, 2),  # -z
    ]
    out = []
    for a, b, c2, d in quads:
        out += [(p[a], p[b], p[c2]), (p[a], p[c2], p[d])]
    return out


def _lathe(profile, seg, c=(0.0, 0.0, 0.0)):
    """Solid of revolution about +Y. profile: [(radius, y)] bottom to top."""
    cx, cy, cz = c
    ring = []
    for r, y in profile:
        ring.append([(cx + r * math.cos(2 * math.pi * i / seg), cy + y, cz + r * math.sin(2 * math.pi * i / seg)) for i in range(seg)])
    tris = []
    for k in range(len(ring) - 1):
        a, b = ring[k], ring[k + 1]
        for i in range(seg):
            j = (i + 1) % seg
            if profile[k][0] > 1e-9:
                tris.append((a[i], b[i], a[j]))
            if profile[k + 1][0] > 1e-9:
                tris.append((a[j], b[i], b[j]))
    # caps
    r0, y0 = profile[0]
    r1, y1 = profile[-1]
    for i in range(seg):
        j = (i + 1) % seg
        if r0 > 1e-9:
            tris.append(((cx, cy + y0, cz), ring[0][j], ring[0][i]))
        if r1 > 1e-9:
            tris.append(((cx, cy + y1, cz), ring[-1][i], ring[-1][j]))
    return tris


def _cyl(c, r, h, seg=10):
    return _lathe([(r, -h / 2), (r, h / 2)], seg, c)


def _cone(c, r, h, seg=10):
    return _lathe([(r, -h / 2), (1e-6, h / 2)], seg, c)


def _sphere(c, rx, ry, rz, seg=10, rings=6):
    prof = []
    for k in range(rings + 1):
        a = math.pi * k / rings
        prof.append((max(math.sin(a), 1e-6), -math.cos(a)))
    tris = _lathe([(p[0], p[1]) for p in prof], seg)
    cx, cy, cz = c
    return [tuple((cx + v[0] * rx, cy + v[1] * ry, cz + v[2] * rz) for v in t) for t in tris]


def _roof(c, w, h, d):
    """Triangular prism, ridge along z."""
    cx, cy, cz = c
    hw, hd = w / 2, d / 2
    a = (cx - hw, cy - h / 2, cz - hd)
    b = (cx + hw, cy - h / 2, cz - hd)
    t = (cx, cy + h / 2, cz - hd)
    a2 = (cx - hw, cy - h / 2, cz + hd)
    b2 = (cx + hw, cy - h / 2, cz + hd)
    t2 = (cx, cy + h / 2, cz + hd)
    return [
        (a2, b2, t2),
        (b, a, t),
        (a, a2, t2), (a, t2, t),
        (b2, b, t), (b2, t, t2),
        (a, b, b2), (a, b2, a2),
    ]


def _norm(t):
    (ax, ay, az), (bx, by, bz), (cx, cy, cz) = t
    ux, uy, uz = bx - ax, by - ay, bz - az
    vx, vy, vz = cx - ax, cy - ay, cz - az
    n = (uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx)
    l = math.sqrt(sum(x * x for x in n)) or 1.0
    return tuple(x / l for x in n)


def write_glb(path: Path, parts, name: str):
    """parts: [(color rgb 0..255, [tri])] -> one primitive per colour."""
    by = {}
    for col, tris in parts:
        by.setdefault(tuple(col), []).extend(tris)
    blob = bytearray()
    views, accs, prims, mats = [], [], [], []

    def pad():
        while len(blob) % 4:
            blob.append(0)

    for mi, (col, tris) in enumerate(sorted(by.items())):
        pos, nor = [], []
        for t in tris:
            n = _norm(t)
            for v in t:
                pos.append(v)
                nor.append(n)
        pad()
        o = len(blob)
        for v in pos:
            blob += struct.pack("<3f", *v)
        views.append({"buffer": 0, "byteOffset": o, "byteLength": len(blob) - o, "target": 34962})
        mn = [min(v[i] for v in pos) for i in range(3)]
        mx = [max(v[i] for v in pos) for i in range(3)]
        accs.append({"bufferView": len(views) - 1, "componentType": 5126, "count": len(pos), "type": "VEC3", "min": mn, "max": mx})
        pa = len(accs) - 1
        pad()
        o = len(blob)
        for v in nor:
            blob += struct.pack("<3f", *v)
        views.append({"buffer": 0, "byteOffset": o, "byteLength": len(blob) - o, "target": 34962})
        accs.append({"bufferView": len(views) - 1, "componentType": 5126, "count": len(nor), "type": "VEC3"})
        na = len(accs) - 1
        mats.append({
            "name": f"mat{mi}",
            "pbrMetallicRoughness": {
                "baseColorFactor": [round((c / 255.0) ** 2.2, 4) for c in col] + [1.0],
                "metallicFactor": 0.0,
                "roughnessFactor": 0.9,
            },
        })
        prims.append({"attributes": {"POSITION": pa, "NORMAL": na}, "material": mi, "mode": 4})
    pad()
    doc = {
        "asset": {"version": "2.0", "generator": "kagra tools/gen_assets/placeholder_models.py (CC0-1.0)"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": name, "mesh": 0}],
        "meshes": [{"name": name, "primitives": prims}],
        "materials": mats,
        "accessors": accs,
        "bufferViews": views,
        "buffers": [{"byteLength": len(blob)}],
    }
    js = json.dumps(doc, separators=(",", ":")).encode()
    while len(js) % 4:
        js += b" "
    total = 12 + 8 + len(js) + 8 + len(blob)
    data = struct.pack("<III", 0x46546C67, 2, total) + struct.pack("<II", len(js), 0x4E4F534A) + js
    data += struct.pack("<II", len(blob), 0x004E4942) + bytes(blob)
    return data


# --- shapes. Each takes the target bbox (min, max) and returns parts. -----

def _fit(bb):
    (x0, y0, z0), (x1, y1, z1) = bb
    return (x0 + x1) / 2, (z0 + z1) / 2, x1 - x0, z1 - z0, y0, y1 - y0


WALL = (150, 128, 104)
ROOF = (120, 54, 44)
WOOD = (96, 66, 40)
STONE = (120, 120, 126)
LEAF = (50, 110, 60)
DARK = (34, 78, 52)


def house(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    return [
        (WALL, _box((cx, y0 + h * 0.3, cz), (w * 0.88, h * 0.6, d * 0.88))),
        (ROOF, _roof((cx, y0 + h * 0.8, cz), w, h * 0.4, d)),
    ]


def inn(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    return [
        (WALL, _box((cx, y0 + h * 0.3, cz), (w * 0.9, h * 0.6, d * 0.9))),
        (ROOF, _roof((cx, y0 + h * 0.8, cz), w, h * 0.4, d)),
        (STONE, _box((cx + w * 0.25, y0 + h * 0.92, cz), (w * 0.1, h * 0.16, d * 0.1))),
    ]


def tower(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    return [
        (WALL, _box((cx, y0 + h * 0.36, cz), (w * 0.8, h * 0.72, d * 0.8))),
        (ROOF, _cone((cx, y0 + h * 0.86, cz), w * 0.62, h * 0.28, 4)),
    ]


def mill(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    return [
        (WALL, _box((cx, y0 + h * 0.3, cz), (w * 0.4, h * 0.6, d * 0.6))),
        (ROOF, _roof((cx, y0 + h * 0.72, cz), w * 0.46, h * 0.24, d * 0.66)),
        (WOOD, _box((cx, y0 + h * 0.62, cz + d * 0.35), (w * 0.98, h * 0.07, d * 0.06))),
        (WOOD, _box((cx, y0 + h * 0.62, cz + d * 0.35), (w * 0.07, h * 0.96 - h * 0.14, d * 0.06))),
    ]


def gazebo(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    out = [(STONE, _cyl((cx, y0 + h * 0.03, cz), min(w, d) * 0.5, h * 0.06, 8))]
    for sx in (-1, 1):
        for sz in (-1, 1):
            out.append((WOOD, _box((cx + sx * w * 0.38, y0 + h * 0.4, cz + sz * d * 0.38), (w * 0.07, h * 0.7, d * 0.07))))
    out.append((ROOF, _cone((cx, y0 + h * 0.85, cz), min(w, d) * 0.62, h * 0.3, 8)))
    return out


def well(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    r = min(w, d) * 0.5
    out = [(STONE, _lathe([(r, y0 + h * 0.0), (r, y0 + h * 0.4)], 10, (cx, 0, cz)))]
    for sx in (-1, 1):
        out.append((WOOD, _box((cx + sx * r * 0.85, y0 + h * 0.6, cz), (w * 0.08, h * 0.6, d * 0.08))))
    out.append((ROOF, _roof((cx, y0 + h * 0.92, cz), w * 1.0, h * 0.16, d * 0.7)))
    return out


def tree(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    return [
        (WOOD, _cyl((cx, y0 + h * 0.2, cz), w * 0.05, h * 0.4, 6)),
        (LEAF, _sphere((cx, y0 + h * 0.65, cz), w * 0.5, h * 0.35, d * 0.5, 8, 5)),
    ]


def pine(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    out = [(WOOD, _cyl((cx, y0 + h * 0.1, cz), w * 0.04, h * 0.2, 6))]
    for k, (f, r) in enumerate([(0.34, 0.5), (0.58, 0.38), (0.8, 0.26)]):
        out.append((DARK, _cone((cx, y0 + h * f, cz), min(w, d) * r, h * 0.34, 8)))
    return out


def rock(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    return [((112, 110, 108), _sphere((cx, y0 + h * 0.5, cz), w * 0.5, h * 0.5, d * 0.5, 7, 4))]


def chest(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    return [
        (WOOD, _box((cx, y0 + h * 0.3, cz), (w, h * 0.6, d))),
        ((124, 84, 44), _box((cx, y0 + h * 0.8, cz), (w, h * 0.4, d))),
        ((190, 160, 70), _box((cx, y0 + h * 0.6, cz + d * 0.5), (w * 0.12, h * 0.16, d * 0.04))),
    ]


def bonfire(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    return [
        (WOOD, _box((cx, y0 + h * 0.25, cz), (w, h * 0.5, d * 0.2))),
        (WOOD, _box((cx, y0 + h * 0.25, cz), (w * 0.2, h * 0.5, d))),
        ((255, 150, 40), _cone((cx, y0 + h * 0.75, cz), w * 0.2, h * 0.5, 6)),
    ]


def fern(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    out = []
    for k in range(5):
        a = 2 * math.pi * k / 5
        out.append((LEAF, _cone((cx + math.cos(a) * w * 0.25, y0 + h * 0.5, cz + math.sin(a) * d * 0.25), w * 0.08, h, 5)))
    return out


def flowers(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    out = []
    for k, col in enumerate([(220, 80, 120), (240, 200, 70), (150, 110, 220)]):
        a = 2 * math.pi * k / 3
        x, z = cx + math.cos(a) * w * 0.25, cz + math.sin(a) * d * 0.25
        out.append((LEAF, _cyl((x, y0 + h * 0.35, z), w * 0.025, h * 0.7, 5)))
        out.append((col, _sphere((x, y0 + h * 0.8, z), w * 0.12, h * 0.15, w * 0.12, 6, 4)))
    return out


def grass(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    out = []
    for k in range(4):
        a = 2 * math.pi * k / 4
        out.append((LEAF, _cone((cx + math.cos(a) * w * 0.2, y0 + h * 0.5, cz + math.sin(a) * d * 0.2), w * 0.12, h, 4)))
    return out


def mushroom(bb):
    cx, cz, w, d, y0, h = _fit(bb)
    return [
        ((230, 220, 200), _cyl((cx, y0 + h * 0.3, cz), w * 0.1, h * 0.6, 6)),
        ((190, 60, 50), _sphere((cx, y0 + h * 0.65, cz), w * 0.5, h * 0.35, d * 0.5, 8, 4)),
    ]


# name -> (shape, bbox of the mesh it replaces)
MODELS = {
    "house": (house, ((-0.968, 0.0, -1.057), (0.968, 1.073, 1.066))),
    "inn": (inn, ((-2.018, 0.0, -2.011), (2.011, 3.486, 2.011))),
    "bell_tower": (tower, ((-0.968, 0.0, -1.125), (0.968, 4.761, 1.103))),
    "mill": (mill, ((-1.693, 0.0, -1.342), (1.704, 4.804, 1.388))),
    "gazebo": (gazebo, ((-0.524, 0.0, -0.621), (0.513, 1.347, 0.638))),
    "well": (well, ((-0.337, 0.0, -0.365), (0.329, 0.998, 0.639))),
    "tree": (tree, ((-2.592, 0.0, -1.837), (3.522, 6.08, 2.4))),
    "pine": (pine, ((-2.282, 0.0, -2.463), (3.064, 6.653, 1.861))),
    "rock": (rock, ((-1.714, 0.0, -1.159), (1.335, 1.848, 1.32))),
    "chest": (chest, ((-0.918, 0.0, -0.961), (0.918, 1.902, 1.031))),
    "bonfire": (bonfire, ((-0.202, 0.0, -0.18), (0.204, 0.079, 0.183))),
    "fern": (fern, ((-4.42, 0.0, -4.091), (4.626, 2.441, 4.396))),
    "flower_group": (flowers, ((-0.614, 0.0, -0.802), (0.875, 2.02, 0.789))),
    "grass": (grass, ((-0.356, 0.0, -0.479), (0.283, 1.312, 0.258))),
    "mushroom": (mushroom, ((-0.333, 0.0, -0.445), (0.23, 0.446, 0.334))),
}


def gate_arch() -> bytes:
    """A stone arch, 4.1 m wide and 4 m tall, opening |x| < 1.12.

    Same outer box as the mesh it replaces (the Neon Cat gate scales it, so
    the box has to match). Pillars, plus a lintel whose underside is a
    half-circle of 1.12 m radius.
    """
    hw_out, hw_in, top, spring = 2.052, 1.12, 4.01, 2.4
    z0, z1 = -0.469, 0.523
    cz, dz = (z0 + z1) / 2, z1 - z0
    stone = (92, 88, 100)
    parts = [
        (stone, _box((-(hw_out + hw_in) / 2, top / 2, cz), (hw_out - hw_in, top, dz))),
        (stone, _box(((hw_out + hw_in) / 2, top / 2, cz), (hw_out - hw_in, top, dz))),
    ]
    n = 12
    arc = [(-hw_in * math.cos(math.pi * i / n), spring + hw_in * math.sin(math.pi * i / n)) for i in range(n + 1)]
    for (xa, ya), (xb, yb) in zip(arc, arc[1:]):
        # prism between the arc segment and the top edge, extruded along z
        p = [(xa, ya), (xb, yb), (xb, top), (xa, top)]
        f = [(x, y, z0) for x, y in p]
        b = [(x, y, z1) for x, y in p]
        tris = [
            (b[0], b[1], b[2]), (b[0], b[2], b[3]),  # +z face
            (f[1], f[0], f[3]), (f[1], f[3], f[2]),  # -z face
            (f[0], f[1], b[1]), (f[0], b[1], b[0]),  # underside
            (f[3], b[3], b[2]), (f[3], b[2], f[2]),  # top
        ]
        parts.append((stone, tris))
    return write_glb(GATE, parts, "gate_arch")


def build() -> dict[Path, bytes]:
    out = {GATE: gate_arch()}
    for name, (fn, bb) in MODELS.items():
        out[OUT / f"{name}.glb"] = write_glb(OUT / f"{name}.glb", fn(bb), name)
    return out


def main(argv: list[str]) -> int:
    files = build()
    if "--check" in argv:
        bad = [str(p.relative_to(ROOT)) for p, d in files.items() if not p.is_file() or p.read_bytes() != d]
        for b in bad:
            print("drift:", b)
        return 1 if bad else 0
    for p, d in files.items():
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(d)
        print("wrote", p.relative_to(ROOT), len(d))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
