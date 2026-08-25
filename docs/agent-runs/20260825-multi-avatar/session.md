# Session — Multi-avatar GPU share

## API search

- `docs/API_INDEX.md`: `avatar(path)`, `load_vrm`, `draw_vrm`, `render_stats`. No clone, no share, no N-body stats.
- `kagra-core/src/engine/mod.rs::load_vrm` always `extract_texture_data_from_glb` + `load_gltf_image` per call. No path cache (unlike `path_texture_cache` for file textures).
- `VrmPrimitive` already wraps VB/IB/morph in `Arc`, but each `load_vrm` creates new buffers.
- Skin palettes are already per-draw (`alloc_skin_palette` / `skin_palette_pool`). Comment in renderer: a shared UBO would freeze every avatar to the last palette.
- Mesh3D LRU (`MESH3D_TEX_BG_MAX = 256`, never evict live diffuse) is **Props**. VRM skinned draws bind `textures` + skin palettes. Hypothesis “second Alicia blows Mesh3D LRU” discarded.

## Approach

Same-path GPU template (`VrmGpuShare` + `VrmModel::instantiate`). Share VB/IB/morph/MToon/textures. Per instance: bones, springs, expressions, morph-weight UBO (`cached_weights = None`), skin palette on draw.

Crest Isle stays 1 player so this PR does not touch `examples/vrm_open_world.py` / locomotion / spatial audio (open #87 / #88). Spawn path is `examples/vrm_multi_avatar.py`.

## Files not touched (merge)

- Locomotion: `set_locomotion`, `_overlay_rots`, `kagra/vrm_action.py`, `walk_wish`
- Spatial: `kagra/spatial.py`, `kagra-core/src/audio.rs`, `set_listener` / `play_loop` / `spatial_mix`
- Crest Isle play script

## Verify

- `pytest tests -m "not golden"` — pass (this VM).
- `python tools/gen_api_index.py --check` — pass.
- GPU smoke: this cloud rustc is 1.83; `Cargo.lock` already pins `hashbrown 0.17.1` (edition2024). `maturin develop` / `kagra.verify` could not run here. Headless metric shipped: `kagra.vrm_gpu_stats()` + `scratch/multi_avatar_stats.json`.
- GitHub CI: **17 checks passed** on `3d649a0` (includes `golden` + `rust-test`). Desktop FPS still needs Emma's GPU HUD.
- Rebased onto `origin/master` after #87 and #88 merged. Kept locomotion (`set_locomotion` / upper overlay), spatial (`set_listener` / `play_loop`), and GPU share. Zero conflict markers. `pytest tests -m "not golden"` pass.
