# Result — Crest Isle opaque title screen

## Cause

Title was not a missing string. `draw` composited the live island, then `fill(..., 118)`.

## Fix

- `examples/vrm_open_world.py` — `mode == "title"` cls + opaque banner, no world/props/VRM/water.
- `tests/test_open_world.py` — GPU-free source test that the title arm never calls those draw paths and uses `overlay_alpha=255`.
- Result overlay unchanged (alpha 118). SMOKE still starts in `play`. Meadow tint / cam clamp / IBL / rehold not touched.

## Verify

(pending — pytest / API index / open_world_smoke)

## Files

- `examples/vrm_open_world.py`
- `tests/test_open_world.py`
- `docs/agent-runs/README.md`
- `docs/agent-runs/20260825-crest-isle-title/`
