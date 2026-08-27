"""Crest Isle のルールと公開 API 規約。GPU 不要。"""
from __future__ import annotations

import math
import sys
from pathlib import Path

import pytest

_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(_ROOT / "examples"))

from open_world_rules import (
    AERIAL_GRASS_ALBEDO,
    AERIAL_GRASS_DIRT_RIM,
    CAM_DISTANCE,
    CAM_HEIGHT,
    CAM_LOOK_Y,
    CAM_MAX_DISTANCE,
    CAM_MIN_DISTANCE,
    CAM_ZOOM_STEP,
    COIN_XZ,
    GLTF_HALF_Y,
    GOLD_METALLIC,
    GOLD_ROUGHNESS,
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
    TERRAIN_UV_RECT,
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
    terrain_uv,
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
    assert len(VISTA_PROPS) >= 280


def test_vista_is_in_opening_frustum():
    # Chase cam looks +Z from START; keep a dense pack in that cone.
    in_shot = [
        (x, z) for _n, x, z, _s, _y, _c in VISTA_PROPS
        if -16.0 <= x <= 18.0 and -8.0 <= z <= 24.0
    ]
    assert len(in_shot) >= 200


def test_vista_kenney_is_varied_not_one_clone():
    names = [n for n, *_ in VISTA_PROPS]
    uniq = set(names)
    assert any("grass" in n for n in uniq)
    assert any("flower" in n for n in uniq)
    assert any("plant_bush" in n for n in uniq)
    assert any("pine" in n for n in uniq)
    assert any("rock" in n for n in uniq)
    trees = {n for n in uniq if "tree" in n}
    assert len(trees) >= 6
    assert sum(1 for n in names if "pine" in n) >= 6
    for n in uniq:
        assert n in GLTF_HALF_Y, n


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


def test_chunk_decor_is_varied_kenney_not_one_clone():
    names: set[str] = set()
    n = 0
    for ix in range(-5, 6):
        for iz in range(-5, 6):
            rows = chunk_decor(ix, iz)
            n += len(rows)
            names.update(r[0] for r in rows)
    assert n >= 200
    assert any("grass" in n for n in names)
    assert any("flower" in n for n in names)
    assert any("plant_bush" in n for n in names)
    assert any("pine" in n for n in names)
    trees = {x for x in names if "tree" in x}
    assert len(trees) >= 4
    for rel in names:
        assert rel in GLTF_HALF_Y, rel


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


def test_nature_unlit_trees_are_not_black_chrome():
    """#91 fixed forest colormap; Nature Kit bark-first pines stayed metallic=1."""
    gm = load_kagra_submodule("gltf_mesh")
    nature = _ROOT / "examples" / "assets" / "open_world" / "kenney" / "nature"
    for name in (
        "tree_pineTallA.glb",
        "tree_pineTallB.glb",
        "tree_default.glb",
        "tree_palm.glb",
        "tree_oak.glb",
        "tree_tall.glb",
        "tree_palmDetailedTall.glb",
    ):
        flat = gm.flatten_gltf(nature / name)
        assert flat.metallic < 0.05, (name, flat.metallic)
        assert flat.image is not None, name
        assert flat.image[:8] == b"\x89PNG\r\n\x1a\n"


def test_placed_crest_trees_flatten_colored():
    """Vista + chunk_decor trees must not flatten to untextured chrome."""
    gm = load_kagra_submodule("gltf_mesh")
    kenney = _ROOT / "examples" / "assets" / "open_world" / "kenney"
    rels = {row[0] for row in VISTA_PROPS if "tree" in row[0]}
    for ix in range(-4, 5):
        for iz in range(-4, 5):
            for row in chunk_decor(ix, iz):
                if "tree" in row[0]:
                    rels.add(row[0])
    assert "nature/tree_pineTallA.glb" in rels
    assert "nature/tree_default.glb" in rels
    assert "forest/tree.glb" in rels
    for rel in sorted(rels):
        path = kenney / rel
        assert path.is_file(), rel
        flat = gm.flatten_gltf(path)
        assert not (flat.image is None and flat.metallic > 0.5), rel
        if rel.startswith("nature/tree"):
            assert flat.metallic < 0.05, rel
            assert flat.image is not None, rel
        if rel.startswith(("forest/", "town/", "castle/")):
            assert flat.image is not None, rel
            assert flat.metallic < 0.05, rel


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
        "World",
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
        "zoom_chase",
        "CAM_ZOOM_STEP",
    ):
        assert name in text, name


def test_readme_sample_line():
    readme = (_ROOT / "README.md").read_text(encoding="utf-8")
    assert "python examples/vrm_open_world.py" in readme
    assert "python -m kagra.play_world" in readme
    ja = (_ROOT / "README.ja.md").read_text(encoding="utf-8")
    assert "python examples/vrm_open_world.py" in ja
    assert "python -m kagra.play_world" in ja


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
    assert "fn mesh3d_tex_ref_add" in src
    assert "fn mesh3d_tex_pinned" in src
    assert "ref>0" in src
    assert "Off-camera ≠ unreferenced" in src or "Off-camera this frame is not" in src
    rend = (_ROOT / "kagra-core" / "src" / "renderer" / "mod.rs").read_text(
        encoding="utf-8",
    )
    assert "mesh3d_tex_refs" in rend
    assert "mesh3d_tex_pinned" in rend
    assert "live_frame.contains(&k)" in rend
    assert "fn retained_mesh3d_tex_keys" in rend
    assert "textures.contains_key(&k.0)" not in rend
    up = rend.find("pub fn upload_mesh_3d")
    assert up != -1
    up_end = rend.find("fn retained_mesh3d_tex_keys", up)
    upload_body = rend[up:up_end]
    assert "ensure_mesh3d_tex_bg" in upload_body
    assert "unload_mesh_3d(id)" in upload_body
    assert "mesh3d_tex_bgs.contains_key" in upload_body
    win = (_ROOT / "kagra-core" / "src" / "window.rs").read_text(encoding="utf-8")
    assert "retain_mesh_texture" in win
    assert "release_mesh_texture" in win
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
    assert "def clamp_chase_arm" in cam
    assert "min_hit" in cam
    assert CAM_ZOOM_STEP > 0.0
    assert "def _zoom_input" in src
    assert '"BracketLeft"' in src
    assert '"BracketRight"' in src
    assert 'kagra.text("[ ] / - = / wheel  zoom"' in src
    assert "頭の中" in src


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
    assert "rings=32" in stage
    assert "segs=48" in stage
    play = (_ROOT / "kagra" / "play.py").read_text(encoding="utf-8")
    assert "skip_fog=True" in play
    assert "def zoom_chase" in play
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
    assert "terrain_uv_pad = TERRAIN_UV_PAD" in src
    assert "terrain_uv_rect = TERRAIN_UV_RECT" in src
    assert "terrain_uv_half = HALF" in src
    assert TERRAIN_UV_PERIOD > TILE
    assert TERRAIN_UV_PERIOD >= TILE * 2.0
    assert abs(TERRAIN_UV_PERIOD / TILE - round(TERRAIN_UV_PERIOD / TILE)) < 1e-9
    assert TERRAIN_UV_BLEND == 0.0
    assert TERRAIN_UV_PAD >= AERIAL_GRASS_DIRT_RIM
    assert TERRAIN_UV_PAD > 0.2
    u0, v0, u1, v1 = TERRAIN_UV_RECT
    assert u1 > u0 and v1 > v0
    assert min(u0, v0, 1.0 - u1, 1.0 - v1) >= AERIAL_GRASS_DIRT_RIM
    assert min(u0, v0) >= TERRAIN_UV_PAD
    assert max(u1, v1) <= 1.0 - TERRAIN_UV_PAD
    relic = (_ROOT / "examples" / "vrm_relic_run.py").read_text(encoding="utf-8")
    assert "terrain_uv_period" not in relic
    assert "terrain_uv_blend" not in relic
    assert "terrain_uv_half" not in relic
    assert "terrain_uv_rect" not in relic


def test_crest_meadow_uvs_stay_inside_jpeg_moss():
    """Vendored aerial_grass_rock has a dirt square-border; Crest UVs must miss it.

    Per-tile local 0..1 (uv_half = TILE/2, no period) is the bald rectangle:
    one 16 m stamp of the JPEG, ClampToEdge dirt everywhere else.
    """
    from tests.conftest import load_kagra_submodule

    kit = load_kagra_submodule("gamekit")
    kwargs = dict(
        tile=TILE, cells=16,
        uv_period=TERRAIN_UV_PERIOD,
        uv_blend=TERRAIN_UV_BLEND,
        uv_pad=TERRAIN_UV_PAD,
        uv_rect=TERRAIN_UV_RECT,
    )

    def fn(_x, _z):
        return 0.4

    verts_a, _ = kit.heightfield_tile(fn, 0.0, 0.0, **kwargs)
    verts_b, _ = kit.heightfield_tile(fn, TILE, 0.0, **kwargs)
    for v in verts_a + verts_b:
        edge = min(v[6], 1.0 - v[6], v[7], 1.0 - v[7])
        assert edge >= AERIAL_GRASS_DIRT_RIM - 1e-9, (v[0], v[2], v[6], v[7], edge)
        u0, v0, u1, v1 = TERRAIN_UV_RECT
        assert u0 - 1e-9 <= v[6] <= u1 + 1e-9
        assert v0 - 1e-9 <= v[7] <= v1 + 1e-9

    def at(verts, x, z):
        hits = [p for p in verts if abs(p[0] - x) < 1e-6 and abs(p[2] - z) < 1e-6]
        assert hits, (x, z)
        return hits[0]

    join_a, join_b = at(verts_a, TILE, 8.0), at(verts_b, TILE, 8.0)
    assert join_a[6] == pytest.approx(join_b[6], abs=1e-6)
    assert join_a[7] == pytest.approx(join_b[7], abs=1e-6)
    c0, c1 = at(verts_a, TILE * 0.5, 8.0), at(verts_b, TILE * 1.5, 8.0)
    # Meadow window is ~0.10 UV, so adjacent tile centers move ~0.03 U, not 0
    # (per-tile 0..1 restart would share U). Not a barcode-sized 0.05 of 0..1.
    assert abs(c0[6] - c1[6]) > 0.02


def test_crest_meadow_lod3_tile_is_not_a_barcode():
    """Emma's remaining ハゲ: 16 m tiles with 1-axis JPEG stretch (barcode).

    Period 9.5 < TILE ping-ponged the moss window inside one chunk. A
    lod_cells=3 triangle that straddled a fold had ΔU≈0 (or aspect ~8) so
    Nearest ClampToEdge smeared one texel column across the tile. Period
    must be > TILE so each chunk is a small 2D moss window; pad still
    skips the dirt rim. Not per-tile 0..1 UV.
    """
    kit = load_kagra_submodule("gamekit")

    def fn(_x, _z):
        return 0.4

    kwargs = dict(
        tile=TILE, cells=3,
        uv_period=TERRAIN_UV_PERIOD,
        uv_blend=TERRAIN_UV_BLEND,
        uv_pad=TERRAIN_UV_PAD,
        uv_rect=TERRAIN_UV_RECT,
        uv_half=HALF,
    )
    assert TERRAIN_UV_PERIOD > TILE
    u0, v0, u1, v1 = TERRAIN_UV_RECT
    win = min(u1 - u0, v1 - v0)
    # Compact meadow window (~0.10 UV). Period 48 still yields 2D triangles
    # (aspect ~1), not a 1-axis sliver. du is smaller than the old 0.44 pad
    # window; require a few texels, not 0.04 of the full photo.
    min_du = win * (TILE / 3.0) / TERRAIN_UV_PERIOD * 0.85
    for ix in range(-5, 5):
        for iz in range(-5, 5):
            verts, idx = kit.heightfield_tile(fn, ix * TILE, iz * TILE, **kwargs)
            for t in range(0, len(idx), 3):
                tri = idx[t:t + 3]
                us = [verts[i][6] for i in tri]
                vs = [verts[i][7] for i in tri]
                du = max(us) - min(us)
                dv = max(vs) - min(vs)
                assert du > min_du and dv > min_du, (ix, iz, du, dv, us, vs)
                aspect = max(du, dv) / min(du, dv)
                assert aspect < 3.0, (ix, iz, aspect, du, dv)
                for u, v in zip(us, vs):
                    edge = min(u, 1.0 - u, v, 1.0 - v)
                    assert edge >= AERIAL_GRASS_DIRT_RIM - 1e-9, (ix, iz, u, v)
                    assert u0 - 1e-9 <= u <= u1 + 1e-9
                    assert v0 - 1e-9 <= v <= v1 + 1e-9


def test_period_below_tile_makes_lod3_barcode():
    """The #94 period 9.5 < TILE sliver is what Emma's screenshot showed."""
    kit = load_kagra_submodule("gamekit")

    def fn(_x, _z):
        return 0.4

    verts, idx = kit.heightfield_tile(
        fn, 16.0, 0.0, tile=TILE, cells=3,
        uv_period=9.5, uv_blend=0.0, uv_pad=0.28, uv_half=HALF,
    )
    skinny = 0
    for t in range(0, len(idx), 3):
        tri = idx[t:t + 3]
        us = [verts[i][6] for i in tri]
        vs = [verts[i][7] for i in tri]
        du = max(us) - min(us)
        dv = max(vs) - min(vs)
        aspect = max(du, dv) / max(min(du, dv), 1e-12)
        if du <= 0.04 or dv <= 0.04 or aspect >= 3.0:
            skinny += 1
    assert skinny >= 1


def test_crest_hillside_tile_uv_is_not_degenerate():
    """TILE-sized Crest chunks (LOD 6 and 8) must not collapse UV to a sliver.

    Zero-area / 1-axis UV is the barcode bug (#95). A black quad with a gold
    spec streak is a different failure (PBR/material); this locks that the
    hillside chunk around the peak is still a 2D window inside TERRAIN_UV_RECT.
    """
    kit = load_kagra_submodule("gamekit")
    land = load_kagra_submodule("land")
    fn = land.open_world_height
    u0, v0, u1, v1 = TERRAIN_UV_RECT
    win = min(u1 - u0, v1 - v0)
    kwargs_base = dict(
        tile=TILE,
        uv_period=TERRAIN_UV_PERIOD,
        uv_blend=TERRAIN_UV_BLEND,
        uv_pad=TERRAIN_UV_PAD,
        uv_rect=TERRAIN_UV_RECT,
        uv_half=HALF,
    )
    # Peak (8, 52) sits on tile (0, 3). Neighbors cover the visible hillside.
    keys = [(0, 3), (0, 2), (1, 3), (-1, 3), (0, 1)]
    for cells in (LOD_CELLS, 8):
        min_du = win * (TILE / float(cells)) / TERRAIN_UV_PERIOD * 0.85
        for ix, iz in keys:
            verts, idx = kit.heightfield_tile(
                fn, ix * TILE, iz * TILE, cells=cells, **kwargs_base,
            )
            assert verts and idx
            us = [v[6] for v in verts]
            vs = [v[7] for v in verts]
            span_u = max(us) - min(us)
            span_v = max(vs) - min(vs)
            assert span_u > min_du * 0.9 and span_v > min_du * 0.9, (
                ix, iz, cells, span_u, span_v,
            )
            aspect = max(span_u, span_v) / max(min(span_u, span_v), 1e-12)
            assert aspect < 3.0, (ix, iz, cells, aspect, span_u, span_v)
            for u, v in zip(us, vs):
                assert u0 - 1e-9 <= u <= u1 + 1e-9, (ix, iz, u, v)
                assert v0 - 1e-9 <= v <= v1 + 1e-9, (ix, iz, u, v)
            for v in verts:
                nx, ny, nz = v[3], v[4], v[5]
                leng = math.sqrt(nx * nx + ny * ny + nz * nz)
                assert leng == pytest.approx(1.0, abs=1e-4)
                assert ny > 0.2, (ix, iz, cells, nx, ny, nz)
            for t in range(0, len(idx), 3):
                tri = idx[t:t + 3]
                tu = [verts[i][6] for i in tri]
                tv = [verts[i][7] for i in tri]
                du = max(tu) - min(tu)
                dv = max(tv) - min(tv)
                area = du * dv
                assert area > 1e-8, (ix, iz, cells, du, dv, tu, tv)
                assert du > min_du * 0.5 and dv > min_du * 0.5, (
                    ix, iz, cells, du, dv,
                )


def test_crest_hillside_jpeg_window_is_not_a_black_texel():
    """Nearest ClampToEdge must not sample a near-black texel for a TILE window."""
    kit = load_kagra_submodule("gamekit")
    land = load_kagra_submodule("land")
    w, h, rgb = _load_aerial_rgb()
    fn = land.open_world_height
    lum_min = 1.0
    kwargs = dict(
        tile=TILE, cells=8,
        uv_period=TERRAIN_UV_PERIOD,
        uv_blend=TERRAIN_UV_BLEND,
        uv_pad=TERRAIN_UV_PAD,
        uv_rect=TERRAIN_UV_RECT,
        uv_half=HALF,
    )
    verts, _ = kit.heightfield_tile(fn, 0.0, 3.0 * TILE, **kwargs)
    for v in verts:
        r, g, b = _tinted_jpeg_at(rgb, w, h, v[6], v[7])
        lum = 0.2126 * r + 0.7152 * g + 0.0722 * b
        lum_min = min(lum_min, lum)
        assert lum > 0.20, (v[0], v[2], v[6], v[7], r, g, b, lum)
        assert g > r and g > b
    assert lum_min > 0.20


def test_world3d_draw_source_uses_tile_meshes():
    src = (_ROOT / "kagra" / "world3d.py").read_text(encoding="utf-8")
    assert "live_tiles" in src
    assert "metallic=0.0" in src
    assert "roughness=1.0" in src
    assert "set_mesh_pbr" in src
    assert "GOLD_METALLIC" not in src


def test_uv_rect_kwarg_is_applied_not_ignored():
    """If gamekit dropped uv_rect, ping-pong would fill 0..1 (or pad), not RECT."""
    kit = load_kagra_submodule("gamekit")
    rect = (0.40, 0.41, 0.52, 0.53)

    def fn(_x, _z):
        return 0.4

    verts, _ = kit.heightfield_tile(
        fn, 0.0, 0.0, tile=TILE, cells=8,
        uv_period=TERRAIN_UV_PERIOD, uv_rect=rect,
    )
    assert verts
    for v in verts:
        assert rect[0] - 1e-9 <= v[6] <= rect[2] + 1e-9
        assert rect[1] - 1e-9 <= v[7] <= rect[3] + 1e-9
    with pytest.raises(ValueError, match="degenerate"):
        kit.heightfield_tile(
            fn, 0.0, 0.0, tile=TILE, cells=4, uv_period=48.0,
            uv_rect=(0.5, 0.5, 0.5, 0.6),
        )


def _load_aerial_rgb() -> tuple[int, int, bytes]:
    """Decode the vendored 1K JPEG, or a 128² box-average fallback (CI)."""
    import shutil
    import subprocess
    import zlib

    path = (
        _ROOT / "examples" / "assets" / "relic_run" / "polyhaven"
        / "aerial_grass_rock_diff_1k.jpg"
    )
    ffmpeg = shutil.which("ffmpeg")
    if ffmpeg:
        raw = subprocess.check_output(
            [
                ffmpeg, "-hide_banner", "-loglevel", "error",
                "-i", str(path),
                "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1",
            ]
        )
        if len(raw) == 1024 * 1024 * 3:
            return 1024, 1024, raw
    zpath = _ROOT / "tests" / "data" / "aerial_grass_rock_128.rgb.z"
    raw = zlib.decompress(zpath.read_bytes())
    n = 128
    assert len(raw) == n * n * 3
    return n, n, raw


def _tinted_jpeg_at(raw: bytes, w: int, h: int, u: float, v: float) -> tuple[float, float, float]:
    x = int(max(0, min(w - 1, float(u) * w)))
    y = int(max(0, min(h - 1, float(v) * h)))
    i = (y * w + x) * 3
    r, g, b = raw[i] / 255.0, raw[i + 1] / 255.0, raw[i + 2] / 255.0
    return r * GRASS_TINT[0], g * GRASS_TINT[1], b * GRASS_TINT[2]


def test_crest_meadow_tinted_jpeg_has_no_tile_biome_step():
    """Adjacent 16 m Crest tiles must not jump lush grass → yellowish dirt.

    ``aerial_grass_rock_diff_1k.jpg`` is mixed moss + brown rock even inside
    pad 0.28. Period 48 made each TILE a different 2D window of that interior
    (Emma's remaining ハゲ after #95). Crest UVs ping-pong into TERRAIN_UV_RECT
    only. Relic Run keeps the uncropped JPEG.
    """
    kit = load_kagra_submodule("gamekit")
    w, h, rgb = _load_aerial_rgb()
    u0, v0, u1, v1 = TERRAIN_UV_RECT

    def fn(_x, _z):
        return 0.4

    kwargs = dict(
        tile=TILE, cells=8,
        uv_period=TERRAIN_UV_PERIOD,
        uv_blend=TERRAIN_UV_BLEND,
        uv_pad=TERRAIN_UV_PAD,
        uv_rect=TERRAIN_UV_RECT,
        uv_half=HALF,
    )
    means: dict[tuple[int, int], tuple[float, float, float]] = {}
    for iz in range(-1, 2):
        for ix in range(-1, 2):
            gs: list[float] = []
            bs: list[float] = []
            rs: list[float] = []
            verts, _ = kit.heightfield_tile(fn, ix * TILE, iz * TILE, **kwargs)
            for v in verts:
                u, vv = v[6], v[7]
                assert u0 - 1e-9 <= u <= u1 + 1e-9
                assert v0 - 1e-9 <= vv <= v1 + 1e-9
                tu, tv = terrain_uv(v[0], v[2])
                assert tu == pytest.approx(u, abs=1e-6)
                assert tv == pytest.approx(vv, abs=1e-6)
            # Dense world samples (not only mesh verts) across the 16 m tile.
            for j in range(12):
                for i in range(12):
                    x = ix * TILE + (i + 0.5) * (TILE / 12.0)
                    z = iz * TILE + (j + 0.5) * (TILE / 12.0)
                    u, vv = terrain_uv(x, z)
                    r, g, b = _tinted_jpeg_at(rgb, w, h, u, vv)
                    rs.append(r)
                    gs.append(g)
                    bs.append(b)
            n = float(len(gs))
            means[ix, iz] = (sum(rs) / n, sum(gs) / n, sum(bs) / n)

    greens = [m[1] for m in means.values()]
    blues = [m[2] for m in means.values()]
    # After GRASS_TINT the meadow is G-dominant with low B (not yellow dirt).
    assert min(greens) > 0.50
    assert max(blues) < 0.05
    for (ix, iz), (r, g, b) in means.items():
        assert g > r and g > b, (ix, iz, r, g, b)
        if (ix + 1, iz) in means:
            nbr = means[ix + 1, iz]
            assert abs(g - nbr[1]) < 0.08, (ix, iz, g, nbr[1])
            assert abs(b - nbr[2]) < 0.025, (ix, iz, b, nbr[2])
        if (ix, iz + 1) in means:
            nbr = means[ix, iz + 1]
            assert abs(g - nbr[1]) < 0.08, (ix, iz, g, nbr[1])
            assert abs(b - nbr[2]) < 0.025, (ix, iz, b, nbr[2])

    # Pad-0.28 interior (the #95 mapping) still has a dirt-biome tile step.
    # That is the bug this rect exists to close; keep the contrast locked.
    pad_b: list[float] = []
    for ix in (0, 1):
        acc = 0.0
        n = 0
        for j in range(8):
            for i in range(8):
                x = ix * TILE + (i + 0.5) * (TILE / 8.0)
                z = (j + 0.5) * (TILE / 8.0)
                t = x / TERRAIN_UV_PERIOD
                nt = math.floor(t)
                f = t - nt
                pu = (1.0 - f) if int(nt) % 2 else f
                t = z / TERRAIN_UV_PERIOD
                nt = math.floor(t)
                f = t - nt
                pv = (1.0 - f) if int(nt) % 2 else f
                u = TERRAIN_UV_PAD + pu * (1.0 - 2.0 * TERRAIN_UV_PAD)
                vv = TERRAIN_UV_PAD + pv * (1.0 - 2.0 * TERRAIN_UV_PAD)
                _, _, b = _tinted_jpeg_at(rgb, w, h, u, vv)
                acc += b
                n += 1
        pad_b.append(acc / n)
    assert abs(pad_b[0] - pad_b[1]) > 0.02, pad_b


def test_crest_gold_orbs_use_metal_not_plastic():
    """Crest coins are gold PBR spheres, not Kenney yellow discs."""
    assert GOLD_METALLIC >= 0.95
    assert GOLD_ROUGHNESS <= 0.14
    src = (_ROOT / "examples" / "vrm_open_world.py").read_text(encoding="utf-8")
    assert 'kagra.Prop(' in src
    assert '"sphere"' in src
    assert 'color="gold"' in src
    assert "GOLD_METALLIC" in src
    assert "GOLD_ROUGHNESS" in src
    assert 'dungeon/coin.glb' not in src
    assert "metallic=0.85" not in src
    look = (_ROOT / "kagra" / "look.py").read_text(encoding="utf-8")
    outdoor = look[look.index("def apply_outdoor_look") :]
    nxt = outdoor.find("\ndef ", 10)
    outdoor = outdoor[:nxt] if nxt != -1 else outdoor
    assert "set_rim" not in outdoor


def test_mtoon_boosts_hair_rim_not_face():
    """Hair material rim is lifted; backface flip stays; face names are skipped."""
    mtoon = (_ROOT / "kagra-core" / "src" / "mtoon.rs").read_text(encoding="utf-8")
    assert "fn is_hair_material" in mtoon
    assert "fn boost_hair_rim" in mtoon
    assert 'lower.contains("hair")' in mtoon
    assert '"face"' in mtoon
    assert "is_hair_material(material_name(mat))" in mtoon
    assert "ds = true" in mtoon
    assert "fn hair_orbit_uses_double_sided" in mtoon
    shaders = (_ROOT / "kagra-core" / "src" / "renderer" / "shaders.rs").read_text(
        encoding="utf-8",
    )
    assert "@builtin(front_facing) front" in shaders
    assert "if !front" in shaders
    assert "not_skin" in shaders
    assert "rim_lift < 0.05" in shaders
