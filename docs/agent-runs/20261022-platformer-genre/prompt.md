KAGRA (Rust kagra-shared + Python). Read AGENTS.md (incl. the new 'how to add a genre' steps), kagra-shared/src/genre_session.rs and the puzzle_pad reference (PR #111) first.

Goal: expose the existing Rust platformer module (kagra-shared/src/platformer.rs and related) to Python as a GenreSession, following the puzzle_pad pattern and scripts/new_genre.py. Core logic stays in Rust (Android later can't run Python). Deliver: py/platformer.rs binding, examples/platformer_minimal.py (a small playable side-scrolling stage with jump, a few platforms, a goal, using bundled .glb assets, keyboard plus a simple on-screen touch control layout), examples/verify_scenarios/platformer_smoke.json, HUD strings ASCII only.

Other agents are adding novel and puzzle genres in parallel: keep edits to shared files (py/mod.rs, kagra/__init__, docs index) to small additive lines to avoid conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and the new plus existing verify scenarios pass. Try for a real headless screenshot (lavapipe/software adapter if no GPU) and save it as an artifact. Open a PR against master with a short Japanese summary. Do not merge.
