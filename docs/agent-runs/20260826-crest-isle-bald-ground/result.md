# Result — Crest Isle bald meadow

## Cause

`aerial_grass_rock_diff_1k.jpg` is a **non-tiling** aerial photo: mossy speckle in the interior, bare earth at UV 0/1 (square dirt rim ~0.12 UV, not 0.035). Sampler is ClampToEdge + Nearest. #91 ping-pong still mapped the full 0..1 JPEG onto the world, so the meadow was a rectangular grass island with an axis-aligned dirt frame.

## Files

- `examples/open_world_rules.py` — `AERIAL_GRASS_DIRT_RIM=0.12`, `TERRAIN_UV_PAD=0.28`, `TERRAIN_UV_PERIOD=9.5`, blend 0
- `examples/vrm_open_world.py` — also `terrain_uv_half = HALF`
- `kagra/gamekit.py` / `kagra/world3d.py` — comments only
- Tests: `tests/test_gamekit.py`, `tests/test_open_world.py`

Relic Run UV / #92 pin / Walk.wish untouched.

## Verify

```
python3 tools/gen_api_index.py --check   # OK (427 entries)
python3 -m pytest tests -m "not golden"  # 475 passed, 10 deselected (after merging #93)
```

GPU smoke (`python -m kagra.verify examples/verify_scenarios/open_world_smoke.json`) not run: no `kagra_core` wheel in this environment.
