# Result — Crest Isle meadow window

## Cause

PR #95 set `TERRAIN_UV_PERIOD=48` so each 16 m tile is a 2D window into `aerial_grass_rock_diff_1k.jpg`. The JPEG is mixed moss + brown rock **inside** pad 0.28. Adjacent tiles sampled different biomes → hard grass / dirt chunk edge (not barcode, not a missing mesh).

## Files

- `examples/open_world_rules.py` — `TERRAIN_UV_RECT=(0.535, 0.485, 0.640, 0.590)`, `terrain_uv()`
- `examples/vrm_open_world.py` — `world.terrain_uv_rect`
- `kagra/gamekit.py` / `kagra/world3d.py` / `kagra/__init__.py` — optional `uv_rect`
- Tests: `tests/test_open_world.py`, `tests/test_world3d.py`, `tests/data/aerial_grass_rock_128.rgb.z`

Period 48, pad 0.28, stream retry / prefetch / delayed unload / LOD upgrade / `LOD_CELLS=6` untouched. Relic Run UV defaults unchanged.

## Verify

```
python3 tools/gen_api_index.py --check   # OK (428 entries)
python3 -m pytest tests -m "not golden"  # 483 passed, 10 deselected
```

GPU smoke (`python -m kagra.verify examples/verify_scenarios/open_world_smoke.json`) not run: no `kagra_core` wheel in this environment. Acceptance is the GPU-free tinted-JPEG 3×3 TILE sample plus Emma's hill reading as continuous 草原.
