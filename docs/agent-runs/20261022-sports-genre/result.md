# Sports GenreSession

- `SportsSession` (`kagra-shared/src/genre_session.rs`, `kagra-shared/src/py/sports.rs`)
- 1v1 pitch: `kagra-shared/tests/fixtures/sports_world.json`
- Example: `examples/sports_minimal.py`
- Smoke: `examples/verify_scenarios/sports_smoke.json`

Headless (lavapipe), scripted shot: `won=True score=2-0 rematch_phase=playing rematch_score=0`.

Screenshot: `scratch/sports.png` (YOU WIN 2-0, stick, KICK, knight and rogue).

`python -m kagra.verify examples/verify_scenarios/sports_smoke.json` — ok, offscreen not skipped (`scratch/sports_shared.png`).

Merged `origin/master` (including the survival session) before push. `cargo test -p kagra-shared --lib --locked` 474 passed. Verify loop fails only `demo_live_smoke` and `physics_stack_smoke`.
