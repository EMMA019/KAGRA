# Result

- Binding: `kagra-shared/src/py/rpg.rs` (`RpgSession`), `kagra-shared/src/py/shop.rs` (`ShopSession`)
- Session: `RpgSession` / `ShopSession` in `kagra-shared/src/genre_session.rs`
- Example: `examples/rpg_minimal.py`
- Fixture: `kagra-shared/tests/fixtures/rpg_world.json`
- Smoke: `examples/verify_scenarios/rpg_smoke.json`
- `rpg_town_smoke` query walker name is `hero` (matches the fixture)
- Screenshot (lavapipe): `scratch/rpg_minimal.png` — はじまりの村 during talk (`村人` / `店で薬を買え。魔物を倒せ。`), knight, rogue, mage, barbarian, inn, house, buttons 話す / 買う / 戦う / 持ち物. Headless run then bought the potion (gold 12), won the turn battle, leveled to 2 (ATK 4, gold 16), and retry restored level 1 and gold 20.

`cargo test -p kagra-shared --lib --locked`: 431 passed.
