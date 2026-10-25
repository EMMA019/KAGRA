"""HOP-BEAT's models, written by this script (CC0-1.0, no input assets).

    hop_bunny.glb       the hero, feet on y=0, facing +z (the camera looks at its back)
    hop_tower_{a,b,c}.glb  rooftop skyline blocks, 3 x 3 m footprint, base on y=0
    hop_windows_{a,b,c}.glb the lit window panels of each tower (a separate mesh,
                        so the page can give them a glow the tower does not get)
    hop_post.glb        a roadside lamp post (the lamp bulb itself is drawn by the game)
    hop_slime.glb       enemy: a stompable slime, feet on y=0, facing -z (toward the hero)
    hop_beetle.glb      enemy: a walking beetle, same frame
    hop_bat.glb         enemy: a hovering bat, bottom of the wings on y=0 (the game lifts it to
                        its flight height), same frame

    python3 tools/gen_assets/hop_models.py          # write all
    python3 tools/gen_assets/hop_models.py --check  # exit 1 on drift

Meshes are flat-coloured primitives from ``placeholder_models`` (one material per
colour). The bunny, towers and post carry baked occlusion in COLOR_0 (height and
facing, see ``_ao_for``). The game tints them per prop, so the window and post colours are white or
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


SLIME = (96, 226, 140)
SLIME_HI = (190, 255, 205)
SHELL = (255, 150, 60)
SHELL_DK = (150, 70, 40)
BAT = (170, 118, 250)
BAT_DK = (120, 80, 210)
YELLOW = (255, 232, 96)


def slime():
    """0.8 m wide, 0.54 m tall: a dome with a glossy highlight, two eyes and a tuft."""
    sph = pm._sphere
    parts = [
        (SLIME, sph((0, 0.25, 0.0), 0.40, 0.25, 0.38, 16, 8)),
        (SLIME, sph((0, 0.06, 0.0), 0.43, 0.06, 0.41, 16, 4)),  # the wobbly skirt
        (SLIME_HI, sph((0.14, 0.40, 0.12), 0.09, 0.05, 0.07, 8, 5)),  # shine
        (SLIME, sph((0, 0.49, 0.02), 0.06, 0.05, 0.06, 8, 5)),  # tuft
        (WHITE, sph((-0.15, 0.26, -0.30), 0.09, 0.10, 0.06, 10, 6)),  # eyes
        (WHITE, sph((0.15, 0.26, -0.30), 0.09, 0.10, 0.06, 10, 6)),
        (INK, sph((-0.15, 0.25, -0.35), 0.04, 0.05, 0.03, 8, 5)),
        (INK, sph((0.15, 0.25, -0.35), 0.04, 0.05, 0.03, 8, 5)),
    ]
    return parts


def beetle():
    """0.8 m long, 0.55 m tall: a round orange shell with spots, a dark head, six legs."""
    sph = pm._sphere
    parts = [
        (SHELL, sph((0, 0.30, 0.06), 0.36, 0.26, 0.40, 16, 8)),
        (SHELL_DK, pm._box((0, 0.545, 0.06), (0.05, 0.02, 0.56))),  # seam down the back
        (INK, sph((-0.17, 0.46, 0.20), 0.07, 0.03, 0.07, 8, 4)),  # spots
        (INK, sph((0.17, 0.46, 0.20), 0.07, 0.03, 0.07, 8, 4)),
        (INK, sph((-0.18, 0.46, -0.06), 0.06, 0.03, 0.06, 8, 4)),
        (INK, sph((0.18, 0.46, -0.06), 0.06, 0.03, 0.06, 8, 4)),
        (SHELL_DK, sph((0, 0.22, -0.36), 0.20, 0.17, 0.17, 12, 6)),  # head
        (WHITE, sph((-0.09, 0.28, -0.50), 0.055, 0.065, 0.04, 8, 5)),  # eyes
        (WHITE, sph((0.09, 0.28, -0.50), 0.055, 0.065, 0.04, 8, 5)),
        (INK, sph((-0.09, 0.28, -0.54), 0.025, 0.035, 0.02, 6, 4)),
        (INK, sph((0.09, 0.28, -0.54), 0.025, 0.035, 0.02, 6, 4)),
    ]
    for sx in (-1.0, 1.0):
        for z in (-0.18, 0.06, 0.30):
            parts.append((SHELL_DK, pm._box((sx * 0.37, 0.07, z), (0.16, 0.12, 0.09))))
        feeler = pm._box((sx * 0.10, 0.50, -0.50), (0.04, 0.04, 0.22))
        parts.append((SHELL_DK, _tilt_x(feeler, 0.50, -0.50, -0.5)))
    return parts


def bat():
    """1.4 m across, 0.66 m tall: a round body, two flat wings with a tip each, ears, yellow eyes."""
    sph = pm._sphere
    parts = [
        (BAT, sph((0, 0.30, 0.0), 0.22, 0.26, 0.20, 12, 7)),
        (BAT, sph((0, 0.50, 0.0), 0.17, 0.15, 0.16, 12, 6)),  # head
        (WHITE, sph((-0.075, 0.52, -0.14), 0.05, 0.06, 0.04, 8, 5)),
        (WHITE, sph((0.075, 0.52, -0.14), 0.05, 0.06, 0.04, 8, 5)),
        (YELLOW, sph((-0.075, 0.52, -0.17), 0.028, 0.04, 0.02, 6, 4)),
        (YELLOW, sph((0.075, 0.52, -0.17), 0.028, 0.04, 0.02, 6, 4)),
        (INK, sph((0, 0.44, -0.16), 0.03, 0.02, 0.02, 6, 4)),
    ]
    for sx in (-1.0, 1.0):
        parts.append((BAT_DK, sph((sx * 0.46, 0.34, 0.02), 0.30, 0.035, 0.24, 12, 4)))  # wing
        parts.append((BAT_DK, sph((sx * 0.68, 0.28, 0.04), 0.20, 0.030, 0.16, 10, 4)))  # tip
        parts.append((BAT, pm._cone((sx * 0.10, 0.68, 0.0), 0.06, 0.16, 8)))  # ear
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


def _ao_for(parts, floor=0.5, reach=0.22, under=0.35):
    """Baked occlusion for a standing model: darker toward the ground, under
    overhangs and where a surface faces down. Cheap and deterministic: it is
    height and normal, no ray casting. The game's lights do the rest."""
    ys = [v[1] for _, tris in parts for t in tris for v in t]
    lo, hi = min(ys), max(ys)
    span = max(hi - lo, 1e-3)

    def ao(pos, nor):
        u = min(1.0, max(0.0, (pos[1] - lo) / (span * reach)))
        ground = floor + (1.0 - floor) * (u * u * (3.0 - 2.0 * u))
        return ground * (1.0 - under * max(0.0, -nor[1]))

    return ao


TOWERS = {
    "a": dict(w=3.0, h=7.0, d=3.0, antenna=True, cols=3, rows=7),
    "b": dict(w=3.0, h=10.0, d=3.0, tank=True, cols=3, rows=10),
    "c": dict(w=3.0, h=13.0, d=3.0, steps=True, ac=True, cols=3, rows=13),
}


def build() -> dict[Path, bytes]:
    b = bunny()
    out = {OUT / "hop_bunny.glb": pm.write_glb(OUT / "hop_bunny.glb", b, "hop_bunny", ao=_ao_for(b, floor=0.62, reach=0.3))}
    for i, (k, t) in enumerate(TOWERS.items()):
        body = _tower_body(t["w"], t["h"], t["d"], t.get("tank", False), t.get("antenna", False), t.get("ac", False), t.get("steps", False))
        out[OUT / f"hop_tower_{k}.glb"] = pm.write_glb(OUT / f"hop_tower_{k}.glb", body, f"hop_tower_{k}", ao=_ao_for(body, floor=0.42, reach=0.18))
        win = _windows(t["w"], t["h"], t["d"], t["cols"], t["rows"], i)
        out[OUT / f"hop_windows_{k}.glb"] = pm.write_glb(OUT / f"hop_windows_{k}.glb", win, f"hop_windows_{k}")
    pst = post()
    out[OUT / "hop_post.glb"] = pm.write_glb(OUT / "hop_post.glb", pst, "hop_post", ao=_ao_for(pst, floor=0.5, reach=0.3))
    for name, shape in (("slime", slime), ("beetle", beetle), ("bat", bat)):
        path = OUT / f"hop_{name}.glb"
        out[path] = pm.write_glb(path, shape(), f"hop_{name}")
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
