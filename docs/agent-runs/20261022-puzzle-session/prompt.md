KAGRA (Rust kagra-shared + Python). Read AGENTS.md (incl. the new 'how to add a genre' steps), kagra-shared/src/genre_session.rs and the puzzle_pad reference (PR #111) first.

Goal: puzzle_pad (crate-push room) is only the reference sample. Expose the existing Rust puzzle module (kagra-shared/src/puzzle.rs and related: grid / match / sliding etc., whatever it supports) to Python as a GenreSession so a full puzzle game can be built, following the same pattern. Core logic stays in Rust. Deliver: py binding for the puzzle module, examples/puzzle_minimal.py (a small complete puzzle with a few levels, win condition, retry, tap/swipe-friendly controls plus keyboard), examples/verify_scenarios/puzzle_smoke.json, HUD strings ASCII only. Don't break puzzle_pad.

Other agents are adding platformer and novel genres in parallel: keep edits to shared files (py/mod.rs, kagra/__init__, docs index) to small additive lines.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass. Try for a real headless screenshot (lavapipe/software adapter if no GPU), save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
