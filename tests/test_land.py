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
