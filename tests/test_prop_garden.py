"""Prop Garden のルールと公開 API 規約。GPU 不要。"""
from __future__ import annotations

import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(_ROOT / "examples"))

from prop_garden_rules import GOLD_XZ, PROPS, START_XZ, facing_yaw, near_gold


def test_start_is_not_on_gold():
    assert not near_gold(*START_XZ)
    assert near_gold(*GOLD_XZ)


def test_garden_has_gold_sphere_and_mix():
    models = {row[0] for row in PROPS}
    colors = {row[5] for row in PROPS}
    assert "sphere" in models and "box" in models and "cylinder" in models
    assert "gold" in colors
    assert any(row[5] == "gold" and row[0] == "sphere" for row in PROPS)


def test_facing_keeps_fallback_when_still():
    assert facing_yaw(0.0, 0.0, 1.2) == 1.2
    assert facing_yaw(1.0, 0.0) != 0.0


def test_game_file_uses_only_public_imports():
    src = Path(__file__).resolve().parents[1] / "examples" / "vrm_prop_garden.py"
    text = src.read_text(encoding="utf-8")
    for name in (
        "_look_at",
        "_perspective_wgpu",
        "_send_bone_rot",
        "_euler_to_quat",
        "from kagra.vrm_avatar import _ID",
        "from kagra.entity import",
    ):
        assert name not in text, name
    for name in (
        "ensure_vrm",
        "Prop",
        "Walk",
        "hovered_prop",
        "destroy",
        "update_all",
        "texture_from_fn",
        "set_parent",
        "sky",
        "World3D",
        "Camera3D",
        "follow",
        "set_position",
        "set_yaw",
        "save_json",
        "ActionController",
    ):
        assert name in text, name
