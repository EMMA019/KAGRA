"""Overworld / Pretty Room Walk facing and camera contracts. GPU 不要。"""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_overworld_faces_walk_face_not_camera_yaw():
    text = (ROOT / "examples" / "vrm_overworld.py").read_text(encoding="utf-8")
    assert "self.avatar.set_yaw(self.walk.face)" in text
    assert "self.avatar.set_yaw(self.walk.yaw)" not in text


def test_pretty_room_starts_third_person_follow():
    text = (ROOT / "examples" / "vrm_pretty_room.py").read_text(encoding="utf-8")
    assert "first_person=True" not in text
    assert "self.cam.follow(" in text
    assert "self.cam.look(0.0, 1.55, 2.4" not in text
    assert "self.avatar.set_yaw(self.walk.face)" in text
    assert "distance=dist" in text or "distance=3.2" in text
