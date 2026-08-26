"""Crest Isle のルールと公開 API 規約。GPU 不要。"""
from __future__ import annotations

import math
import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(_ROOT / "examples"))

from open_world_rules import (
    AERIAL_GRASS_ALBEDO,
    CAM_DISTANCE,
    CAM_HEIGHT,
    CAM_LOOK_Y,
    CAM_MAX_DISTANCE,
    CAM_MIN_DISTANCE,
    COIN_XZ,
    GLTF_HALF_Y,
    GRASS_TINT,
    HALF,
    LOD_CELLS,
    LOD_RADIUS,
    PEAK_XZ,
    STAR_NEED,
    STAR_XZ,
    START_XZ,
    STREAM_RADIUS,
    TERRAIN_UV_BLEND,
    TERRAIN_UV_PAD,
    TERRAIN_UV_PERIOD,
    TILE,
    VISTA_PROPS,
    WATER_Y,
    SparkBurst,
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
    forest = _ROOT / "examples" / "assets" / "open_world" / "kenney" / "forest"
    cmap = forest / "Textures" / "colormap.png"
    assert cmap.is_file()
    assert cmap.stat().st_size == 10659
    assert gm._read_relative_image("Textures/colormap.png", forest / "tree.glb") == cmap.read_bytes()
    for name in ("tree.glb", "tree-high.glb"):
        tree = forest / name
        flat = gm.flatten_gltf(tree)
        assert flat.image is not None, name
        assert flat.image[:8] == b"\x89PNG\r\n\x1a\n"
        assert flat.image == cmap.read_bytes(), name
        uvs = [(v[6], v[7]) for v in flat.verts]
        assert uvs
        assert max(u for u, _ in uvs) > min(u for u, _ in uvs)


def test_crest_isle_sun_and_ibl_are_sane():
    src = (_ROOT / "examples" / "vrm_open_world.py").read_text(encoding="utf-8")
    assert "set_light_dir(-0.32, -1.0" not in src
    assert "set_light_dir(-0.32, 1.0, 0.22)" in src
    assert "strength=0.95" not in src
    assert "strength=0.32" in src
    stage = (_ROOT / "kagra" / "stage.py").read_text(encoding="utf-8")
    assert "enabled=False" in stage
    assert "current_fog" in stage


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
    assert "bind_locomotion" in text
    assert "set_locomotion" in text
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
        "set_locomotion",
        "bind_locomotion",
        "Label",
        "draw_vignette",
        "draw_billboard_instances",
        "quad_y_mesh",
        "draw_mesh_3d",
        "SparkBurst",
        "set_listener",
        "play_loop",
        "play_se",
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


def test_title_draw_skips_live_world():
    """Title is opaque and must not composite the half-streamed island."""
    src = (_ROOT / "examples" / "vrm_open_world.py").read_text(encoding="utf-8")
    assert 'self.mode = "play" if SMOKE else "title"' in src
    start = src.index("    def draw(self):")
    nxt = src.index("\n    def ", start + 1)
    draw = src[start:nxt]
    title_if = draw.index('if self.mode == "title"')
    title_ret = draw.index("return", title_if)
    title_arm = draw[title_if:title_ret]
    for needle in (
        "self.world.draw()",
        "kagra.Prop.draw_all()",
        "kagra.draw_vrm",
        "kagra.water(",
    ):
        assert needle not in title_arm, needle
        assert draw.index(needle) > title_ret, needle
    # Do not search bare "118": the title comment mentions alpha-118.
    call_i = title_arm.index("self._banner(")
    banner_call = title_arm[call_i:]
    assert "overlay_alpha=255" in banner_call
    assert "overlay_alpha=118" not in banner_call
    banner = src[src.index("    def _banner") :]
    assert "overlay_alpha: int = 118" in banner
    assert "Alicia Solid" in banner
    assert '"Crest Isle"' in title_arm
    assert "草原・海・山を走れ" in title_arm
    assert "SPACE" in title_arm


def test_mesh3d_tex_bg_cache_fits_crest_isle_vista():
    """FIFO 64 evicted grass into Fallback White after 120+ Kenney Props."""
    src = (_ROOT / "kagra-core" / "src" / "renderer" / "gpu_helpers.rs").read_text(
        encoding="utf-8",
    )
    assert "MESH3D_TEX_BG_MAX: usize = 256" in src
    assert "fn lru_evict_dead" in src
    assert "Live textures must not become Fallback White" in src
    win = (_ROOT / "kagra-core" / "src" / "window.rs").read_text(encoding="utf-8")
    assert "path_texture_cache" in win
    assert "HashMap<(u32, String), u32>" in win


def test_crest_chase_cam_band_is_authored_3d_distance():
    """Far speck / inside-skull / fog-white came from unbounded follow distance."""
    assert CAM_MIN_DISTANCE == 6.0
    authored = math.hypot(CAM_DISTANCE, CAM_HEIGHT - CAM_LOOK_Y)
    assert abs(CAM_MAX_DISTANCE - authored) < 1e-9
    src = (_ROOT / "examples" / "vrm_open_world.py").read_text(encoding="utf-8")
    assert "min_distance=CAM_MIN_DISTANCE" in src
    assert "max_distance=CAM_MAX_DISTANCE" in src
    cam = (_ROOT / "kagra" / "camera3d.py").read_text(encoding="utf-8")
    assert "def clamp_eye" in cam
    assert "min_hit" in cam


def test_crest_grass_tint_is_green_not_white():
    """Shared aerial JPEG is brown dirt; Crest-only mesh_mat.base must stay meadow."""
    tinted = tuple(a * t for a, t in zip(AERIAL_GRASS_ALBEDO, GRASS_TINT))
    assert tinted[1] > tinted[0] and tinted[1] > tinted[2], tinted
    assert max(tinted) < 0.9, tinted
    src = (_ROOT / "examples" / "vrm_open_world.py").read_text(encoding="utf-8")
    assert "terrain_base = GRASS_TINT" in src
    relic = (_ROOT / "examples" / "vrm_relic_run.py").read_text(encoding="utf-8")
    assert "terrain_base" not in relic


def test_crest_sky_snapshots_fog_off_and_mtoon_flips_backfaces():
    """Stage.draw restore-before-flush fogged puresky; inside-skull rim blew white."""
    rend = (_ROOT / "kagra-core" / "src" / "renderer" / "mod.rs").read_text(
        encoding="utf-8",
    )
    assert "cmd.skip_fog = self.fog_params[2] < 0.5" not in rend
    shaders = (_ROOT / "kagra-core" / "src" / "renderer" / "shaders.rs").read_text(
        encoding="utf-8",
    )
    assert "mesh_mat.base.w < 0.5" in shaders
    assert "@builtin(front_facing) front" in shaders
    assert "if !front" in shaders
    py = (_ROOT / "kagra" / "__init__.py").read_text(encoding="utf-8")
    assert "skip_fog: bool = False" in py
    stage = (_ROOT / "kagra" / "stage.py").read_text(encoding="utf-8")
    assert "skip_fog=True" in stage
    play = (_ROOT / "kagra" / "play.py").read_text(encoding="utf-8")
    assert "skip_fog=True" in play
    inp = (_ROOT / "kagra-core" / "src" / "input.rs").read_text(encoding="utf-8")
    assert "REHOLD_QUIET_FRAMES: u8 = 3" in inp
    assert "REHOLD_QUIET_FRAMES: u8 = 15" not in inp


def test_crest_isle_uses_spatial_sea_and_pickup():
    """Looping west sea + pickup SE at the collectible. Title/win stay 2D."""
    src = (_ROOT / "examples" / "vrm_open_world.py").read_text(encoding="utf-8")
    assert "SEA_LOOP_XZ" in src
    assert "(-28.0, 8.0)" in src
    assert "kagra.play_loop(" in src
    assert "kagra.set_listener(" in src
    assert "def _sync_listener" in src
    pose = src[src.index("    def _pose") :]
    nxt = pose.find("\n    def ", 10)
    pose = pose[:nxt] if nxt != -1 else pose
    assert "set_listener" not in pose
    se = src[src.index("def _se(") : src.index("def _place_gltf")]
    assert "pos is not None" in se
    assert 'kagra.play_se(path, volume=volume)' in se
    assert "_se(self.sfx, \"start\")" in src
    assert "_se(self.sfx, \"win\")" in src
    assert 'pos=(star.x' in src
    assert 'pos=(coin.x' in src


def test_crest_isle_poses_with_speed_blend_not_clip_snap():
    """Locomotion is set_locomotion(speed), not idle↔walk on a velocity threshold."""
    src = (_ROOT / "examples" / "vrm_open_world.py").read_text(encoding="utf-8")
    pose = src[src.index("    def _pose") :]
    nxt = pose.find("\n    def ", 10)
    pose = pose[:nxt] if nxt != -1 else pose
    assert "set_locomotion" in pose
    assert 'want = "walk" if moving else "idle"' not in pose
    assert "self.avatar.play(want" not in pose
    assert "walk_speed=2.2" in pose
    assert "run_speed=" in pose
    assert "hypot(p.vx, p.vz)" in pose


def test_spark_burst_spawns_expires_and_fades():
    burst = SparkBurst()
    n = burst.burst(1.0, 2.0, 3.0, count=8, life=0.4, seed=7)
    assert n == 8
    assert len(burst.sparks) == 8
    assert all(s.fade == 1.0 for s in burst.sparks)
    sizes0 = [s.draw_size for s in burst.sparks]
    burst.update(0.15)
    assert len(burst.sparks) == 8
    assert all(0.0 < s.fade < 1.0 for s in burst.sparks)
    sizes1 = [s.draw_size for s in burst.sparks]
    assert all(b < a for a, b in zip(sizes0, sizes1))
    items = burst.items()
    assert len(items) == 8
    assert all(len(it) == 4 and it[3] > 0 for it in items)
    burst.update(0.5)
    assert burst.sparks == []
    assert burst.items() == []


def test_crest_isle_ships_blob_sparks_and_tile_blend():
    src = (_ROOT / "examples" / "vrm_open_world.py").read_text(encoding="utf-8")
    assert "def _draw_blob" in src
    assert "quad_y_mesh" in src
    assert "skip_fog=True" in src
    assert "self.sparks.burst" in src
    assert "draw_billboard_instances(self.tex_spark" in src
    assert "terrain_uv_period = TERRAIN_UV_PERIOD" in src
    assert "terrain_uv_blend = TERRAIN_UV_BLEND" in src
    assert TERRAIN_UV_PERIOD != TILE
    assert TERRAIN_UV_BLEND > 1.0
    assert 0.0 < TERRAIN_UV_PAD < 0.2
    relic = (_ROOT / "examples" / "vrm_relic_run.py").read_text(encoding="utf-8")
    assert "terrain_uv_period" not in relic
    assert "terrain_uv_blend" not in relic
