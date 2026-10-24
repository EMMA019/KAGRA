"""The bundled demo road grid (`demo_grid.kagra.json`), written by this script.

A plain 3 x 6 grid of streets with a few boxes standing in for buildings. It
is made from numbers here, not from any map data, so it has no outside source.
It is the unit-test and fallback map (`RoadNetwork::demo_city`).

    python3 tools/gen_assets/demo_grid.py          # write
    python3 tools/gen_assets/demo_grid.py --check  # exit 1 on drift
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "kagra-shared" / "assets" / "maps" / "demo_grid.kagra.json"

COLS, ROWS = 3, 6
DX, DZ = 100.0, 120.0
X0 = -DX


def build() -> dict:
    nodes, edges, ids = [], [], {}
    for r in range(ROWS):
        for c in range(COLS):
            ids[(c, r)] = len(nodes)
            nodes.append({"id": len(nodes), "x": X0 + c * DX, "z": r * DZ})

    def edge(a, b, hw, w):
        pa = nodes[a]
        pb = nodes[b]
        edges.append({
            "id": len(edges), "from": a, "to": b, "highway": hw, "width": w, "oneway": False,
            "points": [[pa["x"], pa["z"]], [pb["x"], pb["z"]]],
        })

    for r in range(ROWS):
        for c in range(COLS):
            if r + 1 < ROWS:
                edge(ids[(c, r)], ids[(c, r + 1)], "primary" if c == 1 else "residential", 14 if c == 1 else 8)
            if c + 1 < COLS:
                edge(ids[(c, r)], ids[(c + 1, r)], "secondary" if r % 2 == 0 else "tertiary", 10 if r % 2 == 0 else 9)
    buildings = []
    sizes = [(18, 14, 22), (16, 20, 18), (20, 12, 24), (14, 28, 16), (22, 16, 20)]
    k = 0
    for r in range(ROWS - 1):
        for c in range(COLS - 1):
            sx, sy, sz = sizes[k % len(sizes)]
            buildings.append({"x": X0 + c * DX + DX / 2, "z": r * DZ + DZ / 2, "yaw": 0.0, "sx": sx, "sy": sy, "sz": sz})
            k += 1
    return {
        "version": 1,
        "name": "demo_grid",
        "origin_lonlat": [0.0, 0.0],
        "units": "meters",
        "axes": "x_east_z_north",
        "nodes": nodes,
        "edges": edges,
        "buildings": buildings,
        "spawn_node": ids[(1, 0)],
        "mission_end_node": ids[(1, ROWS - 1)],
    }


def main(argv: list[str]) -> int:
    text = json.dumps(build(), indent=1) + "\n"
    if "--check" in argv:
        return 0 if OUT.is_file() and OUT.read_text(encoding="utf-8") == text else 1
    OUT.write_text(text, encoding="utf-8")
    print("wrote", OUT.relative_to(ROOT))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
