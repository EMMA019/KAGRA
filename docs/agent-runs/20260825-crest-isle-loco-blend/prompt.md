Implement walk-speed blend + upper-body animation layers for Crest Isle. This is next-wave item 2 of a locked sequence. Do NOT also implement spatial audio or multi-avatar FPS in this PR. Do NOT retune terrain/grass. Do NOT add Rapier, a visual editor, CSM, SSAO, volumetrics, or Web/XR.

Repo: https://github.com/EMMA019/KAGRA (Python + Rust VRM engine). Desktop demo: `python examples/vrm_open_world.py`.

Context:
- Crest Isle currently uses built-in idle/walk only. Mixamo/BVH clips were previously skipped because they folded the arms forward; do not re-enable those clips unless you can prove the retarget no longer folds arms. Mixamo FBX exist on Emma's local disk (`D:\program\kagra\assets\`: Catwalk Walk, Female Tough Walk, walk.fbx, etc.) but are NOT necessarily in git — do not commit large binaries that are not already in the repo.
- Sleeve/cloth stiffness is a separate open PR #86 (branch `cursor/sleeve-cloth-stiffness-0753`, SpringBone Verlet + Alicia sleeve helpers). Start from current master. Avoid overlapping SpringBone/sleeve files if you can; if you must touch VrmAvatar update, keep the diff merge-friendly with #86.
- Recent play-bug fixes on master that must stay intact: Windows sticky walk (non-repeat KEYDOWN, quiet gap 3 frames), Mesh3D LRU 256, chase cam min/max clamp, opaque title overlay_alpha=255, IBL/fog/MToon backface.

Goal / done when:
- Locomotion speed blends between idle and walk (and a run/jog layer if a clip already exists — do not invent Mixamo). No hard cut when the player starts/stops or changes speed.
- Upper body can layer independently of legs (e.g. keep an idle/gesture or look/aim on the spine/arms while legs walk). The interesting hard part is two threads of motion not fighting.
- Crest Isle still launches; existing tests pass; add focused tests for the blend/layer path.
- Open a PR against master. Emma merges herself; do not merge.

Hypothesis (non-binding, verify and discard): the current animator likely snaps idle↔walk on a velocity threshold with no crossfade, and the whole skeleton plays one clip so arms cannot stay independent. Prefer the smallest engine-native blend-tree / mask / layer approach that already fits kagra-core. Investigate existing anim clip, bind pose, and mix code first.

Investigate, pick the approach, implement, test, open the PR. Report: what you found, what you shipped, how to try it, and what you left for later waves.
