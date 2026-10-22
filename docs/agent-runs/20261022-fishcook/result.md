# Result

- `FishCookSession` on `kagra_shared` / `kagra.FishCookSession`
- `examples/fishcook_minimal.py` — cast, tension reel, cook, sell, day goal, result. Keyboard and tap/hold pads. `--headless` plays until the goal.
- `examples/verify_scenarios/fishcook_smoke.json`
- `kagra-shared/tests/fixtures/fishcook_world.json`

Headless (lavapipe), scripted day: `phase=complete won=True coins=8/8 sold=2`.

`cargo test -p kagra-shared --lib --locked`: 451 passed after merging action and fight.
`cargo clippy -p kagra-shared --all-targets --locked -- -D warnings`: clean.
`cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings`: clean.
`cargo clippy -p kagra-shared --all-targets --features python --locked -- -D warnings`: clean.
`pytest tests -m "not golden"`: passed.
`python -m kagra.verify` on `examples/verify_scenarios/*.json`: fishcook_smoke and the other genre smokes passed on lavapipe. Skipped the three that already fail on master: `demo_live_smoke`, `physics_stack_smoke`, `rpg_town_smoke`.

Screenshots:

- `/opt/cursor/artifacts/screenshots/fishcook_reel.png` — dock, hold bar, knight, mill, stall
- `/opt/cursor/artifacts/screenshots/fishcook_cook.png` — stove, heat bar
- `/opt/cursor/artifacts/screenshots/fishcook_result.png` — DAY END, goal met, coins 8/8
