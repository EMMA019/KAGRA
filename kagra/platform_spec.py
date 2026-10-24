"""Web-portal settings of a game: the ``platform`` block of ``games/<name>.json``.

Today it holds ``ads``: where a portal's midgame ad may show, which rewarded
slots exist, and the shortest gap between two ads. The browser page reads it
from ``bundle.json`` (``scripts/preview_site/portal.js`` normalises it again,
leniently, so a bad value there never stops the game).

It lives in the manifest layer, not in ``GameSpec`` / ``RunnerSpec``: an ad
break is a page and portal matter, not a rule of the simulation, and putting it
in the spec would change every baked fixture and the Rust mirror for no gain.
A rewarded slot may name an engine effect in ``rewarded[].effect``: ``"none"`` (the
page only gets the callback) or ``"revive"`` (the game offers Continue after game
over and the page calls the session's ``revive()`` once the ad finished; HOP-BEAT).

    "platform": {
      "ads": {
        "commercial_on": ["retry"],
        "rewarded": [],
        "min_interval_secs": 90
      }
    }
"""
from __future__ import annotations

from typing import Any

COMMERCIAL_SLOTS = ("game_over", "retry")
REWARDED_AT = ("game_over", "title")
REWARDED_EFFECTS = ("none", "revive")
MIN_INTERVAL_MAX = 3600

DEFAULT_ADS: dict[str, Any] = {
    "commercial_on": ["retry"],
    "rewarded": [],
    "min_interval_secs": 90,
}


class PlatformError(ValueError):
    """A ``platform`` block that cannot be used as written."""


def normalize_ads(raw: Any) -> dict[str, Any]:
    """Validate ``platform.ads`` and fill defaults. ``None`` gives the defaults."""
    if raw is None:
        return {**DEFAULT_ADS, "commercial_on": list(DEFAULT_ADS["commercial_on"]), "rewarded": []}
    if not isinstance(raw, dict):
        raise PlatformError("platform.ads must be an object")
    unknown = sorted(set(raw) - set(DEFAULT_ADS))
    if unknown:
        raise PlatformError(f"platform.ads has unknown keys: {', '.join(unknown)}")

    on = raw.get("commercial_on", DEFAULT_ADS["commercial_on"])
    if not isinstance(on, list) or not all(isinstance(s, str) for s in on):
        raise PlatformError("platform.ads.commercial_on must be a list of strings")
    bad = [s for s in on if s not in COMMERCIAL_SLOTS]
    if bad:
        raise PlatformError(
            f"platform.ads.commercial_on has unknown slots {bad}; use {list(COMMERCIAL_SLOTS)}"
        )
    on = list(dict.fromkeys(on))

    rewarded_raw = raw.get("rewarded", [])
    if not isinstance(rewarded_raw, list):
        raise PlatformError("platform.ads.rewarded must be a list")
    rewarded: list[dict[str, Any]] = []
    seen: set[str] = set()
    for i, item in enumerate(rewarded_raw):
        where = f"platform.ads.rewarded[{i}]"
        if not isinstance(item, dict):
            raise PlatformError(f"{where} must be an object")
        extra = sorted(set(item) - {"slot", "at", "effect", "per_run"})
        if extra:
            raise PlatformError(f"{where} has unknown keys: {', '.join(extra)}")
        slot = item.get("slot")
        if not isinstance(slot, str) or not slot.strip():
            raise PlatformError(f"{where}.slot must be a non-empty string")
        if slot in seen:
            raise PlatformError(f"{where}.slot {slot!r} is listed twice")
        seen.add(slot)
        at = item.get("at", "game_over")
        if at not in REWARDED_AT:
            raise PlatformError(f"{where}.at must be one of {list(REWARDED_AT)}")
        effect = item.get("effect", "none")
        if effect not in REWARDED_EFFECTS:
            raise PlatformError(
                f"{where}.effect {effect!r} is not available (valid: {list(REWARDED_EFFECTS)})"
            )
        per_run = item.get("per_run", 1)
        if isinstance(per_run, bool) or not isinstance(per_run, int) or not 1 <= per_run <= 9:
            raise PlatformError(f"{where}.per_run must be an integer from 1 to 9")
        rewarded.append({"slot": slot.strip(), "at": at, "effect": effect, "per_run": per_run})

    gap = raw.get("min_interval_secs", DEFAULT_ADS["min_interval_secs"])
    if isinstance(gap, bool) or not isinstance(gap, (int, float)) or not 0 <= gap <= MIN_INTERVAL_MAX:
        raise PlatformError(
            f"platform.ads.min_interval_secs must be a number from 0 to {MIN_INTERVAL_MAX}"
        )
    return {"commercial_on": on, "rewarded": rewarded, "min_interval_secs": gap}


def normalize_platform(raw: Any) -> dict[str, Any]:
    """Validate the whole ``platform`` block. ``None`` means "no portal settings"."""
    if raw is None:
        return {}
    if not isinstance(raw, dict):
        raise PlatformError("platform must be an object")
    unknown = sorted(set(raw) - {"ads"})
    if unknown:
        raise PlatformError(f"platform has unknown keys: {', '.join(unknown)}")
    out: dict[str, Any] = {}
    if "ads" in raw:
        out["ads"] = normalize_ads(raw["ads"])
    return out
