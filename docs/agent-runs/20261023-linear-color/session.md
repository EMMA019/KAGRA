# Session

The sRGB offscreen path already decoded author u8 colors, because `to_linear` keyed off the swapchain format and offscreen is `Rgba8UnormSrgb`. WebGPU canvases are unorm, so the same function skipped the decode, then the composite applied `linear_to_srgb`. That is a missing decode plus an encode (mid gray 128 stored near 188).

Normal maps, including the 1×1 flat normal `(128, 128, 255)`, were created with `create_albedo_texture` (`Rgba8UnormSrgb`). Sampling 128 as sRGB yields ~0.216, so a "flat" normal tilts. ORM was already `Rgba8Unorm`.

Color mipmaps averaged the encoded bytes. A black/white pair averaged to 128 instead of the linear mean, which encodes near 188.

Fixes in `kagra-shared/src/render/color_space.rs`:

- Author colors for lighting, fog, and the 3D clear always decode.
- HUD colors stay display-referred (linear only when the target is sRGB).
- Normal maps use the linear texture path.
- Color mips average in linear and re-encode.
- Bloom intensity 0 does not add an undefined sample (`NaN * 0`).
- When the frame format matches an unorm swapchain, `present_rgb` applies exposure, ACES, and one OETF. The clear uses the same CPU grade.

`palette` 0.7 is a dev-dependency. Tests check the IEC curve against it. The runtime formula matches the WGSL so wasm and Android do not link the crate.
