//! The `meta` block of `games/hop_beat_minimal.json` against the real HOP-BEAT spec:
//! every unlock and every run modifier must still be a valid game, the daily seed must
//! lay one fixed course, and a patch must only move what the kit allowed.

use kagra_shared::game_core::meta::{apply_patch, daily_seed, pick_modifier};
use kagra_shared::hop_beat::{Course, HopSpec};
use serde_json::Value;

fn read(rel: &str) -> Value {
    let path = format!("{}/../{rel}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")))
        .unwrap()
}

fn setup() -> (Value, Vec<String>, Value) {
    let manifest = read("games/hop_beat_minimal.json");
    let world = read("kagra-shared/tests/fixtures/hop_beat_world.json");
    let meta = manifest["meta"].clone();
    let allow = meta["patchable"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    (meta, allow, world["game"].clone())
}

fn layout(game: &Value) -> String {
    let spec = HopSpec::from_value(game).unwrap();
    let mut c = Course::new(&spec);
    c.ensure(&spec, 400.0);
    format!("{:?}", c.obst)
}

#[test]
fn every_catalogue_item_and_modifier_is_a_valid_hop_beat_spec() {
    let (meta, allow, game) = setup();
    assert!(HopSpec::from_value(&game).is_ok());
    let mut seen = 0;
    for item in meta["catalog"].as_array().unwrap() {
        let patched = apply_patch(&game, &item["patch"], &allow);
        if item["price"] == 0 && item.get("requires").is_none() {
            // The default of a slot is the shipped game: equipping nothing changes nothing.
            assert_eq!(
                patched, game,
                "{}: a default must equal the shipped spec",
                item["id"]
            );
        } else {
            assert_ne!(patched, game, "{}: the patch must take effect", item["id"]);
        }
        let spec = HopSpec::from_value(&patched).unwrap_or_else(|e| panic!("{}: {e}", item["id"]));
        assert!(spec.course_problems().is_empty(), "{}", item["id"]);
        seen += 1;
    }
    for m in meta["modifiers"].as_array().unwrap() {
        let patched = apply_patch(&game, &m["patch"], &allow);
        assert_ne!(patched, game, "{}: the patch must take effect", m["id"]);
        let spec = HopSpec::from_value(&patched).unwrap_or_else(|e| panic!("{}: {e}", m["id"]));
        assert!(spec.course_problems().is_empty(), "{}", m["id"]);
        seen += 1;
    }
    assert!(seen >= 8);
}

#[test]
fn skins_change_the_look_and_never_the_rules() {
    let (meta, allow, game) = setup();
    let base = layout(&game);
    for item in meta["catalog"].as_array().unwrap() {
        let patched = apply_patch(&game, &item["patch"], &allow);
        assert_eq!(
            layout(&patched),
            base,
            "{}: a skin must not change the course",
            item["id"]
        );
        for key in ["audio", "judge", "music", "lives", "play", "revive"] {
            assert_eq!(patched[key], game[key], "{}: {key}", item["id"]);
        }
    }
}

#[test]
fn the_daily_seed_lays_one_fixed_course_and_other_days_differ() {
    let (meta, allow, game) = setup();
    let mods = meta["modifiers"].as_array().unwrap();
    let day = 20751;
    let build = |d: i64| {
        let mut g = game.clone();
        let i = pick_modifier("hop_beat", d, mods.len()).unwrap();
        g = apply_patch(&g, &mods[i]["patch"], &allow);
        g["course"]["seed"] = Value::from(daily_seed("hop_beat", d));
        g
    };
    let a = layout(&build(day));
    assert_eq!(a, layout(&build(day)), "same day, same run");
    let distinct: std::collections::BTreeSet<String> =
        (0..30).map(|d| layout(&build(day + d))).collect();
    assert!(
        distinct.len() >= 25,
        "days should differ, got {} distinct of 30",
        distinct.len()
    );
}

#[test]
fn every_modifier_is_valid_for_every_day_of_a_year() {
    let (meta, allow, game) = setup();
    let mods = meta["modifiers"].as_array().unwrap();
    let mut used = std::collections::BTreeSet::new();
    for d in 20751..20751 + 366 {
        let i = pick_modifier("hop_beat", d, mods.len()).unwrap();
        used.insert(i);
        let mut g = apply_patch(&game, &mods[i]["patch"], &allow);
        g["course"]["seed"] = Value::from(daily_seed("hop_beat", d));
        let spec = HopSpec::from_value(&g).unwrap();
        assert!(spec.course_problems().is_empty());
    }
    assert_eq!(used.len(), mods.len(), "every modifier comes up in a year");
}

#[test]
fn a_patch_outside_the_allow_list_changes_nothing() {
    let (_, allow, game) = setup();
    let evil = serde_json::json!({"audio": {"master": 0}, "lives": {"count": 9}});
    assert_eq!(apply_patch(&game, &evil, &allow), game);
}
