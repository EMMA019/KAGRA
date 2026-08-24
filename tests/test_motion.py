"""Tween / Sequence — GPU 不要。"""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

motion = load_kagra_submodule("motion")


class _Box:
    def __init__(self):
        self.y = 0.0


def test_animate_reaches_end():
    motion.clear_animations()
    b = _Box()
    motion.animate(b, "y", 2.0, duration=0.2)
    for _ in range(20):
        motion.tick_animations(0.02)
    assert b.y == 2.0


def test_sequence_runs_in_order():
    motion.clear_animations()
    b = _Box()
    motion.sequence(
        motion.Tween(b, "y", 1.0, duration=0.1),
        motion.Tween(b, "y", 0.0, duration=0.1),
    )
    motion.tick_animations(0.1)
    assert b.y == 1.0
    motion.tick_animations(0.1)
    assert b.y == 0.0


def test_clear_animations_stops():
    motion.clear_animations()
    b = _Box()
    motion.animate(b, "y", 4.0, duration=1.0)
    motion.clear_animations()
    motion.tick_animations(0.5)
    assert b.y == 0.0
