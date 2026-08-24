# Changelog

## Unreleased

- Indoor-spot pairwise golden: larger box, more overhead camera, umbra
  fills enough of 320x180 that CI `mean_abs` is not 0.6 (threshold 2.0).
- Roadmap: engine target is **80%** (now ~33%). 100% means a Python replacement
  of three-vrm + three.js + Ursina for everyday work, not the whole three.js
  repo. Body is already ~80%. Picture 25→85 is the bulk (indoor shadow pixels
  first). Writing 60→80 via 30s demos and 4-level parents. World 25→55 via
  rigid bodies (Rapier or equal). OSM / 4-cascade CSM / SSAO stay outside 80%.
  First-recall stays the north star; 80% is not a substitute. Do not call 80%
  until pixels and demos say so.
- Indoor spot shadows the **local light**, not the directional sun
  (`shadow_u.params.y`). Fill from the sun stays; the lamp umbra reads.
  Pairwise golden `indoor_spot` / `tonemap_on` / `ibl_metal` (CI `golden`
  job; no committed PNGs). ACES still default-off. Garden smoke unchanged.
  Outdoor crawl pixels still open.
- Generic Mesh3D tangent-space normals: `upload_mesh_3d(..., normal_texture_id=)` /
  `set_mesh_normal` / `Prop(..., normal=)` / glTF `normalTexture`. Vertices stay
  `[x,y,z,nx,ny,nz,u,v]` (cotangent frame, no extra stride). Normal maps load
  linear (`texture_from_fn(..., srgb=False)` / `load(..., srgb=False)`). Pretty
  Room brick wall and Prop Garden bump crate opt in when not `KAGRA_SMOKE`.
  GPU pixels unverified here.
- USB/XInput: `poll_pad` reads gilrs from the winit EventLoop (Windows: one
  loop). Stick Y is down-positive like `Walk` / `VirtualPad`. `inject_pad` still
  wins for tests/smoke. Linux CI installs `libudev-dev`.
- Walk strafe matches the camera: `walk_wish` right is screen-right
  (`forward × up`). D / left-stick X no longer move the opposite way in
  Overworld / Prop Garden / Pretty Room. Forward (W) is unchanged.
- Stage 0.5 brain hook: `kagra.brain("kairi"|"ollama"|"openai")` /
  `KairiBrain.ask`. Default kairi is `https://kairi.onrender.com`
  (`KAIRI_API_TOKEN` required for `/api/chat`). Host is pinged ~every 10
  minutes so it stays up. Local override `KAIRI_URL`.
- Roadmap refresh: Stage 0.5 brain (`kagra.brain`) is first for wedges A/B,
  in parallel with usable-week pixels. D-6 stays gated on the 30s demos and
  must be playable 30s+ with a score or goal. Wedge C is a one-liner, lower
  priority. Next PyPI should carry usable-week APIs. Star/DL targets are
  reference at Stage 1. `KAGRA_ENGINE_GUIDE.md` body moved to `docs/archive/`.
- Roadmap tidy: the north star (first-recall) is the final goal. The usable
  week is the current engine bar, not a ceiling. Rapier / OSM / extra CSM
  are deferred, not banned. Position after #52 / #53: APIs landed, pixels
  and normals / USB pad still open.
- Usable-week APIs (not the stranger test yet; GPU unverified here):
  `set_tonemap` (ACES, default off), specular cube mips, spot perspective
  shadow, cascade texel snap. `Walk` pointer lock / coyote / jump buffer /
  `carry`. `clicked_prop`, `animate` / `sequence`, `Label` / `Button`,
  2-level parent, `sound()`. Pretty Room / Overworld / Prop Garden opt in
  (`apply_room_look` / `apply_outdoor_look`). Garden smoke stays without
  tonemap. Not normals, not USB/XInput. Rapier / OSM / 4-cascade CSM were
  not this slice (deferred, not banned).
- Roadmap: the usable week is the current engine bar, not Stage 1 posting
  and not the product ceiling. P0–P8 and room v1 stay foundation. Done
  means a stranger would watch 30s of Pretty Room, Overworld, and Prop
  Garden. D-6 waits on that; brain can proceed in parallel for wedge A.
  Later engine (Rapier / OSM / extra CSM) stays deferred, not banned.
- City JSON, mesh hit, stacking, 2-cascade shadows: `load_city` / `city_chunk`
  (not OSM), `Physics3D.add_trimesh` / `Prop(..., mesh_hit=True)`,
  `add_box(..., is_static=False)` with solver iters + sleep (not Rapier),
  `set_shadow_cascades(2)` (default 1 so Prop Garden stays). Demo:
  `examples/vrm_overworld.py`.
- Heightfield slopes + tiles: walk along the tangent (slide on steep
  grades), not Y-snap only. `height_normal` / `stair_y`. Terrain bakes as
  tiles (`tile=10`) so AABBs stay under the shadow skip (24) — nearby
  ground casts; still one ortho, no CSM. `stream_radius` load/unloads
  tiles while walking. `set_chunk_fill` / `city_boxes` plant box blocks
  (not a city file). Stairs are heightfield steps. Not Rapier, triangle
  mesh hit, or stacking. Demo: `examples/vrm_overworld.py`.
- Heightfield island: `World3D.set_height_fn` / `kagra.island_height` /
  `water()` / `Walk(..., jump=)`. Sea, grass, and a hill. Cliff grade is
  blocked; water has buoyancy. Demo: `examples/vrm_overworld.py`.
- Pretty room (picture leftover): `room()` builds an enclosed floor / walls /
  ceiling (`sky()`'s indoor sibling). `apply_room_look` sets a ceiling
  `set_spot_light`, studio HDRI, and `set_exposure` (default 1 = identity).
  Diffuse IBL uses a small irradiance cube (PMREM-lite); spec still samples
  the sharp cube. Point and spot share one local-light slot; neither casts
  a shadow. Demo: `examples/vrm_pretty_room.py`. Prop Garden smoke pixels
  are unchanged.
- Picture track P6–P8: `set_point_light` (one point, no shadow),
  `set_hdri("studio"|path)` (cube, no PMREM), generic mesh metal/roughness
  via `upload_mesh_3d(..., metallic=, roughness=)` / `set_mesh_pbr` /
  `Prop(..., metallic=)`. glTF flatten reads `pbrMetallicRoughness`.
  MToon is unchanged. Defaults keep Lambert (metal 0, rough 1, lights off).
  Prop Garden non-smoke: chrome sphere + studio HDRI + a point light.
- World shadows (P5): directional map fits VRM + floor / box / Prop AABBs
  (half clamp 28). Immediate, retained, and instanced Mesh3D write the map.
  Sky-sized meshes are skipped. Still one light, no cascades.
- Gamepad API: `axis("left"|"right")`, `pad("a")`, `pad_pressed`, `inject_pad`.
  `Walk` reads the left stick to move and the right stick to look.
  Tests/smoke use `inject_pad`. OS USB/XInput poll is not in the wheel yet.
  Prop Garden: Start toggles view, A deletes.
- glTF as a Prop part: `Prop("crate.glb")` (or alias `cube.glb`). Flattened to
  the static mesh pipeline — not `stage()` / `load_gltf`. Hover and collision
  use the mesh AABB. Bundled unit cube at `kagra/data/unit_cube.glb`.
  Prop Garden places one when not `KAGRA_SMOKE`.
- `Prop` texture (`texture_from_fn` / `load` id) and 1-level parent
  (`set_parent` / `parent=`). Child `x,y,z,yaw` are local; `world_x` /
  hover / collision use the world pose. Grandchildren raise. Destroying a
  parent destroys children. Prop Garden: checker crate + green child on
  gold (skipped under `KAGRA_SMOKE`).
- Sphere / cylinder colliders match the mesh: `World3D.add_sphere` /
  `add_cylinder`, capsule vs circle/disk, hover rays skip AABB corners.
- Kinematic `Prop`: assign `x`/`y`/`z`, `set_position`, `vx` + `Prop.update_all(dt)`.
  `destroy(p)` / `p.enabled` drop draw, hover, and collision. Prop Garden: gold
  bobs; `E` deletes the hovered prop (smoke path unchanged).
- First-person `Walk(first_person=True)` (`Camera3D.look`, mouse pitch) and
  `hovered_prop(cam)` (skips floor `plane`). Prop Garden: F toggles view.
- Engine review + roadmap refresh (`docs/REVIEW.ja.md`, `docs/ROADMAP.ja.md`):
  three-vrm-class body vs three.js picture vs Ursina play surface. Stage 0
  no longer claims `KairiBrain` is done. `KAGRA_ENGINE_GUIDE.md` is marked
  historical.
- Ursina-shaped play surface: `Prop` (box / sphere / cylinder / plane),
  `Walk` (WASD + mouse look), `sky()`, `solid_tex`, `sphere_mesh`,
  `cylinder_mesh`. Not the 2D `Entity`. Demo: `examples/vrm_prop_garden.py`
  (play surface, not an agent-built log).
- 3D mesh frustum culling for `draw_mesh_3d` / `draw_mesh_id` (World3D
  boxes). Last-frame stats via `kagra.render_stats()`; toggle with
  `set_mesh_cull()`.
- VRM primitives cull with padded per-bone AABBs (P1). Spring / morph
  motion is padded so dance should not pop.
- 3D instancing: `draw_mesh_instances` / `draw_billboard_instances`.
  World3D boxes and Dodge / Orb / Heart Catch sprites batch (P2).
  2D `InstanceBatch` is unchanged.
- VRM materials sort by texture; `doubleSided` / VRM0 `_CullMode==Off`
  is the only two-sided path (P3).
- Shadows: 2048 map, ortho fitted to visible VRM AABBs (P4), then world
  casters (P5). Hemisphere ambient via `set_ambient` / `apply_live_look`.
  Not HDRI / cascades.
- README / samples list Dodge Room (`examples/vrm_dodge_room.py`) as the
  third logged agent-built game (`docs/agent-runs/20260823-dodge-room/`).
  Mixamo `.fbx` is `av.dance("clip.fbx")` / `--dance` (YMCA sleeve blow-up
  skipped Mixamo finger axes).
- `*_rules.py` helpers print the real launcher (`vrm_*.py`) instead of
  exiting silently when double-clicked.
- Repo shelf-split: recommended examples stay in `examples/`; legacy 2D /
  tilemap / editor / romance / boids moved to `examples/archive/`. API index
  now has Front (VRM / 3D / agents) vs Shelf. `kagra-shared` + `mobile/` is
  documented as a separate driving demo — renderers are not merged.
- 3D world surface: `upload_mesh_3d` / `draw_mesh_id` (GPU retain),
  `box_mesh`, `World3D` (floor + static boxes + capsule), `Camera3D.follow`.
- Agent-built world game (wedge D): `examples/vrm_switch_room.py` from the
  one-line prompt in `docs/agent-runs/20260823-switch-room/`. Not a
  disc-collect — walk a boxed room and stand on a switch.
- Agent surface (wedge D-3): `ActionController` is a public export with
  `names()`. API index notes 2D vs 3D `world_to_screen`, `save_json` vs
  `load_data`, and `ensure_vrm()`. Recipe `docs/recipes/agent-game.md`.
- Agent-built game log (wedge D-2): `examples/vrm_heart_catch.py` from the
  one-line prompt in `docs/agent-runs/20260823-heart-catch/`. Orb Rush
  smoke JSON now points at the real example (`KAGRA_SMOKE=1`).
- Game surface for agents (wedge D-1): `Camera3D.world_to_screen`,
  `VrmAvatar.set_position` / `set_yaw`, `texture_from_fn`, `tone`,
  `billboard_mesh` / `disk_mesh` / `quad_y_mesh`, `save_json` / `load_json`.
  `examples/vrm_orb_rush.py` now uses only public APIs (no `_` imports).
- First-run: `python -m kagra --vrm me.vrm --song my.wav` on the README;
  checkout `kagra/` shadow prints `cd %TEMP%` / `maturin develop`.
- Compare table (install / code / license / AI hook only) vs UniVRM,
  VSeeFace, three-vrm. Recipes: own VRM, motion, mascot.
- Issue templates for bugs and “it worked” VRM reports.
- CI: macos-14 × 3.12 maturin build + import smoke (publish still
  Windows + Linux until this stays green).
- Live look (Phase 1a): `python -m kagra` no longer opens on a solid purple
  void. Procedural gradient sky, a dark disc + warm spot (no checkerboard),
  vignette, `apply_live_look()` (key light + toon + bloom + fog), and
  `set_rim()` (view fresnel + backlight + floor bounce; 0 keeps goldens).
- `Camera3D.use_showcase()` — slow orbit with full-body ↔ face cuts.
  `--no-orbit` still freezes the camera.
- Default HUD always shows the KAGRA brand + `♪` song title (`StreamHud`).
- WAV lipsync: less “aa” bias so sung vowels read as A/I/U/E/O. `sing()`
  smoothing is lighter.
- `VrmAvatar.dance()` enables foot grounding (lift the root when a foot
  goes through the floor; no push-down).

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
