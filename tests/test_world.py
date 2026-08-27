"""World query / dump / load — GPU-free. World is World3D."""
from __future__ import annotations

import json
from pathlib import Path

from tests.conftest import load_kagra_submodule

ROOT = Path(__file__).resolve().parents[1]
CREST_FIXTURE = ROOT / "kagra-shared" / "tests" / "fixtures" / "crest_isle_world.json"
ORB_FIXTURE = ROOT / "kagra-shared" / "tests" / "fixtures" / "orb_rush_world.json"

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


class _Cam:
    def __init__(self, rec):
        pos = rec["position"]
        tgt = rec["target"]
        self.sid = rec["id"]
        self.name = rec["name"]
        self.position = (float(pos[0]), float(pos[1]), float(pos[2]))
        self.target = (float(tgt[0]), float(tgt[1]), float(tgt[2]))
        self.fov_deg = float(rec["fov"])


def _assert_dump_is_world_doc_v1(data: dict):
    assert data["version"] == 1
    for key in (
        "half",
        "floor_y",
        "gravity",
        "water_y",
        "coins",
        "player",
        "props",
        "walkers",
        "lights",
        "cameras",
        "heightfield",
    ):
        assert key in data, key
    blob = json.dumps(data)
    assert "mesh_id" not in blob
    assert "batches" not in blob


def test_crest_isle_shaped_dump_is_world_doc_v1():
    """Crest Isle-shaped World.dump() is the JSON WorldDoc ingests (no GPU)."""
    land = load_kagra_submodule("land")
    w = _world(half=80.0, gravity=9.8)
    w.set_height_fn(
        land.open_world_height,
        cells=8,
        tile=16.0,
        stream_radius=64.0,
        lod_radius=28.0,
        lod_cells=6,
    )
    w.set_water_y(0.0)
    w.terrain_uv_half = 80.0
    w.terrain_uv_period = 48.0
    w.terrain_uv_blend = 0.12
    w.terrain_uv_pad = 0.28
    w.terrain_uv_rect = [0.22, 0.28, 0.62, 0.78]
    w.add_player(0.0, -8.0)
    w.player.y = 1.2
    w.player.on_ground = True
    w._loaded_tiles.update({(0, 0), (-1, 0)})
    crate = play.Prop(
        "box", x=2.0, y=0.5, z=-1.0, world=w, collision=False, name="crate",
    )
    crate.sid = "prop:crate"
    coin = play.Prop(
        "sphere",
        x=0.3,
        y=0.6,
        z=0.0,
        world=w,
        collision=False,
        name="coin",
        parent=crate,
        scale=0.4,
        color=(255, 220, 80),
    )
    coin.sid = "prop:coin"
    w._lights.append(
        {
            "id": "light:0",
            "name": "key",
            "kind": "spot",
            "slot": 0,
            "position": [6.0, 18.0, -8.0],
            "intensity": 1.15,
            "radius": 36.0,
            "color": [1.0, 0.96, 0.86],
            "direction": [-0.18, -1.0, 0.22],
        }
    )
    w._cameras.append(
        _Cam(
            {
                "id": "camera:main",
                "name": "main",
                "position": [0.0, 5.65, 4.2],
                "target": [0.0, 1.25, -8.0],
                "fov": 54.0,
            }
        )
    )
    data = w.dump()
    _assert_dump_is_world_doc_v1(data)
    assert data["half"] == 80.0
    assert data["heightfield"]["fn"] == "open_world_height"
    assert data["heightfield"]["tile"] == 16.0
    tile_ids = {t["id"] for t in data["heightfield"]["tiles"]}
    assert "tile:0,0" in tile_ids
    assert "tile:-1,0" in tile_ids
    by_id = {p["id"]: p for p in data["props"]}
    assert by_id["prop:coin"]["parent"] == "prop:crate"
    assert by_id["prop:crate"]["position"][0] == 2.0
    assert data["player"]["id"] == "walker:player"
    fixture = json.loads(CREST_FIXTURE.read_text(encoding="utf-8"))
    assert fixture["heightfield"]["fn"] == data["heightfield"]["fn"]
    assert {t["id"] for t in fixture["heightfield"]["tiles"]} == tile_ids
    assert fixture["props"][1]["parent"] == "prop:crate"


def test_orb_rush_shaped_dump_is_world_doc_v1():
    """Orb Rush-shaped World.dump() is the JSON WorldDoc ingests (no GPU)."""
    w = _world(half=6.0)
    w.add_player(0.0, 0.0)
    w.player.y = 0.0
    w.player.on_ground = True
    star_a = play.Prop(
        "sphere", x=1.5, y=0.5, z=-1.0, world=w, collision=False, name="star",
        scale=0.28, color=(255, 220, 80),
    )
    star_a.sid = "prop:star-a"
    star_b = play.Prop(
        "sphere", x=-2.0, y=0.5, z=2.5, world=w, collision=False, name="star",
        scale=0.28, color=(255, 220, 80),
    )
    star_b.sid = "prop:star-b"
    bomb = play.Prop(
        "sphere", x=3.0, y=0.5, z=1.0, world=w, collision=False, name="bomb",
        scale=0.32, color=(255, 70, 90),
    )
    bomb.sid = "prop:bomb-a"
    w._cameras.append(
        _Cam(
            {
                "id": "camera:main",
                "name": "main",
                "position": [0.0, 3.2, 6.5],
                "target": [0.0, 0.85, 0.0],
                "fov": 38.0,
            }
        )
    )
    data = w.dump()
    _assert_dump_is_world_doc_v1(data)
    assert data["half"] == 6.0
    assert data["heightfield"] is None
    assert data["coins"] == 0
    by_id = {p["id"]: p for p in data["props"]}
    assert by_id["prop:star-a"]["position"] == [1.5, 0.5, -1.0]
    assert by_id["prop:star-a"]["parent"] is None
    fixture = json.loads(ORB_FIXTURE.read_text(encoding="utf-8"))
    assert {p["id"] for p in fixture["props"]} == set(by_id)


def test_shared_world_fixtures_load_in_python():
    """Committed WorldDoc fixtures are World.dump() JSON Python can load."""
    land = load_kagra_submodule("land")
    w = _world(half=1.0)
    w.load(str(CREST_FIXTURE))
    assert abs(w.half - 80.0) < 1e-6
    assert w._height_fn is land.open_world_height
    assert (0, 0) in w._loaded_tiles
    coins = w.query(type="prop", name="coin")
    crates = w.query(type="prop", name="crate")
    assert len(coins) == 1 and len(crates) == 1
    assert coins[0]["parent"] == crates[0]["id"]
    again = w.dump()
    crate_row = next(p for p in again["props"] if p.get("name") == "crate")
    assert crate_row.get("model") == "box"
    assert crate_row.get("gltf") == "crate.glb"
    assert again["heightfield"]["fn"] == "open_world_height"
    assert {t["id"] for t in again["heightfield"]["tiles"]} >= {"tile:0,0", "tile:-1,0"}

    play.Prop.clear()
    w2 = _world(half=1.0)
    w2.load(str(ORB_FIXTURE))
    assert abs(w2.half - 6.0) < 1e-6
    stars = w2.query(type="prop", name="star")
    bombs = w2.query(type="prop", name="bomb")
    assert len(stars) == 2
    assert len(bombs) == 1
    assert w2._height_fn is None
