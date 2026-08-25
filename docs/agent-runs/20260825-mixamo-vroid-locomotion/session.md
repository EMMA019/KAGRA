# Session — Mixamo FBX locomotion on VRoid without folded arms

## API search

- `avatar.set_locomotion(speed)` already on master (#87). Crest Isle uses it.
- `_bind_locomotion` still skipped Mixamo/BVH (T-pose rest → folded arms).
- `contracts` alias `"walk"` still lists `tests/fixtures/synthetic_walk.bvh`.
- FBX `to_clip()` maps Mixamo names → `J_Bip_*` then dumps **local** `inv(bind)*frame` deltas. Animator does `bind_q * delta`.
- VRMA already conjugates by **dest** world rest (`dest_delta_from_normalized`). Comment in `add_motion`: dest-only conjugate on Mixamo/BVH makes 骨格お化け. That was dest-only; Mixamo local deltas are not NormalizedLocalRotation until multiplied by **source** world rest.
- Bone names were never the failure (`kagra/fbx_player.py` `_BONE_MAP`).

## Root cause

Two stacked mismatches:

1. **Bone roll.** Mixamo T-pose is Y-along-bone, local X ≈ world +Z. VRoid `J_Bip_*` T-pose keeps the arm along ±X but a different local frame (documented: local X lifts forward, local Z hangs). Mixamo hang is ~90° around Mixamo local X (= world Z drop). Copied as VRoid local X it becomes a world-Y swing → arm ~90° **forward** (carry). Emma.vrm being a true T-pose does not help; rest *direction* matches, rest *axes* do not.
2. **Rest pose.** Mixamo bind is T-pose. Alicia is A-pose. Identity Mixamo delta must stay dest rest (A-pose), not snap to T. Dest-only conjugate without `W_src` also wrecks hips/sleeves (already tried).

`walk` alias resolving `synthetic_walk.bvh` overwrote built-in walk on machines without Mixamo (Relic Run #72).

## Approach

Smallest engine-native path:

- `kagra/retarget.py`: `N = W_src * delta * inv(W_src)` then `delta_dst = inv(W_dst) * N * W_dst`.
- Rust `fbx_loader` exports bind **world** rotations from `node_to_world`.
- BVH stores frame-0 world rest (same rest as its deltas).
- `VrmAvatar.add_motion` retargets FBX/BVH with dest `_bind_worlds`. VRMA path unchanged.
- `resolve_mixamo_locomotion()` / `avatar.bind_locomotion()` load Idle/Walk/Run FBX only. Never the `walk` alias.
- Crest Isle / Relic Run call `bind_locomotion()` then existing `set_locomotion`. Relic Run `_pose` switched off idle↔walk clip snap. `dance()` still full-body. Clap/banzai still ActionController overlay.

Fixture: `tests/fixtures/synthetic_mixamo_hang.json` (~1.3KB). No Emma.vrm, no Mixamo packs.

## Stumbles

- Snapshot git was behind `origin/master` (#87 loco blend, #88 spatial, #89 multi-avatar). Branched from fetched master.
- Dest-only conjugate is still banned for Mixamo; tests assert `add_motion` uses `src_worlds`.
- `import kagra.retarget` for the fixture writer pulls `__init__.py` / missing `kagra_core`. Tests use `load_kagra_submodule`.
- `resolve_mixamo_locomotion` docstring mentions `synthetic_walk.bvh` as the thing it never loads; a first test banned the string entirely.
- GitHub CI on PR #90: 17 checks green (wheels compile the new bind-worlds tuple).
