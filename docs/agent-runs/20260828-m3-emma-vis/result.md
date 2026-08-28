# Result

## Commands

```text
cargo fmt -p kagra-shared
# ok

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok (Rust 1.98.0)

cargo clippy -p kagra-shared --all-targets --offline --locked --features render -- -D warnings
# ok

cargo test -p kagra-shared --locked --offline --lib
# 349 passed; 0 failed
# emma dump: ground Grass plane, Toon skinned slot, not capsule
# missing/bad gltf: load_error dump-visible, tpose_humanoid 8 verts not cylinder
# play HUD: coins=0 → no Crest star pips

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

cargo build -p kagra-shared --features render --example window --locked --offline
# rebuilt target/debug/examples/window.exe

pytest tests -m "not golden"
# ok (includes play_world KAGRA_ROOT)
```

## Why void + capsule

Dump `heightfield` null / `props` [] / `water_y` null → compile_scene emitted no ground (clear sky only + PCF contact blob).
`player.model` is `capsule` (dump style) with `gltf: assets/Emma.vrm`. compile_scene already prefers gltf when the slot loads; the live window still showed the teal/tan capsule because (1) play_world launches the prebuilt examples/window.exe whose cwd can miss repo-root `assets/Emma.vrm`, and (2) parse/missing used to silent-fallback to a primitive instead of dump-visible error + tpose_humanoid. Title else-branch was always Crest Isle; Playing HUD always drew 8 gray STAR_XZ pips.

## Try (local PC)

```text
python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json
```

Should show a VRoid (or tpose_humanoid if Emma.vrm missing) on a grass floor, window title `KAGRA emma_walker_world`, no coin/star pips. No RendererV2.

Did not rewrite `examples/vrm_open_world.py`. No Rapier / SSAO / second renderer / new ECS.
