# Result

- Local versus: `examples/versus_local.py` (WASD/F/C and arrows/1/2, screen halves).
- Online: `examples/versus_online.py --match` and `server/netplay_relay.py`.
- Docs: `docs/netplay.md` (input sync, message JSON, Cloudflare Durable Objects, Render free web service). Nothing deployed.
- Screenshot: `scratch/versus_local.png` (also saved as a run artifact).

Merged `origin/master` (card, fps, sports, physics) before the PR.

`cargo test -p kagra-shared --lib --locked`: 497 passed.
`cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` passed for default, `--features render`, and `--features python`.
`pytest tests -m "not golden"` passed.
`python -m kagra.verify` on `examples/verify_scenarios/*.json` passed except `demo_live_smoke` (script missing, known failure). `physics_stack_smoke` and `versus_local_smoke` passed.
Wasm `cargo build --target wasm32-unknown-unknown --features wasm,render --locked` compiled, including `WasmLockstep`.
`python tools/gen_api_index.py --check` stayed clean. `netplay` is imported as `kagra.netplay`, not re-exported from `kagra/__init__.py`.
