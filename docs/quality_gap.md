# Quality gap: phone web vs Unity URP and three.js

Target is a KAGRA game in a phone browser (wasm / WebGL2) that makes someone say 「おっ」— closer to a small Unity mobile game than to an ungraded real-time view. Android is parked. SSAO, DOF, area lights, and a WebGPU-only path stay off this target: they cost too much on a phone GPU, and the 80% bar already leaves GI bake / HDRP / VRM-on-Wasm out.

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
| Directional light + shadows | partial. One sun, one 2048 depth map, WebGL2 `sampler2DShadow`. Omit `shadow_softness` and the kernel stays 3×3. A wider tent is opt-in. No cascades, no contact-hardening. | URP main light + shadow cascades | P1 | M |
| IBL / ambient from an env map | partial. Procedural L1 spherical harmonics (`env_irradiance`). No cubemap, no PMREM, no HDR file. | URP environment lighting + reflection probe | P1 | L |
| Fog | have. Linear distance fog. Color follows `sky`. | URP linear / exponential fog | P2 | S to add an exponential mode |
| Screen shake | have, as of this wave. `kagra.Juice.shake` offsets camera 0 on a fresh dump. Deterministic, no GPU. | Cinemachine impulse (the 80% bar does not include Cinemachine) | P0 | done (S) |
| Hitstop | have, as of this wave. `Juice.hitstop` returns a shorter dt for the sim. Feel clocks keep running. | Time.timeScale punch, usually game code | P0 | done (S) |
| Squash / stretch | have, as of this wave. `Juice.squash` / `stretch` write `WorldWalker.scale`. The sim ignores it. | A tween on `transform.localScale` | P0 | done (S) |
| Tween / easing | have. `Juice.tween` plus `ease()` (`out_cubic`, `out_back`, `out_bounce`, `out_elastic`, …). `kagra.motion` still has the older smooth/linear helper. | DOTween / AnimationCurve | P0 | done (S) |
| Particles | partial, as of this wave. `kagra.Emitter` is a CPU list drawn as sphere props (cap 48). No GPU particles, no trails. | Particle System / VFX Graph | P0 for sparks, P2 for trails | S done; M for a ribbon trail |
| Trails | missing. | Trail Renderer | P2 | M |
| Sprite / skeletal animation | partial. CPU-skinned glTF/VRM, clip time, morph weight, locomotion blend, overlay bones. Sprites are quads. No sprite-sheet player on the 3D path. | Animator + SpriteRenderer | P1 | M |
| Art-direction presets | partial. `apply_post(world, "mobile"\|"legacy"\|"off")` covers grade. There is no toon/pastel/night bundle (lights + sky + LUT together). | Volume profile / URP renderer asset | P1 | S |

## three.js feature parity

Same renderer. "Priority" is what a phone game needs in order to feel finished, not a line-by-line three.js port.

| Feature | Status | Priority | Effort | What KAGRA does today |
|---|---|---|---|---|
| MeshStandard / MeshPhysical | partial | P1 | L | GGX + Smith + Fresnel on solid and on the metal material. Metallic and roughness are scalars on the instance. No clearcoat, sheen, transmission, or iridescence. |
| Albedo textures | have | — | — | PNG albedo on props (`texture`, `uv_scale`). glTF baseColor. Sampler stays clamp; tiling is a CPU repeat. |
| Normal maps | partial | P1 | M | MToon and glTF normals sample `normal_tex`. Solid/PBR props do not. Water uses a procedural normal. |
| Roughness / metalness maps | partial | P1 | M | Dump `roughness_map` / `metalness_map`: PNG or `preset:worn` / `split` / `brushed`. Linear ORM, G and B. glTF ORM textures are not read from the file yet. |
| Shadows, PCF | partial | P1 | M | One directional map. Omit `shadow_softness` and it stays 3×3 PCF. About 3 is a 5×5 tent. No cascades, no local-light shadows. |
| Directional light | have | — | — | Sun direction comes from light slot 0 (position or direction) plus ambient. |
| Point light | partial | P2 | S | Up to 4 slots. Inverse-square-ish attenuation. No cookie, no shadow. |
| Spot light | partial | P2 | S | Same 4 slots, `kind: "spot"`, a power cone. No shadow. |
| Hemisphere light | missing | P2 | S | The SH ambient is a stand-in, not a `HemisphereLight` with authorable sky/ground colors. |
| Area / rect light | missing | P3 | L | Parked. Too heavy for the phone target. |
| IBL | partial | P1 | L | L1 SH, studio sky vs warm ground, scaled by dump `ibl` (default 0.35). |
| PMREM / env maps | missing | P1 | L | No prefiltered cubemap, no equirect upload, no `scene.environment`. |
| Fog | have | — | — | Linear fog. Start 48, end 220 in `compile_scene`. Color is the sky clear. |
| Tone mapping | have | — | — | ACES in the composite. Exposure is a dump field (default 1). |
| Color space | partial | P2 | M | Frame is linear HDR. Composite writes the surface format (sRGB targets encode on store). There is no author-facing linear/sRGB switch and no display-P3. |
| EffectComposer | partial | P0 | — | A fixed stack, not a user pass list: HDR → bloom → exposure → ACES → LUT → vignette → FXAA → HUD. |
| Bloom pass | have | — | — | See the Unity row. Half resolution, one blur, intensity 0 skips the extract. |
| SSAO | missing | P3 | L | Outside the 80% bar. A phone web view should not pay for it. |
| DOF | missing | P3 | L | Same. A full-resolution blur fights the "cheap on mobile" rule. |
| Outline pass | partial | P2 | M | MToon draws an inverted-hull outline in the mesh pass. There is no screen-space outline. |
| FXAA | have | — | — | On the composited color. |
| SMAA | missing | P2 | M | FXAA is the phone choice. SMAA can wait. |
| Instancing | have | — | — | One instance vertex buffer per batch. No `baseInstance` (GLES does not have it). |
| LOD | partial | P2 | M | Vegetation past a radius becomes one billboard quad. Props and characters do not LOD. |
| Frustum culling | partial | P2 | M | `SceneBuilder` culls when a mesh has bounds. `WorldDoc::compile_scene` does not register bounds, so a dump draws every prop. |
| Skinned animation | partial | P1 | M | glTF / VRM skinning is CPU, then uploaded. Clips, locomotion blend, overlay bones. |
| Morph targets | partial | P2 | M | One dump weight (`morph`) drives blink / viseme on the walker. Not a general morph-target mixer. |
| AnimationMixer | partial | P2 | M | `mixer.rs` plus walker `anim` / `clip` / `anim_blend`. No three.js-style action crossfade graph for arbitrary clips. |
| glTF loader | have | — | — | glTF / GLB / VRM through `gltf_load`. Materials, skin, MToon extras. |
| Sprites | have | — | — | `sprite` / `quad` props, billboards for far vegetation. |
| Points | missing | P3 | M | Sparks are spheres, not `THREE.Points`. |
| Lines | missing | P3 | S | No line primitive. |
| Raycasting | partial | P2 | M | Genre interact uses reach, not a general raycaster. `kagra.annotate` records a preview click. |
| Cameras | partial | P1 | S | Dump camera (eye, target, fov). Chase cameras live in genre code. No OrbitControls. |
| Text | have | — | — | HUD `TextQuad` (PixelMplus, ab_glyph). Not a world-space `TextGeometry`. |
| Render targets | partial | P1 | M | HDR frame, shadow map, bloom halves, composite. Games cannot allocate their own target or bind a custom shader. |
| Custom shaders | missing | P2 | L | Materials are the built-in ids (solid, road, grass, sky, metal, toon, water). |
| WebGPU backend | partial | P3 | L | wgpu can target WebGPU on desktop. The wasm build keeps WebGL2 limits (no storage buffers, no base instance). A WebGPU-only path is parked. |

## This wave

Shipped on the wasm-safe path. Omitted fields keep the previous picture (`shadow_softness` 1, emissive 0, no maps):

- `emissive`, `emissive_color`, `emissive_map` (`preset:core` or a PNG). Added after lighting, before fog, so bloom extracts an authored source.
- `roughness_map` / `metalness_map`. Presets bake a 64² ORM. A PNG fills one channel and leaves the scalar for the other.
- `shadow_softness`. 1 is the old 3×3 kernel. Larger is a wider tent (still one 2048 map, still `sampler2DShadow`).
- Box Hop: the well is an authored glow, the crates use `preset:split`, the sun shadow radius is 4.5. `KAGRA_QUALITY=0` skips it.

## Previous wave

Shipped on the wasm-safe path, all of it toggleable:

- Post stack fields on the dump: `bloom`, `bloom_threshold`, `vignette`, `lut`, `lut_strength`, `fxaa`. Omitted fields keep the previous picture.
- `kagra.Juice`: shake, hitstop, squash/stretch, tween, `ease`.
- `kagra.Emitter`: CPU sparks as sphere props, hard cap, `enabled` flag.
- Box Hop turns the mobile preset on (`KAGRA_QUALITY=0` disables it) and plays squash, hitstop, shake, and sparks on jump / land / death.

`render_world_doc` applies the dump every frame, so a cached renderer cannot leak bloom into the next world. The desktop window example still calls `set_bloom` itself and does not go through that path.

## Next waves

1. **IBL that comes from an image, plus a hemisphere light.** Replace the fixed studio SH with a small prefiltered env (PMREM-style, still `texture_2d` faces or a low mip chain) and authorable sky/ground ambient. This is the "metal reads as metal outdoors" wave. glTF-authored ORM (not only dump maps) can land here too.
2. **Trails, prop LOD, frustum bounds on dumps, one art-direction bundle.** A ribbon of quads for jump trails. Register mesh bounds so `compile_scene` culls. A `pastel` / `night` preset that sets sky, lights, exposure, and LUT together.
3. **Parked.** SSAO, DOF, area lights, SMAA, custom shader graphs, WebGPU-only features, VRM-on-Wasm, Android. Contact-hardening shadows need a depth read, which this wave skipped so WebGL2 stays on the comparison sampler.
