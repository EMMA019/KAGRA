"""Indie-game helpers — GPU 不要。"""
from __future__ import annotations

import wave

from tests.conftest import load_kagra_submodule

kit = load_kagra_submodule("gamekit")


def test_rgba_png_is_valid():
    data = kit.rgba_png(4, 2, lambda x, y: (x * 40, y * 80, 10, 255))
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    assert b"IHDR" in data and b"IEND" in data


def test_write_tone_is_wav():
    path = kit.write_tone("unit", (440,), duration=0.05, volume=0.2)
    assert path.suffix == ".wav"
    with wave.open(str(path), "r") as w:
        assert w.getnchannels() == 1
        assert w.getsampwidth() == 2
        assert w.getframerate() == 22050
        assert w.getnframes() > 100


def test_billboard_mesh_four_corners():
    verts, idx = kit.billboard_mesh(0, 1, 0, 0.5, yaw=0.0)
    assert len(verts) == 4
    assert idx == [0, 1, 2, 0, 2, 3]
    assert all(len(v) == 8 for v in verts)


def test_disk_mesh_has_center_and_rim():
    verts, idx = kit.disk_mesh(0, 0, 0, 2.0, segs=8)
    assert verts[0][:3] == [0, 0, 0]
    assert len(verts) == 1 + 8 * 2
    assert len(idx) == 8 * 3


def test_quad_y_mesh_is_square():
    verts, idx = kit.quad_y_mesh(0, 0, 0, 1.0)
    xs = [v[0] for v in verts]
    zs = [v[2] for v in verts]
    assert min(xs) == -1.0 and max(xs) == 1.0
    assert min(zs) == -1.0 and max(zs) == 1.0
    assert idx[-1] == 3


def test_save_load_roundtrip(tmp_path):
    kit.save_json("hi", {"score": 42}, directory=tmp_path)
    assert kit.load_json("hi", directory=tmp_path) == {"score": 42}
    assert kit.load_json("missing", default={"score": 0}, directory=tmp_path) == {"score": 0}


def test_orb_rush_has_no_private_imports():
    from pathlib import Path

    src = Path(__file__).resolve().parents[1] / "examples" / "vrm_orb_rush.py"
    text = src.read_text(encoding="utf-8")
    for name in (
        "_look_at",
        "_perspective_wgpu",
        "_euler_to_quat",
        "_qmul",
        "_send_bone_rot",
        "from kagra.vrm_avatar import _ID",
    ):
        assert name not in text, name
    assert "texture_from_fn" in text
    assert "set_position" in text
    assert "set_yaw" in text
    assert "world_to_screen" in text
