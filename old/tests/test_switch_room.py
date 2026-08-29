"""Switch Room のルールと公開 API 規約。GPU 不要。"""
from __future__ import annotations

import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(_ROOT / "examples"))

from switch_room_rules import (
    BOXES,
    HOLD_SEC,
    START_XZ,
    SWITCH_XZ,
    facing_yaw,
    on_switch,
    walls,
    wish_velocity,
)


def test_start_is_not_on_switch():
    assert not on_switch(*START_XZ)
    assert on_switch(*SWITCH_XZ)


def test_wish_normalizes_diagonal():
    vx, vz = wish_velocity(1.0, 1.0, speed=2.0)
    assert abs((vx * vx + vz * vz) ** 0.5 - 2.0) < 1e-6
    assert wish_velocity(0.0, 0.0) == (0.0, 0.0)


def test_facing_keeps_fallback_when_still():
    assert facing_yaw(0.0, 0.0, 1.2) == 1.2
    assert facing_yaw(1.0, 0.0) != 0.0


def test_room_has_boxes_and_walls():
    assert len(BOXES) >= 4
    assert len(walls()) == 4
    assert HOLD_SEC > 0


def test_game_file_uses_only_public_imports():
    src = Path(__file__).resolve().parents[1] / "examples" / "vrm_switch_room.py"
    text = src.read_text(encoding="utf-8")
    for name in (
        "_look_at",
        "_perspective_wgpu",
        "_send_bone_rot",
        "_euler_to_quat",
        "from kagra.vrm_avatar import _ID",
    ):
        assert name not in text, name
    for name in (
        "ensure_vrm",
        "World3D",
        "upload_mesh_3d",
        "draw_mesh_id",
        "Camera3D",
        "follow",
        "set_position",
        "set_yaw",
        "save_json",
        "tone",
        "ActionController",
    ):
        assert name in text, name
