"""HOP-BEAT's models, written by this script (CC0-1.0, no input assets).

    hop_bunny.glb       the hero, feet on y=0, facing +z (the camera looks at its back)
    hop_tower_{a,b,c}.glb  rooftop skyline blocks, 3 x 3 m footprint, base on y=0
    hop_windows_{a,b,c}.glb the lit window panels of each tower (a separate mesh,
                        so the page can give them a glow the tower does not get)
    hop_post.glb        a roadside lamp post (the lamp bulb itself is drawn by the game)

    python3 tools/gen_assets/hop_models.py          # write all
    python3 tools/gen_assets/hop_models.py --check  # exit 1 on drift

Meshes are flat-coloured primitives from ``placeholder_models`` (one material per
colour). The game tints them per prop, so the window and post colours are white or
grey here. Standard library only; the output is deterministic.
"""
from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import placeholder_models as pm  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "assets" / "models" / "hop_beat"

CREAM = (255, 238, 205)
PINK = (255, 150, 190)
WHITE = (255, 255, 255)
INK = (30, 20, 44)
CONCRETE = (92, 86, 128)
DARK = (54, 48, 84)
METAL = (120, 124, 150)


def _tilt_x(tris, cy, cz, ang):
    """Rotate triangles about the x axis through (cy, cz); +ang leans the top toward -z."""
    c, s = math.cos(ang), math.sin(ang)

    def r(v):
        x, y, z = v
        dy, dz = y - cy, z - cz
        return (x, cy + dy * c - dz * s, cz + dy * s + dz * c)

    return [tuple(r(v) for v in t) for t in tris]


def bunny():
    sph = pm._sphere
    parts = [
        (CREAM, sph((0, 0.40, 0.0), 0.40, 0.38, 0.42, 14, 8)),  # body
        (CREAM, sph((0, 0.60, 0.30), 0.27, 0.25, 0.26, 14, 8)),  # head
        (WHITE, sph((0, 0.36, -0.42), 0.13, 0.13, 0.13, 10, 6)),  # tail
        (INK, sph((-0.13, 0.66, 0.51), 0.045, 0.06, 0.03, 8, 5)),  # eyes
        (INK, sph((0.13, 0.66, 0.51), 0.045, 0.06, 0.03, 8, 5)),
        (PINK, sph((0, 0.58, 0.55), 0.04, 0.03, 0.03, 8, 5)),  # nose
        (PINK, sph((-0.20, 0.57, 0.46), 0.06, 0.045, 0.02, 8, 5)),  # cheeks
        (PINK, sph((0.20, 0.57, 0.46), 0.06, 0.045, 0.02, 8, 5)),
        (CREAM, sph((-0.15, 0.05, 0.22), 0.09, 0.05, 0.14, 8, 5)),  # feet
        (CREAM, sph((0.15, 0.05, 0.22), 0.09, 0.05, 0.14, 8, 5)),
    ]
    for sx in (-1.0, 1.0):
        ear = sph((sx * 0.11, 0.98, 0.22), 0.065, 0.24, 0.045, 10, 8)
        inner = sph((sx * 0.11, 0.98, 0.245), 0.035, 0.19, 0.02, 8, 6)
        parts.append((CREAM, _tilt_x(ear, 0.80, 0.26, 0.28)))
        parts.append((PINK, _tilt_x(inner, 0.80, 0.26, 0.28)))
    return parts


def _tower_body(w, h, d, tank=False, antenna=False, ac=False, steps=False):
    parts = [(CONCRETE, pm._box((0, h / 2, 0), (w, h, d)))]
    # a darker roof parapet and a cap
    parts.append((DARK, pm._box((0, h + 0.1, 0), (w + 0.12, 0.2, d + 0.12))))
    if steps:
        parts.append((CONCRETE, pm._box((0.35, h + 0.2 + 0.8, -0.2), (w * 0.55, 1.6, d * 0.55))))
        parts.append((DARK, pm._box((0.35, h + 0.2 + 1.65, -0.2), (w * 0.55 + 0.1, 0.12, d * 0.55 + 0.1))))
    if tank:
        parts.append((METAL, pm._cyl((-0.6, h + 0.2 + 0.7, 0.5), 0.55, 1.2, 12)))
        parts.append((DARK, pm._cone((-0.6, h + 0.2 + 1.5, 0.5), 0.62, 0.5, 12)))
        for sx, sz in ((-0.95, 0.15), (-0.25, 0.15), (-0.95, 0.85), (-0.25, 0.85)):
            parts.append((DARK, pm._box((sx, h + 0.2 + 0.0 + 0.1, sz), (0.08, 0.2, 0.08))))
    if antenna:
        parts.append((METAL, pm._cyl((0.7, h + 0.2 + 1.6, -0.6), 0.05, 3.2, 6)))
        parts.append((METAL, pm._box((0.7, h + 0.2 + 2.6, -0.6), (0.7, 0.05, 0.05))))
        parts.append((METAL, pm._box((0.7, h + 0.2 + 2.2, -0.6), (0.45, 0.05, 0.05))))
    if ac:
        for i, x in enumerate((-0.7, 0.0, 0.7)):
            parts.append((METAL, pm._box((x, h + 0.2 + 0.25, 0.7), (0.5, 0.5, 0.5))))
            parts.append((INK, pm._cyl((x, h + 0.2 + 0.52, 0.7), 0.17, 0.04, 10)))
    return parts


def _windows(w, h, d, cols, rows, seed):
    """Thin lit panels on the four faces, skipping some by a fixed pattern."""
    parts = []
    off = 0.015
    mx, my = 0.35, 0.8
    cw = (w - 2 * mx) / cols
    rh = (h - my - 0.6) / rows
    pw, ph = cw * 0.55, rh * 0.5
    for face in range(4):
        for r in range(rows):
            for c in range(cols):
                if ((r * 7 + c * 3 + face * 5 + seed) % 5) in (0, 3):
                    continue
                u = -w / 2 + mx + cw * (c + 0.5)
                y = my + rh * (r + 0.5)
                if face == 0:
                    parts.append((WHITE, pm._box((u, y, d / 2 + off), (pw, ph, 0.03))))
                elif face == 1:
                    parts.append((WHITE, pm._box((u, y, -d / 2 - off), (pw, ph, 0.03))))
                elif face == 2:
                    parts.append((WHITE, pm._box((w / 2 + off, y, u), (0.03, ph, pw))))
                else:
                    parts.append((WHITE, pm._box((-w / 2 - off, y, u), (0.03, ph, pw))))
    return parts


def post():
    return [
        (DARK, pm._cyl((0, 0.06, 0), 0.16, 0.12, 8)),
        (METAL, pm._cyl((0, 0.85, 0), 0.05, 1.7, 8)),
        (DARK, pm._cyl((0, 1.74, 0), 0.10, 0.12, 8)),
        (DARK, pm._cyl((0, 1.80, 0), 0.14, 0.04, 10)),
    ]


TOWERS = {
    "a": dict(w=3.0, h=7.0, d=3.0, antenna=True, cols=3, rows=7),
    "b": dict(w=3.0, h=10.0, d=3.0, tank=True, cols=3, rows=10),
    "c": dict(w=3.0, h=13.0, d=3.0, steps=True, ac=True, cols=3, rows=13),
}


def build() -> dict[Path, bytes]:
    out = {OUT / "hop_bunny.glb": pm.write_glb(OUT / "hop_bunny.glb", bunny(), "hop_bunny")}
    for i, (k, t) in enumerate(TOWERS.items()):
        body = _tower_body(t["w"], t["h"], t["d"], t.get("tank", False), t.get("antenna", False), t.get("ac", False), t.get("steps", False))
        out[OUT / f"hop_tower_{k}.glb"] = pm.write_glb(OUT / f"hop_tower_{k}.glb", body, f"hop_tower_{k}")
        win = _windows(t["w"], t["h"], t["d"], t["cols"], t["rows"], i)
        out[OUT / f"hop_windows_{k}.glb"] = pm.write_glb(OUT / f"hop_windows_{k}.glb", win, f"hop_windows_{k}")
    out[OUT / "hop_post.glb"] = pm.write_glb(OUT / "hop_post.glb", post(), "hop_post")
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
