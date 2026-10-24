"""Game spec core + runner kit: Python specs are the source of the baked worlds."""
from __future__ import annotations

import json
import re
from pathlib import Path

import pytest

from kagra.game_spec import (
    BEHAVIORS,
    CATALOG,
    COIN_COLOR,
    ITEM_COLORS,
    ITEM_DEFAULT_ICON,
    ITEM_EFFECTS,
    ITEM_ICONS,
    MISCHIEF_KINDS,
    GameSpec,
    ItemSpec,
    MischiefSpec,
    measure_glb,
)

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
    "lives": {"count": 1},
    "items": [],
    "mischief": [],
    "perch_route": [],
    "chaser": {"enabled": False},
    # Look, sound and goals are new; the classic runs predate them.
    "atmosphere": {"enabled": False},
    "audio": {"enabled": False},
    "missions": {"enabled": False},
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


def test_hud_result_spec_merges_death_lines_and_validates_the_board():
    spec = GameSpec.from_overrides({"hud": {"result": {"deaths": {"CRATE": "ドン"}, "board": {"top_n": 3, "columns": ["score", "time"], "sort": "time"}}}})
    r = spec.hud.result
    assert r.deaths["CRATE"] == "ドン" and r.deaths["WALL"].startswith("壁")
    assert (r.board.top_n, r.board.columns, r.board.sort) == (3, ["score", "time"], "time")
    for bad in (
        {"top_n": 0},
        {"top_n": 11},
        {"columns": []},
        {"columns": ["kills"]},
        {"columns": ["time"], "sort": "coins"},
    ):
        with pytest.raises(ValueError):
            GameSpec.from_overrides({"hud": {"result": {"board": bad}}})
    assert spec.world()["game"]["hud"]["result"]["board"]["sort"] == "time"


def test_lives_rim_and_eyes_are_specced_and_validated():
    spec = GameSpec()
    assert spec.lives.count == 3 and spec.lives.invuln_secs > 0
    assert spec.player.eyes.enabled and spec.player.rim.radius < 1.6
    world = spec.world()["game"]
    assert world["lives"]["count"] == 3
    assert world["player"]["eyes"]["glow"] > 1.0
    eyes = world["player"]["eyes"]
    assert eyes["ears"] and eyes["tail_tip"] and not eyes["collar"], "marks that read from behind"
    assert GameSpec.from_overrides({"player": {"eyes": {"enabled": False}}}).player.eyes.ears
    for bad in ({"count": 0}, {"count": 10}, {"invuln_secs": -1}, {"blink_hz": 99}):
        with pytest.raises(ValueError):
            GameSpec.from_overrides({"lives": bad})
    with pytest.raises(Exception):
        GameSpec.from_overrides({"player": {"eyes": {"nope": 1}}})


def test_lives_fields_match_rust():
    rust = (ROOT / "kagra-shared/src/game_spec.rs").read_text(encoding="utf-8")
    for name in tuple(GameSpec().player.eyes.__dataclass_fields__):
        assert re.search(rf"pub struct EyesSpec \{{.*?pub {name}:", rust, re.S), name
    for name in ("count", "invuln_secs", "blink_hz"):
        assert re.search(rf"pub struct LivesSpec \{{.*?pub {name}:", rust, re.S), name


def test_items_are_placed_by_rules_and_validated():
    spec = GameSpec()
    assert {i.effect for i in spec.items} == set(ITEM_EFFECTS)
    assert spec.item_rules.rest_every >= 1 and spec.item_rules.boost_speed > 1
    world = spec.world()["game"]
    assert [i["effect"] for i in world["items"]][0] in ITEM_EFFECTS
    assert world["item_rules"]["magnet_radius"] > 1
    for bad in ({"radius": 0.0}, {"magnet_radius": 0.5}, {"boost_speed": 3.0}, {"jump_scale": 0.5}):
        with pytest.raises(ValueError):
            GameSpec.from_overrides({"item_rules": bad})
    assert GameSpec.from_overrides({"items": []}).items == []
    assert GameSpec.from_overrides({"items": [{"effect": "boost", "duration": 2.0}]}).items[0].duration == 2.0


def test_item_rule_fields_match_rust():
    rust = (ROOT / "kagra-shared/src/game_spec.rs").read_text(encoding="utf-8")
    body = re.search(r"pub struct ItemRules \{(.*?)\n\}", rust, re.S).group(1)
    names = re.findall(r"pub (\w+):", body)
    assert names == list(GameSpec().item_rules.__dataclass_fields__)


def test_mischief_vocabulary_rules_and_fields_match_rust():
    rust = (ROOT / "kagra-shared/src/game_spec.rs").read_text(encoding="utf-8")
    body = re.search(r"pub enum MischiefKind \{(.*?)\n\}", rust, re.S).group(1)
    assert tuple(v.lower() for v in re.findall(r"^\s+(\w+),", body, re.M)) == MISCHIEF_KINDS
    names = re.findall(r"pub (\w+):", re.search(r"pub struct MischiefRules \{(.*?)\n\}", rust, re.S).group(1))
    assert names == list(GameSpec().mischief_rules.__dataclass_fields__)
    spec = GameSpec()
    assert {m.kind for m in spec.mischief} == set(MISCHIEF_KINDS)
    assert GameSpec.from_overrides({"mischief": ["yarn"]}).mischief[0].kind == "yarn"
    assert GameSpec.from_overrides({"mischief": []}).mischief == []
    with pytest.raises(ValueError):
        MischiefSpec("anvil")
    for bad in ({"side": 3.0}, {"radius": 0.0}, {"streak_secs": 0.0}, {"streak_cap": 0}):
        with pytest.raises(ValueError):
            GameSpec.from_overrides({"mischief_rules": bad})
    assert spec.world()["game"]["mischief_rules"]["score"] > 0
    assert spec.item_rules.radius >= 1.0


def test_perch_route_and_chaser_match_rust_and_are_checked():
    from kagra.game_spec import PERCH_KINDS, ChaserSpec, PerchRules

    rust = (ROOT / "kagra-shared/src/game_spec.rs").read_text(encoding="utf-8")
    body = re.search(r"pub enum PerchKind \{(.*?)\n\}", rust, re.S).group(1)
    assert tuple(re.sub(r"(?<!^)(?=[A-Z])", "_", v).lower() for v in re.findall(r"^\s+(\w+),", body, re.M)) == PERCH_KINDS
    for cls, name in ((PerchRules, "PerchRules"), (ChaserSpec, "ChaserSpec")):
        names = re.findall(r"pub (\w+):", re.search(rf"pub struct {name} \{{(.*?)\n\}}", rust, re.S).group(1))
        assert names == list(cls.__dataclass_fields__), name
    spec = GameSpec()
    assert [p.kind for p in spec.perch_route] == list(PERCH_KINDS)
    game = spec.world()["game"]
    assert len(game["perch_route"]) == 3
    # The cleaning robot is off by default and a spec can turn it back on.
    assert game["chaser"]["enabled"] is False
    on = GameSpec.from_overrides({"chaser": {"enabled": True}}).world()["game"]["chaser"]
    assert on["enabled"] is True and on["secs"] == 10.0
    # A route can be spelled with bare names or with heights.
    assert GameSpec.from_overrides({"perch_route": ["pipe", {"kind": "sign", "height": 1.2}]}).perch_route[1].top() == 1.2
    assert GameSpec.from_overrides({"perch_route": []}).perch_route == []
    for bad in (
        {"perch_route": ["unicorn"]},
        {"perch_route": [{"kind": "sign", "height": 1.9}]},  # the first step is out of reach of a jump
        {"perch_route": [{"kind": "ac_unit", "height": 0.6}, {"kind": "sign", "height": 1.9}]},  # a step too high
        {"perch_route": ["pipe"] * 7},
        {"perch_rules": {"step_len": 9.0}},
        {"perch_rules": {"half_width": 2.0}},
        {"chaser": {"secs": 0.5}},
        {"chaser": {"near_gap": 4.0, "far_gap": 3.0}},
        {"chaser": {"far_gap": 2.6}},
    ):
        with pytest.raises(ValueError):
            GameSpec.from_overrides(bad).to_dict()


def _l1(a, b):
    return sum(abs(x - y) for x, y in zip(a, b))


def test_item_colours_and_icons_are_distinct_and_match_rust():
    rust = (ROOT / "kagra-shared/src/game_spec.rs").read_text(encoding="utf-8")
    body = re.search(r"pub enum ItemIcon \{(.*?)\n\}", rust, re.S).group(1)
    names = tuple(re.sub(r"(?<!^)(?=[A-Z])", "_", v).lower() for v in re.findall(r"^\s+(\w+),", body, re.M))
    assert names == ITEM_ICONS
    spec = GameSpec()
    codes = list(spec.style.codes.values())
    cols = {i.effect: i.rgb() for i in spec.items}
    assert len(set(ITEM_DEFAULT_ICON.values())) == len(ITEM_EFFECTS)
    for a, ca in cols.items():
        assert _l1(ca, COIN_COLOR) >= 90, f"{a} looks like a coin"
        assert all(_l1(ca, c) >= 90 for c in codes), f"{a} looks like an obstacle code"
        assert all(_l1(ca, cb) >= 90 for b, cb in cols.items() if b != a), f"{a} looks like another item"
    for effect, color in ITEM_COLORS.items():
        assert cols[effect] == color
    body = re.search(r"pub struct ItemSpec \{(.*?)\n\}", rust, re.S).group(1)
    assert re.findall(r"pub (\w+):", body) == ["effect", "glb", "weight", "duration", "color", "icon"]


def test_too_close_item_colours_are_rejected():
    # Two orbs the same colour, one near the coin, one near an obstacle code.
    for items in (
        [{"effect": "magnet", "color": [255, 0, 128]}, {"effect": "boost", "color": [255, 20, 128]}],
        [{"effect": "boost", "color": [255, 214, 100]}],
        [{"effect": "superjump", "color": list(GameSpec().style.codes["jump_over"])}],
    ):
        with pytest.raises(ValueError):
            GameSpec.from_overrides({"items": items}).world()
    ok = GameSpec.from_overrides({"items": [{"effect": "boost", "color": [0, 230, 255]}]})
    assert ok.world()["game"]["items"][0]["color"] == [0, 230, 255]
    with pytest.raises(ValueError):
        ItemSpec("boost", icon="sword")


def test_item_pull_fields_are_spec_values():
    r = GameSpec().item_rules
    assert r.radius >= 1.0 and r.magnet_radius >= 3.0 and r.magnet_lead > 0
    assert r.pull_speed > 0 and r.pull_accel > 0 and 0 < r.warn_secs <= 5
    for bad in ({"pull_speed": 0.0}, {"magnet_lead": 20.0}, {"warn_secs": 9.0}, {"pull_accel": -1.0}):
        with pytest.raises(ValueError):
            GameSpec.from_overrides({"item_rules": bad})


# --- night look, sound, missions (neon_fx_spec.rs) ---------------------------

FX_RS = (ROOT / "kagra-shared/src/neon_fx_spec.rs").read_text(encoding="utf-8")


def test_sfx_cue_names_match_rust():
    from kagra.game_spec import SFX_CUES

    body = re.search(r"pub const SFX_CUES: \[&str; (\d+)\] = \[(.*?)\];", FX_RS, re.S)
    names = tuple(re.findall(r'"(\w+)"', body.group(2)))
    assert int(body.group(1)) == len(names) == len(SFX_CUES)
    assert names == SFX_CUES


def test_fx_defaults_are_on_for_neon_cat_and_cover_every_cue():
    from kagra.game_spec import SFX_CUES

    g = GameSpec().to_dict()
    assert g["atmosphere"]["enabled"] and g["audio"]["enabled"] and g["missions"]["enabled"]
    assert set(g["audio"]["sfx"]) == set(SFX_CUES)
    assert [m["kind"] for m in g["missions"]["list"]] == ["coins", "gates", "perch"]
    assert g["missions"]["bonus"] == 0, "missions must not change scores unless asked"
    # Every file the defaults name exists in the repo, so a fresh clone makes sound.
    for f in GameSpec().audio.files():
        assert (ROOT / f).is_file(), f
    w = GameSpec().atmosphere.wall
    assert (ROOT / w.texture_high).is_file() and (ROOT / w.texture_mobile).is_file()
    assert w.texture_low == "", "the low tier keeps the plain wall"


def test_fx_numbers_match_the_rust_defaults():
    """The Rust Default impls and the Python dataclasses are two copies of one table."""
    g = GameSpec().to_dict()
    rain = g["atmosphere"]["rain"]
    for key, rust in (
        ("count_low", r"count_low: (\d+)"),
        ("count_mobile", r"count_mobile: (\d+)"),
        ("count_high", r"count_high: (\d+)"),
        ("reach", r"reach: ([\d.]+),\n\s+behind"),
        ("fall_speed", r"fall_speed: ([\d.]+)"),
        ("slant", r"slant: ([\d.]+)"),
    ):
        assert float(re.search(rust, FX_RS).group(1)) == rain[key], key
    signs = g["atmosphere"]["signs"]
    assert signs["cell"] == float(re.search(r"cell: ([\d.]+),\n\s+density_low", FX_RS).group(1))
    assert signs["density_high"] == float(re.search(r"density_high: ([\d.]+)", FX_RS).group(1))
    assert g["atmosphere"]["wall"]["segment"] == float(re.search(r"segment: ([\d.]+),\n\s+reach: 48", FX_RS).group(1))
    audio = g["audio"]
    assert audio["master"] == float(re.search(r"master: ([\d.]+),\n\s+bgm_volume", FX_RS).group(1))
    assert audio["bgm"]["fade_secs"] == float(re.search(r"fade_secs: ([\d.]+)", FX_RS).group(1))
    assert audio["bgm"]["file"] == "assets/audio/neon_cat/bgm.wav"
    targets = re.findall(r"m\(MissionKind::\w+, (\d+)\)", FX_RS)
    assert [m["target"] for m in g["missions"]["list"]] == [int(t) for t in targets]


def test_fx_overrides_merge_by_field():
    spec = GameSpec.from_overrides(
        {
            "atmosphere": {"rain": {"count_high": 10}, "signs": {"enabled": False}},
            "audio": {"master": 0.3, "sfx": {"coin": {"file": "assets/audio/mine/coin.mp3", "volume": 0.5}}},
            "missions": {"list": [{"kind": "gates", "target": 3, "label": "ARCHES"}], "bonus": 100},
        }
    )
    g = spec.to_dict()
    assert g["atmosphere"]["rain"]["count_high"] == 10 and g["atmosphere"]["rain"]["count_low"] == 24
    assert g["atmosphere"]["signs"]["enabled"] is False and g["atmosphere"]["wall"]["enabled"] is True
    # `sfx` replaces the table, like the Rust map: only the cues you list make sound.
    assert list(g["audio"]["sfx"]) == ["coin"] and g["audio"]["sfx"]["coin"]["rate"] == 1.0
    assert g["audio"]["master"] == 0.3 and g["audio"]["bgm"]["volume"] == 0.55
    assert g["missions"]["list"] == [{"kind": "gates", "target": 3, "label": "ARCHES"}]
    assert spec.audio.files() == ["assets/audio/neon_cat/bgm.wav", "assets/audio/mine/coin.mp3"]


@pytest.mark.parametrize(
    "bad",
    [
        {"atmosphere": {"rain": {"count_high": 401}}},
        {"atmosphere": {"rain": {"fall_speed": 0}}},
        {"atmosphere": {"signs": {"palette": []}}},
        {"atmosphere": {"signs": {"rows": [0.5]}}},
        {"atmosphere": {"wall": {"segment": 2.0, "reach": 60.0}}},
        {"atmosphere": {"wall": {"texture_high": "../secret.png"}}},
        {"atmosphere": {"wall": {"texture_high": "assets/wall.jpg"}}},
        {"audio": {"master": 1.5}},
        {"audio": {"bgm": {"file": "assets/audio/bgm.flac"}}},
        {"audio": {"bgm": {"file": "/etc/passwd.wav"}}},
        {"audio": {"sfx": {"coin": {"file": "https://x/y.mp3"}}}},
        {"audio": {"sfx": {"coin": {"file": "a.wav", "rate": 9}}}},
        {"missions": {"list": [{"kind": "fish", "target": 1}]}},
        {"missions": {"list": [{"kind": "coins", "target": 0}]}},
        {"missions": {"list": [{"kind": "coins", "target": 1}] * 6}},
        {"missions": {"list": [{"kind": "coins", "target": 1, "label": "x" * 17}]}},
        {"missions": {"bonus": -1}},
    ],
)
def test_fx_bad_values_are_rejected(bad):
    with pytest.raises((ValueError, TypeError, KeyError)):
        GameSpec.from_overrides(bad).to_dict()


def test_fx_formats_and_silence_are_allowed():
    for f in ("a.wav", "a.OGG", "dir/a.mp3", ""):
        GameSpec.from_overrides({"audio": {"bgm": {"file": f}, "sfx": {"coin": {"file": f}}}}).to_dict()
    GameSpec.from_overrides({"atmosphere": {"wall": {"texture_high": "", "texture_mobile": ""}}}).to_dict()
    GameSpec.from_overrides({"missions": {"list": []}}).to_dict()


def test_meteor_cat_keeps_its_own_look():
    manifest = json.loads((ROOT / "games/meteor_cat_minimal.json").read_text(encoding="utf-8"))
    game = GameSpec.from_overrides(manifest["game"]).to_dict()
    assert game["atmosphere"]["enabled"] is False and game["missions"]["enabled"] is False
    assert game["audio"]["enabled"] is True
