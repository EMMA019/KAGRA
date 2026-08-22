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


def test_bone_map_covers_fingers():
    # VRMA は指まで対応済み。FBX (Mixamo) も 5 指 × 3 関節 × 両手を持つ。
    for side_fbx, side_vrm in (("Left", "L"), ("Right", "R")):
        for fbx_name, vrm_name in (
            ("Thumb", "Thumb"),
            ("Index", "Index"),
            ("Middle", "Middle"),
            ("Ring", "Ring"),
            ("Pinky", "Little"),
        ):
            for seg in (1, 2, 3):
                src = f"{side_fbx}Hand{fbx_name}{seg}"
                dst = f"J_Bip_{side_vrm}_{vrm_name}{seg}"
                assert fbx._BONE_MAP[src] == dst
                assert fbx._BONE_MAP[f"mixamorig:{src}"] == dst


def test_bone_map_no_finger_regression_on_body():
    assert fbx._BONE_MAP["Hips"] == "J_Bip_C_Hips"
    assert fbx._BONE_MAP["mixamorig:LeftHand"] == "J_Bip_L_Hand"
