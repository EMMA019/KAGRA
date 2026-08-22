"""FBX ルートスケール（GPU / ufbx 不要）。"""
from tests.conftest import load_kagra_submodule

fbx = load_kagra_submodule("fbx_player")


def test_root_scale_uses_vrm_meters_not_mixamo_cm():
    # Mixamo 立ち姿勢の Armature Y 幅 ≈ 97cm。VRM hips ≈ 0.85m
    s = fbx.root_scale(96.871, 0.853)
    assert 0.005 < s < 0.02
    assert abs(s - (0.853 / 96.871)) < 1e-9


def test_root_scale_rejects_mixamo_hips_as_vrm_height():
    # 以前は Hips ty=96.87 を vrm_hips_y に入れて scale≈1 になっていた
    s = fbx.root_scale(96.871, 96.871)
    assert abs(s - (0.853 / 96.871)) < 1e-9


def test_root_scale_tiny_leg_defaults():
    assert fbx.root_scale(0.0, 0.853) == 0.853
