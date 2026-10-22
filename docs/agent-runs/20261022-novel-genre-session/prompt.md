KAGRA (Rust kagra-shared + Python). Read AGENTS.md (incl. the new 'how to add a genre' steps), kagra-shared/src/genre_session.rs and the puzzle_pad reference (PR #111) first.

Goal: expose the existing Rust novel / visual-novel module (kagra-shared/src/novel.rs and related) to Python as a GenreSession, following the puzzle_pad pattern and scripts/new_genre.py. Core logic stays in Rust (Android later can't run Python). Deliver: py/novel.rs binding, examples/novel_minimal.py (a short branching story with text box, name, choices, a background and a character, tap/click to advance so it works on touch), examples/verify_scenarios/novel_smoke.json. Japanese text must render correctly with the bundled fonts (check glyph fallback; no missing-glyph boxes).

Other agents are adding platformer and puzzle genres in parallel: keep edits to shared files (py/mod.rs, kagra/__init__, docs index) to small additive lines.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass. Try for a real headless screenshot (lavapipe/software adapter if no GPU), save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
