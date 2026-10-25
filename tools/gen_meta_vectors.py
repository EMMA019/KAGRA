#!/usr/bin/env python3
"""Write (or check) kagra-shared/tests/fixtures/meta_vectors.json from kagra/meta_spec.py.

    python3 tools/gen_meta_vectors.py          # rewrite the file
    python3 tools/gen_meta_vectors.py --check  # exit 1 if it is stale

The Python reference produces the expected values; Rust (`game_core/meta.rs`) and JS
(`meta.js`) replay the file and must agree.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from kagra import meta_spec as m  # noqa: E402

OUT = ROOT / "kagra-shared" / "tests" / "fixtures" / "meta_vectors.json"


def build() -> dict:
    v: dict = {
        "_note": "Replayed by game_core/meta.rs, tests/test_meta_spec.py and tests/test_meta_js.py. "
        "Regenerate with: python3 tools/gen_meta_vectors.py"
    }
    dates = [0, 86399999, 86400000, 951782400000, 1761350400000, 1761436799999, 1761436800000,
             1767225600000, 1709164800000, -1]
    v["date"] = [{"ms": ms, "day": m.day_number(ms), "date": m.date_string(m.day_number(ms))} for ms in dates]
    v["seed"] = []
    for game in ("hop_beat", "neon_cat"):
        for day in (0, 20751, 20752, 20753, 20800):
            for mods in (0, 1, 3):
                v["seed"].append({"game": game, "day": day, "mods": mods,
                                  "seed": m.daily_seed(game, day), "pick": m.pick_modifier(game, day, mods)})
    v["streak"] = []
    for last, streak, today in [(None, 0, 10), (10, 3, 10), (10, 3, 11), (10, 3, 12), (10, 3, 40), (10, 3, 9),
                                (10, 0, 10), (10, 0, 11), (None, 5, 10), (10, 9, 11)]:
        r = m.streak_next(last, streak, today)
        v["streak"].append({"last": last, "streak": streak, "today": today,
                            "next": r["streak"], "counted": r["counted"]})
    v["reward"] = []
    for base, per, mx, s in [(10, 5, 50, 0), (10, 5, 50, 1), (10, 5, 50, 2), (10, 5, 50, 9), (10, 5, 50, 100),
                             (0, 0, 0, 3), (10, 5, 12, 4), (100, 0, 100, 5)]:
        v["reward"].append({"base": base, "per_day": per, "max": mx, "streak": s,
                            "coins": m.streak_reward({"base": base, "per_day": per, "max": mx}, s)})
    v["earn"] = []
    for base, per, cap, run in [
        (5, {"score": 0.02, "combo": 0.5, "perfect": 0.25}, 400, {"score": 1234, "combo": 40, "perfect": 10}),
        (5, {"score": 0.02}, 400, {"score": 0}),
        (5, {"score": 0.02}, 400, {"score": -5}),
        (5, {"score": 0.02}, 400, {}),
        (0, {"score": 0.01}, 10, {"score": 99999}),
        (3, {"score": 0.1, "stomps": 2}, 1000, {"score": 55, "stomps": 4}),
        (3, {"score": 0.1, "stomps": 2}, 1000, {"score": 55, "stomps": "x"}),
        (3, {"score": 0.1, "combo": 1}, 1000, {"score": 55, "combo": -9}),
        (4, {}, 100, {"score": 1}),
    ]:
        v["earn"].append({"base": base, "per": per, "cap": cap, "run": run,
                          "coins": m.earn({"base": base, "per": per, "cap": cap}, run)})
    v["patch"] = []
    doc = {"look": {"hero": [1, 2, 3], "slab": [4, 5, 6]}, "course": {"seed": 7, "ramp_bars": 40},
           "audio": {"master": 0.8}, "lives": {"count": 3}}
    for patch, allow in [
        ({"look": {"hero": [9, 9, 9]}}, ["look"]),
        ({"look": {"hero": [9, 9, 9]}}, ["look.slab"]),
        ({"course": {"ramp_bars": 14}}, ["look", "course.ramp_bars"]),
        ({"course": {"ramp_bars": 14, "seed": 1}}, ["course.ramp_bars"]),
        ({"audio": {"master": 0}}, ["look", "course"]),
        ({"lives": {"count": 1}}, ["lives.count"]),
        ({"lives": {"count": 1}}, ["lives.countx"]),
        ({"lives": {"count": 1}}, []),
        ({}, ["look"]),
        ({"look": {}}, ["look"]),
        ({"look": {"hero": [9, 9, 9]}, "audio": {"master": 0}}, ["look", "audio"]),
    ]:
        v["patch"].append({"doc": doc, "patch": patch, "allow": allow,
                           "allowed": m.patch_allowed(patch, allow), "result": m.apply_patch(doc, patch, allow)})
    return v


def text(v: dict) -> str:
    return json.dumps(v, indent=1, ensure_ascii=False) + "\n"


def main() -> int:
    want = text(build())
    if "--check" in sys.argv:
        have = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if have != want:
            print(f"{OUT} is stale; run python3 tools/gen_meta_vectors.py")
            return 1
        return 0
    OUT.write_text(want, encoding="utf-8")
    print(f"wrote {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
