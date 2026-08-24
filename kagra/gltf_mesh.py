"""Static glTF → Prop mesh. Venue halls stay on ``kagra.stage()``.

GPU 不要。``flatten_gltf`` は ``[x,y,z,nx,ny,nz,u,v]`` と AABB を返す。
"""
from __future__ import annotations

import json
import math
import struct
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

from kagra.vrma_player import _read_accessor, _read_glb_or_gltf


def is_gltf_name(model: str) -> bool:
    """``crate.glb`` / 絶対パス / ``.gltf``。プリミティブ名は False。"""
    return Path(str(model)).suffix.lower() in (".glb", ".gltf")


def resolve_gltf_path(name: str, *, root: Path | None = None) -> Path:
    """ファイルそのもの、なければ ``resolve_asset(GLTF, …)``。"""
    raw = Path(str(name))
    if raw.is_file():
        return raw.resolve()
    from kagra.contracts import AssetKind, resolve_asset

    key = raw.stem if raw.suffix else str(name)
    found = resolve_asset(AssetKind.GLTF, key, root=root)
    assert found is not None
    return Path(found)


@dataclass
class FlatMesh:
    """中心寄せした静的メッシュ。``image`` は埋め込み PNG/JPEG（無ければ None）。"""

    verts: list[list[float]]
    indices: list[int]
    aabb: tuple[float, float, float, float, float, float]
    image: Optional[bytes] = None
    metallic: float = 0.0
    roughness: float = 1.0
    base_color: tuple[float, float, float] = (1.0, 1.0, 1.0)


def _mat4_identity() -> list[float]:
    return [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]


def _mat4_mul(a: list[float], b: list[float]) -> list[float]:
    out = [0.0] * 16
    for row in range(4):
        for col in range(4):
            s = 0.0
            for k in range(4):
                s += a[row + k * 4] * b[k + col * 4]
            out[row + col * 4] = s
    return out


def _mat4_from_trs(t, r, s) -> list[float]:
    tx, ty, tz = t
    rx, ry, rz, rw = r
    sx, sy, sz = s
    x2, y2, z2 = rx * 2, ry * 2, rz * 2
    xx, yy, zz = rx * x2, ry * y2, rz * z2
    xy, xz, yz = rx * y2, rx * z2, ry * z2
    wx, wy, wz = rw * x2, rw * y2, rw * z2
    return [
        (1 - (yy + zz)) * sx, (xy + wz) * sx, (xz - wy) * sx, 0,
        (xy - wz) * sy, (1 - (xx + zz)) * sy, (yz + wx) * sy, 0,
        (xz + wy) * sz, (yz - wx) * sz, (1 - (xx + yy)) * sz, 0,
        tx, ty, tz, 1,
    ]


def _xform_point(m, x, y, z) -> tuple[float, float, float]:
    return (
        m[0] * x + m[4] * y + m[8] * z + m[12],
        m[1] * x + m[5] * y + m[9] * z + m[13],
        m[2] * x + m[6] * y + m[10] * z + m[14],
    )


def _xform_normal(m, x, y, z) -> tuple[float, float, float]:
    nx = m[0] * x + m[4] * y + m[8] * z
    ny = m[1] * x + m[5] * y + m[9] * z
    nz = m[2] * x + m[6] * y + m[10] * z
    leng = math.sqrt(nx * nx + ny * ny + nz * nz)
    if leng > 1e-8:
        nx /= leng
        ny /= leng
        nz /= leng
    return nx, ny, nz


def _node_local(node: dict) -> list[float]:
    if "matrix" in node:
        raw = [float(v) for v in node["matrix"]]
        if len(raw) == 16:
            return raw
    t = node.get("translation") or [0.0, 0.0, 0.0]
    r = node.get("rotation") or [0.0, 0.0, 0.0, 1.0]
    s = node.get("scale") or [1.0, 1.0, 1.0]
    return _mat4_from_trs(t, r, s)


def _node_worlds(nodes: list) -> list[list[float]]:
    parent: list[Optional[int]] = [None] * len(nodes)
    for i, node in enumerate(nodes):
        for ch in node.get("children") or []:
            parent[int(ch)] = i
    worlds: list[Optional[list[float]]] = [None] * len(nodes)
    pending = list(range(len(nodes)))
    for _ in range(len(nodes) + 1):
        nxt = []
        for i in pending:
            p = parent[i]
            if p is None:
                worlds[i] = _node_local(nodes[i])
            elif worlds[p] is not None:
                worlds[i] = _mat4_mul(worlds[p], _node_local(nodes[i]))
            else:
                nxt.append(i)
        if not nxt:
            break
        pending = nxt
    return [w if w is not None else _node_local(nodes[i]) for i, w in enumerate(worlds)]


def _first_image(gltf: dict, blob: bytes) -> Optional[bytes]:
    images = gltf.get("images") or []
    views = gltf.get("bufferViews") or []
    if not images:
        return None
    img = images[0]
    uri = img.get("uri")
    if isinstance(uri, str) and uri.startswith("data:"):
        import base64

        _, _, b64 = uri.partition(",")
        try:
            return base64.b64decode(b64)
        except Exception:
            return None
    bv_idx = img.get("bufferView")
    if bv_idx is None or int(bv_idx) >= len(views):
        return None
    bv = views[int(bv_idx)]
    off = int(bv.get("byteOffset") or 0)
    size = int(bv.get("byteLength") or 0)
    if size < 8 or off + size > len(blob):
        return None
    return blob[off:off + size]


def _pbr_from_gltf(gltf: dict) -> tuple[float, float, tuple[float, float, float]]:
    """最初のプリミティブの ``pbrMetallicRoughness``。無ければ Lambert 既定。"""
    metallic = 0.0
    roughness = 1.0
    base = (1.0, 1.0, 1.0)
    materials = gltf.get("materials") or []
    mat_idx = None
    for mesh in gltf.get("meshes") or []:
        for prim in mesh.get("primitives") or []:
            if "material" in prim:
                mat_idx = int(prim["material"])
                break
        if mat_idx is not None:
            break
    if mat_idx is None or mat_idx < 0 or mat_idx >= len(materials):
        return metallic, roughness, base
    pbr = materials[mat_idx].get("pbrMetallicRoughness") or {}
    if "metallicFactor" in pbr:
        metallic = float(pbr["metallicFactor"])
    if "roughnessFactor" in pbr:
        roughness = float(pbr["roughnessFactor"])
    bc = pbr.get("baseColorFactor")
    if isinstance(bc, (list, tuple)) and len(bc) >= 3:
        base = (float(bc[0]), float(bc[1]), float(bc[2]))
    return metallic, roughness, base


def flatten_gltf(path: str | Path, *, center: bool = True) -> FlatMesh:
    """静的プリミティブを 1 メッシュに畳む。スキンは無視（部品用）。"""
    gltf, blob = _read_glb_or_gltf(path)
    meshes = gltf.get("meshes") or []
    if not meshes:
        raise ValueError(f"gltf has no meshes: {path}")
    nodes = gltf.get("nodes") or []
    worlds = _node_worlds(nodes) if nodes else []
    jobs: list[tuple[int, list[float]]] = []
    for i, node in enumerate(nodes):
        if "mesh" in node:
            jobs.append((int(node["mesh"]), worlds[i]))
    if not jobs:
        for mi in range(len(meshes)):
            jobs.append((mi, _mat4_identity()))

    verts: list[list[float]] = []
    indices: list[int] = []
    for mi, world in jobs:
        mesh = meshes[mi]
        for prim in mesh.get("primitives") or []:
            mode = int(prim.get("mode", 4))
            if mode != 4:
                continue
            attrs = prim.get("attributes") or {}
            if "POSITION" not in attrs:
                continue
            pos = _read_accessor(gltf, blob, int(attrs["POSITION"]))
            nrm = _read_accessor(gltf, blob, int(attrs["NORMAL"])) if "NORMAL" in attrs else []
            uvs = _read_accessor(gltf, blob, int(attrs["TEXCOORD_0"])) if "TEXCOORD_0" in attrs else []
            if prim.get("indices") is not None:
                raw_idx = _read_accessor(gltf, blob, int(prim["indices"]))
                face = [int(x) for x in raw_idx]
            else:
                face = list(range(len(pos)))
            base = len(verts)
            for i, p in enumerate(pos):
                x, y, z = float(p[0]), float(p[1]), float(p[2])
                wx, wy, wz = _xform_point(world, x, y, z)
                if i < len(nrm):
                    nx, ny, nz = float(nrm[i][0]), float(nrm[i][1]), float(nrm[i][2])
                    nx, ny, nz = _xform_normal(world, nx, ny, nz)
                else:
                    nx, ny, nz = 0.0, 1.0, 0.0
                if i < len(uvs):
                    u, v = float(uvs[i][0]), float(uvs[i][1])
                else:
                    u, v = 0.0, 0.0
                verts.append([wx, wy, wz, nx, ny, nz, u, v])
            indices.extend(base + i for i in face)

    if not verts:
        raise ValueError(f"gltf has no triangle mesh: {path}")

    xs = [v[0] for v in verts]
    ys = [v[1] for v in verts]
    zs = [v[2] for v in verts]
    aabb = (min(xs), min(ys), min(zs), max(xs), max(ys), max(zs))
    if center:
        cx = 0.5 * (aabb[0] + aabb[3])
        cy = 0.5 * (aabb[1] + aabb[4])
        cz = 0.5 * (aabb[2] + aabb[5])
        for v in verts:
            v[0] -= cx
            v[1] -= cy
            v[2] -= cz
        aabb = (
            aabb[0] - cx, aabb[1] - cy, aabb[2] - cz,
            aabb[3] - cx, aabb[4] - cy, aabb[5] - cz,
        )
    metallic, roughness, base = _pbr_from_gltf(gltf)
    return FlatMesh(
        verts=verts,
        indices=indices,
        aabb=aabb,
        image=_first_image(gltf, blob),
        metallic=metallic,
        roughness=roughness,
        base_color=base,
    )


def _pad4(n: int) -> int:
    return (4 - (n % 4)) % 4


def write_unit_cube_glb(path: str | Path) -> Path:
    """一辺 1 の箱（中心原点）。テストと Prop Garden 用。"""
    faces = (
        ((0.0, 1.0, 0.0), (
            (-0.5, 0.5, -0.5, 0.0, 0.0), (0.5, 0.5, -0.5, 1.0, 0.0),
            (0.5, 0.5, 0.5, 1.0, 1.0), (-0.5, 0.5, 0.5, 0.0, 1.0),
        )),
        ((0.0, -1.0, 0.0), (
            (-0.5, -0.5, 0.5, 0.0, 0.0), (0.5, -0.5, 0.5, 1.0, 0.0),
            (0.5, -0.5, -0.5, 1.0, 1.0), (-0.5, -0.5, -0.5, 0.0, 1.0),
        )),
        ((1.0, 0.0, 0.0), (
            (0.5, -0.5, -0.5, 0.0, 0.0), (0.5, -0.5, 0.5, 1.0, 0.0),
            (0.5, 0.5, 0.5, 1.0, 1.0), (0.5, 0.5, -0.5, 0.0, 1.0),
        )),
        ((-1.0, 0.0, 0.0), (
            (-0.5, -0.5, 0.5, 0.0, 0.0), (-0.5, -0.5, -0.5, 1.0, 0.0),
            (-0.5, 0.5, -0.5, 1.0, 1.0), (-0.5, 0.5, 0.5, 0.0, 1.0),
        )),
        ((0.0, 0.0, 1.0), (
            (-0.5, -0.5, 0.5, 0.0, 0.0), (0.5, -0.5, 0.5, 1.0, 0.0),
            (0.5, 0.5, 0.5, 1.0, 1.0), (-0.5, 0.5, 0.5, 0.0, 1.0),
        )),
        ((0.0, 0.0, -1.0), (
            (0.5, -0.5, -0.5, 0.0, 0.0), (-0.5, -0.5, -0.5, 1.0, 0.0),
            (-0.5, 0.5, -0.5, 1.0, 1.0), (0.5, 0.5, -0.5, 0.0, 1.0),
        )),
    )
    pos: list[float] = []
    nrm: list[float] = []
    uv: list[float] = []
    idx: list[int] = []
    for (nx, ny, nz), corners in faces:
        base = len(pos) // 3
        for x, y, z, u, v in corners:
            pos.extend((x, y, z))
            nrm.extend((nx, ny, nz))
            uv.extend((u, v))
        idx.extend((base, base + 1, base + 2, base, base + 2, base + 3))

    pos_b = struct.pack(f"<{len(pos)}f", *pos)
    nrm_b = struct.pack(f"<{len(nrm)}f", *nrm)
    uv_b = struct.pack(f"<{len(uv)}f", *uv)
    idx_b = struct.pack(f"<{len(idx)}H", *idx)
    idx_pad = b"\x00" * _pad4(len(idx_b))

    off_pos = 0
    off_nrm = off_pos + len(pos_b)
    off_uv = off_nrm + len(nrm_b)
    off_idx = off_uv + len(uv_b)
    blob = pos_b + nrm_b + uv_b + idx_b + idx_pad

    xs, ys, zs = pos[0::3], pos[1::3], pos[2::3]
    gltf = {
        "asset": {"version": "2.0", "generator": "kagra.gltf_mesh"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": "cube"}],
        "meshes": [{
            "name": "cube",
            "primitives": [{
                "attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2},
                "indices": 3,
            }],
        }],
        "accessors": [
            {
                "bufferView": 0, "componentType": 5126, "count": len(pos) // 3,
                "type": "VEC3", "min": [min(xs), min(ys), min(zs)],
                "max": [max(xs), max(ys), max(zs)],
            },
            {
                "bufferView": 1, "componentType": 5126, "count": len(nrm) // 3,
                "type": "VEC3",
            },
            {
                "bufferView": 2, "componentType": 5126, "count": len(uv) // 2,
                "type": "VEC2",
            },
            {
                "bufferView": 3, "componentType": 5123, "count": len(idx),
                "type": "SCALAR",
            },
        ],
        "bufferViews": [
            {"buffer": 0, "byteOffset": off_pos, "byteLength": len(pos_b)},
            {"buffer": 0, "byteOffset": off_nrm, "byteLength": len(nrm_b)},
            {"buffer": 0, "byteOffset": off_uv, "byteLength": len(uv_b)},
            {"buffer": 0, "byteOffset": off_idx, "byteLength": len(idx_b)},
        ],
        "buffers": [{"byteLength": len(blob)}],
    }
    json_b = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_b += b" " * _pad4(len(json_b))
    total = 12 + 8 + len(json_b) + 8 + len(blob)
    header = struct.pack("<4sII", b"glTF", 2, total)
    json_chunk = struct.pack("<II", len(json_b), 0x4E4F534A) + json_b
    bin_chunk = struct.pack("<II", len(blob), 0x004E4942) + blob
    dest = Path(path)
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(header + json_chunk + bin_chunk)
    return dest.resolve()
