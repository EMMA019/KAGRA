# Result

- `python -m kagra.render_world dump.json out.png` shells to shared wgpu 30 (`kagra-offscreen` / built `offscreen` example / `cargo run -p kagra-shared --features render --example offscreen -- W H out.png world dump.json`). Skip when none of those exist. Subprocess only — no wgpu 0.19 mix, no `RendererV2`, no `(-12800,-12800)`.
- `kagra.verify` `expect_offscreen` smokes PNG size / non-empty / IHDR dimensions (not golden). Orb Rush + Crest Isle (`open_world_smoke.json`) attach it. Helper missing → skip (not fail).
- GPU-free: fake helper writes a PNG and IHDR is checked. Real helper test skips here (example not built).
- Did not merge. Did not retarget the desktop window. Crest Isle UV/streaming untouched.

Verify (local, this VM):

```
pytest tests -m "not golden"     # 517 passed, 1 skipped, 10 deselected
python3 tools/gen_api_index.py --check   # OK, 409 entries
python3 kagra/render_world.py --help     # CLI (no kagra_core)
```

`cargo clippy -p kagra-shared --features render --locked` did not run here (crates.io `hashbrown 0.17.1` wants edition2024; this image is rustc 1.83). Example change is JSON-path only.
