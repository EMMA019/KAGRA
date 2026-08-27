# Result

- `python -m kagra.play_world [dump.json]` opens a real desktop window and presents a compiled `WorldDoc` through kagra-shared wgpu 30 (`Renderer::new_for_window` + example `window`). Capsules / boxes / plane. Esc / Q closes. `--seconds` orbits then exits.
- GPU-free: helper discovery, skip without display, skip without helper, fake helper CLI, Crest Isle fixture default. Real window is not required in pytest. Helper with no presentable GPU skips (`Failed to create surface` / `No suitable graphics adapter`).
- Did not delete RendererV2. Did not retarget `examples/vrm_open_world.py`. Did not mix wgpu versions.

Verify (local, this VM, rustc 1.98 via `cargo +stable`; default image rustc 1.83 cannot parse wgpu 30's hashbrown 0.17.1):

```
python3 tools/gen_api_index.py --check              # OK, 409 entries
cargo +stable fmt -p kagra-shared -- --check
cargo +stable clippy -p kagra-shared --all-targets --locked -- -D warnings
cargo +stable clippy -p kagra-shared --all-targets --features render --locked -- -D warnings
cargo +stable test -p kagra-shared --locked         # 115 passed
cargo +stable test -p kagra-shared --features render --locked
  # 116 lib + 10 offscreen (adapter-less skips still report ok)
pytest tests -m "not golden"                        # passed; 1 skip (offscreen helper, no adapter)
python3 kagra/play_world.py dump.json --seconds 1 --no-cargo
  # skip: shared window helper has no display/surface on this VM
```
