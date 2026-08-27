"""GPU-free tests for ``kagra.play_world`` (shared wgpu 30 desktop window).

Does not open a real window. The window path skips without a helper or display.
Never imports kagra-core / RendererV2.
"""
from __future__ import annotations

from pathlib import Path

import pytest

from tests.conftest import ROOT, load_kagra_submodule

_play = load_kagra_submodule("play_world")
find_window_helper = _play.find_window_helper
has_display = _play.has_display
looks_like_no_adapter = _play.looks_like_no_adapter
looks_like_no_display = _play.looks_like_no_display
play_world_dump = _play.play_world_dump
default_world_dump = _play.default_world_dump
resolve_window_cmd = _play.resolve_window_cmd
helper_argv = _play.helper_argv
walk_input_from_keys = _play.walk_input_from_keys


def test_default_dump_is_crest_isle_fixture():
    path = default_world_dump(root=ROOT)
    assert path.is_file()
    text = path.read_text(encoding="utf-8")
    assert '"version": 1' in text or '"version":1' in text
    assert "walker:player" in text


def test_looks_like_no_display_and_adapter():
    assert looks_like_no_display("no display: EventLoopError")
    assert looks_like_no_display("Library libxkbcommon-x11.so could not be loaded")
    assert not looks_like_no_display("WorldDoc window opened")
    assert looks_like_no_adapter("Failed to find an appropriate adapter")
    assert looks_like_no_adapter("No suitable graphics adapter found")
    assert not looks_like_no_adapter("WorldDoc window compile_scene")


def test_find_window_helper_none_in_empty_root(tmp_path, monkeypatch):
    monkeypatch.delenv("KAGRA_WORLD_WINDOW", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))
    assert find_window_helper(root=tmp_path) is None


def test_play_world_skips_without_display(tmp_path, monkeypatch):
    monkeypatch.setenv("KAGRA_WORLD_WINDOW_SKIP", "1")
    monkeypatch.delenv("KAGRA_WORLD_WINDOW_FORCE", raising=False)
    dump = tmp_path / "world.json"
    dump.write_text('{"version": 1, "props": []}', encoding="utf-8")
    result = play_world_dump(
        dump,
        allow_cargo=False,
        root=tmp_path,
        cwd=tmp_path,
        require_display=True,
    )
    assert result.ok
    assert result.skipped
    assert result.skip_reason and "display" in result.skip_reason


def test_play_world_skips_without_helper(tmp_path, monkeypatch):
    monkeypatch.delenv("KAGRA_WORLD_WINDOW", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))
    monkeypatch.setenv("KAGRA_WORLD_WINDOW_FORCE", "1")
    dump = tmp_path / "world.json"
    dump.write_text('{"version": 1}', encoding="utf-8")
    result = play_world_dump(
        dump,
        allow_cargo=False,
        root=tmp_path,
        cwd=tmp_path,
        require_display=True,
    )
    assert result.ok
    assert result.skipped
    assert result.skip_reason and "helper" in result.skip_reason


def test_play_world_missing_dump_is_error(tmp_path, monkeypatch):
    monkeypatch.setenv("KAGRA_WORLD_WINDOW_FORCE", "1")
    result = play_world_dump(
        tmp_path / "nope.json",
        allow_cargo=False,
        root=tmp_path,
        cwd=tmp_path,
    )
    assert not result.ok
    assert not result.skipped
    assert "missing" in (result.error or "")


def test_fake_window_helper_runs_without_gpu(tmp_path, monkeypatch):
    helper = tmp_path / "fake_window.py"
    helper.write_text(
        "import sys\n"
        "from pathlib import Path\n"
        "world = Path(sys.argv[1])\n"
        "assert world.is_file()\n"
        "assert '--width' in sys.argv\n"
        "assert '--height' in sys.argv\n"
        "print('opened window (fake)', world)\n",
        encoding="utf-8",
    )
    monkeypatch.setenv("KAGRA_WORLD_WINDOW", str(helper))
    monkeypatch.setenv("KAGRA_WORLD_WINDOW_FORCE", "1")
    dump = tmp_path / "world.json"
    dump.write_text(
        (ROOT / "kagra-shared/tests/fixtures/orb_rush_world.json").read_text(encoding="utf-8")
    )
    result = play_world_dump(
        dump,
        width=320,
        height=180,
        seconds=1.5,
        allow_cargo=False,
        root=tmp_path,
        cwd=tmp_path,
        timeout_sec=10,
    )
    assert result.ok, result.error
    assert not result.skipped
    assert result.cmd
    argv = helper_argv(helper, dump, width=320, height=180, seconds=1.5)
    assert argv[0]  # python
    assert "--seconds" in argv
    assert "1.5" in argv


def test_resolve_window_cmd_none_without_helper_or_cargo(tmp_path, monkeypatch):
    monkeypatch.delenv("KAGRA_WORLD_WINDOW", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))
    cmd = resolve_window_cmd(
        tmp_path / "w.json",
        width=64,
        height=48,
        seconds=None,
        allow_cargo=False,
        root=tmp_path,
    )
    assert cmd is None


def test_has_display_respects_skip(monkeypatch):
    monkeypatch.setenv("KAGRA_WORLD_WINDOW_SKIP", "1")
    monkeypatch.delenv("KAGRA_WORLD_WINDOW_FORCE", raising=False)
    assert has_display() is False
    monkeypatch.delenv("KAGRA_WORLD_WINDOW_SKIP", raising=False)
    monkeypatch.setenv("KAGRA_WORLD_WINDOW_FORCE", "1")
    assert has_display() is True


def test_cli_help_does_not_need_kagra_core():
    with pytest.raises(SystemExit) as exc:
        _play.main(["--help"])
    assert exc.value.code == 0


def test_walk_input_from_keys_wasd_and_look():
    idle = walk_input_from_keys(())
    assert idle["lx"] == 0.0 and idle["lz"] == 0.0 and idle["jump"] is False
    fwd = walk_input_from_keys(["w"])
    assert fwd["lz"] == 1.0 and fwd["lx"] == 0.0
    strafe = walk_input_from_keys(["a", "d"])  # cancel
    assert strafe["lx"] == 0.0
    left = walk_input_from_keys(["a"])
    assert left["lx"] == -1.0
    look = walk_input_from_keys(["ArrowLeft", "ArrowUp"])
    assert look["look_x"] == -1.0 and look["look_y"] == 1.0
    assert walk_input_from_keys(["space"])["jump"] is True
    # Arrows are look, not wish — WASD owns the walker.
    assert walk_input_from_keys(["left"])["lz"] == 0.0


def test_crest_fixture_has_heightfield_fn_and_player():
    import json

    data = json.loads(default_world_dump(root=ROOT).read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    hf = data["heightfield"]
    assert hf["fn"] == "open_world_height"
    assert hf["samples"]
    crate_p = next(p for p in data["props"] if p.get("name") == "crate")
    assert crate_p.get("gltf") == "crate.glb"
    coin = next(p for p in data["props"] if p.get("name") == "coin")
    assert coin.get("metallic") == 1.0
    assert coin.get("roughness") == 0.12
    tiles = hf["tiles"]
    assert tiles and all(t.get("albedo_ok") for t in tiles)
    slots = {int(lit["slot"]) for lit in data["lights"]}
    assert 0 in slots
    assert data["coins"] >= 1


def test_gltf_prop_dump_shape(tmp_path):
    import json

    dump = {
        "version": 1,
        "half": 8.0,
        "player": {
            "id": "walker:player",
            "type": "walker",
            "position": [0.0, 1.0, 0.0],
            "yaw": 0.0,
            "on_ground": True,
        },
        "props": [
            {
                "id": "prop:crate",
                "type": "prop",
                "name": "crate",
                "position": [2.0, 0.5, 0.0],
                "model": "box",
                "gltf": "cube.glb",
                "scale": [1.0, 1.0, 1.0],
                "enabled": True,
            }
        ],
        "heightfield": {
            "fn": "island_height",
            "samples": [[0.0, 0.0, 0.38], [4.0, 0.0, 0.2]],
        },
        "cameras": [
            {
                "id": "camera:main",
                "type": "camera",
                "position": [0.0, 4.0, 8.0],
                "target": [0.0, 1.0, 0.0],
                "fov": 54.0,
            }
        ],
    }
    path = tmp_path / "gltf_world.json"
    path.write_text(json.dumps(dump), encoding="utf-8")
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["props"][0]["gltf"] == "cube.glb"
    assert data["heightfield"]["fn"] == "island_height"
    wish = walk_input_from_keys(["w", "d"])
    assert wish["lz"] == 1.0 and wish["lx"] == 1.0


def test_walk_input_from_keys_attack_dodge():
    idle = walk_input_from_keys(())
    assert idle["attack"] is False and idle["dodge"] is False
    assert walk_input_from_keys(["j"])["attack"] is True
    assert walk_input_from_keys(["z"])["attack"] is True
    assert walk_input_from_keys(["shift"])["dodge"] is True
    assert walk_input_from_keys(["c"])["dodge"] is True
    assert walk_input_from_keys(["k"])["dodge"] is True
    # WASD still owns the walker.
    assert walk_input_from_keys(["j"])["lz"] == 0.0


def test_action_arena_fixture_has_foes_and_player():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/action_arena_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    assert data.get("heightfield") in (None, {},)
    foes = [p for p in data["props"] if p.get("name") == "foe" and p.get("enabled") is not False]
    assert len(foes) >= 2
    models = {p.get("model") for p in foes}
    assert "capsule" in models
    assert "box" in models
    assert any(p.get("name") == "floor" for p in data["props"])


def test_box_hop_fixture_has_platforms_and_checkpoint():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/box_hop_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    plats = [p for p in data["props"] if p.get("name") == "platform"]
    assert len(plats) >= 2
    assert any(p.get("name") == "checkpoint" for p in data["props"])
    assert any(p.get("name") == "goal" for p in data["props"])
    assert any(p.get("model") == "sprite" for p in data["props"])
    assert data.get("heightfield") in (None, {})


def test_rpg_town_fixture_has_npc_door_and_dungeon():
    import json

    town = json.loads((ROOT / "kagra-shared/tests/fixtures/rpg_town_world.json").read_text(encoding="utf-8"))
    dun = json.loads((ROOT / "kagra-shared/tests/fixtures/rpg_dungeon_world.json").read_text(encoding="utf-8"))
    assert town["player"]["on_ground"] is True
    assert any(p.get("name") == "npc" for p in town["props"])
    assert any(p.get("name") == "door" for p in town["props"])
    assert any(p.get("name") == "crystal" for p in dun["props"])
    assert any(p.get("name") == "door" for p in dun["props"])



def test_sprite_card_fixture_has_quads():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/sprite_card_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    models = {p.get("model") for p in data["props"]}
    assert "sprite" in models
    assert "quad" in models
    assert "plane" in models
    sprites = [p for p in data["props"] if p.get("model") in ("sprite", "quad")]
    assert len(sprites) >= 2
    assert data.get("heightfield") in (None, {})

def test_fps_range_fixture_has_targets_and_player():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/fps_range_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    targets = [p for p in data["props"] if p.get("name") == "target" and p.get("enabled") is not False]
    assert len(targets) >= 2
    models = {p.get("model") for p in targets}
    assert "capsule" in models
    assert "sprite" in models
    assert any(p.get("name") == "floor" for p in data["props"])
    assert data.get("heightfield") in (None, {})


def test_walk_input_from_keys_fire_alias():
    assert walk_input_from_keys(["fire"])["attack"] is True
    assert walk_input_from_keys(["mouse1"])["attack"] is True
    assert walk_input_from_keys(["j"])["attack"] is True
    assert walk_input_from_keys(["r"])["dodge"] is True
    assert walk_input_from_keys(["reload"])["dodge"] is True


def test_td_lane_fixture_has_path_tower_and_creeps():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/td_lane_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    assert any(p.get("name") == "tower" for p in data["props"])
    wps = [p for p in data["props"] if p.get("name") == "waypoint"]
    assert len(wps) >= 3
    creeps = [p for p in data["props"] if p.get("name") == "creep" and p.get("enabled") is not False]
    assert len(creeps) >= 2
    models = {p.get("model") for p in creeps}
    assert "capsule" in models
    assert any(p.get("name") == "path" for p in data["props"])
    assert any(p.get("name") == "floor" for p in data["props"])
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})

def test_race_drive_fixture_has_track_finish_and_car():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/race_drive_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    assert any(p.get("name") == "finish" for p in data["props"])
    assert any(p.get("name") == "split" for p in data["props"])
    assert any(p.get("name") == "flag" for p in data["props"])
    roads = [p for p in data["props"] if p.get("name") == "road"]
    assert len(roads) >= 4
    cars = [p for p in data["props"] if p.get("name") == "car" and p.get("enabled") is not False]
    assert len(cars) == 1
    assert cars[0].get("model") in ("box", "cube", "capsule")
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})

def test_novel_pages_fixture_has_room_and_flag():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/novel_pages_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    assert any(p.get("name") == "page" for p in data["props"])
    speakers = [p for p in data["props"] if p.get("name") == "speaker"]
    assert len(speakers) == 1
    assert speakers[0].get("model") == "capsule"
    flags = [p for p in data["props"] if p.get("name") == "flag"]
    assert len(flags) == 1
    assert flags[0].get("enabled") is False
    assert any(p.get("name") == "floor" for p in data["props"])
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})


def test_fight_hitstun_fixture_has_two_capsules():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/fight_hitstun_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    opps = [p for p in data["props"] if p.get("name") == "opponent" and p.get("enabled") is not False]
    assert len(opps) == 1
    assert opps[0].get("model") == "capsule"
    assert any(p.get("name") == "floor" for p in data["props"])
    assert any(p.get("name") == "ring" for p in data["props"])
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})

def test_stealth_hide_fixture_has_hide_guard_and_exit():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/stealth_hide_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    hides = [p for p in data["props"] if p.get("name") == "hide"]
    assert len(hides) == 1
    assert hides[0].get("model") == "box"
    guards = [p for p in data["props"] if p.get("name") == "guard"]
    assert len(guards) == 1
    assert guards[0].get("model") == "capsule"
    assert any(p.get("name") == "exit" for p in data["props"])
    flags = [p for p in data["props"] if p.get("name") == "flag"]
    assert len(flags) == 1
    assert flags[0].get("enabled") is False
    assert any(p.get("name") == "floor" for p in data["props"])
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})


def test_puzzle_pad_fixture_has_crate_and_pad():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/puzzle_pad_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    pads = [p for p in data["props"] if p.get("name") == "pad"]
    assert len(pads) == 1
    assert pads[0].get("model") == "box"
    crates = [p for p in data["props"] if p.get("name") == "crate"]
    assert len(crates) == 1
    assert crates[0].get("model") == "box"
    flags = [p for p in data["props"] if p.get("name") == "flag"]
    assert len(flags) == 1
    assert flags[0].get("enabled") is False
    assert any(p.get("name") == "floor" for p in data["props"])
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})

def test_sports_goal_fixture_has_ball_and_goal():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/sports_goal_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    balls = [p for p in data["props"] if p.get("name") == "ball"]
    assert len(balls) == 1
    assert balls[0].get("model") == "sphere"
    goals = [p for p in data["props"] if p.get("name") == "goal"]
    assert len(goals) == 1
    assert goals[0].get("model") == "box"
    flags = [p for p in data["props"] if p.get("name") == "flag"]
    assert len(flags) == 1
    assert flags[0].get("enabled") is False
    assert any(p.get("name") == "pitch" for p in data["props"])
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})



def test_sim_meter_fixture_has_zone_and_flag():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/sim_meter_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    zones = [p for p in data["props"] if p.get("name") == "zone"]
    assert len(zones) == 1
    assert zones[0].get("model") == "box"
    flags = [p for p in data["props"] if p.get("name") == "flag"]
    assert len(flags) == 1
    assert flags[0].get("enabled") is False
    assert any(p.get("name") == "floor" for p in data["props"])
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})
    assert data.get("coins", 0) == 0

def test_action_side_fixture_has_sprite_foe_and_wall():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/action_side_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    foes = [p for p in data["props"] if p.get("name") == "foe"]
    assert len(foes) == 1
    assert foes[0].get("model") == "sprite"
    assert any(p.get("name") == "sprite" and p.get("model") == "sprite" for p in data["props"])
    assert any(p.get("name") == "wall" for p in data["props"])
    assert any(p.get("name") == "floor" for p in data["props"])
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})


def test_survival_meter_fixture_has_camp_and_ration():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/survival_meter_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    camps = [p for p in data["props"] if p.get("name") == "camp"]
    assert len(camps) == 1
    assert camps[0].get("model") == "box"
    rations = [p for p in data["props"] if p.get("name") == "ration"]
    assert len(rations) == 1
    assert rations[0].get("model") == "box"
    flags = [p for p in data["props"] if p.get("name") == "flag"]
    assert len(flags) == 1
    assert flags[0].get("enabled") is False
    assert any(p.get("name") == "floor" for p in data["props"])
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})
    assert data.get("coins", 0) == 8


def test_rhythm_beat_fixture_has_stage_and_marker():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/rhythm_beat_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    stages = [p for p in data["props"] if p.get("name") == "stage"]
    assert len(stages) == 1
    assert stages[0].get("model") == "box"
    markers = [p for p in data["props"] if p.get("name") == "marker"]
    assert len(markers) == 1
    assert markers[0].get("model") == "box"
    judges = [p for p in data["props"] if p.get("name") == "judge"]
    assert len(judges) == 1
    assert judges[0].get("model") == "box"
    flags = [p for p in data["props"] if p.get("name") == "flag"]
    assert len(flags) == 1
    assert flags[0].get("enabled") is False
    assert any(p.get("name") == "floor" for p in data["props"])
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})
    assert data.get("coins", 0) == 0


def test_fish_cast_fixture_has_dock_and_water():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/fish_cast_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    assert data.get("water_y") == 0.0
    docks = [p for p in data["props"] if p.get("name") == "dock"]
    assert len(docks) == 1
    assert docks[0].get("model") == "box"
    bobbers = [p for p in data["props"] if p.get("name") == "bobber"]
    assert len(bobbers) == 1
    assert bobbers[0].get("model") == "box"
    assert bobbers[0].get("enabled") is False
    flags = [p for p in data["props"] if p.get("name") == "flag"]
    assert len(flags) == 1
    assert flags[0].get("enabled") is False
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})
    assert data.get("coins", 0) == 0

def test_shop_buy_fixture_has_stall_and_coins():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/shop_buy_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    stalls = [p for p in data["props"] if p.get("name") == "stall"]
    assert len(stalls) == 1
    assert stalls[0].get("model") == "box"
    goods = [p for p in data["props"] if p.get("name") == "goods"]
    assert len(goods) == 1
    assert goods[0].get("model") == "box"
    flags = [p for p in data["props"] if p.get("name") == "flag"]
    assert len(flags) == 1
    assert flags[0].get("enabled") is False
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})
    assert data.get("coins", 0) == 8

def test_cook_stove_fixture_has_stove_and_pan():
    import json

    path = ROOT / "kagra-shared/tests/fixtures/cook_stove_world.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["player"]["id"] == "walker:player"
    assert data["player"]["on_ground"] is True
    stoves = [p for p in data["props"] if p.get("name") == "stove"]
    assert len(stoves) == 1
    assert stoves[0].get("model") == "box"
    pans = [p for p in data["props"] if p.get("name") == "pan"]
    assert len(pans) == 1
    assert pans[0].get("model") == "box"
    meals = [p for p in data["props"] if p.get("name") == "meal"]
    assert len(meals) == 1
    assert meals[0].get("model") == "box"
    assert meals[0].get("enabled") is False
    flags = [p for p in data["props"] if p.get("name") == "flag"]
    assert len(flags) == 1
    assert flags[0].get("enabled") is False
    slots = sorted(int(lit["slot"]) for lit in data["lights"])
    assert slots == [0, 1, 2, 3]
    assert data.get("heightfield") in (None, {})
    assert data.get("coins", 0) == 0
    assert not any(p.get("name") == "stall" for p in data["props"])
