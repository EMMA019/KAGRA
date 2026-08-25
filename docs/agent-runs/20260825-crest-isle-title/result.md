# Result — Crest Isle opaque title screen

## Cause

Title was not a missing string. `draw` composited the live island, then `fill(..., 118)`.

## Fix

- `examples/vrm_open_world.py` — `mode == "title"` cls + opaque banner, no world/props/VRM/water.
- `tests/test_open_world.py` — GPU-free source test that the title arm never calls those draw paths and uses `overlay_alpha=255`.
- Result overlay unchanged (alpha 118). SMOKE still starts in `play`. Meadow tint / cam clamp / IBL / rehold not touched.

## Verify

- `python3 tools/gen_api_index.py --check` → OK (422 entries)
- `python3 -m pytest tests -m "not golden"` → **400 passed**, 10 deselected in 2.99s (this VM has no `kagra_core` wheel; same job as CI `python-unit`)
- GitHub `python-unit` 3.11 failed on the first SHA: `"118" not in title_arm` matched the `alpha-118` comment. Test now checks the `_banner` call for `overlay_alpha=255` / not `overlay_alpha=118`.
- GPU `open_world_smoke` not run here (Rust extension missing). SMOKE still starts in `play`, so it would not screenshot the title anyway.

**GitHub CI: 17 checks passed** on `d9c7d94` (`cursor/crest-isle-title-screen-d128`), including `python-unit` 3.10/3.11/3.12 and golden.

PR: https://github.com/EMMA019/KAGRA/pull/84

## Files

- `examples/vrm_open_world.py`
- `tests/test_open_world.py`
- `docs/agent-runs/README.md`
- `docs/agent-runs/20260825-crest-isle-title/`
