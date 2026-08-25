"""Island Relic Run のルールと公開 API 規約。GPU 不要。"""
from __future__ import annotations

import math
import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(_ROOT / "examples"))

from relic_run_rules import (
    CAM_DISTANCE,
    GLTF_HALF_Y,
    LAND_MIN_Y,
    RELIC_XZ,
    ROCK_PLACEMENTS,
    START_XZ,
    STONE_XZ,
    TREE_PLACEMENTS,
    TREE_XZ,
    WATER_Y,
    can_pick,
    grade_for,
    hero_theta,
    nearest_live,
    round_score,
    sit_y,
    spawn_relics,
    start_face,
)

from tests.conftest import load_kagra_submodule


def test_cam_distance_is_showcase_far():
    assert CAM_DISTANCE >= 6.6


def test_start_and_relics_are_distinct():
    assert len(RELIC_XZ) == 5
    assert START_XZ not in RELIC_XZ
    assert not can_pick(*START_XZ, *RELIC_XZ[0])


def test_can_pick_within_reach():
    rx, rz = RELIC_XZ[0]
    assert can_pick(rx, rz, rx, rz)
    assert can_pick(rx + 0.5, rz, rx, rz)
    assert not can_pick(rx + 3.0, rz, rx, rz)


def test_spawn_and_nearest():
    relics = spawn_relics()
    assert len(relics) == 5
    assert all(r.live for r in relics)
    n = nearest_live(*START_XZ, relics)
    assert n is not None
    relics[0].live = False
    assert nearest_live(relics[0].x, relics[0].z, relics) is not None


def test_score_and_grade():
    assert round_score(0, 30.0) == 0
    assert round_score(5, 20.0) > round_score(3, 20.0)
    assert grade_for(800) == "S"
    assert grade_for(10) == "D"


def test_face_and_hero_theta():
    f = start_face()
    assert isinstance(f, float)
    assert abs(hero_theta(f) - (f + math.pi)) < 1e-9


def test_water_and_land_constants():
    assert WATER_Y == 0.0
    assert LAND_MIN_Y < WATER_Y
    assert TREE_XZ and STONE_XZ
    assert len(TREE_PLACEMENTS) >= 20
    assert len(ROCK_PLACEMENTS) >= 5
    names = {n for n, *_ in TREE_PLACEMENTS}
    assert "fence.glb" in names
    assert "flag.glb" in names
    assert "patch-grass.glb" in names


def test_sit_y_puts_centered_mesh_on_ground():
    assert sit_y(1.0, 0.5, 2.0) == 2.0
    assert GLTF_HALF_Y["tree.glb"] > 0.5
    assert GLTF_HALF_Y["tree-high.glb"] > GLTF_HALF_Y["tree.glb"]


def test_props_and_relics_are_on_grass():
    land = load_kagra_submodule("land")
    spots = list(RELIC_XZ) + [START_XZ]
    spots += [(x, z) for _n, x, z, _s, _y in TREE_PLACEMENTS]
    spots += [(x, z) for _n, x, z, _s, _y in ROCK_PLACEMENTS]
    for x, z in spots:
        kind = land.biome_at(x, z, water_y=WATER_Y, fn=land.overworld_height)
        assert kind == "grass", (x, z, kind)


def test_cc0_assets_are_vendored():
    root = _ROOT / "examples" / "assets" / "relic_run"
    assert (root / "LICENSE.md").is_file()
    for name in (
        "kenney/tree.glb",
        "kenney/tree-high.glb",
        "kenney/rocks-high.glb",
        "kenney/rocks-low.glb",
        "kenney/stones.glb",
        "kenney/plant.glb",
        "kenney/rock_largeA.glb",
        "kenney/rock_tallA.glb",
        "kenney/stone_smallTopA.glb",
        "kenney/tent.glb",
        "kenney/fence.glb",
        "kenney/flag.glb",
        "kenney/patch-grass.glb",
        "kenney/Textures/colormap.png",
        "polyhaven/aerial_grass_rock_diff_1k.jpg",
        "polyhaven/kloofendal_48d_partly_cloudy_puresky_1k.png",
    ):
        path = root / name
        assert path.is_file(), name
        assert path.stat().st_size > 1000, name


def test_kenney_tree_loads_colormap():
    gm = load_kagra_submodule("gltf_mesh")
    tree = _ROOT / "examples" / "assets" / "relic_run" / "kenney" / "tree.glb"
    flat = gm.flatten_gltf(tree)
    assert flat.image is not None
    assert flat.image[:8] == b"\x89PNG\r\n\x1a\n"
    assert flat.aabb[4] - flat.aabb[1] > 1.0


def test_relic_run_ibl_is_not_blown():
    src = (_ROOT / "examples" / "vrm_relic_run.py").read_text(encoding="utf-8")
    assert "strength=0.92" not in src
    assert "strength=0.32" in src


def test_game_file_uses_only_public_imports():
    src = _ROOT / "examples" / "vrm_relic_run.py"
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
        # allow comments mentioning walk.yaw prohibition; ban assignment usage
        if name == "walk.yaw":
            assert "self.avatar.set_yaw(self.walk.yaw)" not in text
            assert "set_yaw(self.walk.yaw)" not in text
            continue
        assert name not in text, name
    assert 'resolve_asset(AssetKind.ANY, "walk"' not in text
    assert "AssetKind.ANY" not in text
    # Optional VRMA override is allowed; Mixamo/BVH walk via resolve_asset is not.
    assert "walk.vrma" in text
    assert "except Exception:\n                pass" not in text
    # Built-in idle/walk/run is the fallback; Mixamo FBX is optional.
    assert "built-in" in text
    assert "bind_locomotion" in text
    assert "set_locomotion" in text
    for name in (
        "ensure_vrm",
        "resolve_asset",
        "Prop",
        "Walk",
        "sky",
        "water",
        "apply_outdoor_look",
        "set_spot_light",
        "set_point_light",
        "set_hdri",
        "stage",
        ".glb",
        "overworld_height",
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
        "set_locomotion",
        "bind_locomotion",
    ):
        assert name in text, name


def test_readme_sample_line_stays():
    readme = (_ROOT / "README.md").read_text(encoding="utf-8")
    assert "python examples/vrm_relic_run.py" in readme


def test_relic_run_poses_with_speed_blend_not_clip_snap():
    src = (_ROOT / "examples" / "vrm_relic_run.py").read_text(encoding="utf-8")
    pose = src[src.index("    def _pose") :]
    nxt = pose.find("\n    def ", 10)
    pose = pose[:nxt] if nxt != -1 else pose
    assert "set_locomotion" in pose
    assert 'want = "walk" if moving else "idle"' not in pose
    assert "self.avatar.play(want" not in pose
    assert "hypot(p.vx, p.vz)" in pose
