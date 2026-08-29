"""Multi-avatar GPU share. GPU 不要（ソース + 純関数）。"""
from __future__ import annotations

from pathlib import Path

from tests.conftest import load_kagra_submodule

ROOT = Path(__file__).resolve().parents[1]
crowd = load_kagra_submodule("vrm_crowd")


def test_crowd_offsets_are_unique_and_off_origin():
    offs = crowd.crowd_offsets(4)
    assert len(offs) == 4
    assert len(set(offs)) == 4
    assert (0.0, 0.0) not in offs
    for x, z in offs:
        assert (x * x + z * z) ** 0.5 > 1.0


def test_crowd_count_clamps():
    assert crowd.crowd_count("0") == 1
    assert crowd.crowd_count("99") == 32
    assert crowd.crowd_count(None) == crowd.DEFAULT_COUNT


def test_same_path_share_invariant():
    prims = 12
    n = 4
    good = {
        "live": n,
        "unique_paths": 1,
        "shared_instances": n - 1,
        "primitives": prims * n,
        "vertex_buffers": prims,
        "textures": 6,
    }
    bad = {
        "live": n,
        "unique_paths": 1,
        "shared_instances": 0,
        "primitives": prims * n,
        "vertex_buffers": prims * n,
        "textures": 6 * n,
    }
    assert crowd.same_path_is_shared(good)
    assert not crowd.same_path_is_shared(bad)
    assert crowd.gpu_uniques_do_not_scale(
        {"live": 1, "vertex_buffers": prims, "textures": 6},
        {"live": n, "vertex_buffers": prims, "textures": 6},
    ) == []
    fails = crowd.gpu_uniques_do_not_scale(
        {"live": 1, "vertex_buffers": prims, "textures": 6},
        {"live": n, "vertex_buffers": prims * n, "textures": 6 * n},
    )
    assert fails


def test_engine_shares_same_path_before_reupload():
    engine = (ROOT / "old" / "kagra-core" / "src" / "engine" / "mod.rs").read_text(
        encoding="utf-8",
    )
    vrm = (ROOT / "old" / "kagra-core" / "src" / "vrm.rs").read_text(encoding="utf-8")
    assert "fn instantiate" in vrm
    assert "cached_weights: None" in vrm
    assert "pub struct VrmGpuShare" in vrm
    assert "vrm_share" in engine
    load = engine[engine.index("pub fn load_vrm") : engine.index("pub fn unload_vrm")]
    hit = load.index("entry.template.instantiate()")
    miss = load.index("extract_texture_data_from_glb(path)")
    assert hit < miss
    assert "pub fn vrm_gpu_stats" in engine


def test_mesh3d_lru_is_not_the_vrm_path():
    src = (ROOT / "old" / "kagra-core" / "src" / "renderer" / "gpu_helpers.rs").read_text(
        encoding="utf-8",
    )
    assert "MESH3D_TEX_BG_MAX: usize = 256" in src
    assert "VRM skinned draws do **not** use this cache" in src
    rend = (ROOT / "old" / "kagra-core" / "src" / "renderer" / "mod.rs").read_text(
        encoding="utf-8",
    )
    assert "fn alloc_skin_palette" in rend
    assert "ドローごとに専用バッファ" in rend or "skin_palette_pool" in rend


def test_crest_isle_stays_single_player():
    src = (ROOT / "old" / "examples" / "vrm_open_world.py").read_text(encoding="utf-8")
    assert src.count("kagra.avatar(") == 1
    assert "KAGRA_AVATARS" not in src
    assert 'self.mode = "play" if SMOKE else "title"' in src


def test_example_is_public_api_and_measures():
    src = (ROOT / "old" / "examples" / "vrm_multi_avatar.py").read_text(encoding="utf-8")
    assert "kagra.avatar(" in src
    assert "kagra.vrm_gpu_stats()" in src
    assert "kagra.draw_vrm(" in src
    assert "set_locomotion" not in src
    assert "set_listener" not in src
    assert "play_loop" not in src
    assert "spatial_mix" not in src
    py = (ROOT / "kagra" / "__init__.py").read_text(encoding="utf-8")
    assert "def vrm_gpu_stats()" in py
    assert "_engine.vrm_gpu_stats()" in py
