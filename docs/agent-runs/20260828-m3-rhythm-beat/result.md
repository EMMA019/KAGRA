# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# rhythm tests: dump, title, idle miss, window hit, ignore early press, four hits clear, held J/click clear, other genres

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline

pytest tests -m "not golden"

python -m kagra.play_world kagra-shared/tests/fixtures/rhythm_beat_world.json
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/rhythm_beat_world.json
```

Space / Enter / click starts. A metal marker marches the stage toward the judge. J / Z / F / click on the window while the marker is in the judge hits. Letting a beat pass is dump-visible `miss`. Four hits = dump-visible `clear` + flag + coins. Overlay lane + moving pip. Stage camera. Indoor lights stay slots 0..3. Picture slice (contact blob + metal GGX) inherited. `--seconds` holds attack so window hits are visible without a human.

Survival is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/survival_meter_world.json
```

## Notes

- Path: **rhythm slice 1** on play_world. Sibling `rhythm.rs`. WorldPlay only dispatches (new/start/tick/hud). Did not invent APIs.
- Dump source of truth: capsule player, stage box, moving marker box, judge box, flag off until clear, camera name `stage`.
- No RendererV2, no VRM, no Rapier, no SSAO/GI, no new ECS, no audio chart. Picture slice intact.
- Did not land: full music game, audio chart editor, net, VRM dance.
