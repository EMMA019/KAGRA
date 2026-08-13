#!/usr/bin/env python3
"""Bake an OpenStreetMap drive network into a kagra-shared map JSON.

Uses OSMnx (no QGIS required). Output is local meters: x=east, z=north,
origin at the bbox / place centroid.

Example:
  pip install -r tools/requirements-osm.txt
  python tools/osm_bake.py --place "Shibuya, Tokyo, Japan" --out kagra-shared/assets/maps/shibuya_demo.kagra.json
  python tools/osm_bake.py --bbox 35.655 35.662 139.696 139.705 --out kagra-shared/assets/maps/shibuya_demo.kagra.json
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path


def _require_osmnx():
    try:
        import osmnx as ox  # noqa: F401
        import networkx as nx  # noqa: F401
    except ImportError as e:
        print(
            "osmnx is required. Install with:\n"
            "  pip install -r tools/requirements-osm.txt\n"
            f"({e})",
            file=sys.stderr,
        )
        sys.exit(1)


def highway_width(highway) -> float:
    if isinstance(highway, list):
        highway = highway[0] if highway else "residential"
    table = {
        "motorway": 18.0,
        "motorway_link": 10.0,
        "trunk": 16.0,
        "trunk_link": 10.0,
        "primary": 14.0,
        "primary_link": 9.0,
        "secondary": 11.0,
        "secondary_link": 8.0,
        "tertiary": 9.0,
        "tertiary_link": 7.0,
        "residential": 8.0,
        "living_street": 7.0,
        "unclassified": 8.0,
        "service": 6.0,
    }
    return table.get(str(highway), 8.0)


def simplify_line(coords: list[tuple[float, float]], min_step: float) -> list[list[float]]:
    if not coords:
        return []
    out = [coords[0]]
    for p in coords[1:]:
        dx = p[0] - out[-1][0]
        dy = p[1] - out[-1][1]
        if math.hypot(dx, dy) >= min_step:
            out.append(p)
    if out[-1] != coords[-1]:
        out.append(coords[-1])
    if len(out) == 1 and len(coords) >= 2:
        out.append(coords[-1])
    return [[float(x), float(y)] for x, y in out]


def bake_graph(G, name: str, origin_lonlat: tuple[float, float], min_edge_m: float = 8.0):
    import networkx as nx
    import osmnx as ox

    # Project to meters (UTM), then shift so centroid ~ origin.
    Gp = ox.project_graph(G)
    nodes_gdf, edges_gdf = ox.graph_to_gdfs(Gp)

    ox_cx = float(nodes_gdf.geometry.x.mean())
    ox_cy = float(nodes_gdf.geometry.y.mean())

    # Remap OSMid -> compact id
    osm_ids = list(nodes_gdf.index)
    id_map = {osm: i for i, osm in enumerate(osm_ids)}

    nodes = []
    for osm, row in nodes_gdf.iterrows():
        x = float(row.geometry.x) - ox_cx  # east
        z = float(row.geometry.y) - ox_cy  # north
        nodes.append({"id": id_map[osm], "x": round(x, 3), "z": round(z, 3)})

    edges = []
    eid = 0
    for _, row in edges_gdf.iterrows():
        u, v = row.name[0], row.name[1]
        if u not in id_map or v not in id_map:
            continue
        geom = row.geometry
        if geom is None or geom.is_empty:
            continue
        if geom.geom_type == "MultiLineString":
            # take longest part
            geom = max(geom.geoms, key=lambda g: g.length)
        coords = [(float(x) - ox_cx, float(y) - ox_cy) for x, y in geom.coords]
        points = simplify_line(coords, min_step=4.0)
        if len(points) < 2:
            continue
        length = 0.0
        for a, b in zip(points, points[1:]):
            length += math.hypot(b[0] - a[0], b[1] - a[1])
        if length < min_edge_m:
            continue
        oneway = bool(row.get("oneway", False))
        if isinstance(row.get("oneway"), str):
            oneway = row.get("oneway") in ("yes", "true", "1")
        edges.append(
            {
                "id": eid,
                "from": id_map[u],
                "to": id_map[v],
                "highway": (
                    row.get("highway")
                    if isinstance(row.get("highway"), str)
                    else (row.get("highway") or ["residential"])[0]
                ),
                "width": round(highway_width(row.get("highway")), 2),
                "oneway": oneway,
                "points": [[round(p[0], 3), round(p[1], 3)] for p in points],
            }
        )
        eid += 1

    # Spawn = southernmost node, mission end = northernmost reachable.
    spawn = min(nodes, key=lambda n: n["z"])["id"]
    end = max(nodes, key=lambda n: n["z"])["id"]

    # Buildings optional — leave empty here; filled by a later pass.
    return {
        "version": 1,
        "name": name,
        "origin_lonlat": [origin_lonlat[0], origin_lonlat[1]],
        "units": "meters",
        "axes": "x_east_z_north",
        "nodes": nodes,
        "edges": edges,
        "buildings": [],
        "spawn_node": spawn,
        "mission_end_node": end,
        "meta": {
            "node_count": len(nodes),
            "edge_count": len(edges),
            "tool": "osm_bake.py",
        },
    }


def main() -> int:
    _require_osmnx()
    import osmnx as ox

    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--place", type=str, help='e.g. "Shibuya, Tokyo, Japan"')
    ap.add_argument(
        "--bbox",
        nargs=4,
        type=float,
        metavar=("SOUTH", "NORTH", "WEST", "EAST"),
        help="WGS84 bbox",
    )
    ap.add_argument(
        "--out",
        type=Path,
        default=Path("kagra-shared/assets/maps/osm_bake.kagra.json"),
    )
    ap.add_argument("--name", type=str, default=None)
    ap.add_argument("--max-dist", type=float, default=900.0, help="place mode: radius meters")
    args = ap.parse_args()

    if args.bbox:
        south, north, west, east = args.bbox
        # osmnx 2.x: bbox = (left/west, bottom/south, right/east, top/north) as Polygon
        # graph_from_bbox signature varies; use bbox tuple (north, south, east, west) for 1.x
        try:
            G = ox.graph_from_bbox(north, south, east, west, network_type="drive", simplify=True)
        except TypeError:
            G = ox.graph_from_bbox(
                bbox=(west, south, east, north),
                network_type="drive",
                simplify=True,
            )
        origin = ((west + east) * 0.5, (south + north) * 0.5)
        name = args.name or f"bbox_{south:.3f}_{east:.3f}"
    elif args.place:
        G = ox.graph_from_place(args.place, network_type="drive", simplify=True)
        # clip by distance from center to keep demo small
        cent = ox.geocode(args.place)
        # cent may be (lat, lon)
        if isinstance(cent, (list, tuple)) and len(cent) == 2:
            lat, lon = float(cent[0]), float(cent[1])
        else:
            lat, lon = 35.659, 139.701
        try:
            G = ox.truncate.truncate_graph_dist(G, ox.nearest_nodes(G, lon, lat), dist=args.max_dist)
        except Exception:
            pass
        origin = (lon, lat)
        name = args.name or args.place.split(",")[0].strip().lower().replace(" ", "_")
    else:
        # Default: small Shibuya-ish bbox
        south, north, west, east = 35.6565, 35.6615, 139.6985, 139.7045
        try:
            G = ox.graph_from_bbox(north, south, east, west, network_type="drive", simplify=True)
        except TypeError:
            G = ox.graph_from_bbox(
                bbox=(west, south, east, north),
                network_type="drive",
                simplify=True,
            )
        origin = ((west + east) * 0.5, (south + north) * 0.5)
        name = args.name or "shibuya_demo"

    data = bake_graph(G, name=name, origin_lonlat=origin)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
    print(
        f"wrote {args.out}  nodes={len(data['nodes'])} edges={len(data['edges'])} name={data['name']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
