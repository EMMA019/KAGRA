# Look presets (finishing pass)

HOP-BEAT's picture is a **look**: one preset id plus optional overrides, all in
the game JSON. Other games on `WorldDoc` can copy the table and the two
override blocks; the renderer side (bloom, vignette, grade, tone curve, fog,
tiers) already lives in `WorldDoc`, so nothing here is renderer code.

```json
"look": {
  "preset": "neon_night",
  "post": { "bloom": 0.16, "vignette": 0.6, "lut": "cool", "lut_strength": 0.2,
            "exposure": 0.9, "tonemap_curve": "", "ibl": 0.3, "bloom_threshold": 1.4 },
  "atmo": { "sky": [14, 10, 40], "fog_start": 34, "fog_end": 110, "stars": 40,
            "clouds": 4, "horizon": [150, 50, 170], "horizon_glow": 2.4, "twinkle": 0.6 }
},
"skin": "classic"
```

Every key in `post` / `atmo` is optional (the preset's value is used). Ranges are
checked in Rust and in `kagra/hop_beat.py`; unknown keys are refused.

| preset | mood | post | atmosphere |
|---|---|---|---|
| `neon_night` (default) | violet night, cyan/pink glow | bloom 0.16, vignette 0.6, `cool` grade | 40 stars, 4 far clouds, magenta horizon band, haze to 110 m |
| `dusk` | warm sunset city | bloom 0.2, vignette 0.55, `warm` grade | 16 stars, 6 clouds, orange horizon, haze to 105 m |
| `plain` | the old flat picture | everything off | nothing added |

`skin` (`classic`, `mint`, `gold`, `ember`, `ghost`) paints the hero; `classic`
keeps `look.hero`. The runtime keys `skin` and `look` in the world dump (or
`?skin=` / `?look=` on the preview page) and `HopBeat::set_skin / set_look`
switch them between runs, which is what an unlock system calls.

## What the renderer already did, and what the look adds

Already there (`WorldDoc`): threshold bloom (HDR chain, 8-bit mip chain on
phone WebGPU), FXAA, ACES / AgX / neutral tone curves, 3D grade LUTs, vignette,
exposure, image-based ambient, distance fog, soft shadows, prop culling by
distance, and the `quality` tiers (low / mobile / high). The look only fills
these in per preset and adds cheap props: stars (emissive spheres that twinkle,
a third at the low tier), flat far clouds (none at low), a two-band horizon
glow and a moon halo, plus window twinkle. Props sit at fixed offsets from the
hero so they never pass by. The haze is the fog the doc already had, now tied
to the sky colour.

Baked occlusion: `tools/gen_assets/hop_models.py` writes `COLOR_0` (8-bit
grey, from height and facing, no ray casting) into the bunny, towers and lamp
posts. The renderer multiplies it into the base colour; models without it are
unchanged.

## A/B on a phone

`?fx_post=0` turns off bloom, grade, vignette and the tone curve;
`?fx_atmo=0` turns off stars, clouds, horizon and the longer haze. Either or both.
`?fx_post=1&fx_atmo=1` is the default.

## Cost

`cargo run --release -p kagra-shared --features render --example hop_fx -- WORLD.json 390 844 150 OUT_DIR [low|mobile|high]`
renders the four combinations on the phone path (8-bit frame) and writes a PNG
of each. See the PR for the numbers: headless lavapipe (CPU), so they rank the
effects against each other. They are **not** phone frame times.
