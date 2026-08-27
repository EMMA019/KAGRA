# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline
# 134 passed; 0 failed (7 action tests: hit, kill, dodge, die, retry)

cargo test -p kagra-shared --features render --locked --offline
# lib: 135 passed
# example offscreen: 9 passed, 1 failed (pre-existing driving grass pixel count;
#   not this slice). Crest isle + orb rush offscreen still ok.

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 535 passed, 10 deselected

python -m kagra.verify examples/verify_scenarios/action_arena_smoke.json
# world assertions ok (player, 2 foes, floor, light) + shared offscreen

python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json
# still ok
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/action_arena_world.json
```

Space / Enter / click starts. WASD walk. Click or J/Z/F attack. Shift/C dodge.
Death overlay + fallen capsule; camera stays on the body. Space retries.

Collectathon is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/crest_isle_world.json
```

## Notes

- Action loop lives in `kagra-shared/src/action.rs`. WorldPlay only dispatches.
- Dump source of truth: foe `enabled`, player `name` hurt/dead/player.
- No RendererV2, no VRM, no Rapier, no new ECS.
