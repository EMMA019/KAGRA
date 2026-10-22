KAGRA (Rust kagra-shared + Python). Read AGENTS.md ('how to add a genre'), kagra-shared/src/genre_session.rs, and the existing session bindings on master first.

Goal: expose the existing Rust sports module (sports.rs and related) to Python as a GenreSession, same pattern. Core logic (ball physics, scoring, simple opponent AI, match timer) in Rust. Deliver: py binding, examples/sports_minimal.py (a 1v1 mini-game such as soccer penalty, tennis or basketball, whichever sports.rs supports best; score to win, result screen and rematch; bundled KayKit .glb characters; keyboard plus on-screen stick and action button for touch), examples/verify_scenarios/sports_smoke.json, HUD strings ASCII only.

Other agents work in parallel (stealth, survival): keep shared-file edits small and additive; before finishing, merge latest master into your branch (no force-push) and resolve conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass (demo_live, physics_stack already fail on master; ignore). Render a real headless screenshot with lavapipe and save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
