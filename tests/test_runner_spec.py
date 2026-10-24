"""Game spec core + runner kit: Python specs are the source of the baked worlds."""
from __future__ import annotations

import json
import re
from pathlib import Path

import pytest

from kagra.game_spec import BEHAVIORS, CATALOG, ITEM_EFFECTS, GameSpec, ItemSpec, measure_glb

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
    if spec.runner.coins.enabled:
        glbs.append(spec.runner.coins.glb)
    for o in spec.obstacles:
        glbs += [o.glb] + [p.glb for p in o.parts]
    for glb in glbs:
        assert not glb or glb in manifest["bundle_assets"], glb


def test_neon_cat_is_the_default():
    assert GameSpec.from_overrides({}) == GameSpec()


def test_catalog_hitboxes_are_the_measured_mesh():
    for name, row in CATALOG.items():
        if "mesh_min" in row and row.get("glb"):
            assert list(measure_glb(ROOT / row["glb"])) == [row["mesh_min"], row["mesh_max"]], name
        for part in row.get("parts", []):
            if "mesh_min" in part:
                got = list(measure_glb(ROOT / part["glb"]))
                assert got == [part["mesh_min"], part["mesh_max"]], (name, part["glb"])


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


CLASSIC = {
    "obstacles": [{"name": n, "weight": 1} for n in ("CRATE", "BARREL", "BARRIER")],
    "runner": {"sections": [], "obstacle_track": {"gap": 14.0}, "score": {"per_gate": 0}},
}


def test_classic_fixture_matches_the_python_spec():
    """The three-crate course the golden runs were recorded on."""
    spec = GameSpec.from_overrides(CLASSIC)
    baked = json.loads((ROOT / "kagra-shared/tests/fixtures/neon_cat_classic_world.json").read_text())
    assert baked == spec.world()


def test_default_course_has_rest_and_reward_and_every_behavior():
    spec = GameSpec()
    kinds = [s.kind for s in spec.runner.sections]
    assert "rest" in kinds and "reward" in kinds
    assert {o.behavior for o in spec.obstacles} == {"solid", "jump_over", "roll_under", "rideable", "cat_gap"}
    for sec in spec.runner.sections:
        assert set(sec.allow) <= set(BEHAVIORS)


def test_behaviors_match_rust():
    rust = (ROOT / "kagra-shared/src/game_spec.rs").read_text(encoding="utf-8")
    body = re.search(r"pub enum Behavior \{(.*?)\}", rust, re.S).group(1)
    names = tuple(re.sub(r"(?<!^)(?=[A-Z])", "_", v).lower() for v in re.findall(r"^\s+([A-Z]\w*),", body, re.M))
    assert set(names) == set(BEHAVIORS)


def test_sections_are_validated():
    from kagra.runner_kit import SECTION_DENSITY, SECTION_KINDS

    assert set(SECTION_DENSITY) == set(SECTION_KINDS)
    rust = (ROOT / "kagra-shared/src/game_spec.rs").read_text(encoding="utf-8")
    for kind in SECTION_KINDS:
        assert kind.capitalize() in rust, kind
    for bad in ({"kind": "boss"}, {"length": 0}, {"level": 6}, {"density": 1.5}):
        with pytest.raises(ValueError):
            GameSpec.from_overrides({"runner": {"sections": [bad]}})


def test_colour_codes_merge_and_reject_unknown_names():
    spec = GameSpec.from_overrides({"style": {"codes": {"solid": [255, 0, 0]}}})
    codes = spec.world()["game"]["style"]["codes"]
    assert codes["solid"] == [255, 0, 0]
    assert codes["jump_over"] == [255, 170, 40]
    assert set(codes) == {"jump_over", "roll_under", "solid", "cat_gap", "rideable", "pass_through"}
    with pytest.raises(ValueError):
        GameSpec.from_overrides({"style": {"codes": {"lava": [1, 2, 3]}}})
