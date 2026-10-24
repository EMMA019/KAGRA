"""Import checks for GLB, glTF and FBX (stdlib parser, no trimesh)."""

from __future__ import annotations

import base64
import importlib.util
import json
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("import_asset", ROOT / "tools" / "import_asset.py")
assert _spec is not None and _spec.loader is not None
ia = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ia)


def png(w: int, h: int) -> bytes:
    def chunk(t: bytes, d: bytes) -> bytes:
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))

    raw = b"".join(b"\x00" + b"\x80\x80\x80" * w for _ in range(h))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


def make_gltf(tris=1, tex=None, ext_used=(), ext_required=(), nodes_using=1):
    verts = tris * 3
    pos = struct.pack("<" + "f" * (verts * 3), *([0.0] * (verts * 3)))
    blob = pos
    doc = {
        "asset": {"version": "2.0"},
        "scene": 0, "scenes": [{"nodes": list(range(nodes_using))}],
        "nodes": [{"mesh": 0} for _ in range(nodes_using)],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}],
        "accessors": [{"bufferView": 0, "componentType": 5126, "count": verts, "type": "VEC3"}],
        "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": len(pos)}],
        "buffers": [{"byteLength": len(pos)}],
    }
    if tex:
        data = png(*tex)
        pad = (-len(blob)) % 4
        blob += b"\0" * pad
        doc["bufferViews"].append({"buffer": 0, "byteOffset": len(blob), "byteLength": len(data)})
        blob += data
        doc["images"] = [{"bufferView": 1, "mimeType": "image/png"}]
        doc["buffers"][0]["byteLength"] = len(blob)
    if ext_used:
        doc["extensionsUsed"] = list(ext_used)
    if ext_required:
        doc["extensionsRequired"] = list(ext_required)
    return doc, blob


def write_glb(path: Path, doc: dict, blob: bytes) -> Path:
    j = json.dumps(doc).encode()
    j += b" " * ((-len(j)) % 4)
    b = blob + b"\0" * ((-len(blob)) % 4)
    total = 12 + 8 + len(j) + 8 + len(b)
    path.write_bytes(b"glTF" + struct.pack("<II", 2, total) + struct.pack("<I4s", len(j), b"JSON") + j
                     + struct.pack("<I4s", len(b), b"BIN\0") + b)
    return path


def run(path, **kw):
    return ia.run(path, ledger_check=False, **kw)


def test_triangles_and_embedded_texture_size(tmp_path):
    doc, blob = make_gltf(tris=12, tex=(64, 32))
    rep = run(write_glb(tmp_path / "a.glb", doc, blob))
    assert rep["errors"] == []
    assert rep["triangles_unique"] == 12
    assert rep["max_texture_side"] == 64


def test_instancing_counts_every_node(tmp_path):
    doc, blob = make_gltf(tris=10, nodes_using=5)
    rep = run(write_glb(tmp_path / "a.glb", doc, blob))
    assert (rep["triangles_unique"], rep["triangles_instanced"]) == (10, 50)


def test_triangle_budget(tmp_path):
    doc, blob = make_gltf(tris=30)
    path = write_glb(tmp_path / "a.glb", doc, blob)
    assert run(path, overrides={"max_triangles": 40})["errors"] == []
    assert any("triangles exceeds" in e for e in run(path, overrides={"max_triangles": 20})["errors"])


def test_texture_budget(tmp_path):
    doc, blob = make_gltf(tex=(256, 128))
    path = write_glb(tmp_path / "a.glb", doc, blob)
    errs = run(path, overrides={"max_texture": 128})["errors"]
    assert any("texture 256px exceeds" in e for e in errs)
    assert run(path, overrides={"max_texture": 256})["errors"] == []


def test_profiles_have_budgets(tmp_path):
    doc, blob = make_gltf(tris=25_000)
    path = write_glb(tmp_path / "a.glb", doc, blob)
    assert run(path, profile="prop")["errors"]
    assert run(path, profile="environment")["errors"] == []


def test_required_meshopt_or_draco_is_an_error(tmp_path):
    for ext in ("EXT_meshopt_compression", "KHR_draco_mesh_compression"):
        doc, blob = make_gltf(ext_used=[ext], ext_required=[ext])
        rep = run(write_glb(tmp_path / "a.glb", doc, blob))
        assert any(ext in e for e in rep["errors"])
        assert rep["compression"] == [ext]


def test_optional_meshopt_is_a_warning(tmp_path):
    doc, blob = make_gltf(ext_used=["EXT_meshopt_compression"])
    rep = run(write_glb(tmp_path / "a.glb", doc, blob))
    assert rep["errors"] == []
    assert rep["compression"] == ["EXT_meshopt_compression"]
    assert any("optional" in w for w in rep["warnings"])


def test_large_uncompressed_geometry_advises_meshopt(tmp_path):
    doc, blob = make_gltf(tris=15_000)  # 45k verts * 12 B = 540 KB
    rep = run(write_glb(tmp_path / "a.glb", doc, blob), overrides={"max_triangles": 10**6})
    assert any("gltfpack" in w for w in rep["warnings"])


def test_missing_external_image_is_an_error(tmp_path):
    doc, blob = make_gltf()
    doc["buffers"][0]["uri"] = "data:application/octet-stream;base64," + base64.b64encode(blob).decode()
    doc["images"] = [{"uri": "nope.png"}]
    path = tmp_path / "a.gltf"
    path.write_text(json.dumps(doc))
    assert any("missing file nope.png" in e for e in run(path)["errors"])
    (tmp_path / "nope.png").write_bytes(png(8, 8))
    rep = run(path)
    assert rep["errors"] == [] and rep["max_texture_side"] == 8


def test_bad_files_are_errors_not_crashes(tmp_path):
    bad = tmp_path / "bad.glb"
    bad.write_bytes(b"nope")
    assert run(bad)["errors"]
    other = tmp_path / "x.obj"
    other.write_text("v 0 0 0")
    assert any("unsupported format" in e for e in run(other)["errors"])


def _fbx_node(name: bytes, props: bytes, nprops: int, children: bytes, offset: int) -> bytes:
    total = 13 + len(name) + len(props) + len(children) + (13 if children else 0)
    head = struct.pack("<IIIB", offset + total, nprops, len(props), len(name))
    body = head + name + props + children + (b"\0" * 13 if children else b"")
    assert len(body) == total
    return body


def make_fbx(path: Path, polys: list[int]) -> Path:
    data = struct.pack("<" + "i" * len(polys), *polys)
    arr = b"i" + struct.pack("<III", len(polys), 0, len(data)) + data
    start = 27
    inner = _fbx_node(b"PolygonVertexIndex", arr, 1, b"", start + 13 + len(b"Geometry"))
    geo = _fbx_node(b"Geometry", b"", 0, inner, start)
    path.write_bytes(b"Kaydara FBX Binary  \0\x1a\0" + struct.pack("<I", 7400) + geo + b"\0" * 13)
    return path


def test_fbx_triangle_count(tmp_path):
    # one quad (2 tris) + one triangle (1 tri); a negative index ends a polygon
    rep = run(make_fbx(tmp_path / "a.fbx", [0, 1, 2, -4, 0, 1, -3]))
    assert rep["triangles_unique"] == 3
    assert rep["errors"] == []
    assert any("convert to GLB" in w for w in rep["warnings"])


def test_ascii_fbx_is_rejected(tmp_path):
    path = tmp_path / "a.fbx"
    path.write_text("; FBX 7.4.0 project file")
    assert any("ASCII FBX" in e for e in run(path)["errors"])


def test_ledger_gate_for_files_inside_the_repo():
    rep = ia.run(ROOT / "assets" / "models" / "biplane.glb")
    assert rep["errors"] == []
    # a path in the repo that no ledger entry covers is refused
    ghost = ROOT / "assets" / "ghost_pack" / "x.glb"
    ghost.parent.mkdir(exist_ok=True)
    try:
        write_glb(ghost, *make_gltf())
        rep = ia.run(ghost)
        assert any("no assets/LEDGER.json entry" in e for e in rep["errors"])
    finally:
        ghost.unlink()
        ghost.parent.rmdir()
