Add sleeve/cloth stiffness so Crest Isle's VRM (Alicia / Emma-style MToon) does not look like paper sleeves. This is the first item of a locked next-wave; do NOT also implement walk-speed blend, upper-body layers, spatial audio, or multi-avatar FPS in this PR.

Repo: https://github.com/EMMA019/KAGRA (Python + Rust game engine). Desktop demo: `python examples/vrm_open_world.py` (Crest Isle). VRM uses MToon; props are Kenney GLBs.

Goal / done when:
- Sleeves (and similar cloth) have noticeable stiffness / secondary motion that reads as fabric, not a skinned tube that either stays glued or flops like paper.
- Crest Isle still launches; existing play bugs stay fixed (Windows sticky walk, Mesh3D LRU 256, chase cam clamp, opaque title overlay_alpha=255, quiet gap 3 frames).
- Tests pass (python-unit / rust-test / whatever the repo already runs). Add focused tests for the cloth path; do not weaken existing ones.
- Open a PR against master. Emma merges herself; do not merge.

Constraints (do not reverse):
- No Unity/Tk visual editor. Agent eyes stay `kagra.annotate()` + `kagra.debug_trace()` if you need traces.
- Do not add Rapier to the pip wheel (size/undecided).
- Out of scope: 4-cascade CSM, SSAO, volumetrics, Web/XR, desktop-pet.
- Do not retune Crest Isle terrain/grass (Emma deferred the brown/green aerial grass-rock tiling). Do not swap Mixamo walk clips in this PR (those FBX exist locally but fold arms; that's wave 2).

Hypothesis (non-binding, verify and discard if wrong): there may be little or no real cloth sim today — sleeves are bone-skinned only, so they look stiff-wrong or paper-floppy. Prefer the smallest engine-native approach that gives sleeve stiffness on the existing VRM. Investigate existing cloth / spring / secondary-motion code first and reuse it if it exists.

Please investigate, pick the approach, implement, test, and open the PR. Report: what you found, what you shipped, how to try it, and any follow-ups you intentionally left out.
