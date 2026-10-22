KAGRA (Rust kagra-shared + Python). Read AGENTS.md ('how to add a genre'), kagra-shared/src/genre_session.rs, and the platformer/novel session bindings already on master first.

Goal: expose the existing Rust tower defense module (td.rs and related) to Python as a GenreSession, same pattern. Core logic in Rust. Deliver: py binding, examples/td_minimal.py (a small map, 5 waves, 2-3 tower types, gold, lives, win/lose and retry, tap-to-place towers plus mouse, bundled .glb assets), examples/verify_scenarios/td_smoke.json, HUD strings ASCII only.

Other agents work in parallel (race, puzzle): keep shared-file edits small and additive; before finishing, merge latest master into your branch (no force-push) and resolve conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass (demo_live, physics_stack, rpg_town already fail on master; ignore). Render a real headless screenshot with lavapipe and save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
