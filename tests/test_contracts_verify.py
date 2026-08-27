"""contracts / verify / touch の単体テスト（GPU 不要）。"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from tests.conftest import load_kagra_submodule

# `import kagra` は Rust 拡張を要求するため、純ロジックのモジュールだけを直接ロードする。
_contracts = load_kagra_submodule("contracts")
_touch = load_kagra_submodule("touch")
_verify = load_kagra_submodule("verify")

AssetKind = _contracts.AssetKind
KagraContractError = _contracts.KagraContractError
candidate_paths = _contracts.candidate_paths
describe_environment = _contracts.describe_environment
resolve_asset = _contracts.resolve_asset

PointerEvent = _touch.PointerEvent
PointerPhase = _touch.PointerPhase
VirtualPad = _touch.VirtualPad

_load_scenario = _verify._load_scenario
load_scenario = _verify.load_scenario


ROOT = Path(__file__).resolve().parents[1]


def test_describe_environment():
    env = describe_environment(ROOT)
    assert env["root"]
    assert "aliases" in env


def test_resolve_walk_fixture():
    # synthetic_walk はリポジトリに含まれる
    p = resolve_asset(AssetKind.BVH, "walk", root=ROOT)
    assert p.exists()
    assert p.suffix.lower() == ".bvh"


def test_resolve_missing_raises_structured():
    with pytest.raises(KagraContractError) as ei:
        resolve_asset(AssetKind.VRM, "definitely_missing_xyz", root=ROOT)
    err = ei.value
    assert err.code == "ASSET_NOT_FOUND"
    assert err.hint
    assert "python -m kagra.demo" in err.hint
    d = err.to_dict()
    assert "candidates" in d


def test_candidate_paths_ordered():
    cands = candidate_paths(AssetKind.VRM, "Emma", root=ROOT)
    assert any("Emma" in str(c) for c in cands)


def test_bundled_dance_bvh_resolves():
    p = resolve_asset(AssetKind.ANY, "dance", root=ROOT)
    assert p.is_file()
    assert p.suffix.lower() == ".bvh"


def test_resolve_demo_headbang_vrma():
    p = resolve_asset(AssetKind.VRMA, "coolHeadbangWalk", root=ROOT, required=False)
    if p is None:
        pytest.skip("assets/coolHeadbangWalk.vrma not present")
    assert p.is_file()
    assert p.suffix.lower() == ".vrma"


def test_resolve_demo_song_wav():
    p = resolve_asset(AssetKind.AUDIO, "cute_song_trial", root=ROOT, required=False)
    if p is None:
        pytest.skip("assets/cute_song_trial.wav not present")
    assert p.is_file()
    assert p.suffix.lower() == ".wav"


def test_virtual_pad_wasd():
    pad = VirtualPad(deadzone=0.2)
    pad.set_stick(0.9, 0.0)
    assert pad.desired_keys() == {"D"}
    ev = list(pad.key_events())
    assert ("D", True) in ev
    pad.set_stick(0.0, 0.0)
    ev2 = list(pad.key_events())
    assert ("D", False) in ev2


def test_pointer_json_roundtrip_shape():
    e = PointerEvent(0, 10, 20, PointerPhase.BEGIN)
    assert e.phase.value == "begin"


def test_load_blank_scenario():
    sc = load_scenario(ROOT / "examples/verify_scenarios/blank_smoke.json")
    assert sc.name == "blank_smoke"
    assert sc.inline is not None
    assert sc.expect


def test_load_scenario_dict():
    sc = _load_scenario(
        {
            "name": "x",
            "script": "scratch/smoke_orb_rush.py",
            "expect_files": ["a.png"],
            "min_file_bytes": 10,
        }
    )
    assert sc.script.endswith("smoke_orb_rush.py")
    assert sc.expect[0].min_bytes == 10
    assert sc.expect_world is None


def test_load_open_world_scenario_has_world_expect():
    sc = load_scenario(ROOT / "examples/verify_scenarios/open_world_smoke.json")
    assert sc.expect_world
    assert sc.expect_world["path"].endswith("open_world_world.json")
    assert sc.expect_world["player.on_ground"] is True


def test_eval_expect_world_missing_dump(tmp_path):
    errors = _verify._eval_expect_world(
        {"path": str(tmp_path / "nope.json"), "coins": 0},
        tmp_path,
    )
    assert errors
    assert "missing" in errors[0]


def _rgba_png(width: int, height: int, pixel=(40, 80, 160, 255)) -> bytes:
    import struct
    import zlib

    r, g, b, a = pixel
    row = bytes([0]) + bytes([r, g, b, a]) * width
    raw = zlib.compress(row * height, 9)

    def chunk(tag: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", raw)
        + chunk(b"IEND", b"")
    )


_render_world = load_kagra_submodule("render_world")
png_dimensions = _render_world.png_dimensions
check_offscreen_png = _render_world.check_offscreen_png
find_offscreen_helper = _render_world.find_offscreen_helper
render_world_dump = _render_world.render_world_dump
eval_expect_offscreen = _render_world.eval_expect_offscreen
looks_like_no_adapter = _render_world.looks_like_no_adapter


def test_load_orb_rush_scenario_has_offscreen_expect():
    sc = load_scenario(ROOT / "examples/verify_scenarios/orb_rush_smoke.json")
    assert sc.expect_world
    assert sc.expect_offscreen
    assert sc.expect_offscreen["out"].endswith("orb_rush_shared.png")
    assert sc.expect_offscreen["width"] == 320
    assert sc.expect_offscreen["height"] == 180


def test_load_collectathon_scenario_is_world_and_offscreen():
    sc = load_scenario(ROOT / "examples/verify_scenarios/collectathon_smoke.json")
    assert sc.script is None
    assert sc.inline is None
    assert sc.expect_world
    assert sc.expect_world["path"].endswith("crest_isle_world.json")
    assert sc.expect_world["player.on_ground"] is True
    assert sc.expect_world["coins"] == 1
    assert sc.expect_offscreen
    assert sc.expect_offscreen["out"].endswith("collectathon_shared.png")


def test_load_action_arena_scenario_is_world_and_offscreen():
    sc = load_scenario(ROOT / "examples/verify_scenarios/action_arena_smoke.json")
    assert sc.expect_world
    assert sc.expect_world["path"].endswith("action_arena_world.json")
    assert sc.expect_world["player.on_ground"] is True
    assert sc.expect_offscreen
    assert sc.expect_offscreen["out"].endswith("action_arena_shared.png")


def test_load_box_hop_scenario_is_world_and_offscreen():
    sc = load_scenario(ROOT / "examples/verify_scenarios/box_hop_smoke.json")
    assert sc.expect_world
    assert sc.expect_world["path"].endswith("box_hop_world.json")
    assert sc.expect_offscreen
    assert sc.expect_offscreen["out"].endswith("box_hop_shared.png")


def test_load_rpg_town_scenario_is_world_and_offscreen():
    sc = load_scenario(ROOT / "examples/verify_scenarios/rpg_town_smoke.json")
    assert sc.expect_world
    assert sc.expect_world["path"].endswith("rpg_town_world.json")
    assert sc.expect_offscreen



def test_load_sprite_card_scenario_is_world_and_offscreen():
    sc = load_scenario(ROOT / "examples/verify_scenarios/sprite_card_smoke.json")
    assert sc.expect_world
    assert sc.expect_world["path"].endswith("sprite_card_world.json")
    assert sc.expect_offscreen


def test_load_fps_range_scenario_is_world_and_offscreen():
    sc = load_scenario(ROOT / "examples/verify_scenarios/fps_range_smoke.json")
    assert sc.expect_world
    assert sc.expect_world["path"].endswith("fps_range_world.json")
    assert sc.expect_world["player.on_ground"] is True
    assert sc.expect_offscreen
    assert sc.expect_offscreen["out"].endswith("fps_range_shared.png")



def test_load_td_lane_scenario_is_world_and_offscreen():
    sc = load_scenario(ROOT / "examples/verify_scenarios/td_lane_smoke.json")
    assert sc.expect_world
    assert sc.expect_world["path"].endswith("td_lane_world.json")
    assert sc.expect_world["player.on_ground"] is True
    assert sc.expect_offscreen
    assert sc.expect_offscreen["out"].endswith("td_lane_shared.png")


def test_png_dimensions_reads_ihdr(tmp_path):
    p = tmp_path / "a.png"
    p.write_bytes(_rgba_png(32, 24))
    assert png_dimensions(p) == (32, 24)


def test_check_offscreen_png_smoke_not_golden(tmp_path):
    p = tmp_path / "ok.png"
    p.write_bytes(_rgba_png(16, 9))
    assert check_offscreen_png(p, width=16, height=9, min_bytes=10) == []
    assert check_offscreen_png(p, width=8, height=9, min_bytes=10)
    empty = tmp_path / "missing.png"
    assert "missing" in check_offscreen_png(empty, width=16, height=9)[0]


def test_eval_expect_offscreen_skips_without_helper(tmp_path, monkeypatch):
    monkeypatch.setenv("KAGRA_OFFSCREEN", str(tmp_path / "no-such-helper"))
    monkeypatch.delenv("KAGRA_OFFSCREEN_CARGO", raising=False)
    dump = tmp_path / "world.json"
    dump.write_text('{"version": 1, "props": []}', encoding="utf-8")
    errors, skipped, result = eval_expect_offscreen(
        {"out": "shot.png", "width": 16, "height": 9, "world": str(dump)},
        None,
        tmp_path,
        allow_cargo=False,
        root=tmp_path,
    )
    assert errors == []
    assert skipped
    assert result is not None and result.skipped


def test_eval_expect_offscreen_required_fails_without_helper(tmp_path, monkeypatch):
    monkeypatch.setenv("KAGRA_OFFSCREEN", str(tmp_path / "no-such-helper"))
    dump = tmp_path / "world.json"
    dump.write_text('{"version": 1}', encoding="utf-8")
    errors, skipped, _result = eval_expect_offscreen(
        {
            "out": "shot.png",
            "world": str(dump),
            "required": True,
        },
        None,
        tmp_path,
        allow_cargo=False,
        root=tmp_path,
    )
    assert errors
    assert skipped is None


def test_offscreen_helper_writes_png_and_checks_dimensions(tmp_path, monkeypatch):
    """GPU-free: a helper that speaks the offscreen CLI writes a PNG; we check IHDR."""
    helper = tmp_path / "fake_offscreen.py"
    helper.write_text(
        "import sys, struct, zlib\n"
        "from pathlib import Path\n"
        "w, h = int(sys.argv[1]), int(sys.argv[2])\n"
        "out = Path(sys.argv[3])\n"
        "assert sys.argv[4] == 'world'\n"
        "world = Path(sys.argv[5])\n"
        "assert world.is_file()\n"
        "row = bytes([0]) + bytes([40, 80, 160, 255]) * w\n"
        "raw = zlib.compress(row * h, 9)\n"
        "def chunk(tag, data):\n"
        "    crc = zlib.crc32(tag + data) & 0xFFFFFFFF\n"
        "    return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', crc)\n"
        "out.parent.mkdir(parents=True, exist_ok=True)\n"
        "out.write_bytes(\n"
        "    b'\\x89PNG\\r\\n\\x1a\\n'\n"
        "    + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0))\n"
        "    + chunk(b'IDAT', raw)\n"
        "    + chunk(b'IEND', b'')\n"
        ")\n"
        "print('wrote', out, w, h)\n",
        encoding="utf-8",
    )
    monkeypatch.setenv("KAGRA_OFFSCREEN", str(helper))
    dump = tmp_path / "world.json"
    dump.write_text((ROOT / "kagra-shared/tests/fixtures/orb_rush_world.json").read_text(encoding="utf-8"))
    out = tmp_path / "shared.png"
    result = render_world_dump(
        dump,
        out,
        width=32,
        height=24,
        min_bytes=32,
        allow_cargo=False,
        root=tmp_path,
        cwd=tmp_path,
    )
    assert result.ok, result.error
    assert not result.skipped
    assert out.is_file()
    assert png_dimensions(out) == (32, 24)
    assert result.width == 32 and result.height == 24


def test_verify_eval_offscreen_skips_without_binary(tmp_path, monkeypatch):
    monkeypatch.setenv("KAGRA_OFFSCREEN", str(tmp_path / "missing-bin"))
    monkeypatch.delenv("KAGRA_OFFSCREEN_CARGO", raising=False)
    dump = tmp_path / "world.json"
    dump.write_text('{"version": 1}', encoding="utf-8")
    errors, skipped = _verify._eval_expect_offscreen(
        {"path": str(dump), "out": str(tmp_path / "x.png"), "width": 8, "height": 8},
        {"path": str(dump)},
        tmp_path,
    )
    assert errors == []
    assert skipped


def test_looks_like_no_adapter():
    assert looks_like_no_adapter("Failed to find an appropriate adapter")
    assert looks_like_no_adapter(
        'Error: "No suitable graphics adapter found; noop support not compiled in"'
    )
    assert not looks_like_no_adapter("wrote out.png from WorldDoc")


def test_find_offscreen_helper_none_in_empty_root(tmp_path, monkeypatch):
    monkeypatch.delenv("KAGRA_OFFSCREEN", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))
    assert find_offscreen_helper(root=tmp_path) is None


def test_real_shared_offscreen_png_dimensions(tmp_path, monkeypatch):
    """If the wgpu 30 example/binary is already built, smoke IHDR — not golden."""
    monkeypatch.delenv("KAGRA_OFFSCREEN", raising=False)
    helper = find_offscreen_helper(root=ROOT)
    if helper is None:
        pytest.skip("no shared offscreen helper")
    out = tmp_path / "real.png"
    result = render_world_dump(
        ROOT / "kagra-shared/tests/fixtures/orb_rush_world.json",
        out,
        width=64,
        height=48,
        min_bytes=64,
        allow_cargo=False,
        root=ROOT,
        cwd=tmp_path,
    )
    if result.skipped:
        pytest.skip(result.skip_reason or "offscreen skipped")
    assert result.ok, result.error
    assert png_dimensions(out) == (64, 48)
