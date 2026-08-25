# Result — sleeve / cloth stiffness

## Cause

Sleeves on Alicia are arm-skinned tubes (no SpringBone). Authored hair/skirt springs used a lerp Verlet that either glued (stiffness=1) or flopped (stiffness>1). Leaf ribbons were skipped.

## Fix

- `kagra-core/src/vrm_spring.rs` — UniVRM `stiffness * dt²`, virtual tails, sleeve follow/weight transfer, coverage so VRoid sleeves are not double-injected.
- `kagra-core/src/vrm.rs` — inject `_kagraSleeve*` helpers when the VRM has no sleeve chain; remap outer-tube JOINTS_0.
- `kagra/vrm_spring.py` — same Verlet / virtual tail / transfer for the Python fallback and GPU-free tests.
- `tests/test_vrm_spring_colliders.py` — `load_kagra_submodule`; leaf tail; dt² does not snap; sleeve names/weights.

Crest Isle play bugs (sticky walk, Mesh3D LRU, chase cam clamp, title `overlay_alpha=255`, quiet gap 3) not edited.

## Verify

- `python3 tools/gen_api_index.py --check` → OK (422 entries, no new public API)
- `python3 -m pytest tests -m "not golden"` → passed (includes 3 new cloth tests; collider tests still pass)
- `rustup run stable cargo test --no-default-features --locked` in `kagra-core` → **133 passed** (11 `vrm_spring` including leaf / dt² / sleeve transfer)
- GPU `open_world_smoke` / desktop Crest Isle **not** run here (no `kagra_core` wheel / no wgpu adapter). Engine path is load-time + `step_vrm_spring` already called from `VrmAvatar.update`.

## Try

```bash
python examples/vrm_open_world.py
```

Walk Alicia. Sleeves should lag the arm then settle; skirt/hair should sway instead of looking glued or paper. Console still prints `[VrmAvatar] SpringBone: N chains` (N is higher than before on Alicia because of four sleeve helpers + ribbon tails).

## Left out (intentional)

Walk-speed blend, upper-body layers, Mixamo walk swap, spatial audio, multi-avatar FPS, Rapier, CSM/SSAO/WebXR, terrain/grass retune, visual editor.
