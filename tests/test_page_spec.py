"""The manifest's ``page`` block: what a game wants from the browser page."""
from __future__ import annotations

import json
import re

import pytest

from kagra.distribute import load_manifest
from kagra.page_spec import PageError, normalize_page
from tests.conftest import ROOT

PLAY = ROOT / "scripts/preview_site/play.html"


def test_no_block_means_no_page_features():
    assert normalize_page(None) == {}
    assert normalize_page({}) == {}
    assert normalize_page({"best": False}) == {}, "false flags are dropped"


def test_flags_names_and_params_are_checked():
    page = normalize_page({
        "best": True,
        "fixed_step": True,
        "music_sync": True,
        "theme": "hop",
        "blurb": "hop.blurb",
        "default_quality": "mobile",
        "quality_words": {"safe": "low"},
        "url_params": {"fx": ["low", "high"], "skin": "string", "fx_post": "bool"},
    })
    assert page["url_params"]["fx_post"] == "bool" and page["theme"] == "hop"
    for bad in (
        {"bogus": True},
        {"best": 1},
        {"theme": "a b"},
        {"default_quality": "ultra"},
        {"quality_words": {"huge": "high"}},
        {"quality_words": {"max": "ultra"}},
        {"url_params": {"fx": []}},
        {"url_params": {"fx": "number"}},
        {"music_sync": True},
    ):
        with pytest.raises(PageError):
            normalize_page(bad)


@pytest.mark.parametrize("name", sorted(p.name for p in (ROOT / "games").glob("*.json")))
def test_every_manifest_page_block_is_valid(name):
    raw = json.loads((ROOT / "games" / name).read_text(encoding="utf-8"))
    normalize_page(raw.get("page"))


def test_the_games_that_had_page_features_still_ask_for_them():
    page = {
        n: load_manifest(ROOT / "games" / f"{n}_minimal.json", ROOT).page
        for n in ("hop_beat", "survivor_arena", "neon_cat", "meteor_cat")
    }
    assert {"music_sync", "one_button", "revive", "direct", "tagline", "pause", "fixed_step"} <= set(page["hop_beat"])
    assert page["survivor_arena"]["quality_words"] == {"safe": "low", "mid": "mobile", "max": "high"}
    assert {"pause", "fixed_step", "lang"} <= set(page["survivor_arena"])
    assert page["neon_cat"]["default_quality"] == "mobile" and "fx" in page["neon_cat"]["url_params"]
    assert set(page["meteor_cat"]) == {"best", "audio"}, "Meteor Cat never had the Neon Cat link switches"


def test_the_page_switches_features_from_the_manifest_not_the_genre():
    html = PLAY.read_text(encoding="utf-8")
    assert "page = bundle.page" in html
    for gone in ("hopMode", "survivorMode", "hopDirect", 'data-hop="1"'):
        assert gone not in html, gone
    assert not re.search(r"world\.genre === \"", html), "no genre branches in the page"
    assert not re.search(r"slug === \"", html), "no game-name branches in the page"
