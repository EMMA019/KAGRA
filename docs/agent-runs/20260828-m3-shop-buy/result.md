# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# shop tests: dump, title, stall buy, ignore away, no-coins, no second spend, retry, held J/click buy, other genres

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline

pytest tests -m "not golden"

python -m kagra.play_world kagra-shared/tests/fixtures/shop_buy_world.json
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/shop_buy_world.json
```

Space / Enter / click starts. Player has coins. Stand at the stall. J / Z / F / click spends coins and lands dump-visible `bought` + flag, coins decreased. Overlay coin pip. Stall camera. Indoor lights stay slots 0..3. Picture slice (contact blob + metal GGX) inherited. `--seconds` holds attack so the buy is visible without a human.

Fish is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/fish_cast_world.json
```

## Notes

- Path: **shop slice 1 — buy with coins** on play_world. Sibling `shop.rs`. WorldPlay only dispatches (new/start/tick/hud). Did not invent APIs.
- Dump source of truth: capsule player, stall box, goods box, flag off until buy, `coins` start 8 / price 5, camera name `stall`.
- No RendererV2, no VRM, no Rapier, no SSAO/GI, no new ECS, no inventory grid, no net, no economy sim. Did not rewrite RPG. Picture slice intact.
- Did not land: full shop UI, inventory grid, net, economy sim.
