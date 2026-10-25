"""Retention meta-layer shared by every game: wallet, unlocks, daily run, modifiers.

The ``meta`` block of ``games/<name>.json`` is validated here (like ``platform``),
copied into ``bundle.json`` and read by ``scripts/preview_site/meta.js``. It lives in
the manifest layer, not in ``GameSpec``: a shop and a daily seed are page matters that
change which *spec* is loaded, not rules of the simulation. A kit opts in by listing the
spec paths a patch may touch (``patchable``); nothing else can be changed from here.

    "meta": {
      "currency": {"id": "coins", "name": {"en": "Coins", "ja": "コイン"}},
      "earn": {"base": 4, "per": {"score": 0.02, "combo": 0.5}, "cap": 400},
      "patchable": ["look", "course.ramp_bars", "lives.count"],
      "catalog": [
        {"id": "hero.sun", "slot": "hero", "name": {"en": "Sun", "ja": "サン"},
         "price": 0, "patch": {"look": {"hero": [255, 214, 120]}}},
        {"id": "hero.mint", "slot": "hero", "name": "Mint", "price": 120,
         "requires": {"stat": "runs", "at_least": 3}, "patch": {"look": {"hero": [120, 255, 190]}}}
      ],
      "daily": {"enabled": true, "seed_path": "course.seed",
                "streak_reward": {"base": 10, "per_day": 5, "max": 50}},
      "modifiers": [{"id": "sprint", "name": "Sprint", "patch": {"course": {"ramp_bars": 14}}}],
      "rewarded": {"slot": "double_coins"}
    }

The pure rules below (seed, date, streak, earn, patch, pick) are mirrored in Rust
(``kagra-shared/src/game_core/meta.rs``) and in ``meta.js``. One vector file,
``kagra-shared/tests/fixtures/meta_vectors.json``, is replayed by all three.
"""
from __future__ import annotations

import copy
import math
import re
from datetime import datetime, timezone
from typing import Any

SCHEMA_VERSION = 1
SLOTS = ("hero", "look", "music")
STATS = ("runs", "best_score", "best_combo", "best_streak", "daily_played")
MAX_ITEMS = 64
MAX_MODIFIERS = 16
MAX_PER_KEYS = 8
DEFAULT_SEED_PATH = "course.seed"
DAY_MS = 86_400_000

_ID_RE = re.compile(r"^[a-z0-9][a-z0-9_.-]{0,39}$")
_KEY_RE = re.compile(r"^[a-z][a-z0-9_]{0,23}$")
_PATH_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*$")


class MetaError(ValueError):
    """A ``meta`` block that cannot be used as written."""


# ---------------------------------------------------------------- pure rules --


def fnv1a32(text: str) -> int:
    """32-bit FNV-1a over the UTF-8 bytes. Same in Python, Rust and JS."""
    h = 0x811C9DC5
    for b in text.encode("utf-8"):
        h ^= b
        h = (h * 0x01000193) & 0xFFFFFFFF
    return h


def day_number(ms: float) -> int:
    """UTC days since 1970-01-01 for a unix time in milliseconds."""
    return int(math.floor(ms / DAY_MS))


def date_string(day: int) -> str:
    """``YYYY-MM-DD`` (UTC) of a day number."""
    d = datetime.fromtimestamp(day * 86400, tz=timezone.utc)
    return f"{d.year:04d}-{d.month:02d}-{d.day:02d}"


def daily_seed(game_id: str, day: int) -> int:
    """The seed every player gets on `day` (UTC date) for this game. A u32."""
    return fnv1a32(f"{game_id}:{date_string(day)}")


def pick_modifier(game_id: str, day: int, count: int) -> int | None:
    """Index of the day's modifier among `count`, or None when there are none."""
    if count <= 0:
        return None
    return fnv1a32(f"{game_id}:{date_string(day)}:mod") % count


def streak_next(last_day: int | None, streak: int, today: int) -> dict[str, Any]:
    """The streak after a daily run finished on `today`.

    ``counted``: a new day (the streak moved, the reward may be paid).
    A clock that went backwards counts nothing and changes nothing.
    """
    if last_day is None:
        return {"streak": 1, "counted": True}
    if today == last_day:
        return {"streak": max(streak, 1), "counted": False}
    if today == last_day + 1:
        return {"streak": max(streak, 0) + 1, "counted": True}
    if today > last_day + 1:
        return {"streak": 1, "counted": True}
    return {"streak": streak, "counted": False}


def streak_reward(reward: dict[str, Any], streak: int) -> int:
    """Coins for the first daily run of a day at this streak length."""
    if streak < 1:
        return 0
    return int(min(reward["max"], reward["base"] + reward["per_day"] * (streak - 1)))


def earn(spec: dict[str, Any], run: dict[str, Any]) -> int:
    """Coins for one finished run: floor(base + sum(per[k] * run[k])), capped.

    A run that scored nothing earns nothing. Keys are summed in sorted order so the
    float result is the same in Python, Rust and JS.
    """

    def num(key: str) -> float | None:
        v = run.get(key)
        if isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v):
            return None
        return float(v)

    score = num("score")
    if score is None or score <= 0:
        return 0
    total = float(spec["base"])
    for key in sorted(spec["per"]):
        v = num(key)
        if v is not None:
            total += spec["per"][key] * max(v, 0.0)
    return int(min(spec["cap"], max(0, math.floor(total))))


def merge_patch(doc: Any, patch: Any) -> Any:
    """RFC 7386 JSON merge patch (objects merge, everything else replaces). Returns a new value."""
    if not isinstance(patch, dict):
        return copy.deepcopy(patch)
    out = dict(doc) if isinstance(doc, dict) else {}
    for key, value in patch.items():
        if value is None:
            out.pop(key, None)
        else:
            out[key] = merge_patch(out.get(key), value)
    return out


def patch_paths(patch: Any, prefix: str = "") -> list[str]:
    """Dotted paths of every leaf of a patch (arrays and scalars are leaves)."""
    if isinstance(patch, dict) and patch:
        out: list[str] = []
        for key, value in patch.items():
            out.extend(patch_paths(value, f"{prefix}.{key}" if prefix else key))
        return out
    return [prefix] if prefix else []


def patch_allowed(patch: Any, allow: list[str]) -> bool:
    """True when every leaf of `patch` is at or below an allowed path."""
    if not isinstance(patch, dict):
        return False
    paths = patch_paths(patch)
    if not paths:
        return False
    return all(any(p == a or p.startswith(a + ".") for a in allow) for p in paths)


def apply_patch(doc: dict[str, Any], patch: dict[str, Any], allow: list[str]) -> dict[str, Any]:
    """`doc` with `patch` merged in, or `doc` unchanged when the patch strays outside `allow`."""
    if not patch_allowed(patch, allow):
        return copy.deepcopy(doc)
    return merge_patch(doc, patch)


# ---------------------------------------------------------------- validation --


def _num(value: Any, where: str, lo: float, hi: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise MetaError(f"{where} must be a number")
    if not lo <= value <= hi:
        raise MetaError(f"{where} must be {lo:g}..{hi:g}")
    return value


def _int(value: Any, where: str, lo: int, hi: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise MetaError(f"{where} must be an integer")
    if not lo <= value <= hi:
        raise MetaError(f"{where} must be {lo}..{hi}")
    return value


def _only(raw: dict[str, Any], allowed: set[str], where: str) -> None:
    extra = sorted(set(raw) - allowed)
    if extra:
        raise MetaError(f"{where} has unknown keys: {', '.join(extra)}")


def _text(raw: Any, where: str) -> dict[str, str]:
    """A label: a string (both languages) or {"en", "ja"}. Returns both."""
    if isinstance(raw, str):
        if not raw.strip() or len(raw) > 60:
            raise MetaError(f"{where} must be 1..60 characters")
        return {"en": raw.strip(), "ja": raw.strip()}
    if not isinstance(raw, dict):
        raise MetaError(f"{where} must be a string or {{en, ja}}")
    _only(raw, {"en", "ja"}, where)
    en = raw.get("en") or raw.get("ja")
    ja = raw.get("ja") or raw.get("en")
    for lang, value in (("en", en), ("ja", ja)):
        if not isinstance(value, str) or not value.strip() or len(value) > 60:
            raise MetaError(f"{where}.{lang} must be 1..60 characters")
    return {"en": en.strip(), "ja": ja.strip()}


def _patch(raw: Any, where: str, allow: list[str]) -> dict[str, Any]:
    if not isinstance(raw, dict) or not raw:
        raise MetaError(f"{where} must be a non-empty object")
    stack = [raw]
    while stack:
        cur = stack.pop()
        for key, value in cur.items():
            if value is None:
                raise MetaError(f"{where} must not delete keys (null)")
            if isinstance(value, dict):
                if not value:
                    raise MetaError(f"{where} has an empty object at {key!r}")
                stack.append(value)
    if not patch_allowed(raw, allow):
        bad = [p for p in patch_paths(raw) if not any(p == a or p.startswith(a + ".") for a in allow)]
        raise MetaError(f"{where} touches paths the game did not allow: {', '.join(bad)}")
    return raw


def normalize_meta(raw: Any) -> dict[str, Any]:
    """Validate a ``meta`` block and fill defaults. ``None`` means "no meta layer" and gives ``{}``."""
    if raw is None:
        return {}
    if not isinstance(raw, dict):
        raise MetaError("meta must be an object")
    _only(raw, {"currency", "earn", "patchable", "catalog", "daily", "modifiers", "rewarded"}, "meta")

    cur_raw = raw.get("currency", {})
    if not isinstance(cur_raw, dict):
        raise MetaError("meta.currency must be an object")
    _only(cur_raw, {"id", "name"}, "meta.currency")
    cur_id = cur_raw.get("id", "coins")
    if not isinstance(cur_id, str) or not _KEY_RE.match(cur_id):
        raise MetaError("meta.currency.id must be a lowercase word")
    currency = {"id": cur_id, "name": _text(cur_raw.get("name", {"en": "Coins", "ja": "コイン"}), "meta.currency.name")}

    earn_raw = raw.get("earn", {})
    if not isinstance(earn_raw, dict):
        raise MetaError("meta.earn must be an object")
    _only(earn_raw, {"base", "per", "cap"}, "meta.earn")
    per_raw = earn_raw.get("per", {"score": 0.02})
    if not isinstance(per_raw, dict) or len(per_raw) > MAX_PER_KEYS:
        raise MetaError(f"meta.earn.per must be an object with at most {MAX_PER_KEYS} keys")
    per: dict[str, float] = {}
    for key, weight in per_raw.items():
        if not _KEY_RE.match(key):
            raise MetaError(f"meta.earn.per key {key!r} must be a lowercase word")
        per[key] = _num(weight, f"meta.earn.per.{key}", 0, 1000)
    earn_spec = {
        "base": _num(earn_raw.get("base", 5), "meta.earn.base", 0, 1000),
        "per": per,
        "cap": _int(earn_raw.get("cap", 500), "meta.earn.cap", 1, 100_000),
    }

    allow_raw = raw.get("patchable", [])
    if not isinstance(allow_raw, list) or not all(isinstance(p, str) and _PATH_RE.match(p) for p in allow_raw):
        raise MetaError("meta.patchable must be a list of dotted spec paths (e.g. \"look\", \"course.ramp_bars\")")
    patchable = list(dict.fromkeys(allow_raw))

    items_raw = raw.get("catalog", [])
    if not isinstance(items_raw, list) or len(items_raw) > MAX_ITEMS:
        raise MetaError(f"meta.catalog must be a list of at most {MAX_ITEMS} items")
    catalog: list[dict[str, Any]] = []
    seen: set[str] = set()
    for i, item in enumerate(items_raw):
        where = f"meta.catalog[{i}]"
        if not isinstance(item, dict):
            raise MetaError(f"{where} must be an object")
        _only(item, {"id", "slot", "name", "price", "requires", "patch", "preview"}, where)
        iid = item.get("id")
        if not isinstance(iid, str) or not _ID_RE.match(iid):
            raise MetaError(f"{where}.id must be lowercase letters, digits, . _ - (max 40)")
        if iid in seen:
            raise MetaError(f"{where}.id {iid!r} is listed twice")
        seen.add(iid)
        slot = item.get("slot")
        if slot not in SLOTS:
            raise MetaError(f"{where}.slot must be one of {list(SLOTS)}")
        requires = item.get("requires")
        if requires is not None:
            if not isinstance(requires, dict):
                raise MetaError(f"{where}.requires must be an object")
            _only(requires, {"stat", "at_least"}, f"{where}.requires")
            if requires.get("stat") not in STATS:
                raise MetaError(f"{where}.requires.stat must be one of {list(STATS)}")
            requires = {
                "stat": requires["stat"],
                "at_least": _int(requires.get("at_least"), f"{where}.requires.at_least", 0, 1_000_000_000),
            }
        preview = item.get("preview", [])
        if not isinstance(preview, list) or len(preview) > 4:
            raise MetaError(f"{where}.preview must be a list of at most 4 colours")
        for c in preview:
            if not (isinstance(c, list) and len(c) == 3 and all(
                isinstance(v, int) and not isinstance(v, bool) and 0 <= v <= 255 for v in c
            )):
                raise MetaError(f"{where}.preview colours must be [r, g, b] with 0..255")
        catalog.append({
            "id": iid,
            "slot": slot,
            "name": _text(item.get("name"), f"{where}.name"),
            "price": _int(item.get("price", 0), f"{where}.price", 0, 1_000_000),
            "requires": requires,
            "patch": _patch(item.get("patch"), f"{where}.patch", patchable),
            "preview": preview,
        })
    for slot in SLOTS:
        in_slot = [c for c in catalog if c["slot"] == slot]
        if in_slot and not any(c["price"] == 0 and c["requires"] is None for c in in_slot):
            raise MetaError(f"meta.catalog slot {slot!r} needs a free item with no requirement (the default)")

    daily_raw = raw.get("daily", {})
    if not isinstance(daily_raw, dict):
        raise MetaError("meta.daily must be an object")
    _only(daily_raw, {"enabled", "seed_path", "streak_reward"}, "meta.daily")
    enabled = daily_raw.get("enabled", False)
    if not isinstance(enabled, bool):
        raise MetaError("meta.daily.enabled must be true or false")
    seed_path = daily_raw.get("seed_path", DEFAULT_SEED_PATH)
    if not isinstance(seed_path, str) or not _PATH_RE.match(seed_path):
        raise MetaError("meta.daily.seed_path must be a dotted spec path")
    sr = daily_raw.get("streak_reward", {})
    if not isinstance(sr, dict):
        raise MetaError("meta.daily.streak_reward must be an object")
    _only(sr, {"base", "per_day", "max"}, "meta.daily.streak_reward")
    reward = {
        "base": _int(sr.get("base", 10), "meta.daily.streak_reward.base", 0, 10_000),
        "per_day": _int(sr.get("per_day", 5), "meta.daily.streak_reward.per_day", 0, 10_000),
        "max": _int(sr.get("max", 50), "meta.daily.streak_reward.max", 0, 100_000),
    }
    daily = {"enabled": enabled, "seed_path": seed_path, "streak_reward": reward}

    mods_raw = raw.get("modifiers", [])
    if not isinstance(mods_raw, list) or len(mods_raw) > MAX_MODIFIERS:
        raise MetaError(f"meta.modifiers must be a list of at most {MAX_MODIFIERS}")
    modifiers: list[dict[str, Any]] = []
    mseen: set[str] = set()
    for i, mod in enumerate(mods_raw):
        where = f"meta.modifiers[{i}]"
        if not isinstance(mod, dict):
            raise MetaError(f"{where} must be an object")
        _only(mod, {"id", "name", "desc", "patch"}, where)
        mid = mod.get("id")
        if not isinstance(mid, str) or not _ID_RE.match(mid):
            raise MetaError(f"{where}.id must be lowercase letters, digits, . _ - (max 40)")
        if mid in mseen:
            raise MetaError(f"{where}.id {mid!r} is listed twice")
        mseen.add(mid)
        entry = {
            "id": mid,
            "name": _text(mod.get("name"), f"{where}.name"),
            "patch": _patch(mod.get("patch"), f"{where}.patch", patchable),
        }
        if "desc" in mod:
            entry["desc"] = _text(mod["desc"], f"{where}.desc")
        modifiers.append(entry)

    out: dict[str, Any] = {
        "version": SCHEMA_VERSION,
        "currency": currency,
        "earn": earn_spec,
        "patchable": patchable,
        "catalog": catalog,
        "daily": daily,
        "modifiers": modifiers,
    }
    rew = raw.get("rewarded")
    if rew is not None:
        if not isinstance(rew, dict):
            raise MetaError("meta.rewarded must be an object")
        _only(rew, {"slot"}, "meta.rewarded")
        slot = rew.get("slot")
        if not isinstance(slot, str) or not slot.strip():
            raise MetaError("meta.rewarded.slot must be a non-empty string")
        out["rewarded"] = {"slot": slot.strip()}
    return out
