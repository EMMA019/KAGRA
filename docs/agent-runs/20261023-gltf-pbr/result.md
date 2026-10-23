CPU: `pbr_factors_alpha_uv_color_and_sampler` checks the linear 0.5 red byte, roughness 0.25, metallic 0, emissiveStrength 3, MASK 0.3, BLEND, OPAQUE, REPEAT, the UV transform, linear `COLOR_0`, and a generated tangent.

GPU: a factor-only red cube's center pixel is redder than the blue clear (`offscreen_gltf_base_color_factor_beats_the_clear`). DamagedHelmet and MetalRoughSpheres render when `KAGRA_GLTF_SAMPLE` is set. Those files are not committed.

Local checks: `cargo fmt -p kagra-shared -- --check`, clippy `-D warnings` with and without `--features render`, lib tests, `offscreen_render` (16), and `cargo check --target wasm32-unknown-unknown --features wasm,render`.

Not in this change: clearcoat, transmission, KTX2, Draco, cascades, the post-process graph.
