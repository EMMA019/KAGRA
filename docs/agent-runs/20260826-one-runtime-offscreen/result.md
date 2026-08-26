# Result

- `kagra_shared::render_world_doc` draws a compiled `WorldDoc` on the existing shared wgpu 30 offscreen `Renderer` and returns RGBA8 (PNG via the `offscreen` example `world` mode).
- GPU-free: dump JSON → WorldDoc roundtrip and WorldDoc → Scene3D still pass. Compiled batch mesh ids are in `compile_meshes()`.
- GPU: Crest Isle / Orb Rush fixtures through offscreen readback; skip without an adapter.
- Did not merge. Did not retarget the desktop window.

Verify (filled after local run):

```
cargo test -p kagra-shared --locked
cargo test -p kagra-shared --features render --locked
cargo clippy -p kagra-shared --all-targets --locked -- -D warnings
cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings
pytest tests -m "not golden"
python3 tools/gen_api_index.py --check
```
