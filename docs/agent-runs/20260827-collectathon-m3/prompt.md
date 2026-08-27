Repo EMMA019/KAGRA, current master (PRs #100–#102, #104, #105 merged). Do not merge. Open a PR Emma can try at 21:00 JST 2026-08-27.

Emma restated 80% (2026-08-27 ~19:03 JST):
- 80% bar unchanged: agent closes major genres without a human. Multi stays 100%.
- Picture is NOT a separate mountain. Raise shared wgpu 30 visuals each time a genre closes. HQ density/light/material (Unreal-like) is kept. VRM does not set that ceiling — VRM is a loader/costume. World picture is Prop / heightfield / glTF / light.
- M0–M2 done (~40%). Closed genres still 0. M3 = genre+picture (40%→80%). M4 = agent ships alone.
- Official play: `python -m kagra.play_world` (wgpu 30, WorldDoc, WASD, heightfield, glTF, live tick). `examples/vrm_open_world.py` RendererV2 is leftover VRM. New games do not ride V2. Verify is world assertions + shared offscreen. Human screenshot-fix does not close a genre.

## THIS PR: close collectathon (first M3 genre)
Kit + picture together. Short official APIs. Do not invent a new ECS.

Close: walk the island, pick up, count, finish. Title → play → result.
Picture (shared wgpu 30 only): heightfield looks like an island (not a flat plane); grass does not go bald; coins read as metal (PBR); light slots 1:1 (no slot leak); capsule is readable.
Verify: coins / on_ground / query; tile albedo_ok; offscreen is not empty. GPU-free tests must stay green.

Do NOT close with: VRM skin, GI, vehicles, SSAO, Rapier, editor, a second HQ renderer, adding picture on RendererV2, mixing wgpu 0.19 and 30 in one process, deleting RendererV2, going to action/RPG/next genre.

Collectathon already exists in kagra-shared (`collectathon.rs`, Crest dump, WorldPlay). Desktop official path is play_world / example window. Reuse that. Add title → play → result and pickup/count/end on WorldDoc/WorldPlay so `python -m kagra.play_world` is one complete loop. compile_scene heightfield+albedo must be the production picture (not a placeholder plane). Metal coins via existing PBR, not a new renderer.

Open the PR as soon as title/play/result + pick/count + island heightfield picture are real enough to try. Emma checks at 21:00 JST. Do not wait for Unreal-complete foliage.

pytest -m "not golden" green. kagra-shared tests green. wasm32 + features wasm,render still builds (`new_for_window` stays cfg not wasm32). Short ROADMAP + agent-runs log.
