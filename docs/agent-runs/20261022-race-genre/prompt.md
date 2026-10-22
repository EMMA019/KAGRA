KAGRA (Rust kagra-shared + Python). Read AGENTS.md ('how to add a genre'), kagra-shared/src/genre_session.rs, and the puzzle_pad and platformer (PR #112) references first.

Goal: expose the existing Rust race stack (race.rs, vehicle.rs, driving.rs, road.rs and related) to Python as a GenreSession, same pattern. Core logic in Rust. Deliver: py binding, examples/race_minimal.py (a small 3-lap course with lap timer, a ghost of the best lap, bundled .glb car/track props, keyboard plus on-screen touch steering/accel), examples/verify_scenarios/race_smoke.json, HUD strings ASCII only.

Other agents work in parallel (novel, puzzle): keep shared-file edits (py/mod.rs, kagra/__init__, docs index) to small additive lines; before finishing, merge latest master into your branch (no force-push) and resolve conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass (demo_live, physics_stack, rpg_town already fail on master; ignore). Render a real headless screenshot with lavapipe and save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
