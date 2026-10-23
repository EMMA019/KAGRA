"""glTF asset checks. Skipped unless trimesh and pygltflib are installed.

The default python-unit job does not install them. The gltf-asset-checks
job does. Neither package is part of the wheel.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

pytest.importorskip("trimesh")
pytest.importorskip("pygltflib")

import numpy as np
import pygltflib as p

ROOT = Path(__file__).resolve().parents[1]
CUBE = ROOT / "tests" / "fixtures" / "unit_cube.glb"


def _checker():
    path = ROOT / "tools" / "check_gltf_asset.py"
    spec = importlib.util.spec_from_file_location("check_gltf_asset", path)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _triangle_glb(path: Path, materials: list, images: list, textures: list) -> None:
    positions = np.array([[0, 0, 0], [1, 0, 0], [0, 1, 0]], dtype=np.float32)
    indices = np.array([0, 1, 2], dtype=np.uint16)
    blob = positions.tobytes() + indices.tobytes()
    gltf = p.GLTF2()
    gltf.set_binary_blob(blob)
    gltf.buffers = [p.Buffer(byteLength=len(blob))]
    gltf.bufferViews = [
        p.BufferView(
            buffer=0,
            byteOffset=0,
            byteLength=len(positions.tobytes()),
            target=p.ARRAY_BUFFER,
        ),
        p.BufferView(
            buffer=0,
            byteOffset=len(positions.tobytes()),
            byteLength=len(indices.tobytes()),
            target=p.ELEMENT_ARRAY_BUFFER,
        ),
    ]
    gltf.accessors = [
        p.Accessor(
            bufferView=0,
            componentType=p.FLOAT,
            count=3,
            type=p.VEC3,
            min=[0, 0, 0],
            max=[1, 1, 0],
        ),
        p.Accessor(
            bufferView=1,
            componentType=p.UNSIGNED_SHORT,
            count=3,
            type=p.SCALAR,
        ),
    ]
    gltf.materials = materials
    gltf.textures = textures
    gltf.images = images
    gltf.meshes = [
        p.Mesh(
            primitives=[
                p.Primitive(
                    attributes=p.Attributes(POSITION=0),
                    indices=1,
                    material=0 if materials else None,
                )
            ]
        )
    ]
    gltf.nodes = [p.Node(mesh=0)]
    gltf.scenes = [p.Scene(nodes=[0])]
    gltf.scene = 0
    gltf.save(str(path))


def test_unit_cube_has_triangles_and_finite_bounds():
    report = _checker().check_gltf(CUBE)
    assert report["errors"] == []
    assert report["triangles"] == 12
    assert report["missing_images"] == []
    assert report["default_metal_rough"] == []
    low, high = report["bounds"]
    assert low == pytest.approx([-0.5, -0.5, -0.5])
    assert high == pytest.approx([0.5, 0.5, 0.5])


def test_missing_image_fails_and_default_metal_rough_is_a_note(tmp_path: Path):
    path = tmp_path / "bad.glb"
    _triangle_glb(
        path,
        materials=[
            p.Material(
                name="kenney",
                pbrMetallicRoughness=p.PbrMetallicRoughness(
                    metallicFactor=1.0,
                    roughnessFactor=1.0,
                    baseColorTexture=p.TextureInfo(index=0),
                ),
            ),
            p.Material(
                name="painted",
                pbrMetallicRoughness=p.PbrMetallicRoughness(
                    metallicFactor=0.2,
                    roughnessFactor=0.4,
                ),
            ),
        ],
        images=[p.Image(uri="missing.png")],
        textures=[p.Texture(source=0)],
    )
    report = _checker().check_gltf(path)
    assert report["triangles"] == 1
    assert any("missing.png" in err for err in report["errors"])
    assert "kenney" in report["default_metal_rough"]
    assert "painted" not in report["default_metal_rough"]


def test_checker_does_not_import_the_extension():
    # The asset packages stay out of the wheel. This module must not pull
    # the Rust extension in while it checks a file.
    assert "kagra.kagra_shared" not in sys.modules
    _checker().check_gltf(CUBE)
    assert "kagra.kagra_shared" not in sys.modules
