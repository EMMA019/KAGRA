"""サンプル VRM 解決（ネットワーク不要）。"""
from __future__ import annotations

from pathlib import Path

import pytest

from tests.conftest import load_kagra_submodule

samples = load_kagra_submodule("samples")
KagraContractError = load_kagra_submodule("contracts").KagraContractError


def test_cache_dir_override(tmp_path, monkeypatch):
    monkeypatch.setenv("KAGRA_CACHE_DIR", str(tmp_path / "c"))
    assert samples.cache_dir() == tmp_path / "c"


def test_ensure_vrm_from_env(tmp_path, monkeypatch):
    fake = tmp_path / "me.vrm"
    fake.write_bytes(b"glTF")
    monkeypatch.setenv("KAGRA_VRM", str(fake))
    assert samples.ensure_vrm("ignored", download=False) == fake.resolve()


def test_ensure_vrm_env_missing(tmp_path, monkeypatch):
    monkeypatch.setenv("KAGRA_VRM", str(tmp_path / "nope.vrm"))
    with pytest.raises(KagraContractError) as ei:
        samples.ensure_vrm("Emma", download=False)
    assert ei.value.code == "ASSET_NOT_FOUND"


def test_ensure_vrm_unknown_does_not_use_cache(tmp_path, monkeypatch):
    cache = tmp_path / "cache"
    cache.mkdir()
    (cache / samples.SAMPLE_FILENAME).write_bytes(b"x")
    monkeypatch.setenv("KAGRA_CACHE_DIR", str(cache))
    monkeypatch.delenv("KAGRA_VRM", raising=False)
    with pytest.raises(KagraContractError) as ei:
        samples.ensure_vrm("definitely_missing_xyz", download=False, root=tmp_path)
    assert ei.value.code == "ASSET_NOT_FOUND"
    assert "python -m kagra.demo" in ei.value.hint


def test_ensure_vrm_alias_uses_cache(tmp_path, monkeypatch):
    cache = tmp_path / "cache"
    dest = cache / samples.SAMPLE_FILENAME
    dest.parent.mkdir()
    dest.write_bytes(b"x")
    monkeypatch.setenv("KAGRA_CACHE_DIR", str(cache))
    monkeypatch.delenv("KAGRA_VRM", raising=False)
    assert samples.ensure_vrm("Emma", download=False, root=tmp_path) == dest


def test_download_checksum_mismatch(tmp_path, monkeypatch):
    class _Resp:
        def __enter__(self):
            return self

        def __exit__(self, *a):
            return False

        def read(self, n=-1):
            if getattr(self, "done", False):
                return b""
            self.done = True
            return b"not-a-vrm"

    monkeypatch.setattr(samples.urllib.request, "urlopen", lambda *a, **k: _Resp())
    dest = tmp_path / "out.vrm"
    with pytest.raises(KagraContractError) as ei:
        samples._download("http://example.invalid/x.vrm", dest)
    assert ei.value.code == "SAMPLE_CHECKSUM_MISMATCH"


def test_cli_help():
    cli = load_kagra_submodule("cli")
    assert cli.main(["--help"]) == 0


def test_demo_parser_offline():
    demo = load_kagra_submodule("demo")
    args = demo.build_parser().parse_args(["--offline", "--width", "320"])
    assert args.offline is True
    assert args.width == 320
    assert args.dance == demo.DEFAULT_DANCE
    assert args.song == demo.DEFAULT_SONG


def test_demo_parser_song_dance_override():
    demo = load_kagra_submodule("demo")
    args = demo.build_parser().parse_args(
        ["--dance", "coolHeadbangWalk", "--song", "cute_song_trial", "--hidden", "--max-frames", "8"]
    )
    assert args.hidden is True
    assert args.max_frames == 8
    assert args.dance == "coolHeadbangWalk"
    assert args.song == "cute_song_trial"


def test_demo_parser_loop_mascot():
    demo = load_kagra_submodule("demo")
    args = demo.build_parser().parse_args(["--loop", "--mascot"])
    assert args.loop is True
    assert args.mascot is True
    assert args.width is None
    assert args.height is None


def test_demo_auto_dance_helpers():
    demo = load_kagra_submodule("demo")
    assert demo.DEFAULT_DANCE == "auto"
    assert demo._is_auto_dance("auto")
    assert demo._is_auto_dance("ALL")
    assert demo._is_auto_dance("*")
    assert not demo._is_auto_dance("Samba Dancing")
    used: set[str] = set()
    a = demo._unique_clip_name(Path("Samba Dancing.fbx"), used)
    b = demo._unique_clip_name(Path("other/Samba Dancing.fbx"), used)
    assert a == "Samba Dancing"
    assert b == "Samba Dancing_2"
    assert demo._frames_duration([({}, 0.25, (0, 0, 0)), ({}, 0.5)]) == 0.75
    assert demo._frames_duration([]) == 0.5


def test_demo_discover_explicit(tmp_path, monkeypatch):
    demo = load_kagra_submodule("demo")
    fbx = tmp_path / "wave.fbx"
    fbx.write_bytes(b"x")
    monkeypatch.setattr(demo, "_resolve_optional", lambda kind, name: str(fbx))
    assert demo._discover_dance_paths("wave") == [fbx]
