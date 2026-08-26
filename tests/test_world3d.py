"""World3D — 床と箱の衝突。GPU 不要。"""
from __future__ import annotations

import pytest

from tests.conftest import load_kagra_submodule


def _world():
    return load_kagra_submodule("world3d")


def test_player_stopped_by_box():
    m = _world()
    w = m.World3D(gravity=0.0)
    w.add_box(1.1, 0.0, 0.0, 0.6, 1.6, 1.6)
    p = w.add_player(0.0, 0.0, radius=0.28, height=1.6)
    p.use_gravity = False
    w.move_player(5.0, 0.0)
    for _ in range(40):
        w.update(0.016)
    assert p.x < 0.85


def test_player_walks_open_floor():
    m = _world()
    w = m.World3D(gravity=0.0)
    p = w.add_player(0.0, 0.0, radius=0.25, height=1.6)
    p.use_gravity = False
    p.friction = 0.0
    w.move_player(0.0, -3.0)
    for _ in range(20):
        w.update(0.016)
    assert p.z < -0.5


def test_bake_without_engine_is_empty():
    m = _world()
    w = m.World3D()
    w.add_floor()
    w.add_box(0, 0, 0, 1, 1, 1)
    assert w.bake(1, 2) == []
    assert w.mesh_ids == []


def test_box_xforms_recorded_without_bake():
    m = _world()
    w = m.World3D()
    w.add_box(2, 0, -1, 1.2, 1.0, 1.2)
    assert len(w.box_xforms) == 1
    assert abs(w.box_xforms[0][0] - 2.0) < 1e-6
    assert abs(w.box_xforms[0][1] - 0.5) < 1e-6


def test_add_box_draw_false_is_physics_only():
    m = _world()
    w = m.World3D()
    body = w.add_box(1, 0, 1, 1, 1, 1, draw=False)
    assert w.box_xforms == []
    assert w._pending == []
    assert body in w.boxes
    w.add_box(2, 0, 1, 1, 1, 1, draw=True)
    assert len(w.box_xforms) == 1


def test_add_sphere_and_cylinder_are_physics_only():
    m = _world()
    w = m.World3D()
    ball = w.add_sphere(0, 0, 0, 0.5)
    cyl = w.add_cylinder(2, 0, 0, 0.4, 1.5)
    assert ball.shape == "sphere"
    assert cyl.shape == "cylinder"
    assert w.box_xforms == []
    assert w._pending == []


def test_height_fn_player_spawns_on_terrain():
    m = _world()
    w = m.World3D()
    w.set_height_fn(lambda _x, _z: 1.25)
    p = w.add_player(0.0, 0.0)
    assert p.y == pytest.approx(1.25)
    assert w.ground_y(1.0, 1.0) == pytest.approx(1.25)


def test_stream_tiles_load_and_unload():
    m = _world()
    w = m.World3D(half=48.0)
    w.set_height_fn(lambda _x, _z: 0.0, tile=10.0, stream_radius=12.0)
    n = w.stream_tiles(0.0, 0.0)
    assert n >= 1
    near = set(w.loaded_tiles())
    assert any(abs(ix) <= 1 and abs(iz) <= 1 for ix, iz in near)
    w.stream_tiles(50.0, 0.0)
    # Delayed unload: origin may linger one frame after leaving want.
    w.stream_tiles(50.0, 0.0)
    far = set(w.loaded_tiles())
    assert near != far
    assert (0, 0) not in far


def test_load_city_y_zero_snaps_to_ground(tmp_path):
    path = tmp_path / "c0.json"
    path.write_text(
        '{"version":1,"tile":10,"boxes":[{"x":2.0,"z":1.0,"w":1,"h":2,"d":1}]}',
        encoding="utf-8",
    )
    m = _world()
    w = m.World3D(half=24.0)
    w.set_height_fn(lambda _x, _z: 1.5, tile=10.0, stream_radius=16.0)
    w.load_city(str(path))
    w.stream_tiles(0.0, 0.0)
    assert any(abs(b.y - 1.5) < 1e-6 for b in w.boxes)


def test_load_city_places_on_stream(tmp_path):
    city = load_kagra_submodule("city")
    path = tmp_path / "c.json"
    path.write_text(
        '{"version":1,"tile":10,"boxes":[{"x":2.0,"y":0.4,"z":1.0,"w":1,"h":2,"d":1}]}',
        encoding="utf-8",
    )
    m = _world()
    w = m.World3D(half=24.0)
    w.set_height_fn(lambda _x, _z: 0.4, tile=10.0, stream_radius=16.0)
    w.load_city(str(path))
    w.stream_tiles(0.0, 0.0)
    assert any(abs(b.x - 2.0) < 1e-6 for b in w.boxes)


def test_player_stands_on_dynamic_stack():
    """Walk 相当のカプセルが積み木の上に立つ。"""
    m = _world()
    w = m.World3D(gravity=22.0)
    w.add_box(0.0, 0.8, 0.0, 1.0, 0.45, 1.0, is_static=False)
    for _ in range(140):
        w.update(0.016)
    crate = w.boxes[0]
    top = crate.y + crate.h
    p = w.add_player(0.0, 0.0, radius=0.26, height=1.6)
    p.x = crate.x
    p.z = crate.z
    p.y = top + 1.3
    for _ in range(160):
        w.update(0.016)
    assert p.on_ground
    assert p.y == pytest.approx(top, abs=0.14)
    assert crate.y == pytest.approx(0.0, abs=0.12)


def test_dynamic_box_xform_tracks_body():
    m = _world()
    w = m.World3D(gravity=0.0)
    b = w.add_box(0.0, 1.0, 0.0, 0.6, 0.6, 0.6, is_static=False)
    b.use_gravity = False
    b.vx = 2.0
    b.friction = 0.0
    w.update(0.05)
    assert w.box_xforms[0][0] == pytest.approx(b.x)


def test_lod_cells_far_tiles_are_coarser():
    m = _world()
    w = m.World3D(half=80.0)
    w.set_height_fn(
        lambda _x, _z: 0.0,
        tile=16.0, stream_radius=64.0, cells=8,
        lod_radius=20.0, lod_cells=3,
    )
    w.stream_tiles(0.0, 0.0)
    assert w._tile_lod[(0, 0)] == 8
    far = [k for k, cells in w._tile_lod.items() if cells == 3]
    assert far
    near = [k for k, cells in w._tile_lod.items() if cells == 8]
    assert (0, 0) in near
    w.stream_tiles(48.0, 0.0)
    # origin is now far / unloaded
    if (0, 0) in w._tile_lod:
        assert w._tile_lod[(0, 0)] == 3


def test_chunk_fill_once_per_tile():
    m = _world()
    w = m.World3D(half=24.0)
    hits: list[tuple[int, int]] = []
    w.set_height_fn(lambda _x, _z: 0.0, tile=10.0, stream_radius=8.0)
    w.set_chunk_fill(lambda ix, iz: hits.append((ix, iz)))
    w.stream_tiles(0.0, 0.0)
    first = list(hits)
    assert first
    w.stream_tiles(16.0, 0.0)
    w.stream_tiles(0.0, 0.0)
    assert hits[: len(first)] == first
    assert len(hits) == len(set(hits))


def test_stream_tiles_budget_caps_new_tiles():
    m = _world()
    w = m.World3D(half=48.0)
    w.set_height_fn(lambda _x, _z: 0.0, tile=10.0, stream_radius=12.0)
    w.stream_tiles(0.0, 0.0)
    first = set(w.loaded_tiles())
    assert first
    w.stream_tiles(30.0, 0.0, max_new=1)
    far = set(w.loaded_tiles())
    added = far - first
    assert len(added) <= 1


def test_stream_lod_budget_does_not_open_holes():
    """Stale-LOD tiles must stay until a replacement uploads (max_new=1)."""
    m = _world()
    w = m.World3D(half=80.0)
    w.set_height_fn(
        lambda _x, _z: 0.0,
        tile=16.0, stream_radius=48.0, cells=8,
        lod_radius=20.0, lod_cells=3,
    )
    w.stream_tiles(0.0, 0.0)
    first = set(w.loaded_tiles())
    assert first
    nx, nz = 36.0, 0.0
    still_wanted = first & set(w.wanted_tiles(nx, nz))
    assert still_wanted
    w.stream_tiles(nx, nz, max_new=1)
    loaded = set(w.loaded_tiles())
    assert still_wanted <= loaded
    for key in still_wanted:
        assert key in w._tile_lod


def test_terrain_base_defaults_white_for_relic_run():
    """Crest Isle tints meadow via world.terrain_base; Relic Run keeps JPEG albedo."""
    m = _world()
    w = m.World3D(half=24.0)
    assert w.terrain_base == (1.0, 1.0, 1.0)
    assert w.terrain_uv_period is None
    assert w.terrain_uv_blend == 0.0
    assert w.terrain_uv_pad == 0.0
    assert w.terrain_uv_rect is None
    w.terrain_base = (0.55, 1.55, 0.70)
    assert w.terrain_base[1] > w.terrain_base[0]


def test_bake_terrain_invokes_chunk_fill():
    """bake_terrain streams at once — fill callbacks must be ready first.

    Crest Isle crashed because ``_chunk_props`` was assigned after bake.
    """
    m = _world()
    w = m.World3D(half=24.0)
    w.set_height_fn(lambda _x, _z: 0.0, tile=10.0, stream_radius=8.0)
    w.add_player(0.0, 0.0)

    class Host:
        def __init__(self):
            self._chunk_props = 0

        def fill(self, _ix, _iz):
            self._chunk_props += 1

    host = Host()
    w.set_chunk_fill(host.fill)
    w.bake_terrain(1)
    assert host._chunk_props >= 1


def test_world_update_feeds_debug_trace_on_slope():
    """Crest Isle / Relic Run / Overworld all call World3D.update."""
    tr = load_kagra_submodule("trace")
    m = _world()
    tr._ACTIVE = None
    world = m.World3D(gravity=20.0, half=24.0)
    world.set_height_fn(lambda x, _z: 0.4 * x)
    p = world.add_player(1.0, 0.0, radius=0.28, height=1.7)
    p.friction = 0.0
    tracer = tr.DebugTrace(
        height_fn=lambda x, _z: 0.4 * x, persist=False, threshold=0.05,
    )
    tr._ACTIVE = tracer
    try:
        for _ in range(80):
            world.move_player(3.0, 0.0)
            world.update(0.016)
        assert tracer.summary() == "ok"
        world.physics.foot_radius = 0.28
        world.physics.snap_to_plane = False
        tracer2 = tr.DebugTrace(
            height_fn=lambda x, _z: 0.4 * x, persist=False, threshold=0.05,
        )
        tr._ACTIVE = tracer2
        for _ in range(80):
            world.move_player(3.0, 0.0)
            world.update(0.016)
        assert "floated" in tracer2.summary()
    finally:
        tr._ACTIVE = None


def test_failed_upload_is_not_sticky_loaded(monkeypatch):
    """GPU upload fail / zero id must not skip forever as a bald rectangle."""
    import sys

    m = _world()
    w = m.World3D(half=24.0)
    w.set_height_fn(lambda _x, _z: 0.0, tile=10.0, stream_radius=12.0)
    w._terrain_tex = 7
    kagra = sys.modules["kagra"]
    calls = {"n": 0}

    def boom(*_a, **_k):
        calls["n"] += 1
        raise RuntimeError("upload failed")

    monkeypatch.setattr(kagra, "upload_mesh_3d", boom, raising=False)
    monkeypatch.setattr(kagra, "unload_mesh_3d", lambda *_a, **_k: None, raising=False)
    w.stream_tiles(0.0, 0.0)
    want = w.wanted_tiles(0.0, 0.0)
    assert want
    assert calls["n"] >= 1
    for key in want:
        assert key not in w._loaded_tiles
        assert key not in w._tile_lod
        assert key not in w._tile_meshes

    def zero(*_a, **_k):
        calls["n"] += 1
        return 0

    monkeypatch.setattr(kagra, "upload_mesh_3d", zero, raising=False)
    w.stream_tiles(0.0, 0.0)
    for key in want:
        assert key not in w._loaded_tiles
        assert key not in w._tile_lod

    ids = {"n": 0}

    def ok(*_a, **_k):
        ids["n"] += 1
        return ids["n"]

    monkeypatch.setattr(kagra, "upload_mesh_3d", ok, raising=False)
    w.stream_tiles(0.0, 0.0)
    loaded = set(w.loaded_tiles())
    assert loaded
    assert loaded <= set(want)
    assert all(k in w._tile_lod for k in loaded)
    assert all(k in w._tile_meshes for k in loaded)
    assert ids["n"] >= 1


def test_cpu_stream_then_gpu_retries_missing_mesh(monkeypatch):
    """stream_tiles before bake_terrain used to mark keys loaded with no mesh."""
    import sys

    m = _world()
    w = m.World3D(half=24.0)
    w.set_height_fn(lambda _x, _z: 0.0, tile=10.0, stream_radius=12.0)
    w.stream_tiles(0.0, 0.0)
    first = set(w.loaded_tiles())
    assert first
    assert not w._tile_meshes
    w._terrain_tex = 3
    kagra = sys.modules["kagra"]
    ids = {"n": 0}

    def ok(*_a, **_k):
        ids["n"] += 1
        return 100 + ids["n"]

    monkeypatch.setattr(kagra, "upload_mesh_3d", ok, raising=False)
    monkeypatch.setattr(kagra, "unload_mesh_3d", lambda *_a, **_k: None, raising=False)
    w.stream_tiles(0.0, 0.0)
    assert ids["n"] >= 1
    assert first <= set(w.loaded_tiles())
    assert first <= set(w._tile_meshes)


def test_stream_delayed_unload_keeps_tile_one_frame():
    land = load_kagra_submodule("land")
    m = _world()
    w = m.World3D(half=48.0)
    w.set_height_fn(lambda _x, _z: 0.0, tile=10.0, stream_radius=12.0)
    w.stream_tiles(0.0, 0.0)
    assert (0, 0) in w.loaded_tiles()
    want_far = set(w.wanted_tiles(40.0, 0.0))
    vis_far = set(land.tile_keys(40.0, 0.0, tile=10.0, radius=12.0, half=48.0))
    assert (0, 0) not in want_far
    assert (0, 0) not in vis_far
    w.stream_tiles(40.0, 0.0)
    assert (0, 0) in w.loaded_tiles()
    w.stream_tiles(40.0, 0.0)
    assert (0, 0) not in w.loaded_tiles()


def test_stream_prefetches_plus_x_before_visible():
    land = load_kagra_submodule("land")
    m = _world()
    w = m.World3D(half=48.0)
    w.set_height_fn(lambda _x, _z: 0.0, tile=10.0, stream_radius=12.0)
    vis0 = set(land.tile_keys(0.0, 0.0, tile=10.0, radius=12.0, half=48.0))
    want0 = set(w.wanted_tiles(0.0, 0.0))
    extra = want0 - vis0
    assert extra
    east = max(ix for ix, _iz in vis0)
    plus = {(east + 1, iz) for ix, iz in vis0 if ix == east}
    assert plus <= want0
    assert not plus <= vis0
    step = 4.0
    vis1 = set(land.tile_keys(step, 0.0, tile=10.0, radius=12.0, half=48.0))
    want1 = set(w.wanted_tiles(step, 0.0))
    new_vis = vis1 - vis0
    assert plus <= want1
    if new_vis and new_vis <= plus:
        assert plus <= want0


def test_stream_upgrades_near_lod_before_new_far_tiles():
    m = _world()
    w = m.World3D(half=80.0)
    w.set_height_fn(
        lambda _x, _z: 0.0,
        tile=16.0, stream_radius=48.0, cells=8,
        lod_radius=20.0, lod_cells=3,
    )
    w.stream_tiles(0.0, 0.0)
    coarse = [k for k, cells in w._tile_lod.items() if cells == 3]
    assert coarse
    target = min(coarse, key=lambda k: (k[0] - 0) ** 2 + k[1] ** 2)
    ox = (target[0] + 0.5) * 16.0
    oz = (target[1] + 0.5) * 16.0
    assert w._cells_for(target, 0.0, 0.0) == 3
    assert w._cells_for(target, ox, oz) == 8
    before = set(w.loaded_tiles())
    w.stream_tiles(ox, oz, max_new=1)
    after = set(w.loaded_tiles())
    added = after - before
    assert target in w.loaded_tiles()
    assert w._tile_lod[target] == 8
    assert len(added) == 0


def test_upload_tile_forces_lambert_not_coin_pbr(monkeypatch):
    """Crest coins are metallic=1 / roughness=0.12. Terrain must stay Lambert.

    A PBR default (or leftover mesh_mat slot) is the black quad + gold spec
    on one streamed hillside tile. Do not paper over with a brighter tint.
    """
    import sys

    hdri = load_kagra_submodule("hdri")
    m = _world()
    w = m.World3D(half=24.0)
    w.set_height_fn(lambda _x, _z: 0.4, tile=16.0, stream_radius=12.0)
    w.terrain_base = (0.55, 1.55, 0.70)
    w._terrain_tex = 7
    kagra = sys.modules["kagra"]
    calls = []

    def upload(*_a, **kw):
        calls.append(kw)
        return 40 + len(calls)

    pbr = []

    def set_pbr(mid, **kw):
        pbr.append((int(mid), kw))

    monkeypatch.setattr(kagra, "upload_mesh_3d", upload, raising=False)
    monkeypatch.setattr(kagra, "set_mesh_pbr", set_pbr, raising=False)
    monkeypatch.setattr(kagra, "unload_mesh_3d", lambda *_a, **_k: None, raising=False)
    w.stream_tiles(0.0, 0.0)
    assert calls
    for kw in calls:
        metallic = float(kw.get("metallic", 99.0))
        roughness = float(kw.get("roughness", 0.0))
        assert metallic == 0.0, kw
        assert roughness == 1.0, kw
        assert not hdri.pbr_enabled(metallic, roughness)
        assert kw.get("base_color") == (0.55, 1.55, 0.70)
    assert pbr
    for _mid, kw in pbr:
        assert float(kw.get("metallic", 99.0)) == 0.0
        assert float(kw.get("roughness", 0.0)) == 1.0
        assert kw.get("base_color") == (0.55, 1.55, 0.70)
    assert hdri.pbr_enabled(1.0, 0.12)


def test_draw_emits_live_tile_meshes_not_stale_mesh_ids(monkeypatch):
    """Draw path must use ``_tile_meshes``, not a desynced ``mesh_ids`` list.

    A live tile missing from mesh_ids used to vanish (or a leftover id kept
    drawing). Extra mesh_ids (Relic/Overworld ramp) still draw.
    """
    import sys

    m = _world()
    w = m.World3D(half=24.0)
    w._tile_meshes = {(0, 0): 11, (1, 0): 12}
    w.mesh_ids = [99, 11]
    w.box_mesh_id = 5
    kagra = sys.modules["kagra"]
    drawn = []
    instanced = []
    monkeypatch.setattr(kagra, "draw_mesh_id", lambda mid: drawn.append(int(mid)), raising=False)
    monkeypatch.setattr(
        kagra, "draw_mesh_instances",
        lambda mid, xforms: instanced.append((int(mid), xforms)),
        raising=False,
    )
    w.box_xforms = [[0.0, 0.5, 0.0, 1.0, 1.0, 1.0, 0.0]]
    w.draw()
    assert 11 in drawn
    assert 12 in drawn
    assert 99 in drawn
    assert 5 not in drawn
    assert instanced and instanced[0][0] == 5
    assert drawn.count(11) == 1


