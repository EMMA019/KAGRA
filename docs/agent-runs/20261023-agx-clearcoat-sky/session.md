# Session

Picture only. No new genre on WorldPlay.

- `tonemap` stays a bool so old dumps still parse. `tonemap_curve` is `aces` (omit), `agx`, or `neutral`. Unknown names stay on ACES.
- AgX uses Troy Sobotka's inset and outset. The curve between them is a log2 range and a smoothstep, shared by `color_space.rs`, `bloom.wgsl`, and `shader3d.wgsl`. Neutral is the Khronos PBR Neutral curve (start 0.76). The CPU clear on the direct unorm path uses the same switch.
- The composite reads the curve from `params.extra.z`. The direct fragment path reads it from `hemi_sky.w`. `tonemap: false` forces both back to no curve.
- Clearcoat is a second GGX lobe, F0 0.04, on `Instance` / `WorldProp.clearcoat`. `KHR_materials_clearcoat` lands on the mesh and is used when the dump coat is 0. It does not replace base roughness.
- `sky_atmosphere` replaces the studio gradient on the sky sphere with a short Rayleigh / Mie mix. The flag rides in `cascade.w`. Omitted dumps keep the gradient.
- Lavapipe: AgX and Neutral clears match the CPU bytes on sRGB and unorm targets. A coated sphere moves the frame (mean channel delta above 4, max 74). Looking at the horizon, the atmosphere is a pale band over a dark ground and the gradient is a flat blue.
- First coat shot was identical because IBL was 0 and the sun spec was in shadow. The coat env term needs `ibl`. The zenith sky shot only moved by ~2 levels; the horizon shot is the one that shows the band.
