# Changelog

## Unreleased

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
