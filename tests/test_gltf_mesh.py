"""Static glTF flatten — GPU 不要。"""
from __future__ import annotations

import json
import struct
from pathlib import Path

import pytest

from tests.conftest import load_kagra_submodule

gm = load_kagra_submodule("gltf_mesh")
contracts = load_kagra_submodule("contracts")


def _pad4(n: int) -> int:
    return (4 - (n % 4)) % 4


def _write_tri_glb(path: Path, *, translation=None) -> Path:
    """One triangle. Optional node translation."""
    pos = [-0.5, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0, 0.0]
    idx = [0, 1, 2]
    pos_b = struct.pack("<9f", *pos)
    idx_b = struct.pack("<3H", *idx)
    idx_b += b"\x00" * _pad4(len(idx_b))
    blob = pos_b + idx_b
    node = {"mesh": 0}
    if translation is not None:
        node["translation"] = list(translation)
    gltf = {
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [node],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1}]}],
        "accessors": [
            {
                "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                "min": [-0.5, 0.0, 0.0], "max": [0.5, 1.0, 0.0],
            },
            {"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"},
        ],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": len(pos_b)},
            {"buffer": 0, "byteOffset": len(pos_b), "byteLength": 6},
        ],
        "buffers": [{"byteLength": len(blob)}],
    }
    json_b = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_b += b" " * _pad4(len(json_b))
    total = 12 + 8 + len(json_b) + 8 + len(blob)
    data = (
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(json_b), 0x4E4F534A) + json_b
        + struct.pack("<II", len(blob), 0x004E4942) + blob
    )
    path.write_bytes(data)
    return path


def test_is_gltf_name():
    assert gm.is_gltf_name("crate.glb")
    assert gm.is_gltf_name("/tmp/a.GLTF")
    assert not gm.is_gltf_name("box")
    assert not gm.is_gltf_name("cube")


def test_unit_cube_flatten_is_centered_unit():
    root = Path(__file__).resolve().parents[1]
    path = root / "kagra" / "data" / "unit_cube.glb"
    flat = gm.flatten_gltf(path)
    assert len(flat.verts) == 24
    assert len(flat.indices) == 36
    assert flat.aabb[0] == pytest.approx(-0.5)
    assert flat.aabb[3] == pytest.approx(0.5)
    assert flat.aabb[1] == pytest.approx(-0.5)
    assert flat.aabb[4] == pytest.approx(0.5)


def test_flatten_applies_node_translation(tmp_path: Path):
    path = _write_tri_glb(tmp_path / "tri.glb", translation=(10.0, 0.0, 0.0))
    raw = gm.flatten_gltf(path, center=False)
    xs = [v[0] for v in raw.verts]
    assert min(xs) == pytest.approx(9.5)
    assert max(xs) == pytest.approx(10.5)
    centered = gm.flatten_gltf(path, center=True)
    cxs = [v[0] for v in centered.verts]
    assert min(cxs) == pytest.approx(-0.5)
    assert max(cxs) == pytest.approx(0.5)


def test_resolve_cube_alias():
    path = gm.resolve_gltf_path("cube.glb")
    assert path.name == "unit_cube.glb"
    assert path.is_file()
    assert contracts.resolve_asset(contracts.AssetKind.GLTF, "cube").name == "unit_cube.glb"


def test_flatten_rejects_empty(tmp_path: Path):
    bad = tmp_path / "empty.glb"
    json_b = b'{"asset":{"version":"2.0"}}'
    json_b += b" " * _pad4(len(json_b))
    data = (
        struct.pack("<4sII", b"glTF", 2, 12 + 8 + len(json_b) + 8)
        + struct.pack("<II", len(json_b), 0x4E4F534A) + json_b
        + struct.pack("<II", 0, 0x004E4942)
    )
    # total length field may be off; flatten should still fail on no meshes
    bad.write_bytes(data)
    with pytest.raises(ValueError, match="no meshes"):
        gm.flatten_gltf(bad)
