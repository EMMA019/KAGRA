"""kagra/meta_spec.py: the `meta` block of a manifest, the pure rules, and how they reach the bundle."""
from __future__ import annotations

import copy
import importlib.util
import json
import os
import subprocess
import sys
import zipfile
from pathlib import Path

import pytest

from kagra import meta_spec as m
from kagra.distribute import ManifestError, load_manifest

ROOT = Path(__file__).resolve().parents[1]
VECTORS = ROOT / "kagra-shared/tests/fixtures/meta_vectors.json"
HOP_MANIFEST = ROOT / "games/hop_beat_minimal.json"


def _load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


@pytest.fixture(scope="module")
def vectors():
    return json.loads(VECTORS.read_text(encoding="utf-8"))


def good():
    return {
        "patchable": ["look", "course.ramp_bars"],
        "catalog": [
            {"id": "hero.sun", "slot": "hero", "name": "Sun", "price": 0, "patch": {"look": {"hero": [1, 2, 3]}}},
            {"id": "hero.mint", "slot": "hero", "name": {"en": "Mint"}, "price": 50,
             "requires": {"stat": "runs", "at_least": 2}, "patch": {"look": {"hero": [4, 5, 6]}}},
        ],
        "daily": {"enabled": True},
        "modifiers": [{"id": "fast", "name": "Fast", "patch": {"course": {"ramp_bars": 10}}}],
    }


def bad(mutate, text):
    raw = good()
    mutate(raw)
    with pytest.raises(m.MetaError, match=text):
        m.normalize_meta(raw)


# ---- the vectors ------------------------------------------------------------------


def test_vectors_are_current():
    done = subprocess.run([sys.executable, str(ROOT / "tools/gen_meta_vectors.py"), "--check"],
                          capture_output=True, text=True, cwd=ROOT)
    assert done.returncode == 0, done.stdout + done.stderr


def test_known_values_not_taken_from_the_code_under_test():
    # FNV-1a 32 reference values, and calendar dates worked out by hand.
    assert m.fnv1a32("") == 0x811C9DC5
    assert m.fnv1a32("a") == 0xE40C292C
    assert m.fnv1a32("foobar") == 0xBF9CF968
    assert m.date_string(0) == "1970-01-01"
    assert m.date_string(m.day_number(951782400000)) == "2000-02-29"
    assert m.date_string(20751) == "2026-10-25"
    assert m.day_number(1761436799999) == 20386 and m.day_number(1761436800000) == 20387


def test_seed_is_per_game_per_day_and_a_u32(vectors):
    seeds = {(r["game"], r["day"]): r["seed"] for r in vectors["seed"]}
    assert all(0 <= s < 2**32 for s in seeds.values())
    assert len(set(seeds.values())) == len(seeds)  # no collisions in the sample
    assert m.daily_seed("hop_beat", 20751) == m.daily_seed("hop_beat", 20751)
    assert m.daily_seed("hop_beat", 20751) != m.daily_seed("hop_beat", 20752)
    assert m.daily_seed("hop_beat", 20751) != m.daily_seed("neon_cat", 20751)


def test_streak_rules():
    assert m.streak_next(None, 0, 10) == {"streak": 1, "counted": True}
    assert m.streak_next(10, 3, 10) == {"streak": 3, "counted": False}
    assert m.streak_next(10, 3, 11) == {"streak": 4, "counted": True}
    assert m.streak_next(10, 3, 13) == {"streak": 1, "counted": True}
    assert m.streak_next(10, 3, 9) == {"streak": 3, "counted": False}  # the clock went back


def test_reward_grows_then_caps():
    r = {"base": 10, "per_day": 5, "max": 50}
    assert [m.streak_reward(r, s) for s in (0, 1, 2, 3, 9, 50)] == [0, 10, 15, 20, 50, 50]


def test_earn_formula_and_edges():
    spec = {"base": 3, "per": {"score": 0.01, "combo": 0.5}, "cap": 300}
    assert m.earn(spec, {"score": 1000, "combo": 20}) == 23
    assert m.earn(spec, {"score": 0, "combo": 20}) == 0           # nothing scored, nothing earned
    assert m.earn(spec, {"score": float("inf")}) == 0
    assert m.earn(spec, {"score": 10**9}) == 300                 # capped
    assert m.earn(spec, {"score": 100, "combo": float("nan")}) == 4
    assert m.earn(spec, {"score": 100, "combo": -9}) == 4        # a negative stat never takes coins away


def test_patch_rules_leave_other_paths_alone():
    doc = {"look": {"hero": [1, 1, 1], "slab": [2, 2, 2]}, "audio": {"master": 1}}
    out = m.apply_patch(doc, {"look": {"hero": [9, 9, 9]}}, ["look"])
    assert out == {"look": {"hero": [9, 9, 9], "slab": [2, 2, 2]}, "audio": {"master": 1}}
    assert doc["look"]["hero"] == [1, 1, 1]                      # the input is not changed
    assert m.apply_patch(doc, {"audio": {"master": 0}}, ["look"]) == doc
    assert m.apply_patch(doc, {"look": {"hero": [9, 9, 9]}, "audio": {"master": 0}}, ["look"]) == doc  # all or nothing
    assert not m.patch_allowed({"lives": {"count": 1}}, ["lives.countx"])


# ---- validation -------------------------------------------------------------------


def test_none_is_no_meta_and_a_good_block_gets_defaults():
    assert m.normalize_meta(None) == {}
    n = m.normalize_meta(good())
    assert n["version"] == m.SCHEMA_VERSION
    assert n["earn"] == {"base": 5, "per": {"score": 0.02}, "cap": 500}
    assert n["currency"]["name"]["ja"] == "コイン"
    assert n["catalog"][0]["name"] == {"en": "Sun", "ja": "Sun"}
    assert n["catalog"][1]["name"] == {"en": "Mint", "ja": "Mint"}
    assert n["daily"]["seed_path"] == "course.seed" and n["daily"]["streak_reward"]["max"] == 50
    assert "rewarded" not in n


def test_the_block_is_idempotent():
    n = m.normalize_meta(good())
    assert m.normalize_meta(_strip(n)) == n


def _strip(n):
    n = copy.deepcopy(n)
    n.pop("version")
    return n


def test_rejections():
    bad(lambda r: r.update(extra=1), "unknown keys")
    bad(lambda r: r["catalog"][0].update(slot="shoe"), "slot")
    bad(lambda r: r["catalog"][1].update(id="hero.sun"), "listed twice")
    bad(lambda r: r["catalog"][1].update(id="Bad Id"), "lowercase")
    bad(lambda r: r["catalog"][1].update(price=-1), "price")
    bad(lambda r: r["catalog"][1].update(price=1.5), "integer")
    bad(lambda r: r["catalog"][1].update(price=True), "integer")
    bad(lambda r: r["catalog"][1].update(requires={"stat": "luck", "at_least": 1}), "stat")
    bad(lambda r: r["catalog"][1].update(requires={"stat": "runs"}), "at_least")
    bad(lambda r: r["catalog"][1].update(patch={"audio": {"master": 0}}), "did not allow")
    bad(lambda r: r["catalog"][1].update(patch={"look": {"hero": None}}), "null")
    bad(lambda r: r["catalog"][1].update(patch={}), "non-empty")
    bad(lambda r: r["catalog"][1].update(patch={"look": {}}), "empty object")
    bad(lambda r: r["catalog"][1].update(preview=[[1, 2]]), "colours")
    bad(lambda r: r["catalog"][1].update(preview=[[1, 2, 300]]), "0..255")
    bad(lambda r: r["catalog"].__delitem__(0), "free item")  # no default left in the slot
    bad(lambda r: r["catalog"][0].update(price=5), "free item")
    bad(lambda r: r["catalog"][0].update(requires={"stat": "runs", "at_least": 1}), "free item")
    bad(lambda r: r.update(patchable=["look", "x..y"]), "dotted")
    bad(lambda r: r["modifiers"][0].update(patch={"lives": {"count": 1}}), "did not allow")
    bad(lambda r: r["modifiers"].append(dict(r["modifiers"][0])), "listed twice")
    bad(lambda r: r.update(earn={"cap": 0}), "cap")
    bad(lambda r: r.update(earn={"per": {"Score": 1}}), "lowercase")
    bad(lambda r: r.update(earn={"per": {"score": -1}}), "0..1000")
    bad(lambda r: r.update(earn={"per": {"score": float("nan")}}), "number")
    bad(lambda r: r["daily"].update(enabled="yes"), "true or false")
    bad(lambda r: r["daily"].update(seed_path="a b"), "dotted")
    bad(lambda r: r["daily"].update(streak_reward={"base": -1}), "base")
    bad(lambda r: r.update(rewarded={"slot": ""}), "slot")
    bad(lambda r: r.update(catalog=[good()["catalog"][0]] * 65), "at most")


# ---- the manifest and the bundle --------------------------------------------------


def test_hop_beat_manifest_has_a_valid_meta_block():
    man = load_manifest(HOP_MANIFEST, ROOT)
    meta = man.meta
    assert meta["rewarded"] == {"slot": "double_coins"}
    assert {c["slot"] for c in meta["catalog"]} == {"hero", "look"}
    assert len(meta["modifiers"]) >= 2 and meta["daily"]["enabled"]
    slots = {r["slot"]: r for r in man.platform["ads"]["rewarded"]}
    assert slots["double_coins"]["per_run"] == 1 and slots["revive"]["effect"] == "revive"


def test_other_games_have_no_meta():
    assert load_manifest(ROOT / "games/neon_cat_minimal.json", ROOT).meta == {}


def test_a_rewarded_slot_the_platform_does_not_offer_is_refused(tmp_path):
    raw = json.loads(HOP_MANIFEST.read_text(encoding="utf-8"))
    raw["meta"]["rewarded"] = {"slot": "nope"}
    path = tmp_path / "hop_beat_minimal.json"
    path.write_text(json.dumps(raw), encoding="utf-8")
    with pytest.raises(ManifestError, match="not a platform.ads.rewarded slot"):
        load_manifest(path, ROOT)
    raw["meta"]["catalog"][1]["slot"] = "shoe"
    path.write_text(json.dumps(raw), encoding="utf-8")
    with pytest.raises(ManifestError, match="meta.catalog"):
        load_manifest(path, ROOT)


def test_preview_bundle_carries_meta_and_ships_meta_js(tmp_path):
    bbp = _load("build_browser_preview")
    assert "meta.js" in bbp.SITE_SCRIPTS
    out = tmp_path / "site"
    bbp.build_preview(ROOT, out, None, require_pkg=False)
    bundle = json.loads((out / "games/hop_beat/bundle.json").read_text(encoding="utf-8"))
    assert bundle["meta"] == load_manifest(HOP_MANIFEST, ROOT).meta
    assert "meta" not in json.loads((out / "games/neon_cat/bundle.json").read_text(encoding="utf-8"))
    assert (out / "meta.js").is_file()


def test_portal_package_ships_meta_js_and_no_urls(tmp_path):
    pp = _load("package_portal")
    pkg = tmp_path / "pkg"
    pkg.mkdir()
    (pkg / "kagra_shared.js").write_text("export default function init() {}\n", encoding="utf-8")
    (pkg / "kagra_shared_bg.wasm").write_bytes(b"\0asm\1\0\0\0" + os.urandom(1024))
    out = tmp_path / "out"
    code = pp.main(["--game", "hop_beat_minimal", "--pkg", str(pkg), "--out", str(out), "--keep-tree"])
    assert code == 0
    tree = out / "hop_beat"
    assert (tree / "meta.js").is_file()
    bundle = json.loads((tree / "games/hop_beat/bundle.json").read_text(encoding="utf-8"))
    assert bundle["meta"]["catalog"] and bundle["platform"]["ads"]["rewarded"][1]["slot"] == "double_coins"
    assert pp.unexpected_urls(tree) == []
    assert pp.broken_paths(tree) == []
    with zipfile.ZipFile(next(out.glob("*.zip"))) as zf:
        assert "meta.js" in zf.namelist()
    assert "http" not in (ROOT / "scripts/preview_site/meta.js").read_text(encoding="utf-8")
