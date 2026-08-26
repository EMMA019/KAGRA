# Result — Crest Isle black tile + gold spec

## Code

- `kagra/world3d.py` — terrain upload pins Lambert (`metallic=0`, `roughness=1`) + `set_mesh_pbr`; `draw()` uses live `_tile_meshes`.
- `kagra-core/src/renderer/mod.rs` — mesh_mat slots init Lambert; pack list 1:1 with visible retained; slot from index.
- Tests: `tests/test_world3d.py` (Lambert kwargs / draw live tiles), `tests/test_open_world.py` (hillside TILE UV not degenerate / not a black texel; `uv_rect` applied), `tests/test_gamekit.py` (`uv_rect` window + degenerate raises), `tests/test_hdri.py` (coin PBR vs Lambert).
- `examples/verify_scenarios/open_world_smoke.json` — notes field (SMOKE still does not inspect hillside pixels).
- `CHANGELOG.md` Unreleased.

Did **not** raise `GRASS_TINT`. Did **not** touch Relic Run UV. Did **not** revert #94/#95/#96.

## Verify

GPU-free: `pytest tests -m "not golden"` → **490 passed, 10 deselected**. `python tools/gen_api_index.py --check` OK (428 entries, no new public names).

GPU smoke (`python -m kagra.verify examples/verify_scenarios/open_world_smoke.json`) not run here (no `kagra_core` wheel). `cargo test -p kagra-core` if the registry is reachable (`mesh_mat_lambert_is_not_coin_pbr`).

Acceptance is Emma's hillside: one 16 m chunk must stay meadow like its neighbors, with trees still sitting on the mesh — not a black quad with a gold spec streak.
