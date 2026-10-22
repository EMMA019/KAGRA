# Result — fly the island + backyard 3v3 CTF

## Artifacts

- `kagra/flight.py` / `examples/flight_minimal.py` / `examples/verify_scenarios/flight_smoke.json`
- `kagra/backyard_brawl.py` / `examples/backyard_brawl_minimal.py` / `examples/verify_scenarios/backyard_brawl_smoke.json`
- `WorldProp.pitch` / `roll` / `local`, `model: "ring"`
- Multi-primitive static glTF `baseColorFactor` palette + node transforms, KayKit skinned part filtering + `Walking_*` clip selection, and bundled asset aliases
- `kagra.gameloop.render_rgba` / `rgba_to_ppm`

## Verify

- `cargo test -p kagra-shared --lib --locked`: **398 passed**
- `cargo clippy -p kagra-shared --all-targets [--features render] --locked -- -D warnings`: clean
- `cargo fmt -p kagra-shared -- --check`: clean
- `pytest tests -m "not golden"`: passed
- `python tools/gen_api_index.py --check`: **56 entries**
- lavapipe: `flight_smoke` and `backyard_brawl_smoke` both **ok** (offscreen not skipped)
