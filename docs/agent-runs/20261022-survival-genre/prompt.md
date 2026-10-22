KAGRA (Rust kagra-shared + Python). Read AGENTS.md ('how to add a genre'), kagra-shared/src/genre_session.rs, and the existing session bindings on master (platformer, novel, puzzle, race, td, rhythm, action, fight, rpg, shop) first.

Goal: expose the existing Rust survival module (survival.rs and related) to Python as a GenreSession, same pattern. Core logic (day/night cycle, hunger, health, gathering, crafting, base building) in Rust. Deliver: py binding, examples/survival_minimal.py (survive 3 days: gather wood and food, craft a campfire, build a simple shelter, night is dangerous; HUD bars and a day counter; bundled .glb nature/props assets; keyboard plus on-screen stick and action buttons for touch), examples/verify_scenarios/survival_smoke.json, HUD strings ASCII only.

Other agents work in parallel (fishing/cooking, stealth): keep shared-file edits small and additive; before finishing, merge latest master into your branch (no force-push) and resolve conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass (demo_live, physics_stack already fail on master; ignore). Render a real headless screenshot with lavapipe and save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
