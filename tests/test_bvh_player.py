"""BVH パーサ / デルタ変換（GPU 不要）。"""
from __future__ import annotations

from pathlib import Path

from tests.conftest import load_kagra_submodule

ROOT = Path(__file__).resolve().parents[1]
BVH = ROOT / "tests" / "fixtures" / "synthetic_dance.bvh"


def test_synthetic_bvh_delta_clip():
    bvh = load_kagra_submodule("bvh_player")
    motion = bvh.load_bvh(str(BVH), up_axis="y")
    assert abs(motion.fps - 30.0) < 0.01
    assert len(motion.frames) == 48
    clip = motion.to_clip()
    assert len(clip) == 48

    # フレーム0のデルタは恒等
    bones0, _, root0 = clip[0]
    assert root0 == (0.0, 0.0, 0.0)
    for q in bones0.values():
        assert abs(q[0]) < 1e-5 and abs(q[1]) < 1e-5 and abs(q[2]) < 1e-5
        assert abs(q[3] - 1.0) < 1e-5

    # 中盤は腕が動いている
    bones_m = clip[12][0]
    assert "J_Bip_L_UpperArm" in bones_m
    assert abs(bones_m["J_Bip_L_UpperArm"][3] - 1.0) > 0.01


def test_detect_up_axis_y():
    bvh = load_kagra_submodule("bvh_player")
    motion = bvh.load_bvh(str(BVH), up_axis="auto")
    assert motion.up_axis == "y"
