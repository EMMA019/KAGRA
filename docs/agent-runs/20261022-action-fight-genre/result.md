# Result

- `ActionSession` and `FightSession` on `kagra_shared` / `kagra.ActionSession` / `kagra.FightSession`
- Arena: `kagra-shared/tests/fixtures/action_world.json`
- Play: `examples/action_minimal.py` (WASD / J / K, on-screen stick, ATK, DODGE)
- Smoke: `examples/verify_scenarios/action_smoke.json`
- Headless (lavapipe): `scratch/action.png` (WIN, combo, HP bar, empty boss bar, KayKit bodies, stick and buttons) and `scratch/action_shared.png`

Hitboxes, HP, combos, minion chase, boss dash, and the 1v1 fight loop stay in Rust. Python draws input and the ASCII HUD.

`cargo test -p kagra-shared --lib --locked` 424 passed. Clippy `-D warnings` clean for default and `render`. `pytest tests -m "not golden"` passed. Verify scenarios passed except `demo_live`, `physics_stack`, and `rpg_town` (already failing on master).
