# Result

- Rust: `kagra-shared/src/physics.rs`（`PhysicsWorld`）、`kagra-shared/src/py/physics.rs`
- Python: `kagra/rigid.py`、`examples/physics_minimal.py`
- Verify: `examples/verify_scenarios/physics_smoke.json`、`physics_stack_smoke` は PNG を必ず書く
- Schema: `docs/schemas/world.json` の `joints` と prop の `body` / `collider` / `sensor` / `mass`
- Screenshot: lavapipe headless `scratch/physics.png`（クリア後。岩が箱と宝箱を叩いている）

## Verify

`python -m kagra.verify examples/verify_scenarios/*.json`

- pass: physics_smoke, physics_stack_smoke, and every other scenario in that directory
- fail: demo_live_smoke only (`examples/demo_live_smoke.py` is missing)

`physics_smoke` stdout: won, bob y about 1.50, target knocked, ray `prop:target`, collisions 14.

## Tests

- `cargo test -p kagra-shared --lib --locked` — 474 passed after merging master
- physics feature unit tests — 17 passed
- `pytest tests -m "not golden"` — passed with the physics extension installed
- clippy `-D warnings` — default, render, physics, and python+physics
