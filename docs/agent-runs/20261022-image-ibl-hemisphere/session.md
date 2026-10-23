# Session

Stacked on `cursor/emissive-pbr-shadows-4a92` (PR #137) so this diff is only the env.

## Decisions

- `hemi_sky` / `hemi_ground` feed the existing L1 SH. Omitted dumps use `(0.48, 0.56, 0.78)` and `(0.42, 0.30, 0.18)`, the constants the shader used to hardcode.
- `env_map` bakes a 64×64 lat-long on the CPU: 4 roughness rows. `preset:studio` is the hemisphere plus a sun disk. A PNG path is sampled as an equirect and box-blurred. No cubemap, no HDR, group 3 of 4 (WebGL2's max).
- Specular (metal, water, solid gloss) calls `env_specular`. The sample is `textureSampleLevel` and always runs; `hemi_ground.w` mixes back to the SH when `env_map` is omitted, so an old dump stays on the old picture.
- Diffuse stays `env_irradiance`. Toon is unchanged.

## Stumble

The first Box Hop pair moved the platforms (a row mean around 6) but the full-frame mean was 0.71 because the sky clear does not use the SH. Stronger hemisphere colors (`hemi_sky` 0.28, 0.55, 1.70 and `hemi_ground` 1.25, 0.48, 0.10) bring the full-frame mean to 0.95. The IBL compare guard is 0.75. The wave-2 guard stays 1.0.
