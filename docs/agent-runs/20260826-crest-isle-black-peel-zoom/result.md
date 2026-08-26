# Result — Crest Isle black trees / peel / zoom

## Files

- `kagra/gltf_mesh.py` — glb-dir URI (not cwd); file-level unlit; multi-material color atlas
- `kagra-core/src/gltf_common.rs` — same URI contract + cwd unit test
- `kagra/world3d.py` — LOD swap without holes; upload-then-unload
- `kagra/play.py` — `Walk.zoom_chase`; water `skip_fog`; wheel zoom
- `kagra/camera3d.py` — `clamp_chase_arm`
- `kagra/stage.py` — denser outdoor sky sphere
- `kagra-core/src/renderer/mod.rs` — Mesh3D this-frame **or** live texture; morph BG this-frame
- `kagra-core/src/mtoon.rs` — hair `double_sided`
- `kagra-core/src/engine/mod.rs`, `input.rs` — `[` `]` `-` `=` key names
- `examples/vrm_open_world.py`, `open_world_rules.py` — zoom keys / HUD / `CAM_ZOOM_STEP`
- Tests: `test_gltf_mesh.py`, `test_open_world.py`, `test_play.py`, `test_camera3d.py`, `test_world3d.py`

## Play

```bash
python examples/vrm_open_world.py
```

Nature Kit bark-first trees (default / palm / pineTallA) should read brown+mint, not black chrome. Forest Kenney still uses colormap. Orbit/pan should not punch missing-tile rectangles, a white fog slab, or bald hair. `[` `]` / `-` `=` / wheel zoom the chase cam inside the existing 3D clamp.

## Verify

```bash
python tools/gen_api_index.py --check
pytest tests -m "not golden"
```

GPU smoke (`python -m kagra.verify examples/verify_scenarios/open_world_smoke.json`) if a `kagra_core` wheel is present.

## Left out

SSAO, 4-cascade CSM, volumetric fog, Rapier, visual editor, Web/XR. Did not swap Nature trees to a different Kenney; they color via the unlit atlas. Emma merges.
