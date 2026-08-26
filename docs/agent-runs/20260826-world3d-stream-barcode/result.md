# Result — World3D stream harden + Crest barcode

## Code

- `kagra/world3d.py` — failed upload not sticky; 1-tile prefetch; delayed unload; LOD upgrades before new far tiles.
- `examples/open_world_rules.py` — `TERRAIN_UV_PERIOD=48`, `LOD_CELLS=6`. Pad 0.28 unchanged.
- `examples/vrm_open_world.py` — comment only (still assigns the same public UV knobs).
- Tests: `tests/test_world3d.py` (retry / linger / prefetch / LOD order), `tests/test_open_world.py` (lod3 + Crest knobs not a barcode; period 9.5 *is*), `tests/test_gamekit.py` (period 48).

## Verify

GPU-free: `pytest tests -m "not golden"` → **482 passed, 10 deselected**. `python tools/gen_api_index.py --check` OK (428). No `kagra.verify` GPU pass in this environment (no `kagra_core` wheel). Acceptance is Emma's barcode rectangle vs world-continuous 2D moss windows.

Relic Run does not set `terrain_uv_period` / pad / half.

## Not done

- Did not merge.
- Did not Repeat the uncropped JPEG.
- Did not switch to per-tile 0..1 UV.
