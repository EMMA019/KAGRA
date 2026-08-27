# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# survival tests: dump, title, idle starve, camp fill/ok, ration pick, WASD ok, other genres

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline

pytest tests -m "not golden"

python -m kagra.play_world kagra-shared/tests/fixtures/survival_meter_world.json
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/survival_meter_world.json
```

Space / Enter / click starts. Hunger/stamina (`coins`) ticks down. WASD into the metal camp pad, or walk onto the ration box, to fill. Empty = dump-visible `starve`. Refill to full = dump-visible `ok` + flag. Chase camera. Indoor lights stay slots 0..3. Picture slice (contact blob + metal GGX) inherited. `--seconds` walks W so camp fill is visible without a human.

2D action is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/action_side_world.json
```

## Notes

- Path: **survival slice 1** on play_world. Sibling `survival.rs`. WorldPlay only dispatches (new/start/tick/hud). Did not invent APIs.
- Dump source of truth: capsule player, camp box, ration box, flag off until ok, chase camera name `chase`.
- No RendererV2, no VRM, no Rapier, no SSAO/GI, no new ECS. Picture slice intact.
- Did not land: crafting, inventory, multi-stat, Rapier, VRM hunger, Unity/Godot API.
