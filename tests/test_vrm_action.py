"""ActionController の公開面。GPU 不要。"""
from tests.conftest import load_kagra_submodule

action = load_kagra_submodule("vrm_action")


def test_names_lists_built_ins():
    names = action.ActionController.names()
    for need in ("banzai", "nod", "clap", "bow", "wave", "jump_joy"):
        assert need in names
    assert names == list(dict.fromkeys(names))


def test_clap_and_banzai_return_to_empty_terminal():
    """Last keyframe {} = blend back to idle, not hold the clap arms."""
    for name in ("clap", "banzai", "nod", "bow", "wave"):
        frames = action._ACTIONS[name]
        assert frames[-1][1] == {}, name


def test_empty_keyframe_uses_saved_idle_not_live_overlay():
    idle = [0.0, 0.0, 0.0, 1.0]
    clap = [0.1, 0.2, 0.3, 0.9]
    saved = {"J_Bip_L_UpperArm": idle}
    bind = {"J_Bip_L_UpperArm": [0.0, 0.0, 0.5, 0.8]}
    # Mid-action keyframe keeps the overlay quat.
    got = action._overlay_bone_quat(
        {"J_Bip_L_UpperArm": clap}, "J_Bip_L_UpperArm", saved, bind,
    )
    assert got is clap
    # Empty terminal must restore saved idle, not bind and not "live" clap.
    got = action._overlay_bone_quat({}, "J_Bip_L_UpperArm", saved, bind)
    assert got is idle
    # Missing saved idle falls back to bind.
    got = action._overlay_bone_quat({}, "J_Bip_L_UpperArm", {}, bind)
    assert got == bind["J_Bip_L_UpperArm"]


def test_empty_keyframe_prefers_live_locomotion_over_saved_idle():
    """Walk keeps moving during clap; empty {} releases to live legs/arms."""
    idle = [0.0, 0.0, 0.0, 1.0]
    walk = [0.2, 0.0, 0.0, 0.98]
    saved = {"J_Bip_L_UpperArm": idle}
    bind = {"J_Bip_L_UpperArm": [0.0, 0.0, 0.5, 0.8]}
    live = {"J_Bip_L_UpperArm": walk}
    got = action._overlay_bone_quat({}, "J_Bip_L_UpperArm", saved, bind, live)
    assert got is walk
    # Overlay pose still wins when the keyframe names the bone.
    clap = [0.1, 0.2, 0.3, 0.9]
    got = action._overlay_bone_quat(
        {"J_Bip_L_UpperArm": clap}, "J_Bip_L_UpperArm", saved, bind, live,
    )
    assert got is clap
