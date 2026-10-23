# Quality gap: phone web vs Unity URP and three.js

Target is a KAGRA game in a phone browser (wasm / WebGL2) that makes someone say 「おっ」— closer to a small Unity mobile game than to an ungraded real-time view. Android is parked. Area lights and a WebGPU-only path stay off this target: they cost too much on a phone GPU, and the 80% bar already leaves GI bake / HDRP / VRM-on-Wasm out. SSAO, TAA, DOF, and camera motion blur are opt-in passes. `KAGRA_QUALITY=0` / `off` / `low` forces them off so a phone keeps the historical frame.

Status words: **have** (usable on the shared wgpu 30 path), **partial** (a cheaper or narrower version), **missing**. Priority is for that 「おっ」 on a phone, not for matching a desktop engine. Effort is S (one subsystem), M (a pass plus a dump field and tests), L (a new asset pipeline or a port that touches the loader and the shader).

The shared picture path is `WorldDoc` → `compile_scene` → `Renderer` (`kagra-shared/src/render`). HDR lives in `Rgba16Float`. Bloom, ACES, FXAA, and the vignette curve live in `bloom.wgsl`. Lighting lives in `shader3d.wgsl`.

## Unity URP mobile

| Feature | KAGRA status | Unity equivalent | Priority | Effort |
|---|---|---|---|---|
| HDR color target | have. 3D draws into `Rgba16Float`. HUD is composited after the grade so UI stays in its own colors. | URP HDR + camera color target | P0 | done |
| Bloom | have. Threshold, soft knee, half-resolution Gaussian, intensity. Off when the dump omits `bloom` (intensity 0). | URP Bloom | P0 | done |
| ACES tonemap | have. Narkowicz fit in the composite, after bloom, before the LUT. Dump `tonemap` defaults on. | URP ACES | P0 | done |
| Color grading LUT | have, as of this wave. 16³ strip in a 256×16 `texture_2d` (no `texture_3d`). Presets `off` / `warm` / `punch` / `cool`. `lut_strength` mixes after ACES. | URP Color Lookup / LDR LUT | P0 | done (S) |
| Vignette | have, as of this wave. Strength is a uniform. Omit = 1, the curve that already shipped. 0 turns it off. | URP Vignette | P0 | done (S) |
| FXAA | have. On by default. Dump `fxaa: false` skips it. SMAA is not implemented. | URP FXAA (SMAA is the higher option) | P1 | S for a SMAA pass later |
| Directional light + shadows | partial. One sun, one 2048 depth map, WebGL2 `sampler2DShadow`. Omit `shadow_cascades` and it stays one full map (24 m, 3×3 PCF). 2 or 3 pack concentric rings (8 / 24 / 80 m) into tiles of that map. `shadow_pcss` is an opt-in blocker search. No local-light shadows. | URP main light + shadow cascades | P1 | M |
| IBL / ambient from an env map | partial. Authorable hemisphere (`hemi_sky` / `hemi_ground`) drives the L1 SH. `env_map` bakes a 64×64 prefiltered lat-long (4 roughness rows, `texture_2d`). No cubemap, no HDR file. Omit `env_map` and specular stays the SH. | URP environment lighting + reflection probe | P1 | done (M) |
| Fog | have. Linear distance fog. Color follows `sky`. | URP linear / exponential fog | P2 | S to add an exponential mode |
| Screen shake | have, as of this wave. `kagra.Juice.shake` offsets camera 0 on a fresh dump. Deterministic, no GPU. | Cinemachine impulse (the 80% bar does not include Cinemachine) | P0 | done (S) |
| Hitstop | have, as of this wave. `Juice.hitstop` returns a shorter dt for the sim. Feel clocks keep running. | Time.timeScale punch, usually game code | P0 | done (S) |
| Squash / stretch | have, as of this wave. `Juice.squash` / `stretch` write `WorldWalker.scale`. The sim ignores it. | A tween on `transform.localScale` | P0 | done (S) |
| Tween / easing | have. `Juice.tween` plus `ease()` (`out_cubic`, `out_back`, `out_bounce`, `out_elastic`, …). `kagra.motion` still has the older smooth/linear helper. | DOTween / AnimationCurve | P0 | done (S) |
| Particles | partial, as of this wave. `kagra.Emitter` is a CPU list drawn as sphere props (cap 48). No GPU particles, no trails. | Particle System / VFX Graph | P0 for sparks, P2 for trails | S done; M for a ribbon trail |
| Trails | have, as of this wave. `kagra.Trail` is a ribbon of quads along recent positions. CPU, capped, draw-only. | Trail Renderer | P2 | done (S) |
| Sprite / skeletal animation | partial. CPU-skinned glTF/VRM, clip time, morph weight, locomotion blend, overlay bones. Sprites are quads. No sprite-sheet player on the 3D path. | Animator + SpriteRenderer | P1 | M |
| Art-direction presets | have, as of this wave. `apply_post` is still grade-only. `apply_look(world, "pastel"\|"night")` sets sky, lights, exposure, hemisphere, and the LUT together. | Volume profile / URP renderer asset | P1 | done (S) |

## three.js feature parity

Same renderer. "Priority" is what a phone game needs in order to feel finished, not a line-by-line three.js port.

| Feature | Status | Priority | Effort | What KAGRA does today |
|---|---|---|---|---|
| MeshStandard / MeshPhysical | partial | P1 | L | GGX + Smith + Fresnel on solid and on the metal material. glTF factors, vertex color, and a tangent-space normal feed that lighting. No clearcoat, sheen, transmission, or iridescence. |
| Albedo textures | have | — | — | PNG albedo and emissive are sRGB. Color mips average in linear, then re-encode, down to 1×1. A glTF sampler sets wrap, filter, and anisotropy. Procedural meshes stay clamp. |
| Normal maps | have | — | — | Sampled as linear unorm (a flat 128 stays 0.5). When `mtoon4.w` is set, lighting uses a TBN from glTF `TANGENT` or generated tangents. Water keeps its procedural normal. |
| Roughness / metalness maps | partial | P1 | M | Dump maps, plus glTF `metallicRoughnessTexture`. A material with factors and no texture still emits a 1×1 ORM, so the file beats the dump scalars. Dump maps still replace the image. Toon primitives skip ORM. |
| Shadows, PCF | partial | P1 | M | One directional map. Omit `shadow_softness` and it stays 3×3 PCF. About 3 is a 5×5 tent. `shadow_cascades` 2 or 3 tile that map. `shadow_pcss` hardens the penumbra. No local-light shadows. |
| Directional light | have | — | — | Sun direction comes from light slot 0 (position or direction) plus ambient. |
| Point light | partial | P2 | S | Up to 4 slots. Inverse-square-ish attenuation. No cookie, no shadow. |
| Spot light | partial | P2 | S | Same 4 slots, `kind: "spot"`, a power cone. No shadow. |
| Hemisphere light | partial | P2 | S | Dump `hemi_sky` / `hemi_ground` feed the SH. Omit them and the studio pair stays. Not a separate light object. |
| Area / rect light | missing | P3 | L | Parked. Too heavy for the phone target. |
| IBL | partial | P1 | M | L1 SH from the hemisphere, scaled by dump `ibl` (default 0.35). Specular can sample `env_map`. |
| PMREM / env maps | partial | P1 | M | `preset:studio` or a PNG equirect, baked on the CPU to 64×64, 4 roughness rows, `texture_2d` + `textureSampleLevel`. Not a cubemap and not an HDR file. |
| Fog | have | — | — | Linear fog. Start 48, end 220 in `compile_scene`. Color is the sky clear. |
| Tone mapping | have | — | — | ACES in the composite. Exposure is a dump field (default 1). |
| Color space | have, as of this wave. Base color and emissive are `Rgba8UnormSrgb` (the sampler decodes on WebGPU and WebGL2). Normal and ORM are `Rgba8Unorm`. Author u8 colors decode before lighting, whatever the swapchain is. One OETF after ACES and the LUT. No display-P3. | P2 | done (M) |
| EffectComposer | partial | P0 | — | Switchable passes, not a user shader list. Order: optional half-res SSAO → DOF → camera motion → bloom extract → exposure → ACES → LUT → vignette → one sRGB OETF → TAA or FXAA → HUD. Omit the new fields and those passes stay off. |
| Bloom pass | have | — | — | See the Unity row. Half resolution, one blur, intensity 0 skips the extract. |
| SSAO | partial | P2 | done (M) | Half resolution, 8 taps, bilateral blur, linear eye depth. Dump `ssao` is the radius in pixels. Omit it and the pass stays off. `KAGRA_QUALITY=high` uses radius 8 when the field is absent. Quality 0 forces it off. |
| DOF | partial | P2 | done (M) | Disk gather, 14 px cap. Dump `dof_focus` (meters) and `dof_aperture`. Omit the aperture and the pass stays off. The high tier does not force it. |
| TAA | partial | P2 | done (M) | Halton jitter, history, Karis 3×3 clamp. Dump `taa: true` replaces FXAA. The first frame copies the current color. `KAGRA_QUALITY=high` turns it on when the field is absent. |
| Motion blur | partial | P2 | done (S) | Camera only, 8 taps. Dump `motion_blur` is a 0..1 shutter. A still camera is a copy. Object motion vectors are not stored. |
| Outline pass | partial | P2 | M | MToon draws an inverted-hull outline in the mesh pass. There is no screen-space outline. |
| FXAA | have | — | — | On the composited color. |
| SMAA | missing | P2 | M | FXAA is the phone choice. SMAA can wait. |
| Instancing | have | — | — | One instance vertex buffer per batch. No `baseInstance` (GLES does not have it). |
| LOD | partial | P2 | M | Vegetation past `lod_radius` is a billboard. `prop_lod` does the same for other props. Omit it and props stay full meshes. Characters do not LOD. |
| Frustum culling | partial | P2 | S | `cull: true` registers mesh bounds so `compile_scene` drops props outside the camera. Omit it and every prop still draws. |
| Skinned animation | partial | P1 | M | glTF / VRM skinning is CPU, then uploaded. Clips, locomotion blend, overlay bones. |
| Morph targets | partial | P2 | M | One dump weight (`morph`) drives blink / viseme on the walker. Not a general morph-target mixer. |
| AnimationMixer | partial | P2 | M | `mixer.rs` plus walker `anim` / `clip` / `anim_blend`. No three.js-style action crossfade graph for arbitrary clips. |
| glTF loader | have | — | — | glTF / GLB / VRM through `gltf_load`. Factors, emissiveStrength, alpha mode, double sided, vertex color, texture transform, samplers. Skin and MToon extras stay. |
| Sprites | have | — | — | `sprite` / `quad` props, billboards for far vegetation. |
| Points | missing | P3 | M | Sparks are spheres, not `THREE.Points`. |
| Lines | missing | P3 | S | No line primitive. |
| Raycasting | partial | P2 | M | Genre interact uses reach, not a general raycaster. `kagra.annotate` records a preview click. |
| Cameras | partial | P1 | S | Dump camera (eye, target, fov). Chase cameras live in genre code. No OrbitControls. |
| Text | have | — | — | HUD `TextQuad` (PixelMplus, ab_glyph). Not a world-space `TextGeometry`. |
| Render targets | partial | P1 | M | HDR frame, shadow map, an opt-in raw-depth copy for PCSS, bloom halves, composite. Games cannot allocate their own target or bind a custom shader. |
| Custom shaders | missing | P2 | L | Materials are the built-in ids (solid, road, grass, sky, metal, toon, water). |
| WebGPU backend | partial | P3 | L | wgpu can target WebGPU on desktop. The wasm build keeps WebGL2 limits (no storage buffers, no base instance). A WebGPU-only path is parked. |

## This wave

Shipped on the wasm-safe path. An 8-bit swapchain (phone WebGPU, `rgba8unorm`) always draws 3D into a separate `Rgba8Unorm` frame and runs the post graph. FXAA is the minimum. The frame is never `Rgba16Float` on that path.

- Bloom stays on. A threshold above 1 (Neon uses 1.45) folds into the top of the 0..1 luma range. A second render target, same format as the frame, carries emissive strength. Extract takes the max of the soft-knee luma weight and that mask, then a 5-level mip chain (Karis on the first hop, additive tent upsample). The HDR path is unchanged: half-res separable blur, intensity 0 still contributes nothing.
- SSAO on that path still uses the half-res pass. Unorm depth spans 96 m so the next gate is not sky. TAA, DOF, motion, and PCSS stay off. No optional WebGPU features.
- Two cascades reach 8 m and 80 m in the same 2048 map, so the gate ahead of the ship can land in the map. Count 1 stays 24 m. Count 3 stays 8 / 24 / 80.
- Neon Run no longer emits the stacked quad ribbon (`trail`). Speed lines and sparks stay. Rock emissive is 0.06 so the Kenney normals read.
- `pictureStatus()` and the `#grade` line on `play.html` print `{tier} {backend} fxaa= bloom= ssao= cN post|swap|hdr`, and the console adds `ask=` for the backend the page requested.

## Previous wave

Shipped on the wasm-safe path. A dump can name a picture tier. Omitted `quality` keeps the previous picture, including env `KAGRA_QUALITY=high` filling only omitted SSAO, TAA, cascades, and PCSS.

- `quality=low` / `0` / `off` forces SSAO, TAA, DOF, motion, extra cascades, and PCSS off, even when those fields are set. Authored bloom, tonemap, and FXAA stay.
- `quality=mobile` fills omitted fields: SSAO radius 4 (the pass is already half-res), TAA off so FXAA stays, 2 cascades in the same 2048 map, PCSS off, bloom 0.22. An explicit field wins.
- `quality=high` fills omitted fields: SSAO 8, TAA, 3 cascades, PCSS, bloom 0.35. Env high still does not turn bloom on by itself.
- `KAGRA_QUALITY=0` / `off` / `low` still forces the floor ahead of the dump tier.
- The web demo (`play.html`, Neon Run only) defaults to `mobile`. `?quality=low|mobile|high` overrides that scene. Other genres are unchanged. Neon Run's authored bloom, grade, and motion stay; the tier only fills the omitted passes.

## Previous wave

Shipped on the wasm-safe path. A glTF base color may be a pre-encoded KTX2. PNG, JPEG, and WebP stay the RGBA path. Omitted images keep the previous picture.

- `ktx2` 0.5 parses the container. Level 0 of a 2D ETC2, ASTC, or BC color block is stored on the mesh. Basis Universal (`VK_FORMAT_UNDEFINED`), supercompression, arrays, cubemaps, 3D images, BC4/BC5/BC6, and ASTC HDR are rejected. There is no transcoder.
- The device requests `TEXTURE_COMPRESSION_BC`, `TEXTURE_COMPRESSION_ETC2`, and `TEXTURE_COMPRESSION_ASTC` only when `adapter.features()` already contains that bit. A missing bit, or a non-identity `baseColorFactor`, keeps the RGBA albedo (a 1×1 factor bake when the block cannot be multiplied in place).
- The copy row pitch is padded to 256 bytes. Mips above 0 are not uploaded. Normal, ORM, and emissive images stay RGBA.
- Timestamp queries stay off on wasm. On other targets they are requested only when the adapter already lists both timestamp features, together with any compression bits above.

## Previous wave

Shipped on the wasm-safe path. The picture does not change. A headless dump can carry a `profile` object; `to_json` still omits it.

- Draws, triangles, and texture bytes (shadow map, env strip, authored images) come from `compile_scene`. No editor.
- `puffin` scopes the frame and the sim tick. The HTTP viewer is not linked. CPU milliseconds are filled when a desktop or Android frame actually draws. GPU milliseconds are filled when the device exposes timestamp queries. Wasm leaves both times empty and does not request timestamp queries.
- Warnings fire over a phone budget: 16.7 ms, 200 draws, 100k triangles, 48 MB of textures. A warning does not fail the render.
- The desktop window watches the dump JSON and material files (`notify`). A dump change reloads the document. A PNG change reuploads meshes. Wasm and Android do not link the watcher.

## Previous wave

Shipped on the wasm-safe path. Omitted fields keep the previous picture (one 2048 map, 3×3 or the wider tent, no blocker search):

- `shadow_cascades`. 1 is the historical full map, centered on the camera target, ortho half 24 m. 2 or 3 pack concentric rings (8 m, 24 m, 80 m) into tiles of that same map. No texture array, so the phone allocation stays one depth texture.
- `shadow_pcss`. A blocker search, then a 3×3 compare whose radius follows the average blocker depth. WebGL2 cannot read a comparison depth, so the sun pass also writes device z to an `Rgba16Float` target. The lighting shader samples that only while the flag is on. The raw target is nearest-sampled (`textureSampleLevel`), so `float32-filterable` stays off.
- `KAGRA_QUALITY=high` or `2` selects 3 cascades and PCSS when those fields are omitted. `0` / `off` / `low` forces the historical map. An explicit dump wins over the high tier.
- `shadow.z` / `shadow.w` stay camera near / far for the depth prepass. Cascade count and the PCSS flag live in `cascade.x` / `cascade.y`.

## Previous wave

Shipped on the wasm-safe path. Omitted fields keep the previous picture (no SSAO, no DOF, no motion blur, FXAA on, vignette inside the composite):

- Pass graph in `render/post.rs`: SSAO, DOF, camera motion, then the existing bloom grade, then TAA or FXAA, then vignette when TAA is on. Each pass is one fullscreen WGSL entry. No storage buffers.
- Dump fields `ssao`, `taa`, `dof_focus`, `dof_aperture`, `motion_blur`. A depth prepass writes linear eye meters into its own target. The color pipeline stays alpha-blended, so depth does not ride in the color alpha.
- `KAGRA_QUALITY=0` / `off` / `low` forces the new passes off. `high` / `2` enables SSAO radius 8 and TAA when those fields are omitted. DOF and motion stay opt-in.
- The direct unorm path (frame format equals the swapchain) still skips bloom and these passes. Browser WebGPU takes that path, so the phone frame does not sample `Rgba16Float` and does not pay for SSAO, TAA, DOF, or motion. The shadow compare sampler stays nearest (`depth32float-filterable` is not requested). `shadow.z` / `shadow.w` stay camera near / far for that depth prepass.

## Previous wave

Shipped on the wasm-safe path. A glTF material now reaches the shared PBR shader instead of stopping at a tinted texture.

- `baseColorFactor` is multiplied in linear light and stored back as sRGB (a 1×1 texel when the material has no texture). `COLOR_0` stays linear and multiplies the albedo.
- `metallicFactor` / `roughnessFactor` bake into the ORM. A file that writes a factor other than the pair (1, 1) and has no texture gets a 1×1, so that factor beats the dump scalars. Omitted factors stay on the dump. The written pair metal 1 / rough 1 also stays on the dump: Kenney stores that glTF default on every base-color material, and a 1×1 of it would replace the dump lighting. A real `metallicRoughnessTexture` is still sampled, with non-default factors baked into the texels.
- `emissiveFactor` bakes into the emissive map. `KHR_materials_emissive_strength` is the instance strength (it can be above 1). A dump `emissive` still wins when it is set.
- `alphaMode` is OPAQUE / MASK / BLEND. Procedural meshes keep the old `tex.a < 0.5` cutout. `doubleSided` draws with cull off, including the shadow pass.
- `KHR_texture_transform` on the base color, normal, ORM, or emissive texture rewrites `TEXCOORD_0` (`uv' = R * (uv * scale) + offset`).
- The base-color sampler sets wrap, mag/min, mip filter, and anisotropy 4 when every filter is linear. Mip chains are complete to 1×1, including sizes that are not powers of two.
- A normal map builds a TBN. Missing `TANGENT` is filled with Lengyel tangents. The shader does not flip the green channel.
- JPEG and WebP images decode through the `image` crate (`jpeg` and `webp` features, default features off) so rayon stays out of the wasm build. PNG stays on the `png` crate.

## Previous wave

Shipped on the wasm-safe path. The picture on an sRGB offscreen target stays the one that already decoded author colors. An unorm target (WebGPU canvases, and the direct 8-bit fallback) now decodes before lighting and encodes once at the end.

- Base color, emissive, and matcap stay `Rgba8UnormSrgb`. Normal maps and the default flat normal are `Rgba8Unorm`, same as ORM. Color mips average in linear light.
- Instance color, fog, and the 3D clear always enter the frame as linear. HUD bytes stay display-referred: linear on an sRGB target, the code itself on unorm.
- Bloom still thresholds the HDR frame before ACES. The LUT stays after ACES. The OETF is one step, after the vignette. FXAA reads that encoded image.
- When the frame format matches the swapchain, composite cannot sample it. `present_rgb` applies exposure, ACES, and the OETF in the fragment shader, and the clear is graded with the same function. Bloom and the LUT stay off on that fallback.

## Previous wave

Shipped on the wasm-safe path. Omitted fields keep the previous picture (every prop draws, full meshes, no trail, day grade):

- `cull: true` registers mesh bounds and drops props outside the camera.
- `prop_lod` is the distance where a non-vegetation prop becomes one billboard. Vegetation still uses `lod_radius`.
- glTF `metallicRoughnessTexture` becomes the linear ORM. Factors are baked into the texels. A dump `roughness_map` / `metalness_map` still replaces it. Toon primitives are left alone.
- `kagra.Trail` is a ribbon of emissive quads. `apply_look(world, "pastel"|"night")` sets sky, lights, exposure, hemisphere, and the LUT.
- Box Hop, with quality on, draws the hop trail and the night bundle, and turns cull and prop LOD on. `KAGRA_QUALITY=0` skips it.

## Previous wave

Shipped on the wasm-safe path. Omitted fields keep the previous picture (studio SH, no env strip):

- `hemi_sky` / `hemi_ground`. Linear colors for the L1 SH. Omit them and the hardcoded studio pair stays (`0.48, 0.56, 0.78` / `0.42, 0.30, 0.18`).
- `env_map`. `preset:studio` bakes a lat-long from those colors plus a sun disk. A PNG path is sampled as an equirect. Four roughness rows, 64×64, group 3, `textureSampleLevel`. Specular on metal, water, and solid gloss uses it. Omit it and specular returns the SH.
- Box Hop adds a cooler sky, a warmer ground, and `preset:studio` on top of the emissive / ORM / soft-shadow picture. `KAGRA_QUALITY=0` skips it.

## Previous wave

Shipped on the wasm-safe path. Omitted fields keep the previous picture (`shadow_softness` 1, emissive 0, no maps):

- `emissive`, `emissive_color`, `emissive_map` (`preset:core` or a PNG). Added after lighting, before fog, so bloom extracts an authored source.
- `roughness_map` / `metalness_map`. Presets bake a 64² ORM. A PNG fills one channel and leaves the scalar for the other.
- `shadow_softness`. 1 is the old 3×3 kernel. Larger is a wider tent (still one 2048 map, still `sampler2DShadow`).
- Box Hop: the well is an authored glow, the crates use `preset:split`, the sun shadow radius is 4.5. `KAGRA_QUALITY=0` skips it.

## Earlier wave

Shipped on the wasm-safe path, all of it toggleable:

- Post stack fields on the dump: `bloom`, `bloom_threshold`, `vignette`, `lut`, `lut_strength`, `fxaa`. Omitted fields keep the previous picture.
- `kagra.Juice`: shake, hitstop, squash/stretch, tween, `ease`.
- `kagra.Emitter`: CPU sparks as sphere props, hard cap, `enabled` flag.
- Box Hop turns the mobile preset on (`KAGRA_QUALITY=0` disables it) and plays squash, hitstop, shake, and sparks on jump / land / death.

`render_world_doc` applies the dump every frame, so a cached renderer cannot leak bloom into the next world. The desktop window example still calls `set_bloom` itself and does not go through that path.

## Next waves

1. **Parked.** Area lights, SMAA, custom shader graphs, WebGPU-only features, VRM-on-Wasm, Android. A real cubemap PMREM and HDR env files stay parked. Character LOD stays parked with that. Local-light shadows stay parked.
