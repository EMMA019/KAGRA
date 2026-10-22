# Result

- `kagra.PuzzleSession` / `kagra_shared.PuzzleSession` — slide, grid, match
- `examples/puzzle_minimal.py` — four levels, win, retry, keyboard + tap/swipe
- `examples/verify_scenarios/puzzle_smoke.json`
- `kagra-shared/tests/fixtures/puzzle_world.json` — title picture of level 1
- `puzzle_pad` crate-push loop unchanged

## Verify

- `cargo test -p kagra-shared --lib --locked` — 410 passed
- `cargo test -p kagra-shared --lib --features render --locked` — 416 passed
- `cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` — clean
- `cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings` — clean
- `pytest tests -m "not golden"` — passed (after the example docstring lost its em dash)
- `python -m kagra.verify examples/verify_scenarios/puzzle_smoke.json` — ok (lavapipe, pixels)
- `python -m kagra.verify examples/verify_scenarios/puzzle_pad_smoke.json` — ok
- `python examples/puzzle_minimal.py --headless scratch/puzzle_win.png` — `won=True phase=complete level=3 moves=4`

Screenshots: `scratch/puzzle_shared.png` (title 3x3), `scratch/puzzle_win.png` (solved 4x4).
