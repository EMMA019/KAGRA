"""Graphics tier spec mirror (Python side of ``kagra-shared/src/quality.rs``).

The tiers (safe / mid / high / max) and the auto-selection tuning live in one JSON file,
``kagra-shared/src/quality_tiers.json``, which the Rust session includes at build time and the page
reads through ``qualityJson``. This file has the same defaults and bounds for Python tools and
validates a game's ``quality_tiers`` override the way Rust does (unknown keys refused).
``python -m kagra.quality --check`` fails when the defaults below drift from the JSON file.

A manifest's ``"game"`` object may carry a top-level ``quality_tiers`` object laid over the defaults,
key by key.
"""
from __future__ import annotations

import argparse
import copy
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "kagra-shared/src/quality_tiers.json"
TIER_NAMES = ("safe", "mid", "high", "max")
PICTURES = ("legacy", "low", "mobile", "high")

_FX = {"fx_post": True, "fx_atmo": True}


def _tier(scale, picture, ssao, taa, cascades, pcss, softness, bloom, atmo, particles) -> dict[str, Any]:
    return {
        "scale": scale, "picture": picture, "ssao": ssao, "taa": taa, "shadow_cascades": cascades,
        "shadow_pcss": pcss, "shadow_softness": softness, "bloom_scale": bloom, "atmo": atmo,
        "particles": particles, "fx": dict(_FX),
    }


def default_spec() -> dict[str, Any]:
    """``QualitySpec::builtin()``."""
    return {
        "tiers": {
            "safe": _tier(1.0, "low", 0.0, False, 1, False, 1.0, 0.0, 0.3, 0.4),
            "mid": _tier(1.5, "legacy", 0.0, False, 1, False, 1.0, 1.0, 0.75, 0.75),
            "high": _tier(2.0, "legacy", 0.0, False, 2, False, 1.5, 1.0, 1.0, 1.0),
            "max": _tier(2.5, "legacy", 4.0, True, 3, True, 2.0, 1.0, 1.4, 1.0),
        },
        "auto": {
            "start": "mid", "ceiling": "high", "probe_secs": 3.0, "settle_secs": 1.0,
            "good_p95_ms": 17.5, "good_p99_ms": 22.0, "down_drop_pct": 6.0, "down_secs": 3.0,
            "up_secs": 25.0, "cooldown_secs": 8.0, "ban_secs": 180.0,
        },
    }


# (path, lo, hi): the same numbers as `Tier::validate` / `QualitySpec::validate`
TIER_BOUNDS = (
    ("scale", 0.5, 4.0), ("ssao", 0.0, 16.0), ("shadow_cascades", 1, 3), ("shadow_softness", 0.25, 8.0),
    ("bloom_scale", 0.0, 2.0), ("atmo", 0.0, 2.0), ("particles", 0.0, 1.0),
)
AUTO_BOUNDS = (
    ("probe_secs", 1.0, 30.0), ("settle_secs", 0.0, 10.0), ("good_p95_ms", 8.0, 100.0),
    ("good_p99_ms", 8.0, 200.0), ("down_drop_pct", 0.5, 100.0), ("down_secs", 1.0, 60.0),
    ("up_secs", 5.0, 3600.0), ("cooldown_secs", 1.0, 600.0), ("ban_secs", 0.0, 86400.0),
)


def _merge(base: dict, over: dict, path: str = "") -> dict:
    """Deep merge; unknown keys are refused (same as ``deny_unknown_fields``)."""
    if not isinstance(over, dict):
        raise ValueError(f"quality_tiers: {path.rstrip('.') or 'quality_tiers'} must be an object")
    out = copy.deepcopy(base)
    for key, val in over.items():
        here = f"{path}{key}"
        if key not in base:
            raise ValueError(f"quality_tiers: unknown field {here!r}")
        if here.endswith(".fx"):  # a free-form switch map: any name, boolean values
            if not isinstance(val, dict):
                raise ValueError(f"quality_tiers: {here} must be an object")
            out[key] = {**out[key], **copy.deepcopy(val)}
        elif isinstance(base[key], dict) and isinstance(val, dict):
            out[key] = _merge(base[key], val, here + ".")
        else:
            out[key] = copy.deepcopy(val)
    return out


def _num(name: str, v: Any, lo: float, hi: float) -> None:
    if isinstance(v, bool) or not isinstance(v, (int, float)) or not lo <= v <= hi:
        raise ValueError(f"quality_tiers: {name} must be a number in {lo}..={hi}, got {v!r}")


def validate(spec: dict[str, Any]) -> None:
    tiers = spec["tiers"]
    for name in TIER_NAMES:
        if name not in tiers:
            raise ValueError(f"quality_tiers: tier {name!r} is missing")
    for name in tiers:
        if name not in TIER_NAMES:
            raise ValueError(f"quality_tiers: unknown tier {name!r}")
    for name in TIER_NAMES:
        t = tiers[name]
        for key, lo, hi in TIER_BOUNDS:
            _num(f"{name}.{key}", t[key], lo, hi)
        if t["picture"] not in PICTURES:
            raise ValueError(f"quality_tiers: {name}.picture must be {'|'.join(PICTURES)}, got {t['picture']!r}")
        for key in ("taa", "shadow_pcss"):
            if not isinstance(t[key], bool):
                raise ValueError(f"quality_tiers: {name}.{key} must be true or false")
        if isinstance(t["shadow_cascades"], float) and not t["shadow_cascades"].is_integer():
            raise ValueError(f"quality_tiers: {name}.shadow_cascades must be a whole number")
        for k, v in t["fx"].items():
            if not k or len(k) > 32:
                raise ValueError(f"quality_tiers: {name}.fx has a bad switch name")
            if not isinstance(v, bool):
                raise ValueError(f"quality_tiers: {name}.fx.{k} must be true or false")
    a = spec["auto"]
    if a["start"] not in TIER_NAMES or a["ceiling"] not in TIER_NAMES:
        raise ValueError("quality_tiers: auto.start and auto.ceiling must be tier names")
    if TIER_NAMES.index(a["start"]) > TIER_NAMES.index(a["ceiling"]) and a["start"] != "max":
        raise ValueError("quality_tiers: auto.start must not be above auto.ceiling")
    for key, lo, hi in AUTO_BOUNDS:
        _num(f"auto.{key}", a[key], lo, hi)
    scales = [tiers[n]["scale"] for n in TIER_NAMES]
    if any(x > y for x, y in zip(scales, scales[1:])):
        raise ValueError("quality_tiers: scale must not fall from safe to max")


def resolve(over: dict[str, Any] | None = None) -> dict[str, Any]:
    """The defaults with a game's ``quality_tiers`` laid over them, validated."""
    spec = _merge(default_spec(), over or {})
    validate(spec)
    return spec


def check(path: Path = SOURCE) -> list[str]:
    """Differences between the defaults here and the JSON file Rust includes (empty when equal)."""
    shipped = json.loads(path.read_text(encoding="utf-8"))
    mine = default_spec()
    problems: list[str] = []
    for name in TIER_NAMES:
        for key in sorted(set(mine["tiers"][name]) | set(shipped["tiers"].get(name, {}))):
            a, b = mine["tiers"][name].get(key), shipped["tiers"].get(name, {}).get(key)
            if a != b:
                problems.append(f"tiers.{name}.{key}: python {a!r}, json {b!r}")
    for key in sorted(set(mine["auto"]) | set(shipped["auto"])):
        a, b = mine["auto"].get(key), shipped["auto"].get(key)
        if a != b:
            problems.append(f"auto.{key}: python {a!r}, json {b!r}")
    if set(shipped) != {"tiers", "auto"}:
        problems.append(f"top-level keys are {sorted(shipped)}")
    return problems


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--check", action="store_true", help="fail if the defaults drift from quality_tiers.json")
    args = ap.parse_args(argv)
    validate(default_spec())
    if args.check:
        problems = check()
        for p in problems:
            print(p, file=sys.stderr)
        return 1 if problems else 0
    print(json.dumps(default_spec(), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
