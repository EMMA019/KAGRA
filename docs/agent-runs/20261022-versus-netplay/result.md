# Result

- Local versus: `examples/versus_local.py` (WASD/F/C and arrows/1/2, screen halves).
- Online: `examples/versus_online.py --match` and `server/netplay_relay.py`.
- Docs: `docs/netplay.md` (input sync, message JSON, Cloudflare Durable Objects, Render free web service). Nothing deployed.
- Screenshot: `scratch/versus_local.png` (also saved as a run artifact).

`cargo test -p kagra-shared --lib --locked`: 475 passed.
`cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` and the same with `--features render` passed.
`pytest tests -m "not golden"` passed.
`python -m kagra.verify` on `examples/verify_scenarios/*.json` passed except `demo_live_smoke` (skipped, known failure) and `physics_stack_smoke` (missing `scratch/verify_physics.png`, same as master when the physics feature is not in the wheel).
Wasm `cargo build --target wasm32-unknown-unknown --features wasm,render` compiled, including `WasmLockstep`.
