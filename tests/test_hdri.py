"""HDRI cube math — GPU 不要。"""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

hdri = load_kagra_submodule("hdri")


def test_plus_y_maps_near_equirect_top():
    _u, v = hdri.dir_to_equirect_uv(0.0, 1.0, 0.0)
    assert v < 0.05


def test_plus_x_face_center_is_horizon():
    dx, dy, dz = hdri.face_dir(0, 0.0, 0.0)
    assert dx > 0.9
    _u, v = hdri.dir_to_equirect_uv(dx, dy, dz)
    assert 0.4 < v < 0.6


def test_studio_top_is_bluer_than_ground():
    pix = hdri.studio_equirect(16, 8)
    top = pix[0]
    bot = pix[-1]
    assert top[2] > top[0]
    assert bot[0] > bot[2]


def test_equirect_to_face_size():
    pix = hdri.studio_equirect(8, 4)
    face = hdri.equirect_to_face(pix, 8, 4, 2, face_size=4)
    assert len(face) == 16


def test_pbr_enabled_defaults_off():
    assert not hdri.pbr_enabled(0.0, 1.0)
    assert hdri.pbr_enabled(1.0, 1.0)
    assert hdri.pbr_enabled(0.0, 0.2)
