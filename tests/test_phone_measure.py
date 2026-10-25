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
    assert p["skip"][-1] == "all-on+skip"


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
    assert [s["name"] for s in steps] == ["all-off", "post", "atmo", "both", "all-on+skip"]
    assert all(s["n"] > 200 and s["grade"] == "good" for s in steps)
    # more picture work costs more CPU per frame, in this fake: off < one on < both on
    cpu = {s["name"]: s["cpu"] for s in steps}
    assert cpu["all-off"] < cpu["post"] == cpu["atmo"] < cpu["both"]
    assert r["switchLog"][:2] == ["fx_post=0", "fx_atmo=0"]
    assert r["restored"] == {"sw": {"fx_post": True, "fx_atmo": True}, "renderSkip": False}


def test_render_skip_halves_the_draws_on_a_120hz_screen_with_a_60hz_sim(r):
    s = {x["name"]: x for x in r["report"]["steps"]}
    assert s["both"]["rps"] == 120 and s["all-on+skip"]["rps"] == 60
    assert s["all-on+skip"]["cpu"] < s["both"]["cpu"]
    assert s["all-on+skip"]["p50"] == s["both"]["p50"], "the frame pacing itself is not changed"


def test_report_has_device_and_adapter_facts_and_nothing_personal(r):
    rep = r["report"]
    assert rep["tool"] == "kagra-perf" and rep["v"] == 1 and rep["hz"] == 120
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
