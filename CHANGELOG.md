# Changelog

## Unreleased

- ``Renderer::new_for_window`` requires ``Send + Sync`` so wgpu 30
  ``new_with_display_handle`` type-checks on wasm32 (CI ``kagra-shared``
  job after #102). Desktop ``Arc<Window>`` already is. No API change.

- One runtime (M2 window wedge): ``python -m kagra.play_world dump.json`` (or ``python examples/world_doc_window.py``) opens a **real** desktop window and presents a compiled ``WorldDoc`` through the existing kagra-shared wgpu 30 ``Renderer`` (same as collectathon / mobile / ``render_world``). Esc / close / optional ``--seconds`` camera orbit. Subprocess so wgpu 0.19 and 30 never mix. Does not use kagra-core ``RendererV2`` or ``window.rs`` / ``(-12800,-12800)``. Desktop VRM / Crest Isle stay on RendererV2. Missing helper, no display, or no adapter skips. No WASD, no Rapier, no extra PNG goldens.

- One runtime (M2 verify): ``python -m kagra.render_world dump.json out.png`` shells to the shared wgpu 30 offscreen helper (installed ``kagra-offscreen``, already-built ``target/*/examples/offscreen``, or ``cargo run -p kagra-shared --features render --example offscreen -- W H out.png world dump.json``). ``kagra.verify`` ``expect_offscreen`` smokes PNG file size / non-empty / IHDR dimensions (not golden pixels). Missing helper or no GPU adapter skips. Does not delete ``RendererV2``, does not retarget the desktop window, does not mix wgpu 0.19 and 30, does not use ``(-12800,-12800)``. Crest Isle UV/streaming untouched.

- One runtime (M2): persistent ``WorldDoc`` in ``kagra-shared`` matches ``docs/schemas/world.json`` version 1 (stable string ids, props, parent, heightfield, lights, cameras, walkers). ``from_json`` / ``to_json`` roundtrip dump JSON. ``compile_scene`` builds a one-frame ``Scene3D`` (camera + batches; capsules / box / sphere / plane). ``render_world_doc`` (feature ``render``) uploads ``compile_meshes``, draws those batches on the existing shared wgpu 30 offscreen ``Renderer``, and reads RGBA8. No kagra-core window, no ``RendererV2``, no wgpu 0.19 mix, no ``(-12800,-12800)``. GPU tests skip without an adapter. Python dump stays the exporter; no new public game API. Desktop window retarget is next. Crest Isle UV stream is not this slice.

- Crest Isle hillside black tile + gold spec: one streamed 16 m chunk was a pitch-black quad with a GGX highlight (geometry still there). Not the #94 dirt-rim stamp, not the #95 barcode / failed-upload hole, not the #96 JPEG biome window, not the #97 dead-albedo leftover. Terrain upload now pins Lambert (``metallic=0``, ``roughness=1``) so coin PBR cannot stick to a heightfield tile; ``World3D.draw`` draws live ``_tile_meshes``; mesh_mat slots init Lambert and pack 1:1 with the draw list (a missing mesh no longer shifts later tiles onto leftover gold). ``uv_rect`` / dead-albedo unload stay. Relic Run UV / PBR defaults unchanged. No Rapier / SSAO / brighter ``GRASS_TINT``.

- World as data (15% → toward 35%): ``World`` is ``World3D``. Stable string ids. ``world.query(type=, name=, aabb=)`` returns position / name / type / id without a screenshot; terrain tiles expose ``loaded`` / ``albedo_ok`` so Crest Isle はげ is detectable without a PNG. ``world.dump()`` / ``world.load()`` JSON (schema ``docs/schemas/world.json``) for Prop + parent id, heightfield name/samples, lights, camera, walkers. ``kagra.verify`` ``expect_world`` asserts player.on_ground / coins / query counts. Entity / tilemap / Tk are off ``import kagra`` (files stay; archive 2D imports from ``kagra.entity`` / ``kagra.tilemap``). Roadmap 80% redefined (100% = screenless indie ship; now ~15%; old 63% archived). Drawing / Rapier / SSAO / terrain stream (PR #97) / Relic Run UV defaults untouched.

- Crest Isle remaining ハゲ (GGX-only 16 m TILE): ``World3D._upload_tile`` no longer swallows TypeError / bind failure into a leftover mesh. Failed / 1×1 / missing albedo unloads the GPU id and retries (does not stick ``lod_ok``). LOD upgrade keeps the previous good mesh. Mesh3D bind groups are created at ``upload_mesh_3d`` and culled stream tiles stay pinned (PR #92 family). ``TERRAIN_UV_RECT`` / stream retry / prefetch unchanged. No Rapier / SSAO / Repeat / per-tile 0..1.

- Crest Isle remaining meadow ハゲ: ``aerial_grass_rock_diff_1k.jpg`` is mixed moss + brown rock even inside pad 0.28. Period 48 made each 16 m TILE a different 2D biome window (green tile glued to yellowish dirt). Crest UVs now ping-pong into a compact meadow-green ``TERRAIN_UV_RECT`` measured on the tinted JPEG. Period 48 / stream retry / LOD_CELLS=6 stay. Relic Run keeps the uncropped JPEG / default UV. No Rapier / SSAO / Repeat / per-tile 0..1.

- World3D terrain stream: failed ``upload_mesh_3d`` no longer sticks as loaded (bald tile skip-forever). Streaming worlds prefetch a 1-tile ring and delay unload one frame; LOD upgrades beat brand-new far tiles (still 1 GPU upload/frame while walking). Crest Isle meadow UVs: ``TERRAIN_UV_PERIOD=48`` (3× TILE, world-continuous, pad 0.28) so a 16 m chunk is a small 2D moss window — period 9.5 < TILE + ``lod_cells=3`` was the barcode / 1-axis JPEG stretch. ``LOD_CELLS=6``. Relic Run UV defaults unchanged. No Rapier / SSAO / Repeat sampler / per-tile 0..1 UV.

- Crest Isle bald meadow: `aerial_grass_rock_diff_1k.jpg` is a non-tiling aerial photo (mossy interior, dirt at UV 0/1). ClampToEdge + ping-pong of the full 0..1 range stamped that square as a grass island. Crest Isle now pads UVs past the dirt rim (`TERRAIN_UV_PAD=0.28`) with a world-continuous period ≠ TILE. Relic Run keeps default UV. No Rapier / SSAO / Walk.wish / Mesh3D pin revert.
- Character controller (no Rapier): `Walk.wish` / `Walk.move` / `Walk.try_jump` and `kagra.CharacterController`. Accel/decel (defaults 14 / 22 m/s²), ground stick, 8-point foot ring, bump-raise only for real ledges (one-sided max-Y on a slope was the remaining float — not missing Rapier). Capsules skip ground friction so the motor owns stop. Static prop lips ≤ `step_height` are climbable. Crest Isle constructs `Walk(..., controller=CharacterController(...))`. Sticky-walk quiet gap 3 is unchanged (input). Wheel size: no Rapier crate.

- Crest Isle picture pass (black trees / fake AO / terrain seams / CPU sparks / hair rim / gold orbs): Kenney forest GLBs keep `Textures/colormap.png` next to the `.glb` (no Blender re-export). `flatten_gltf` / Rust `extract_texture_data_from_glb` resolve that directory-relative URI; `KHR_texture_transform` is applied to atlas UVs (Kenney currently ships `{texCoord:0}`). Character blob is a soft Y-quad under the feet (`quad_y_mesh` + skip_fog), not SSAO. Stream tiles sample height *outside* the chunk for shared normals and Crest Isle blends/pads meadow UVs so the 16 m join is not a grass/dirt knife. Pickups spawn a CPU `{position, velocity, life, fade}` burst drawn with `draw_billboard_instances`. VRoid / Alicia **hair** MToon gets a stronger rim (`Hair` / `髪` names, warm color, lift 0.62); face/skin stay off that path and backfaces stay flipped so the skull does not white-mask. Crest coins are gold PBR spheres (`metallic=1`, `roughness=0.12`), not Kenney yellow discs. `apply_outdoor_look` does not raise global `set_rim`. Sticky-walk quiet gap 3, Mesh3D LRU 256, chase cam clamp, opaque title, spatial listener, `set_locomotion` / Mixamo `bind_locomotion` unchanged. No Rapier / SSAO / 4-cascade CSM / volumetric fog / visual editor / Web/XR / Mixamo binaries.

- Mixamo FBX locomotion on VRoid: rest-pose + bone-roll retarget so Idle/Walk/Run hang and swing instead of folding into a carry pose (T-pose Emma and A-pose Alicia). `avatar.bind_locomotion()` loads local Mixamo FBX into the existing `set_locomotion` mixer (never the `walk` alias / `synthetic_walk.bvh`). `dance()` stays a full-body drop path. No Mixamo binaries in git.

- Multi-avatar GPU share: same-path `kagra.avatar()` / `load_vrm()` clones Arc-share mesh / texture / MToon. Joint palettes, pose, expressions, and SpringBone stay per instance. Mesh3D LRU (256, never evict live diffuse) is still Props-only — a second Alicia does not multiply that cache. Measure with `kagra.vrm_gpu_stats()`. Spawn extras in `examples/vrm_multi_avatar.py` (`KAGRA_AVATARS=N`). Crest Isle play stays one player (title / input / camera). No Rapier / Mixamo / spatial audio / terrain retune.
- Crest Isle spatial audio: engine listener + world sources (`set_listener`, `play_se(..., x=, y=, z=)`, `play_loop`). Inverse-distance gain and equal-power stereo pan (no HRTF). Crest Isle loops a procedural sea drone at west water; crest/coin pickups play at the collectible. Title/start/win stay 2D. `sound()` unchanged. No Rapier / Mixamo / locomotion / multi-avatar / CSM.
- Crest Isle locomotion: `avatar.set_locomotion(speed)` blends built-in idle/walk/run (no Mixamo). Start/stop and analog stick no longer hard-cut the clip. `play_upper` / ActionController own spine/arms while legs keep walking (overlay does not mutate locomotion `current_rots`). `walk_wish` keeps analog magnitude. Mixamo/BVH walk still skipped (folded arms). No Rapier / terrain retune / spatial audio / multi-avatar.
- Sleeve / cloth stiffness on VRM SpringBone (Crest Isle Alicia / Emma-style MToon). Verlet now matches UniVRM / three-vrm (`stiffness * dt²` along the rest axis) so authored hair/skirt no longer glue (stiffness=1) or flop like paper (stiffness>1). VRM 0.x leaf bones (ribbons) get a 7cm virtual tail instead of being dropped. Models with no sleeve helper bones (Alicia Solid) get four arm-parented helpers; outer-tube weights transfer so the sailor sleeve lags as fabric. VRoid `*Sleeve*` / 袖 bones already in the file are chained, not duplicated. No Rapier. Crest Isle terrain / Mixamo walk / blend trees / spatial audio / multi-avatar are unchanged.
- Crest Isle look (on top of #82 cache/stream): chase cam clamps eye distance so wall-clip / hitch lerp / orbit zoom cannot fly to a tiny speck, slam into the VRM skull, or fog-white the world. Meadow `mesh_mat.base` tint is Crest-only so the shared aerial grass JPEG reads green without blowing white. Sky/backdrop pass `draw_mesh_3d(..., skip_fog=True)` so puresky is unlit and unfogged; fog-off is not inferred (that unlit every Mesh3D and broke goldens). MToon flips backface normals so inside-skull hair/face does not rim-blow to a white mask. Long-hold leftover KEYDOWN still uses #82's refresh window, but the quiet gap is 3 frames (~50ms) not 15 (~250ms) so a real same-key press after ~3 silent frames walks. Taps keep #80's 1–2 frame window. Does not revert #81 IBL `albedo * 0.35`.
- Sticky Walk after key-up on Windows: ignore post-`WM_KEYUP` KEYDOWN (`repeat=false`, which #71 missed), pair IME/JIS `Unidentified`/`Process` releases to WASD/arrows, disable IME on the game window, and stop leftover vx/vz when wish is 0 (steep slide kept). GPU-free tests cover the remaining path, not only #71.
- Slope grounding: tight foot AABB (0.08) + extra samples + snap-to-plane. `|foot_y − terrain|` while `on_ground` stays under 0.05 (`debug_trace`). Fat capsule AABB max-Y on a slope still floats — measure first. Still no Rapier.
- `kagra.stage` is the documented callable again (it was shadowed by `kagra/stage.py`). Same guard for `annotate` / `pad` / `brain`. Crest Isle / Relic Run sky spheres no longer TypeError.
- Crest Isle: init `_chunk_props` before `bake_terrain` so the first tile stream does not AttributeError.
- Agent eyes: `kagra.annotate` (click → JSONL) and `kagra.debug_trace` (foot vs terrain). Not a visual editor.
- `Camera3D.follow(..., world=)` pulls the chase camera in so it does not go through walls.
- Prop / terrain Lambert uses the same `cam.toon` stepped lighting as VRM when `set_toon_params` softness < 0.999.

## 0.1.4

Playable 3D, picture pixels, and `kagra.brain` on the wheel. Windows / Linux,
~5MB, no Rust. Rapier crate stays out. 30-second stranger demos are not this
release. Do not call 80% or three.js-class.

### 3D play

- `Prop` / `Walk` / `sky` / `room` / `World3D`. First-person, hover, carry,
  kinematic move, `destroy`, sphere/cylinder hit, 4-level parent, glTF parts
  (`Prop("crate.glb")`).
- Heightfield island, city JSON (not OSM), triangle mesh hit, tiled terrain.
- AABB rigid boxes: `add_box(..., is_static=False)` fall, stack, and `Walk`
  stands on them. Character vs crate does not sink the crate.
- Gamepad: `axis` / `pad` / `inject_pad`. USB/XInput via gilrs on the
  EventLoop (Windows: one loop). Real-hardware 30s not claimed.
- Agent games: Heart Catch, Switch Room, Dodge Room (logged). Orb Rush is
  the reference (no generation log).

### Picture

- Four local lights (`slot=0..3`; slot 0 is the key / shadow). Indoor spot
  umbra, 2-cascade outdoor shadows (per-layer light-VP), tangent-space
  normals, HDRI / irradiance, generic metal/roughness, opt-in ACES.
- Pairwise goldens on Windows CI: `indoor_spot`, `normal_bump`, `local_four`,
  `outdoor_crawl`, `tonemap_on`, `ibl_metal`.
- World mesh casters, VRM cull, 3D instancing, material sort.

### Brain

- `kagra.brain("kairi"|"ollama"|"openai")`. Default kairi is
  `https://kairi.onrender.com` (`KAIRI_API_TOKEN`). Models stay out of the
  wheel.

### Docs / install

- README / README.ja: `pip install kagra` is 0.1.4. Not yet is the remaining
  holes (macOS wheels, 30s demos, chat APIs, NDI/RTMP, autopilot, bundled
  TTS, pointer lock). Tilemaps / ECS stay on the shelf.

## 0.1.3

Streaming-V slice. OBS can take a virtual camera; VOICEVOX and chat stay
outside the wheel. WAV / VRMA still download on first run (keeps the ~5MB
install).

- Virtual camera extra: `pip install "kagra[stream]"` (`pyvirtualcam`).
  GPU `set_grab_frames` / `grab_frame()` returns RGB. 720p recommended —
  readback is every frame. `python -m kagra --loop --stream`.
- Stream HUD: song title, subtitle, recent chat via existing 2D text
  (`StreamHud`).
- Chat inbox: JSONL `{user,text}` (`ChatInbox`). YouTube / Twitch APIs are
  not in the engine — an external script writes the file.
- Official VOICEVOX recipe (`docs/recipes/voicevox.md`).
  `avatar.speak_voicevox("こんにちは")` uses mora lipsync. Engine not bundled.
- Mic extra: `pip install "kagra[mic]"` (`sounddevice`) → `MicLipsync`.
- Bloom: give extract / blur / composite their own uniforms. A single
  `write_buffer` was applied only after submit, so every pass used
  `intensity` as the extract threshold and smeared the whole frame
  (ghost trails next to the VRM).
- 3D physics: Y-up capsule, yaw OBB, layer/mask, triggers, `physics.sync_vrm`.
  Character-controller style (no Rapier). GPU-free tests in `tests/test_physics3d.py`.
- `Camera3D.ray_from_screen` and `avatar.pick(sx, sy)` → humanoid bone name
  (`head`, `leftHand`, …). Bone world spheres; gesture recognition stays out.
- Threshold bloom: extract only high-luminance pixels (eye highlights, outline,
  MToon rimLift), blur that, add back. Full-screen blur is not used — it muddies
  toon edges. `kagra.set_bloom(threshold=0.85, intensity=0.35)`.
- VRM SpringBone colliders (0.x spheres / 1.0 spheres and capsules). Hair and
  skirts now push off the body instead of passing through.
- `VRMC_node_constraint` 1.0: rotation, roll (twist bones), and aim. Applied
  after pose, before skinning.
- VRM 1.0 expression `overrideBlink` / `overrideMouth` / `overrideLookAt`
  (`none` / `block` / `blend`) and `isBinary`. A smiling face can now suppress
  blinks the way the author specified.
- First-person layer: `avatar.first_person = True` / `kagra.set_vrm_first_person`.
  `Auto` meshes drop head-weighted triangles; `ThirdPersonOnly` hides in FP.
- MToon: matcap, UV scroll/rotate (optional mask), and normal maps.
- glTF textures: BMP / TGA / GIF / TIFF in addition to PNG / JPEG. Unknown
  MIME types fall back to magic-byte sniffing.
- SpringBone Verlet + colliders run in `kagra-core` after pose (Python keeps
  parse tests and a fallback). Hair / skirt chains no longer do FK + FFI per
  joint on the Python side.
- Lipsync: `timeline_from_audio_query` / `play_audio_query` consume VOICEVOX
  mora timings instead of throwing the `audio_query` away. WAV Goertzel remains
  the fallback when there is no query.
- Live body ingest: `avatar.apply_pose({name: quat})` / `kagra.set_vrm_pose`.
  Capture stays outside the engine (VR / Kinect / Holistic).
- Drop-in venues: `kagra.stage("venue.glb")` / `kagra.load_gltf` now load
  JSON `.gltf` plus sibling `.bin` / image URIs (not just GLB). A PNG/JPEG
  path becomes an inverted sky sphere. `python -m kagra --stage` /
  `--backdrop` use `assets/stage.glb` when present, else the checkerboard.

## 0.1.2

- Renderer: each skinned draw now gets its own bone-matrix palette. Multi-skin
  VRMs (e.g. the Alicia Solid sample, 12 skins) rendered every mesh with the
  last-uploaded palette, so arms, hands, fingers, and legs stayed frozen in
  bind pose no matter what the animation sent. Single-skin VRoid exports were
  unaffected, which is why it went unnoticed.
- The bundled demo dance is now full-body: alternating arm swings (front and
  side), wrist flicks, head bob and tilt, hip sway, and small steps in place.
  Previously only the spine and both forearms moved.
- `VrmAvatar.relax_hands()` gently curls the fingers. `dance()` applies it
  automatically when a clip has no finger data, so hands no longer look like
  splayed bind-pose paddles during BVH dances.
- sdist: ship `LICENSE` in the tarball. PyPI rejected uploads whose metadata
  lists `License-File: LICENSE` but the file is missing (`v0.1.0` / `v0.1.1`
  wheels still published; only the `.tar.gz` failed).
- README: `pip install kagra` is the full product. Only webcam face tracking
  is an extra (`kagra[facetrack]`); Mac still has no wheel.

## 0.1.1

- Windows first-run: `python -m kagra` loaded the VRM before `run()`, so
  `Renderer not initialized`. `kagra.run(..., on_ready=)` now fires after the
  GPU exists; the demo / README / examples load `avatar()` there.
- `kagra.load_vrma()` / `avatar.load_motion(..., "*.vrma")` / `av.dance("wave.vrma")`
  — VRM Animation (`VRMC_vrm_animation` 1.0). Humanoid retarget (including
  fingers), LookAt yaw/pitch → eye blendshapes, and preset expressions.
  Files from [text-to-vrma](https://github.com/Kirakun0328/text-to-vrma) play as-is.
  See `examples/vrm_vrma.py`.
- Publish / CI: drop macOS runners. Wheels are Linux + Windows only until a Mac
  can verify them (macos-13 also sat queued and blocked `v0.1.0` upload).
- Publish: Linux wheels now request CPython 3.10–3.12 inside manylinux_2_28
  (v0.1.0 failed with “Couldn't find any python interpreters from 'python3'”).

## 0.1.0

First public-facing cut.

- `pip install kagra` wheels via tag-triggered publish (`v*`)
- `python -m kagra` / `kagra` — sing & dance demo; downloads a sample VRM once
- `VrmAvatar.sing()` / `dance()` and a built-in song synthesizer
- `kagra.line()` no longer raises `NameError`
- Engine no longer writes `keymap.json` into the working directory
- Examples use `kagra.font()` instead of a Windows-only Meiryo path
