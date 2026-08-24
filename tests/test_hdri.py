"""HDRI cube math — GPU 不要。"""
from __future__ import annotations

import pytest

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


def test_irradiance_constant_matches_source():
    pix = [(0.40, 0.50, 0.60)] * (8 * 4)
    faces = hdri.irradiance_cube(pix, 8, 4, face_size=2, samples=8)
    assert len(faces) == 6
    assert len(faces[0]) == 4
    for face in faces:
        for r, g, b in face:
            assert abs(r - 0.40) < 0.04
            assert abs(g - 0.50) < 0.04
            assert abs(b - 0.60) < 0.04


def test_studio_irradiance_plus_y_is_bluer_than_minus_y():
    pix = hdri.studio_equirect(16, 8)
    plus_y = hdri.irradiance_face(pix, 16, 8, 2, face_size=2, samples=12)
    minus_y = hdri.irradiance_face(pix, 16, 8, 3, face_size=2, samples=12)
    top = plus_y[0]
    bot = minus_y[0]
    assert top[2] > top[0]
    assert bot[0] > bot[2]


def test_spot_on_axis_is_one():
    import math

    outer, inner = hdri.spot_cone_params(0.9, 0.3)
    assert hdri.spot_cone_factor((0.0, -1.0, 0.0), (0.0, -1.0, 0.0), outer, inner) == pytest.approx(1.0)


def test_spot_outside_cone_is_zero():
    import math

    outer, inner = hdri.spot_cone_params(0.6, 0.2)
    assert hdri.spot_cone_factor((1.0, 0.0, 0.0), (0.0, -1.0, 0.0), outer, inner) == 0.0


def test_spot_penumbra_is_between():
    outer, inner = hdri.spot_cone_params(0.9, 0.5)
    mid_angle = 0.9 * 0.75
    import math
    # from-light direction at mid_angle from -Y toward +X
    dx = math.sin(mid_angle)
    dy = -math.cos(mid_angle)
    t = hdri.spot_cone_factor((dx, dy, 0.0), (0.0, -1.0, 0.0), outer, inner)
    assert 0.0 < t < 1.0
