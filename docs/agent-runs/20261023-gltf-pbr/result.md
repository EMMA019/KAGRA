CPU: `pbr_factors_alpha_uv_color_and_sampler` checks the linear 0.5 red byte, roughness 0.25, metallic 0, emissiveStrength 3, MASK 0.3, BLEND, OPAQUE, REPEAT, the UV transform, linear `COLOR_0`, and a generated tangent.

GPU: a factor-only red cube's center pixel is redder than the blue clear (`offscreen_gltf_base_color_factor_beats_the_clear`). DamagedHelmet and MetalRoughSpheres render when `KAGRA_GLTF_SAMPLE` is set. Those files are not committed.

Local checks: `cargo fmt -p kagra-shared -- --check`, clippy `-D warnings` with and without `--features render`, lib tests, `offscreen_render` (16), and `cargo check --target wasm32-unknown-unknown --features wasm,render`.

Merged with master after #152. Metal 1 / rough 1 (written or omitted) leaves `orm` empty so the dump scalars stay. Kenney `craft_speederC.glb` is covered by `kenney_speeder_keeps_base_color_and_skips_default_metal`. The shadow compare sampler stays nearest and the PCF calls are `textureSampleCompareLevel`. No `depth32float-filterable` request.

Chrome headless on lavapipe (`--enable-unsafe-webgpu --enable-features=Vulkan --use-angle=vulkan`) reports `depth32float-filterable` false. A nearest comparison sampler on `depth32float` binds. The lavapipe offscreen factor cube is `gltf-factor-cube.png` (center pixel redder than the blue clear).

Not in this change: clearcoat, transmission, KTX2, Draco, cascades, the post-process graph.
