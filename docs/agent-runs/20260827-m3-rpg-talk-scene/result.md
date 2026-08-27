# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 144 passed (talk overlay + dump flag, door stays without flag, town->dungeon switch)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 539 passed, 10 deselected

python -m kagra.verify examples/verify_scenarios/rpg_town_smoke.json
# world + offscreen ok
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/rpg_town_world.json
```

Space starts. WASD walk. J / click near the blue NPC talks (flag). J near the door with the flag switches to the dungeon.
