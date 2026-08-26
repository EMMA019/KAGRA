"""World query / dump / load — GPU-free. World is World3D."""
from __future__ import annotations

import json

from tests.conftest import load_kagra_submodule

play = load_kagra_submodule("play")
world_mod = load_kagra_submodule("world")


def setup_function(_fn=None):
    play.Prop.clear()


def teardown_function(_fn=None):
    play.Prop.clear()


def _world(**kwargs):
    return world_mod.World(**kwargs)


def test_world_is_world3d():
    assert world_mod.World is world_mod.World3D
    w = _world(half=8.0)
    assert type(w).__name__ == "World3D"
    assert callable(w.query)
    assert callable(w.dump)
    assert callable(w.load)


def test_player_query_without_screenshot():
    w = _world(half=12.0)
    p = w.add_player(1.5, -2.0)
    p.on_ground = True
    rows = w.query(type="walker", name="player")
    assert len(rows) == 1
    rec = rows[0]
    assert rec["id"] == "walker:player"
    assert rec["type"] == "walker"
    assert rec["name"] == "player"
    assert rec["position"][0] == 1.5
    assert rec["position"][2] == -2.0
    assert rec["on_ground"] is True
    dump = w.dump()
    assert dump["player"]["id"] == "walker:player"
    assert dump["coins"] == 0


def test_query_filters_type_name_aabb():
    w = _world(half=12.0)
    play.Prop("box", x=0.0, y=0.5, z=0.0, world=w, collision=False, name="coin")
    play.Prop("box", x=8.0, y=0.5, z=8.0, world=w, collision=False, name="coin")
    play.Prop("box", x=1.0, y=0.5, z=1.0, world=w, collision=False, name="crate")
    coins = w.query(type="prop", name="coin")
    assert len(coins) == 2
    near = w.query(type="prop", name="coin", aabb=(-1.0, -1.0, 2.0, 2.0))
    assert len(near) == 1
    assert near[0]["id"].startswith("prop:")
    crates = w.query(name="crate")
    assert len(crates) == 1


def test_missing_tile_albedo_is_visible_to_query():
    """Bald leftover: key in _loaded_tiles, GPU tex set, no mesh → albedo_ok False."""
    w = _world(half=24.0)
    w.set_height_fn(lambda _x, _z: 0.4, tile=16.0, stream_radius=32.0)
    w._terrain_tex = 7
    w._loaded_tiles.add((1, 2))
    w._tile_meshes.pop((1, 2), None)
    tiles = w.query(type="terrain_tile")
    bald = [t for t in tiles if t["ix"] == 1 and t["iz"] == 2]
    assert len(bald) == 1
    rec = bald[0]
    assert rec["id"] == "tile:1,2"
    assert rec["type"] == "terrain_tile"
    assert rec["loaded"] is True
    assert rec["has_mesh"] is False
    assert rec["albedo_ok"] is False


def test_gpu_free_stream_tile_is_not_bald():
    """Tests track keys without GPU. tex==0 + loaded + no mesh is not leftover はげ."""
    w = _world(half=24.0)
    w.set_height_fn(lambda _x, _z: 0.2, tile=16.0)
    w._terrain_tex = 0
    w._loaded_tiles.add((0, 0))
    tiles = w.query(type="tile")
    assert tiles
    assert tiles[0]["loaded"] is True
    assert tiles[0]["albedo_ok"] is True


def test_dump_load_roundtrip_props_parent_height_lights(tmp_path):
    w = _world(half=16.0, gravity=9.8)
    land = load_kagra_submodule("land")
    w.set_height_fn(land.island_height, tile=10.0, stream_radius=20.0)
    w.set_water_y(0.0)
    w.add_player(0.0, 4.0)
    w.player.on_ground = True
    w._loaded_tiles.add((0, 0))
    parent = play.Prop(
        "box", x=2.0, y=0.5, z=-1.0, world=w, collision=False, name="crate",
    )
    child = play.Prop(
        "box", x=0.3, y=0.6, z=0.0, world=w, collision=False, name="coin",
        parent=parent,
    )
    w._lights.append(
        {
            "id": "light:0",
            "name": "light:0",
            "kind": "point",
            "slot": 0,
            "position": [1.0, 2.0, 3.0],
            "intensity": 0.8,
            "radius": 12.0,
            "color": [1.0, 0.9, 0.8],
            "direction": None,
        }
    )
    path = tmp_path / "world.json"
    data = w.dump(str(path))
    assert path.is_file()
    assert data["version"] == 1
    assert data["coins"] == 1
    assert data["heightfield"]["fn"] == "island_height"
    assert data["heightfield"]["uv"]["period"] is None
    coin_row = next(p for p in data["props"] if p["name"] == "coin")
    assert coin_row["parent"] == parent.sid
    assert coin_row["id"] == child.sid
    assert not any("mesh_id" in p for p in data["props"])

    play.Prop.clear()
    w2 = _world(half=1.0)
    w2.load(str(path))
    assert abs(w2.half - 16.0) < 1e-6
    assert w2.player is not None
    assert abs(w2.player.z - 4.0) < 1e-6
    coins = w2.query(type="prop", name="coin")
    crates = w2.query(type="prop", name="crate")
    assert len(coins) == 1
    assert len(crates) == 1
    assert coins[0]["parent"] == crates[0]["id"]
    lights = w2.query(type="light")
    assert lights and lights[0]["id"] == "light:0"
    assert w2._height_fn is land.island_height
    again = json.loads(json.dumps(w2.dump()))
    assert again["coins"] == 1
    assert again["heightfield"]["fn"] == "island_height"


def test_eval_world_expect_player_coins_query():
    w = _world(half=8.0)
    w.add_player(0.0, 0.0)
    w.player.on_ground = True
    play.Prop("box", x=1, y=0.5, z=1, world=w, collision=False, name="coin")
    play.Prop("box", x=2, y=0.5, z=2, world=w, collision=False, name="coin")
    data = w.dump()
    ok = world_mod.eval_world_expect(
        data,
        {
            "player.on_ground": True,
            "coins": 2,
            "query": [
                {"type": "walker", "name": "player", "count": 1},
                {"type": "prop", "name": "coin", "count": 2},
            ],
        },
    )
    assert ok == []
    bad = world_mod.eval_world_expect(data, {"coins": 0, "player.on_ground": False})
    assert any("coins" in e for e in bad)
    assert any("on_ground" in e for e in bad)
