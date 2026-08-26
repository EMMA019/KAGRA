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

    kwargs = dict(tile=16.0, cells=8, uv_period=48.0, uv_blend=0.0, uv_pad=0.28)
    a, _ = kit.heightfield_tile(fn, 0.0, 0.0, **kwargs)
    b, _ = kit.heightfield_tile(fn, 16.0, 0.0, **kwargs)
    ae = sorted(_edge_verts(a, x=16.0), key=lambda v: v[2])
    be = sorted(_edge_verts(b, x=16.0), key=lambda v: v[2])
    for va, vb in zip(ae, be):
        assert va[6] == pytest.approx(vb[6], abs=1e-6)
        assert va[7] == pytest.approx(vb[7], abs=1e-6)
        assert 0.28 <= va[6] <= 0.72
        assert 0.28 <= va[7] <= 0.72
    interior = [v for v in a if abs(v[0] - 8.0) < 1e-6 and abs(v[2] - 8.0) < 1e-6]
    assert interior
    join_u = ae[len(ae) // 2][6]
    # World-continuous ping-pong: tile center and join are different X, so UV
    # must move. A per-tile 0..1 restart would put both centers at the same U.
    assert abs(interior[0][6] - join_u) > 1e-4


def _synth_dirt_border(u, v, rim=0.12):
    """Stand-in for aerial_grass_rock: dirt near UV 0/1, grass in the interior."""
    edge = min(u, 1.0 - u, v, 1.0 - v)
    return 0.15 if edge < rim else 0.85


def test_heightfield_crest_uvs_skip_jpeg_dirt_border():
    """ClampToEdge + full 0..1 ping-pong stamped the JPEG's dirt frame."""

    def fn(_x, _z):
        return 0.4

    rim = 0.12
    kwargs = dict(tile=16.0, cells=8, uv_period=48.0, uv_blend=0.0, uv_pad=0.28)
    tiles = []
    for ix in range(-1, 2):
        for iz in range(-1, 2):
            verts, _ = kit.heightfield_tile(fn, ix * 16.0, iz * 16.0, **kwargs)
            tiles.append(verts)
    for verts in tiles:
        for v in verts:
            u, vv = v[6], v[7]
            assert rim <= u <= 1.0 - rim
            assert rim <= vv <= 1.0 - rim
            assert _synth_dirt_border(u, vv, rim=rim) == 0.85


def test_heightfield_adjacent_tiles_have_no_tile_sized_albedo_step():
    """Per-tile local 0..1 would restart the JPEG every TILE metres."""

    def fn(_x, _z):
        return 0.4

    kwargs = dict(tile=16.0, cells=16, uv_period=48.0, uv_blend=0.0, uv_pad=0.28)
    a, _ = kit.heightfield_tile(fn, 0.0, 0.0, **kwargs)
    b, _ = kit.heightfield_tile(fn, 16.0, 0.0, **kwargs)
    z = 8.0

    def at(verts, x):
        hits = [v for v in verts if abs(v[0] - x) < 1e-6 and abs(v[2] - z) < 1e-6]
        assert hits, (x, z)
        return hits[0]

    join_a, join_b = at(a, 16.0), at(b, 16.0)
    assert join_a[6] == pytest.approx(join_b[6], abs=1e-6)
    assert join_a[7] == pytest.approx(join_b[7], abs=1e-6)
    grass_a = _synth_dirt_border(join_a[6], join_a[7])
    grass_b = _synth_dirt_border(join_b[6], join_b[7])
    assert grass_a == pytest.approx(grass_b, abs=1e-9)
    assert grass_a == 0.85

    # Tile centers must not share the same U (that is the per-tile restart).
    c0, c1 = at(a, 8.0), at(b, 24.0)
    assert abs(c0[6] - c1[6]) > 0.05
    assert _synth_dirt_border(c0[6], c0[7]) == 0.85
    assert _synth_dirt_border(c1[6], c1[7]) == 0.85

    # The 16 m join is not a grass/dirt knife: albedo 2 m either side matches
    # to a small ping-pong delta, not a dirt-rim vs moss jump (~0.7).
    left = at(a, 14.0)
    right = at(b, 18.0)
    al = _synth_dirt_border(left[6], left[7])
    ar = _synth_dirt_border(right[6], right[7])
    assert abs(al - ar) < 0.05


def test_heightfield_uv_rect_maps_pingpong_into_window():
    """World3D passes uv_rect into heightfield_tile; ignoring it is a bug."""

    def fn(_x, _z):
        return 0.4

    rect = (0.535, 0.485, 0.640, 0.590)
    verts, _ = kit.heightfield_tile(
        fn, 16.0, 48.0, tile=16.0, cells=8,
        uv_period=48.0, uv_pad=0.28, uv_rect=rect,
    )
    u0, v0, u1, v1 = rect
    for v in verts:
        assert u0 - 1e-9 <= v[6] <= u1 + 1e-9
        assert v0 - 1e-9 <= v[7] <= v1 + 1e-9
    with pytest.raises(ValueError, match="degenerate"):
        kit.heightfield_tile(
            fn, 0.0, 0.0, tile=16.0, cells=4, uv_rect=(0.2, 0.2, 0.2, 0.8),
        )



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
