# Quality tiers

Dump field `quality` resolves in `WorldDoc` accessors.

- Env `KAGRA_QUALITY=0` / `off` / `low` is the floor, ahead of the dump.
- Dump `low` forces SSAO, TAA, DOF, motion, extra cascades, and PCSS off. Bloom, tonemap, and FXAA stay authored.
- Dump `mobile` fills omitted fields: SSAO radius 4, TAA off, 2 cascades, PCSS off, bloom 0.22.
- Dump `high` fills omitted fields: SSAO 8, TAA, 3 cascades, PCSS, bloom 0.35.
- An explicit field wins. Env `high` with `quality` omitted does not turn bloom on.
- Neon Run copies the JSON string through `rebuild` and does not overwrite its authored grade.
- `play.html` applies `?quality=` only when the loaded world is Neon Run, and defaults that demo to `mobile`.

WebGPU cannot sample `Rgba16Float`. A Chrome lavapipe probe rendered `rgba8unorm` and sampled it: pixel `[51, 102, 153, 255]`, not white. The device features were only `core-features-and-limits` (`float32-filterable` was advertised and not requested; `depth32float-filterable` was absent). The direct path therefore draws the 3D pass into a separate unorm frame when SSAO is on, packs eye depth into 32 m, runs half-res SSAO and FXAA, and leaves TAA, DOF, motion, PCSS, and bloom extract off. HDR / lavapipe offscreen still stores meters and runs real bloom.

Phone budget: the second cascade reuses the 2048 map, and SSAO is a framebuffer, so `FrameProfile` draws, triangles, and texture bytes do not grow. FXAA instead of TAA, radius 4, and no PCSS are the cost choice. Lavapipe milliseconds are not a phone measurement.
