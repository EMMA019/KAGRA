KAGRA (Rust kagra-shared + Python). Read AGENTS.md ('how to add a genre'), kagra-shared/src/genre_session.rs, and the platformer/novel/puzzle session bindings already on master first.

Goal: expose the existing Rust rhythm module (rhythm.rs and related) to Python as a GenreSession, same pattern. Core logic (timing windows, judgement, combo, score) in Rust. Deliver: py binding, examples/rhythm_minimal.py (one short song chart with 4 lanes, perfect/good/miss judgement, combo and result screen, tap lanes on touch plus keys D F J K; if a bundled audio file exists use it, otherwise a generated click track or silent fixed-BPM timing is fine), examples/verify_scenarios/rhythm_smoke.json, HUD strings ASCII only.

Other agents work in parallel (race, tower defense): keep shared-file edits small and additive; before finishing, merge latest master into your branch (no force-push) and resolve conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default and render), pytest tests -m "not golden" and new plus existing verify scenarios pass (demo_live, physics_stack, rpg_town already fail on master; ignore). Render a real headless screenshot with lavapipe and save as artifact. Open a PR against master with a short Japanese summary. Do not merge.
