"""街 JSON — GPU 不要。OSM ではない。"""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

city = load_kagra_submodule("city")
land = load_kagra_submodule("land")


def test_load_city_and_chunk(tmp_path):
    path = tmp_path / "town.json"
    path.write_text(
        '{"version":1,"tile":10,"boxes":[{"x":12.0,"z":3.0,"w":2,"h":3,"d":2}]}',
        encoding="utf-8",
    )
    data = city.load_city(path)
    assert data["version"] == 1
    ix, iz = land.tile_index(12.0, 3.0, 10.0)
    boxes = city.city_chunk(data, ix, iz)
    assert len(boxes) == 1
    assert boxes[0][0] == 12.0
    assert boxes[0][4] == 3.0
    assert city.city_chunk(data, 0, 0) == []


def test_bad_version(tmp_path):
    path = tmp_path / "bad.json"
    path.write_text('{"version":2,"boxes":[]}', encoding="utf-8")
    try:
        city.load_city(path)
    except ValueError as e:
        assert "version" in str(e)
    else:
        raise AssertionError("expected ValueError")
