# Prompt

Close THIS slice:
- One directional shadow map (existing key light / sun dir) sampled with a small PCF kernel in shader3d.wgsl. Contact blob can stay. No cascade required unless one map is unreadable on Crest.
- Crest `python -m kagra.play_world` and water_plane_world.json and emma_walker_world.json should show a softer ground/capsule-or-human contact than blob-only.
- WebGL2: no storage buffers, no base_instance. Depth texture compare if wgpu+webgl2 allows; otherwise a manual PCF on a depth texture. Vertex/instance locations unchanged.
- Do not add Bevy. Do not rewrite vrm_open_world.py. No SSAO, GI, SSR, caustics, Rapier. world_play.rs tiny.
- Clippy CI Rust 1.98 -D warnings (as_chunks).

Junk never: assets/library/, models/, cache/, mp4, python/, huge bins.
Do NOT invent Python APIs.

Push origin master when green, no force. Log docs/agent-runs/20260828-m3-shadow-pcf/
