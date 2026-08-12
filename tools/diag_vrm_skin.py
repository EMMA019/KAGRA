"""VRM のスキン構成とノード順序を診断する。

KAGRA のスキニング実装が前提にしている条件
  1. skin.joints の要素数 <= 256（シェーダのパレット上限）
  2. 親ノードが子ノードより先の index に並んでいる（recompute_world が
     単純な昇順ループで親を先に解決するため）
を満たしているかを確認する。
"""
from __future__ import annotations

import json
import struct
import sys


def load_gltf_json(path: str) -> dict:
    with open(path, "rb") as f:
        data = f.read()
    if data[0:4] != b"glTF":
        raise SystemExit(f"{path} is not a GLB/VRM")
    offset = 12
    while offset + 8 <= len(data):
        chunk_len = struct.unpack("<I", data[offset : offset + 4])[0]
        chunk_type = struct.unpack("<I", data[offset + 4 : offset + 8])[0]
        chunk = data[offset + 8 : offset + 8 + chunk_len]
        if chunk_type == 0x4E4F534A:
            return json.loads(chunk.decode("utf-8").rstrip("\0"))
        offset += 8 + chunk_len
    raise SystemExit("no JSON chunk")


def main(path: str) -> None:
    g = load_gltf_json(path)
    nodes = g.get("nodes", [])
    skins = g.get("skins", [])
    meshes = g.get("meshes", [])

    print(f"file            : {path}")
    print(f"nodes           : {len(nodes)}")
    print(f"skins           : {len(skins)}")
    print(f"meshes          : {len(meshes)}")

    # --- 1. パレット上限 ---
    print("\n[1] skin.joints count vs 256 palette limit")
    over = False
    for i, s in enumerate(skins):
        n = len(s.get("joints", []))
        flag = "OVER LIMIT" if n > 256 else "ok"
        if n > 256:
            over = True
        print(f"  skin[{i}]: joints={n}  ({flag})")

    # --- 2. 親子の index 順序 ---
    parent = {}
    for ni, node in enumerate(nodes):
        for c in node.get("children", []):
            parent[c] = ni

    inversions = [(c, p) for c, p in parent.items() if c < p]
    print("\n[2] parent-before-child ordering")
    print(f"  nodes with a parent : {len(parent)}")
    print(f"  child index < parent index : {len(inversions)}")
    for c, p in inversions[:15]:
        cn = nodes[c].get("name", "?")
        pn = nodes[p].get("name", "?")
        print(f"    node[{c}] {cn!r}  has parent node[{p}] {pn!r}")
    if len(inversions) > 15:
        print(f"    ... and {len(inversions) - 15} more")

    # --- 3. 階層の深さ ---
    def depth(n: int, seen=None) -> int:
        seen = seen or set()
        d = 0
        while n in parent and n not in seen:
            seen.add(n)
            n = parent[n]
            d += 1
        return d

    max_depth = max((depth(n) for n in range(len(nodes))), default=0)
    print(f"\n[3] max hierarchy depth : {max_depth}")

    # --- 4. mesh -> skin ---
    print("\n[4] mesh -> skin binding")
    mesh_to_skin = {}
    for ni, node in enumerate(nodes):
        if "mesh" in node:
            mesh_to_skin[node["mesh"]] = node.get("skin")
    for mi, mesh in enumerate(meshes):
        si = mesh_to_skin.get(mi, "NOT IN ANY NODE")
        name = mesh.get("name", "?")
        prims = len(mesh.get("primitives", []))
        has_joints = any(
            "JOINTS_0" in p.get("attributes", {}) for p in mesh.get("primitives", [])
        )
        print(
            f"  mesh[{mi}] {name!r}: skin={si}, primitives={prims}, JOINTS_0={has_joints}"
        )

    # --- 5. 実際に使われている joint index の最大値 ---
    print("\n[5] verdict")
    if over:
        print("  - skin.joints exceeds the 256-matrix palette")
    if inversions:
        print(
            "  - node order is NOT parent-first: recompute_world() will read a stale"
        )
        print("    parent world matrix for those nodes")
    distinct_skins = {s for s in mesh_to_skin.values() if s is not None}
    if len(distinct_skins) > 1:
        print(f"  - multiple skins in use {sorted(distinct_skins)}: a single shared")
        print("    skinning uniform buffer cannot serve them all")
    if not (over or inversions or len(distinct_skins) > 1):
        print("  - none of the three suspected conditions triggered")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "assets/Emma.vrm")
