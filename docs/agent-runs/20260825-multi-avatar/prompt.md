Implement multi-avatar FPS for Crest Isle / KAGRA. This is next-wave item 4 (last of the locked sequence). Do NOT also retune terrain/grass, Mixamo walk, Rapier, a visual editor, CSM, SSAO, volumetrics, Web/XR, HRTF, or doppler.

Repo: https://github.com/EMMA019/KAGRA (Python + Rust VRM engine). Desktop demo: `python examples/vrm_open_world.py`.

Context:
- Wave 1 sleeve SpringBone is on master (PR #86).
- Wave 2 walk-speed blend is OPEN as PR #87 (`cursor/crest-isle-loco-blend-9026`). Do not touch locomotion/animator files (`set_locomotion`, `_overlay_rots`, ActionController, `walk_wish`) so #87 stays mergeable.
- Wave 3 spatial audio is OPEN as PR #88 (`cursor/crest-isle-spatial-audio-7ba1`). Do not touch `set_listener` / `play_se` spatial args / `play_loop` / `spatial_mix` / `kagra/spatial.py` / `kagra-core/src/audio.rs` so #88 stays mergeable.
- Start from **current master**. Keep diffs merge-friendly with #87 and #88.
- Recent play-bug fixes that must stay: Windows sticky walk (quiet gap 3), Mesh3D LRU 256, chase cam clamp, opaque title overlay_alpha=255, IBL/fog/MToon, sleeve Verlet.

Goal / done when:
- The engine can draw several VRM avatars in one scene without falling over (shared mesh/texture/bind-group where possible; no N copies of the whole GPU pipeline per avatar).
- Crest Isle (or a focused example if Crest Isle must stay single-player) can spawn extra nearby avatars for a real FPS measurement. Document how to run it and what FPS you measured (or a SMOKE/headless metric if the VM has no GPU — in that case still ship the spawn path and a unit/bench that would catch N-avatar regressions).
- Target: make multi-avatar a first-class path, not a hidden crash. Prefer instancing / shared skeleton buffers / not re-uploading identical VRM textures per clone.
- Existing single-avatar Crest Isle play must not regress (title, input, camera).
- Tests pass. Open a PR against master. Emma merges herself; do not merge.

Hypothesis (non-binding, verify and discard): today each VrmAvatar likely owns a full GPU copy (textures, bind groups, joint buffers), so a second Alicia would blow the Mesh3D LRU or stall. Investigate existing avatar/GPU instancing and the Mesh3D LRU (max 256, never evict live diffuse) first.

Investigate, pick the approach, implement, test, open the PR. Report: what you found, what you shipped, how to try it / measure FPS, and what you left out.
