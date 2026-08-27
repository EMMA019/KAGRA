# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline
# 140 passed (jump/land, fall/die, checkpoint retry; action+collectathon still ok)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 537 passed, 10 deselected

python -m kagra.verify examples/verify_scenarios/box_hop_smoke.json
# world + offscreen ok
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/box_hop_world.json
```

Space starts / jumps. WASD walk. Fall into the pit to die; Space retries at last checkpoint.
