# Result — Bar sim on a cut-over mainline

## Artifacts

- `kagra/bar_sim.py` / `examples/bar_sim_minimal.py`
- `kagra-shared/tests/fixtures/bar_room_world.json` (free world, no genre)
- `examples/verify_scenarios/bar_sim_smoke.json`
- `assets/bar/` (manifest + tex + se)
- `tests/test_bar_sim.py` / `tests/test_world_expect.py`

## Verify (this session)

- `cargo +stable test -p kagra-shared --locked --lib`: **394 passed**
- `cargo +stable clippy -p kagra-shared -- -D warnings`: clean
- `cargo +stable fmt -p kagra-shared -- --check`: clean
- `pytest tests -m "not golden"`: **636 passed, 9 skipped**
- `python tools/gen_api_index.py --check`: **54 entries**, clean
- `import kagra`: works without `kagra_core`; `WorldPlay` present; `get_engine` absent
- `python examples/bar_sim_minimal.py --headless /tmp/bar.png --days 3 --seed 1`: day 4, 650G, PNG 1251 bytes (solid fallback — no GPU adapter)
- `python -m kagra.verify examples/verify_scenarios/bar_sim_smoke.json`: **ok** (world dump); offscreen skipped (no helper)
- `python -m kagra.verify examples/verify_scenarios/blank_smoke.json`: **ok**

## Loops

One combined cutover + game slice.
