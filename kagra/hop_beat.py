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
SFX_CUES = ("hop", "land", "perfect", "good", "combo", "hit", "fall", "stomp", "warn", "gameover", "revive", "newbest")
CUE_VOLUME = {"hop": 0.6, "land": 0.4, "good": 0.65, "perfect": 0.85, "combo": 0.8, "warn": 0.55, "hit": 0.95}
STINGERS = ("gameover", "revive", "newbest")


ENEMY_LOOKS = ("slime", "beetle", "bat")
MAX_WALK_M = 12.0


def default_enemy() -> dict[str, Any]:
    """``EnemyType::default()``: what a type in ``enemies.types`` starts from (a slime)."""
    return {
        "id": "slime",
        "unlock_level": 0,
        "y_lo": 0.0,
        "y_hi": 0.38,
        "stompable": True,
        "step_m": 0.0,
        "steps_per_beat": 1,
        "walk_steps": 0,
        "look": "slime",
        "color": [120, 240, 150],
    }


def default_game() -> dict[str, Any]:
    sfx = {}
    for name in SFX_CUES:
        ext = "mp3" if name in STINGERS else "wav"
        stem = f"hop_beat_{name}" if name in STINGERS else name
        sfx[name] = {"file": f"{AUDIO_DIR}/{stem}.{ext}", "rate": 1.0, "volume": CUE_VOLUME.get(name, 0.9)}
    return {
        "id": GAME_ID,
        "title": "Hop Beat",
        "music": {"bpm": 126.0, "offset_ms": 0.0, "beats_per_bar": 4, "lead_in_bars": 2, "loop_bars": 32},
        "hero": {
            "beat_len_m": 3.0,
            "tap": {"air_beats": 1.0, "height": 0.9},
            "hold": {"air_beats": 2.0, "height": 1.6},
            "tap_window_secs": 0.16,
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
        "recover": {"enabled": True, "drop_secs": 0.9, "ready_beats": 3.0, "invuln_secs": 2.4, "clear_beats": 4.0},
        "fall": {"edge_stop_secs": 0.07, "tumble_turns_per_sec": 1.1, "stretch": 0.3, "carry": 0.55,
                 "cam_lag": 0.6, "slow_scale": 0.5, "card_delay_secs": 1.0, "vignette": 0.7, "dust": 8},
        "play": {"direct": False},
        "enemies": {
            "types": [
                default_enemy(),
                {**default_enemy(), "id": "beetle", "unlock_level": 1, "y_hi": 0.42, "step_m": 1.0,
                 "steps_per_beat": 2, "walk_steps": 8, "look": "beetle", "color": [255, 170, 80]},
                {**default_enemy(), "id": "bat", "unlock_level": 2, "y_lo": 1.75, "y_hi": 2.4,
                 "stompable": False, "look": "bat", "color": [190, 130, 255]},
            ],
            "body_h": 0.45,
            "stomp_depth": 0.3,
            "stomp_score": 150,
            "bounce_height": 0.7,
            "bounce_beats": 0.5,
            "rest_beats": 0.5,
        },
        "look": {
            "hero": [255, 214, 120],
            "slab": [58, 52, 92],
            "glow": [96, 232, 255],
            "accent": [255, 92, 170],
            "preset": "neon_night",
            "post": {},
            "atmo": {},
        },
        "skin": "classic",
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
    ("recover.drop_secs", 0.2, 3),
    ("recover.ready_beats", 0, 8),
    ("recover.invuln_secs", 0, 10),
    ("recover.clear_beats", 0, 32),
    ("fall.edge_stop_secs", 0, 0.5),
    ("fall.tumble_turns_per_sec", 0, 4),
    ("fall.stretch", 0, 1),
    ("fall.carry", 0, 1),
    ("fall.cam_lag", 0, 1),
    ("fall.slow_scale", 0.1, 1),
    ("fall.card_delay_secs", 0.3, 3),
    ("fall.vignette", 0, 1),
    ("fall.dust", 0, 32),
    ("enemies.body_h", 0.1, 2),
    ("enemies.stomp_depth", 0.05, 1),
    ("enemies.stomp_score", 0, 10**6),
    ("enemies.bounce_height", 0, 3),
    ("enemies.bounce_beats", 0, 4),
    ("enemies.rest_beats", 0, 4),
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
        if here == "enemies.types":
            out[key] = _merge_types(val)
        elif isinstance(base[key], dict) and isinstance(val, dict):
            out[key] = _merge(base[key], val, here + ".")
        else:
            out[key] = copy.deepcopy(val)
    return out


def _merge_types(types: Any) -> list[dict[str, Any]]:
    """Each type is laid over ``default_enemy()``; unknown keys are refused."""
    if not isinstance(types, list):
        raise ValueError("hop spec: enemies.types must be a list")
    base = default_enemy()
    out = []
    for i, t in enumerate(types):
        if not isinstance(t, dict):
            raise ValueError(f"hop spec: enemies.types[{i}] must be an object")
        for key in t:
            if key not in base:
                raise ValueError(f"hop spec: unknown field 'enemies.types[{i}].{key}'")
        out.append({**base, **copy.deepcopy(t)})
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
    _validate_look(game)
    _validate_enemies(game)
    for pi, ph in enumerate(game["course"]["phrases"]):
        w = ph.get("weight", 1)
        if isinstance(w, bool) or not isinstance(w, int) or not 1 <= w <= 16:
            raise ValueError(f"hop spec: phrase {pi} weight must be an integer in 1..=16")
        for item in ph.get("items", []):
            if item.get("kind") not in ("gap", "block", "enemy") or item.get("hop") not in ("tap", "hold"):
                raise ValueError("hop spec: phrase item kind is gap|block|enemy and hop is tap|hold")


def _num(name: str, v: Any, lo: float, hi: float) -> None:
    if isinstance(v, bool) or not isinstance(v, (int, float)) or not lo <= v <= hi:
        raise ValueError(f"hop spec: {name} must be a number in {lo}..={hi}, got {v!r}")


LOOK_PRESETS = ("neon_night", "dusk", "plain")
SKINS = ("classic", "mint", "gold", "ember", "ghost")
LUT_NAMES = ("off", "warm", "cool", "punch")
POST_KEYS = {"bloom": (0, 2), "bloom_threshold": (0, 2), "vignette": (0, 2), "lut_strength": (0, 1),
             "exposure": (0.3, 2), "ibl": (0, 2)}
ATMO_KEYS = {"fog_start": (0, 400), "fog_end": (1, 400), "horizon_glow": (0, 4), "twinkle": (0, 2),
             "stars": (0, 200), "clouds": (0, 12)}


def _validate_look(game: dict[str, Any]) -> None:
    look = game["look"]
    if game["skin"] not in SKINS:
        raise ValueError(f"hop spec: skin is {'|'.join(SKINS)}")
    if look["preset"] not in LOOK_PRESETS:
        raise ValueError(f"hop spec: look.preset is {'|'.join(LOOK_PRESETS)}")
    post, atmo = look["post"], look["atmo"]
    for sect, keys, extra in (("post", POST_KEYS, {"lut", "tonemap_curve"}), ("atmo", ATMO_KEYS, {"sky", "horizon"})):
        for k, v in look[sect].items():
            if k in keys:
                _num(f"look.{sect}.{k}", v, *keys[k])
            elif k not in extra:
                raise ValueError(f"hop spec: look.{sect}.{k} is not a look setting")
    if post.get("lut", "off") not in LUT_NAMES:
        raise ValueError(f"hop spec: look.post.lut is {'|'.join(LUT_NAMES)}")
    if post.get("tonemap_curve", "") not in ("", "agx", "neutral"):
        raise ValueError("hop spec: look.post.tonemap_curve is agx|neutral or empty")
    for k in ("sky", "horizon"):
        c = atmo.get(k)
        if c is not None and not (isinstance(c, list) and len(c) == 3 and all(isinstance(v, int) and 0 <= v <= 255 for v in c)):
            raise ValueError(f"hop spec: look.atmo.{k} is three 0..=255 integers")
    if "fog_start" in atmo and "fog_end" in atmo and atmo["fog_end"] <= atmo["fog_start"]:
        raise ValueError("hop spec: look.atmo.fog_end must be above fog_start")


def _validate_enemies(game: dict[str, Any]) -> None:
    types = game["enemies"]["types"]
    if len(types) > 8:
        raise ValueError("hop spec: enemies.types holds at most 8 kinds")
    seen: set[str] = set()
    for i, t in enumerate(types):
        n = f"enemies.types[{i}]"
        eid = t["id"]
        if not (isinstance(eid, str) and 1 <= len(eid) <= 16 and all(c in "abcdefghijklmnopqrstuvwxyz0123456789_" for c in eid)):
            raise ValueError(f"hop spec: {n}.id is 1..=16 of a-z 0-9 _")
        if eid in seen:
            raise ValueError(f"hop spec: {n}.id {eid!r} is used twice")
        seen.add(eid)
        _num(f"{n}.unlock_level", t["unlock_level"], 0, 20)
        _num(f"{n}.y_lo", t["y_lo"], 0, 4)
        _num(f"{n}.y_hi", t["y_hi"], 0.05, 4)
        if t["y_hi"] <= t["y_lo"]:
            raise ValueError(f"hop spec: {n}.y_hi must be above y_lo")
        if t["stompable"] and t["y_lo"] > 0:
            raise ValueError(f"hop spec: {n}: only an enemy on the slab can be stomped")
        _num(f"{n}.step_m", t["step_m"], 0, 4)
        _num(f"{n}.steps_per_beat", t["steps_per_beat"], 1, 4)
        _num(f"{n}.walk_steps", t["walk_steps"], 0, 16)
        if t["step_m"] * t["walk_steps"] > MAX_WALK_M:
            raise ValueError(f"hop spec: {n}: step_m * walk_steps must be at most {MAX_WALK_M} m")
        if t["look"] not in ENEMY_LOOKS:
            raise ValueError(f"hop spec: {n}.look is {'|'.join(ENEMY_LOOKS)}")
        if len(t["color"]) != 3 or any(not isinstance(c, int) or not 0 <= c <= 255 for c in t["color"]):
            raise ValueError(f"hop spec: {n}.color is 0..=255 per channel")


def phrase_problems(game: dict[str, Any], phrases: list[dict[str, Any]] | None = None) -> list[str]:
    """The placement rules that need no simulation, for a phrase list (default: the spec's).

    Unknown enemy ids, the level an enemy unlocks at, the rest kept around an enemy and the
    bounce reserve after a stompable one. The timing windows (can the allowed inputs clear
    it?) are checked by the Rust sim: ``HopSpec::course_problems`` runs on every load.
    """
    out: list[str] = []
    types = {t["id"]: t for t in game["enemies"]["types"]}
    rest = game["enemies"]["rest_beats"]
    bounce = game["enemies"]["bounce_beats"]
    bpb = game["music"]["beats_per_bar"]
    for pi, ph in enumerate(game["course"]["phrases"] if phrases is None else phrases):
        end = ph["bars"] * bpb
        free_from, prev_enemy = 0.0, False
        for ii, it in enumerate(ph.get("items", [])):
            where = f"phrase {pi} item {ii}"
            enemy = it.get("enemy", "")
            if it["kind"] != "enemy":
                if enemy:
                    out.append(f"{where}: only kind enemy names an enemy")
            elif enemy not in types:
                out.append(f"{where}: unknown enemy {enemy!r}")
                continue
            else:
                t = types[enemy]
                if ph["level"] < t["unlock_level"]:
                    out.append(f"{where}: {enemy} is unlocked at level {t['unlock_level']} but the phrase is level {ph['level']}")
                if t["y_lo"] > 0:  # a flyer needs no press
                    continue
            air = game["hero"][it["hop"]]["air_beats"]
            is_enemy = it["kind"] == "enemy"
            gap = rest if (is_enemy or prev_enemy) else 0.0
            if it["beat"] + 0.001 < free_from + gap:
                out.append(f"{where}: starts before the last hop lands" + (" (plus rest_beats)" if gap else ""))
            stompable = is_enemy and types[enemy]["stompable"]
            free_from = it["beat"] + air + (bounce if stompable else 0.0)
            prev_enemy = is_enemy
            if free_from > end + 0.001:
                out.append(f"{where}: lands after the phrase ends")
    return out + flyer_gap_problems(game, phrases)


def flyer_gap_problems(game: dict[str, Any], phrases: list[dict[str, Any]] | None = None) -> list[str]:
    """No flyer (a bat) over a gap: the span of the hop across the gap, plus 100 ms of press
    slack either way, must be clear of it. A bat above a hole leaves a thumb that stays down
    a moment too long (a tap that becomes a hold) nowhere to go."""
    out: list[str] = []
    types = {t["id"]: t for t in game["enemies"]["types"]}
    slack = 0.1 / beat_secs(game)
    for pi, ph in enumerate(game["course"]["phrases"] if phrases is None else phrases):
        items = ph.get("items", [])
        for fi, f in enumerate(items):
            t = types.get(f.get("enemy", ""))
            if f.get("kind") != "enemy" or t is None or t["y_lo"] <= 0:
                continue
            c = f["beat"] + game["hero"][f["hop"]]["air_beats"] * 0.5
            f0, f1 = c - f["beats"] * 0.5, c + f["beats"] * 0.5
            for gi, g in enumerate(items):
                if g.get("kind") != "gap":
                    continue
                air = game["hero"][g["hop"]]["air_beats"]
                if f0 < g["beat"] + air + slack and f1 > g["beat"] - slack:
                    out.append(f"phrase {pi} item {fi}: a flyer over the gap of item {gi} (keep it clear of the hop across the gap)")
    return out


def game_from_overrides(over: dict[str, Any] | None = None) -> dict[str, Any]:
    over = dict(over or {})
    tiers = over.pop("quality_tiers", None)  # the session strips it before the kit loader sees the game
    if tiers is not None:
        from kagra import quality

        quality.resolve(tiers)  # refuses unknown keys and out-of-range numbers, like Rust
    game = _merge(default_game(), over)
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
