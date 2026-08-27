Repo EMMA019/KAGRA, start from current master (PRs #100 WorldDoc and #101 verify-shared offscreen are merged).

Next slice of one-runtime: a REAL desktop window that draws WorldDoc through kagra-shared wgpu 30. Not fake headless (-12800). Not RendererV2.

## Do
1. A small Python entry (e.g. `python -m kagra.play_world` or `python examples/world_doc_window.py`) that: loads a world.dump JSON (Crest Isle fixture or live World.dump()), compiles WorldDoc → Scene3D, opens a normal winit/shared window, presents frames via the existing kagra-shared wgpu 30 Renderer (the same one collectathon/mobile uses). Capsules/boxes/plane are OK.
2. Keyboard close / Esc / a few seconds of camera. Optional WASD later, not required.
3. GPU-free tests stay. Window path may skip without a display. Do not use kagra-core RendererV2 or kagra-core window.rs for this path.
4. Short ROADMAP note: desktop VRM/Crest Isle still RendererV2; this window is the wgpu 30 desktop wedge. agent-runs log.

## Do NOT
- Delete RendererV2
- Retarget examples/vrm_open_world.py (Crest Isle) onto shared this PR — VRM skin is not on Wasm/shared yet
- Mix 0.19 and 30 in one process
- Rapier, SSAO, editor, extra PNG goldens, tile UV, M3 lights/joints
- Merge

Investigate kagra-shared render + existing window for collectathon/desktop examples first. pytest -m "not golden" green. Open a PR.
