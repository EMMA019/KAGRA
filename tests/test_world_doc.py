"""GPU-free WorldDoc fallback (no kagra_shared)."""
from __future__ import annotations

import pytest

from tests.conftest import load_kagra_submodule

wd = load_kagra_submodule("world_doc")


def test_from_json_version_1():
    doc = wd.WorldDoc.from_json('{"version":1,"player":{"position":[1,2,3]}}')
    assert doc.player_position() == [1.0, 2.0, 3.0]


def test_from_json_rejects_other_version():
    with pytest.raises(ValueError, match="version"):
        wd.WorldDoc.from_json('{"version":2}')


def test_world_play_from_json_and_dump():
    play = wd.WorldPlay.from_json('{"version":1}')
    assert '"version": 1' in play.dump() or '"version":1' in play.dump()


def test_tick_needs_shared():
    play = wd.WorldPlay.from_json('{"version":1}')
    with pytest.raises(ImportError, match="kagra_shared"):
        play.tick(1.0 / 60.0)
