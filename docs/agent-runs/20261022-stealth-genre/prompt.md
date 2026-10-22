KAGRA (Rust kagra-shared + Python). Read AGENTS.md ('how to add a genre'), kagra-shared/src/genre_session.rs, and the existing session bindings on master (platformer, novel, puzzle, race, td, rhythm, action, fight) first.

Goal: expose the existing Rust stealth module (stealth.rs and related) to Python as a GenreSession, same pattern. Core logic (guard patrols, vision cones, noise, alert states) in Rust. Deliver: py binding, examples/stealth_minimal.py (sneak past patrolling guards, grab a key, escape through the exit; visible vision cones and an alert meter; caught means retry; bundled KayKit .glb assets; keyboard plus on-screen stick and a crouch button for touch), examples/verify_scenarios/stealth_smoke.json, HUD strings ASCII only.

Other agents work in parallel (RPG, fishing/cooking): keep shared-file edits small and additive; before finishing, merge latest master into your branch (no force-push) and resolve conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass (demo_live, physics_stack, rpg_town already fail on master; ignore). Render a real headless screenshot with lavapipe and save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
