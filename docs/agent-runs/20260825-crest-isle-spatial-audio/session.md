# Session — Crest Isle spatial audio

## API search

- `docs/API_INDEX.md`: `play_se(path, volume=1.0)`, `sound()`, `tone()`, `se()`, `play_bgm`. No listener, no world position, no pan.
- `kagra-core/src/audio.rs`: rodio 0.17 `Sink` + volume. BGM one sink, SE pool of 32. No SpatialSink, no listener.
- `kagra-shared/src/audio.rs`: truck engine/wind/brake **levels** for mobile, not rodio. Out of scope (not Web/XR).
- Crest Isle `_se` always `play_se(path, volume=)` (2D). Coin/star/start/win tones already exist via `kagra.tone`. No wav binaries in git.
- Camera3D has `position` / `target` / `up`. Walk.step already updates the chase cam before Crest Isle `update` continues.

## Hypothesis

| Claim | Verdict |
|---|---|
| Audio is global PlaySound with no listener | **Kept.** `play_se` sets sink volume only. |
| Reuse rodio SpatialSink / HRTF | **Discarded.** SpatialSink exists in rodio but uses `1/(1+d)` per ear, not a testable public mix. No HRTF crate. Equal-power pan is enough. |
| Need cute_song_trial.wav | **Discarded.** Procedural `tone(..., decay=False)` for a 1.8s sea drone. No binaries. |
| Footsteps as the in-world source | **Discarded.** Always at the player → no pan/distance. Sea loop at west water reads in play. Pickup SE at the collectible is extra. |

## Approach

1. Pure `spatial_mix` in `kagra/spatial.py` **and** `kagra-core/src/audio.rs` (same inverse-distance + equal-power pan; listener right = `up × forward` so look +Z, sea at -X is the left speaker).
2. Engine: `set_listener`, `play_se_at`, `play_loop_at`, `stop_loop`. Live L/R via `Arc<AtomicU32>` on a tiny `StereoPan` source. Existing `play_se` / `sound` / BGM unchanged.
3. Crest Isle: looping sea at `SEA_LOOP_XZ = (-28, 8)` (confirmed `biome_at` sea). `_sync_listener` after `_pose` (does **not** edit `_pose` / locomotion so #87 stays mergeable). Crest/coin `_se(..., pos=)`. Title/start/win stay 2D.
4. Tests: `tests/test_spatial.py` via `load_kagra_submodule`; rust `audio::tests`; Crest Isle source scan.

## Stumbles

- Local cargo 1.83 could not parse lockfile `hashbrown 0.17.1` (edition2024). `rustup update stable` → 1.98.
- First cargo test needed `libasound2-dev` + `python3.12-dev` (pyo3 link). CI already has these.
- `impl Source` decode helper compiled; no SpatialSink.
- GPU `open_world_smoke` not run (no `kagra_core` wheel / no adapter here).
