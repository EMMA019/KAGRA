"""HOP-BEAT spec mirror (Python side of ``kagra-shared/src/hop_beat.rs``).

The sim lives in Rust. This file owns what Python tools need: the spec with the
same defaults and bounds, the baked world file, and the beat-grid helpers used to
check the music pack. ``python -m kagra.hop_beat --check`` fails when the baked
fixture drifts from these defaults; ``--bake`` rewrites it.

A manifest's ``"game"`` object overrides the defaults, like the runner kit.
"""
from __future__ import annotations

import argparse
import copy
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "kagra-shared/tests/fixtures/hop_beat_world.json"
AUDIO_DIR = "assets/audio/hop_beat"
GAME_ID = "hop_beat"
SFX_CUES = ("hop", "land", "perfect", "good", "combo", "hit", "fall", "gameover", "revive", "newbest")
STINGERS = ("gameover", "revive", "newbest")


def default_game() -> dict[str, Any]:
    sfx = {}
    for name in SFX_CUES:
        ext = "mp3" if name in STINGERS else "wav"
        stem = f"hop_beat_{name}" if name in STINGERS else name
        sfx[name] = {"file": f"{AUDIO_DIR}/{stem}.{ext}", "rate": 1.0, "volume": 0.5 if name == "land" else 0.9}
    return {
        "id": GAME_ID,
        "title": "Hop Beat",
        "music": {"bpm": 126.0, "offset_ms": 0.0, "beats_per_bar": 4, "lead_in_bars": 2, "loop_bars": 32},
        "hero": {
            "beat_len_m": 3.0,
            "tap": {"air_beats": 1.0, "height": 0.9},
            "hold": {"air_beats": 2.0, "height": 1.6},
            "tap_window_secs": 0.11,
            "coyote_secs": 0.09,
            "buffer_secs": 0.12,
            "half_width": 0.24,
            "edge_tol": 0.35,
            "fall_gravity": 28.0,
        },
        "course": {
            "seed": 7,
            "ramp_bars": 40,
            "rest_every_bars": 8,
            "block_height": 0.5,
            "min_window_ms": 110.0,
            "phrases": [],
        },
        "judge": {
            "perfect_ms": 70.0,
            "good_ms": 130.0,
            "score_perfect": 100,
            "score_good": 60,
            "score_ok": 30,
            "combo_bonus": 25,
            "combo_cap": 8,
            "per_beat": 2.0,
            "regain_clears": 12,
        },
        "lives": {"count": 3, "invuln_secs": 1.6, "blink_hz": 10.0},
        "revive": {"enabled": True, "max_per_run": 1, "lives": 2, "invuln_secs": 2.0, "clear_beats": 4.0},
        "play": {"direct": False},
        "look": {
            "hero": [255, 214, 120],
            "slab": [58, 52, 92],
            "glow": [96, 232, 255],
            "accent": [255, 92, 170],
        },
        "audio": {
            "enabled": True,
            "master": 0.8,
            "bgm_volume": 0.7,
            "sfx_volume": 1.0,
            "bgm": {"file": f"{AUDIO_DIR}/hop_beat_loop.mp3", "volume": 0.8, "fade_secs": 0.3},
            "sfx": sfx,
        },
    }


# (path, lo, hi) — the numeric bounds validate() in Rust enforces.
BOUNDS = (
    ("music.bpm", 60, 200),
    ("music.offset_ms", -500, 500),
    ("music.beats_per_bar", 2, 8),
    ("music.lead_in_bars", 1, 16),
    ("music.loop_bars", 1, 64),
    ("hero.beat_len_m", 1, 8),
    ("hero.tap.air_beats", 0.25, 4),
    ("hero.tap.height", 0.1, 4),
    ("hero.hold.air_beats", 0.25, 4),
    ("hero.hold.height", 0.1, 4),
    ("hero.tap_window_secs", 0.02, 0.4),
    ("hero.coyote_secs", 0, 0.3),
    ("hero.buffer_secs", 0, 0.3),
    ("hero.half_width", 0.05, 1),
    ("hero.edge_tol", 0, 1),
    ("hero.fall_gravity", 5, 80),
    ("course.block_height", 0.1, 2),
    ("course.min_window_ms", 0, 400),
    ("course.ramp_bars", 1, 10**6),
    ("course.rest_every_bars", 1, 10**6),
    ("judge.perfect_ms", 5, 250),
    ("judge.good_ms", 5, 400),
    ("judge.combo_cap", 1, 64),
    ("judge.per_beat", 0, 100),
    ("lives.count", 1, 9),
    ("lives.invuln_secs", 0, 10),
    ("revive.invuln_secs", 0, 10),
    ("revive.clear_beats", 0, 32),
    ("revive.lives", 1, 9),
    ("revive.max_per_run", 0, 9),
    ("audio.master", 0, 1),
    ("audio.bgm_volume", 0, 1),
    ("audio.sfx_volume", 0, 1),
    ("audio.bgm.volume", 0, 1),
    ("audio.bgm.fade_secs", 0, 10),
)


def _merge(base: dict, over: dict, path: str = "") -> dict:
    """Deep merge; unknown keys are refused (same as ``deny_unknown_fields``)."""
    out = copy.deepcopy(base)
    for key, val in over.items():
        here = f"{path}{key}"
        if key not in base:
            if path == "audio.sfx.":
                raise ValueError(f"hop spec: audio.sfx.{key} is not a cue {SFX_CUES}")
            raise ValueError(f"hop spec: unknown field {here!r}")
        if isinstance(base[key], dict) and isinstance(val, dict):
            out[key] = _merge(base[key], val, here + ".")
        else:
            out[key] = copy.deepcopy(val)
    return out


def _get(d: dict, dotted: str) -> Any:
    for part in dotted.split("."):
        d = d[part]
    return d


def validate(game: dict[str, Any]) -> None:
    for path, lo, hi in BOUNDS:
        v = _get(game, path)
        if isinstance(v, bool) or not isinstance(v, (int, float)) or not (lo <= v <= hi):
            raise ValueError(f"hop spec: {path} must be a number in {lo}..={hi}, got {v!r}")
    if game["hero"]["tap"]["air_beats"] >= game["hero"]["hold"]["air_beats"]:
        raise ValueError("hop spec: hero.tap.air_beats must be shorter than hold")
    if game["judge"]["perfect_ms"] >= game["judge"]["good_ms"]:
        raise ValueError("hop spec: judge.perfect_ms must be below good_ms")
    for name, cue in game["audio"]["sfx"].items():
        if name not in SFX_CUES:
            raise ValueError(f"hop spec: audio.sfx.{name} is not a cue {SFX_CUES}")
        if not 0 <= cue["volume"] <= 1 or not 0.25 <= cue["rate"] <= 4:
            raise ValueError(f"hop spec: audio.sfx.{name} volume 0..=1, rate 0.25..=4")
    for ph in game["course"]["phrases"]:
        for item in ph.get("items", []):
            if item.get("kind") not in ("gap", "block") or item.get("hop") not in ("tap", "hold"):
                raise ValueError("hop spec: phrase item kind is gap|block and hop is tap|hold")


def game_from_overrides(over: dict[str, Any] | None = None) -> dict[str, Any]:
    game = _merge(default_game(), over or {})
    validate(game)
    return game


def world(over: dict[str, Any] | None = None) -> dict[str, Any]:
    """The JSON the Rust ``HopBeat::from_json`` and the page load."""
    return {"version": 1, "genre": GAME_ID, "game": game_from_overrides(over)}


def beat_secs(game: dict[str, Any]) -> float:
    return 60.0 / game["music"]["bpm"]


def loop_secs(game: dict[str, Any]) -> float:
    m = game["music"]
    return m["loop_bars"] * m["beats_per_bar"] * beat_secs(game)


def beat_times(game: dict[str, Any], count: int) -> list[float]:
    """The first ``count`` beats on the audio clock (``offset_ms`` shifts the grid)."""
    off = game["music"]["offset_ms"] / 1000.0
    return [off + i * beat_secs(game) for i in range(count)]


def check_beatgrid(game: dict[str, Any], grid: dict[str, Any], tol: float = 0.002) -> list[str]:
    """Problems between a pack's ``*_beatgrid.json`` and the spec's music block."""
    problems = []
    if abs(grid["bpm"] - game["music"]["bpm"]) > 1e-6:
        problems.append(f"bpm {grid['bpm']} != spec {game['music']['bpm']}")
    want = beat_times(game, len(grid["beat_times_s"]))
    worst = max((abs(a - b) for a, b in zip(grid["beat_times_s"], want)), default=0.0)
    if worst > tol:
        problems.append(f"beat grid drifts {worst * 1000:.2f} ms from the spec's bpm/offset")
    return problems


def bake(path: Path = FIXTURE) -> None:
    path.write_text(json.dumps(world(), indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--bake", action="store_true", help="rewrite the baked world fixture")
    ap.add_argument("--check", action="store_true", help="fail if the fixture drifts from the defaults")
    args = ap.parse_args(argv)
    if args.bake:
        bake()
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
        return 0
    baked = json.loads(FIXTURE.read_text(encoding="utf-8"))
    if baked != world():
        print("hop_beat_world.json differs from kagra.hop_beat defaults; run --bake", file=sys.stderr)
        return 1
    print("hop_beat_world.json matches")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
