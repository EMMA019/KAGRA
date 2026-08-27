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
