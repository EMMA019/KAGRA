# Result

Lavapipe readback of an empty scene, author clear `(128, 64, 32)`, bloom off, FXAA off, vignette off:

| Target | Tonemap | Center pixel |
|---|---|---|
| `Rgba8UnormSrgb` | off | `(128, 64, 32, 255)` |
| `Rgba8Unorm` | off | `(128, 64, 32, 255)` |
| `Rgba8UnormSrgb` | ACES | `(154, 61, 19, 255)` |
| `Rgba8Unorm` | ACES | `(154, 61, 19, 255)` |

A missing decode would store red near 188. HUD gray ramp `0, 64, 128, 192, 255` round-trips on both targets (±1).

`cargo test -p kagra-shared --features render`: 553 lib tests, 16 offscreen, 3 screen-right, all passed. `cargo clippy -p kagra-shared --all-targets --features render -- -D warnings` clean. `cargo fmt -p kagra-shared -- --check` clean.

`docs/quality_gap.md` color-space row is **have**. SSAO, full glTF PBR, cascades, and the profiler are still later items in this wave.
