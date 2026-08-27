# Result

## Commands

```text
cargo fmt -p kagra-shared
# ok

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo test -p kagra-shared --locked --offline --lib
# 274 passed; 0 failed (10 fight tests: dump, title, attack+stun, KO+retry, player KO dump-visible, stun lock, dual camera, guard block, 2-hit combo, other genres)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 571 passed, 10 deselected
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/fight_hitstun_world.json
```

Space / Enter / click starts. WASD or arrows walk the capsule. J / Z / F / click attacks. Hold Shift / C / K (`WalkInput.dodge`) to guard: incoming hit is blocked (dump name `block`, no damage / no KO). Two attacks in the combo window after a landed hit register as a combo (dump name `combo`, coins = hit count). Dual camera keeps both bodies in view. Overlay HP pips + hit flash (cyan on block). Dump-visible stun/KO/win/block/combo (`name` stun/hurt/block/combo/ko/win + opponent enable + `coins` hits). Indoor lights stay slots 0..3.

## Notes

- Attack / facing / stun / KO / guard / combo live in `fight.rs`. WorldPlay only dispatches (new/start/tick/hud). Did not touch world_play.rs.
- Dump source of truth: two capsules (player walker + opponent prop), ring floor, dual camera name `dual`.
- No RendererV2, no VRM, no Rapier, no new ECS, no SSAO / extra lights. Action dodge-room loop untouched.
- Did not land: combo editor, specials, net, multi-round timer, VRM fighters.
