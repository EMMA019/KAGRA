# Result — Crest Isle locomotion blend + upper-body layers

## Found

Crest Isle snapped idle↔walk on `vx²+vz² > 0.04` via `avatar.play`. `_Animator` already had a 0.2s pose fade and a `play_upper` bone filter, but gait restarted, speed was ignored, analog stick was digital, and `ActionController` polluted locomotion `current_rots` so clap fought walk arms.

## Shipped

- `VrmAvatar.set_locomotion(speed, walk_speed=2.4, run_speed=5.0)` — idle/walk/built-in **run** speed blend. `play()` disables the mixer.
- Overlay mask: `play_upper` / ActionController own bones; legs keep walking in `current_rots`.
- ActionController `_overlay_rots` (does not mutate locomotion). Empty `{}` releases to live loco.
- `walk_wish` preserves analog magnitude.
- Crest Isle `_pose` uses `set_locomotion`. Mixamo/BVH still skipped. No SpringBone/sleeve files.

## Verify

- `python3 tools/gen_api_index.py --check` → OK (422 entries; agent note for `set_locomotion`)
- `python3 -m pytest tests -m "not golden"` → **416 passed**, 10 deselected
- Focused: `tests/test_vrm_locomotion.py` (weights continuous, idle→walk legs, overlay mask, speed ease, upper/action bone union)
- GPU `open_world_smoke` / desktop Crest Isle **not** run here (no `kagra_core` wheel / no wgpu adapter)
- GitHub CI on `c75006c` (PR #87): **17 checks green** — `python-unit` 3.10/3.11/3.12, `build` Ubuntu/Windows/macOS, `rust-test`, `golden`, `kagra-shared`, Android APK, iOS SwiftPM. Cursor Bugbot neutral.

## Try

```bash
python examples/vrm_open_world.py
```

WASD / left stick: idle eases into walk, full speed uses built-in run (PLAYER_SPEED=5.6). Partial stick is slower (analog). Pick a crest while moving: clap/banzai on the arms, legs keep walking. Mixamo is still not loaded (`[CrestIsle] walk ← built-in idle/walk (arm swing)`).

## Left out (later waves)

Spatial audio, multi-avatar FPS, Mixamo retarget that does not fold arms, sneak in the blend tree, persistent `play_upper("idle")` as default Crest Isle style (arm swing kept), Rapier, visual editor, CSM / SSAO / volumetrics / WebXR, terrain/grass retune.
