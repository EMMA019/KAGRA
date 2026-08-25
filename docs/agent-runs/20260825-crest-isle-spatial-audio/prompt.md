Implement spatial audio for Crest Isle. This is next-wave item 3 of a locked sequence. Do NOT also implement multi-avatar FPS, Mixamo walk retarget, terrain/grass retune, Rapier, a visual editor, CSM, SSAO, volumetrics, or Web/XR.

Repo: https://github.com/EMMA019/KAGRA (Python + Rust VRM engine). Desktop demo: `python examples/vrm_open_world.py`.

Context:
- Next-wave 1 (sleeve SpringBone) is already on master (PR #86).
- Next-wave 2 (walk-speed blend + upper-body layers) is open as PR #87, branch `cursor/crest-isle-loco-blend-9026`. Start this work from **current master**, not from #87. Do not overlap locomotion/animator files (`set_locomotion`, `_overlay_rots`, ActionController, `walk_wish`) so #87 stays mergeable. If you must touch a shared file, keep the diff tiny and merge-friendly.
- Emma has `cute_song_trial.wav` on her local disk (`D:\program\kagra\assets\`) but it may not be in git. Do not commit large audio binaries that are not already in the repo. Use existing demo sounds / generate a tiny fixture wav in tests if needed.
- Recent play-bug fixes on master that must stay intact: Windows sticky walk (quiet gap 3), Mesh3D LRU 256, chase cam clamp, opaque title overlay_alpha=255, IBL/fog/MToon, sleeve Verlet.

Goal / done when:
- Crest Isle (and the engine) can play sounds in 3D: position relative to the listener (camera / player), with distance attenuation and stereo pan (or HRTF if something already exists — do not invent a heavy HRTF stack).
- Moving away from a sound source makes it quieter; circling changes left/right. At least one in-world source in Crest Isle (pickup/crest, water, or footsteps — pick the smallest that reads in play).
- Existing 2D/UI sounds must not break.
- Tests cover attenuation/pan math (no GPU required). Existing tests pass.
- Open a PR against master. Emma merges herself; do not merge.

Hypothesis (non-binding, verify and discard): audio today is probably global PlaySound with no listener transform. Prefer the smallest engine-native listener + world-source path in kagra-core. Investigate existing audio / cpal / rodio / wgpu-unrelated sound code first and reuse it.

Investigate, pick the approach, implement, test, open the PR. Report: what you found, what you shipped, how to try it, and what you left out.
