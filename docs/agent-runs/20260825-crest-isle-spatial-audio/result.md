# Result — Crest Isle spatial audio

## Found

`play_se` was global rodio `Sink::set_volume`. No listener, no pan. rodio SpatialSink exists but is not HRTF and is not the mix we test. `kagra-shared` audio is truck levels, not this stack.

## Shipped

- `kagra.set_listener(x,y,z, fx,fy,fz)` — camera/player ears.
- `kagra.play_se(path, volume=1, *, x=, y=, z=)` — 2D if `x` is omitted; else inverse-distance + equal-power stereo pan.
- `kagra.play_loop` / `stop_loop` — looping world source; `set_listener` refreshes L/R.
- `kagra.spatial_mix` — same math, GPU-free.
- Crest Isle: looping sea drone at west water `(-28, 8)`; crest/coin pickups at the collectible; start/win 2D.
- `_pose` untouched (PR #87 mergeable). No Mixamo wav, no HRTF, no Rapier.

## Verify

- `python3 tools/gen_api_index.py --check` → OK (426 entries)
- `python3 -m pytest tests -m "not golden"` → **410 passed**, 10 deselected
- Focused: `tests/test_spatial.py` (closer louder, left/right pan, front equal-power, coincident, circling)
- `cargo test -p kagra-core --no-default-features --locked --lib` → **138 passed** (5 new `audio::tests`)
- GPU `open_world_smoke` / desktop Crest Isle **not** run here (no `kagra_core` wheel / no wgpu adapter). CI / Emma's Windows is the GPU+audio stand-in.

**GitHub CI: 17 checks passed** on `01384fe` (`cursor/crest-isle-spatial-audio-7ba1`).

PR: https://github.com/EMMA019/KAGRA/pull/88

## Try

```bash
python examples/vrm_open_world.py
```

Headphones. Spawn looks +Z; sea is on the left — a low drone should sit in the left speaker and get louder toward the west shore, quieter toward the peak. Circle the west beach: left/right swaps. Pick a coin/crest: chirp at that world point. SPACE start / result win stay screen-centered.

## Left out (later waves)

Multi-avatar FPS, Mixamo walk retarget, terrain/grass retune, Rapier, visual editor, CSM / SSAO / volumetrics / WebXR, HRTF, doppler, occlusion, `kagra-shared` mobile audio, Emma's local `cute_song_trial.wav`.
