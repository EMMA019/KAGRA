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
