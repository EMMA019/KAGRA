# Result — Crest Isle trees / blob / seams / sparks

## Files

- `kagra/gltf_mesh.py` — GLB-relative image URI + `KHR_texture_transform` UVs
- `kagra-core/src/gltf_common.rs`, `kagra-core/src/gltf.rs` — same for `load_gltf`
- `kagra/gamekit.py`, `kagra/world3d.py`, `kagra/__init__.py` — outside-tile normals, optional UV period/blend/pad
- `examples/open_world_rules.py` — `SparkBurst` + Crest UV knobs
- `examples/vrm_open_world.py` — blob, pickup burst, meadow UV, gold PBR coin spheres
- `kagra-core/src/mtoon.rs` — hair-only rim boost (Face skipped)
- `kagra-core/src/renderer/shaders.rs` — unnamed-hair albedo silhouette; backface flip kept
- Tests: `tests/test_gltf_mesh.py`, `test_open_world.py`, `test_gamekit.py`, `test_world3d.py`

## Play

```bash
python examples/vrm_open_world.py
```

Forest Kenney trees should show colormap (not black). Blob under feet. Meadow joins should not be a 16 m knife. Crest/coin pickup pops a short spark burst. Hair rim on Alicia/Emma (not a white face). Coins read as gold metal spheres.

## Verify

```bash
python tools/gen_api_index.py --check   # OK (427 entries)
pytest tests -m "not golden"            # 449 passed, 10 deselected
cargo test -p kagra-core --lib          # blocked: Cargo 1.83 / hashbrown 0.17.1 edition2024
```

GPU smoke (`python -m kagra.verify examples/verify_scenarios/open_world_smoke.json`) not run here (no `kagra_core` wheel). GitHub CI on PR #91 head `20fd713`: **17 checks green** (trees/blob/seams/sparks + hair rim + gold orbs). Prior commit `1ad693e` was also 17 green.

## Left out

SSAO, 4-cascade CSM, volumetric fog, Rapier, visual editor, Web/XR, Mixamo binaries. Sticky-walk quiet gap 3, Mesh3D LRU 256, chase cam clamp, opaque title, spatial listener, `set_locomotion` / `bind_locomotion` unchanged. Did not raise global `set_rim`. Kenney crests stay flags/banners. Relic Run gold orbs not retuned.
