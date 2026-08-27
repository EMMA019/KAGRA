# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 148 passed (sprite dump → MESH_QUAD batch; box-hop checkpoint card on the same Scene3D)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 541 passed, 10 deselected

python -m kagra.verify examples/verify_scenarios/sprite_card_smoke.json
# world + offscreen ok
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/sprite_card_world.json
```

Standing XY cards + floor + capsule on the same WorldDoc / wgpu 30 window. WASD walks. No second renderer.

Follow-up (same runtime): box-hop dump includes `prop:flag-card` (`model: sprite`). Jump loop unchanged.

```text
python -m kagra.play_world kagra-shared/tests/fixtures/box_hop_world.json
```
