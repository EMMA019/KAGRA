"""HOP-BEAT: the Python spec mirror, the music pack, the manifest and the page's synced BGM."""
from __future__ import annotations

import copy
import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

from kagra import hop_beat as hb

ROOT = Path(__file__).resolve().parents[1]
PLAY = ROOT / "scripts/preview_site/play.html"
GRID = ROOT / hb.AUDIO_DIR / "hop_beat_beatgrid.json"


def test_baked_fixture_equals_the_defaults():
    assert json.loads(hb.FIXTURE.read_text(encoding="utf-8")) == hb.world()
    assert hb.main(["--check"]) == 0


def test_world_shape():
    w = hb.world()
    assert w["genre"] == "hop_beat" and w["version"] == 1
    assert w["game"]["lives"]["count"] == 3
    assert w["game"]["music"]["bpm"] == 126.0


def test_overrides_merge_and_unknown_keys_are_refused():
    assert hb.game_from_overrides({"lives": {"count": 5}})["lives"]["count"] == 5
    with pytest.raises(ValueError, match="unknown field"):
        hb.game_from_overrides({"nope": 1})
    with pytest.raises(ValueError, match="unknown field"):
        hb.game_from_overrides({"music": {"tempo": 100}})
    with pytest.raises(ValueError, match="not a cue"):
        hb.game_from_overrides({"audio": {"sfx": {"zap": {"file": "x", "rate": 1, "volume": 1}}}})


@pytest.mark.parametrize("over", [
    {"music": {"bpm": 10}},
    {"music": {"bpm": 999}},
    {"lives": {"count": 0}},
    {"judge": {"perfect_ms": 200}},
    {"hero": {"tap": {"air_beats": 3.0}}},
    {"course": {"phrases": [{"items": [{"kind": "wall", "hop": "tap"}]}]}},
])
def test_out_of_range_values_are_refused(over):
    with pytest.raises(ValueError):
        hb.game_from_overrides(over)


def test_default_enemies_are_the_three_staged_kinds():
    en = hb.default_game()["enemies"]
    ids = [t["id"] for t in en["types"]]
    assert ids == ["slime", "beetle", "bat"]
    assert [t["unlock_level"] for t in en["types"]] == [0, 1, 2], "one new kind per level"
    slime, beetle, bat = en["types"]
    assert slime["stompable"] and beetle["stompable"] and not bat["stompable"]
    assert bat["y_lo"] > 1.0, "a bat hovers where a hold hop climbs"
    assert beetle["step_m"] * beetle["walk_steps"] <= hb.MAX_WALK_M
    assert "stomp" in hb.SFX_CUES and "stomp" in hb.default_game()["audio"]["sfx"]
    assert (ROOT / hb.AUDIO_DIR / "stomp.wav").is_file()


def test_enemy_types_merge_over_the_slime_defaults_and_reject_nonsense():
    g = hb.game_from_overrides({"enemies": {"types": [{"id": "ghost", "look": "bat", "y_lo": 1.4,
                                                       "y_hi": 2.0, "stompable": False}]}})
    assert g["enemies"]["types"][0]["step_m"] == 0.0 and g["enemies"]["types"][0]["color"] == [120, 240, 150]
    for bad in (
        {"types": [{"id": "x", "bogus": 1}]},
        {"types": [{"id": "x"}, {"id": "x"}]},
        {"types": [{"id": "Bad Id"}]},
        {"types": [{"id": "x", "y_lo": 1.0, "y_hi": 2.0}]},  # stompable but airborne
        {"types": [{"id": "x", "look": "dragon"}]},
        {"types": [{"id": "x", "step_m": 3.0, "walk_steps": 8}]},
        {"types": [{"id": f"t{i}"} for i in range(9)]},
        {"body_h": 0.0},
    ):
        with pytest.raises(ValueError):
            hb.game_from_overrides({"enemies": bad})


def test_phrases_may_name_enemies_and_the_static_placement_rules_hold():
    def phrase(items, level=0):
        return {"level": level, "bars": 4, "items": items}

    def foe(beat, enemy, beats=0.2, hop="tap"):
        return {"beat": beat, "kind": "enemy", "beats": beats, "hop": hop, "enemy": enemy}

    ok = hb.game_from_overrides({"course": {"phrases": [phrase([foe(1, "slime"), foe(3, "slime")])]}})
    assert hb.phrase_problems(ok) == []
    with pytest.raises(ValueError, match="weight"):
        hb.game_from_overrides({"course": {"phrases": [{**phrase([foe(1, "slime")]), "weight": 99}]}})
    g = hb.default_game()
    assert hb.phrase_problems(g, [phrase([foe(1, "ghost")])])
    assert hb.phrase_problems(g, [phrase([foe(1, "beetle", 0.25)], level=0)]), "beetle unlocks at level 1"
    assert hb.phrase_problems(g, [phrase([foe(1, "slime"), foe(2, "slime")])]), "no room to land and bounce"
    assert hb.phrase_problems(g, [phrase([foe(1, "slime"), {"beat": 3, "kind": "gap", "beats": 0.5, "hop": "tap", "enemy": "slime"}])])
    assert hb.phrase_problems(g, [phrase([foe(1, "slime"), foe(3, "slime")])]) == []
    # a hovering bat needs no press, so it may share a beat with the slime below it
    assert hb.phrase_problems(g, [phrase([foe(1, "slime"), foe(1, "bat", 0.9)], level=2)]) == []


def test_the_built_in_phrase_set_lives_in_rust_so_the_fixture_leaves_it_empty():
    assert hb.default_game()["course"]["phrases"] == []


def test_music_pack_matches_the_spec():
    grid = json.loads(GRID.read_text(encoding="utf-8"))
    game = hb.default_game()
    assert hb.check_beatgrid(game, grid) == []
    assert hb.loop_secs(game) == pytest.approx(60.952, abs=0.001)
    bad = copy.deepcopy(grid)
    bad["bpm"] = 120.0
    assert hb.check_beatgrid(game, bad)
    shifted = copy.deepcopy(grid)
    shifted["beat_times_s"] = [t + 0.02 for t in shifted["beat_times_s"]]
    assert any("drifts" in p for p in hb.check_beatgrid(game, shifted))


def test_every_cue_file_exists_and_stingers_are_mp3():
    game = hb.default_game()
    for name, cue in game["audio"]["sfx"].items():
        path = ROOT / cue["file"]
        assert path.is_file(), cue["file"]
        assert path.suffix == (".mp3" if name in hb.STINGERS else ".wav")
    loop = ROOT / game["audio"]["bgm"]["file"]
    assert loop.is_file() and loop.suffix == ".mp3"
    assert loop.read_bytes()[:3] in (b"ID3", b"\xff\xfb\x90") or loop.read_bytes()[0] == 0xFF


def test_sfx_generator_matches_the_committed_wavs():
    done = subprocess.run([sys.executable, str(ROOT / "tools/gen_assets/hop_audio.py"), "--check"],
                          capture_output=True, text=True, timeout=120)
    assert done.returncode == 0, done.stdout + done.stderr


def test_manifest_is_a_direct_play_game_with_a_revive_slot():
    m = json.loads((ROOT / "games/hop_beat_minimal.json").read_text(encoding="utf-8"))
    assert m["world"].endswith("hop_beat_world.json")
    assert "assets/audio/hop_beat" in m["bundle_assets"]
    ads = m["platform"]["ads"]
    assert ads["rewarded"][0]["effect"] == "revive" and ads["rewarded"][0]["per_run"] == 1
    assert "retry" in ads["commercial_on"]


def test_i18n_keys_match_between_languages():
    node = shutil.which("node")
    if node is None:
        pytest.skip("node is not installed")
    script = (
        "const I=require(process.argv[1]);const e=I.create('en'),j=I.create('ja');"
        "const keys=['hop.blurb','hop.continue','hop.sync','hop.sync.help'];"
        "console.log(JSON.stringify(keys.map(k=>[e.t(k),j.t(k)])));"
    )
    done = subprocess.run([node, "-e", script, str(ROOT / "scripts/preview_site/i18n.js")],
                          capture_output=True, text=True, timeout=30)
    assert done.returncode == 0, done.stderr
    for en, ja in json.loads(done.stdout):
        assert en and ja and en != ja
        assert not en.startswith("hop.") and not ja.startswith("hop.")


def test_page_starts_the_loop_at_the_sims_position_and_reads_the_clock_back():
    node = shutil.which("node")
    if node is None:
        pytest.skip("node is not installed")
    done = subprocess.run([node, str(ROOT / "tests/data/hop_audio_sync_harness.mjs"), str(PLAY)],
                          capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stderr
    r = json.loads(done.stdout)
    assert r["before_start"] is True
    assert r["not_started_while_suspended"] == 0, "a synced loop waits for a running clock"
    assert r["sources"] == 1 and r["loop"] is True
    assert r["start_offset"] == 5 and r["start_at"] == pytest.approx(10.03)
    assert r["clock"] == pytest.approx(12 - 5.03 - 0.04), "clock = now - t0 - output latency"
    assert r["sources_after"] == 1
    assert r["stopped"] and r["clock_when_stopped"]
    assert r["wrapped_offset"] == pytest.approx(70 % 60.952)
