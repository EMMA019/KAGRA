# Session

- PR #100 still open on `cursor/one-runtime-scene3d-dump-d6bc`. Stacked a new branch `cursor/verify-shared-offscreen-96b7` from that head. Did not push onto #100.
- No installed `kagra-offscreen` binary after #100. Documented the cargo example path. `python -m kagra.render_world dump.json out.png` searches `$KAGRA_OFFSCREEN` / PATH / `target/{release,debug}/examples/offscreen`, then cargo. Verify scenarios do not cargo-compile unless `KAGRA_OFFSCREEN_CARGO=1`.
- Extended `kagra-shared` `offscreen` example: `world` mode takes an optional dump JSON path (fixture remains the default).
- `kagra.verify` `expect_offscreen` smokes PNG exists / non-empty / IHDR size. Skip when helper or adapter is missing. Orb Rush and Crest Isle (`open_world_smoke.json`) attach it. RendererV2 / window.rs / `(-12800,-12800)` / Crest UV untouched. Subprocess only — no wgpu mix.
