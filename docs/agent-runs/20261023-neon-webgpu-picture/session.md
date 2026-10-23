# Phone 8-bit picture

This log is the first of the six. Juice, GPU particles, AgX, the character controller, and UI polish are separate branches so Neon Cat Run can take the shared modules without this picture fix.

- An 8-bit swapchain (`frame` format equals the target) used to draw 3D straight at the canvas unless SSAO was on, and `set_params` forced bloom intensity to 0 because an HDR threshold extracts nothing from a clamped frame. Any 3D world on that path now draws a separate `Rgba8Unorm` plus a same-format emissive mask and always runs grade, the mip bloom, and FXAA (or a blit).
- The mip chain follows three.js UnrealBloomPass (MIT, cited in NOTICE). The WGSL is original. A threshold above 1 folds into 0.5..0.92 (Neon 1.45 → 0.7675). Extract is max(soft-knee luma, emissive mask). HDR keeps the separable blur.
- TAA, DOF, motion, and PCSS stay off. No optional WebGPU features. No `Rgba16Float` canvas.
- SSAO unorm depth span is 96 m so the next gate is not treated as sky. Count-2 cascades are 8 m and 80 m in the same 2048 map. Count 1 stays 24 m. Count 3 stays 8 / 24 / 80.
- Neon Run no longer emits `trail` quads. Speed lines and sparks stay. Obstacle emissive is 0.06 so Kenney normals are not washed out.
- `pictureStatus()` prints `{tier} {backend} fxaa= bloom= ssao= cN post|swap|hdr`. `play.html` shows it in `#grade` and logs `ask=` for the requested backend.
- `Renderer::new_ldr_unorm` forces the phone path on lavapipe, which would otherwise pick an HDR float frame.
