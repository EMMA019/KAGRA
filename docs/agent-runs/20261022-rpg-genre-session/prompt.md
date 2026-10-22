KAGRA (Rust kagra-shared + Python). Read AGENTS.md ('how to add a genre'), kagra-shared/src/genre_session.rs, and the existing session bindings on master (platformer, novel, puzzle, race, td) first.

Goal: expose the existing Rust RPG and shop modules (rpg.rs, shop.rs and related) to Python as a GenreSession, same pattern. Core logic (stats, turn battle, XP/level up, inventory, gold, shop) in Rust. Note the existing rpg_town_smoke fails on master because the walker is named 'hero'; fix that scenario as part of this PR. Deliver: py binding, examples/rpg_minimal.py (small village, talk to an NPC, a turn-based battle, buy an item in a shop, level up; bundled KayKit .glb assets; keyboard plus tap/on-screen buttons; Japanese or ASCII text must render without missing glyphs), examples/verify_scenarios/rpg_smoke.json.

Other agents work in parallel (rhythm, action): keep shared-file edits small and additive; before finishing, merge latest master into your branch (no force-push) and resolve conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass (demo_live, physics_stack already fail on master; ignore). Render a real headless screenshot with lavapipe and save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
