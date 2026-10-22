"""FishCookSession: cast, reel, cook, sell until the day goal. GPU optional."""
from __future__ import annotations

import json
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "kagra-shared/tests/fixtures/fishcook_world.json"


def _session():
    try:
        from kagra.kagra_shared import FishCookSession
    except ImportError:
        try:
            from kagra_shared import FishCookSession
        except ImportError:
            pytest.skip("kagra_shared not built")
    return FishCookSession


def _prop_xz(world: dict, name: str) -> tuple[float, float]:
    for prop in world["props"]:
        if prop.get("name") == name:
            pos = prop["position"]
            return float(pos[0]), float(pos[2])
    raise AssertionError(name)


def _steer(world: dict, tx: float, tz: float) -> tuple[float, float]:
    pos = world["player"]["position"]
    dx, dz = tx - float(pos[0]), tz - float(pos[2])
    dist = (dx * dx + dz * dz) ** 0.5
    if dist < 0.2:
        return 0.0, 0.0
    return dx / dist, dz / dist


def test_fishcook_fixture_uses_bundled_glbs():
    data = json.loads(FIXTURE.read_text(encoding="utf-8"))
    assert data["genre"] == "fishcook"
    assert data["player"]["on_ground"] is True
    assert data["player"]["gltf"] == "knight"
    gltfs = {p.get("gltf") for p in data["props"]}
    for name in ("gazebo", "mill", "chest", "inn", "stylized_tree", "stylized_pine", "rock", "crate.glb"):
        assert name in gltfs
    assert any(t.get("name") == "day" for t in data["timers"])
    assert any(t.get("name") == "goal" and t.get("seconds") == 8 for t in data["timers"])


def test_fishcook_session_day_goal_and_ascii_hud():
    session_cls = _session()
    session = session_cls.from_json(FIXTURE.read_text(encoding="utf-8"))
    assert session.genre_id == "fishcook"
    assert session.phase == "title"
    title = json.loads(session.hud_json(960, 540))
    for text in title["texts"]:
        assert text["text"].isascii()
        assert "\u2014" not in text["text"]
    session.confirm()
    assert session.phase == "playing"
    world = json.loads(session.dump())
    stove = _prop_xz(world, "stove")
    stall = _prop_xz(world, "stall")
    dock = _prop_xz(world, "dock")
    for _ in range(5000):
        if session.won:
            break
        world = json.loads(session.dump())
        beat = session.beat
        if beat == "reel":
            marker = float(session.reel_marker)
            attack = session.reel_lo <= marker <= session.reel_hi
            lx = lz = 0.0
        elif beat == "cook":
            heat = float(session.heat)
            attack = session.heat_lo <= heat <= session.heat_hi
            lx = lz = 0.0
        elif beat == "catch":
            lx, lz = _steer(world, *stove)
            attack = False
        elif beat == "cooked":
            if session.at_stall:
                lx = lz = 0.0
                attack = True
            else:
                lx, lz = _steer(world, *stall)
                attack = False
        elif beat == "player":
            if session.on_dock:
                lx = lz = 0.0
                attack = True
            else:
                lx, lz = _steer(world, *dock)
                attack = False
        else:
            lx = lz = 0.0
            attack = False
        session.set_input(lx, lz, False, attack, False)
        session.tick(1.0 / 60.0)
        hud = json.loads(session.hud_json(320, 180))
        for text in hud["texts"]:
            assert text["text"].isascii()
    assert session.won
    assert session.phase == "complete"
    assert session.coins == session.goal
    assert session.sold >= 2
    result = json.loads(session.hud_json(640, 360))
    blob = " ".join(t["text"] for t in result["texts"])
    assert "DAY END" in blob
    assert "goal met" in blob
    sold = session.take_events("sold")
    assert len(sold) == 1
