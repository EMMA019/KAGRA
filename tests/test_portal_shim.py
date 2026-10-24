"""The portal host shim (scripts/preview_site/portal.js) and the page strings (i18n.js), run under node."""
from __future__ import annotations

import json
import re
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "tests/data/portal_harness.mjs"
PLAY = ROOT / "scripts/preview_site/play.html"
PORTAL_JS = ROOT / "scripts/preview_site/portal.js"


@pytest.fixture(scope="module")
def r():
    node = shutil.which("node")
    if node is None:
        pytest.skip("node is not installed")
    done = subprocess.run([node, str(HARNESS)], capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stderr
    return json.loads(done.stdout)


# ---- which portal ----------------------------------------------------------------


def test_default_build_selects_no_portal_and_requests_nothing(r):
    assert r["pick"]["default"] == "none" and r["pick"]["junk"] == "none" and r["pick"]["empty_none"] == "none"
    n = r["none"]
    assert n["status"] == "none" and n["scripts"] == 0
    assert n["res"] == {"shown": False, "completed": False, "reason": "no_portal"}
    assert n["rw"]["rewarded"] is False and n["offer"] is False
    assert r["none_from_page"] == {"name": "none", "scripts": 0}


def test_query_selects_a_portal_and_the_sdk_loads_only_on_init(r):
    assert r["pick"]["query"] == "crazygames" and r["pick"]["query_gm"] == "gamemonetize"
    q = r["query_selects"]
    assert q["name"] == "crazygames" and q["scriptsBeforeInit"] == 0
    assert q["scriptUrl"] == "https://sdk.crazygames.com/crazygames-sdk-v3.js"


def test_build_flag_wins_over_the_query(r):
    assert r["pick"]["meta_wins"] == "crazygames"
    assert r["pick"]["meta_none_defers"] == "crazygames", "a none flag leaves the query in charge"


# ---- ads spec --------------------------------------------------------------------


def test_ads_spec_is_normalised_without_throwing(r):
    assert r["ads_default"] == {"commercial_on": ["retry"], "rewarded": [], "min_interval_secs": 90}
    assert r["ads_junk"] == r["ads_default"]
    clean = r["ads_clean"]
    assert clean["commercial_on"] == ["retry", "game_over"]
    assert clean["min_interval_secs"] == 3600
    assert clean["rewarded"] == [
        {"slot": "revive", "at": "title", "effect": "none", "per_run": 9},
        {"slot": "x", "at": "game_over", "effect": "none", "per_run": 1},
    ]


# ---- CrazyGames -------------------------------------------------------------------


def test_crazygames_events_are_deduplicated_and_focus_does_not_stop_gameplay(r):
    assert r["cg_before_load"]["status"] == "loading"
    assert r["cg_before_load"]["url"] == "https://sdk.crazygames.com/crazygames-sdk-v3.js"
    e = r["cg_events"]
    assert e["status"] == "ready"
    assert e["calls"] == [
        "init", "loadingStart", "loadingStop",
        "gameplayStart",            # twice asked, once sent; the tab-focus pause sent nothing
        "gameplayStop", "gameplayStart",  # the player's own pause and resume
        "gameplayStop",
    ]


def test_midgame_ad_blocks_the_ui_stops_gameplay_and_honours_the_interval(r):
    m = r["cg_midgame"]
    assert m["off"]["reason"] == "slot_off", "game_over is not in commercial_on"
    assert m["during"] == {"busy": True, "active": True, "playing": False,
                           "again": {"shown": False, "completed": False, "reason": "busy"}}
    assert m["res"] == {"shown": True, "completed": True, "reason": "finished"}
    assert m["after"] == {"busy": False, "active": False, "playing": True}
    assert m["soon"]["reason"] == "too_soon"
    assert m["later"] is True
    assert m["calls"][:4] == ["init", "gameplayStart", "gameplayStop", "requestAd:midgame"]
    assert m["seen"][:4] == ["busy:true", "ad:true:midgame", "ad:false:midgame", "busy:false"]


def test_a_failed_ad_never_blocks_the_game(r):
    e = r["cg_error"]
    assert e["res"]["shown"] is False and e["res"]["reason"] == "no fill"
    assert e["busy"] is False and e["active"] is False
    assert e["playing"] is True, "gameplay start is sent again once the failed ad is over"
    assert r["cg_timeout"] == {"mid": True, "res": {"shown": False, "completed": False, "reason": "timeout"}, "busy": False}
    stuck = r["cg_stuck"]
    assert stuck["res"]["reason"] == "stuck" and stuck["busy"] is False and stuck["active"] is False


def test_rewarded_ad_pays_only_after_the_ad_finishes(r):
    w = r["cg_rewarded"]
    assert w["offered"] is True
    assert w["unknown"]["rewarded"] is False and w["unknown"]["reason"] == "not_offered"
    assert w["first"]["rewarded"] is True
    assert w["offeredAfter"] is False, "per_run is 1"
    assert w["offeredNewRun"] is True
    assert w["calls"] == ["init", "requestAd:rewarded"]
    e = r["cg_rewarded_error"]
    assert e["r"]["rewarded"] is False and e["offeredAgain"] is True


def test_a_blocked_or_broken_sdk_leaves_the_game_running(r):
    assert r["cg_blocked"] == {"status": "unavailable",
                               "res": {"shown": False, "completed": False, "reason": "no_portal"}}
    assert r["cg_hang"]["status"] == "unavailable" and r["cg_hang"]["res"]["reason"] == "no_portal"
    assert r["cg_no_global"]["status"] == "unavailable"


# ---- other portals ---------------------------------------------------------------------


def test_gamemonetize_adapter(r):
    assert r["gm_options"] == {"gameId": "abc123", "hasEvent": "function", "url": "https://api.gamemonetize.com/sdk.js"}
    g = r["gm"]
    assert g["status"] == "ready" and g["banner"] is True
    assert g["mid"] == {"busy": True, "active": True}
    assert g["res"]["completed"] is True and g["seen"] == [True, False]
    assert g["offer"] is False and g["rew"]["rewarded"] is False, "GameMonetize has no rewarded API"


def test_stub_provider_logs_and_never_loads_a_script(r):
    s = r["stub"]
    assert s["name"] == "poki" and s["scripts"] == 0
    assert s["res"]["shown"] is False
    assert s["log"][0] == "[portal:poki] loadingStart" and "[portal:poki] gameplayStart" in s["log"]


# ---- page glue -------------------------------------------------------------------------


def test_page_pauses_when_hidden_or_blurred_while_playing_only(r):
    p = r["page_pause"]
    assert p["title"] == {"log": [], "paused": False}, "nothing to pause on the title"
    assert p["hidden"]["paused"] is True and p["hidden"]["log"] == ["pause"]
    assert p["hidden"]["audio"] == ["hidden:true"]
    assert p["hidden"]["calls"] == p["startCalls"], "CrazyGames is not told about a focus pause"
    assert p["back"]["paused"] is True, "coming back does not resume by itself"
    assert p["resumed"] == {"paused": False, "log": ["pause", "resume"],
                            "audio": ["hidden:true", "hidden:false"], "cardVisible": False}
    assert p["blur"] is True
    assert p["afterEsc"] == {"paused": False, "prevented": True}
    assert p["afterP"] is True and p["afterP2"] is False
    assert p["calls"][-2:] == ["gameplayStop", "gameplayStart"], "an explicit pause is reported"


def test_page_blocks_scroll_and_gestures_but_not_game_keys(r):
    assert r["page_guards"] == {"wheel": True, "pagedown": True, "context": True, "gesture": True,
                                "arrowUntouched": True}


def test_retry_waits_for_the_midgame_ad_and_ignores_extra_taps(r):
    p = r["page_retry"]
    assert p["resultAdBefore"] == 0 and p["gameOverAd"] == 1, "the result card comes first, the ad a beat later"
    mid = p["midRetry"]
    assert mid["blocked"] is True and mid["went"] == 0 and mid["adCard"] is True
    assert p["wentAfter"] == 1, "one retry, not two"
    assert p["blocked"] is False


def test_ad_mutes_and_pauses_then_restores_the_players_choice(r):
    a = r["page_ad_playing"]
    assert a["during"]["muted"] is True and a["during"]["session"] == ["pause"] and a["during"]["paused"] is True
    assert a["session"] == ["pause", "resume"] and a["muted"] is True and a["paused"] is False
    u = r["page_ad_unmuted"]
    assert u == {"during": True, "after": False}
    assert r["page_start_title"] == 1


# ---- language --------------------------------------------------------------------------


def test_language_default_is_english_and_japanese_is_asked_or_detected(r):
    lang = r["lang"]
    assert lang["default_en"] == "en" and lang["none"] == "en" and lang["junk"] == "en"
    assert lang["ja_nav"] == "ja" and lang["ja_second"] == "en"
    assert lang["query_ja"] == "ja" and lang["query_en_over_nav"] == "en"


def test_string_tables_match(r):
    i = r["i18n"]
    assert i["only_ja"] == []
    # English-only keys are the death lines: Japanese reads the spec's own text.
    assert i["only_en"] and all(k.startswith("death.") for k in i["only_en"])
    assert i["placeholders_match"] and i["empty"] == []
    assert i["vars"][0] == "Collect 40 coins" and "40" in i["vars"][1]
    assert i["unknown"] == "no.such.key"
    assert i["death_en"] and i["death_ja"] is None
    assert i["install"] == {"lang": "ja", "winLang": "ja", "applied": ["はじめる", "一覧"], "htmlLang": "ja"}


# ---- the page file itself -----------------------------------------------------------------


def test_play_html_loads_the_shim_and_keeps_dev_tools_behind_dev():
    html = PLAY.read_text(encoding="utf-8")
    assert html.index('src="./i18n.js"') < html.index('src="./portal.js"') < html.index('<script type="module">')
    assert 'const dev = params.get("dev") === "1";' in html
    assert 'dev ? (params.get("backend") || "auto") : "auto"' in html
    assert 'dev && params.get("debug") === "1"' in html
    assert re.search(r'params\.get\("debug"\)', html).start() > html.index("const dev =")
    for hook in ("portalPage.phase(phase)", "portalPage.start(from", "portalHost.loadingFinished()"):
        assert hook in html


def test_every_string_key_the_page_uses_exists_in_both_languages():
    node = shutil.which("node")
    if node is None:
        pytest.skip("node is not installed")
    script = (
        "const fs=require('fs');const I=require('./scripts/preview_site/i18n.js');"
        "const h=fs.readFileSync('scripts/preview_site/play.html','utf8');"
        "const keys=new Set([...h.matchAll(/\\bt\\(\"([\\w.]+)\"/g)].map(m=>m[1]));"
        "for(const m of h.matchAll(/data-i18n=\"([\\w.]+)\"/g))keys.add(m[1]);"
        "const miss=[];for(const k of keys)for(const l of ['en','ja'])"
        "if(!Object.prototype.hasOwnProperty.call(I.TABLE[l],k))miss.push(l+':'+k);"
        "console.log(JSON.stringify({n:keys.size,miss}));"
    )
    done = subprocess.run([node, "-e", script], cwd=ROOT, capture_output=True, text=True, timeout=30)
    assert done.returncode == 0, done.stderr
    got = json.loads(done.stdout)
    assert got["n"] >= 25 and got["miss"] == []


def test_portal_js_names_only_the_two_sdk_urls():
    text = PORTAL_JS.read_text(encoding="utf-8")
    urls = set(re.findall(r"https?://[^\s\"'<>)\\`]+", text))
    assert urls == {"https://sdk.crazygames.com/crazygames-sdk-v3.js", "https://api.gamemonetize.com/sdk.js"}
