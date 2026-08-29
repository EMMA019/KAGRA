"""Overworld / Pretty Room Walk facing and camera contracts. GPU 不要。"""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_overworld_faces_walk_face_not_camera_yaw():
    text = (ROOT / "old" / "examples" / "vrm_overworld.py").read_text(encoding="utf-8")
    assert "self.avatar.set_yaw(self.walk.face)" in text
    assert "self.avatar.set_yaw(self.walk.yaw)" not in text


def test_pretty_room_starts_third_person_follow():
    text = (ROOT / "old" / "examples" / "vrm_pretty_room.py").read_text(encoding="utf-8")
    assert "kagra.Walk(" in text
    assert "first_person=True," not in text
    assert "first_person=True)" not in text
    assert "self.cam.follow(" in text
    assert "self.cam.look(0.0, 1.55, 2.4" not in text
    assert "self.avatar.set_yaw(self.walk.face)" in text
    assert "distance=dist" in text or "distance=3.2" in text


def test_switch_room_camera_yaw_fixed_not_body_facing():
    text = (ROOT / "old" / "examples" / "vrm_switch_room.py").read_text(encoding="utf-8")
    assert "yaw=self.facing" not in text
    assert "bounds_half" in text
    assert "CAM_YAW" in text
    assert "self.avatar.set_yaw(self.facing)" in text


def test_dodge_room_follow_uses_room_bounds():
    text = (ROOT / "old" / "examples" / "vrm_dodge_room.py").read_text(encoding="utf-8")
    assert "bounds_half" in text
    assert "yaw=math.pi" in text
    assert "yaw=self.facing" not in text
