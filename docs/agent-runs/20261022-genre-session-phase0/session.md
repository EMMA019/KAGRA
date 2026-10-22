# Phase 0 — GenreSession + py bridge split

## Prompt

Split `py.rs` into `py/mod.rs` + per-area files; define `GenreSession` trait;
reference `puzzle_pad` end-to-end (`PuzzlePadSession`, minimal example, glb crate);
`scripts/new_genre.py` + AGENTS.md steps.

## Decisions

- `GenreSession` wraps `WorldPlay` (no second tick loop). Android FFI can call the same Rust methods later.
- Reference genre: `puzzle_pad` — fixture crate uses `crate.glb` (collision still box AABB).
- Python class: `PuzzlePadSession` on `kagra_shared` / `kagra.PuzzlePadSession`.

## Verify (2026-10-22)

- `cargo test -p kagra-shared --lib --locked` — 400 passed
- `cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` — clean
- `cargo clippy -p kagra-shared --all-targets --locked --no-default-features --features render -- -D warnings` — clean (fixed `offscreen_render` Instance init)
- `cargo fmt -p kagra-shared -- --check` — clean
- `pytest tests -m "not golden"` — passed
- `python -m kagra.verify examples/verify_scenarios/puzzle_pad_smoke.json` — ok (offscreen skipped: no GPU adapter on agent VM)
- collectathon / chess / flight / backyard_brawl smokes — ok
