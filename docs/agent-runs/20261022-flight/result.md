# Result — fly the island

## Artifacts

- `kagra/flight.py` / `examples/flight_minimal.py`
- `examples/verify_scenarios/flight_smoke.json`
- `WorldProp.pitch` / `roll` / `local`, `model: "ring"`
- `kagra.gameloop.render_rgba` / `rgba_to_ppm`

## Verify

- `cargo test -p kagra-shared --lib --locked`: **397 passed**
- `cargo clippy -p kagra-shared --all-targets [--features render] --locked -- -D warnings`: clean
- `cargo fmt -p kagra-shared -- --check`: clean
- `pytest tests/test_flight.py tests/test_gameloop.py tests/test_api_bindings.py -m "not golden"`: passed
- `python tools/gen_api_index.py --check`: **56 entries**
- lavapipe: `examples/flight_minimal.py --headless scratch/flight.png` score 3, rendered PNG (not the solid fallback)
- `python -m kagra.verify examples/verify_scenarios/flight_smoke.json`: **ok**, offscreen not skipped (sky band + not_solid)
