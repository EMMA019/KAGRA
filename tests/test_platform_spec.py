"""``platform.ads`` in a game manifest: what is accepted and what is refused."""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from kagra.distribute import ManifestError, load_manifest
from kagra.platform_spec import PlatformError, normalize_ads, normalize_platform

ROOT = Path(__file__).resolve().parents[1]


def test_none_gives_defaults():
    ads = normalize_ads(None)
    assert ads == {"commercial_on": ["retry"], "rewarded": [], "min_interval_secs": 90}
    ads["commercial_on"].append("x")  # a caller's edit must not leak into the defaults
    assert normalize_ads(None)["commercial_on"] == ["retry"]


def test_full_block_round_trips():
    raw = {
        "commercial_on": ["game_over", "retry"],
        "rewarded": [{"slot": "revive", "at": "game_over", "effect": "none", "per_run": 2}],
        "min_interval_secs": 0,
    }
    assert normalize_ads(raw)["commercial_on"] == ["game_over", "retry"]
    assert normalize_ads(raw)["rewarded"][0]["per_run"] == 2
    assert normalize_ads(raw)["min_interval_secs"] == 0


@pytest.mark.parametrize(
    "raw, text",
    [
        ("yes", "must be an object"),
        ({"surprise": 1}, "unknown keys"),
        ({"commercial_on": "retry"}, "list of strings"),
        ({"commercial_on": ["title"]}, "unknown slots"),
        ({"min_interval_secs": -1}, "0 to"),
        ({"min_interval_secs": 99999}, "0 to"),
        ({"min_interval_secs": True}, "0 to"),
        ({"rewarded": [{"slot": "r", "at": "nowhere"}]}, "at"),
        ({"rewarded": [{"slot": "r", "effect": "extra_life"}]}, "effect"),
        ({"rewarded": [{"slot": "r", "per_run": 0}]}, "per_run"),
        ({"rewarded": [{"at": "title"}]}, "slot"),
    ],
)
def test_bad_ads_are_refused(raw, text):
    with pytest.raises(PlatformError, match=text):
        normalize_ads(raw)


def test_platform_block():
    assert normalize_platform(None) == {}
    assert "ads" in normalize_platform({"ads": {}})
    with pytest.raises(PlatformError, match="unknown keys"):
        normalize_platform({"ads": {}, "iap": {}})
    with pytest.raises(PlatformError, match="object"):
        normalize_platform([])


def test_neon_cat_manifest_carries_ads():
    m = load_manifest(ROOT / "games" / "neon_cat_minimal.json", ROOT)
    assert m.platform["ads"]["commercial_on"] == ["retry"]
    assert m.platform["ads"]["rewarded"] == []


def test_manifest_without_platform_is_unchanged():
    m = load_manifest(ROOT / "games" / "neon_run_minimal.json", ROOT)
    assert m.platform == {}


def test_manifest_rejects_bad_platform(tmp_path):
    raw = json.loads((ROOT / "games" / "neon_cat_minimal.json").read_text(encoding="utf-8"))
    raw["platform"] = {"ads": {"commercial_on": ["nope"]}}
    bad = tmp_path / "bad.json"
    bad.write_text(json.dumps(raw), encoding="utf-8")
    with pytest.raises(ManifestError, match="commercial_on"):
        load_manifest(bad, ROOT)
