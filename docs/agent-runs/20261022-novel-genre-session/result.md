# Result

- Binding: `kagra-shared/src/py/novel.rs` (`NovelSession`)
- Example: `examples/novel_minimal.py`
- Smoke: `examples/verify_scenarios/novel_smoke.json`
- Stage: `kagra-shared/tests/fixtures/novel_world.json`
- Screenshot (lavapipe): `scratch/novel_minimal.png`

`python examples/novel_minimal.py --headless scratch/novel_minimal.png`
printed `phase=complete flag=stay speaker=ユキ` after the choice tap.

## Checks (2026-10-22)

- `cargo test -p kagra-shared --lib --locked` — 406 passed
- `cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` — clean
- same clippy with `--features render` and `--features python` — clean
- `cargo fmt -p kagra-shared -- --check` — clean
- `pytest tests -m "not golden"` — passed
- `python -m kagra.verify` on `examples/verify_scenarios/*.json` — novel_smoke, novel_pages, puzzle_pad, and the other genre smokes passed on lavapipe. Three scenarios already fail on this tree without novel changes: `demo_live_smoke` (missing script), `physics_stack_smoke` (no physics feature, png not written), `rpg_town_smoke` (fixture walker is `hero`, query asks for `player`).
