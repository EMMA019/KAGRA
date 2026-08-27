Repo EMMA019/KAGRA, current master (PRs #100 WorldDoc, #101 verify offscreen, #102 play_world window are merged).

Emma (2026-08-27) remaining M2 before the kit. Do these IN ORDER on one branch. Open a PR when the first walkable slice is real. Do not merge.

## Must land (this mountain)
1. **Window input.** Walk WASD + look (mouse or arrow) flow into WorldDoc (walker position/yaw, camera). python -m kagra.play_world window.
2. **compile_scene heightfield + glTF.** Primitives-only cannot carry Crest. Heightfield from dump samples (or named fn if you can share the Python island fn as data). glTF props via existing kagra-shared gltf_load.rs. Capsule player is OK (do NOT port VRM to shared).
3. **Live per-frame.** Not a static dump. Prefer shared advancing WorldDoc each tick (WASD → wish → move on heightfield). Python ticking and streaming JSON each frame is acceptable if shared physics is not ready, but do not re-enter RendererV2.
4. **Retarget Crest play to play_world** when 1–3 work: examples/vrm_open_world.py can keep V2 for old VRM path, but NEW play (docs/README/play_world) is the wgpu 30 window walking a Crest dump. Collectathon-on-official-path is the M2 exit.
5. New games must not start on RendererV2. Fake-headless (-12800) stays only for leftover V2 smokes; new verify stays on shared offscreen.

## Do NOT
- Port VRM skin to shared/Wasm then load Crest
- Add M3 kit (hit/death, RPG talk, vehicles) on RendererV2
- Mix wgpu 0.19 and 30 in one process
- Delete RendererV2 this PR (old VRM demos may keep it)
- Rapier, SSAO, editor, extra PNG goldens, tile UV retune

GPU-free tests for: WASD updates walker in WorldDoc; compile_scene emits heightfield batches + a glTF/box prop; tick moves the walker. pytest -m "not golden" green. kagra-shared tests green. Short ROADMAP + agent-runs log.

Investigate kagra-shared example window, world_doc.rs compile_scene, gltf_load.rs, collectathon WalkInput, Python Walk/CharacterController contract (document, don't copy Rapier). Prefer shared-side tick.
