KAGRA (Rust kagra-shared + Python). Read AGENTS.md ('how to add a genre'), kagra-shared/src/genre_session.rs, and the existing session bindings on master first.

Goal (Phase 2-2): expose the existing Rust fps module (fps.rs and related) to Python as a GenreSession. Core logic (first-person camera/look, movement, hitscan, ammo/reload, enemy AI, HP) in Rust. Deliver: py binding, examples/fps_minimal.py (a small arena: clear a few waves of enemies, crosshair, ammo and HP UI, win/lose and retry; bundled KayKit .glb assets; mouse+WASD on PC, plus touch: left on-screen stick to move, right-side drag to look, FIRE and RELOAD buttons), examples/verify_scenarios/fps_smoke.json, HUD strings ASCII only.

Other agents work in parallel (sports genre, rapier physics): keep shared-file edits small and additive; before finishing, merge latest master into your branch (no force-push) and resolve conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass (demo_live, physics_stack known failures; ignore). Render a real headless screenshot with lavapipe and save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
