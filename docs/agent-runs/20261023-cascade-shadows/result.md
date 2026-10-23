# Result

Opt-in cascades on the shared wgpu 30 path. Omit `shadow_cascades` and the picture stays one 2048 map. `shadow_pcss` stays off unless the dump or the high tier asks.

Lavapipe, 192×108, ambient 0.15, caster outside the 24 m ring:

- Count 1: no umbra on that floor.
- Count 3: 28 pixels darker by a median of 191.
- Count 3 + PCSS: 44 pixels darker, frame not blank.

`cargo test -p kagra-shared --lib`: 543 passed. With `--features render` on lavapipe: 559 passed. `cargo clippy -p kagra-shared --all-targets -- -D warnings` and the same with `--features render` are clean. `cargo fmt -p kagra-shared -- --check` is clean. `cargo check --target wasm32-unknown-unknown --features wasm,render` is clean.

Skipped: local-light shadows, a texture-array atlas, and frustum-slice practical splits (the rings are fixed 8 / 24 / 80 m). Instancing and frustum culling were already present.
