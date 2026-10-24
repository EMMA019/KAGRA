"""Neon Cat's page side: Web Audio playback, mission cards, and the generated placeholder assets."""
from __future__ import annotations

import json
import shutil
import struct
import subprocess
import sys
import wave
from pathlib import Path

import pytest

from kagra.game_spec import SFX_CUES, GameSpec

ROOT = Path(__file__).resolve().parents[1]
PLAY = ROOT / "scripts/preview_site/play.html"
HARNESS = ROOT / "tests/data/page_audio_harness.mjs"


@pytest.fixture(scope="module")
def page():
    node = shutil.which("node")
    if node is None:
        pytest.skip("node is not installed")
    done = subprocess.run([node, str(HARNESS), str(PLAY)], capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stderr
    return json.loads(done.stdout)


def test_nothing_plays_before_the_first_touch_but_files_are_fetched(page):
    b = page["before_unlock"]
    assert b["contexts"] == 0 and b["sources"] == 0
    assert b["fetched"] == ["a/bgm.wav", "a/coin.mp3", "a/hit.ogg", "a/missing.wav"]


def test_first_touch_opens_the_output_and_decodes_the_files(page):
    a = page["after_unlock"]
    assert a["contexts"] == 1 and a["state"] == "running" and a["resumes"] == 1
    assert a["primed"] == 1, "a silent buffer wakes the iOS audio route"
    assert a["loaded"] == 3 and a["expected"] == 4, "a 404 costs one file, not the page"
    assert page["contexts_after_second_unlock"] == 1


def test_cues_play_their_own_file_rate_and_volume(page):
    coin, hit = page["shots"]
    assert (coin["file"], coin["rate"], coin["gain"]) == ("a/coin.mp3", 1.5, 0.6)
    assert (hit["file"], hit["rate"], hit["gain"]) == ("a/hit.ogg", 1.0, 1.0)
    assert coin["toSfx"] and hit["toSfx"]
    assert len(page["shots"]) == 2, "an unknown cue, an empty file and a missing file stay silent"
    assert page["group_gains"] == {"master": 0.8, "bgm": 0.0, "sfx": 0.5}
    assert page["same_instant_dedup"] == 0 and page["later_again"] == 1


def test_bgm_loops_once_follows_the_fade_and_stops(page):
    bgm = page["bgm"]
    assert bgm["sources"] == 1 and bgm["buffer"] == "a/bgm.wav" and bgm["started"] == 1
    # bgm_volume 0.5 * bgm.volume 0.5 * the mixer's fade gain.
    assert bgm["target"] == [0.125, 0.25]
    assert page["bgm_stopped"] == 1 and page["bgm_restart"] == 2


def test_mute_and_off_switches(page):
    assert page["muted_master"] == 0 and page["muted_shots"] == 0
    assert page["unmuted_master"] == 0.8 and page["unmuted_shots"] == 1
    assert page["disabled"] == {"usable": False, "contexts": 0, "fetched": 0}
    assert page["no_ctor"] == {"usable": False}
    assert page["visibility"] == ["suspended", "running"]


def test_mission_wording(page):
    assert page["goal_text"] == ["コインを40枚あつめる", "ゲートを5回くぐる", "高いところに1回のる", "TUNA 3"]


def test_mission_wording_in_english(page):
    assert page["goal_text_en"] == ["Collect 40 coins", "Pass 5 gates", "Perch up high 1 time(s)"]


def test_page_wires_audio_missions_and_best():
    html = PLAY.read_text(encoding="utf-8")
    assert "session.audioCuesJson()" in html
    for event in ("touchend", "pointerdown", "keydown"):
        assert f'"{event}"' in html
    assert "visibilitychange" in html and 'id="sound"' in html
    # The best score goes in before loadGame and comes out at the result.
    assert html.index("world.best") < html.index("session.loadGame(payload)")
    assert "saveBest(slug" in html and "loadBest(slug)" in html
    assert 'id="goalsTitle"' in html and 'id="runGoals"' in html
    # Only a spec with `audio` makes sound; the other genres stay as they were.
    assert 'world.genre === "neon_cat"' in html and "createAudio(audioSpec" in html


def test_preview_stages_every_audio_format():
    src = (ROOT / "scripts/build_browser_preview.py").read_text(encoding="utf-8")
    assert '".wav", ".ogg", ".mp3"' in src
    manifest = json.loads((ROOT / "games/neon_cat_minimal.json").read_text(encoding="utf-8"))
    for f in (GameSpec().atmosphere.wall.texture_high, GameSpec().atmosphere.wall.texture_mobile):
        assert f in manifest["bundle_assets"], f
    assert "assets/audio" in manifest["bundle_assets"]


# --- the generated placeholder assets ---------------------------------------


def _gen(script: str, *args: str):
    return subprocess.run(
        [sys.executable, str(ROOT / "tools/gen_assets" / script), *args],
        capture_output=True,
        text=True,
        timeout=120,
    )


@pytest.mark.parametrize("script", ["cat_audio.py", "cat_wall.py"])
def test_generated_assets_match_their_generators(script):
    done = _gen(script, "--check")
    assert done.returncode == 0, done.stdout + done.stderr


def test_every_default_cue_has_a_clean_short_wav():
    audio = GameSpec().audio
    assert set(audio.sfx) == set(SFX_CUES)
    for name, cue in audio.sfx.items():
        path = ROOT / cue.file
        with wave.open(str(path), "rb") as w:
            assert w.getnchannels() == 1 and w.getsampwidth() == 2, name
            frames = w.readframes(w.getnframes())
            secs = w.getnframes() / w.getframerate()
        samples = struct.unpack(f"<{len(frames) // 2}h", frames)
        assert 0.05 < secs < 1.6, f"{name} lasts {secs:.2f}s"
        assert max(abs(s) for s in samples) > 3000, f"{name} is nearly silent"
        assert max(abs(s) for s in samples) < 32000, f"{name} clips"
        assert abs(samples[-1]) < 40, f"{name} ends on a click"
    with wave.open(str(ROOT / audio.bgm.file), "rb") as w:
        secs = w.getnframes() / w.getframerate()
        frames = w.readframes(w.getnframes())
    assert 4 < secs < 30
    samples = struct.unpack(f"<{len(frames) // 2}h", frames)
    # A loop with a seam is a click every few seconds.
    assert abs(samples[0] - samples[-1]) < 1500


def test_wall_texture_is_a_png_with_power_of_two_free_ratio():
    for name, size in (("concrete_wall.png", (320, 276)), ("concrete_wall_s.png", (160, 138))):
        data = (ROOT / "assets/textures/neon_cat" / name).read_bytes()
        assert data[:8] == b"\x89PNG\r\n\x1a\n"
        width, height = struct.unpack(">II", data[16:24])
        assert (width, height) == size
