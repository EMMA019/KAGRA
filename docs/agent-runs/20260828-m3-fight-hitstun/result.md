# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 180 passed; 0 failed (8 fight tests: dump, title, attack+stun, KO+retry, player KO dump-visible, stun lock, dual camera, other genres)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 550 passed, 10 deselected

python -m kagra.play_world kagra-shared/tests/fixtures/fight_hitstun_world.json --width 640 --height 360 --seconds 2
# opened via rebuilt target/debug/examples/window.exe, exited 0
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/fight_hitstun_world.json
```

Space / Enter / click starts. WASD or arrows walk the capsule. J / Z / F / click attacks. Dual camera keeps both bodies in view. Overlay HP pips + hit flash. Dump-visible stun/KO/win (`name` stun/hurt/ko/win + opponent enable + `coins` hits). Indoor lights stay slots 0..3.

Collectathon is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/crest_isle_world.json
```

## Notes

- Attack / facing / stun / KO live in `fight.rs`. WorldPlay only dispatches (new/start/tick/hud).
- Dump source of truth: two capsules (player walker + opponent prop), ring floor, dual camera name `dual`.
- No RendererV2, no VRM, no Rapier, no new ECS. Action dodge-room loop untouched.
- Did not land: combo editor, specials, net, novel, vehicles, tower UI, multi-round timer, VRM fighters.
