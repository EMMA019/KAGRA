# Session

## What was already there

`Renderer` already draws 3D into an `Rgba16Float` target. `bloom.wgsl` extracts a half-res bloom, adds it, runs Narkowicz ACES, then a hardcoded vignette, then FXAA. `shader3d.wgsl` has one directional shadow map (3×3 PCF), four local point/spot slots, linear fog, and a procedural L1 SH for IBL. glTF skinning, morph weight, and instancing already exist. `kagra.motion` only eases smooth/linear. `kagra.effects` is the old 2D rect path and is not on `import kagra`.

`render_world_doc` never called `set_bloom`, so a Python dump could not turn bloom on. The vignette could not be turned off.

## Decisions

- New dump fields default to the old picture: bloom 0, vignette 1, LUT off, FXAA on. The window example still sets its own bloom and does not go through `render_world_doc`.
- LUT is a 16³ strip in `texture_2d` (256×16). No `texture_3d`, no storage buffer. Presets are `off`, `warm`, `punch`, `cool`.
- Composite bind group 2 carries the bloom texture and the LUT, so the pass stays at 3 bind groups (WebGL2's max is 4).
- Juice and particles are pure Python. Squash writes `WorldWalker.scale`, which the sim ignores. Sparks are sphere props appended to the draw dump only.
- Box Hop enables the `mobile` preset unless `KAGRA_QUALITY=0`.
- PBR maps, image IBL, and softer shadows are the next wave. SSAO, DOF, area lights, and Android stay parked. See `docs/quality_gap.md`.

## Stumbles

- `maturin develop` refused to run without a virtualenv, and this image has no `python3-venv`. `maturin build` plus `pip install` of the wheel worked.
- The `python` feature pulls rodio, which needs `libasound2-dev` and `patchelf` (`~/.local/bin` is not on `PATH` until added).
- `Tween.step` used `t >= duration`. Ten steps of `0.02` land at `1.9999999999999998`, so the tween never snapped. It now snaps when `t + 1e-4 >= duration`.
