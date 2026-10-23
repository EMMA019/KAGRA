# Result

Lavapipe shots (8-bit phone path and the HDR mobile/low pair):

- `neon-phone-unorm.png` — `new_ldr_unorm`, quality mobile, bloom on. No staircase.
- `neon-phone-unorm-nobloom.png` — same frame with bloom intensity 0. The test requires the two frames to differ.
- `neon-quality-mobile.png` / `neon-quality-low.png` — HDR offscreen A/B. Mobile still moves the picture.

Rust: `neon_phone_unorm_runs_post_and_bloom`, `picture_line_names_the_phone_path`, `ldr_threshold_folds_hdr_headroom_into_the_top_of_the_range`, `straight_flight_scores_and_a_wall_ends_the_run` (no prop named `trail`), `shader3d_has_ibl_and_aces`, `unorm_ssao_darkens_contact_without_a_float_target`, `neon_mobile_tier_changes_the_lavapipe_picture`, `one_cascade_matches_the_historical_map`. Clippy clean with and without `render`. `cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --release --locked` finished. `pytest tests/test_neon_run.py tests/test_world_json.py` passed.

`python -m kagra.verify` was not run here: `kagra_shared` is not installed in this environment. The trail count in `examples/verify_scenarios/neon_run_smoke.json` is 0, matching the Rust assertion.
