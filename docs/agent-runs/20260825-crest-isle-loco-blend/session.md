# Session — Crest Isle walk-speed blend + upper-body layers

## API search

- `docs/API_INDEX.md`: no `set_locomotion`. `play_upper` / `stop_upper` already exist on `VrmAvatar` but are not indexed (class methods). Built-in `PRESETS` already has `idle` / `walk` / `run` / `sneak` (`run` = `_make_walk(speed=2.4, …)`).
- `ActionController` overlays one-shot poses by writing into `_anim.current_rots` (the locomotion pose).
- Crest Isle `_pose` was `want = "walk" if vx²+vz² > 0.04 else "idle"` then `avatar.play(want)`. `_Animator.play` does have a 0.2s pose crossfade, but it **restarts the clip** and is still a binary threshold.
- Mixamo/BVH still skipped in `_bind_locomotion` (T-pose rest → folded arms). No new binaries.
- PR #86 (sleeve stiffness) **already merged** on master (`155a78e`). Touched `vrm_spring.rs` / `vrm.rs` / `kagra/vrm_spring.py`, **not** `vrm_avatar.py`. This PR does not touch SpringBone files. `VrmAvatar.update` spring block only gained two lines to merge `_overlay_rots`.

## Hypothesis

| Claim | Verdict |
|---|---|
| idle↔walk snaps on a velocity threshold | **Kept.** `> 0.04` then `play(want)`. |
| No crossfade at all | **Discarded as absolute.** `_Animator.play(fade=0.2)` already slerps the last pose, but gait phase resets and speed is ignored. |
| Whole skeleton is one clip so arms cannot stay independent | **Kept.** `play_upper` existed but Crest Isle called `stop_upper()` and never used it. ActionController wrote clap into locomotion `current_rots`, so the next walk keyframe interpolated **from clap arms**. |
| Need Mixamo / new clips | **Discarded.** Built-in `run` is enough. Mixamo still folds arms. |
| Analog stick already varies speed | **Discarded.** `walk_wish` normalized to unit then `* speed`, so half-stick was full speed. |

## Approach

Smallest engine-native path that already fits:

1. Pure functions + `_LocomotionMixer` in `kagra/vrm_avatar.py` (not kagra-core, not a new state machine — ROADMAP parks full anim graphs outside 80%).
2. `VrmAvatar.set_locomotion(speed)` samples idle/walk/run at a shared gait phase, slerps by speed weights, eases `speed` with `exp(-dt/τ)` so keyboard 0↔5.6 is not a hard cut.
3. Overlay-owned bones (upper clip + ActionController) stay in locomotion `current_rots` (gait keeps moving) but are **not sent**. Upper / ActionController write the engine.
4. ActionController writes `_overlay_rots` instead of mutating locomotion `current_rots`. Empty `{}` keyframes release to **live** locomotion, not the frozen snapshot.
5. `walk_wish` keeps analog magnitude (`min(1, mag) * speed`); diagonal keys still clamp.

Crest Isle `_pose` calls `set_locomotion(hypot(vx,vz), walk_speed=2.2, run_speed=Walk.speed)`. Still `stop_upper()` on enter; clap/banzai is the in-game upper overlay.

## Stumbles

- `play_upper("idle")` only names arm bones. Skipping *all* upper bones would freeze walk spine at bind; skipping **owned** bones lets walk keep counter-rotate on the spine while clap owns the arms.
- Loading `vrm_avatar` in tests cannot construct `VrmAvatar` (needs `load_vrm`). Mixer + helpers are tested on `_Animator(0)` with send wrapped in `try`.
- `python` is not on PATH here; `python3`. No `kagra_core` wheel, so GPU `open_world_smoke` was not run here. GitHub CI on PR #87 later went green (17 checks, including `golden` and Windows wheels).
