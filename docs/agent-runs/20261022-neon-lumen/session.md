# Lumen

- Branch: `cursor/neon-lumen-run-6250`. Fast-forwarded through #137/#138, then merged `origin/master` at #139 (`b99dabf`). No force-push.
- Sim is `NeonRun` in `kagra-shared`. `SharedSession::load_game_json` intercepts `genre: neon_run` before `WorldPlay`. Not a WorldPlay genre.
- Glow is dump `emissive` and `emissive_map: preset:core` (ship core and orbs). Nose and islands are metallic. Reflections are `hemi_sky` / `hemi_ground` and `env_map: preset:studio`. `shadow_softness` is 3. A separate emissive material and a custom sky gradient were dropped.
- Night picture matches `apply_look(world, "night")`: sky `[10, 16, 36, 255]`, cool LUT, bloom 0.72, vignette 1.35, ibl 0.55. The window calls `apply_look` (that also scales light intensity). The dump sets the same sky, hemisphere, post, `cull: true`, and `prop_lod: 80` so far rings stay meshes. `prop_lod` 48 would turn them into cards and drop the glow.
- The key spot stays behind and above the camera. A spot on the eye puts the shadow frustum through the ribs; the compare returns NaN and bloom wipes the frame.
- Distance score is an `f32` accumulator. `speed * dt` truncated to 0 every frame and the result shot showed SCORE 0.
- `kagra.Trail` is the shared quad ribbon (`kagra.particles.Trail`). It measures length in XY and faces +Z, so a flyer that only moves in Z emitted nothing. The window keeps ship samples and folds `(head_z - z) * 0.40` into Y before `Trail.push`. Wasm cannot run that class, so the dump emits the same quads. The window drops those and draws `Trail` itself. The old sphere wake is gone.
- `Emitter.emissive` is the burst. Python juice and `kagra.audio` wrap the sim for the window. Wasm plays `audioCuesJson`. Clips are the CC0 tones in `assets/audio/LICENSE.txt`.
- Pages default is `kagra-shared/www/play.html`. `index.html` sends you there. Corridor Haul is `corridor.html`. `scripts/package_web.py` stages the site. The Pages workflow runs on push to master only.
