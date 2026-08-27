# Result

- `kagra-shared/src/world_play.rs` — WASD / look tick mutates `WorldDoc` walker + camera
- `WorldDoc::compile_scene` / `compile_meshes` — heightfield grid + glTF slots
- `kagra-shared/examples/window.rs` — live wgpu 30 window (`python -m kagra.play_world`)
- Official Crest play: `python -m kagra.play_world` (capsule). Leftover VRM: `examples/vrm_open_world.py`

Verify (fill in after the run):

```
cargo test -p kagra-shared --locked
cargo test -p kagra-shared --features render --locked
cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings
pytest tests -m "not golden"
python3 tools/gen_api_index.py --check
```

GPU window present is Emma's desktop. This VM skips when there is no display / adapter.
