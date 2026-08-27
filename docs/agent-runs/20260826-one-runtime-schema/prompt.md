Repo EMMA019/KAGRA, start from current master (PR #99 World query/dump/load is already merged). Emma just asked “次は？”. Next mountain is ONE RUNTIME (not lights/joints/prefab, not VRM-on-Wasm, not adding z to 2D ECS).

This PR is the FIRST SLICE of that mountain. Do not try to finish the whole mountain.

## Goal of this slice
JSON is the source of truth. Grow kagra-shared’s existing Scene3D (find it; do not invent a second scene type) so it can ingest `world.dump()` JSON from `docs/schemas/world.json` (version 1: props, parent ids, heightfield name/samples/uv, lights, cameras, walkers, stable string ids). Python mutates the world; integer GPU mesh ids are not game objects (already true in kagra/world.py — keep it that way).

Done when:
- GPU-free tests: a World dump (Crest Isle-shaped and Orb Rush-shaped, can be synthetic) parses as Scene3D in Rust and roundtrips the stable ids + positions + parent + heightfield fn name / tile keys. No GPU.
- Desktop Python can export dump JSON that the shared crate accepts (pyo3 or a small CLI/test harness already in-repo — use what exists; do not add a new public game API).
- docs/schemas/world.json and Scene3D stay aligned. Short note in docs/ROADMAP.ja.md that this mountain started (schema), renderer switch is next. Log under docs/agent-runs/.

## Explicitly do NOT in this PR
- Do not merge wgpu 0.19 (kagra-core RendererV2) with wgpu 30. Do not delete RendererV2 yet. Do not retarget the desktop window to shared.
- Do not replace the (-12800,-12800) fake-headless hack yet unless it is a 20-line isolated change with tests; if it needs the renderer switch, leave it and say so in the PR.
- Do not port VRM skin to Wasm.
- Do not add Rapier, SSAO, editor, extra PNG goldens, M3 lights/joints/TRS hierarchy.
- Do not touch Crest Isle terrain UV/streaming.
- Do not merge the PR.

Investigate kagra-shared Scene3D, docs/schemas/world.json, kagra/world.py dump(), mobile JSON playback first. pytest tests -m “not golden” and kagra-shared tests must stay green. Open a PR.
