# Result — Multi-avatar GPU share

## Shipped

- Engine: same-path `load_vrm` Arc-shares mesh / texture / MToon. `kagra.vrm_gpu_stats()`.
- Example: `examples/vrm_multi_avatar.py` (`KAGRA_AVATARS=N`, default 4).
- Verify: `examples/verify_scenarios/multi_avatar_smoke.json`.
- Tests: `tests/test_vrm_share.py` (share invariant, offsets, Crest Isle still 1 `avatar()`).
- Crest Isle play unchanged (title / input / camera).

## How to try / measure

```bash
python examples/vrm_multi_avatar.py
KAGRA_AVATARS=8 python examples/vrm_multi_avatar.py
python -m kagra.verify examples/verify_scenarios/multi_avatar_smoke.json
# → scratch/multi_avatar_stats.json
```

Share must hold: `vertex_buffers * live == primitives` and `shared_instances == live - unique_paths`. Desktop FPS is the HUD. This cloud VM had no usable GPU/rustc pair (rustc 1.83 vs lock `hashbrown 0.17.1` edition2024), so FPS is not a desktop number here.

**GitHub CI: 17 checks passed** on `3d649a0` (`cursor/crest-isle-multi-avatar-06e8`), including `golden`, `rust-test`, `python-unit` 3.10/3.11/3.12, and Windows/Linux/macOS builds. PR: https://github.com/EMMA019/KAGRA/pull/89

Rebased onto master after #87 + #88. All three feature sets kept. Local `pytest tests -m "not golden"` pass.

## Left out

True GPU instancing of skinned meshes (different palettes). Sharing across different VRM files. Extra bodies inside Crest Isle. Mixamo / Rapier / spatial / terrain / CSM / SSAO / WebXR / HRTF / doppler.
