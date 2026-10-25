//! Retention rules every kit can share: the coin formula, the daily seed and
//! streak, and the JSON patch that unlocks, skins and run modifiers use to
//! change a spec.
//!
//! Everything here is pure: same inputs, same answer, on every platform. The
//! `meta` block of a game's manifest (validated in `kagra/meta_spec.py`, read
//! by the page in `scripts/preview_site/meta.js`) says what to earn, sell and
//! rotate; this module is the Rust side of the same rules, so a native or
//! Android shell can apply the same catalogue and the same daily run. One
//! vector file (`tests/fixtures/meta_vectors.json`) is replayed by Rust,
//! Python and JS.
//!
//! A kit opts in by listing the spec paths a patch may touch. A patch that
//! strays outside that list is ignored whole, so a catalogue entry can never
//! change audio, timing or anything the kit did not offer.

use serde_json::{Map, Value};
use std::collections::BTreeMap;

const DAY_MS: f64 = 86_400_000.0;

/// 32-bit FNV-1a over the UTF-8 bytes.
pub fn fnv1a32(text: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in text.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// UTC days since 1970-01-01 for a unix time in milliseconds.
pub fn day_number(ms: f64) -> i64 {
    (ms / DAY_MS).floor() as i64
}

/// `YYYY-MM-DD` (UTC) of a day number.
pub fn date_string(day: i64) -> String {
    // Howard Hinnant's civil_from_days.
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// The seed every player gets on `day` (UTC date) for this game.
pub fn daily_seed(game_id: &str, day: i64) -> u32 {
    fnv1a32(&format!("{game_id}:{}", date_string(day)))
}

/// Index of the day's modifier among `count`, or `None` when there are none.
pub fn pick_modifier(game_id: &str, day: i64, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(fnv1a32(&format!("{game_id}:{}:mod", date_string(day))) as usize % count)
}

/// The streak after a daily run finished on `today`: `(streak, counted)`.
/// `counted` is true on a new day (the streak moved; the reward may be paid).
/// A clock that went backwards counts nothing and changes nothing.
pub fn streak_next(last_day: Option<i64>, streak: u32, today: i64) -> (u32, bool) {
    match last_day {
        None => (1, true),
        Some(last) if today == last => (streak.max(1), false),
        Some(last) if today == last + 1 => (streak + 1, true),
        Some(last) if today > last + 1 => (1, true),
        Some(_) => (streak, false),
    }
}

/// Coins for the first daily run of a day at this streak length.
pub fn streak_reward(base: u32, per_day: u32, max: u32, streak: u32) -> u32 {
    if streak < 1 {
        return 0;
    }
    let raw = u64::from(base) + u64::from(per_day) * u64::from(streak - 1);
    raw.min(u64::from(max)) as u32
}

/// Coins for one finished run: `floor(base + sum(per[k] * run[k]))`, capped.
/// A run that scored nothing earns nothing.
pub fn earn(base: f64, per: &BTreeMap<String, f64>, cap: u32, run: &Value) -> u32 {
    let num = |k: &str| run.get(k).and_then(Value::as_f64).filter(|v| v.is_finite());
    if num("score").unwrap_or(0.0) <= 0.0 {
        return 0;
    }
    let mut total = base;
    for (key, weight) in per {
        if let Some(v) = num(key) {
            total += weight * v.max(0.0);
        }
    }
    total.floor().clamp(0.0, f64::from(cap)) as u32
}

/// RFC 7386 JSON merge patch: objects merge, everything else replaces, null deletes.
pub fn merge_patch(doc: &mut Value, patch: &Value) {
    let Value::Object(p) = patch else {
        *doc = patch.clone();
        return;
    };
    if !doc.is_object() {
        *doc = Value::Object(Map::new());
    }
    let Value::Object(d) = doc else { return };
    for (key, value) in p {
        if value.is_null() {
            d.remove(key);
        } else {
            merge_patch(d.entry(key.clone()).or_insert(Value::Null), value);
        }
    }
}

/// Dotted paths of every leaf of a patch (arrays and scalars are leaves).
pub fn patch_paths(patch: &Value) -> Vec<String> {
    fn walk(v: &Value, prefix: &str, out: &mut Vec<String>) {
        match v {
            Value::Object(m) if !m.is_empty() => {
                for (k, child) in m {
                    let p = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    walk(child, &p, out);
                }
            }
            _ if !prefix.is_empty() => out.push(prefix.to_string()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(patch, "", &mut out);
    out
}

/// True when the patch is a non-empty object and every leaf sits at or below an allowed path.
pub fn patch_allowed(patch: &Value, allow: &[String]) -> bool {
    if !patch.is_object() {
        return false;
    }
    let paths = patch_paths(patch);
    !paths.is_empty()
        && paths.iter().all(|p| {
            allow.iter().any(|a| {
                p == a
                    || p.strip_prefix(a.as_str())
                        .is_some_and(|r| r.starts_with('.'))
            })
        })
}

/// `doc` with `patch` merged in; unchanged when the patch strays outside `allow`.
pub fn apply_patch(doc: &Value, patch: &Value, allow: &[String]) -> Value {
    let mut out = doc.clone();
    if patch_allowed(patch, allow) {
        merge_patch(&mut out, patch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vectors() -> Value {
        serde_json::from_str(include_str!("../../tests/fixtures/meta_vectors.json")).unwrap()
    }

    fn rows<'a>(v: &'a Value, key: &str) -> &'a Vec<Value> {
        v[key]
            .as_array()
            .unwrap_or_else(|| panic!("vector section {key}"))
    }

    #[test]
    fn dates_and_seeds_match_the_vectors() {
        let v = vectors();
        for r in rows(&v, "date") {
            let ms = r["ms"].as_f64().unwrap();
            assert_eq!(day_number(ms), r["day"].as_i64().unwrap(), "{r}");
            assert_eq!(
                date_string(r["day"].as_i64().unwrap()),
                r["date"].as_str().unwrap(),
                "{r}"
            );
        }
        for r in rows(&v, "seed") {
            let day = r["day"].as_i64().unwrap();
            assert_eq!(
                daily_seed(r["game"].as_str().unwrap(), day),
                r["seed"].as_u64().unwrap() as u32,
                "{r}"
            );
            assert_eq!(
                pick_modifier(
                    r["game"].as_str().unwrap(),
                    day,
                    r["mods"].as_u64().unwrap() as usize
                ),
                r["pick"].as_u64().map(|n| n as usize),
                "{r}"
            );
        }
        assert_eq!(fnv1a32(""), 0x811c_9dc5);
        assert_eq!(fnv1a32("a"), 0xe40c_292c);
    }

    #[test]
    fn streaks_and_rewards_match_the_vectors() {
        let v = vectors();
        for r in rows(&v, "streak") {
            let (s, counted) = streak_next(
                r["last"].as_i64(),
                r["streak"].as_u64().unwrap() as u32,
                r["today"].as_i64().unwrap(),
            );
            assert_eq!(u64::from(s), r["next"].as_u64().unwrap(), "{r}");
            assert_eq!(counted, r["counted"].as_bool().unwrap(), "{r}");
        }
        for r in rows(&v, "reward") {
            let g = |k: &str| r[k].as_u64().unwrap() as u32;
            assert_eq!(
                u64::from(streak_reward(
                    g("base"),
                    g("per_day"),
                    g("max"),
                    g("streak")
                )),
                r["coins"].as_u64().unwrap(),
                "{r}"
            );
        }
    }

    #[test]
    fn earn_matches_the_vectors() {
        let v = vectors();
        for r in rows(&v, "earn") {
            let per: BTreeMap<String, f64> = r["per"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, w)| (k.clone(), w.as_f64().unwrap()))
                .collect();
            let got = earn(
                r["base"].as_f64().unwrap(),
                &per,
                r["cap"].as_u64().unwrap() as u32,
                &r["run"],
            );
            assert_eq!(u64::from(got), r["coins"].as_u64().unwrap(), "{r}");
        }
    }

    #[test]
    fn patches_match_the_vectors() {
        let v = vectors();
        for r in rows(&v, "patch") {
            let allow: Vec<String> = r["allow"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap().to_string())
                .collect();
            assert_eq!(
                patch_allowed(&r["patch"], &allow),
                r["allowed"].as_bool().unwrap(),
                "{r}"
            );
            assert_eq!(
                apply_patch(&r["doc"], &r["patch"], &allow),
                r["result"],
                "{r}"
            );
        }
    }

    #[test]
    fn merge_patch_replaces_arrays_and_deletes_nulls() {
        let mut d = json!({"a": {"b": 1, "c": [1, 2]}, "z": 1});
        merge_patch(&mut d, &json!({"a": {"c": [9], "b": null}}));
        assert_eq!(d, json!({"a": {"c": [9]}, "z": 1}));
    }

    #[test]
    fn a_far_future_and_a_leap_day_format_right() {
        assert_eq!(date_string(0), "1970-01-01");
        assert_eq!(date_string(day_number(951_782_400_000.0)), "2000-02-29");
        assert_eq!(date_string(-1), "1969-12-31");
    }
}
