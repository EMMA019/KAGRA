"""「直してここ」 block, gesture, and repro CLI. No Rust extension."""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from tests.conftest import load_kagra_submodule

ROOT = Path(__file__).resolve().parents[1]


def _pt():
    return load_kagra_submodule("playtest")


def test_layout_1280_matches_rust():
    lay = _pt().layout(1280, 720)
    assert abs(lay["scale"] - 1.0) < 1e-6
    assert abs(lay["panel"][2] - 440.0) < 1e-3
    assert abs(lay["panel"][3] - 300.0) < 1e-3
    assert abs(lay["panel"][0] - 420.0) < 1e-3
    assert abs(lay["panel"][1] - 210.0) < 1e-3
    assert abs(lay["note"][1] - 252.0) < 1e-3
    assert abs(lay["note"][3] - 72.0) < 1e-3


def test_three_finger_long_press_opens_and_drift_cancels():
    pt = _pt()
    rec = pt.Playtest(True)
    rec.set_viewport(800, 600)
    assert rec.on_pointer(0, 80, 120, "begin") is False
    assert rec.on_pointer(1, 120, 120, "begin") is False
    assert rec.on_pointer(2, 160, 120, "begin") is True
    assert rec.blocks_sim()
    assert not rec.is_open()
    rec.advance(0.3)
    assert not rec.is_open()
    rec.advance(0.35)
    assert rec.is_open()

    drifted = pt.Playtest(True)
    drifted.set_viewport(800, 600)
    for ident in range(3):
        drifted.on_pointer(ident, 100, 100 + ident * 30, "begin")
    assert drifted.on_pointer(0, 400, 100, "move") is False
    drifted.advance(1.0)
    assert not drifted.is_open()


def test_f12_toggles_and_disabled_ignores_it():
    pt = _pt()
    rec = pt.Playtest(True)
    assert rec.on_key("F12", True)
    assert rec.is_open()
    rec.advance(0.3)
    assert rec.on_key("F12", True)
    assert not rec.is_open()
    off = pt.Playtest(False)
    assert off.on_key("F12", True) is False
    assert not off.is_open()


def test_japanese_roundtrip_and_rust_block():
    pt = _pt()
    rec = pt.Playtest(True)
    assert rec.try_open()
    rec.push_text("直してここ")
    rec.push_text("!")
    rec.backspace()
    assert rec.report()["note"] == "直してここ"
    again = pt.decode_block(pt.encode_block(rec.report()))
    assert again["note"] == "直してここ"
    rust = (ROOT / "kagra-shared/tests/fixtures/playtest_rs_block.txt").read_text(encoding="utf-8")
    decoded = pt.decode_block(rust)
    assert decoded["note"] == "直してここ"
    assert decoded["v"] == 1
    py = (ROOT / "kagra-shared/tests/fixtures/playtest_py_block.txt").read_text(encoding="utf-8")
    assert pt.decode_block(py)["note"] == "直してここ"


def test_issue_url_carries_template_and_overflow_becomes_a_block():
    pt = _pt()
    packet = pt.issue_from_report(
        {"v": 1, "note": "直してここ", "dump": {"version": 1, "genre": "box_hop"}, "inputs": []}
    )
    assert packet["fits"]
    assert packet["url"].startswith(pt.ISSUE_NEW)
    assert "template=playtest.yml" in packet["url"]
    assert "labels=playtest" in packet["url"]
    assert "%E7%9B%B4%E3%81%97%E3%81%A6%E3%81%93%E3%81%93" in packet["url"]
    big = "".join(f"k{i:08x}-" for i in range(12000))
    huge = pt.issue_from_report(
        {"v": 1, "note": "直してここ", "dump": {"version": 1, "blob": big}, "inputs": []}
    )
    assert not huge["fits"]
    assert len(huge["url"]) <= pt.URL_LIMIT
    assert big not in huge["url"]
    assert pt.decode_block(huge["block"])["dump"]["blob"] == big


def test_input_window_and_replay_does_not_fill_gaps():
    pt = _pt()
    rec = pt.Playtest(True)
    rec.snapshot_dump('{"version":1,"genre":"box_hop"}')
    for _ in range(12):
        rec.advance(1.0)
        rec.note_walk(1, 0, False, False, False)
        rec.commit_walk(1.0)
        rec.snapshot_dump('{"version":1,"genre":"box_hop"}')
    assert rec.try_open()
    report = rec.report()
    assert report["dump_t"] >= 1.0
    assert report["dump"]["version"] == 1
    assert report["inputs"]
    for ev in report["inputs"]:
        assert ev["t"] + 1e-3 >= report["dump_t"]
    sample = {
        "dump_t": 1.0,
        "captured_t": 5.0,
        "inputs": [
            {"kind": "walk", "t": 0.5, "dt": 0.016, "lx": 0, "lz": 1},
            {"kind": "walk", "t": 2.0, "dt": 0.5, "lx": 1, "lz": 0, "jump": False, "attack": False, "dodge": False},
            {"kind": "pointer", "t": 2.5},
            {"kind": "confirm", "t": 3.0},
            {"kind": "walk", "t": 9.0, "dt": 0.016, "lx": 0, "lz": 1},
        ],
    }
    steps = list(pt.iter_replay(sample))
    assert [ev["kind"] for ev in steps] == ["walk", "confirm"]
    assert steps[0]["dt"] == 0.5
    assert steps[0]["t"] == 2.0


def test_overlay_hud_does_not_mutate_the_game_hud():
    pt = _pt()
    rec = pt.Playtest(True)
    rec.try_open()
    rec.set_note("直してここ")
    game = {"quads": [{"x": 1}], "texts": [{"text": "HP"}]}
    hud = pt.overlay_hud(game, rec, 1280, 720)
    assert game == {"quads": [{"x": 1}], "texts": [{"text": "HP"}]}
    assert any(t.get("text") == "直してここ" for t in hud["texts"])
    assert any(t.get("text") == "送る" for t in hud["texts"])
    assert hud["quads"][0] == {"x": 1}


def test_agents_and_template_mention_playtest():
    agents = (ROOT / "AGENTS.md").read_text(encoding="utf-8")
    assert "scripts/repro_issue.py" in agents
    template = (ROOT / ".github/ISSUE_TEMPLATE/playtest.yml").read_text(encoding="utf-8")
    assert "playtest" in template
    assert "id: note" in template
    assert "id: build" in template
    assert "id: repro" in template


def test_repro_dump_only_and_missing_version(tmp_path):
    pt = _pt()
    script = ROOT / "scripts" / "repro_issue.py"
    report = {
        "v": 1,
        "note": "直してここ",
        "captured_t": 1.0,
        "dump_t": 0.0,
        "dump": {"version": 1, "genre": "box_hop"},
        "inputs": [
            {
                "kind": "walk",
                "t": 0.5,
                "dt": 0.016,
                "lx": 0,
                "lz": 1,
                "jump": False,
                "attack": False,
                "dodge": False,
            }
        ],
    }
    src = tmp_path / "block.txt"
    src.write_text(pt.encode_block(report), encoding="utf-8")
    out = tmp_path / "out"
    proc = subprocess.run(
        [sys.executable, str(script), str(src), "--dump-only", "--out", str(out)],
        capture_output=True,
        text=True,
    )
    assert proc.returncode == 0, proc.stderr
    saved = json.loads((out / "report.json").read_text(encoding="utf-8"))
    assert saved["note"] == "直してここ"
    assert saved["dump"]["version"] == 1

    bare = tmp_path / "bare.txt"
    bare.write_text(
        pt.encode_block({"v": 1, "note": "x", "dump": {"genre": "nope"}, "inputs": []}),
        encoding="utf-8",
    )
    missing = tmp_path / "missing"
    proc = subprocess.run(
        [sys.executable, str(script), str(bare), "--dump-only", "--out", str(missing)],
        capture_output=True,
        text=True,
    )
    assert proc.returncode == 2
    assert (missing / "report.json").is_file()
