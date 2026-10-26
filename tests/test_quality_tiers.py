"""Graphics tiers: the Python mirror (kagra/quality.py), the page logic (scripts/preview_site/quality.js
under node, tests/data/quality_harness.mjs) and where quality.js is and is not loaded.

The Rust side is tested in kagra-shared (quality.rs, session.rs, hop_beat.rs); the one tier file is
kagra-shared/src/quality_tiers.json.
"""
from __future__ import annotations

import copy
import importlib.util
import json
import os
import re
import shutil
import subprocess
import zipfile
from pathlib import Path

import pytest

from kagra import quality
from kagra.hop_beat import game_from_overrides

ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "tests/data/quality_harness.mjs"
PLAY = ROOT / "scripts/preview_site/play.html"
QUALITY_JS = ROOT / "scripts/preview_site/quality.js"
I18N_JS = ROOT / "scripts/preview_site/i18n.js"
TIER_JSON = ROOT / "kagra-shared/src/quality_tiers.json"
RUST = ROOT / "kagra-shared/src"


def _load(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


pp = _load("package_portal")
bbp = _load("build_browser_preview")


# ---- the Python mirror ----------------------------------------------------------------------------


def test_python_defaults_equal_the_json_rust_includes():
    assert quality.check() == []
    assert json.loads(TIER_JSON.read_text(encoding="utf-8")) == quality.default_spec()


def test_defaults_validate_and_scale_climbs_from_safe_to_max():
    spec = quality.default_spec()
    quality.validate(spec)
    scales = [spec["tiers"][n]["scale"] for n in quality.TIER_NAMES]
    assert scales == sorted(scales) and scales[0] == 1.0
    assert spec["auto"]["ceiling"] == "high", "max is never picked by auto"


def test_overrides_merge_key_by_key():
    spec = quality.resolve({"tiers": {"mid": {"scale": 1.25}}})
    assert spec["tiers"]["mid"]["scale"] == 1.25 and spec["tiers"]["mid"]["atmo"] == 0.75
    assert spec["tiers"]["high"] == quality.default_spec()["tiers"]["high"]
    fx = quality.resolve({"tiers": {"safe": {"fx": {"fx_atmo": False}}}})["tiers"]["safe"]["fx"]
    assert fx == {"fx_post": True, "fx_atmo": False}


@pytest.mark.parametrize(
    "bad",
    [
        {"tiers": {"mid": {"scale": 9.0}}},
        {"tiers": {"mid": {"sparkle": 1}}},
        {"tiers": {"ultra": {}}},
        {"tiers": {"mid": {"picture": "cinematic"}}},
        {"auto": {"ceiling": "huge"}},
        {"auto": {"up_secs": 0.1}},
        {"auto": {"nope": 1}},
        {"tiers": {"safe": {"scale": 2.0}}},
        {"tiers": {"mid": {"particles": 1.5}}},
        {"tiers": {"mid": {"taa": "yes"}}},
        {"tiers": {"mid": {"shadow_cascades": 4}}},
        {"tiers": {"mid": {"shadow_cascades": 1.5}}},
        {"tiers": {"mid": {"fx": {"fx_post": 1}}}},
        {"tiers": {"mid": {"scale": True}}},
        {"tiers": []},
    ],
)
def test_bad_overrides_are_refused_like_rust(bad):
    with pytest.raises(ValueError, match="quality_tiers"):
        quality.resolve(bad)


def test_a_manifest_game_may_carry_quality_tiers():
    game = game_from_overrides({"quality_tiers": {"tiers": {"mid": {"scale": 1.25}}}})
    assert "quality_tiers" not in game, "stripped before the kit's own merge, as the session does"
    with pytest.raises(ValueError, match="quality_tiers"):
        game_from_overrides({"quality_tiers": {"tiers": {"mid": {"scale": 99}}}})
    with pytest.raises(ValueError, match="unknown field"):
        game_from_overrides({"qualityTiers": {}})


def test_every_number_in_the_rust_validator_is_in_the_python_bounds():
    rs = (RUST / "quality.rs").read_text(encoding="utf-8")
    for key, lo, hi in quality.TIER_BOUNDS:
        pat = rf'num\(\s*&format!\("\{{name\}}\.{key}"\),\s*self\.{key}(?: as f32)?,\s*{float(lo)},\s*{float(hi)},?\s*\)'
        assert re.search(pat, rs), key
    for key, lo, hi in quality.AUTO_BOUNDS:
        assert re.search(rf'num\("auto\.{key}", a\.{key}, {float(lo)}, {float(hi)}\)', rs), key


def test_the_cli_check_passes():
    assert quality.main(["--check"]) == 0


# ---- the page logic under node ---------------------------------------------------------------------


@pytest.fixture(scope="module")
def q():
    node = shutil.which("node")
    if node is None:
        pytest.skip("node is not installed")
    done = subprocess.run([node, str(HARNESS)], capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stderr
    return json.loads(done.stdout)


def test_quality_word_aliases(q):
    a = q["alias"]
    assert a["auto"] == a["AUTO"] == "auto"
    assert [a[k] for k in ("safe", "mid", "high", "max")] == ["safe", "mid", "high", "max"]
    assert a["low"] == "safe" and a["mobile"] == "mid"
    assert a["ultra"] is None and a[""] is None and a["null"] is None and a["legacy"] is None


def test_window_stats_and_goodness(q):
    assert q["stats"] == {"n": 103, "p95": 16.7, "slowPct": 2.9}
    assert q["good"] == [True, False, False], "20 ms frames are not good; ten frames are too few to say"


def test_probe_climbs_to_the_ceiling_and_stops_at_the_first_failure(q):
    p = q["probe"]
    assert (p["allGood"]["tried"], p["allGood"]["done"]) == (["high"], "high"), "never past the ceiling"
    assert (p["highBad"]["tried"], p["highBad"]["done"]) == (["high"], "mid")
    assert (p["midBad"]["tried"], p["midBad"]["done"]) == (["safe"], "safe")
    assert p["allBad"]["done"] == "safe"
    assert p["allGood"]["at"] == 8, "settle 1 s + 3 s per window, two windows"
    assert p["stall"]["at"] > p["allGood"]["at"], "a stop restarts the window"


def test_guard_steps_down_on_a_slow_stretch_and_up_only_when_calm(q):
    g = q["guard"]
    assert g["steady"] == [] and g["threePct"] == [], "3% slow frames is not worth a step"
    assert g["tenPct"][0][0] == "down" and g["tenPct"][0][1] <= 9
    assert g["gap"] == [], "a one-off stop is not a slow stretch"
    assert g["upCalmOnly"] == [], "never in the middle of play"
    assert g["upCalm"] == [["up", 25]], "after up_secs of good frames at a calm moment"
    assert g["atFloor"] == [], "nothing below safe"
    assert q["guardBan"]["down"] <= 9 and q["guardBan"]["up"] - q["guardBan"]["down"] >= q["guardBan"]["banSecs"]


def test_setting_round_trip_and_bad_storage(q):
    s = q["setting"]
    assert s["roundTrip"] == {"mode": "auto", "tier": "high", "ts": 123}
    assert s["key"] == ["kagra.quality"]
    assert s["garbage"] is None and s["badMode"] is None and s["throwing"] is None and s["none"] is None
    assert s["badTier"] == {"mode": "auto", "tier": None, "ts": 1}


def test_a_game_without_tiers_is_left_alone(q):
    assert q["noTiers"] == {"active": False, "row": False, "applied": 0, "hasGlobal": False}


def test_quality_query_is_shown_for_the_visit_and_not_saved(q):
    a = q["asked"]
    assert a["applied"] == ["high"] and a["caps"] == [[2, "high"]] and a["saved"] == [] and a["afterTen"] == 1
    assert a["mode"] == "high" and a["label"] == "Graphics: High" and a["probing"] is False
    assert a["ahead"] is True, "the Graphics row sits before the sound row"
    assert q["askedOld"] == ["mid", "safe", "auto"], "mobile, low, and an unknown word"


def test_first_run_probes_then_trusts_the_result_for_a_week(q):
    f = q["first"]
    assert f["start"] == ["mid"] and f["probing"] is True and f["label0"] == "Graphics: Auto (Mid)"
    assert f["tier"] == "high" and f["probing2"] is False and f["saved"]["mode"] == "auto" and f["saved"]["tier"] == "high"
    assert f["caps"][:2] == [1.5, 2]
    assert q["second"]["applied"] == ["high"] and q["second"]["probing"] is False
    assert q["stale"]["probing"] is True and q["stale"]["applied"] == ["mid"]


def test_probe_waits_for_a_run_and_a_slow_phone_lands_on_safe(q):
    w = q["probeWaits"]
    assert w["waiting"] == {"applied": ["mid"], "probing": True} and w["after"][-1] == "high"
    s = q["slow"]
    assert s["tier"] == "safe" and s["saved"] == "safe" and s["probing"] is False


def test_guard_in_the_page_steps_down_saves_and_ignores_a_hidden_tab(q):
    g = q["guardPage"]
    assert g["after"] == ["high", "mid"] and g["tier"] == "mid" and g["saved"] == "mid"
    assert g["changes"] == ["auto", "guard-down"]
    assert q["hidden"]["applied"] == ["high"]


def test_graphics_button_cycles_auto_and_the_four_tiers_and_manual_stays_put(q):
    c = q["cycle"]
    assert c["seen"] == ["Graphics: Auto (High)", "Graphics: Safe", "Graphics: Mid", "Graphics: High", "Graphics: Max",
                         "Graphics: Auto (High)", "Graphics: Safe"]
    assert c["saved"]["mode"] == "safe"
    assert q["manualFixed"] == {"applied": ["safe"], "tier": "safe"}


def test_hold_pauses_the_guard_and_custom_show_restore_work_for_the_recorder(q):
    h = q["hold"]
    assert h["heldApplied"] == ["mid"], "nothing moved while held"
    assert h["afterCustom"]["tier"] == "custom" and h["afterCustom"]["customs"] == [{"scale": 1.25, "atmo": 0.5}]
    assert h["afterCustom"]["cap"] == [1.25, "custom"]
    assert h["afterShow"] == {"tier": "safe", "cap": [1, "safe"]}
    assert h["restored"] == "mid" and h["showUnknown"] is False


def test_no_network_calls(q):
    assert q["net"] == 0


def test_quality_js_has_no_network_calls_and_one_storage_key():
    src = QUALITY_JS.read_text(encoding="utf-8")
    code = re.sub(r"/\*.*?\*/|//[^\n]*", "", src, flags=re.S)
    for bad in ("fetch(", "XMLHttpRequest", "sendBeacon", "WebSocket", "sessionStorage", "indexedDB", "document.cookie",
                "http://", "https://", "import(", "eval("):
        assert bad not in code, bad
    assert code.count("localStorage") == 1
    assert len(re.findall(r'"kagra\.[a-z]+"', code)) == 1


# ---- where quality.js is loaded ------------------------------------------------------------------------


def test_play_html_loads_quality_js_before_the_module_and_keeps_the_meta_hooks():
    html = PLAY.read_text(encoding="utf-8")
    assert html.count('<script src="./quality.js"></script>') == 1
    assert html.index("./i18n.js") < html.index("./quality.js") < html.index('<script type="module">')
    for n in (1, 2, 3, 4):
        assert f"META hook {n}/4" in html, n
    block = re.search(r"// <kagra-devtools>\n(.*?)// </kagra-devtools>\n", html, re.S)
    assert block, "the portal cut still finds its block"
    assert "quality.js" not in block.group(1)
    assert "KagraQuality?.start(" in html and "setScaleCap" in html
    # the page's own ladder stays below the tier's cap, and the cap starts at the old 2
    assert "const effectiveCap" in html and "let tierCap = SCALE_STEPS[0];" in html
    # the tier calls come after the ladder's variables (no temporal dead zone)
    assert html.index("let avgGap") < html.index("KagraQuality?.start(")


def test_gfx_strings_exist_in_both_languages_with_the_same_keys():
    src = I18N_JS.read_text(encoding="utf-8")
    keys = {k for k in re.findall(r'"(gfx\.[a-z.]+)":', src)}
    assert keys == {"gfx.label", "gfx.auto", "gfx.auto.now", "gfx.safe", "gfx.mid", "gfx.high", "gfx.max", "gfx.hint"}
    assert len(re.findall(r'"gfx\.label":', src)) == 2


def test_quality_js_is_a_site_script_in_the_preview_and_in_a_portal_zip(tmp_path):
    assert "quality.js" in bbp.SITE_SCRIPTS and "quality.js" not in bbp.DEV_SCRIPTS
    out = tmp_path / "site"
    bbp.build_preview(ROOT, out, None, require_pkg=False)
    assert (out / "quality.js").is_file()
    pkg = tmp_path / "pkg"
    pkg.mkdir()
    (pkg / "kagra_shared.js").write_text("export default function init() {}\n", encoding="utf-8")
    (pkg / "kagra_shared_bg.wasm").write_bytes(b"\0asm\1\0\0\0" + os.urandom(512))
    zout = tmp_path / "zip"
    assert pp.main(["--pkg", str(pkg), "--out", str(zout), "--keep-tree"]) == 0
    assert (zout / "neon_cat" / "quality.js").is_file()
    assert 'src="./quality.js"' in (zout / "neon_cat" / "index.html").read_text(encoding="utf-8")
    for z in zout.glob("*.zip"):
        with zipfile.ZipFile(z) as zf:
            assert "quality.js" in zf.namelist()


def test_the_copy_of_the_spec_is_not_shared_between_calls():
    a = quality.default_spec()
    b = copy.deepcopy(a)
    a["tiers"]["mid"]["scale"] = 3.0
    assert quality.default_spec() == b
