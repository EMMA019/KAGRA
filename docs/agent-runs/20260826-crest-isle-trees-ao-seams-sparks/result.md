# Result — Crest Isle trees / blob / seams / sparks

## Files

- `kagra/gltf_mesh.py` — GLB-relative image URI + `KHR_texture_transform` UVs
- `kagra-core/src/gltf_common.rs`, `kagra-core/src/gltf.rs` — same for `load_gltf`
- `kagra/gamekit.py`, `kagra/world3d.py`, `kagra/__init__.py` — outside-tile normals, optional UV period/blend/pad
- `examples/open_world_rules.py` — `SparkBurst` + Crest UV knobs
- `examples/vrm_open_world.py` — blob, pickup burst, meadow UV
- Tests: `tests/test_gltf_mesh.py`, `test_open_world.py`, `test_gamekit.py`, `test_world3d.py`

## Play

```bash
python examples/vrm_open_world.py
```

Forest Kenney trees should show colormap (not black). Blob under feet. Meadow joins should not be a 16 m knife. Crest/coin pickup pops a short spark burst.

## Verify

```bash
python tools/gen_api_index.py --check   # OK (427 entries; heightfield kwargs)
pytest tests -m "not golden"            # 447 passed, 10 deselected
cargo test -p kagra-core --lib          # not run here: Cargo 1.83 cannot parse lockfile hashbrown 0.17.1 (edition2024)
```

GPU smoke (`python -m kagra.verify examples/verify_scenarios/open_world_smoke.json`) not run (no `kagra_core` wheel on this VM).

## Left out

SSAO, 4-cascade CSM, volumetric fog, Rapier, visual editor, Web/XR, Mixamo binaries. Sticky-walk quiet gap 3, Mesh3D LRU 256, chase cam clamp, opaque title, spatial listener, `set_locomotion` / `bind_locomotion` unchanged.
