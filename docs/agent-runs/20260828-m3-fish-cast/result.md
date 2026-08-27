# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# fish tests: dump, title, dock cast, wait bite, land catch, ignore off-dock, held J/click catch, other genres

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline

pytest tests -m "not golden"

python -m kagra.play_world kagra-shared/tests/fixtures/fish_cast_world.json
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/fish_cast_world.json
```

Space / Enter / click starts. Stand on the dock over the water plane. J / Z / F / click casts. After a short wait a bite. J / click lands dump-visible `catch` + flag + coins. Overlay wait pip. Dock camera. Indoor lights stay slots 0..3. Picture slice (contact blob + metal GGX) inherited. `--seconds` holds attack so cast → bite → catch is visible without a human.

Rhythm is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/rhythm_beat_world.json
```

## Notes

- Path: **fishing slice 1** on play_world. Sibling `fish.rs`. WorldPlay only dispatches (new/start/tick/hud). Did not invent APIs.
- Dump source of truth: capsule player, official `water_y` plane, dock box, bobber box (hidden until cast), flag off until catch, camera name `dock`.
- No RendererV2, no VRM, no Rapier, no SSAO/GI, no new ECS, no inventory, no net. Picture slice intact.
- Did not land: full fishing sim, inventory, net, VRM.

