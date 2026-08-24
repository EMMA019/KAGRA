"""Island Relic Run のルールと公開 API 規約。GPU 不要。"""
from __future__ import annotations

import math
import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(_ROOT / "examples"))

from relic_run_rules import (
    CAM_DISTANCE,
    LAND_MIN_Y,
    RELIC_XZ,
    START_XZ,
    STONE_XZ,
    TREE_XZ,
    WATER_Y,
    can_pick,
    grade_for,
    hero_theta,
    nearest_live,
    round_score,
    spawn_relics,
    start_face,
)


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
    ):
        assert name in text, name
