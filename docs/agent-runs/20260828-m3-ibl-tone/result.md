# Result

## Commands

```text
cargo fmt -p kagra-shared
# clean

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo test -p kagra-shared --locked --offline --lib
# 338 passed; 0 failed (new: outdoor_dump_defaults_ibl_and_aces; metal/toon/emma still green)

cargo test -p kagra-shared --locked --offline --features render --test offscreen_render
# 10 passed including world_doc_offscreen_crest_isle_is_not_flat, orb_rush batches, lighting_shades_faces_differently

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# exited 0
```

## Try

```text
python -m kagra.play_world
python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json
```

Default Crest: key+fill+rim plus SH hemisphere IBL and ACES (not sun-only Lambert). Empty-light dumps (orb rush) keep default key+fill and pick up IBL 0.35 + ACES. Emma VRoid toon + gold GGX coins still the same materials.

## Notes

- Shared `shader3d.wgsl` + `Globals.env` only. No cubemap texture bind (WebGL2). No Bevy crate / source. No RendererV2.
- Dump fields optional; omit = IBL 0.35 / exposure 1 / ACES on. `"tonemap": false` / `"ibl": 0` still work.
- Did not land: water material, SSAO, GI, Rapier, 4K HDR, a second renderer, Python API, vrm_open_world.py rewrite.
