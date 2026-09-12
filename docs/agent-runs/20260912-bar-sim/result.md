# Result — Bar sim on a cut-over mainline

## Artifacts

- `kagra/bar_sim.py` / `examples/bar_sim_minimal.py`
- `kagra-shared/tests/fixtures/bar_room_world.json` (free world, no genre)
- `examples/verify_scenarios/bar_sim_smoke.json`
- `assets/bar/` (manifest + tex + se)
- `tests/test_bar_sim.py` / `tests/test_world_expect.py`

## Verify (this session)

- `cargo +stable test -p kagra-shared --locked`: **394 passed** (all targets)
- `cargo +stable clippy -p kagra-shared --all-targets [--features render] -- -D warnings`: clean
- `cargo +stable fmt -p kagra-shared -- --check`: clean
- `cargo build --target wasm32-unknown-unknown --features wasm,render --release`: ok
- root `maturin build --release` → `kagra-0.2.0-cp312-manylinux_2_35_x86_64.whl` (4.0 MB);
  `import kagra` from the wheel: `WorldPlay` present, `get_engine` absent
- `pytest tests -m "not golden"`: **636 passed, 9 skipped**
- `python tools/gen_api_index.py --check`: **54 entries**, clean
- Real render (lavapipe): `python -m kagra.render_world bar_room_world.json` ok;
  `examples/bar_sim_minimal.py --headless` writes a rendered 30 KB PNG
  (`rendered`, not fallback). Serve / message frames show room, shelf,
  three textured bottle sprites, portrait, choice menu, message window.
- `python -m kagra.verify examples/verify_scenarios/bar_sim_smoke.json`: **ok**,
  offscreen not skipped when an adapter exists
- `python -m kagra.verify examples/verify_scenarios/blank_smoke.json`: **ok**

## Loops

Two: cutover + game slice, then a screenshot pass that fixed the room dump
and the silent `draw_world` fallback.
