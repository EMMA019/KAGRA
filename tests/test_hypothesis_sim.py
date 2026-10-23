"""Property tests for the pure sim. Skipped unless hypothesis is installed.

Loaded through ``tests.conftest.load_kagra_submodule`` so this file does not
import ``kagra/__init__.py`` or the Rust extension. Desktop and CI only.
"""

from __future__ import annotations

import math

import pytest

pytest.importorskip("hypothesis")

from hypothesis import assume, example, given, settings, strategies as st

from tests.conftest import load_kagra_submodule

axes = load_kagra_submodule("axes")
controller = load_kagra_submodule("controller")
event_bus = load_kagra_submodule("event_bus")

finite = st.floats(min_value=-40.0, max_value=40.0, allow_nan=False, allow_infinity=False)
vec3 = st.tuples(finite, finite, finite)


def _forward(eye, target):
    delta = (target[0] - eye[0], target[1] - eye[1], target[2] - eye[2])
    dist = math.sqrt(delta[0] ** 2 + delta[1] ** 2 + delta[2] ** 2)
    if dist < 1e-8:
        return delta, 0.0
    return (delta[0] / dist, delta[1] / dist, delta[2] / dist), dist


@settings(max_examples=120, deadline=None)
@given(eye=vec3, target=vec3)
@example((0.0, 2.0, 6.0), (0.0, 0.0, 0.0))
@example((0.0, 2.0, -6.0), (0.0, 0.0, 0.0))
def test_screen_right_follows_the_look_and_the_stick(eye, target):
    """+X is screen-right only while the camera looks toward -Z.

    ``screen_left`` is the negation. ``+lx`` moves toward screen-right and
    ``+lz`` moves into the frame. A chase camera looking along +Z has
    ``screen_right.x < 0``.
    """
    forward, dist = _forward(eye, target)
    assume(dist > 1e-3)
    right = axes.screen_right(eye, target)
    left = axes.screen_left(eye, target)
    assert math.isclose(math.sqrt(sum(c * c for c in right)), 1.0, abs_tol=1e-6)
    for component, opposite in zip(left, right):
        assert math.isclose(component, -opposite, abs_tol=1e-6)

    # Straight up or down uses the basis fallback. The ±Z rule is for a
    # camera that still has a horizontal look.
    if abs(forward[1]) < 0.9:
        if forward[2] < -0.05:
            assert right[0] > 0.0
        if forward[2] > 0.05:
            assert right[0] < 0.0

    if math.hypot(right[0], right[2]) > 0.2:
        toward = axes.wish_world(1.0, 0.0, eye, target)
        away = axes.wish_world(-1.0, 0.0, eye, target)
        assert toward[0] * right[0] + toward[1] * right[2] > 0.0
        assert away[0] * right[0] + away[1] * right[2] < 0.0
    if abs(forward[1]) < 0.9 and math.hypot(forward[0], forward[2]) > 0.2:
        into = axes.wish_world(0.0, 1.0, eye, target)
        assert into[0] * forward[0] + into[1] * forward[2] > 0.0


@settings(max_examples=80, deadline=None)
@given(
    jump=st.floats(min_value=0.05, max_value=20.0, allow_nan=False, allow_infinity=False),
    on_ground=st.booleans(),
    coyote=st.booleans(),
    in_water=st.booleans(),
)
def test_jump_vy_is_ground_coyote_or_water(jump, on_ground, coyote, in_water):
    vy = controller.jump_vy(on_ground, in_water, jump, coyote=coyote)
    if in_water:
        assert vy == pytest.approx(jump * 0.42)
    elif on_ground or coyote:
        assert vy == pytest.approx(jump)
    else:
        assert vy is None


@settings(max_examples=40, deadline=None)
@given(jump=st.floats(min_value=-8.0, max_value=0.0, allow_nan=False, allow_infinity=False))
def test_nonpositive_jump_does_nothing(jump):
    assert controller.jump_vy(True, True, jump, coyote=True) is None


class _Body:
    def __init__(self):
        self.vx = 0.0
        self.vy = 0.0
        self.vz = 0.0
        self.on_ground = False


@settings(max_examples=40, deadline=None)
@given(
    jump=st.floats(min_value=0.5, max_value=12.0, allow_nan=False, allow_infinity=False),
    dt=st.floats(min_value=1.0 / 120.0, max_value=1.0 / 30.0, allow_nan=False, allow_infinity=False),
)
def test_landing_is_reported_on_one_frame(jump, dt):
    motor = controller.CharacterController(jump=jump)
    body = _Body()
    body.on_ground = True
    motor.apply(body, dt)
    assert motor.landed is False

    motor.try_jump()
    motor.apply(body, dt)
    assert body.vy == pytest.approx(jump)
    assert body.on_ground is False

    motor.apply(body, dt)
    assert motor.landed is False
    assert body.vy == pytest.approx(jump)

    body.on_ground = True
    motor.apply(body, dt)
    assert motor.landed is True
    motor.apply(body, dt)
    assert motor.landed is False


@settings(max_examples=40, deadline=None)
@given(times=st.integers(min_value=1, max_value=24))
def test_event_once_fires_on_the_first_emit_only(times):
    bus = event_bus.EventBus()
    hits = []
    bus.once("go", lambda data: hits.append(data.get("_event")))
    for _ in range(times):
        bus.emit("go", {})
    assert hits == ["go"]
