# RPG and shop GenreSession

## Prompt

Expose `rpg.rs` (`town_gate`) and `shop.rs` (`shop_buy`) as GenreSessions. Village play: talk, turn battle, shop buy, level up. KayKit glb, keyboard and tap buttons, Japanese or ASCII with no missing glyphs. Fix `rpg_town_smoke` (walker name is `hero`).

## Decisions

- `RpgSession` wraps the existing `town_gate` world. Classic town (coins as HP, talk, key, door, turn) is unchanged. A prop named `village` turns on village mode: level, XP, ATK, DEF, max HP, gold in `doc.coins`, inventory, and a shop overlay. Stats live on `prop:stats`.
- Damage is `(atk - def).max(1)`. The foe has 6 HP; two hero hits win, grant 6 XP and 4 gold, and level to 2 (ATK 4, max HP 10). A win sets `won`, so WorldPlay goes to Complete. Retry reloads the seed (level 1, gold 20, no potion).
- Potion costs 8. A successful buy closes the shop so walking resumes (`blocks_walk` includes `shop_open`). First shop tap opens; second buys.
- `ShopSession` stays the stall genre (`shop_buy`, price 5). Shared spend math is `shop::try_spend`. HUD text is ASCII (`shop - gold N  price 5`, `BUY`).
- Village HUD is PixelMplus Japanese (`はじまりの村`, `店で薬を買え。魔物を倒せ。`, buttons 話す / 買う / 戦う / 持ち物) plus an ASCII status line `Lv N  HP a/b  ATK n  G n`. No U+2014. Every non-space glyph must rasterize.
- New dump `rpg_world.json` (does not replace `rpg_town_world.json`): knight, ally rogue, mage NPC, barbarian, inn, house, well, trees, rock, chest. `rpg_town_smoke` query now asks for walker name `hero`.
- `action_buttons` builds the four rects with `core::array::from_fn`. A `for i in 0..4` index loop failed `clippy::needless_range_loop`.

## Verify

- `cargo test -p kagra-shared --lib --locked` — 431 passed
- clippy `-D warnings` default, `render`, and `python` — clean
- `cargo fmt -p kagra-shared -- --check` — clean
- `pytest tests -m "not golden"` — 660 passed, 10 skipped
- Headless lavapipe: `examples/rpg_minimal.py --headless` talks, buys, fights, levels to 2 (gold 16), retries to level 1 / gold 20. Screenshot shows 村人 / 店で薬を買え。魔物を倒せ。 / 話す and the KayKit village.
- `rpg_smoke` and `rpg_town_smoke` ok. Other current smokes ok except `demo_live_smoke` (script missing) and `physics_stack_smoke` (png only when the physics feature is built).
