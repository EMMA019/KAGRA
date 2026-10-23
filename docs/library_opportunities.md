# Library opportunities

Audit of what KAGRA already links, and high-impact libraries that are not in the tree yet. Base is `origin/master` at `f797826` (after #152). The workspace member is `kagra-shared` only. `old/kagra-core` is the archived wgpu 0.19 engine and is not the phone or wasm runtime.

Effort uses the same scale as `docs/quality_gap.md`: **S** is one subsystem, **M** is a pass plus a dump field and tests, **L** is a new asset pipeline or a change that touches the loader and the shader.

Wasm and Android mean the shared runtime (`kagra-shared`, `wasm32` CI, Android APK). Python game masters and `tools/` run on desktop and CI. They do not ship inside the wasm module or the phone APK.

## Top 5

| Rank | Library | Why first | Effort | Where it runs |
|---|---|---|---|---|
| 1 | [puffin](#puffin) | Frame scopes for the profiler step. Headless dumps still need the budget object. | S | wasm and Android if `puffin_http` stays off |
| 2 | [zune-jpeg + image-webp](#zune-jpeg-and-image-webp) | JPEG and WebP inside a GLB. JPEG is in via `image`'s `jpeg` feature. WebP is the follow-up. | S | wasm and Android (pure Rust, no rayon) |
| 3 | [ktx2](#ktx2-and-basis-universal) read-only | Phone texture memory. Pre-encoded ETC2 / ASTC / BC, feature-detected. | M loader, L pipeline | wasm and Android. No basis-universal encoder in the engine |
| 4 | [trimesh + pygltflib](#trimesh-and-pygltflib) | Agents can reject a bad GLB before a lavapipe shot. | S | desktop and CI only |
| 5 | [pydantic + hypothesis](#pydantic) | World JSON stops silently ignoring bad fields. Sim tests get property checks. | S validator, M hypothesis | desktop and CI only |

`meshopt` is the next phone-download win after KTX2. It is listed below and is not in this five.

## Already on the mainline

Direct dependencies of `kagra-shared` (`kagra-shared/Cargo.toml`, locked in `Cargo.lock`):

| Crate | Locked | Role |
|---|---|---|
| `ab_glyph` 0.2.32 | runtime | HUD glyphs. PixelMplus + Public Sans, rasterized to quads (`font.rs`). GPU-free, so wasm and Android match CI. |
| `glam` 0.33.3 | runtime | Matrices and vectors. `default-features = false`, `std` on. |
| `bytemuck` 1 | runtime | Pod vertex and uniform types. |
| `png` 0.17.16 | runtime | PNG decode and the screenshot encoder. |
| `image` 0.25 | runtime | JPEG (`zune-jpeg`) and WebP (`image-webp`). Default features off, so rayon stays out. |
| `serde` / `serde_json` 1 | runtime | World dumps and playtest JSON. |
| `flate2` 1.1.9 | runtime | Playtest zlib. `rust_backend` only, so libz stays out. |
| `once_cell` 1.19 | runtime | Embedded font cache. |
| `wgpu` 30.0.0 | `render` | Vulkan, Metal, GLES, WGSL. No DX12. Wasm target overrides to `webgpu` + `webgl`. |
| `raw-window-handle` 0.6, `pollster` 0.3 | `render` | Surface and blocking poll. |
| `pyo3` 0.21.2 | `python` | The extension module. |
| `rodio` 0.20.1 | `playback` | Desktop WAV. `default-features = false`, feature `wav`. Wasm stays silent. Android drains mixer cues. |
| `rapier3d` 0.35.3 | `physics` | Rigid bodies. `enhanced-determinism`. Wasm builds without parry SIMD. |
| `wasm-bindgen`, `js-sys`, `web-sys` | `wasm` | Browser entry. |
| `palette` 0.7.7 | **dev-dependency** | sRGB oracle in `color_space.rs` tests. The runtime curve and the Narkowicz ACES fit stay in `color_space.rs` and the WGSL shaders. `NOTICE` already records this. Do not link `palette` into wasm or Android. |
| `winit` 0.29.15 | dev, `not(wasm32)` | The `window` example only. |

Hand-rolled stand-ins that a new crate would replace:

- Image decode is `decode_raster` in `gltf_load.rs`. PNG uses the `png` crate. JPEG and WebP use `image` with the `jpeg` and `webp` features.
- Water and grass normals are a WGSL `hash21` / `fbm` in `shader3d.wgsl`. `island_height` / `overworld_height` are closed-form bumps, not a noise crate.
- Audio levels live in `mixer.rs` with no `rodio` types. `playback.rs` opens `rodio` only behind the `playback` feature.
- glTF mesh decode is `gltf_load.rs`. There is no `meshopt` decoder.
- Color grading is the shader ACES fit plus the IEC curve. `palette` is the test oracle.

Python (`pyproject.toml`): no runtime third-party packages. The wheel is maturin over `kagra-shared` with features `python` and `physics`. Optional extras, not imported by the game runtime:

| Extra | Packages |
|---|---|
| `facetrack` | `mediapipe`, `opencv-python` |
| `stream` | `pyvirtualcam` |
| `mic` | `sounddevice` |
| `dev` / `agent` | `pytest>=7`, `mcp>=1.1` |

`tools/requirements-osm.txt` is optional and only for `tools/osm_bake.py`: `osmnx`, `geopandas`, `shapely`, `networkx`. That script imports `networkx`. Game code under `kagra/` and `examples/` does not.

No import of `numpy`, `PIL`, `trimesh`, `pygltflib`, `pydantic`, `hypothesis`, `noise`, or `opensimplex` exists in the tree.

## Archived engine (do not copy forward)

`old/kagra-core/Cargo.toml` is a separate workspace (wgpu 0.19). It links `image` 0.24 (png, jpeg, bmp, tga, gif, tiff), `rodio` 0.17 (mp3, wav, vorbis), `nalgebra` 0.32, `ufbx` 0.9, `rayon` 1.10, `gilrs` 0.10, `winit` 0.29, `env_logger`. Rayon must stay out of the wasm runtime. Those crates are not a reason to add them to `kagra-shared`.

## Open PRs that change this list

`#149` added `image` 0.25 with `default-features = false` and feature `jpeg` (that feature is `zune-jpeg`). WebP is the same crate's `webp` feature, which is `image-webp`. Default features stay off so rayon does not return. PNG stays on the `png` crate.

## Candidates

### puffin

CPU scopes (frame, tick, upload, shadow pass) that a desktop viewer can read. The headless contract stays a dump object: cpu ms, gpu ms when `TIMESTAMP_QUERY` is real, draws, triangles, texture bytes, and budget warnings. `puffin` does not replace that JSON. It is the instrumentation inside the process.

`puffin` itself is pure Rust and builds for `wasm32` and Android. Leave `puffin_http` (the viewer socket server) off those targets, the same way `rodio` is a desktop feature. Do not request timestamp queries on wasm: that can fail device creation. GPU time stays `None` when the adapter lacks `TIMESTAMP_QUERY` and `TIMESTAMP_QUERY_INSIDE_ENCODERS`.

Effort **S**. Pin the version in `Cargo.lock`. Add a `NOTICE` line.

### zune-jpeg and image-webp

A GLB or a `Texture` alias can embed JPEG or WebP. `decode_raster` sends PNG to the `png` crate, JPEG to `image`/`zune-jpeg`, and a `RIFF`/`WEBP` header to `image`/`image-webp`. Both image features are pure Rust, with no rayon and no C toolchain, and they build for wasm and Android.

Effort **S**. The WebP path is a magic-byte branch plus a 1×1 lossless fixture in the glTF loader tests. A lavapipe shot is optional once the CPU decode asserts the albedo texels.

### ktx2 and basis-universal

Phone bandwidth. A read-only `ktx2` loader uploads pre-encoded **ETC2**, **ASTC**, or **BC** when `adapter.features()` contains the matching `TEXTURE_COMPRESSION_*` bit. Chrome on lavapipe advertises those three; a Pixel must be feature-detected, and a device without the format keeps the RGBA8 PNG path. Uncompressed fallback stays the default. `KAGRA_QUALITY=0` does not need a second code path if the dump simply omits compressed textures.

**basis-universal does not belong in the engine.** The encoder is a C++/transcode stack, it bloats the wasm build, and CI would have to keep it off `wasm32` and Android anyway. Artists or a desktop tool can encode ahead of time. The runtime only parses KTX2.

Effort **M** for the loader once a fixture exists, **L** for the bake pipeline that produces the files. The `ktx2` crate is pure Rust and is fine on wasm and Android. Do not enable a transcoder feature.

### trimesh and pygltflib

Desktop and CI checks an agent can run before `kagra.verify`: triangle count, bounds, missing images, metallic/roughness factors left at the glTF default, and a non-PNG texture that the runtime cannot decode. `pygltflib` reads the JSON and the chunks. `trimesh` checks the mesh (watertight, degenerate faces, vertex count).

Both are CPython packages. They do not import into the wasm module or the Android APK. Put them in a tools extra (`tools/requirements-assets.txt` or a `pyproject` optional extra), not in the wheel's required dependencies.

Effort **S** for a script under `tools/` plus a pytest that runs on one bundled `.glb`.

### pydantic

`WorldDoc::from_json` ignores unknown fields. A pydantic model generated from `docs/schemas/world.json` (or hand-written next to it) fails the agent's edit when a dump field is misspelled. That is a desktop and CI check. The Rust parser stays the runtime.

Effort **S** for the model and one test that a bad key fails. Do not add pydantic to the phone package.

### hypothesis

Property tests for the pure sim: walk wish direction matches `screen_right` / `screen_left`, jump lands, timers fire once, `from_json` round-trips the fields the schema owns. `pytest` is already the runner. Hypothesis stays in the `dev` extra.

Effort **M** to cover walk and one genre session without importing the Rust extension from `tests/` (load pure-logic modules through `tests/conftest.py`). Desktop and CI only.

### numpy, noise, and opensimplex

Procedural islands, scatter, and tile masks in a Python game master or a bake script. `numpy` is the array layer. `noise` and `opensimplex` are the 2D/3D samples. The GPU already has `fbm` for water and grass, so a noise crate is not required to keep that shader. CPU terrain today is `island_height` (a shelf, a hill, and a bay), which is enough for the bundled island.

The Rust `noise` crate is pure Rust, wasm-safe and Android-safe if its rayon feature stays off. Use it only if a *runtime* height field must move off the closed form. Python `noise` / `opensimplex` / `numpy` stay in tools. None of them belong in the wasm import graph.

Effort **S** for a bake script, **M** if `overworld_height` starts sampling a crate and the walk tests have to change.

### Pillow

Authoring and bake-time textures: LUT strips, albedo pads, the 256-byte row pitch the WebGPU upload needs, font atlases if HUD ever leaves the quad path. The runtime already encodes screenshots with the `png` crate. Pillow does not build into wasm or the APK.

Effort **S** as a `tools/` extra next to trimesh. Skip it while textures stay hand-exported PNGs.

### networkx

Already an optional OSM dependency. `tools/osm_bake.py` imports it to read the street graph. Game sessions, quest graphs, and room links do not import it.

A level graph (rooms, doors, locks) in a desktop tool can use the same package without `osmnx`. That stays out of the wheel. Wasm and Android never see it.

Effort **S** the day a tool needs a graph. No engine change until a dump field needs the result.

### lyon

Fill and stroke tessellation for HUD shapes, roads-as-ribbons, and SVG-like icons. Output is triangles the existing quad and mesh path can draw. Lyon is pure Rust, no GPU, and builds for wasm and Android.

The HUD is already quads from `ab_glyph` and colored rects. Lyon pays off when a game needs curves that are not a texture. Effort **M** (a mesh builder plus one offscreen shot). Leave it until a game asks for vector UI.

### cosmic-text and fontdue

`ab_glyph` already rasterizes the two embedded faces into coverage quads, with the same pixels on wasm, Android, and CI. `fontdue` is a smaller rasterizer with the same job and no shaping. Swapping it is effort **S** and does not raise the picture.

`cosmic-text` adds rustybuzz shaping, fallback, and bidi. That is the right crate if world-space paragraphs or mixed Japanese and Latin need real layout. It is pure Rust and can target wasm and Android, at a larger binary. Effort **M** to **L** because the HUD would stop being one quad per glyph. Keep `ab_glyph` until that layout is a product requirement.

### meshopt

Decode `EXT_meshopt_compression` and optionally quantize vertices so a phone download shrinks. The Rust `meshopt` crate decodes without the C++ encoder. Encoding stays a desktop asset step, same split as KTX2.

Wasm and Android can run the decoder. Do not compile the encoder into those targets. Effort **M** in `gltf_load.rs` plus a fixture that still matches the current vertex layout (WebGL2's 16-attribute ceiling stays). This is the best mesh-size crate once JPEG/WebP decode is in.

### palette

Already a dev-dependency (0.7.7). Tests compare the IEC sRGB transfer against it. The shader owns grading, so linking `palette` into the runtime would add wasm weight without a new look. A CPU LUT bake can call it from a native test or a desktop example. That is effort **S** and still does not belong on `wasm32` or Android.

### kira

Sequenced music, tweened mixer parameters, and sub-mixes. KAGRA already has that state machine in `mixer.rs` (BGM loop, one-shots, master/bgm/sfx, mute) and uses `rodio` only for desktop WAV playback. Android plays the cue list through the shell. Wasm is silent.

`kira` sits on cpal. It fits the `playback` feature on desktop and does not belong in the wasm or Android lib, same gate as `rodio`. Replacing `rodio` is effort **M** and does not change the phone. Prefer extending `mixer.rs` until a desktop game needs kira's clock.

## Decisions to keep

- Pin new crates in `Cargo.lock`. CI uses `--locked`.
- Every borrowed crate gets a `NOTICE` entry when it is actually added. This document adds none.
- No `rayon` on `wasm32`.
- No `basis-universal` encoder or transcoder in `kagra-shared`.
- Python asset and test packages stay in extras or `tools/`. The wheel's import graph stays the maturin extension.
- `palette` stays a dev-dependency.
- `networkx` stays an optional OSM tool dependency until a level-graph tool needs it.
