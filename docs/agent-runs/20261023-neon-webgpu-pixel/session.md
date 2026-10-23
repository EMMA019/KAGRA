# Pixel WebGPU follow-up

## What the phone was actually running

`pages.yml` for the #147/#148 merges finished, then Pages Deploy
`39946193074` published the gh-pages tree at 11:27:53Z. Before that
deploy, `https://emma019.github.io/KAGRA/play.html` was the 11:15 tree
(no glass BOOST, no `showLinks`). After it, the live HTML has the 64px
glass button and hides `#links` unless `phase === "title"`. GitHub
Pages still sends `cache-control: max-age=600`, so a phone that loaded
the old document kept it for ten minutes.

The button can also sit under a WebGPU canvas layer. The canvas is now
`position: absolute; z-index: 0` and the button and links are
`z-index: 4`. Links start `hidden`, and `#links[hidden]` forces
`display: none`.

## WebGPU on this machine

Chrome headless with `--enable-unsafe-webgpu --enable-features=Vulkan
--use-angle=vulkan --use-vulkan=native` and lavapipe
(`VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json`) got an
adapter. `depth32float-filterable` is **false**. `float32-filterable`
is true. The Lumen page logged `backend webgpu` and
`surface Rgba8Unorm`, and `HDR bloom disabled, drawing an 8-bit frame`.
CDP confirmed `boost` class `on`, `border-radius: 18px`, and links
`hidden` during play.

A cyan `clear` on a WebGPU canvas screenshots as white, and
`drawImage` readback of that canvas is zeros. The same happens for the
game canvas, so this headless compositor cannot show the 3D WebGPU
frame. The picture below is lavapipe through the same shaders
(`offscreen` 390×844, 90 frames). A small WebGPU probe (mapAsync, not
the canvas) showed `writeTexture` with `bytesPerRow: 16` is accepted
and round-trips on this Chrome. The 256-byte pad stays because the
spec requires it and the wasm backend forwards the stride unchanged.

## Causes

1. **Smear.** The shadow map is `depth32float`. The compare sampler was
   linear. That filter needs `depth32float-filterable`, which wgpu 30
   cannot request and this adapter does not have. A bad linear compare
   comes back as black/white bands. Polygon offset `constant: 4` is
   about four units of 2^-23, so it does not hold the receiver off the
   caster on that format. The trail and the grid also depth-write while
   alpha-blending, and the ring (emissive 0.9) was in the shadow map.
   The sampler is nearest, PCF uses `textureSampleCompareLevel`, the
   receiver is pushed along its normal, the shadow-pass bias is 8192
   with a 0.008 clamp, emissive props skip the shadow pass, and glow
   quads get a small clip bias. Trail segments are staggered in Y.

2. **White Kenney meshes.** The GLBs have no vertex colors and no
   palette texture. Color is `baseColorFactor` per primitive (already
   atlased). Every material also sets `metallicFactor: 1` and
   `roughnessFactor: 1`. That 1×1 ORM replaced the dump's metal 0.06–0.22
   and roughness 0.55–0.72, so the shader lit a fully rough metal and
   the 8-bit grade crushed it to flat white. The 1/1 pair is the glTF
   default and is no longer baked. A real pair (0.25 / 0.40) still is.
   The ship instance tint is cool `(176, 220, 255)` and obstacles are
   `(255, 128, 168)` with a small pink emissive so the albedo stays
   visible.

3. **Bars.** The ceiling ribs (y = 2.25, full width) project to about
   28% of a portrait frame. Several of them read as a stack of cyan and
   pink bars. The night horizon color `(0.50, 0.12, 0.46)` sits on
   `dir.y == 0`, which is about 45% down this camera, and the 8-bit
   present turned that into a pink line. Ceiling and floor ribs are
   gone. Side posts stay thin and dim. The horizon is a low warm wash.
   Rails are 32 m and start in front of the camera so they cannot be
   clipped into a screen-filling cap. The long center grid line is
   shortened the same way.

## Checks

- `cargo test -p kagra-shared --lib` filters: speeder atlas, factor ORM,
  straight flight, and with `--features render` the row-pitch and shader
  tests.
- `cargo clippy -p kagra-shared --all-targets --locked -D warnings`
- `cargo clippy -p kagra-shared --all-targets --features render --locked -D warnings`
- `cargo fmt -p kagra-shared -- --check`
- `cargo run -p kagra-shared --features render --release --example offscreen -- 390 844 … neon 90`
