KAGRA (Rust kagra-shared + Python). Read AGENTS.md ('how to add a genre'), kagra-shared/src/genre_session.rs, and the existing session bindings on master (platformer, novel, puzzle, race, td, rhythm) first.

Goal: expose the existing Rust fishing and cooking modules (fish.rs, cook.rs and related) to Python as a GenreSession, same pattern. Core logic in Rust. Deliver: py binding, examples/fishcook_minimal.py (a small loop: cast and reel with a timing/tension minigame, cook the catch, sell for coins, repeat; a day goal and result screen; bundled .glb assets; keyboard plus tap/hold for touch), examples/verify_scenarios/fishcook_smoke.json, HUD strings ASCII only.

Other agents work in parallel (action, RPG): keep shared-file edits small and additive; before finishing, merge latest master into your branch (no force-push) and resolve conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass (demo_live, physics_stack, rpg_town already fail on master; ignore). Render a real headless screenshot with lavapipe and save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
