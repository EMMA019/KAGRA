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


def test_flatten_reads_pbr_factors(tmp_path: Path):
    path = _write_tri_glb(tmp_path / "metal.glb")
    # rewrite with a material (reuse writer by patching JSON is heavy; append via flatten input)
    gltf = {
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1, "material": 0}]}],
        "materials": [{
            "pbrMetallicRoughness": {
                "baseColorFactor": [0.2, 0.4, 0.8, 1.0],
                "metallicFactor": 0.7,
                "roughnessFactor": 0.25,
            },
        }],
        "accessors": [
            {
                "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                "min": [-0.5, 0.0, 0.0], "max": [0.5, 1.0, 0.0],
            },
            {"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"},
        ],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": 36},
            {"buffer": 0, "byteOffset": 36, "byteLength": 6},
        ],
        "buffers": [{"byteLength": 44}],
    }
    pos = [-0.5, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0, 0.0]
    idx = [0, 1, 2]
    pos_b = struct.pack("<9f", *pos)
    idx_b = struct.pack("<3H", *idx) + b"\x00" * _pad4(6)
    blob = pos_b + idx_b
    json_b = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_b += b" " * _pad4(len(json_b))
    total = 12 + 8 + len(json_b) + 8 + len(blob)
    path.write_bytes(
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(json_b), 0x4E4F534A) + json_b
        + struct.pack("<II", len(blob), 0x004E4942) + blob
    )
    flat = gm.flatten_gltf(path)
    assert flat.metallic == pytest.approx(0.7)
    assert flat.roughness == pytest.approx(0.25)
    assert flat.base_color[0] == pytest.approx(0.2)
    assert flat.base_color[2] == pytest.approx(0.8)


def _png_1x1(r: int, g: int, b: int, a: int = 255) -> bytes:
    import zlib

    def chunk(tag: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    raw = b"\x00" + bytes((r, g, b, a))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )


def test_flatten_reads_normal_texture(tmp_path: Path):
    al_raw = _png_1x1(200, 40, 40)
    n_raw = _png_1x1(128, 128, 255)
    al_pad = al_raw + b"\x00" * _pad4(len(al_raw))
    n_pad = n_raw + b"\x00" * _pad4(len(n_raw))
    pos = [-0.5, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0, 0.0]
    pos_b = struct.pack("<9f", *pos)
    idx_b = struct.pack("<3H", 0, 1, 2) + b"\x00" * _pad4(6)
    blob = pos_b + idx_b + al_pad + n_pad
    off_al = len(pos_b) + len(idx_b)
    off_n = off_al + len(al_pad)
    gltf = {
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1, "material": 0}]}],
        "materials": [{
            "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}},
            "normalTexture": {"index": 1},
        }],
        "textures": [{"source": 0}, {"source": 1}],
        "images": [
            {"bufferView": 2, "mimeType": "image/png"},
            {"bufferView": 3, "mimeType": "image/png"},
        ],
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
            {"buffer": 0, "byteOffset": off_al, "byteLength": len(al_raw)},
            {"buffer": 0, "byteOffset": off_n, "byteLength": len(n_raw)},
        ],
        "buffers": [{"byteLength": len(blob)}],
    }
    json_b = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_b += b" " * _pad4(len(json_b))
    total = 12 + 8 + len(json_b) + 8 + len(blob)
    path = tmp_path / "bump.glb"
    path.write_bytes(
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(json_b), 0x4E4F534A) + json_b
        + struct.pack("<II", len(blob), 0x004E4942) + blob
    )
    flat = gm.flatten_gltf(path)
    assert flat.image == al_raw
    assert flat.normal_image == n_raw


def test_flatten_reads_sibling_png_uri(tmp_path: Path):
    png = _png_1x1(10, 200, 30)
    (tmp_path / "Textures").mkdir()
    (tmp_path / "Textures" / "colormap.png").write_bytes(png)
    path = _write_tri_glb(tmp_path / "tree.glb")
    # Patch JSON chunk: add images[].uri (Kenney Mini Forest style).
    raw = path.read_bytes()
    json_len = struct.unpack_from("<I", raw, 12)[0]
    gltf = json.loads(raw[20:20 + json_len])
    gltf["images"] = [{"uri": "Textures/colormap.png"}]
    json_b = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_b += b" " * _pad4(len(json_b))
    blob = raw[20 + json_len + 8:]
    total = 12 + 8 + len(json_b) + 8 + len(blob)
    path.write_bytes(
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(json_b), 0x4E4F534A) + json_b
        + struct.pack("<II", len(blob), 0x004E4942) + blob
    )
    flat = gm.flatten_gltf(path)
    assert flat.image == png


def test_read_relative_image_windows_backslash(tmp_path: Path):
    png = _png_1x1(10, 200, 30)
    (tmp_path / "Textures").mkdir()
    (tmp_path / "Textures" / "colormap.png").write_bytes(png)
    glb = tmp_path / "tree.glb"
    glb.write_bytes(b"glTF")
    got = gm._read_relative_image("Textures\\colormap.png", glb)
    assert got == png


def test_flatten_rejects_parent_png_uri(tmp_path: Path):
    secret = tmp_path / "secret.png"
    secret.write_bytes(_png_1x1(1, 2, 3))
    nested = tmp_path / "models"
    nested.mkdir()
    path = _write_tri_glb(nested / "tree.glb")
    raw = path.read_bytes()
    json_len = struct.unpack_from("<I", raw, 12)[0]
    gltf = json.loads(raw[20:20 + json_len])
    gltf["images"] = [{"uri": "../secret.png"}]
    json_b = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_b += b" " * _pad4(len(json_b))
    blob = raw[20 + json_len + 8:]
    total = 12 + 8 + len(json_b) + 8 + len(blob)
    path.write_bytes(
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(json_b), 0x4E4F534A) + json_b
        + struct.pack("<II", len(blob), 0x004E4942) + blob
    )
    flat = gm.flatten_gltf(path)
    assert flat.image is None
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


def _write_uv_tri_glb(path: Path, *, khr: dict | None = None, image_uri: str | None = None) -> Path:
    pos = [-0.5, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0, 0.0]
    uv = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0]
    idx = [0, 1, 2]
    pos_b = struct.pack("<9f", *pos)
    uv_b = struct.pack("<6f", *uv)
    idx_b = struct.pack("<3H", *idx)
    idx_b += b"\x00" * _pad4(len(idx_b))
    blob = pos_b + uv_b + idx_b
    tex_info: dict = {"index": 0}
    if khr is not None:
        tex_info["extensions"] = {"KHR_texture_transform": khr}
    gltf = {
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{
            "attributes": {"POSITION": 0, "TEXCOORD_0": 1},
            "indices": 2,
            "material": 0,
        }]}],
        "materials": [{"pbrMetallicRoughness": {"baseColorTexture": tex_info, "metallicFactor": 0.0}}],
        "textures": [{"source": 0}],
        "images": [{"uri": image_uri or "Textures/colormap.png"}],
        "accessors": [
            {
                "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                "min": [-0.5, 0.0, 0.0], "max": [0.5, 1.0, 0.0],
            },
            {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC2"},
            {"bufferView": 2, "componentType": 5123, "count": 3, "type": "SCALAR"},
        ],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": len(pos_b)},
            {"buffer": 0, "byteOffset": len(pos_b), "byteLength": len(uv_b)},
            {"buffer": 0, "byteOffset": len(pos_b) + len(uv_b), "byteLength": 6},
        ],
        "buffers": [{"byteLength": len(blob)}],
        "extensionsUsed": ["KHR_texture_transform"],
    }
    json_b = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_b += b" " * _pad4(len(json_b))
    total = 12 + 8 + len(json_b) + 8 + len(blob)
    path.write_bytes(
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(json_b), 0x4E4F534A) + json_b
        + struct.pack("<II", len(blob), 0x004E4942) + blob
    )
    return path


def test_flatten_applies_khr_texture_transform_atlas(tmp_path: Path):
    png = _png_1x1(10, 200, 30)
    (tmp_path / "Textures").mkdir()
    (tmp_path / "Textures" / "colormap.png").write_bytes(png)
    path = _write_uv_tri_glb(
        tmp_path / "tree.glb",
        khr={"offset": [0.25, 0.5], "scale": [0.25, 0.25], "texCoord": 0},
    )
    flat = gm.flatten_gltf(path, center=False)
    assert flat.image == png
    uvs = [(round(v[6], 5), round(v[7], 5)) for v in flat.verts]
    # (0,0) → (0.25, 0.5); (1,0) → (0.50, 0.5); (0,1) → (0.25, 0.75)
    assert (0.25, 0.5) in uvs
    assert (0.5, 0.5) in uvs
    assert (0.25, 0.75) in uvs


def test_flatten_identity_khr_texcoord_keeps_uvs(tmp_path: Path):
    png = _png_1x1(4, 5, 6)
    (tmp_path / "Textures").mkdir()
    (tmp_path / "Textures" / "colormap.png").write_bytes(png)
    path = _write_uv_tri_glb(tmp_path / "tree.glb", khr={"texCoord": 0})
    flat = gm.flatten_gltf(path, center=False)
    uvs = {(round(v[6], 5), round(v[7], 5)) for v in flat.verts}
    assert (0.0, 0.0) in uvs
    assert (1.0, 0.0) in uvs
    assert (0.0, 1.0) in uvs
