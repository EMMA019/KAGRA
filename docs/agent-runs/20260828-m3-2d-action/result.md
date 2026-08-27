# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 222 passed; 0 failed (7 action2d tests: dump, title, walk +X, hit/hurt dump, kill disable, contact retry, other genres)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 562 passed (2 new python tests: action_side fixture + smoke scenario)

python -m kagra.play_world kagra-shared/tests/fixtures/action_side_world.json --width 640 --height 360 --seconds 2
# opened via rebuilt target/debug/examples/window.exe, exited 0
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/action_side_world.json
```

Space / Enter / click starts. A/D or W (`--seconds` injects W) walks the player card along +X on the 2D plane. J / click hits the foe sprite. Hurt is dump-visible `name` (hurt/dead) plus overlay flash; kill disables the foe sprite in the dump. Side camera stays framed on the XY cards against the back wall/floor. `--seconds` walks W and attacks so the hit is visible without a human.

3D action arena is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/action_arena_world.json
```

Sprite card (no combat) is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/sprite_card_world.json
```

## Notes

- Path: **2D action slice** on the existing sprite/quad WorldDoc (`MESH_QUAD` in `compile_scene`). Did not invent APIs. `docs/API_INDEX.md` has no `enemy.chase`.
- Combat lives in `action2d.rs`. WorldPlay only dispatches (new/start/tick/hud) and skips 3D `action.rs` for sprite foes. Other genre loops not rewritten.
- Dump source of truth: walker + hero `model: "sprite"` card, foe `model: "sprite"` named `foe`, wall box, floor box, camera name `side`.
- No RendererV2, no VRM, no Rapier, no new ECS, no billboards, no net, no full 2D engine.
- Did not land: camera billboards, sprite-as-FPS-player, foe chase API, Rapier, inventory, extra pages of 2D engine.