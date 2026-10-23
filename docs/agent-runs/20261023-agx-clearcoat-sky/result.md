# Result

AgX, Neutral, clearcoat, and a procedural sky on `cursor/agx-sky-2c16`. Not a genre.

- Dump fields: `tonemap_curve` (`aces` / `agx` / `neutral`), `sky_atmosphere`, prop `clearcoat` / `clearcoat_roughness`. `tonemap` is still the on/off bool.
- glTF `KHR_materials_clearcoat` fills the mesh coat when the dump leaves it at 0.
- Lavapipe (`VK_ICD_FILENAMES` lavapipe): `offscreen_agx_and_neutral_match_the_cpu_curve` and `offscreen_clearcoat_and_atmosphere_change_the_picture` passed. Shots in `scratch/grade/` (not committed): clearcoat on/off, sky gradient vs atmosphere.
- `cargo fmt -p kagra-shared -- --check` and `cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings` passed.
- AgX matrices and the Neutral curve are credited in `NOTICE`. The sky shader is original. No three.js source is copied.
