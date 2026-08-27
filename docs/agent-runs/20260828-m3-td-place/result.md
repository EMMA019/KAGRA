# Result

## Commands

```text
cargo fmt -p kagra-shared
# ok

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok

cargo test -p kagra-shared --locked --offline --lib
# 281 passed; 0 failed (15 td tests: dump/slots/coins, title, path walk, tower hit,
# clear/retry, leak name, overview, place+spend, off-slot, insufficient coins,
# occupied slot, placed tower auto-fire, title no-place, retry restores)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 571 passed, 10 deselected
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/td_lane_world.json
```

Space / Enter / click starts. WASD walks. Stand on a green `slot` and J / click
places a tower for 5 coins (start 10). Overlay gold pips are remaining coins.
Dump shows new `prop:tower-N`, slot name `used`, and `coins`. Existing path walk
and auto-fire stay. Title/result same as other M3 genres.

## Notes

- Place + cost live in `td.rs`. WorldPlay only dispatches tick input and restores
  `td::START` coins after `refresh_coin_count` (same pattern as shop/fps).
- Slots are named `slot` (not puzzle `pad`). Cost is const COST; remaining is
  `doc.coins`.
- No RendererV2, no VRM, no Rapier, no new ECS, no extra lights/SSAO.
- Remaining TD: waves editor. Lives UI is leak name + overlay, not a lives counter.
