# Result

## Commands

```text
cargo test -p kagra-shared --locked
# 127 passed; 0 failed

cargo test -p kagra-shared --features render --locked
# lib: 128 passed (includes world_doc_offscreen_crest_isle_is_not_flat)
# example offscreen: 10 passed

cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings
# ok (rustc 1.98.0)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 531 passed, 1 skipped, 10 deselected

python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json
# world: 5 assertions ok
# Python offscreen helper: skipped (no GPU adapter in this VM subprocess)
# GPU picture is covered by kagra-shared offscreen test (crest isle is not a flat plane)
```

## Notes

- Collectathon loop lives on `WorldPlay` / `WorldDoc`. Official play is `python -m kagra.play_world`.
- Picture is compile_scene + shader3d.wgsl (wgpu 30). No RendererV2, no VRM, no Rapier.
- Foliage is grass albedo + height biomes, not Unreal trees. Emma can try at 21:00 JST.
