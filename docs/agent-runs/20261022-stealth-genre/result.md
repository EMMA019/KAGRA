# Result

- Binding: `kagra-shared/src/py/stealth.rs` (`StealthSession`)
- Session: `StealthSession` in `kagra-shared/src/genre_session.rs`
- Logic: `kagra-shared/src/stealth.rs` (patrols, cones, noise, alert, key, retry)
- Example: `examples/stealth_minimal.py`
- Fixture: `kagra-shared/tests/fixtures/stealth_world.json` (KayKit rogue, two knights, crate hide, dagger key, box exit)
- Smoke: `examples/verify_scenarios/stealth_smoke.json`
- Screenshot (lavapipe): `/opt/cursor/artifacts/stealth.png` — yard after the scripted escape (`clear=True`, `key=True`, alert 0). HUD is ASCII (`stealth - clear  key yes`, `WASD move   C crouch   alert 0%`) with the stick and CROUCH pad. Yellow vision cones sit on the two knights.

`cargo test -p kagra-shared --lib --locked`: 463 passed after merging fishcook.
Clippy `-D warnings` (`--all-targets`, default, `--features render`, and `--features python`): clean.
`pytest tests -m "not golden"`: 669 passed, 10 skipped (after the fishcook merge).
`python -m kagra.verify` on `examples/verify_scenarios/*smoke.json`: stealth_smoke, stealth_hide_smoke, and fishcook_smoke passed. `rpg_town_smoke` also passed. `demo_live_smoke` (missing `examples/demo_live_smoke.py`) and `physics_stack_smoke` (no `scratch/verify_physics.png`) still fail as on master.
