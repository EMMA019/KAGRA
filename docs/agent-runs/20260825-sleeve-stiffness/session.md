# Session — sleeve / cloth stiffness

## What was there

KAGRA already has VRM SpringBone (Rust Verlet + colliders, Python fallback). Crest Isle loads Alicia via `ensure_vrm` / Emma alias and calls `avatar.update` which steps springs. There is no MagicaCloth / Rapier cloth.

Alicia Solid v0.51 (the sample VRM):

- Hair + skirt + ribbons are in `secondaryAnimation.boneGroups`. Skirt stiffness=1.0, bangs=2.0, side hair=1.6.
- **No sleeve bones.** Sailor sleeves live in `body_top.baked`, skinned only to `LeftArm` / `LeftForeArm` / right equivalents. Mesh nodes named `cloth` / `cloth1` are the skirt parts, not arms.
- Ribbon roots are **leaves** (no child). Parser required `idxs.len() >= 2`, so UniVRM's 7cm virtual tail was dropped.

Verlet was `(target - curr) * stiffness` with no `dt²`. At stiffness 1 that glues the tail every frame; at 2 it overshoots (paper). UniVRM / three-vrm use `parentRot * boneAxis * stiffness * dt * dt`.

Arm-vertex radii on Alicia (dominant weight > 0.35): upper arm 1.7–4.2cm, forearm 0.2–5.5cm. Inner ~2cm stays on the arm; outer sailor tube ~4cm can move to a helper.

## Approach (smallest engine-native)

Reuse SpringBone. Do not add a cloth solver or Rapier.

1. Verlet = UniVRM (`stiffness * dt²` along rest axis).
2. VRM 0.x leaf → virtual tail 7cm (ribbons).
3. If the file already has `*Sleeve*` / 袖 / sode bones, chain them.
4. Else inject four helpers parented to upper/lower arms, transfer outer-tube weights (`smoothstep(2.2cm, 3.8cm) * 0.82`). Stiffness 2.4, drag 0.45.

Crest Isle Python is unchanged (terrain / Mixamo / title / cam / rehold not touched).

## Stumble

`from kagra.vrm_spring import` needs `kagra_core`. CI `python-unit` has no wheel. Existing collider tests now load via `load_kagra_submodule` (same as the rest of `tests/`). Logic unchanged; three cloth tests added.
