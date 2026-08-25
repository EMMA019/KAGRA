"""debug_trace — slope-float detector. GPU 不要。偽の height_fn。"""
from __future__ import annotations

from tests.conftest import load_kagra_submodule


def _tr():
    return load_kagra_submodule("trace")


def test_quiet_when_stuck_to_terrain():
    m = _tr()
    tracer = m.DebugTrace(height_fn=lambda x, z: 0.3 * x, persist=False, threshold=0.05)
    # foot sits on the slope
    rec = tracer.sample(foot_y=0.3, x=1.0, z=0.0, on_ground=True, frame=1)
    assert rec is None
    assert tracer.summary() == "ok"


def test_emits_when_grounded_and_floating():
    m = _tr()
    tracer = m.DebugTrace(height_fn=lambda x, z: 0.0, persist=False, threshold=0.05)
    rec = tracer.sample(
        foot_y=0.16, x=0.0, z=0.0, on_ground=True, vx=1.0, vz=0.2,
        camera_distance=4.8, frame=10, timestamp=0.0,
    )
    assert rec is not None
    assert rec["delta"] == 0.16
    assert rec["ground_y"] == 0.0
    assert rec["on_ground"] is True
    assert rec["vx"] == 1.0
    assert rec["camera_distance"] == 4.8


def test_skips_airborne_even_if_delta_large():
    m = _tr()
    tracer = m.DebugTrace(height_fn=lambda x, z: 0.0, persist=False)
    rec = tracer.sample(foot_y=1.2, x=0.0, z=0.0, on_ground=False, frame=3)
    assert rec is None


def test_summary_collapses_run():
    m = _tr()
    tracer = m.DebugTrace(height_fn=lambda x, z: 0.0, persist=False, threshold=0.05)
    for fr in range(32, 49):
        tracer.sample(foot_y=0.15, x=0.0, z=0.0, on_ground=True, frame=fr)
    assert tracer.summary() == "frames 32-48 floated 0.15"


def test_jsonl_only_hits(tmp_path):
    m = _tr()
    path = tmp_path / "trace.jsonl"
    tracer = m.DebugTrace(height_fn=lambda x, z: 0.0, persist=True, path=path, threshold=0.05)
    tracer.sample(foot_y=0.0, x=0.0, z=0.0, on_ground=True, frame=1, timestamp=0.0)
    tracer.sample(foot_y=0.2, x=0.0, z=0.0, on_ground=True, frame=2, timestamp=0.0)
    lines = path.read_text(encoding="utf-8").strip().splitlines()
    assert len(lines) == 1
    assert '"frame": 2' in lines[0]


def test_world_ground_y():
    m = _tr()
    w3 = load_kagra_submodule("world3d")
    world = w3.World3D()
    world.set_height_fn(lambda x, z: 0.4)
    tracer = m.DebugTrace(world=world, persist=False, threshold=0.05)
    rec = tracer.sample(foot_y=0.55, x=1.0, z=2.0, on_ground=True, frame=1)
    assert rec is not None
    assert rec["ground_y"] == 0.4
    assert abs(rec["delta"] - 0.15) < 1e-9


def test_debug_trace_fn_and_summary():
    m = _tr()
    m._ACTIVE = None
    rec = m.debug_trace(
        foot_y=0.2, x=0.0, z=0.0, height_fn=lambda x, z: 0.0,
        on_ground=True, persist=False, reset=True, frame=7, threshold=0.05,
    )
    assert rec is not None
    assert "floated" in m.debug_trace_summary()
