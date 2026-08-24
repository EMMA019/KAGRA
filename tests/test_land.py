"""島の高さ — GPU 不要。"""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

land = load_kagra_submodule("land")


def test_island_has_sea_grass_mountain():
    assert land.biome_at(-12.0, 0.0) == "sea"
    assert land.biome_at(0.0, 0.0) == "grass"
    assert land.island_height(9.0, 6.0) > 2.2
    assert land.biome_at(9.0, 6.0) == "mountain"


def test_terrain_rgba_sea_is_bluer_than_grass():
    sea = land.terrain_rgba(0.1, 0.5, half=24.0)
    grass = land.terrain_rgba(0.5, 0.5, half=24.0)
    assert sea[2] > sea[0]
    assert grass[1] > grass[2]


def test_tile_keys_cover_origin_and_clip_half():
    keys = land.tile_keys(0.0, 0.0, tile=10.0, radius=12.0)
    assert (0, 0) in keys
    assert (-1, 0) in keys
    clipped = land.tile_keys(0.0, 0.0, tile=10.0, radius=40.0, half=24.0)
    assert all(abs(ix) <= 3 and abs(iz) <= 3 for ix, iz in clipped)
    far = land.tile_keys(30.0, 0.0, tile=10.0, radius=12.0, half=24.0)
    assert (0, 0) not in far
    assert (2, 0) in far


def test_stair_y_rises_in_steps():
    y0 = land.stair_y(0.0, 0.1, x0=-1, x1=1, z0=0, z1=3, y0=0.0, y1=1.8, steps=6)
    y1 = land.stair_y(0.0, 2.9, x0=-1, x1=1, z0=0, z1=3, y0=0.0, y1=1.8, steps=6)
    assert y0 is not None and y1 is not None
    assert y1 > y0 + 1.0
    assert land.stair_y(0.0, 9.0, x0=-1, x1=1, z0=0, z1=3, y0=0.0, y1=1.8) is None


def test_ramp_and_overworld_compose():
    r = land.ramp_y(4.0, -5.5, x0=2.5, x1=7.0, z0=-7.0, z1=-4.5, y0=0.35, y1=1.7)
    assert r is not None
    assert 0.5 < r < 1.4
    assert land.overworld_height(-4.2, 5.5) > land.island_height(-4.2, 5.5)


def test_city_boxes_skip_spawn_and_sea():
    assert land.city_boxes(0, 0) == []
    # west bay is sea
    sea_ix = land.tile_index(-12.0, 0.0)[0]
    assert land.city_boxes(sea_ix, 0) == []
