# Result — Mixamo VRoid locomotion retarget

## Shipped

- `kagra/retarget.py` — rest-pose + bone-roll transfer (GPU-free).
- `kagra-core` FBX loader returns bind world quats; Python maps Mixamo names → `J_Bip_*`.
- `VrmAvatar.add_motion` retargets FBX/BVH onto dest bind worlds.
- `avatar.bind_locomotion()` + `contracts.resolve_mixamo_locomotion()`.
- Crest Isle / Relic Run load local Mixamo Idle/Walk/Run if present; else built-in. Both use `set_locomotion`.
- Fixture `tests/fixtures/synthetic_mixamo_hang.json`.
- Tests: `tests/test_retarget.py` (T-pose Mixamo on T-pose VRoid does not fold +Z; A-pose identity stays rest; A-pose hang does not fold).

## Verify

```
python3 tools/gen_api_index.py --check          # OK (427 entries)
python3 -m pytest tests -m "not golden"         # pass (this environment, no kagra_core wheel)
```

GPU `open_world_smoke` / Emma.vrm not run here (no extension). GitHub CI on PR #90: **17 checks green** (including wheels that compile the `load_fbx_anim` 4-tuple).

## How Emma tries it

1. Mixamo clips stay **out of git**. On the Windows box they already live in `D:\program\kagra\assets\mixamo\` (`Idle.fbx`, `walk.fbx`, `Running.fbx`, Catwalk / Tough Walk as walk fallbacks).
2. Optional: `set KAGRA_MIXAMO_DIR=D:\program\kagra\assets\mixamo`
3. `python examples/vrm_open_world.py` — console should print `locomotion idle/walk/run ← ...fbx`. Walk with WASD; arms hang/swing, not a carry pose. Clap a crest: upper overlay still works.
4. Same for `python examples/vrm_relic_run.py`.
5. Point Crest at Emma.vrm via the `Emma` alias (`assets/Emma.vrm`). Alicia (ensure_vrm) is the A-pose path.
6. `av.dance("ymca.fbx")` is unchanged (full body, not `set_locomotion`).

## Left out

- No Mixamo binaries, no Emma.vrm in the repo.
- No Rapier / visual editor / CSM / SSAO / WebXR / terrain retune.
- Finger bones still skipped on FBX (sleeve balloon). Feet still skipped.
- True weighted 3-clip bone blend of Mixamo idle *pose* vs hang is the existing mixer; we only replace the clip sources.
- GPU screenshot of Emma+Mixamo is for Emma's desktop / CI wheels.
