"""CPU spark emitter. No GPU."""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

particles = load_kagra_submodule("particles")


def test_burst_is_capped_and_deterministic():
    a = particles.Emitter(max_count=4, seed=7)
    b = particles.Emitter(max_count=4, seed=7)
    assert a.burst((0.0, 1.0, 0.0), count=10) == 10
    b.burst((0.0, 1.0, 0.0), count=10)
    assert len(a) == 4
    assert a.props()[0]["position"] == b.props()[0]["position"]
    prop = a.props()[0]
    assert prop["model"] == "sphere"
    assert prop["collider"] == "none"
    assert prop["name"] == "spark"


def test_gravity_pulls_down_then_life_ends():
    em = particles.Emitter(max_count=8, seed=2)
    em.burst((0.0, 2.0, 0.0), count=1, speed=0.0, upward=0.0, gravity=-10.0, life=0.4, size=0.2)
    y0 = em.props()[0]["position"][1]
    em.tick(0.1)
    y1 = em.props()[0]["position"][1]
    assert y1 < y0
    em.tick(2.0)
    assert em.props() == []


def test_disabled_emits_nothing():
    em = particles.Emitter(max_count=8, seed=1, enabled=False)
    assert em.burst((0.0, 0.0, 0.0), count=5) == 0
    em.tick(0.1)
    assert em.props() == []


def test_trail_is_a_ribbon_of_quads():
    trail = particles.Trail(max_points=4, width=0.2)
    assert trail.props() == []
    trail.push((0.0, 1.0, 0.0))
    trail.push((0.01, 1.0, 0.0))
    assert len(trail) == 1
    trail.push((1.0, 1.4, 0.0))
    props = trail.props()
    assert len(props) == 1
    assert props[0]["model"] == "quad"
    assert props[0]["scale"][0] > 0.5
    assert props[0]["emissive"] > 0.0
    assert props[0]["collider"] == "none"
    off = particles.Trail(enabled=False)
    off.push((0.0, 0.0, 0.0))
    off.push((1.0, 0.0, 0.0))
    assert off.props() == []
