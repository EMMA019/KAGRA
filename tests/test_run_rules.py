"""The shared run rules (lives, goals, result) match the Rust core, step by step."""

import json
from pathlib import Path

import pytest

from kagra.game_spec import LivesSpec, MissionsSpec, _build
from kagra.run_rules import HURT_IDLE, Lives, MissionTracker, ResultState

ROOT = Path(__file__).resolve().parents[1]
VECTORS = json.loads((ROOT / "kagra-shared/tests/fixtures/run_rules_vectors.json").read_text(encoding="utf-8"))


@pytest.mark.parametrize("case", VECTORS["lives"], ids=lambda c: c["name"])
def test_lives_vectors(case):
    spec = LivesSpec(**case["spec"])
    lives = Lives()
    for step in case["steps"]:
        op = step["op"]
        if op == "reset":
            lives.reset(spec)
        elif op == "tick":
            lives.tick(step["dt"])
        elif op == "hit":
            assert lives.hit(spec, step["last_chance"]) == step["expect"]
        elif op == "regain":
            assert lives.regain(spec) is step["expect"]
        elif op == "state":
            assert lives.left == step["left"]
            assert lives.invuln == pytest.approx(step["invuln"], abs=1e-4)
            assert lives.hurt_t == pytest.approx(step["hurt_t"], abs=1e-4)
            assert lives.protected is step["protected"]
            if "blink" in step:
                assert lives.blink_shown(spec) is step["blink"]
        else:  # pragma: no cover
            raise AssertionError(op)


@pytest.mark.parametrize("case", VECTORS["missions"], ids=lambda c: c["name"])
def test_mission_vectors(case):
    pool = MissionsSpec() if case["spec"] is None else _build(MissionsSpec, case["spec"], MissionsSpec())
    active = MissionsSpec(**{**pool.__dict__, "list": pool.active(case["perch"], case["decks"]), "draw": 0})
    tracker = MissionTracker()
    for step in case["steps"]:
        assert tracker.update(active, step["counts"]) == step["fresh"]
        assert tracker.completed() == step["completed"]
        if "json" in step:
            assert tracker.json(active, step["counts"]) == step["json"]


def test_hurt_idle_matches_rust():
    src = (ROOT / "kagra-shared/src/game_core/lives.rs").read_text(encoding="utf-8")
    assert f"pub const HURT_IDLE: f32 = {HURT_IDLE};" in src


def test_result_state():
    r = ResultState()
    assert not r.take_gameover(0.0)
    r.crashed()
    assert not r.take_gameover(0.2) and r.take_gameover(0.0) and not r.take_gameover(0.0)
    r.announce("MISSION  COINS", 2.2)
    r.tick(1.0)
    assert r.banner_t == pytest.approx(1.2)
    r.reset()
    assert r.banner_t == 0.0
    assert ResultState.best_shown(500, 120) == 500 and ResultState.best_shown(500, 900) == 900
