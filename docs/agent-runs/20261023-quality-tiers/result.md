# Result

- Branch `cursor/quality-tiers-2c16`.
- Lavapipe offscreen, Neon Run, 480×270: `neon-quality-low.png` (authored bloom 0.14, tonemap, FXAA, 1 cascade, no SSAO) and `neon-quality-mobile.png` (same grade, SSAO radius 4, 2 cascades). The frames differ. Gameplay is unchanged.
- Chrome WebGPU on lavapipe sampled a rendered `rgba8unorm` target as `[51, 102, 153, 255]`. Device features were `core-features-and-limits` only. `float32-filterable` was advertised and not requested. `depth32float-filterable` was not a device feature.
- `FrameProfile` draws, triangles, and texture bytes are the same for Neon Run `low` and `mobile`, and draws plus texture bytes stay inside the phone budget. The tier does not add a shadow texture.
- `cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` and the same with `--features render` are clean. `cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --release --locked` finished. `pytest tests/test_world_json.py tests/test_neon_run.py` passed.

WebGPU on a phone still cannot HDR-bloom (the extract stays off because an 8-bit frame clamps at 1). `?quality=mobile` versus `low` on that path is half-res SSAO, FXAA, tonemap, and 2 cascades. `?backend=webgl2` is the path that keeps the float bloom.
