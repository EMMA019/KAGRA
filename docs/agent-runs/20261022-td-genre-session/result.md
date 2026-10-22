# Result

- Binding: `kagra-shared/src/py/td.rs` (`TdSession`)
- Session: `TdSession` in `kagra-shared/src/genre_session.rs`
- Example: `examples/td_minimal.py`
- Fixture: `kagra-shared/tests/fixtures/td_world.json`
- Smoke: `examples/verify_scenarios/td_smoke.json`
- Screenshot (lavapipe): `scratch/td_minimal.png` — wave 2/5, gold, lives 15, ARROW / CANNON / MAGE, gazebo, mill, bell tower, raccoon and wolf on the lane. Full run then cleared 5/5 with 0 leaks and retry restored gold 80 and lives 15.

`cargo test -p kagra-shared --lib --locked`: 426 passed (race and puzzle sessions from master included).
