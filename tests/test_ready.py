"""on_ready フック（GPU 不要）。"""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

ready = load_kagra_submodule("ready")


def test_wrap_on_ready_fires_once_before_update():
    order = []

    def on_ready():
        order.append("ready")

    def update(dt):
        order.append(("update", dt))

    wrapped = ready.wrap_on_ready(update, on_ready)
    wrapped(0.016)
    wrapped(0.016)
    assert order == ["ready", ("update", 0.016), ("update", 0.016)]


def test_wrap_on_ready_none_is_passthrough():
    def update(dt):
        return dt * 2

    assert ready.wrap_on_ready(update, None) is update
    assert ready.wrap_on_ready(None, None) is None


def test_wrap_on_ready_without_update():
    hits = []
    wrapped = ready.wrap_on_ready(None, lambda: hits.append(1))
    wrapped(0.1)
    wrapped(0.1)
    assert hits == [1]
