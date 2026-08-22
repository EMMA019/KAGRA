"""Drop-in venue helpers（GPU 不要）。"""
from __future__ import annotations

from pathlib import Path

import pytest

from tests.conftest import load_kagra_submodule

_contracts = load_kagra_submodule("contracts")
_stage = load_kagra_submodule("stage")

AssetKind = _contracts.AssetKind
KagraContractError = _contracts.KagraContractError
resolve_asset = _contracts.resolve_asset
candidate_paths = _contracts.candidate_paths
describe_environment = _contracts.describe_environment

backdrop_sphere = _stage.backdrop_sphere
classify_stage_file = _stage.classify_stage_file
resolve_stage_path = _stage.resolve_stage_path

ROOT = Path(__file__).resolve().parents[1]


def test_classify_stage_file():
    assert classify_stage_file("assets/venue.glb") == "gltf"
    assert classify_stage_file("hall.gltf") == "gltf"
    assert classify_stage_file("sky.PNG") == "backdrop"
    assert classify_stage_file(Path("env/hdri.jpg")) == "backdrop"
    with pytest.raises(KagraContractError) as ei:
        classify_stage_file("clip.fbx")
    assert ei.value.code == "UNSUPPORTED_STAGE"


def test_backdrop_sphere_inward_and_closed():
    verts, idx = backdrop_sphere(radius=10.0, rings=8, segs=12)
    assert len(verts) == (8 + 1) * (12 + 1)
    assert len(idx) == 8 * 12 * 6
    assert max(idx) < len(verts)
    # 原点から見て法線は内向き（頂点位置と逆符号）
    x, y, z, nx, ny, nz, u, v = verts[0]
    assert x * nx + y * ny + z * nz < 0
    assert all(len(row) == 8 for row in verts)
    assert all(0.0 <= row[6] <= 1.0 and 0.0 <= row[7] <= 1.0 for row in verts)
    # 半径
    r2 = verts[len(verts) // 2]
    dist = (r2[0] ** 2 + r2[1] ** 2 + r2[2] ** 2) ** 0.5
    assert abs(dist - 10.0) < 1e-4


def test_backdrop_sphere_rejects_nonpositive_radius():
    with pytest.raises(ValueError):
        backdrop_sphere(radius=0)


def test_resolve_stage_none_optional():
    assert resolve_stage_path("none", root=ROOT, required=False) is None
    assert resolve_stage_path("", root=ROOT, required=False) is None


def test_resolve_stage_missing_raises():
    with pytest.raises(KagraContractError) as ei:
        resolve_stage_path("definitely_missing_hall_xyz", root=ROOT)
    assert ei.value.code == "ASSET_NOT_FOUND"


def test_resolve_stage_gltf_file(tmp_path: Path):
    hall = tmp_path / "venue.glb"
    hall.write_bytes(b"glTF")
    got = resolve_stage_path(str(hall), root=ROOT)
    assert got == hall.resolve()
    assert classify_stage_file(got) == "gltf"


def test_resolve_stage_image_file(tmp_path: Path):
    sky = tmp_path / "sky.png"
    sky.write_bytes(b"\x89PNG")
    got = resolve_stage_path(str(sky), root=ROOT)
    assert got == sky.resolve()
    assert classify_stage_file(got) == "backdrop"


def test_gltf_kind_candidates_include_alias():
    cands = candidate_paths(AssetKind.GLTF, "stage", root=ROOT)
    assert any(str(c).endswith("stage.glb") for c in cands)


def test_describe_environment_lists_gltf():
    env = describe_environment(ROOT)
    assert "gltf_files" in env
    assert isinstance(env["gltf_files"], list)
