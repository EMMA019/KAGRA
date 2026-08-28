# Result

## Commands

```text
cargo fmt -p kagra-shared
# ok

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok (Rust 1.98.0)

cargo clippy -p kagra-shared --all-targets --offline --locked --features render -- -D warnings
# ok

cargo test -p kagra-shared --locked --offline --lib
# 337 passed; 0 failed (emma dump compiles Toon not capsule;
# WASD Mixamo-walks verts vs bind, idle clip 0)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 573 passed, 10 deselected
```

## Emma.vrm

On disk at `assets/Emma.vrm` (16,937,136 bytes). Gitignored (`*.vrm`); not committed.
Dump `gltf` is repo-relative `assets/Emma.vrm` (same as `resolve_alias("emma")`). No `D:\`.
Missing file (CI) falls back to bundled clip-less `tpose_humanoid.vrm`.

## Try (local PC)

```text
python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json
```

WASD Mixamo-walks Emma on shared wgpu 30. No RendererV2.
`python -m kagra.play_world` with no args stays Crest Isle capsule collectathon.

Did not rewrite `examples/vrm_open_world.py`. No Rapier / SSAO / second renderer / new ECS.
