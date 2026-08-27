Repo EMMA019/KAGRA. PR #100 (WorldDoc + shared wgpu 30 offscreen) is open and CI green on branch cursor/one-runtime-scene3d-dump-d6bc — prefer starting FROM that branch (or rebase onto it after merge if master already has it). Emma reviews ~18:00 JST 2026-08-27.

Next SAFE slice (do NOT delete RendererV2 / do NOT retarget the desktop winit window yet):

Wire agent verify to the shared offscreen path without the (-12800,-12800) fake headless.

1. Add a thin Python entry that runs `cargo run -p kagra-shared --features render --example offscreen -- W H out.png world` (or a small `kagra-shared` binary) on a world.dump JSON file, writing RGBA/PNG. Prefer invoking an installed helper if one exists after #100; otherwise document the cargo path and add a `python -m kagra.render_world` (name freely) that shells to it when the shared binary is available, and skips cleanly when not.
2. Extend `kagra.verify` so a scenario can dump the World, assert `expect_world`, AND optionally attach/compare the shared offscreen PNG as smoke (file size / non-empty / dimensions) — still not golden pixel. Crest Isle or Orb Rush smoke JSON should exercise this when GPU+shared are present.
3. Tests: GPU-free path still passes without the binary. When the binary exists, one smoke writes a PNG and checks dimensions.

Do NOT:
- Delete or bypass RendererV2 for Crest Isle play
- Mix wgpu 0.19 and 30 in one process
- Touch Crest Isle UV/streaming
- Merge

If #100 is not yet on master, base your branch on `cursor/one-runtime-scene3d-dump-d6bc` or wait/merge master+100. Open a PR (or push to #100 if same agent branch and Emma prefers one PR — prefer a NEW PR stacked on #100 if #100 is still open so review stays clear). Short agent-runs log. pytest -m "not golden" green.
