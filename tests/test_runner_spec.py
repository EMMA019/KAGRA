"""Game spec core + runner kit: Python specs are the source of the baked worlds."""
from __future__ import annotations

import json
import re
from pathlib import Path

import pytest

from kagra.game_spec import CATALOG, ITEM_EFFECTS, GameSpec, ItemSpec, measure_glb

ROOT = Path(__file__).resolve().parents[1]
GAMES = ("neon_cat_minimal", "meteor_cat_minimal")


@pytest.mark.parametrize("game", GAMES)
def test_baked_world_matches_the_manifest_overrides(game):
    # Rebake with: python examples/<game>.py --bake
    manifest = json.loads((ROOT / f"games/{game}.json").read_text(encoding="utf-8"))
    baked = json.loads((ROOT / manifest["world"]).read_text(encoding="utf-8"))
    spec = GameSpec.from_overrides(manifest["game"])
    assert baked == spec.world()
    glbs = [spec.player.glb, spec.runner.rings.glb, spec.runner.gates.glb]
    for glb in glbs + [o.glb for o in spec.obstacles]:
        assert glb in manifest["bundle_assets"], glb


def test_neon_cat_is_the_default():
    assert GameSpec.from_overrides({}) == GameSpec()


def test_catalog_hitboxes_are_the_measured_mesh():
    for name, row in CATALOG.items():
        if "mesh_min" in row:
            assert list(measure_glb(ROOT / row["glb"])) == [row["mesh_min"], row["mesh_max"]], name


def test_overrides_merge_and_reject_typos():
    spec = GameSpec.from_overrides(
        {"obstacles": ["METEOR", {"name": "CRATE", "weight": 3}], "runner": {"speed": {"cap": 30}}}
    )
    assert [o.name for o in spec.obstacles] == ["METEOR", "CRATE"]
    assert spec.obstacles[1].weight == 3 and spec.obstacles[1].scale == [1.15] * 3
    assert spec.runner.speed.cap == 30.0 and spec.runner.speed.start == 10.5
    with pytest.raises(KeyError):
        GameSpec.from_overrides({"runner": {"sped": {}}})


def test_item_vocabulary_matches_rust_and_is_closed():
    rust = (ROOT / "kagra-shared/src/game_spec.rs").read_text(encoding="utf-8")
    body = re.search(r"pub enum ItemEffect \{(.*?)\}", rust, re.S).group(1)
    assert tuple(v.lower() for v in re.findall(r"(\w+),", body)) == ITEM_EFFECTS
    assert GameSpec.from_overrides({"items": ["magnet"]}).items[0].effect == "magnet"
    with pytest.raises(ValueError):
        ItemSpec("laser")


def test_spec_needs_a_spawnable_obstacle():
    with pytest.raises(ValueError):
        GameSpec.from_overrides({"obstacles": [{"name": "CRATE", "weight": 0}]}).to_dict()


SECTIONED = [
    {"kind": "intro", "length": 140, "level": 1},
    {"kind": "learn", "length": 200, "level": 2},
    {"kind": "challenge", "length": 260, "level": 3},
    {"kind": "rest", "length": 150, "level": 0},
    {"kind": "challenge", "length": 300, "level": 3},
    {"kind": "reward", "length": 120, "level": 0},
]


def test_sectioned_fixture_matches_the_python_spec():
    spec = GameSpec.from_overrides({"runner": {"sections": SECTIONED}})
    baked = json.loads((ROOT / "kagra-shared/tests/fixtures/neon_cat_sectioned_world.json").read_text())
    assert baked == spec.world()


def test_sections_are_validated():
    from kagra.runner_kit import SECTION_DENSITY, SECTION_KINDS

    assert set(SECTION_DENSITY) == set(SECTION_KINDS)
    rust = (ROOT / "kagra-shared/src/game_spec.rs").read_text(encoding="utf-8")
    for kind in SECTION_KINDS:
        assert kind.capitalize() in rust, kind
    for bad in ({"kind": "boss"}, {"length": 0}, {"level": 6}, {"density": 1.5}):
        with pytest.raises(ValueError):
            GameSpec.from_overrides({"runner": {"sections": [bad]}})
