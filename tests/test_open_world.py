"""Crest Isle のルールと公開 API 規約。GPU 不要。"""
from __future__ import annotations

import math
import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(_ROOT / "examples"))

from open_world_rules import (
    CAM_DISTANCE,
    COIN_XZ,
    GLTF_HALF_Y,
    HALF,
    LOD_CELLS,
    LOD_RADIUS,
    PEAK_XZ,
    STAR_NEED,
    STAR_XZ,
    START_XZ,
    STREAM_RADIUS,
    TILE,
    VISTA_PROPS,
    WATER_Y,
    chunk_decor,
    grade_for,
    hero_theta,
    nearest_live,
    round_score,
    sit_y,
    spawn_coins,
    spawn_stars,
    start_face,
    won,
)

from tests.conftest import load_kagra_submodule


def test_world_is_much_larger_than_relic_run():
    assert HALF >= 80.0
    assert STREAM_RADIUS > 28.0
    assert STREAM_RADIUS >= 64.0
    assert LOD_RADIUS < STREAM_RADIUS
    assert LOD_CELLS < 8
    assert TILE >= 10.0
    assert CAM_DISTANCE >= 10.0


def test_spawn_looks_at_grass_sea_mountain():
    land = load_kagra_submodule("land")
    fn = land.open_world_height
    assert land.biome_at(*START_XZ, fn=fn) == "grass"
    assert land.biome_at(-22.0, 10.0, fn=fn) == "sea"
    assert land.biome_at(*PEAK_XZ, fn=fn) == "mountain"
    assert fn(*PEAK_XZ) > fn(*START_XZ) + 6.0


def test_collectathon_layout():
    assert len(STAR_XZ) == 8
    assert STAR_NEED == 6
    assert PEAK_XZ in STAR_XZ
    assert START_XZ not in STAR_XZ
    assert len(COIN_XZ) >= 20
    assert len(set(STAR_XZ)) == 8
    assert len(VISTA_PROPS) >= 120


def test_vista_is_in_opening_frustum():
    # Chase cam looks +Z from START; keep a dense pack in that cone.
    in_shot = [
        (x, z) for _n, x, z, _s, _y, _c in VISTA_PROPS
        if -16.0 <= x <= 18.0 and -8.0 <= z <= 24.0
    ]
    assert len(in_shot) >= 80


def test_stars_and_coins_are_reachable_land():
    land = load_kagra_submodule("land")
    fn = land.open_world_height
    for x, z in STAR_XZ + COIN_XZ:
        assert fn(x, z) > WATER_Y - 0.2, (x, z, fn(x, z))


def test_pick_and_score():
    from open_world_rules import PICK_REACH

    sx, sz = STAR_XZ[0]
    assert math.hypot(sx - sx, sz - sz) <= PICK_REACH
    stars = spawn_stars()
    coins = spawn_coins()
    assert all(s.live for s in stars)
    assert nearest_live(*START_XZ, stars) is not None
    assert round_score(0, 0, 10.0) == 0
    assert round_score(6, 10, 60.0) > round_score(3, 10, 60.0)
    assert won(6)
    assert not won(5)
    assert grade_for(2500) == "S"
    assert grade_for(10) == "D"


def test_face_never_uses_camera_yaw_for_body():
    f = start_face()
    assert abs(hero_theta(f) - (f + math.pi)) < 1e-9


def test_sit_y_and_gltf_table():
    assert sit_y(1.0, 0.5, 2.0) == 2.0
    assert GLTF_HALF_Y["forest/tree.glb"] > 0.5
    assert GLTF_HALF_Y["dungeon/coin.glb"] > 0.1
    assert GLTF_HALF_Y["castle/flag-wide.glb"] > 0.3


def test_chunk_decor_skips_spawn_tiles():
    assert chunk_decor(0, 0) == []
    far = chunk_decor(3, 2)
    assert far
    for _n, x, z, _s, _y, _c in far:
        assert abs(x) > 8.0 or abs(z) > 8.0


def test_cc0_assets_are_vendored():
    root = _ROOT / "examples" / "assets" / "open_world"
    assert (root / "LICENSE.md").is_file()
    for name in (
        "kenney/forest/tree.glb",
        "kenney/forest/tree-high.glb",
        "kenney/forest/fence.glb",
        "kenney/forest/flag.glb",
        "kenney/forest/plant.glb",
        "kenney/forest/Textures/colormap.png",
        "kenney/nature/cliff_large_rock.glb",
        "kenney/nature/flower_redA.glb",
        "kenney/nature/grass_large.glb",
        "kenney/nature/tree_pineTallA.glb",
        "kenney/nature/tree_palmDetailedTall.glb",
        "kenney/town/banner-red.glb",
        "kenney/town/wall-broken.glb",
        "kenney/castle/flag-wide.glb",
        "kenney/dungeon/coin.glb",
        "kenney/dungeon/chest.glb",
    ):
        path = root / name
        assert path.is_file(), name
        assert path.stat().st_size > 1000, name
    # Poly Haven is shared with Relic Run (not duplicated in the wheel).
    ph = _ROOT / "examples" / "assets" / "relic_run" / "polyhaven"
    assert (ph / "aerial_grass_rock_diff_1k.jpg").is_file()
    assert (ph / "kloofendal_48d_partly_cloudy_puresky_1k.png").is_file()


def test_kenney_tree_loads_colormap():
    gm = load_kagra_submodule("gltf_mesh")
    tree = _ROOT / "examples" / "assets" / "open_world" / "kenney" / "forest" / "tree.glb"
    flat = gm.flatten_gltf(tree)
    assert flat.image is not None
    assert flat.image[:8] == b"\x89PNG\r\n\x1a\n"


def test_game_file_uses_only_public_imports():
    src = _ROOT / "examples" / "vrm_open_world.py"
    text = src.read_text(encoding="utf-8")
    for name in (
        "_look_at",
        "_perspective_wgpu",
        "_send_bone_rot",
        "_euler_to_quat",
        "from kagra.vrm_avatar import _ID",
        "first_person=True",
        "walk.yaw",
    ):
        if name == "walk.yaw":
            assert "self.avatar.set_yaw(self.walk.yaw)" not in text
            assert "set_yaw(self.walk.yaw)" not in text
            continue
        assert name not in text, name
    assert 'resolve_asset(AssetKind.ANY, "walk"' not in text
    assert "AssetKind.ANY" not in text
    assert "except Exception:\n                pass" not in text
    assert "built-in" in text
    for name in (
        "ensure_vrm",
        "resolve_asset",
        "Prop",
        "Walk",
        "sky",
        "water",
        "apply_outdoor_look",
        "set_hdri",
        "stage",
        "open_world_height",
        "can_pick",
        "lod_radius",
        "stream_radius",
        "World3D",
        "Camera3D",
        "follow",
        "walk.face",
        "texture_from_fn",
        "tone",
        "save_json",
        "ActionController",
        "Label",
        "draw_vignette",
        "draw_billboard_instances",
    ):
        assert name in text, name


def test_readme_sample_line():
    readme = (_ROOT / "README.md").read_text(encoding="utf-8")
    assert "python examples/vrm_open_world.py" in readme
    ja = (_ROOT / "README.ja.md").read_text(encoding="utf-8")
    assert "python examples/vrm_open_world.py" in ja


def test_chunk_props_ready_before_first_stream():
    """``_fill_chunk`` runs during bake_terrain; the counter must exist first."""
    src = (_ROOT / "examples" / "vrm_open_world.py").read_text(encoding="utf-8")
    init_i = src.index("self._chunk_props = 0")
    fill_i = src.index("self.world.set_chunk_fill")
    bake_i = src.index("self.world.bake_terrain")
    assert init_i < fill_i < bake_i
    assert src.count("self._chunk_props = 0") == 1
