"""Idle/walk/run speed blend + upper-body layer mask. GPU 不要."""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

av = load_kagra_submodule("vrm_avatar")


def _arm_q(pose: dict, side: str = "L"):
    return pose.get(f"J_Bip_{side}_UpperArm")


def _leg_q(pose: dict, side: str = "L"):
    return pose.get(f"J_Bip_{side}_UpperLeg")


def test_locomotion_weights_are_continuous():
    i0, w0, r0 = av.locomotion_weights(0.0, 2.4, 5.0)
    assert i0 == 1.0 and w0 == 0.0 and r0 == 0.0
    i1, w1, r1 = av.locomotion_weights(1.2, 2.4, 5.0)
    assert abs(i1 + w1 + r1 - 1.0) < 1e-9
    assert i1 > 0.0 and w1 > 0.0 and r1 == 0.0
    i2, w2, r2 = av.locomotion_weights(2.4, 2.4, 5.0)
    assert i2 == 0.0 and abs(w2 - 1.0) < 1e-9 and r2 == 0.0
    i3, w3, r3 = av.locomotion_weights(3.7, 2.4, 5.0)
    assert i3 == 0.0 and w3 > 0.0 and r3 > 0.0
    i4, w4, r4 = av.locomotion_weights(9.0, 2.4, 5.0)
    assert i4 == 0.0 and w4 == 0.0 and r4 == 1.0
    # Tiny speed change must not snap (no 0/1 jump across the threshold).
    a = av.locomotion_weights(2.39, 2.4, 5.0)
    b = av.locomotion_weights(2.41, 2.4, 5.0)
    assert abs(a[1] - b[1]) < 0.05
    assert abs(a[2] - b[2]) < 0.05


def test_locomotion_weights_without_run_stay_on_walk():
    i, w, r = av.locomotion_weights(8.0, 2.4, 5.0, has_run=False)
    assert r == 0.0
    assert abs(w - 1.0) < 1e-9
    assert i == 0.0


def test_built_in_run_clip_exists_idle_has_no_legs():
    assert "run" in av.PRESETS and "walk" in av.PRESETS and "idle" in av.PRESETS
    idle_bones = av.PRESETS["idle"][0][0]
    walk_bones = av.PRESETS["walk"][0][0]
    assert "J_Bip_L_UpperLeg" not in idle_bones
    assert "J_Bip_L_UpperLeg" in walk_bones
    assert len(av.PRESETS["walk"]) == len(av.PRESETS["run"]) == 16


def test_blend_idle_to_walk_moves_legs_without_snap():
    idle = av.sample_clip_bind_pose(av.PRESETS["idle"], 0.0, {})
    walk = av.sample_clip_bind_pose(av.PRESETS["walk"], 0.25, {})
    low = av.blend_locomotion_pose(
        idle=idle, walk=walk, run={}, weights=(0.8, 0.2, 0.0), bind_rots={},
    )
    high = av.blend_locomotion_pose(
        idle=idle, walk=walk, run={}, weights=(0.2, 0.8, 0.0), bind_rots={},
    )
    # Legs exist once walk is in the mix, and the pose eases toward walk.
    assert _leg_q(low) is not None and _leg_q(high) is not None
    d_low = sum((a - b) ** 2 for a, b in zip(_leg_q(low), _leg_q(walk)))
    d_high = sum((a - b) ** 2 for a, b in zip(_leg_q(high), _leg_q(walk)))
    assert d_high < d_low
    # Arms exist on both clips; blend is between, not a copy of either.
    d_idle = sum((a - b) ** 2 for a, b in zip(_arm_q(low), _arm_q(idle)))
    d_walk = sum((a - b) ** 2 for a, b in zip(_arm_q(low), _arm_q(walk)))
    assert d_idle > 0.0 and d_walk > 0.0


def test_full_idle_returns_walk_legs_to_bind():
    idle = av.sample_clip_bind_pose(av.PRESETS["idle"], 0.0, {})
    walk = av.sample_clip_bind_pose(av.PRESETS["walk"], 0.3, {})
    pose = av.blend_locomotion_pose(
        idle=idle, walk=walk, run={}, weights=(1.0, 0.0, 0.0), bind_rots={},
    )
    leg = _leg_q(pose)
    assert leg is not None
    assert abs(leg[0]) < 1e-6 and abs(leg[1]) < 1e-6 and abs(leg[2]) < 1e-6
    assert abs(leg[3] - 1.0) < 1e-6


def test_mixer_keeps_gait_in_current_rots_when_upper_owns_arms():
    anim = av._Animator(0, bind_rots={})
    mixer = av._LocomotionMixer(anim)
    mixer.enabled = True
    mixer.smooth = 0.001
    mixer.speed = 2.4
    mixer.walk_speed = 2.4
    mixer.run_speed = 5.6
    skip = {
        "J_Bip_L_UpperArm", "J_Bip_R_UpperArm",
        "J_Bip_L_LowerArm", "J_Bip_R_LowerArm",
    }
    mixer.update(0.05, skip_send=skip)
    first_leg = list(_leg_q(anim.current_rots))
    first_arm = list(_arm_q(anim.current_rots))
    assert first_leg is not None and first_arm is not None
    mixer.update(0.20, skip_send=skip)
    later_leg = list(_leg_q(anim.current_rots))
    later_arm = list(_arm_q(anim.current_rots))
    # Legs keep cycling under the overlay mask.
    assert first_leg != later_leg
    # Arms still evaluate in current_rots (gait) even though they were not sent.
    assert first_arm != later_arm
    assert mixer.clip_name == "walk"


def test_play_upper_idle_owns_arms_while_legs_walk():
    """Two threads: idle clip on the upper mask, walk legs underneath."""
    anim = av._Animator(0, bind_rots={})
    mixer = av._LocomotionMixer(anim)
    mixer.enabled = True
    mixer.smooth = 0.001
    mixer.speed = 2.4
    idle_pose = av.sample_clip_bind_pose(av.PRESETS["idle"], 0.0, {})
    skip = set(idle_pose)
    mixer.update(0.16, skip_send=skip)
    walk_arm = list(_arm_q(anim.current_rots))
    idle_arm = list(_arm_q(idle_pose))
    # The mixer still stores the walk arm (for when the overlay ends).
    d = sum((a - b) ** 2 for a, b in zip(walk_arm, idle_arm))
    assert d > 1e-6
    # Legs are a walk bone, not an idle bone.
    assert _leg_q(anim.current_rots) is not None
    assert _leg_q(idle_pose) is None


def test_is_upper_bone_mask():
    assert av._is_upper_bone("J_Bip_L_UpperArm")
    assert av._is_upper_bone("J_Bip_C_Spine")
    assert not av._is_upper_bone("J_Bip_L_UpperLeg")
    assert not av._is_upper_bone("J_Bip_C_Hips")


def test_mixer_eases_speed_instead_of_snapping_clip():
    anim = av._Animator(0, bind_rots={})
    mixer = av._LocomotionMixer(anim)
    mixer.enabled = True
    mixer.smooth = 0.18
    mixer.speed = 5.6
    mixer.update(1.0 / 60.0)
    # One frame at full run speed must not already be a hard run clip.
    assert mixer.weights[0] > 0.5
    assert mixer.clip_name == "idle"
    for _ in range(40):
        mixer.update(1.0 / 60.0)
    assert mixer.weights[2] > mixer.weights[0]
    assert mixer.clip_name in ("walk", "run")


def test_overlay_owned_bones_union_upper_and_action():
    class _Upper:
        playing = True
        _frames = [({"J_Bip_L_UpperArm": (0, 0, 0)}, 0.2)]

    class _Action:
        playing_action = "clap"
        _saved_idle_rots = {"J_Bip_C_Spine": [0, 0, 0, 1]}
        _keyframes = [(0.0, {}), (0.3, {"J_Bip_R_UpperArm": [0, 0, 0, 1]})]
        _overlay_rots = {}

    class _Fake:
        _upper = _Upper()
        _action_controller = _Action()
        _overlay_owned_bones = av.VrmAvatar._overlay_owned_bones

    owned = av.VrmAvatar._overlay_owned_bones(_Fake())
    assert "J_Bip_L_UpperArm" in owned
    assert "J_Bip_R_UpperArm" in owned
    assert "J_Bip_C_Spine" in owned
    assert "J_Bip_L_UpperLeg" not in owned
