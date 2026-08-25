"""Mixamo → VRoid rest+roll retarget (GPU 不要)."""
from __future__ import annotations

import math
from pathlib import Path

from tests.conftest import load_kagra_submodule

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests" / "fixtures" / "synthetic_mixamo_hang.json"

rt = load_kagra_submodule("retarget")
fbx = load_kagra_submodule("fbx_player")
contracts = load_kagra_submodule("contracts")
av = load_kagra_submodule("vrm_avatar")


def _dot(a, b) -> float:
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def test_fixture_is_tiny_and_checked_in():
    assert FIXTURE.is_file()
    assert FIXTURE.stat().st_size < 4000
    clip, src = rt.load_synthetic_mixamo_clip(FIXTURE)
    assert len(clip) == 2
    assert "J_Bip_L_UpperArm" in src
    # Frame 0 is identity (T-pose rest).
    q0 = clip[0][0]["J_Bip_L_UpperArm"]
    assert abs(q0[0]) < 1e-8 and abs(q0[3] - 1.0) < 1e-8


def test_tpose_mixamo_on_tpose_vroid_does_not_fold_forward():
    clip, src = rt.load_synthetic_mixamo_clip(FIXTURE)
    dst = rt.vroid_tpose_worlds(rolled=True)
    hang = clip[1][0]["J_Bip_L_UpperArm"]
    w_src = src["J_Bip_L_UpperArm"]
    w_dst = dst["J_Bip_L_UpperArm"]
    rest_dir = rt.animated_bone_dir(w_dst, (0.0, 0.0, 0.0, 1.0))
    raw_dir = rt.animated_bone_dir(w_dst, hang)
    fixed = rt.retarget_delta(hang, w_src, w_dst)
    fixed_dir = rt.animated_bone_dir(w_dst, fixed)

    # Unfixed Mixamo local-X hang on rolled VRoid is ~90° into +Z (carry).
    assert rt.folded_forward(rest_dir, raw_dir)
    assert abs(raw_dir[2]) > 0.55

    # Rest+roll compensation hangs (down / not forward).
    assert not rt.folded_forward(rest_dir, fixed_dir)
    assert abs(fixed_dir[2]) < 0.35
    # Must not stay at T-pose rest either (that was the old "arms out" look).
    assert _dot(rest_dir, fixed_dir) < 0.5


def test_apose_vroid_identity_stays_at_rest_not_folded():
    src = rt.mixamo_tpose_worlds()
    dst = rt.vroid_apose_worlds()
    w_dst = dst["J_Bip_L_UpperArm"]
    ident = (0.0, 0.0, 0.0, 1.0)
    rest_dir = rt.animated_bone_dir(w_dst, ident)
    delta = rt.retarget_delta(ident, src["J_Bip_L_UpperArm"], w_dst)
    anim_dir = rt.animated_bone_dir(w_dst, delta)
    assert _dot(rest_dir, anim_dir) > 0.98
    assert not rt.folded_forward(rest_dir, anim_dir)


def test_apose_vroid_hang_does_not_fold_forward():
    clip, src = rt.load_synthetic_mixamo_clip(FIXTURE)
    dst = rt.vroid_apose_worlds()
    hang = clip[1][0]["J_Bip_L_UpperArm"]
    w_dst = dst["J_Bip_L_UpperArm"]
    rest_dir = rt.animated_bone_dir(w_dst, (0.0, 0.0, 0.0, 1.0))
    raw_dir = rt.animated_bone_dir(w_dst, hang)
    fixed = rt.retarget_delta(hang, src["J_Bip_L_UpperArm"], w_dst)
    fixed_dir = rt.animated_bone_dir(w_dst, fixed)
    assert rt.folded_forward(rest_dir, raw_dir)
    assert not rt.folded_forward(rest_dir, fixed_dir)
    assert abs(fixed_dir[2]) < 0.40


def test_retarget_clip_rewrites_upper_arm():
    clip, src = rt.load_synthetic_mixamo_clip(FIXTURE)
    dst = rt.vroid_tpose_worlds()
    out = rt.retarget_clip(clip, src, dst)
    q_raw = clip[1][0]["J_Bip_L_UpperArm"]
    q_fix = out[1][0]["J_Bip_L_UpperArm"]
    # Same clip object is not mutated; dest local differs from Mixamo local.
    assert clip[1][0]["J_Bip_L_UpperArm"] == q_raw
    assert abs(q_fix[0] - q_raw[0]) > 0.1 or abs(q_fix[2] - q_raw[2]) > 0.1


def test_resolve_mixamo_locomotion_ignores_walk_alias(tmp_path: Path):
    # A synthetic BVH in the folder must not be picked as walk.
    (tmp_path / "synthetic_walk.bvh").write_text("HIERARCHY\n", encoding="utf-8")
    (tmp_path / "walk.fbx").write_bytes(b"Kaydara FBX dummy")
    (tmp_path / "Idle.fbx").write_bytes(b"Kaydara FBX dummy")
    (tmp_path / "Running.fbx").write_bytes(b"Kaydara FBX dummy")
    found = contracts.resolve_mixamo_locomotion(directory=tmp_path, root=ROOT)
    assert found["walk"].name == "walk.fbx"
    assert found["idle"].name == "Idle.fbx"
    assert found["run"].name == "Running.fbx"
    assert all(p.suffix.lower() == ".fbx" for p in found.values())
    src = (ROOT / "kagra" / "contracts.py").read_text(encoding="utf-8")
    fn = src[src.index("def resolve_mixamo_locomotion") : src.index("def describe_environment")]
    assert "resolve_asset" not in fn
    assert "tests/fixtures" not in fn
    assert "AssetKind" not in fn


def test_bind_locomotion_never_uses_walk_alias():
    src = (ROOT / "kagra" / "vrm_avatar.py").read_text(encoding="utf-8")
    fn = src[src.index("    def bind_locomotion") : src.index("    def _overlay_owned_bones")]
    assert "resolve_mixamo_locomotion" in fn
    assert "AssetKind.ANY" not in fn
    assert 'resolve_asset' not in fn
    assert "apply_damp = False" in fn


def test_add_motion_retargets_non_vrma():
    src = (ROOT / "kagra" / "vrm_avatar.py").read_text(encoding="utf-8")
    fn = src[src.index("    def add_motion") : src.index("    def _retarget_vrma_clip")]
    assert "retarget_clip" in fn
    assert "src_worlds" in fn
    # Dest-only conjugate on Mixamo is the old 骨格お化け path — still VRMA-only.
    assert "isinstance(motion, VrmaMotion)" in fn


def test_map_bind_worlds_uses_mixamo_names():
    raw = [("mixamorig:LeftArm", 0.0, 0.0, 0.7071, 0.7071)]
    worlds = fbx._map_bind_worlds(raw)
    assert "J_Bip_L_UpperArm" in worlds
    q = worlds["J_Bip_L_UpperArm"]
    assert abs(q[2] - 0.7071) < 1e-4
    # Canonical fill for bones the FBX omitted.
    assert "J_Bip_R_UpperArm" in worlds


def test_mixer_still_blends_after_custom_walk_clip():
    """Retargeted Mixamo walk is just another clip in the existing mixer."""
    anim = av._Animator(0, bind_rots={})
    hang = rt.mixamo_hang_delta()
    anim._clips["walk"] = [
        ({"J_Bip_L_UpperArm": hang, "J_Bip_L_UpperLeg": (0.2, 0, 0)}, 0.1),
        ({"J_Bip_L_UpperArm": hang, "J_Bip_L_UpperLeg": (-0.2, 0, 0)}, 0.1),
    ]
    mixer = av._LocomotionMixer(anim)
    mixer.enabled = True
    mixer.smooth = 0.001
    mixer.speed = 2.4
    mixer.update(0.05)
    assert mixer.clip_name == "walk"
    assert "J_Bip_L_UpperArm" in anim.current_rots
    assert "J_Bip_L_UpperLeg" in anim.current_rots
