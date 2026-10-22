# Sports GenreSession

## Prompt

Expose `sports.rs` as a `GenreSession`, with a 1v1 example (score to win, result, rematch), KayKit characters, keyboard plus an on-screen stick and action button, ASCII HUD, and a smoke scenario. Shared edits stay additive.

## Decisions

- Class name `SportsSession`. Genre id stays `sports_goal` (`sports::GAME_ID`). The session wraps `WorldPlay`, same shape as `RaceSession`.
- The existing one-goal slice is unchanged: a dump with no `opponent` walker still ends on the first ball in `goal`. `SportsGame::from_doc` returns default there, so `match_mode` stays false.
- A dump with an `opponent` walker is a 1v1 kickabout. The same kinematic ball (impulse, damp, wall bounce, no Rapier) is shared. The north volume `goal` is the player's score; the south volume `net` is the opponent's. First to N (`win` prop scale.x, default 2) or the higher score when the `match` timer hits zero. Confirm on the result reloads the seed (rematch).
- Opponent chase, the aimed shot (`WalkInput.attack`, reach 2.35), and the clock live in `sports.rs`. The clock is mirrored onto the dump timer named `match`. Player shots have priority over the opponent's kick.
- KayKit `knight` (player) and `rogue` (opponent). `sit_named` now clones the walker so a glTF body is not replaced by a capsule each tick. Capsule pitches (empty `gltf`) still sit at `BODY_H`.
- On-screen stick uses `kagra.touch.VirtualPad` (screen-down is positive, so up on the stick is forward). KICK is attack. Keys: WASD, J/K, Space. HUD text is ASCII hyphen only (`YOU WIN  2-0`, `SPACE rematch`). `k` was added to the gameloop keymap.

## Verify

After merging `origin/master` (other genre sessions landed in parallel; shared files kept both sides):

- `cargo test -p kagra-shared --lib --locked` — 469 passed
- `cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` — clean
- `cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings` — clean
- `cargo clippy -p kagra-shared --features python --locked -- -D warnings` — clean
- `cargo fmt -p kagra-shared -- --check` — clean
- `python tools/gen_api_index.py --check` — 70 entries
- `pytest tests -m "not golden"` — 672 passed, 10 skipped
- `python examples/sports_minimal.py --headless` — won 2-0, rematch phase playing, score 0 (lavapipe)
- `python -m kagra.verify examples/verify_scenarios/sports_smoke.json` — ok, offscreen not skipped
- `sports_goal_smoke` still ok
- Other current smokes ok on lavapipe except two that already fail without this change: `demo_live_smoke` (script missing), `physics_stack_smoke` (png only when the physics feature is built)
