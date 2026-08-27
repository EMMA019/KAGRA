# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 196 passed; 0 failed (4 new: empty key+fill, crest keeps dump lights, bare coin Metal, contact blob)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

cargo test -p kagra-shared --locked --offline --features render --test offscreen_render
# world_doc_offscreen_crest_isle_is_not_flat ok
# world_doc_offscreen_orb_rush_draws_batches ok
# driving_scene_has_sky_road_and_truck failed on parent too (29 grass px); not this slice

pytest tests -m "not golden"
# exited 0

python -m kagra.play_world kagra-shared/tests/fixtures/crest_isle_world.json --width 640 --height 360 --seconds 2
# opened via rebuilt target/debug/examples/window.exe, exited 0

python -m kagra.play_world kagra-shared/tests/fixtures/orb_rush_world.json --width 640 --height 360 --seconds 2
# ok (empty dump now has default key+fill)
```

## Try

```text
python -m kagra.play_world
python -m kagra.play_world kagra-shared/tests/fixtures/orb_rush_world.json
```

Default Crest: gold coins pick up GGX from key+fill (not yellow plastic), capsules/crate sit on a dark ground contact blob, island lights unchanged (dump already has key+fill+rim). Orb rush (empty lights): key spot + cool fill instead of a sun-only Lambert void; stars/bomb/walker get contact blobs.

## Notes

- Shared `compile_scene` only. Every genre dump inherits. `world_play.rs` genre loops not rewritten.
- Empty lights → slots 0 key + 1 fill; 2 and 3 stay OFF. Explicit dumps (Crest, stealth, novel, …) unchanged.
- Contact blob is `MESH_PLANE` + instance alpha, not SSAO / V2 umbra / a second shadow pass.
- Metal shader: GGX locals, coin defaults metallic=1 / roughness=0.12. No per-instance roughness API (Material enum already the hook).
- Did not land: SSAO, GI, RT, Nanite-like, a new renderer, VRM skin, picture on RendererV2, Rapier, 0.19+30 mix, merging master.