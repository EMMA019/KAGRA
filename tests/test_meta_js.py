"""scripts/preview_site/meta.js under node: the shared vectors, the wallet, the shop, the daily run, bad saves."""
from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

import pytest

from kagra import meta_spec as m
from kagra.distribute import load_manifest

ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "tests/data/meta_harness.mjs"
VECTORS = ROOT / "kagra-shared/tests/fixtures/meta_vectors.json"
D0 = 20751  # 2026-10-25 UTC; the harness clock starts at 05:00 that day


@pytest.fixture(scope="module")
def cfg():
    return load_manifest(ROOT / "games/hop_beat_minimal.json", ROOT).meta


@pytest.fixture(scope="module")
def r(cfg, tmp_path_factory):
    node = shutil.which("node")
    if node is None:
        pytest.skip("node is not installed")
    path = tmp_path_factory.mktemp("meta") / "cfg.json"
    path.write_text(json.dumps(cfg), encoding="utf-8")
    done = subprocess.run([node, str(HARNESS), str(path), str(VECTORS)], capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stderr
    return json.loads(done.stdout)


def coins(cfg, score, combo, perfect):
    return m.earn(cfg["earn"], {"score": score, "combo": combo, "perfect": perfect})


def test_js_replays_every_shared_vector(r):
    assert r["vector_mismatches"] == []
    assert all(n > 0 for n in r["vector_counts"].values())


def test_no_meta_block_means_a_game_that_is_unchanged(r):
    assert r["inert"] == {"enabled": False, "same": True, "items": 0}


def test_wallet_pays_once_per_run_and_only_the_difference_after_a_continue(r, cfg):
    w = r["wallet"]
    first = coins(cfg, 1234, 40, 10)
    cont = coins(cfg, 2000, 55, 20)
    second = coins(cfg, 500, 5, 2)
    assert w["start"] == 0 and w["run1"] == first
    assert w["run1_award"]["coins"] == first and w["run1_award"]["daily"] is False
    assert w["after_revive"] == cont and w["runs_after_revive"] == 1   # same run, not a second one
    assert w["same_again"] == cont                                      # drawing the card again pays nothing
    assert w["run2"] == cont + second and w["runs"] == 2
    assert w["best_score"] == 2000
    assert w["zero"] == w["run2"]                                       # a run that scored nothing
    assert w["saved"] == w["run2"] == w["reloaded"]


def test_shop_rules_buy_equip_and_prepare(r, cfg):
    s = r["shop"]
    free = {c["id"] for c in cfg["catalog"] if c["price"] == 0 and not c["requires"]}
    assert {k for k, v in s["initial"].items() if v == "equipped"} == free
    assert s["initial"]["hero.mint"] == "poor"       # no requirement, no money
    assert s["initial"]["hero.berry"] == "locked"    # needs runs
    assert s["poor"] == {"ok": False, "reason": "poor"} and s["locked"] == {"ok": False, "reason": "locked"}
    assert s["runs"] == 12 and s["coins"] == 12 * coins(cfg, 1000, 20, 10)
    assert s["after_runs"]["hero.berry"] == "buyable"   # 12 runs >= 5
    assert s["after_runs"]["hero.ice"] == "buyable"     # best combo 20 >= 16
    assert s["after_runs"]["hero.gold"] == "locked"     # needs a 3-day streak
    assert s["after_runs"]["look.mono"] == "locked"     # needs a score of 1500
    assert s["buy"] == {"ok": True} and s["spent"] == 100
    assert s["buy_again"]["reason"] == "owned" and s["equip_unowned"]["reason"] == "not_owned"
    assert s["hero"] == [120, 255, 190] and s["slab_untouched"] == [58, 52, 92]
    assert s["seed_free"] == 7 and s["world_not_mutated"] == [255, 214, 120]


def test_daily_seed_streak_best_and_the_day_rollover(r, cfg):
    d = r["daily"]
    assert d["mode"] == "daily" and d["seed"] == d["expect_seed"] == m.daily_seed("hop_beat", D0)
    assert d["info"]["date"] == "2026-10-25" and d["info"]["modifier"]["id"] in {x["id"] for x in cfg["modifiers"]}
    bonus1 = m.streak_reward(cfg["daily"]["streak_reward"], 1)
    assert d["day1"]["streak"] == 1 and d["day1"]["best"] == 1500 and d["day1"]["played"] == 1
    assert d["day1"]["award"]["bonus"] == bonus1 and d["day1"]["award"]["daily"] is True
    assert d["day1"]["coins"] == coins(cfg, 1500, 30, 12) + bonus1
    # a second, worse run the same day: best stays, streak and bonus do not move
    assert d["day1b"]["best"] == 1500 and d["day1b"]["streak"] == 1 and d["day1b"]["played"] == 1
    assert d["day1b"]["today"]["runs"] == 2
    # next day: a different seed, a longer streak, a bigger bonus, a separate best for today
    assert d["next_day_seed_differs"] is True
    assert d["day2"] == {"streak": 2, "best_today": 800, "best_all": 1500,
                         "bonus": m.streak_reward(cfg["daily"]["streak_reward"], 2)}
    # a missed day: the streak shows 0 and the next daily starts again at 1
    assert d["info_after_gap"] == 0
    assert d["day5"]["streak"] == 1 and d["day5"]["bonus"] == bonus1 and d["day5"]["best_streak"] == 2
    # the page stayed open past midnight: that run is not the daily any more
    assert d["stale"] == {"streak": True, "info_stale": True, "daily_best_unchanged": True}
    assert d["share"].startswith("HOP-BEAT score 5000 combo 90")
    # the modifier changes only what the kit listed
    assert d["mod_audio_untouched"] == 1 and d["lives_untouched"] == 3


def test_share_card_text_and_clipboard(r):
    s = r["share"]
    assert s["ok"] is True
    assert s["text"].splitlines() == ["HOP-BEAT Daily 2026-10-30 score 1234 combo 40", "https://example.invalid/play"]
    assert s["written"][0] == s["text"]
    assert s["free_text"].splitlines()[0] == "HOP-BEAT score 77 combo 3"
    assert s["denied_ok"] is False and "score 50" in s["denied_text"]   # a refused clipboard hands back the text
    assert s["ja"].startswith("HOP-BEAT スコア 10 コンボ 1")


def test_double_coins_is_optional_once_per_run_and_paid_only_after_the_ad(r, cfg):
    w = r["rewarded"]
    base = coins(cfg, 1234, 40, 10)
    assert w["c0"] == base and w["can1"] is True
    assert w["ok"] is True and w["c1"] == 2 * base
    assert w["can2"] is False and w["again"] is False            # once per run
    assert w["refused"] is False and w["refused_coins"] is True  # no ad finished, no pay, run coins stay
    assert w["none_can"] is False                                # a portal without rewarded: no button
    assert ["rw", "double_coins"] in w["calls"] and all(c[1] == "double_coins" for c in w["calls"])


@pytest.mark.parametrize("name", [
    "garbage", "empty", "array", "number", "nulls", "strings", "negative", "nan_like",
    "unknown_ids", "owned_list_v0", "future", "proto", "bad_daily",
])
def test_bad_saves_never_crash_and_end_up_in_a_valid_state(r, cfg, name):
    c = r["corrupt"][name]
    assert c["threw"] is None
    assert c["v"] == 1 and c["mode"] in {"free", "daily"}
    assert set(c["equipped"]) == {"hero", "look"}
    assert all(c["equipped"][s] in c["owned"] for s in c["equipped"])
    assert c["coins"] >= 0 and 0 <= c["best_score"] <= 1_000_000_000


def test_bad_saves_keep_what_is_good_and_drop_what_is_not(r, cfg):
    c = r["corrupt"]
    one_run = coins(cfg, 100, 3, 1)
    assert c["garbage"]["coins"] == one_run and c["garbage"]["bad_kept"] is True   # a copy is kept for support
    assert c["empty"]["bad_kept"] is False
    assert c["unknown_ids"]["owned"] == ["hero.mint", "hero.sun", "look.neon"]     # unknown and non-true ids dropped
    assert c["unknown_ids"]["equipped"]["hero"] == "hero.sun"                      # equipping an unowned item is undone
    assert c["owned_list_v0"]["coins"] == 40 + one_run and c["owned_list_v0"]["equipped"]["hero"] == "hero.mint"
    assert set(c["owned_list_v0"]["owned"]) >= {"hero.mint", "look.sunset"}        # the old list shape is migrated
    assert c["future"]["coins"] == 12 + one_run and c["future"]["v"] == 1          # a newer save is read, not rejected
    assert c["negative"]["coins"] == one_run                                       # negatives clamp to zero
    assert c["nan_like"]["runs"] == 8                                              # "7" read as 7, then this run
    assert c["polluted"] is True                                                   # no prototype pollution
    assert c["hostile"]["coins"] == c["nostore"]["coins"] == coins(cfg, 1000, 10, 5)
    assert c["hostile"]["persistent"] is False and c["nostore"]["persistent"] is False


def test_a_patch_outside_the_kits_allowlist_is_ignored_at_runtime_too(r):
    assert r["guard"] == {"audio": 1, "hero": [255, 214, 120]}
