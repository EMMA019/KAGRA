# Lumen

- Branch: `cursor/neon-lumen-run-6250`, fast-forwarded onto `origin/master` (`f57c017`, #137 and #138) before the game commit. No force-push.
- Sim is `NeonRun` in `kagra-shared`. `SharedSession::load_game_json` intercepts `genre: neon_run` before `WorldPlay`. Not a WorldPlay genre.
- Glow uses dump `emissive` and `emissive_map: preset:core` (ship core and orbs). The nose and islands are metallic so `env_map: preset:studio` reflects `hemi_sky` (cyan) and `hemi_ground` (magenta). `shadow_softness` is 3. A separate emissive material and a custom sky gradient were dropped after the merge.
- Phone post (bloom, punch LUT, vignette, FXAA) is applied on the live draw path when the dump sets those fields, so wasm matches the offscreen readback. Omitted fields leave the host grade alone.
- `kagra.Trail` and `Emitter.emissive` append sphere props. Python juice and `kagra.audio` wrap the same sim for the window. Wasm plays `audioCuesJson` in the page. Clips are the existing CC0 tones in `assets/audio/LICENSE.txt`.
- Pages default is `kagra-shared/www/play.html`. `index.html` sends you there. Corridor Haul is `corridor.html`. `scripts/package_web.py` stages the site. The Pages workflow runs on push to master only.
