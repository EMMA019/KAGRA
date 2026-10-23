# Result

## Shipped

- `games/neon_run_minimal.json` — Lumen, the Pages default (`kagra-shared/www/play.html`). Corridor Haul is `corridor.html`.
- Sim is `NeonRun` (wasm, Android, Python). Not a WorldPlay genre.
- Picture uses the shared dump: `emissive`, `preset:core`, `hemi_sky` / `hemi_ground`, `env_map: preset:studio`, `shadow_softness`, `apply_look(world, "night")`, `cull`, `prop_lod` 80.
- The window draws `kagra.Trail`. Z motion is folded into Y so the ribbon emits. Wasm writes the same quads. The sphere wake is gone.

## Verify

`python3 -m kagra.verify examples/verify_scenarios/neon_run_smoke.json` (lavapipe): `ok: true`. World query found `ship-core`, `gate`, and `trail`. Offscreen region mean passed `mean_min [12, 6, 22]`. Headless score 481.

`cargo test -p kagra-shared --lib neon_run --locked` (2 passed). `cargo clippy -p kagra-shared --all-targets --locked -D warnings` and the same with `--features render`. `cargo fmt -p kagra-shared -- --check`. `pytest tests/test_neon_run.py tests/test_juice.py tests/test_particles.py tests/test_distribute.py` (26 passed). `python3 tools/gen_api_index.py --check`.

Screenshots (lavapipe, night look, Trail ribbon):

- `docs/screenshots/neon_run_title.png` — title, score 0
- `docs/screenshots/neon_run_play.png` — play, score 293, 15 trail quads
- `docs/screenshots/neon_run_result.png` — crash, score 300

## How to play

WASD or arrows steer. Shift or Space boosts. Drag aims the ship. P or Esc pauses. Enter, Space, or a click starts and retries.

- Browser: `./scripts/build_wasm.sh` then `python3 -m http.server -d kagra-shared/www 8000` → `http://localhost:8000/play.html`
- Desktop: `python3 examples/neon_run_minimal.py`
- Web package: `python3 scripts/package_web.py --game examples/neon_run_minimal.py --build-wasm`
- Windows: `python3 scripts/package_windows.py --game examples/neon_run_minimal.py`
- Android: `python3 scripts/package_android.py --game examples/neon_run_minimal.py` (stick steers, jump boosts)

Best score is `localStorage` key `lumen-best` in the browser, and the Rust save dir on desktop when persist is enabled.
