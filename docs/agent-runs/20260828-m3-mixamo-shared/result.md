# Result

## Commands

```text
cargo fmt -p kagra-shared
# ok

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok
cargo clippy -p kagra-shared --all-targets --features render --offline --locked -- -D warnings
# ok

cargo test -p kagra-shared --locked --offline --lib
# 305 passed; 0 failed (Mixamo rest+roll, clip-less tpose_humanoid walk,
# Walk clip kept, Crest stays capsule, WASD clip dump-visible)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 571 passed, 10 deselected
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/mixamo_walker_world.json
python -m kagra.play_world kagra-shared/tests/fixtures/vrm_walker_world.json
python -m kagra.play_world kagra-shared/tests/fixtures/skinned_walker_world.json
```

Space / Enter / click starts. WASD walks. Clip-less `tpose_humanoid.vrm` retargets bundled Mixamo walk (rest+roll, not raw bind*delta). Dump `clip` advances while walking; release returns to bind pose (clip 0).

`walk_skinned.gltf` / `walk_skinned.vrm` keep their own Walk clip.

Point a dump `gltf` at `assets/Emma.vrm` on this PC to retarget the same Mixamo walk onto Emma (T-pose, no clip). Crest Isle (`python -m kagra.play_world`) stays capsule.

## Notes

- No Mixamo FBX, no Emma.vrm in git. JSON clip is tiny (subsampled walk.fbx deltas + src worlds).
- CPU-skin still writes Vertex3. No storage buffers, Rapier, SSAO, second renderer, new ECS.
- Remaining: MToon, spring bones, look-at, blendshapes / morph.
