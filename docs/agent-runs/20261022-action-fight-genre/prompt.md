KAGRA (Rust kagra-shared + Python). Read AGENTS.md ('how to add a genre'), kagra-shared/src/genre_session.rs, and the existing session bindings on master (platformer, novel, puzzle, race) first.

Goal: expose the existing Rust action and fight modules (action.rs, fight.rs and related) to Python as a GenreSession (one or two sessions, your call), same pattern. Core logic (hitboxes, HP, combos, enemy AI) in Rust. Deliver: py binding(s), examples/action_minimal.py (3D arena: defeat a few enemies then one boss, attack/dodge, HP bars, win/lose and retry, bundled KayKit .glb characters, keyboard plus on-screen stick and buttons for touch), examples/verify_scenarios/action_smoke.json, HUD strings ASCII only.

Other agents work in parallel (tower defense, rhythm): keep shared-file edits small and additive; before finishing, merge latest master into your branch (no force-push) and resolve conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass (demo_live, physics_stack, rpg_town already fail on master; ignore). Render a real headless screenshot with lavapipe and save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
