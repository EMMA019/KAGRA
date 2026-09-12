"""GPU-free WorldDoc fallback (no kagra_shared)."""
from __future__ import annotations

from tests.conftest import load_kagra_submodule


def test_fallback_worlddoc_roundtrips_dump():
    mod = load_kagra_submodule("world_fallback")
    raw = '{"version": 1, "player": {"position": [1, 2, 3]}, "props": [{"id": "p", "name": "crate", "position": [4, 0, 5], "enabled": true}]}'
    doc = mod.WorldDoc.from_json(raw)
    assert doc.player_position() == [1.0, 2.0, 3.0]
    assert doc.props_named("crate") == [("p", [4.0, 0.0, 5.0])]
    again = mod.WorldDoc.from_json(doc.to_json())
    assert again.player_position() == [1.0, 2.0, 3.0]


def test_fallback_rejects_unknown_version():
    mod = load_kagra_submodule("world_fallback")
    try:
        mod.WorldDoc.from_json('{"version": 99}')
    except ValueError as exc:
        assert "99" in str(exc)
    else:
        raise AssertionError("expected ValueError")


def test_fallback_worldplay_dump_without_tick():
    mod = load_kagra_submodule("world_fallback")
    play = mod.WorldPlay.from_json('{"version": 1, "coins": 2}')
    assert '"coins": 2' in play.dump() or '"coins":2' in play.dump()
    try:
        play.tick(1.0 / 60.0)
    except ImportError as exc:
        assert "kagra_shared" in str(exc)
    else:
        raise AssertionError("tick must need kagra_shared")
