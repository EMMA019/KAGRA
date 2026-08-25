"""Distance attenuation + stereo pan. GPU 不要. Keep in sync with kagra-core/src/audio.rs."""
from __future__ import annotations

import math

from tests.conftest import load_kagra_submodule

sp = load_kagra_submodule("spatial")


def test_closer_is_louder_than_far():
    near = sp.spatial_mix(0, 0, 0, 0, 0, 1, 0, 0, 4, ref_distance=4, max_distance=48)
    mid = sp.spatial_mix(0, 0, 0, 0, 0, 1, 0, 0, 8, ref_distance=4, max_distance=48)
    far = sp.spatial_mix(0, 0, 0, 0, 0, 1, 0, 0, 80, ref_distance=4, max_distance=48)
    assert abs(near[0] - 1.0) < 1e-6
    assert abs(mid[0] - 0.5) < 1e-6
    assert far[0] == 0.0
    assert near[0] > mid[0]


def test_right_source_pans_right():
    gain, pan, left, right = sp.spatial_mix(
        0, 0, 0, 0, 0, 1, 4, 0, 0, ref_distance=4, max_distance=48,
    )
    assert pan > 0.9
    assert right > left
    assert left < 0.05
    assert abs(gain - 1.0) < 1e-6


def test_left_source_pans_left():
    gain, pan, left, right = sp.spatial_mix(
        0, 0, 0, 0, 0, 1, -8, 0, 0, ref_distance=4, max_distance=48,
    )
    assert pan < -0.9
    assert left > right
    assert abs(gain - 0.5) < 1e-6


def test_front_is_centered_equal_power():
    gain, pan, left, right = sp.spatial_mix(
        0, 0, 0, 0, 0, 1, 0, 0, 4, ref_distance=4, max_distance=48,
    )
    assert abs(pan) < 1e-6
    half = math.sqrt(0.5)
    assert abs(left - half) < 1e-6
    assert abs(right - half) < 1e-6
    assert abs(gain - 1.0) < 1e-6


def test_coincident_is_full_and_centered():
    gain, pan, left, right = sp.spatial_mix(
        1, 2, 3, 0, 0, 1, 1, 2, 3, ref_distance=4, max_distance=48,
    )
    assert abs(gain - 1.0) < 1e-6
    assert abs(pan) < 1e-6
    assert abs(left - right) < 1e-6


def test_circling_changes_left_right():
    """Walk a circle in XZ: pan sweeps -1 → 0 → +1 → 0."""
    pans = []
    for ang in (math.pi, math.pi * 0.5, 0.0, -math.pi * 0.5):
        x = 4.0 * math.sin(ang)
        z = 4.0 * math.cos(ang)
        _g, pan, _l, _r = sp.spatial_mix(0, 0, 0, 0, 0, 1, x, 0, z)
        pans.append(pan)
    # +Z front, +X right, -Z back, -X left
    assert abs(pans[0]) < 0.05  # behind, no HRTF rear
    assert pans[1] > 0.9
    assert abs(pans[2]) < 0.05
    assert pans[3] < -0.9
