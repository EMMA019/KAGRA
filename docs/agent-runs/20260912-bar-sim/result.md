# Result — Bar sim on a cut-over mainline

## Artifacts

- `kagra/bar_sim.py` / `examples/bar_sim_minimal.py`
- `kagra-shared/tests/fixtures/bar_room_world.json` (free world)
- `examples/verify_scenarios/bar_sim_smoke.json`
- `assets/bar/` (manifest + tex + se)
- `tests/test_bar_sim.py`

## Verify (this session)

See `result` after pytest / cargo test / `kagra.verify bar_sim_smoke.json`.
Offscreen PNG may skip without a GPU adapter.

## Loops

One combined cutover + game slice (not logged as separate engine PRs).
