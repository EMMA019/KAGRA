# Result

- `SurvivalSession` on `kagra_shared` / `kagra.SurvivalSession` (`genre: survival`)
- Clearing: `kagra-shared/tests/fixtures/survival_world.json`
- Play: `examples/survival_minimal.py`
- Smoke: `examples/verify_scenarios/survival_smoke.json` — ok on lavapipe, with the other current smokes after the `origin/master` merge
- Headless (lavapipe): `scratch/survival.png` — survived 3 days, campfire and shelter up, stick + GATHER / EAT / BUILD

Day/night, hunger, health, gathering, crafting, and the shelter are Rust. Python draws input and the ASCII HUD. The `survival_meter` WorldPlay slice is unchanged.
