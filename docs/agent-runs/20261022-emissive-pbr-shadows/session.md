# Session

Branched from `origin/master` after #136 (`a08efb2`).

## Decisions

- Omitted dump fields keep the old picture: emissive 0, no maps, `shadow_softness` 1 (the 3×3 PCF, same loop).
- Emissive is an instance scalar plus an optional color. It is added after lighting and before fog, so the bloom extract can see it. Toon keeps the MToon slots, so a toon mesh does not read the scalar.
- Roughness and metalness are one linear ORM (`texture_2d`, G and B). Presets `worn` / `split` / `brushed` are baked 64² images. A PNG fills one channel. glTF files are not parsed for ORM yet.
- Maps on a glTF ride that mesh (every crate.glb shares one ORM). Box Hop uses `preset:split` so the platforms read as half shiny metal and half rough. A primitive with no glTF gets its own slot, same as an albedo PNG.
- The soft shadow is a 5×5 tent on the existing comparison sampler. A depth read for contact-hardening was skipped so the wasm/WebGL2 path stays on `sampler2DShadow`.
- Image IBL and the hemisphere light stay the next PR. This wave is already a shader, a bind group, and a dump field.

## Stumbles

- `preset:core` on the well would multiply the glow by a UV-space falloff. KayKit UVs do not guarantee the bright texel, so Box Hop uses the scalar only. The map path is still there for `preset:core` and PNGs.
