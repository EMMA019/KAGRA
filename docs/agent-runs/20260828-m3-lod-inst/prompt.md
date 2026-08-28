# Prompt

Work on Emma's Windows PC. Do NOT clone. Repo D:\program\kagra (EMMA019/KAGRA).

origin/master is now f474d6e (PCF directional shadow + water + IBL/ACES on kagra-shared wgpu 30). Emma: quality-only, no Bevy ECS, no second renderer. Next: LOD + GPU instancing so Crest trees/grass (and a lake of props) stay dense without unique draws.

If quota cuts off, log docs/agent-runs/ for resume 2026-08-31.

Close THIS slice:
- Distance LOD for repeated props (tree/vegetation): at least 2 levels (full mesh vs cheaper mesh or existing billboard/quad). Use existing heightfield lod_radius/lod_cells if already there; extend to vegetation props if they are unique draws today.
- GPU instancing: shared renderer already has instance buffers (locations 2..7, no base_instance). Batch same-mesh props into one draw. WebGL2-safe.
- Crest python -m kagra.play_world should still look dense (do not thin Kenney vegetation). Dump-visible lod or instance count in render_stats if that API exists on shared; otherwise a unit test that N trees = 1 batch.
- Do not add Bevy. Do not rewrite vrm_open_world.py. No SSAO/GI/SSR/caustics/Rapier. world_play.rs tiny. Relic Run UV defaults stay.
- Clippy CI Rust 1.98 -D warnings (as_chunks).

Junk never: assets/library/, models/, cache/, mp4, python/, huge bins.
Do NOT invent Python APIs.

Tests: cargo fmt -p kagra-shared; clippy -p kagra-shared --all-targets --offline --locked -- -D warnings (also --features render); cargo test -p kagra-shared --locked --offline --lib; pytest tests -m "not golden"; wasm32 --features wasm,render builds.
Push origin master when green, no force. Log docs/agent-runs/20260828-m3-lod-inst/

Report SHA, pushed, try command, tests, remaining.