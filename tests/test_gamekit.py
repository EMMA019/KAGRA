"""Indie-game helpers — GPU 不要。"""
from __future__ import annotations

import wave

import pytest

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


def test_box_mesh_six_faces():
    verts, idx = kit.box_mesh(0, 0.5, 0, 2.0, 1.0, 2.0)
    assert len(verts) == 24
    assert len(idx) == 36
    ys = [v[1] for v in verts]
    assert min(ys) == 0.0 and max(ys) == 1.0
    assert all(len(v) == 8 for v in verts)


def test_sphere_mesh_unit_diameter():
    verts, idx = kit.sphere_mesh(0, 0, 0, 0.5, segs=8)
    assert len(verts) == 5 * 9
    assert len(idx) == 4 * 8 * 6
    xs = [v[0] for v in verts]
    assert min(xs) == pytest.approx(-0.5, abs=1e-6)
    assert max(xs) == pytest.approx(0.5, abs=1e-6)
    assert all(len(v) == 8 for v in verts)


def test_cylinder_mesh_unit_size():
    verts, idx = kit.cylinder_mesh(0, 0, 0, 0.5, 1.0, segs=8)
    assert len(idx) == 8 * 6 + 8 * 3 + 8 * 3
    ys = [v[1] for v in verts]
    assert min(ys) == pytest.approx(-0.5, abs=1e-6)
    assert max(ys) == pytest.approx(0.5, abs=1e-6)
    assert all(len(v) == 8 for v in verts)


def test_heightfield_mesh_flat_is_level():
    verts, idx = kit.heightfield_mesh(lambda _x, _z: 1.5, half=2.0, cells=4)
    assert len(verts) == 5 * 5
    assert len(idx) == 4 * 4 * 6
    assert all(abs(v[1] - 1.5) < 1e-9 for v in verts)
    xs = [v[0] for v in verts]
    assert min(xs) == pytest.approx(-2.0)
    assert max(xs) == pytest.approx(2.0)


def test_ramp_mesh_two_tris():
    verts, idx = kit.ramp_mesh(0.0, 4.0, -1.0, 1.0, 0.0, 1.2)
    assert len(verts) == 4
    assert idx == [0, 1, 2, 0, 2, 3]
    assert verts[1][1] == pytest.approx(1.2)
    assert verts[0][1] == pytest.approx(0.0)


def test_heightfield_tile_aabb_fits_shadow():
    land = load_kagra_submodule("land")
    verts, idx = kit.heightfield_tile(land.island_height, 0.0, 0.0, tile=10.0, cells=8)
    assert len(verts) == 9 * 9
    assert idx[-1] == 9 * 8 + 8
    xs = [v[0] for v in verts]
    ys = [v[1] for v in verts]
    zs = [v[2] for v in verts]
    extent = max(max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    assert extent <= 24.0
    assert min(xs) == pytest.approx(0.0)
    assert max(xs) == pytest.approx(10.0)


def _edge_verts(verts, *, x=None, z=None, tol=1e-6):
    out = []
    for v in verts:
        if x is not None and abs(v[0] - x) > tol:
            continue
        if z is not None and abs(v[2] - z) > tol:
            continue
        out.append(v)
    return out


def test_heightfield_adjacent_tiles_share_edge_normals():
    """One-sided in-tile diffs used to light a hard knife at stream borders."""

    def fn(x, z):
        return 0.15 * x + 0.22 * z

    left, _ = kit.heightfield_tile(fn, 0.0, 0.0, tile=10.0, cells=8, uv_half=80.0)
    right, _ = kit.heightfield_tile(fn, 10.0, 0.0, tile=10.0, cells=8, uv_half=80.0)
    le = sorted(_edge_verts(left, x=10.0), key=lambda v: v[2])
    re = sorted(_edge_verts(right, x=10.0), key=lambda v: v[2])
    assert len(le) == 9 and len(re) == 9
    for a, b in zip(le, re):
        assert a[1] == pytest.approx(b[1], abs=1e-9)
        assert a[3] == pytest.approx(b[3], abs=1e-6)
        assert a[4] == pytest.approx(b[4], abs=1e-6)
        assert a[5] == pytest.approx(b[5], abs=1e-6)
        assert a[6] == pytest.approx(b[6], abs=1e-6)
        assert a[7] == pytest.approx(b[7], abs=1e-6)


def test_heightfield_uv_blend_is_not_a_step_at_the_join():
    def fn(_x, _z):
        return 0.4

    kwargs = dict(tile=16.0, cells=8, uv_period=13.5, uv_blend=2.6, uv_pad=0.035)
    a, _ = kit.heightfield_tile(fn, 0.0, 0.0, **kwargs)
    b, _ = kit.heightfield_tile(fn, 16.0, 0.0, **kwargs)
    ae = sorted(_edge_verts(a, x=16.0), key=lambda v: v[2])
    be = sorted(_edge_verts(b, x=16.0), key=lambda v: v[2])
    for va, vb in zip(ae, be):
        assert va[6] == pytest.approx(vb[6], abs=1e-6)
        assert va[7] == pytest.approx(vb[7], abs=1e-6)
        assert 0.02 <= va[6] <= 0.98
        assert 0.02 <= va[7] <= 0.98
    # Interior UV differs from the join (blend actually moves something).
    interior = [v for v in a if abs(v[0] - 8.0) < 1e-6 and abs(v[2] - 8.0) < 1e-6]
    assert interior
    join_u = ae[len(ae) // 2][6]
    assert abs(interior[0][6] - join_u) > 1e-4


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
