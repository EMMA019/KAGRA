# Close remaining RPG on play_world

## Try
`python -m kagra.play_world kagra-shared/tests/fixtures/rpg_town_world.json`

J/click talks (grants flag+key). Space opens menu overlay; J uses/holds the key. Door with flag switches town↔dungeon. In the dungeon, J next to the enemy starts a turn overlay (hero HP in `coins`, foe HP bar `hp`); J is the fight action. Win/lose → result.

## Verify (Emma's Windows, 2026-08-28)
- `cargo fmt -p kagra-shared`
- `cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings`
- `cargo test -p kagra-shared --locked --offline --lib` — 269 passed
- `pytest tests -m "not golden"` — 571 passed, 10 deselected
- `cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --offline --locked`

## Closed vs remaining
Closed: menu overlay, party hero+ally in dump, talk-grant inventory + slots + held query, dump save roundtrip (party/inventory/flags/scene), turn combat HP/win. Town vs dungeon still distinct. Indoor dungeon lights 4 slots. Talk/menu/combat dump-visible overlay names.

Not this slice: net, VRM, Rapier, human editor, Unity parity, enemy.chase, RendererV2 picture, SSAO/GI.
