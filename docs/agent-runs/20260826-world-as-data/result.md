# Result

- Public: `World is World3D`. `world.query` / `dump` / `load`. Schema: `docs/schemas/world.json`.
- Verify: `examples/verify_scenarios/open_world_smoke.json` and `orb_rush_smoke.json` assert world state (PNG size stays smoke).
- Tests: `tests/test_world.py` (query / dump-load roundtrip / missing tile albedo_ok). `pytest tests -m "not golden"`: 497 passed. `python3 tools/gen_api_index.py --check`: OK (409 entries).
- Log of this mountain: this directory. Drawing / Rapier / SSAO / terrain stream not touched.
- PR title: World query/dump/load + replace 63% roadmap.
