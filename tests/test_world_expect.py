"""GPU-free WorldDoc expect (genre + query)."""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

we = load_kagra_submodule("world_expect")


def test_genre_mismatch():
    err = we.eval_world_expect({"version": 1, "props": []}, {"genre": "town_gate"})
    assert any("genre" in e for e in err)


def test_no_genre_is_ok_when_unspecified():
    assert we.eval_world_expect({"version": 1, "props": []}, {}) == []


def test_query_counts_props():
    dump = {
        "version": 1,
        "props": [
            {"id": "a", "type": "prop", "name": "door"},
            {"id": "b", "type": "prop", "name": "counter"},
        ],
    }
    assert we.eval_world_expect(
        dump, {"query": [{"type": "prop", "name": "door", "count": 1}]}
    ) == []
    err = we.eval_world_expect(
        dump, {"query": [{"type": "prop", "name": "door", "count": 2}]}
    )
    assert err
