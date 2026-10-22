# Session

Card duel as a slim `GenreSession`, not a `WorldPlay` genre module. Picture stays a `WorldDoc` table (felt, `crate.glb` decks, chest, knight). Cards are HUD.

## Decisions

- Rules live in `kagra-shared/src/card.rs`. `dump` nests the full match under `card` (seed, rng, both decks, both hands, boards, grave, mana, life, log) so a later net client can resume. `view_json` hides the opponent hand and both deck orders (counts only).
- 1v1, life 20, mana +1 per turn capped at 10, opening 3 then the player draws (you 4, foe 3). Hand max 7 burns. Board max 5. Units have summoning sickness. Spells damage the foe or heal. Empty deck is fatigue. AI is greedy and synchronous inside `end_turn`, then the player's next turn starts. `confirm` on title or complete deals again with the same seed.
- Shuffle is SplitMix64 Fisher-Yates. Seed 0 is valid. `confirm` during `Playing` does nothing.
- HUD quads cannot rotate, so the fan is an arc of positions (edges lift, center sits lower). Flip squeezes width by `abs(cos)`. Mouse and touch share `begin` / `move` / `end` / `cancel`. `TableView` owns local tweens so tests do not touch the global motion registry.
- Chrome strings live in Rust (`カード対戦`, `ドラッグして出す`, `マナ`, `体力`, …) so the glyph test covers them. Python only prints `view_json` labels. No em dash / en dash.

## Stumbles

- `cargo test` takes one filter. `card:: genre_session::tests::card_session` did not run. Filter `card` did.
- `status()` matched `Option<u8>` without the rest of `Some`. A `_` arm fixed it.
- Tests that did `hand.push(make_inst(...))` double-borrowed. Locals first.
- The shuffle test called `confirm` while already `Playing`, so seed 2 was ignored. `deal()` after setting the seed.
- `static_copy` was dead outside tests. `#[cfg(test)]`.
- Headless `_greedy` was passed the `CardPlay` scene. It needs the session. The first lavapipe run died before the PNG.
- A fan step of `0.52 * card_w` stacked faces so the titles were unreadable. Step is now `0.78 * card_w`, the hand sits lower, and the hint sits between the board and the hand.
- Moving the knight to z=4.55 cropped the head out of frame. Restored z=3.15.
- `maturin develop` needs a virtualenv. `python3-venv` is not installed. `virtualenv` created `.venv` (gitignored).
- Running every smoke in one process made `update_mesh: N out of range` on flight / novel / platformer. Fresh processes pass. `rpg_town_smoke` fails on its own: the walker is named `hero`, the scenario asks for `player`. That mismatch is on master and was not edited here.
