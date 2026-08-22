"""VRMA パーサ / デルタ変換（GPU 不要）。"""
from __future__ import annotations

from pathlib import Path

from tests.conftest import load_kagra_submodule

_contracts = load_kagra_submodule("contracts")
vrma = load_kagra_submodule("vrma_player")

AssetKind = _contracts.AssetKind
resolve_asset = _contracts.resolve_asset
ROOT = Path(__file__).resolve().parents[1]


def test_synthetic_vrma_delta_clip(tmp_path: Path):
    path = vrma.write_synthetic_vrma(tmp_path / "wave.vrma", frames=16, duration=1.0)
    assert vrma.is_vrma(path)

    motion = vrma.load_vrma(path, sample_fps=16.0)
    assert motion.spec_version == "1.0"
    assert "J_Bip_C_Hips" in motion.bones
    assert "J_Bip_L_UpperArm" in motion.bones
    assert "aa" in motion.expressions
    assert motion.duration == 1.0

    clip = motion.to_clip()
    assert len(clip) >= 2

    bones0, _, root0, expr0 = clip[0]
    assert root0[0] == 0.0
    assert abs(root0[1]) < 1e-5
    for q in bones0.values():
        assert abs(q[0]) < 1e-5 and abs(q[1]) < 1e-5 and abs(q[2]) < 1e-5
        assert abs(q[3] - 1.0) < 1e-5
    assert expr0.get("aa", 0.0) < 0.05

    mid = clip[len(clip) // 4]
    assert abs(mid[0]["J_Bip_L_UpperArm"][3] - 1.0) > 0.01
    assert mid[3]["aa"] > 0.5


def test_is_vrma_rejects_random(tmp_path: Path):
    p = tmp_path / "nope.glb"
    p.write_bytes(b"not a gltf")
    assert vrma.is_vrma(p) is False
    assert vrma.is_vrma(tmp_path / "x.vrma") is True


def test_resolve_expression_name():
    avail = {"Fcl_MTH_A", "Blink", "Joy"}
    assert vrma.resolve_expression_name("aa", avail) == "Fcl_MTH_A"
    assert vrma.resolve_expression_name("blink", avail) == "Blink"
    assert vrma.resolve_expression_name("happy", avail) == "Joy"
    assert vrma.resolve_expression_name("missing", avail) is None


def test_load_motion_dispatches_vrma(tmp_path: Path):
    path = vrma.write_synthetic_vrma(tmp_path / "clip.vrma")
    motion = vrma.load_vrma(path)
    # load_motion は VrmAvatar 側。ここでは拡張子分岐と同じ結果になることだけ見る
    assert path.suffix == ".vrma"
    assert motion.to_clip()[0][3]["aa"] < 0.05


def test_any_alias_prefers_existing_bvh():
    p = resolve_asset(AssetKind.ANY, "dance", root=ROOT)
    assert p.suffix.lower() == ".bvh"
