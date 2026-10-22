# Result — fly the island + backyard 3v3 CTF + studio 3D chess

## Artifacts

- `kagra/flight.py` / `examples/flight_minimal.py` / `examples/verify_scenarios/flight_smoke.json`
- `kagra/backyard_brawl.py` / `examples/backyard_brawl_minimal.py` / `examples/verify_scenarios/backyard_brawl_smoke.json`
- `kagra/chess_game.py` / `examples/chess_minimal.py` / `examples/verify_scenarios/chess_smoke.json`
- `WorldProp.pitch` / `roll` / `local`, `model: "ring"`
- PBR GGX specular + Fresnel environment reflections on `Material::Solid`, per-instance `metallic`/`roughness`, and CPU mipmap chain generation on textures
- Multi-primitive static glTF `baseColorFactor` palette + node transforms, KayKit skinned part filtering + `Walking_*` clip selection, `#<mesh_index>` sub-mesh selector (`chess_king`, `chess_queen`, ...), and bundled asset aliases
- `kagra.gameloop.render_rgba` / `rgba_to_ppm`

## Verify

- `cargo test -p kagra-shared --lib --locked`: **398 passed**
- `cargo clippy -p kagra-shared --all-targets [--features render] --locked -- -D warnings`: clean
- `cargo fmt -p kagra-shared -- --check`: clean
- `pytest tests -m "not golden"`: passed
- `python tools/gen_api_index.py --check`: **56 entries**
- lavapipe: `flight_smoke`, `backyard_brawl_smoke`, and `chess_smoke` all **ok** (offscreen not skipped)
