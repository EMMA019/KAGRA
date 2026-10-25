"""The ?dev=1 on-phone measurement tool (scripts/preview_site/perf.js, docs/phone_measure.md).

perf.js runs under node on a fake page with a virtual clock; the build tests check that the
tool reaches the preview site and never a portal zip, and that play.html only loads it for
?dev=1.
"""
from __future__ import annotations

import importlib.util
import json
import os
import re
import shutil
import subprocess
import zipfile
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "tests/data/perf_harness.mjs"
LIMIT_HARNESS = ROOT / "tests/data/limit_harness.mjs"
PLAY = ROOT / "scripts/preview_site/play.html"
PERF = ROOT / "scripts/preview_site/perf.js"


def _load(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


pp = _load("package_portal")
bbp = _load("build_browser_preview")


@pytest.fixture(scope="module")
def r():
    node = shutil.which("node")
    if node is None:
        pytest.skip("node is not installed")
    done = subprocess.run([node, str(HARNESS)], capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stderr
    return json.loads(done.stdout)


# ---- the numbers -------------------------------------------------------------------


def test_summary_of_a_steady_60_and_of_spikes(r):
    assert r["flat"] == {"fps": 60, "p50": 16.67, "p95": 16.67, "jank": 0, "drop": 0, "grade": "good"}
    assert r["spiky"]["jank"] == 3 and r["spiky"]["hitches"] == 1 and r["spiky"]["max"] == 70
    assert 0.5 < r["spiky"]["drop"] < 1.5
    assert r["slow"] == "poor"
    assert r["empty"]["n"] == 0


def test_refresh_estimate_snaps_to_common_rates_and_ignores_hitches(r):
    e = r["refresh"]
    assert (e["hz60"], e["hz90"], e["hz120"]) == (60, 90, 120)
    assert e["few"] is None, "three frames are not enough to say"
    assert e["withHitch"] == 120


def test_thermal_guess_needs_a_minute_and_reads_the_drift(r):
    t = r["thermal"]
    assert t["few"].startswith("n/a")
    assert t["flat"].startswith("stable")
    assert "maybe warming" in t["warm"]
    assert "likely throttling" in t["hot"]["label"] and t["hot"]["drift"] == 0.4
    assert t["faster"].startswith("faster")


def test_plan_is_all_off_post_atmo_both_then_the_skip_step(r):
    p = r["plan"]
    assert [x[0] for x in p["two"]] == ["all-off", "post", "atmo", "both"]
    assert p["two"][0][1] == {"fx_post": False, "fx_atmo": False}
    assert p["two"][1][1] == {"fx_post": True, "fx_atmo": False}
    assert p["ms"] == [15000] * 4, "four steps of 15 s are the 60 s"
    assert p["none"] == ["as-is"], "a game without switches gets one plain window"
    assert p["one"] == ["all-off", "post"]
    assert p["skip"][-1] == "both+skip"


def test_table_has_one_row_per_step(r):
    assert len(r["table"]) == 2 and r["table"][0].startswith("step") and "all-off" in r["table"][1]


# ---- the tool on a fake page --------------------------------------------------------


def test_overlay_shows_the_requested_facts(r):
    text = r["overlayText"]
    for needle in ("fps", "p50", "p95", "p99", "jank", "disp ~120 Hz", "webgpu", "scale cap", "1080x2424",
                   "dpr", "heap", "thermal", "tier mobile", "post:on", "atmo:on", "render_skip:off"):
        assert needle in text, needle
    assert r["refreshLive"]["hz"] == 120


def test_recording_runs_every_mix_once_and_puts_everything_back(r):
    assert r["recordStarted"] is True and r["recordTwice"] is False
    assert r["recordingAfter"] is False
    steps = r["report"]["steps"]
    assert [s["name"] for s in steps] == ["all-off", "post", "atmo", "both", "both+skip"]
    assert all(s["n"] > 200 and s["grade"] == "good" for s in steps)
    # more picture work costs more CPU per frame, in this fake: off < one on < both on
    cpu = {s["name"]: s["cpu"] for s in steps}
    assert cpu["all-off"] < cpu["post"] == cpu["atmo"] < cpu["both"]
    assert r["switchLog"][:2] == ["fx_post=0", "fx_atmo=0"]
    assert r["restored"] == {"sw": {"fx_post": True, "fx_atmo": True}, "renderSkip": False}


def test_render_skip_halves_the_draws_on_a_120hz_screen_with_a_60hz_sim(r):
    s = {x["name"]: x for x in r["report"]["steps"]}
    assert abs(s["both"]["rps"] - 120) <= 1 and abs(s["both+skip"]["rps"] - 60) <= 1
    assert s["both+skip"]["cpu"] < s["both"]["cpu"]
    assert s["both+skip"]["p50"] == s["both"]["p50"], "the frame pacing itself is not changed"


def test_report_has_device_and_adapter_facts_and_nothing_personal(r):
    rep = r["report"]
    assert rep["tool"] == "kagra-perf" and rep["v"] == 2 and rep["hz"] == 120
    assert rep["device"]["ua"].startswith("Mozilla/5.0") and rep["device"]["dpr"] == 2.63
    assert rep["adapter"]["webgpu"]["description"] == "Mali-G925"
    assert rep["canvas"]["w"] == 1080 and rep["quality"] == "mobile"
    # only the plain device keys; no URL, no storage, no ids
    assert set(rep["device"]) - {"uaHigh"} == {"ua", "platform", "mobile", "cores", "memGB", "touch", "dpr", "screen", "viewport"}
    assert rep["page"]["params"] == {"game": "hop_beat", "dev": "1", "quality": "mobile"}, "only whitelisted query keys"
    assert "location" not in json.dumps(rep).lower()


def test_copy_json_uses_the_clipboard_then_falls_back_to_the_text_area(r):
    c = r["copy"]
    assert c["btnAfter"] == {"rec": "Record again", "copyHidden": False}
    assert c["clipboard"] == "kagra-perf" and c["clipboardBytes"] < 4000
    assert c["hasNoUrl"] is True
    assert c["fallback"] == {"shown": True, "parses": "kagra-perf", "selected": True, "exec": 1}


def test_no_network_and_the_overlay_goes_away_cleanly(r):
    assert r["net"] == 0
    assert r["removed"] is True


def test_perf_js_has_no_network_or_storage_calls():
    src = PERF.read_text(encoding="utf-8")
    code = re.sub(r"/\*.*?\*/|//[^\n]*", "", src, flags=re.S)
    for bad in ("fetch(", "XMLHttpRequest", "sendBeacon", "WebSocket", "localStorage", "sessionStorage",
                "indexedDB", "document.cookie", "location.href", "http://", "https://", "import("):
        assert bad not in code, bad


# ---- where it is loaded and where it is not ------------------------------------------


def test_play_html_loads_perf_only_for_dev_and_not_in_a_portal_build():
    html = PLAY.read_text(encoding="utf-8")
    block = re.search(r"// <kagra-devtools>\n(.*?)// </kagra-devtools>\n", html, re.S)
    assert block, "the marker block is what package_portal.py cuts"
    body = block.group(1)
    assert 'if (dev && BUILD !== "portal")' in body and "perf.js" in body
    assert "perf.js" not in html.replace(block.group(0), ""), "no other place mentions or loads the tool"
    assert re.search(r'const dev = params\.get\("dev"\) === "1"', html)


def test_render_skip_is_off_by_default_and_never_skips_a_stepped_frame():
    html = PLAY.read_text(encoding="utf-8")
    assert 'let renderSkip = params.get("render_skip") === "1";' in html
    assert "const skipDraw = renderSkip && stepped === 0 && redraw <= 0;" in html
    assert "if (rendering && !skipDraw)" in html
    # a game that is not on the fixed 60 Hz sim steps every frame, so it never skips
    assert re.search(r"if \(!hopMode\) \{ session\.requestFrame\(\); return 1; \}", html)


def test_preview_site_gets_perf_js_and_a_portal_zip_does_not(tmp_path):
    assert "perf.js" in bbp.DEV_SCRIPTS and "perf.js" not in bbp.SITE_SCRIPTS
    out = tmp_path / "site"
    bbp.build_preview(ROOT, out, None, require_pkg=False)
    assert (out / "perf.js").is_file()
    assert "?v=" in (out / "play.html").read_text(encoding="utf-8")

    pkg = tmp_path / "pkg"
    pkg.mkdir()
    (pkg / "kagra_shared.js").write_text("export default function init() {}\n", encoding="utf-8")
    (pkg / "kagra_shared_bg.wasm").write_bytes(b"\0asm\1\0\0\0" + os.urandom(512))
    zout = tmp_path / "zip"
    assert pp.main(["--pkg", str(pkg), "--out", str(zout), "--keep-tree"]) == 0
    tree = zout / "neon_cat"
    assert not (tree / "perf.js").exists()
    page = (tree / "index.html").read_text(encoding="utf-8")
    for needle in ("perf.js", "KagraPerf", "kagraPerf", "kagra-devtools"):
        assert needle not in page, needle
    for z in zout.glob("*.zip"):
        with zipfile.ZipFile(z) as zf:
            assert "perf.js" not in zf.namelist()
    # the page still parses as one module script around the cut
    assert 'const dev = params.get("dev") === "1"' in page


def test_portal_cut_fails_loudly_if_the_block_goes_missing():
    src = PLAY.read_text(encoding="utf-8").replace("// <kagra-devtools>", "// <gone>")
    with pytest.raises(pp.PackageError, match="kagra-devtools"):
        pp.portal_index_html(src, "neon_cat", "none")


# ---- the robust recorder and "Find my limit" (tests/data/limit_harness.mjs) --------------------------


@pytest.fixture(scope="module")
def lim():
    node = shutil.which("node")
    if node is None:
        pytest.skip("node is not installed")
    done = subprocess.run([node, str(LIMIT_HARNESS)], capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stderr
    return json.loads(done.stdout)


def test_the_ladder_walks_tier_by_tier_one_knob_at_a_time(lim):
    assert lim["ladder3"] == ["safe", "scale 1.25", "scale 1.5", "bloom 1", "atmo 0.75", "particles 0.75[mid]", "scale 2",
                              "atmo 1", "particles 1", "shadows 2c[high]", "scale 2.5", "atmo 1.4", "shadows 3c", "ssao 4",
                              "soft shadows", "taa[max]"]
    assert "scale 2" not in lim["ladder1"], "no step above what a 1x display can show"
    assert lim["ladderFirst"]["tier"] == "safe" and lim["ladderFirst"]["over"] == {}


def test_suggested_tier_is_the_highest_one_the_last_good_step_covers(lim):
    s = lim["suggest"]
    assert s == {"safe": "safe", "mid": "mid", "high": "high", "max": "max", "midButNoBloom": "safe", "dprOne": "mid"}


def test_a_step_holds_only_with_p95_p99_and_enough_undisturbed_frames(lim):
    assert lim["pass"] == [True, False, False, False, False]


def test_the_recorder_keeps_a_run_going_and_reports_what_it_saw(lim):
    r = lim["rec1"]
    assert r["started"] is True
    steps = r["steps"]
    assert [x["name"] for x in steps] == ["all-off", "post", "atmo", "both"]
    assert all(x["bot"] is True and x["runs"] >= 2 and x["runEnds"] >= 1 and x["playPct"] >= 80 for x in steps), "the bot restarts the run"
    assert steps[0]["coverage"] == 1 and steps[0]["stalls"] == 0 and steps[0]["note"] is None
    # a 2.5 s stop in the second step is counted, not silently dropped
    st = steps[1]
    assert st["stalls"] == 1 and st["stallMs"] == 2500 and st["stalled"] is True and st["coverage"] < 0.75
    assert "drawing stopped" in st["note"]
    assert all(x["wallMs"] >= 6000 for x in steps), "a step is a wall time, not a frame count"
    assert r["log"][0] == "autoplay=1" and r["log"][-1] == "autoplay=0"
    assert r["sw"] == {"fx_post": True, "fx_atmo": True, "autoplay": False}, "the bot goes back off"
    assert r["limitInReport"] is None


def test_without_a_bot_the_note_says_nobody_was_playing(lim):
    n = lim["nobot"]
    assert all(b is False for b in n["bot"])
    assert all("no autoplay bot" in x for x in n["notes"])
    assert n["limitWithoutTiers"] is False, "no tiers, no ladder"


def test_find_my_limit_stops_at_the_first_miss_and_reports_the_last_good_step(lim):
    L = lim["limit"]
    assert L["probe"] == {"started": True, "again": False}
    assert [x[0] for x in L["steps"]] == ["safe", "scale 1.25", "scale 1.5", "bloom 1", "atmo 0.75", "particles 0.75", "scale 2"]
    assert [x[1] for x in L["steps"]] == [True] * 6 + [False]
    assert L["lastGood"] == "particles 0.75" and L["firstFail"] == "scale 2" and L["complete"] is False
    assert L["suggested"] == "mid"
    assert L["criteria"]["p95"] == 17.2 and L["criteria"]["p99"] == 20
    assert "limit" in L["keys"]


def test_find_my_limit_puts_everything_back(lim):
    L = lim["limit"]
    assert L["kq"][0] == "hold" and L["kq"][1] == "show safe" and L["kq"][-2:] == ["restore", "release"]
    assert L["holds"] == 0 and L["autoplay"] is False and L["recording"] is False
    assert L["switchLog"] == ["autoplay=1", "autoplay=0"]


def test_gpu_time_comes_from_the_webgl2_timer_or_the_report_says_why_not(lim):
    assert lim["limitGpu"]["gpu"]["available"] is True and lim["limitGpu"]["gpu"]["source"] == "EXT_disjoint_timer_query_webgl2"
    assert all(v == 4.5 for v in lim["limitGpu"]["gpuMs"])
    for k in ("limit", "limitStall", "limitWebgpu"):
        assert lim[k]["gpu"]["available"] is False and all(v is None for v in lim[k]["gpuMs"])
    assert "not offered" in lim["limit"]["gpu"]["why"]
    w = lim["limitWebgpu"]["gpu"]
    assert "timestamp-query" in w["why"] and "backend=gl" in w["why"] and w["webgpuTimestamp"] is True


def test_a_stalled_step_is_measured_again_before_it_counts(lim):
    S = lim["limitStall"]
    assert [x[1] for x in S["steps"]] == [True] * 6 + [False]
    assert S["steps"][1][3] == "second try after a stall"
    assert S["kq"].count("custom 1.25") == 2, "the stalled step ran twice"
    assert S["lastGood"] == "particles 0.75"


def test_a_game_without_tiers_has_no_limit_run(lim):
    N = lim["limitNoTiers"]
    assert N["probe"]["started"] is False and N["steps"] is None and N["kq"] == []
