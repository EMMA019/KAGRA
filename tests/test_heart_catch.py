"""Heart Catch のルールと「公開 API だけ」規約。GPU 不要。"""
from __future__ import annotations

import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(_ROOT / "examples"))

from heart_catch_rules import (
    CATCH_Z,
    catch_score,
    clamp_lane,
    is_catch,
    is_miss,
    lane_x,
    spawn_heart,
    step_heart,
)


def test_lanes_are_three_and_ordered():
    assert clamp_lane(-3) == 0
    assert clamp_lane(9) == 2
    assert lane_x(0) < lane_x(1) < lane_x(2)


def test_catch_same_lane_near_zero():
    h = spawn_heart(lane=1)
    h.z = 0.0
    assert is_catch(1, h)
    assert not is_catch(0, h)


def test_miss_past_player():
    h = spawn_heart(lane=0)
    h.z = 2.0
    assert is_miss(h)
    assert not is_catch(0, h)


def test_step_moves_forward():
    h = spawn_heart(lane=2)
    nxt = step_heart(h, 0.2)
    assert nxt.z > h.z
    assert nxt.z < CATCH_Z or nxt.z < 0.0


def test_combo_score_grows_then_caps():
    assert catch_score(0) == 10
    assert catch_score(3) == 16
    assert catch_score(99) == 50


def test_game_file_uses_only_public_imports():
    src = Path(__file__).resolve().parents[1] / "examples" / "vrm_heart_catch.py"
    text = src.read_text(encoding="utf-8")
    for name in (
        "_look_at",
        "_perspective_wgpu",
        "_send_bone_rot",
        "_euler_to_quat",
    ):
        assert name not in text, name
    for name in (
        "ensure_vrm",
        "ActionController",
        "texture_from_fn",
        "set_position",
        "set_yaw",
        "draw_billboard_instances",
        "save_json",
        "tone",
    ):
        assert name in text, name
